# Voiceo

Voice dictation for Linux powered by Groq. Hold a hotkey, speak, text appears at your cursor.

- Whisper large-v3 on Groq for transcription, then an LLM cleanup (punctuation, fillers, modes)
- Overlay bubble with reactive spectrum bars and the Voi mascot (wlr-layer-shell, never steals focus)
- Learns from your corrections: a searchable list of corrections and terms, filled automatically from edits you make after dictating (AT-SPI) or by hand
- Wayland-friendly: global hotkey via evdev (any key combination, Esc = off), insertion by clipboard paste or layout-aware typing via uinput
- Retries Groq rate limits and network errors automatically; a failed dictation can be repeated from the tray without speaking again
- Built-in laptop microphone by default, other devices selectable
- German and English UI, themes, font choice

## Setup (Fedora/KDE)

```bash
sudo dnf install webkit2gtk4.1-devel gtk3-devel gtk-layer-shell-devel alsa-lib-devel libappindicator-gtk3-devel librsvg2-devel openssl-devel wl-clipboard python3-gobject at-spi2-core
npm install
npm run dev
```

On first start Voiceo asks once (polkit password prompt) to grant access to the keyboard and `/dev/uinput` via a udev rule. No re-login needed. Get a free API key at console.groq.com and paste it in Settings.

Pick a rarely used hotkey (default: Right Ctrl). Combinations like Ctrl+Alt+Space are often claimed by other apps.

## Android

`android/` is a native Kotlin app with the same pipeline (Groq Whisper, LLM cleanup, learned corrections, glossary), the same settings UI (`src/` runs in a WebView) and the same Voi mascot. The bubble pops out of the camera cutout. Builds are published as APKs under GitHub Releases (tags `android-v*`).

- Enable the **Voiceo accessibility service** once (Settings, Accessibility): it draws the overlay, inserts the text into the focused field and watches that field to learn your corrections.
- Start/stop a dictation with the Quick Settings tile, the launcher shortcut "Diktat" (Samsung: Side key, Double press, Open app), or the accessibility button/shortcut. Tap once to start, again to insert.
- Build: `cd android && ./gradlew :app:assembleDebug` (JDK 17-25, Android SDK in `local.properties`). Releases are signed with `VOICEO_KEYSTORE*` env vars.

## Learning in Electron apps (Claude Desktop, VS Code, ...)

Chromium/Electron apps only expose their text fields to AT-SPI when started with `--force-renderer-accessibility` (the accessibility bus flag alone is not enough, tested with Electron 44). Add the flag to the app's launcher, e.g. a copy of `com.anthropic.Claude.desktop` in `~/.local/share/applications` with `Exec=claude-desktop --force-renderer-accessibility %U`, then restart the app. Firefox and Qt/GTK apps need nothing.

## Quality tooling

`tools/eval` contains a backtesting harness: synthetic speech with ground truth, run through the same preprocessing, Whisper and cleanup steps, scored by word error rate. See `tools/eval/README.md`.

## Notes

Groq free tier limits apply (Whisper 20 requests per minute, chat models 8000 tokens per minute). `project-resume.md` holds the current development state for new sessions.
