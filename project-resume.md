# Project Resume

> Instruction for Claude: read this whole file first. Then read every file under "Must read before any change". Do not modify anything until you have fully understood the project. Keep this file updated as you work (see "Maintenance rules").

Last updated: 2026-10-17

## 1. What this project is
Voiceo: voice dictation for Linux (Fedora/KDE, Wayland) on Tauri 2 (Rust backend, plain HTML/JS frontend). Hold a hotkey, speak, text is inserted at the cursor. Groq Whisper large-v3 transcribes, an LLM cleans up (punctuation, fillers, modes). Overlay bubble with spectrum bars and the Voi mascot (layer-shell, never steals focus). Learns from user corrections via AT-SPI. Repo `corixien/voiceo` (public), branch `main`, push directly to `main`, no PRs. User speaks German, informal, mixed with English tech words; UI is DE/EN.

## 2. Must read before any change
1. `README.md` - setup, features.
2. `src-tauri/src/main.rs` - app wiring, pipeline (`process_audio`, `run_pipeline`), commands, tray.
3. `src-tauri/src/groq.rs` - Whisper + cleanup calls, prompts, `send_retry` backoff, model fallback.
4. `src-tauri/src/audio.rs` (capture, built-in mic selection, VAD, filters), `hotkey.rs`, `typing.rs`, `output.rs`.
5. `src-tauri/src/learn.rs`, `glossary.rs`, `watch.rs`, `data/atspi_helper.py` - corrections list, term hints, AT-SPI bridge.
6. `src-tauri/src/setup.rs` - one-time polkit/udev setup (keyboard + `/dev/uinput`).
7. `src/app.js`, `src/i18n.js`, `src/index.html`, `src/overlay.html`, `src/mascot.js` - UI.
8. `tools/eval/README.md` and `tools/eval/*.py` - the quality "backtest" harness.

## 3. How to navigate
- `src-tauri/` Rust crate; `config.rs` settings (`~/.config/dev.voiceo.app/settings.json`, also `learned.json`, `history.json`); `data/curated.txt`, `data/auto.txt` glossary.
- `src/` frontend, fonts in `src/fonts/` (Lexend Deca). `src/bin/denoise.rs` is a dev tool (RNNoise via `nnnoiseless`) used only for evaluation; `default-run = "voiceo"` in Cargo.toml.
- Flow now: hotkey (evdev) -> record (device = built-in mic via `PIPEWIRE_NODE`) -> resample 16k -> highpass/normalize (settings, currently default ON) -> `vad_compress` (strict) -> Whisper (primer prompt) -> LLM cleanup -> learned replacements -> insert (uinput typing or clipboard paste) -> AT-SPI watch.
- UI strings only via `data-i18n` keys in `src/i18n.js` (de/en).

## 4. Commands
Run from `~/voiceo`.
- System deps (Fedora): see README.
- `npm install`, `npm run dev`; `cd src-tauri && cargo check`.
- Restarting the dev app: kill `voiceo` and `tauri dev`, then `npm run dev` (use exact-name `pgrep -x voiceo`; `pkill -f` with a pattern also present in the command line kills your own shell).
- Eval harness: see `tools/eval/README.md`.

