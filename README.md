# Voiceo

Voice dictation for Linux powered by Groq. Hold a hotkey, speak, text appears at your cursor.

- Whisper large-v3 on Groq for transcription, then an LLM cleanup (punctuation, fillers, modes)
- Overlay bubble with reactive spectrum bars and the Voi mascot (wlr-layer-shell, never steals focus)
- Learns from your corrections: edit the dictated text, send it, Voiceo derives dictionary entries
- Wayland-friendly: global hotkey via evdev, paste via uinput

## Setup (Fedora/KDE)

```bash
sudo dnf install webkit2gtk4.1-devel gtk3-devel gtk-layer-shell-devel alsa-lib-devel libappindicator-gtk3-devel librsvg2-devel openssl-devel wl-clipboard python3-gobject at-spi2-core
npm install
npm run dev
```

On first start Voiceo asks once (polkit password prompt) to grant access to the keyboard and `/dev/uinput` via a udev rule. No re-login needed. Get a free API key at console.groq.com and paste it in Settings.

Pick a single, rarely used hotkey (default: Right Ctrl). Combinations like Ctrl+Alt+Space are often claimed by other apps.
