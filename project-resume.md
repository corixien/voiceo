# Project Resume

> Instruction for Claude: read this whole file first. Then read every file under "Must read before any change". Do not modify anything until you have fully understood the project. Keep this file updated as you work (see "Maintenance rules").

Last updated: 2026-10-16

## 1. What this project is
Voiceo: voice dictation for Linux (Fedora/KDE, Wayland) on Tauri 2 (Rust backend, plain HTML/JS frontend). Hold a hotkey (default Right Ctrl), speak, text is typed at the cursor. Groq Whisper large-v3 transcribes, an LLM cleans up (punctuation, fillers, modes). Overlay bubble with spectrum bars and the Voi mascot (layer-shell, never steals focus). Learns from user corrections via AT-SPI. Repo `corixien/voiceo`. Version 0.1.0.

## 2. Must read before any change
1. `README.md` - setup, features.
2. `src-tauri/src/main.rs` - app wiring, commands.
3. `src-tauri/src/groq.rs` - transcription + cleanup calls.
4. `src-tauri/src/audio.rs`, `hotkey.rs`, `typing.rs`, `output.rs` - capture chain, evdev hotkey, uinput typing, paste/clipboard.
5. `src-tauri/src/learn.rs`, `glossary.rs`, `watch.rs`, `data/atspi_helper.py` - correction learning, term glossary, AT-SPI bridge.
6. `src-tauri/src/setup.rs` - one-time polkit/udev setup for keyboard + `/dev/uinput`.
7. `src/app.js`, `src/i18n.js`, `src/index.html`, `src/overlay.html`, `src/mascot.js` - UI (settings, overlay, DE/EN).

## 3. How to navigate
- `src-tauri/` Rust crate; `config.rs` settings; `data/curated.txt`, `data/auto.txt` glossary data.
- `src/` frontend, fonts in `src/fonts/` (Lexend Deca).
- `tools/eval` untracked eval tooling; `src-tauri/src/bin/denoise.rs` untracked experiment.
- `.claude/launch.json` dev-server config.
- Flow: hotkey (evdev) -> record (VAD, highpass, normalize) -> Groq Whisper -> LLM cleanup with glossary hints -> insert via uinput typing (layout-aware) or clipboard paste -> optional AT-SPI watch learns corrections.

## 4. Commands
Run from `~/voiceo`.
- System deps (Fedora): `sudo dnf install webkit2gtk4.1-devel gtk3-devel gtk-layer-shell-devel alsa-lib-devel libappindicator-gtk3-devel librsvg2-devel openssl-devel wl-clipboard python3-gobject at-spi2-core`
- `npm install`, then `npm run dev` (tauri dev), `npm run build` (tauri build).
- Groq API key (console.groq.com) is entered in Settings, never in repo.

## 5. Invariants and gotchas
- Hotkey needs `input` group / udev rule; first start asks once via polkit.
- Overlay must never take focus.
- Clipboard is cleared after paste unless keep-in-clipboard is on (sensitive hint).
- `src-tauri/target/` is about 8.9 GB and gitignored; safe to `cargo clean`.
- Pick a rare single-key hotkey; combos often collide with other apps.

## 6. Current state
Branch `main`. Uncommitted work in progress at time of writing: edits in `Cargo.toml/lock`, `audio.rs`, `groq.rs`, `main.rs`, `i18n.js`, `index.html`, `overlay.html`, plus untracked `src-tauri/src/bin/` and `tools/`. Review `git status` and `git diff` before continuing.

## Maintenance rules
Update this file when structure, commands, invariants or state change. Bump "Last updated".
