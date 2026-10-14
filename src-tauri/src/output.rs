use crate::config::Settings;
use evdev::uinput::{VirtualDevice, VirtualDeviceBuilder};
use evdev::{AttributeSet, EventType, InputEvent, Key};
use std::io::Write;
use std::process::{Command, Stdio};
use std::time::Duration;

pub struct VirtualKb(VirtualDevice);

impl VirtualKb {
    pub fn new() -> Result<Self, String> {
        let mut keys = AttributeSet::<Key>::new();
        for k in [Key::KEY_LEFTCTRL, Key::KEY_LEFTSHIFT, Key::KEY_V, Key::KEY_INSERT] {
            keys.insert(k);
        }
        let dev = VirtualDeviceBuilder::new()
            .map_err(|e| e.to_string())?
            .name("voiceo-virtual-kb")
            .with_keys(&keys)
            .map_err(|e| e.to_string())?
            .build()
            .map_err(|e| e.to_string())?;
        Ok(Self(dev))
    }

    fn key(&mut self, k: Key, val: i32) -> Result<(), String> {
        self.0
            .emit(&[InputEvent::new(EventType::KEY, k.code(), val)])
            .map_err(|e| e.to_string())?;
        std::thread::sleep(Duration::from_millis(12));
        Ok(())
    }

    pub fn paste(&mut self, shift: bool) -> Result<(), String> {
        self.key(Key::KEY_LEFTCTRL, 1)?;
        if shift {
            self.key(Key::KEY_LEFTSHIFT, 1)?;
        }
        self.key(Key::KEY_V, 1)?;
        self.key(Key::KEY_V, 0)?;
        if shift {
            self.key(Key::KEY_LEFTSHIFT, 0)?;
        }
        self.key(Key::KEY_LEFTCTRL, 0)
    }
}

fn wl_copy(text: &str) -> Result<(), String> {
    let mut child = Command::new("wl-copy")
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("wl-copy fehlt: {e}"))?;
    child
        .stdin
        .take()
        .ok_or("wl-copy stdin")?
        .write_all(text.as_bytes())
        .map_err(|e| e.to_string())?;
    child.wait().map_err(|e| e.to_string())?;
    Ok(())
}

pub fn wtype_available() -> bool {
    Command::new("wtype").arg("--help").stdout(Stdio::null()).stderr(Stdio::null()).status().is_ok()
}

pub fn wl_copy_available() -> bool {
    Command::new("wl-copy").arg("--version").stdout(Stdio::null()).stderr(Stdio::null()).status().is_ok()
}

/// Copies and/or inserts `text` according to settings. Returns a warning string on partial failure.
pub fn deliver(s: &Settings, kb: &mut Option<VirtualKb>, text: &str) -> Result<Option<String>, String> {
    let mut warn = None;
    let needs_clip = s.copy_to_clipboard || (s.auto_insert && s.insert_method == "paste");
    if needs_clip {
        wl_copy(text)?;
    }
    if s.auto_insert {
        std::thread::sleep(Duration::from_millis(60));
        if s.insert_method == "type" {
            let ok = Command::new("wtype")
                .arg("--")
                .arg(text)
                .status()
                .map(|st| st.success())
                .unwrap_or(false);
            if !ok {
                warn = Some("wtype fehlgeschlagen, Text liegt in der Zwischenablage".to_string());
                if !s.copy_to_clipboard {
                    wl_copy(text)?;
                }
            }
        } else {
            match kb.as_mut() {
                Some(k) => k.paste(s.paste_shortcut == "ctrl+shift+v")?,
                None => {
                    warn = Some("Einfuegen nicht moeglich (uinput), Text liegt in der Zwischenablage".to_string())
                }
            }
        }
    }
    Ok(warn)
}
