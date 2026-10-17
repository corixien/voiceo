# Dictation quality harness

Backtest for Voiceo's pipeline: fixed corpus with ground truth, TTS audio in two conditions (clean, noisy/quiet/reverb), real Groq calls, WER scoring. API results are cached on disk.

## Files
- `corpus.py` - 48 tuning utterances (`spoken` = what the TTS says, `truth` = ideal typed text). `corpus2.py` - 24 hold-out utterances, never tune on them.
- `gen_audio.py <piper_dir> <out_dir>` - synthesizes `corpus.py`; `gen_audio2.py` does the same for the hold-out set.
- `evalkit.py` - Python ports of the audio steps (`highpass`, `normalize`, `vad_compress`, `vad_soft`), Whisper/chat calls with retry and cache, metrics (WER with number words normalized, strict CER, punctuation+case accuracy).
- `exp_whisper.py <variant...>` - stage 1: preprocessing and Whisper settings, WER against the spoken text. Variants are defined in `VARIANTS`.
- `exp_llm.py <raw_variant> <prompt@model...>` - stage 2: cleanup prompts and models on a stage-1 output, WER against the truth.
- `prompts.py` - prompt variants (`P0` is parsed from `src-tauri/src/groq.rs`) and model settings.

## Setup
Data lives outside the repo (set `EVAL_DIR`, default is the session scratchpad):
1. Piper TTS: release `2023.11.14-2` `piper_linux_x86_64.tar.gz` from github.com/rhasspy/piper, voices `de_DE-thorsten-medium`, `de_DE-thorsten_emotional-medium`, `en_US-lessac-medium` (.onnx and .onnx.json) from huggingface.co/rhasspy/piper-voices, all unpacked into `$EVAL_DIR/tts`.
2. `python3 gen_audio.py $EVAL_DIR/tts $EVAL_DIR/audio` (and `gen_audio2.py ... audio2`).
3. Needs `numpy`, `scipy`, `requests`; the Groq key is read from `~/.config/dev.voiceo.app/settings.json`.
4. Optional RNNoise: `cd src-tauri && cargo build --release --bin denoise`, run it over `audio/*.wav` into `audio_dn/`.

## Limits
Groq free tier: Whisper 20 requests/min (one variant on 96 clips takes about 5 minutes), chat models 8000 tokens/min and about 1000 requests/day per model. TTS speech is cleaner than real speech, so absolute numbers are optimistic; compare variants against each other, and confirm on the hold-out set.
