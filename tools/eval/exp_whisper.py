#!/usr/bin/env python3
"""Stage 1: which audio preprocessing / Whisper settings give the most accurate raw transcript?
Reference for WER is the text the TTS actually spoke (pure recognition accuracy)."""
import json, sys
import numpy as np
import os
HOLDOUT = os.environ.get("HOLDOUT") == "1"   # 1 = corpus2 / audio2 (never tune on it)
if HOLDOUT:
    from corpus2 import C
else:
    from corpus import C
import evalkit as E

GLOSS = ("Glossar: Meeting, Call, Deadline, Ticket, Sprint, Review, Pull Request, Commit, Merge, Branch, Feature Flag, "
         "Deployment, Pipeline, Build, Release, Bug, Fix, Backend, Frontend, API, JSON, Server, Cache, Config, Docker, Kubernetes, GitHub.")

VARIANTS = {
    # name: (preprocess kwargs or None, whisper kwargs)
    "base":      (dict(hp=True, vad=True, norm=True), dict(prompt=E.PRIMER_V1)),
    "no_prompt": (dict(hp=True, vad=True, norm=True), dict(prompt="")),
    "nopre":     (dict(hp=False, vad=False, norm=False), dict(prompt=E.PRIMER_V1)),
    "vad_only":  (dict(hp=False, vad=True, norm=False), dict(prompt=E.PRIMER_V1)),
    "hp_norm":   (dict(hp=True, vad=False, norm=True), dict(prompt=E.PRIMER_V1)),
    "turbo":     (dict(hp=True, vad=True, norm=True), dict(prompt=E.PRIMER_V1, model="whisper-large-v3-turbo")),
    "vad2":      (dict(hp=False, vad=True, norm=False, soft=True), dict(prompt=E.PRIMER_V1)),
    "vad2_hp":   (dict(hp=True, vad=True, norm=False, soft=True), dict(prompt=E.PRIMER_V1)),
    "gloss":     (dict(hp=False, vad=False, norm=False), dict(prompt=GLOSS + " " + E.PRIMER_V1)),
    "gloss_vad2": (dict(hp=False, vad=True, norm=False, soft=True), dict(prompt=GLOSS + " " + E.PRIMER_V1)),
    "dn":        (dict(hp=False, vad=False, norm=False), dict(prompt=GLOSS + " " + E.PRIMER_V1, audio_dir="audio_dn")),
    "dn_vad2":   (dict(hp=False, vad=True, norm=False, soft=True), dict(prompt=GLOSS + " " + E.PRIMER_V1, audio_dir="audio_dn")),
    "de_pinned": (dict(hp=True, vad=True, norm=True), dict(prompt=E.PRIMER_V1, language="de")),
}

def run(name, conds=("clean", "noisy")):
    pre, wk = VARIANTS[name]
    jobs = [(u, c) for u in C for c in conds]
    def one(job):
        (uid, voice, spoken, truth, tags), cond = job
        x = E.load(f"{E.SP}/{wk.get('audio_dir', 'audio2' if HOLDOUT else 'audio')}/{uid}_{cond}.wav")
        if wk.get("language") == "de" and voice == "en":
            lang = None          # pinned German only applies to German utterances (auto-retry logic in the app)
        else:
            lang = wk.get("language")
        if pre:
            x = E.preprocess(x, **pre)
        if x is None:
            return dict(uid=uid, cond=cond, raw="", spoken=spoken, truth=truth, tags=tags)
        kw = {k: v for k, v in wk.items() if k not in ("language", "audio_dir")}
        v = E.whisper_raw(x, language=lang, **kw)
        return dict(uid=uid, cond=cond, raw=E.segments_text(v), plain=v.get("text", "").strip(), lang=v.get("language"),
                    spoken=spoken, truth=truth, tags=tags)
    rows = E.pmap(one, jobs, workers=2)
    json.dump(rows, open(f"{E.SP}/raw_{name}{'_h' if HOLDOUT else ''}.json", "w"), ensure_ascii=False, indent=1)
    return rows

def report(name, rows):
    out = {}
    for cond in ("clean", "noisy"):
        r = [x for x in rows if x["cond"] == cond]
        w = np.mean([E.wer(x["spoken"], x["raw"]) for x in r]) * 100
        empty = sum(1 for x in r if not x["raw"].strip())
        out[cond] = (w, empty)
    allw = np.mean([E.wer(x["spoken"], x["raw"]) for x in rows]) * 100
    print(f"{name:10s} WER_vs_spoken  clean {out['clean'][0]:5.1f}% (empty {out['clean'][1]})  noisy {out['noisy'][0]:5.1f}% (empty {out['noisy'][1]})  all {allw:5.1f}%", flush=True)

if __name__ == "__main__":
    names = sys.argv[1:] or ["base"]
    for n in names:
        report(n, run(n))
