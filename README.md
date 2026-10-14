# Voiceo

Voice dictation for Linux powered by Groq. Hold a hotkey, speak, text appears at your cursor.

- Whisper large-v3 on Groq for transcription
- Llama cleanup modes (clean, message, email, prompt, notes, custom)
- Dictionary for names and terms
- Pixel mascot overlay, Claude-style palette
- Wayland-friendly: global hotkey via evdev, paste via uinput

## Setup (Fedora/KDE)

```bash
sudo dnf install webkit2gtk4.1-devel gtk3-devel alsa-lib-devel libappindicator-gtk3-devel librsvg2-devel openssl-devel wl-clipboard
npm install
npm run dev
```

On first start Voiceo asks once (polkit password prompt) to grant access to the keyboard and `/dev/uinput` via a udev rule. No re-login needed. Get a free API key at console.groq.com and paste it in Settings.
