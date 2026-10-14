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
sudo usermod -aG input $USER          # global hotkey, then re-login
sudo bash scripts/setup-linux.sh      # auto-paste (uinput)
npm install
npm run dev
```

Get a free API key at console.groq.com and paste it in Settings.