## 4b. Android app (`android/`)
Native Kotlin (no Compose, no NDK, AGP 9.4.1, Gradle 9.8.1, JDK 25 ok, minSdk 29, target 35), package `dev.voiceo.app`. Pieces: `Store` (settings/learned/history JSON, same keys as desktop), `Groq` (port of groq.rs incl. retry; loads `prompt_base.txt` from shared assets, `DEFAULT_VOCAB`/`PRIMER`/mode prompts are duplicated from groq.rs: keep in sync), `Glossary`/`Learn`/`Dsp` (ports of glossary.rs, learn.rs, audio.rs incl. `vad_soft`), `Pipeline`, `Dictation` (process-wide state + listeners), `DictationService` (microphone foreground service: record, pipeline, insert), `VoiceoAccessibilityService` (overlay window of type accessibility-overlay with a WebView, text insertion by ACTION_PASTE with SET_TEXT fallback, edit watcher = port of `atspi_helper.py`, accessibility button), `MainActivity` (WebView with shared `src/index.html`, `android-shim.js` maps Tauri `invoke`/`listen` to `VoiceoBridge`, `mobile.css` = phone layout), `TriggerActivity` (invisible; reached by the launcher shortcut. A second launcher entry (activity-alias) for the Samsung side key was tried in 0.1.2 and removed again in 0.1.3 because installs failed with "App nicht installiert"; cause still unconfirmed, get the adb install error from the phone before re-adding), `DictateTileService`, `KeyboardButton` (key-sized Voi button as accessibility overlay above the IME window, found via `getWindows()` TYPE_INPUT_METHOD; tap = toggle dictation, drag when unlocked; settings `kb_button`, `kb_button_size` dp, `kb_button_locked`, `kb_button_x` fraction, `kb_button_y` dp above keyboard; position owned by the service, `Store.saveSettings` keeps it).
- Shared with desktop via Gradle asset dirs: `src/` (UI, mascot, fonts) and `src-tauri/data/` (glossary, `prompt_base.txt` which `groq.rs` also `include_str!`s).
- Overlay: `assets/overlay-android.html` (mirrored version of `src/overlay.html`; bubble morphs out of the camera cutout, rings, mascot hangs below). Cutout read from `WindowMetrics.windowInsets.displayCutout`.
- Build/test: `cd android && ./gradlew :app:testDebugUnitTest :app:assembleDebug`. Release signing: `source ~/.config/voiceo-android/keystore.env` (key never in repo; back it up, losing it breaks updates), `VOICEO_VERSION_NAME/CODE`, `:app:assembleRelease`. Publish: `gh release create android-vX.Y.Z --prerelease <apk>`, then ALWAYS `gh release download` it and compare sha256 with the local file (the first 0.1.0 upload arrived truncated by 8 KB and the phone said "App nicht installiert"; 0.1.1 is the first verified-intact release). CI: `.github/workflows/android-release.yml` needs secrets `VOICEO_KEYSTORE_B64`, `VOICEO_KEYSTORE_PASSWORD`, `VOICEO_KEY_ALIAS`, `VOICEO_KEY_PASSWORD` (not set yet).
- Tested on the user's Galaxy S23+ (SM-S916B, Android 16, adb id R5CW60TH73W, connected by USB when they say so): 0.1.3/0.1.4 install, accessibility service runs, microphone recording and the cutout overlay work. Keyboard button verified on device (appears above keyboard, tap toggles, drag works). Not yet verified on device: text insertion, learning, Groq round trip, tile, volume-key shortcut. The user's phone has their own Groq key entered in the app; never copy keys around. Open risks to check first: mic foreground service start from tile/shortcut/accessibility button on Android 14+, `ACTION_PASTE` acceptance in Chromium/Electron fields, WebView transparency in the accessibility overlay, cutout rect values, Samsung side-key "open app" listing the Voiceo shortcut.
- Sideloaded APKs hit Android's "restricted settings" for accessibility services: user must allow it in App info (menu, Allow restricted settings); cannot be fixed in code. Latest release android-v0.2.1 (final for now; user does not distribute the app, repo is public only for access). Insertion: keep-in-clipboard off => ACTION_SET_TEXT (clipboard untouched, caret set by delayed ACTION_SET_SELECTION), on => paste; Samsung keyboard history recorded pastes, restore-after-paste did not help. E2E test trick: play a Piper wav on the PC speakers next to the phone (paplay, sink volume up temporarily) and tap the keyboard button via adb. The Diktat-starten card in Allgemein was removed. Previous: 0.2.0 (signed v2+v3, cert SHA-256 ca05397a...06ce08, must stay identical). Audio callbacks must be posted to the main thread before touching WebViews (crash fixed in 0.2.0). After `adb install -r` or `am force-stop` toggle the service in `enabled_accessibility_services` off/on to rebind. Careful with blind `adb input tap`: Samsung pops up a USB mode dialog. Unblock without the Play Store: `android/scripts/enable-a11y.sh <pkg> <service> [serial]` (adb: `cmd appops set <pkg> ACCESS_RESTRICTED_SETTINGS allow` + append service to `enabled_accessibility_services`), or install through a session-based installer such as Obtainium. The ⋮ menu in App info is not shown on the user's Samsung. After `adb install -r` the service may need the script again.
- Desktop UI hook: `app.js refreshStatus` uses `window.platformChecks` when present (Android only).

