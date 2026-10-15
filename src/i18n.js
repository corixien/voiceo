// UI strings. Keys are used via data-i18n / data-i18n-ph in HTML and t() in JS.
window.I18N = {
  de: {
    tagline: "Sprich. Es erscheint.",
    "nav.general": "Allgemein", "nav.output": "Ausgabe", "nav.modes": "Modi", "nav.dict": "Wörterbuch", "nav.history": "Verlauf", "nav.advanced": "Erweitert",
    ready: "Bereit", incomplete: "Setup unvollständig", checking: "Prüfe…",

    "general.sub": "Hotkey, Mikrofon, Groq.",
    "groq": "Groq", "apikey": "API-Key", "apikey.hint": "Bleibt lokal.", "test": "Testen", "testing": "Teste…",
    "connected": "Verbunden, {n} Modelle",
    "dictation": "Diktat", "hotkey": "Hotkey", "hotkey.hint": "Eine einzelne Taste ist am sichersten.", "change": "Ändern", "press": "Taste drücken…", "off": "aus",
        "behavior": "Verhalten", "hold": "Halten", "toggle": "Umschalten",
    "mic": "Mikrofon", "mic.default": "Standard",
    "dictlang": "Diktat-Sprache", "lang.auto": "Automatisch (DE/EN)",
    "overlay": "Overlay",
    "system": "System",
    "chk.key": "API-Key", "chk.key.ok": "Gesetzt", "chk.key.bad": "Fehlt",
    "chk.hotkey": "Globaler Hotkey", "chk.hotkey.ok": "{n} Tastatur(en)", "chk.hotkey.bad": "Kein Zugriff",
    "chk.clip": "Zwischenablage", "chk.clip.ok": "wl-copy bereit", "chk.clip.bad": "wl-clipboard fehlt",
    "chk.paste": "Einfügen", "chk.paste.ok": "uinput bereit", "chk.paste.bad": "Kein Zugriff",
    "chk.learn": "Auto-Lernen", "chk.learn.ok": "AT-SPI bereit", "chk.learn.bad": "Nicht aktiv",
    "setup": "Jetzt einrichten", "setup.hint": "Einmal Passwort, kein Neustart.", "setting-up": "Richte ein…", "setup.done": "Eingerichtet", "setup.retry": "Noch nicht bereit, nochmal versuchen",

    "output.sub": "Was mit dem Text passiert.",
    "autoinsert": "Automatisch einfügen", "method": "Methode", "paste": "Einfügen", "type": "Tippen",
    "shortcut": "Einfüge-Kürzel", "keepclip": "In Zwischenablage behalten", "keepclip.hint": "Aus = alter Inhalt bleibt.",

    "modes.sub": "Wie der Text aufbereitet wird.",
    "cleanup": "Nachbearbeitung",
    "mode.raw": "Roh", "mode.raw.d": "Nur Whisper",
    "mode.clean": "Sauber", "mode.clean.d": "Textnah",
    "mode.message": "Nachricht", "mode.message.d": "Locker",
    "mode.email": "E-Mail", "mode.email.d": "Formell",
    "mode.prompt": "Prompt", "mode.prompt.d": "Für KI",
    "mode.notes": "Notizen", "mode.notes.d": "Stichpunkte",
    "mode.custom": "Eigener", "mode.custom.d": "Dein Prompt",
    "customprompt": "Eigener Prompt", "customprompt.ph": "z. B. Als Jira-Ticket formulieren.",

    "dict.sub": "Aus deinen Korrekturen gelernt.",
    "learning": "Lernen", "autolearn": "Automatisch lernen", "autolearn.hint": "Beobachtet das Feld nach dem Einfügen.",
    "learnkey": "Lern-Taste", "learnkey.hint": "Text markieren, Taste drücken.", "learned": "Gelernte Korrekturen",
    "learned.empty": "Noch nichts gelernt.", "times": "{n}× gelernt", "wrong": "falsch", "right": "richtig", "add": "Hinzufügen", "remove": "Entfernen",
    "terms": "Eigene Begriffe", "terms.hint": "Ein Begriff pro Zeile",

    "history.sub": "Letzte Diktate.", "clear": "Löschen", "history.empty": "Noch keine Diktate.", "copy": "Kopieren", "copied": "Kopiert", "raw": "Roh",

    "advanced.sub": "Modelle und Feintuning.",
    "appearance": "Darstellung", "theme": "Design", "theme.system": "System", "theme.dark": "Dunkel", "theme.light": "Hell",
    "font": "Schrift", "font.anthropic": "Anthropic Sans", "font.lexend": "Lexend Deca", "font.system": "System",
    "uilang": "Sprache der App",
    "models": "Modelle", "stt": "Transkription", "llm": "Nachbearbeitung",
    "audio": "Audio", "highpass": "Rumpelfilter", "normalize": "Lautstärke angleichen", "trim": "Stille kürzen",
    "silence": "Stille-Schwelle", "maxlen": "Max. Länge (s)",
    "quality": "Qualität", "sttTemp": "Whisper-Temperatur", "llmTemp": "Cleanup-Temperatur", "minWords": "Cleanup ab Wörtern",
    "whisperctx": "Whisper-Kontext", "histsize": "Verlauf-Größe",

    saved: "Gespeichert", error: "Fehler",
    "ov.done": "Eingefügt",
  },
  en: {
    tagline: "Speak. It appears.",
    "nav.general": "General", "nav.output": "Output", "nav.modes": "Modes", "nav.dict": "Dictionary", "nav.history": "History", "nav.advanced": "Advanced",
    ready: "Ready", incomplete: "Setup incomplete", checking: "Checking…",

    "general.sub": "Hotkey, microphone, Groq.",
    "groq": "Groq", "apikey": "API key", "apikey.hint": "Stays local.", "test": "Test", "testing": "Testing…",
    "connected": "Connected, {n} models",
    "dictation": "Dictation", "hotkey": "Hotkey", "hotkey.hint": "A single key is safest.", "change": "Change", "press": "Press a key…", "off": "off",
        "behavior": "Behavior", "hold": "Hold", "toggle": "Toggle",
    "mic": "Microphone", "mic.default": "Default",
    "dictlang": "Dictation language", "lang.auto": "Automatic (DE/EN)",
    "overlay": "Overlay",
    "system": "System",
    "chk.key": "API key", "chk.key.ok": "Set", "chk.key.bad": "Missing",
    "chk.hotkey": "Global hotkey", "chk.hotkey.ok": "{n} keyboard(s)", "chk.hotkey.bad": "No access",
    "chk.clip": "Clipboard", "chk.clip.ok": "wl-copy ready", "chk.clip.bad": "wl-clipboard missing",
    "chk.paste": "Insert", "chk.paste.ok": "uinput ready", "chk.paste.bad": "No access",
    "chk.learn": "Auto-learn", "chk.learn.ok": "AT-SPI ready", "chk.learn.bad": "Inactive",
    "setup": "Set up now", "setup.hint": "One password, no restart.", "setting-up": "Setting up…", "setup.done": "Done", "setup.retry": "Not ready yet, try again",

    "output.sub": "What happens to the text.",
    "autoinsert": "Insert automatically", "method": "Method", "paste": "Paste", "type": "Type",
    "shortcut": "Paste shortcut", "keepclip": "Keep in clipboard", "keepclip.hint": "Off = old content stays.",

    "modes.sub": "How the text is polished.",
    "cleanup": "Cleanup",
    "mode.raw": "Raw", "mode.raw.d": "Whisper only",
    "mode.clean": "Clean", "mode.clean.d": "Close to spoken",
    "mode.message": "Message", "mode.message.d": "Casual",
    "mode.email": "Email", "mode.email.d": "Formal",
    "mode.prompt": "Prompt", "mode.prompt.d": "For AI",
    "mode.notes": "Notes", "mode.notes.d": "Bullets",
    "mode.custom": "Custom", "mode.custom.d": "Your prompt",
    "customprompt": "Custom prompt", "customprompt.ph": "e.g. Write it as a Jira ticket.",

    "dict.sub": "Learned from your corrections.",
    "learning": "Learning", "autolearn": "Learn automatically", "autolearn.hint": "Watches the field after pasting.",
    "learnkey": "Learn key", "learnkey.hint": "Select text, press the key.", "learned": "Learned corrections",
    "learned.empty": "Nothing learned yet.", "times": "learned {n}×", "wrong": "wrong", "right": "right", "add": "Add", "remove": "Remove",
    "terms": "Custom terms", "terms.hint": "One term per line",

    "history.sub": "Recent dictations.", "clear": "Clear", "history.empty": "No dictations yet.", "copy": "Copy", "copied": "Copied", "raw": "Raw",

    "advanced.sub": "Models and fine-tuning.",
    "appearance": "Appearance", "theme": "Theme", "theme.system": "System", "theme.dark": "Dark", "theme.light": "Light",
    "font": "Font", "font.anthropic": "Anthropic Sans", "font.lexend": "Lexend Deca", "font.system": "System",
    "uilang": "App language",
    "models": "Models", "stt": "Transcription", "llm": "Cleanup",
    "audio": "Audio", "highpass": "Rumble filter", "normalize": "Level audio", "trim": "Trim silence",
    "silence": "Silence threshold", "maxlen": "Max length (s)",
    "quality": "Quality", "sttTemp": "Whisper temperature", "llmTemp": "Cleanup temperature", "minWords": "Cleanup from words",
    "whisperctx": "Whisper context", "histsize": "History size",

    saved: "Saved", error: "Error",
    "ov.done": "Inserted",
  },
};
window.LANG = "de";
window.t = (k, vars) => {
  let s = (window.I18N[window.LANG] || {})[k] ?? window.I18N.de[k] ?? k;
  if (vars) for (const [a, b] of Object.entries(vars)) s = s.replace(`{${a}}`, b);
  return s;
};
