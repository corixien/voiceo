#!/usr/bin/env python3
"""Voiceo AT-SPI helper.

Watches the text field Voiceo pasted into and reports the user's final version of the
pasted passage. Protocol: JSON lines. stdin: {"cmd":"track","text":...}, {"cmd":"flush"},
{"cmd":"submit"}, {"cmd":"stop"}. stdout: {"event":"ready"}, {"event":"final",...}.
"""
import json
import os
import sys
import threading
import time

import gi

gi.require_version("Atspi", "2.0")
from gi.repository import Atspi  # noqa: E402

IDLE_FINALIZE_S = 45
BIND_TIMEOUT_S = 6

lock = threading.RLock()
focused = None
track = None  # dict: original, obj, prefix, suffix, region, changed
pending = None  # dict: text, t
recent = []  # (time, obj, is_system) of text-changed sources


def dbg(msg):
    if os.environ.get("VOICEO_ATSPI_DEBUG"):
        sys.stderr.write(msg + "\n")
        sys.stderr.flush()


def emit(obj):
    sys.stdout.write(json.dumps(obj, ensure_ascii=False) + "\n")
    sys.stdout.flush()


def read_text(obj):
    try:
        ti = obj.get_text_iface()
        if ti is None:
            return None
        n = ti.get_character_count()
        return Atspi.Text.get_text(ti, 0, n)
    except Exception:
        return None


def norm(s):
    return " ".join(s.split())


def extract(t, tr):
    """Current version of the pasted passage inside full field text `t`."""
    prefix, suffix, original = tr["prefix"], tr["suffix"], tr["original"]
    start = None
    if prefix:
        p = t.rfind(prefix)
        if p >= 0:
            start = p + len(prefix)
    else:
        start = 0
    if start is None:
        # prefix edited away: locate the original approximately
        import difflib

        m = difflib.SequenceMatcher(None, t, original, autojunk=False).find_longest_match(
            0, len(t), 0, len(original)
        )
        if m.size < max(4, len(original) // 4):
            return None
        start = max(0, m.a - m.b)
    end = None
    if suffix:
        e = t.find(suffix, start)
        if e >= 0:
            end = e
    else:
        end = len(t)
    if end is None:
        end = min(len(t), start + len(original) + 40)
    return t[start:end]


def candidates():
    now = time.time()
    recent[:] = [r for r in recent if now - r[0] < 10][-40:]
    seen, out = [], []
    ordered = [r[1] for r in reversed(recent) if not r[2]] + [focused] + [r[1] for r in reversed(recent) if r[2]]
    for o in ordered:
        if o is None or any(o is x for x in seen):
            continue
        seen.append(o)
        out.append(o)
    return out[:12]


def bind_to(obj):
    global pending, track
    t = read_text(obj)
    if not t:
        return False
    orig = pending["text"]
    idx = t.rfind(orig)
    dbg(f"bind_to idx={idx} text_tail={t[-40:]!r} orig={orig!r}")
    if idx < 0:
        return False
    track = {
        "original": orig,
        "obj": obj,
        "prefix": t[max(0, idx - 30):idx],
        "suffix": t[idx + len(orig): idx + len(orig) + 30],
        "region": orig,
        "changed": time.time(),
    }
    pending = None
    return True


def try_bind(_obj=None):
    if pending is None:
        return False
    for o in candidates():
        if bind_to(o):
            return True
    return False


def finalize(reason):
    global track
    dbg(f"finalize {reason} track={track is not None} region={(track or {}).get('region')!r}")
    with lock:
        tr = track
        track = None
    if not tr:
        return
    region = tr["region"] or ""
    if norm(region) and norm(region) != norm(tr["original"]):
        emit({"event": "final", "original": tr["original"], "final": region.strip(), "reason": reason})


def on_event(event, *_):
    global focused
    try:
        et = event.type
        src = event.source
        with lock:
            if et.startswith("object:state-changed:focused"):
                if event.detail1 == 1:
                    focused = src
                    return
                if track and track["obj"] == src:
                    finalize("focus-lost")
                return
            if et.startswith("object:text-changed"):
                recent.append((time.time(), src, et.endswith(":system")))
                if pending is not None and try_bind():
                    return
                if track and track["obj"] == src:
                    t = read_text(src)
                    if t is not None:
                        r = extract(t, track)
                        dbg(f"change region={r!r}")
                        if r is not None:
                            track["region"] = r
                            track["changed"] = time.time()
    except Exception:
        pass


def stdin_loop():
    global pending
    import os

    for line in sys.stdin:
        try:
            msg = json.loads(line)
        except Exception:
            continue
        cmd = msg.get("cmd")
        with lock:
            if cmd == "track":
                if track:
                    finalize("new-dictation")
                pending = {"text": msg.get("text", ""), "t": time.time()}
                try_bind()  # the paste event usually arrived before this command
            elif cmd in ("flush", "submit"):
                if track:
                    finalize(cmd)
                pending = None if cmd == "submit" else pending
            elif cmd == "stop":
                os._exit(0)
    os._exit(0)  # stdin closed: parent is gone


def timer_loop():
    global pending
    while True:
        time.sleep(0.5)
        with lock:
            if pending is not None:
                if try_bind():
                    pass
                elif time.time() - pending["t"] > BIND_TIMEOUT_S:
                    pending = None
            if track:
                if read_text(track["obj"]) is None and time.time() - track["changed"] > 1:
                    finalize("gone")
                elif time.time() - track["changed"] > IDLE_FINALIZE_S:
                    finalize("idle")


def main():
    Atspi.init()
    listener = Atspi.EventListener.new(on_event)
    for ev in (
        "object:state-changed:focused",
        "object:text-changed:insert",
        "object:text-changed:delete",
    ):
        listener.register(ev)
    threading.Thread(target=stdin_loop, daemon=True).start()
    threading.Thread(target=timer_loop, daemon=True).start()
    emit({"event": "ready"})
    Atspi.event_main()


if __name__ == "__main__":
    main()
