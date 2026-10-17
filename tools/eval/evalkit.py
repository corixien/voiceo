"""Evaluation kit for Voiceo's transcription pipeline ("backtesting" for dictation quality).

Mirrors the Rust pipeline (src-tauri/src/audio.rs + groq.rs) so changes can be measured on a fixed
corpus before porting them. Results of API calls are cached on disk, so re-runs are cheap.
"""
import hashlib, io, json, os, re, sys, time, wave
from concurrent.futures import ThreadPoolExecutor
import numpy as np
import requests

BASE = "https://api.groq.com/openai/v1"
KEY = json.load(open(os.path.expanduser("~/.config/dev.voiceo.app/settings.json")))["api_key"]
SP = os.environ.get("EVAL_DIR", "/tmp/claude-1000/-home-mateo-voiceo/824836f0-71f8-459c-9855-5ecebc61ad70/scratchpad")
AUDIO = f"{SP}/audio"
CACHE = f"{SP}/cache"
os.makedirs(CACHE, exist_ok=True)
SR = 16000


# ---------------------------------------------------------------- caching + http
def cached(namespace, key, fn):
    h = hashlib.sha1(json.dumps(key, sort_keys=True, default=str).encode()).hexdigest()
    p = f"{CACHE}/{namespace}_{h}.json"
    if os.path.exists(p):
        return json.load(open(p))
    v = fn()
    json.dump(v, open(p, "w"), ensure_ascii=False)
    return v


def post(url, **kw):
    for attempt in range(60):
        try:
            r = requests.post(url, headers={"Authorization": f"Bearer {KEY}"}, timeout=120, **kw)
        except (requests.exceptions.ConnectionError, requests.exceptions.Timeout):
            time.sleep(5); continue
        if r.status_code == 429:
            wait = float(r.headers.get("retry-after", 8)) + 1
            if wait > 400:
                raise RuntimeError(f"rate limit, wait {wait:.0f}s: {r.text[:200]}")
            time.sleep(min(wait, 120)); continue
        if r.status_code >= 500:
            time.sleep(3); continue
        if not r.ok:
            raise RuntimeError(f"{r.status_code}: {r.text[:300]}")
        return r
    raise RuntimeError("too many retries")


# ---------------------------------------------------------------- audio (ports of audio.rs)
def load(path):
    with wave.open(path) as w:
        return np.frombuffer(w.readframes(w.getnframes()), dtype=np.int16).astype(np.float32) / 32768


def wav_bytes(x):
    b = io.BytesIO()
    with wave.open(b, "wb") as w:
        w.setnchannels(1); w.setsampwidth(2); w.setframerate(SR)
        w.writeframes((np.clip(x, -1, 1) * 32767).astype(np.int16).tobytes())
    return b.getvalue()


def highpass(x):
    rc = 1 / (2 * np.pi * 80); dt = 1 / SR; a = rc / (rc + dt)
    from scipy.signal import lfilter
    return lfilter([a, -a], [1, -a], x).astype(np.float32)


def normalize(x, target=0.85, maxgain=16.0):
    p = np.abs(x).max()
    if 0.0005 < p < 0.7:
        x = x * min(target / p, maxgain)
    return x


