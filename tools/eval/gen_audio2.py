#!/usr/bin/env python3
"""Synthesizes the evaluation corpus with Piper TTS and adds mic-like degradation.
Usage: gen_audio.py <piper_dir> <out_dir>   (piper_dir holds ./piper/piper and the *.onnx voices)"""
import os, random, subprocess, sys, wave
import numpy as np
from scipy.signal import resample_poly, fftconvolve
import os
from corpus2 import C

PIPER_DIR, OUT = sys.argv[1], sys.argv[2]
os.makedirs(OUT, exist_ok=True)
EMO = {"amused": 0, "neutral": 4, "sleepy": 5, "surprised": 6}

def tts(text, voice, seed):
    rnd = random.Random(seed)
    ls = rnd.uniform(0.85, 1.1)
    if voice.startswith("de:"):
        model, extra = "de_DE-thorsten_emotional-medium.onnx", ["--speaker", str(EMO[voice[3:]])]
    elif voice == "en":
        model, extra = "en_US-lessac-medium.onnx", []
    else:
        model, extra = "de_DE-thorsten-medium.onnx", []
    tmp = os.path.join(OUT, "_tmp.wav")
    subprocess.run([f"{PIPER_DIR}/piper/piper", "--model", f"{PIPER_DIR}/{model}", "--output_file", tmp,
                    "--length_scale", f"{ls:.2f}", *extra], input=text.encode(), capture_output=True, check=True)
    with wave.open(tmp) as w:
        sr = w.getframerate(); x = np.frombuffer(w.readframes(w.getnframes()), dtype=np.int16).astype(np.float32) / 32768
    return resample_poly(x, 16000, sr).astype(np.float32)

def write(path, x):
    x = np.clip(x, -1, 1)
    with wave.open(path, "wb") as w:
        w.setnchannels(1); w.setsampwidth(2); w.setframerate(16000); w.writeframes((x * 32767).astype(np.int16).tobytes())

def pad(x, rnd):
    return np.concatenate([np.zeros(int(16000 * rnd.uniform(0.3, 0.9))), x, np.zeros(int(16000 * rnd.uniform(0.3, 0.8)))]).astype(np.float32)

def clean_cond(x, rnd):
    x = x / (np.abs(x).max() + 1e-6) * 10 ** (rnd.uniform(-14, -8) / 20)
    return pad(x, rnd)

def noisy_cond(x, rnd):
    x = x / (np.abs(x).max() + 1e-6)
    # small-room reverb
    n = int(16000 * 0.25); ir = np.random.default_rng(rnd.randint(0, 9999)).standard_normal(n) * np.exp(-np.arange(n) / (16000 * 0.06))
    ir[0] = 1.0
    x = fftconvolve(x, ir * 0.35, mode="full")[: len(x) + n // 2]
    x = pad(x, rnd)
    rms = np.sqrt(np.mean(x ** 2)) + 1e-9
    rng = np.random.default_rng(rnd.randint(0, 9999))
    snr = rnd.uniform(12, 22)
    noise = rng.standard_normal(len(x)).astype(np.float32)
    noise = np.convolve(noise, np.ones(6) / 6, mode="same")           # slightly dull, like a laptop mic
    noise *= rms / (10 ** (snr / 20)) / (np.sqrt(np.mean(noise ** 2)) + 1e-9)
    t = np.arange(len(x)) / 16000
    hum = 0.35 * rms / (10 ** (snr / 20)) * np.sin(2 * np.pi * 50 * t)
    y = x + noise + hum
    y = y / (np.abs(y).max() + 1e-6) * 10 ** (rnd.uniform(-30, -22) / 20)   # quiet mic
    return y.astype(np.float32)

pause_ids = set()
for i, (uid, voice, spoken, truth, tags) in enumerate(C):
    rnd = random.Random(1000 + i)
    if uid in pause_ids:
        words = spoken.split(); h = len(words) // 2
        a = tts(" ".join(words[:h]), voice, i); b = tts(" ".join(words[h:]), voice, i + 77)
        base = np.concatenate([a, np.zeros(int(16000 * 2.0)), b])
    else:
        base = tts(spoken, voice, i)
    write(f"{OUT}/{uid}_clean.wav", clean_cond(base, rnd))
    write(f"{OUT}/{uid}_noisy.wav", noisy_cond(base, rnd))
    print(uid, f"{len(base)/16000:.1f}s", flush=True)