## 5. Invariants and gotchas
- Overlay must never take focus (layer-shell, `KeyboardMode::None`).
- Hotkeys are free key combinations; Esc as first key while capturing switches that hotkey off. Combos can collide with other apps (Ctrl+Alt+Space opens Claude Desktop).
- Clipboard: when "keep in clipboard" is off, text is copied with `wl-copy --sensitive` and cleared after paste.
- Typing (`typing.rs`) reads the real XKB layout (kxkbrc); chars not typable fall back to paste. `€` was not typed in a test.
- Groq free tier: Whisper 20 requests/min; chat models 8000 tokens/min and ~1000 requests/day per model. `gpt-oss-120b` is the tightest.
- `src-tauri/target/` is large and gitignored; safe to `cargo clean`.
- Never print `settings.json` contents (API key). Remove test entries from the user's `learned.json`/`history.json` after tests.

- Learning (AT-SPI) in Electron apps (Claude Desktop) only works if the app is launched with `--force-renderer-accessibility`; `IsEnabled`/`ScreenReaderEnabled` on the a11y bus are not enough (tested live with Electron 44). Fix is a launcher override, not Voiceo code; see README. Only release builds write `~/.local/share/applications/voiceo.desktop` (debug builds used to overwrite it, which caused the recurring "connection refused": a debug binary needs the dev server on 127.0.0.1:1430). The desktop entry runs the release build (`npm run tauri build -- --no-bundle`); the debug binary needs `tauri dev` running on 127.0.0.1:1430.

## 6. Current state
Committed on `main`: Groq retry/backoff, tray "repeat last dictation", built-in mic default, `tools/eval` harness, `denoise` dev bin, and the winning quality config ported to Rust: Whisper prompt = glossary (`DEFAULT_VOCAB` + user terms) + primer, `vad_soft` in `run_pipeline` (old `vad_compress` removed), defaults `highpass=false`, `normalize=false`, `llm_model=qwen/qwen3.8-27b` (fallbacks gpt-oss-120b/20b, `<think>` stripped), `BASE_PROMPT` = P1 (`tools/eval/prompts.py`). Existing `settings.json` keep their old values; new defaults only apply to fresh installs.

Backtest results (word error rate, lower is better; 48 TTS utterances, clean + noisy):
- Tuning set, end-to-end: old pipeline about 12.1, new about 4.7.
- Hold-out set (`corpus2.py`, 24 utterances, never tuned on), end-to-end vs truth: old (base audio + P0 on gpt-oss-120b) 9.8, new (gloss + soft VAD + P1 on qwen) 5.7; noisy 16.9 -> 8.7, clean 2.7 -> 2.6. Whisper stage vs spoken: base 10.8, gloss+vad_soft 7.6.
- Eval scripts: `HOLDOUT=1` switches `exp_whisper.py`/`exp_llm.py` to corpus2/audio2. `prompts.py` P0 now reads the old prompt from git HEAD of `groq.rs`.

## 7. Next steps
0. Android: test on the phone once the user allows it (see 4b), fix what breaks, then publish stable release.
1. RNNoise integration only if a `dn_vad2` Whisper run shows a gain; otherwise leave `denoise` as a dev tool.
2. P4 (8 examples) vs P1 on the tuning set only (hold-out is spent).
3. Offline language packs (user asked): not implemented. Option: `whisper-rs` + downloadable ggml model as offline fallback when Groq is rate-limited. Needs cmake/clang build, CPU inference slower and less accurate than Groq large-v3. Ask the user before building it.

## Maintenance rules
Update this file when structure, commands, invariants or state change. Bump "Last updated".