def vad_compress(x, thr_mul=3.5, thr_min=0.006, pad=8, gap_s=0.25, min_speech=12):
    win = SR // 50
    n = len(x) // win + (1 if len(x) % win else 0)
    if len(x) < win * 5:
        return None
    e = np.array([np.sqrt(np.mean(x[i * win:(i + 1) * win] ** 2)) for i in range(n)])
    floor = np.sort(e)[len(e) // 10]
    thr = max(floor * thr_mul, thr_min)
    voiced = e > thr
    segs, start, last, count = [], None, 0, 0
    for i, v in enumerate(voiced):
        if v:
            if start is None:
                start, count = i, 0
            last = i; count += 1
        elif start is not None and i - last > 10:
            if count >= 5: segs.append((start, last))
            start = None
    if start is not None and count >= 5:
        segs.append((start, last))
    if not segs or sum(b - a + 1 for a, b in segs) < min_speech:
        return None
    out, prev = [], None
    gap = np.zeros(int(SR * gap_s), dtype=np.float32)
    for s, t in segs:
        a = max(0, s - pad) * win; b = min((t + 1 + pad) * win, len(x))
        if prev is None: out.append(x[a:b])
        elif a < prev: out.append(x[min(prev, b):b])
        elif a - prev < SR // 2: out.append(x[prev:b])
        else: out += [gap, x[a:b]]
        prev = b
    return np.concatenate(out)


def vad_soft(x, lo_mul=2.2, lo_min=0.0025, pad_s=0.3, merge_s=0.7, long_gap_s=1.0, keep_gap_s=0.4, min_speech_frames=10):
    """Inclusive VAD: only trims leading/trailing silence and shortens pauses longer than `long_gap_s`.
    Everything between speech stays untouched, so quiet word endings are never clipped."""
    win = SR // 50
    n = (len(x) + win - 1) // win
    if len(x) < win * 5:
        return None
    e = np.array([np.sqrt(np.mean(x[i * win:(i + 1) * win] ** 2)) for i in range(n)])
    floor = np.sort(e)[len(e) // 10]
    thr = max(floor * lo_mul, lo_min)
    v = e > thr
    if v.sum() < min_speech_frames:
        return None
    merge = int(merge_s / 0.02)
    idx = np.flatnonzero(v)
    segs, s0, prev = [], idx[0], idx[0]
    for i in idx[1:]:
        if i - prev > merge:
            segs.append((s0, prev)); s0 = i
        prev = i
    segs.append((s0, prev))
    segs = [(a, b) for a, b in segs if b - a + 1 >= 4]
    if not segs:
        return None
    pad = int(pad_s / 0.02)
    out = []
    last_end = None
    for a, b in segs:
        a0, b0 = max(0, a - pad) * win, min(n, b + 1 + pad) * win
        if last_end is not None and a0 < last_end:
            a0 = last_end
        if last_end is not None and a0 - last_end > 0:
            gap = a0 - last_end
            out.append(x[last_end:last_end + min(gap, int(keep_gap_s * SR))] if gap > long_gap_s * SR else x[last_end:a0])
        out.append(x[a0:b0])
        last_end = b0
    return np.concatenate(out).astype(np.float32)


def preprocess(x, hp=True, vad=True, norm=True, soft=False):
    if hp: x = highpass(x)
    if vad:
        x = vad_soft(x) if soft else vad_compress(x)
        if x is None: return None
    if norm: x = normalize(x)
    return x


# ---------------------------------------------------------------- whisper
PRIMER_V1 = "Ja, also ich hab das gestern mal ausprobiert, und es funktioniert eigentlich ganz gut. Danach muss ich noch das Deployment checken, weil die Pipeline manchmal hängt. Okay, passt."


def whisper_raw(x, model="whisper-large-v3", language=None, prompt="", temperature=0.0, tag=""):
    def call():
        files = {"file": ("audio.wav", wav_bytes(x), "audio/wav")}
        data = {"model": model, "temperature": str(temperature), "response_format": "verbose_json"}
        if language: data["language"] = language
        if prompt: data["prompt"] = prompt
        return post(f"{BASE}/audio/transcriptions", files=files, data=data).json()
    key = ["w", model, language, prompt, temperature, tag, hashlib.sha1(x.tobytes()).hexdigest()]
    return cached("whisper", key, call)


def segments_text(v, nsp=0.5, nsp_lp=-0.8, nsp_hard=0.85, lp_min=-1.3, cr_max=2.4):
    segs = v.get("segments")
    if segs is None:
        return v.get("text", "").strip()
    keep = []
    for s in segs:
        a, lp, cr = s.get("no_speech_prob", 0), s.get("avg_logprob", 0), s.get("compression_ratio", 1)
        if (a > nsp and lp < nsp_lp) or a > nsp_hard or lp < lp_min or cr > cr_max:
            continue
        keep.append(s["text"].strip())
    return " ".join(keep).strip()


# ---------------------------------------------------------------- llm
def chat(model, system, user, temperature=0.2, reasoning=None, max_tokens=None):
    def call():
        body = {"model": model, "temperature": temperature,
                "messages": [{"role": "system", "content": system}, {"role": "user", "content": user}]}
        if reasoning and model.startswith("openai/gpt-oss"):
            body["reasoning_effort"] = reasoning
        if max_tokens: body["max_tokens"] = max_tokens
        r = post(f"{BASE}/chat/completions", json=body).json()
        return r["choices"][0]["message"]["content"]
    return cached("chat", [model, system, user, temperature, reasoning], call)


# ---------------------------------------------------------------- metrics
NUM = {w: str(i) for i, w in enumerate("null eins zwei drei vier fünf sechs sieben acht neun zehn elf zwölf dreizehn vierzehn fünfzehn sechzehn siebzehn achtzehn neunzehn zwanzig".split())}
NUM.update({w: str(i) for i, w in enumerate("zero one two three four five six seven eight nine ten eleven twelve thirteen fourteen fifteen sixteen seventeen eighteen nineteen twenty".split())})
NUM["ein"] = "1"; NUM["eine"] = "1"; NUM["einen"] = "1"


def words(s):
    """Content words: lowercase, no punctuation, number words -> digits (so '2' == 'zwei')."""
    s = s.lower().replace("ß", "ss")
    out = re.findall(r"[a-zäöü0-9]+(?:[,.][0-9]+)?", s)
    return [NUM.get(w, w) if w not in ("ein", "eine", "einen", "one") else w for w in out]


def lev(a, b):
    prev = list(range(len(b) + 1))
    for i, x in enumerate(a, 1):
        cur = [i]
        for j, y in enumerate(b, 1):
            cur.append(min(prev[j] + 1, cur[j - 1] + 1, prev[j - 1] + (x != y)))
        prev = cur
    return prev[-1]


def wer(ref, hyp):
    r, h = words(ref), words(hyp)
    return lev(r, h) / max(1, len(r))


def strict_cer(ref, hyp):
    n = lambda s: " ".join(s.split())
    r, h = n(ref), n(hyp)
    return lev(r, h) / max(1, len(r))


def punct_case_acc(ref, hyp):
    """Share of aligned words whose trailing punctuation and capitalization match."""
    def toks(s):
        out = []
        for t in s.split():
            m = re.match(r"^\W*([\wäöüÄÖÜß'-]+(?:[,.][0-9]+)?)(\W*)$", t)
            if m: out.append((m.group(1), m.group(2)[:1] if m.group(2)[:1] in ",.?!:;" else "", m.group(1)[:1].isupper()))
        return out
    import difflib
    r, h = toks(ref), toks(hyp)
    sm = difflib.SequenceMatcher(None, [t[0].lower() for t in r], [t[0].lower() for t in h], autojunk=False)
    ok = tot = 0
    for a, b, n in sm.get_matching_blocks():
        for k in range(n):
            tot += 1
            ok += (r[a + k][1] == h[b + k][1]) and (r[a + k][2] == h[b + k][2])
    return ok / max(1, len(r))


def score(rows):
    """rows: list of (truth, hyp). Returns dict of means."""
    w = [wer(t, h) for t, h in rows]
    c = [strict_cer(t, h) for t, h in rows]
    p = [punct_case_acc(t, h) for t, h in rows]
    exact = [" ".join(t.split()) == " ".join(h.split()) for t, h in rows]
    return {"WER": 100 * np.mean(w), "CER*": 100 * np.mean(c), "punct+case": 100 * np.mean(p), "exact": 100 * np.mean(exact)}


def pmap(fn, items, workers=3):
    with ThreadPoolExecutor(workers) as ex:
        return list(ex.map(fn, items))
