#!/usr/bin/env python3
"""Stage 2: cleanup prompt / model comparison on real Whisper outputs.
Input: raw_<variant>.json from exp_whisper.py. Reference: corpus truth (punctuation, case, fillers removed)."""
import json, re, sys
import numpy as np
import os
import evalkit as E
H = "_h" if os.environ.get("HOLDOUT") == "1" else ""
from prompts import PROMPTS, MODELS

def run(raw_name, prompt_name, model_name, only_cond=None):
    rows = json.load(open(f"{E.SP}/raw_{raw_name}{H}.json"))
    system = PROMPTS[prompt_name]
    model, kw = MODELS[model_name]
    todo = [r for r in rows if r["raw"].strip() and (only_cond is None or r["cond"] == only_cond)]
    def one(r):
        if len(r["raw"].split()) < 3:       # the app skips cleanup for very short dictations
            return r["raw"]
        user = f"<transcript>\n{r['raw']}\n</transcript>"
        out = E.chat(model, system, user, **kw).strip()
        for t in ("<transcript>", "</transcript>"):
            out = out.replace(t, "")
        return out.strip()
    outs = E.pmap(one, todo, workers=2)
    for r, o in zip(todo, outs):
        r["final"] = o
    return todo

def report(label, rows):
    s = E.score([(r["truth"], r["final"]) for r in rows])
    by = {}
    for c in ("clean", "noisy"):
        rr = [r for r in rows if r["cond"] == c]
        if rr:
            by[c] = E.score([(r["truth"], r["final"]) for r in rr])
    print(f"{label:34s} n={len(rows):3d}  WER {s['WER']:5.1f}  CER* {s['CER*']:5.1f}  punct+case {s['punct+case']:5.1f}  exact {s['exact']:5.1f}"
          + "".join(f"  | {c}: WER {v['WER']:4.1f} p+c {v['punct+case']:4.1f}" for c, v in by.items()), flush=True)
    return s

if __name__ == "__main__":
    raw = sys.argv[1]
    for combo in sys.argv[2:]:
        p, m = combo.split("@")
        rows = run(raw, p, m)
        json.dump(rows, open(f"{E.SP}/final_{raw}_{p}_{m}{H}.json", "w"), ensure_ascii=False, indent=1)
        report(combo, rows)
