use serde::{Deserialize, Serialize};
use std::{fs, path::Path};

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(default)]
pub struct Settings {
    pub api_key: String,
    /// evdev key codes that make up the hotkey
    pub hotkey: Vec<u16>,
    /// "hold" (push-to-talk) or "toggle"
    pub hotkey_mode: String,
    /// evdev codes: select corrected text, press this to learn word corrections (empty = off)
    pub learn_hotkey: Vec<u16>,
    /// "auto" or ISO-639-1 code
    pub language: String,
    pub stt_model: String,
    pub llm_model: String,
    pub cleanup: bool,
    /// raw | clean | message | email | prompt | notes | custom
    pub mode: String,
    pub custom_prompt: String,
    pub dictionary: Vec<String>,
    pub auto_insert: bool,
    /// "paste" (clipboard + shortcut) or "type" (wtype)
    pub insert_method: String,
    /// "ctrl+v" or "ctrl+shift+v"
    pub paste_shortcut: String,
    pub copy_to_clipboard: bool,
    pub stt_temperature: f32,
    pub llm_temperature: f32,
    pub min_words_cleanup: u32,
    pub whisper_prompt: String,
    pub overlay: bool,
    pub max_seconds: u32,
    pub silence_threshold: f32,
    pub trim_silence: bool,
    pub history_limit: usize,
    /// "" = default microphone
    pub input_device: String,
    pub normalize: bool,
    pub highpass: bool,
    /// watch edited text after insertion and learn corrections (needs accessibility)
    pub auto_learn: bool,
    /// "system" | "dark" | "light"
    pub theme: String,
    /// "lexend" | "anthropic" | "system"
    pub font: String,
    /// "de" | "en"
    pub ui_lang: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            api_key: String::new(),
            hotkey: vec![97], // KEY_RIGHTCTRL
            hotkey_mode: "hold".into(),
            learn_hotkey: vec![],
            language: "auto".into(),
            stt_model: "whisper-large-v3".into(),
            llm_model: "openai/gpt-oss-120b".into(),
            cleanup: true,
            mode: "clean".into(),
            custom_prompt: String::new(),
            dictionary: vec![],
            auto_insert: true,
            insert_method: "paste".into(),
            paste_shortcut: "ctrl+v".into(),
            copy_to_clipboard: true,
            stt_temperature: 0.0,
            llm_temperature: 0.2,
            min_words_cleanup: 3,
            whisper_prompt: String::new(),
            overlay: true,
            max_seconds: 300,
            silence_threshold: 0.01,
            trim_silence: true,
            history_limit: 100,
            input_device: String::new(),
            normalize: true,
            highpass: true,
            auto_learn: true,
            theme: "system".into(),
            font: "lexend".into(),
            ui_lang: "de".into(),
        }
    }
}

impl Settings {
    pub fn load(path: &Path) -> Self {
        let mut s: Self = fs::read_to_string(path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();
        // retired Groq models -> current default
        if s.llm_model.starts_with("llama-") {
            s.llm_model = Self::default().llm_model;
        }
        // hotkeys are single keys now
        if s.hotkey.len() != 1 {
            s.hotkey = Self::default().hotkey;
        }
        if s.learn_hotkey.len() > 1 {
            s.learn_hotkey.clear();
        }
        s
    }

    pub fn save(&self, path: &Path) -> Result<(), String> {
        let json = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;
        fs::write(path, json).map_err(|e| e.to_string())?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = fs::set_permissions(path, fs::Permissions::from_mode(0o600));
        }
        Ok(())
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct HistoryItem {
    pub ts: u64,
    pub raw: String,
    pub text: String,
    pub mode: String,
    pub ms: u64,
}

pub fn load_history(path: &Path) -> Vec<HistoryItem> {
    fs::read_to_string(path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

pub fn save_history(path: &Path, items: &[HistoryItem]) {
    if let Ok(json) = serde_json::to_string(items) {
        let _ = fs::write(path, json);
    }
}
