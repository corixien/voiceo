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
        for code in 1u16..=248 {
            keys.insert(Key::new(code));
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
        std::thread::sleep(Duration::from_millis(4));
        Ok(())
    }

    pub fn press(&mut self, code: u16, down: bool) -> Result<(), String> {
        self.key(Key::new(code), down as i32)
    }

    pub fn tap(&mut self, code: u16) -> Result<(), String> {
        self.key(Key::new(code), 1)?;
        self.key(Key::new(code), 0)
    }

    pub fn copy(&mut self) -> Result<(), String> {
        self.key(Key::KEY_LEFTCTRL, 1)?;
        self.key(Key::KEY_C, 1)?;
        self.key(Key::KEY_C, 0)?;
        self.key(Key::KEY_LEFTCTRL, 0)
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

fn wl_copy_with(text: &str, sensitive: bool) -> Result<(), String> {
    let mut cmd = Command::new("wl-copy");
    if sensitive {
        cmd.arg("--sensitive"); // keeps clipboard managers (Klipper) from storing it
    }
    let mut child = cmd
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

pub fn wl_copy_available() -> bool {
    Command::new("wl-copy").arg("--version").stdout(Stdio::null()).stderr(Stdio::null()).status().is_ok()
}

fn wl_clear() {
    let _ = Command::new("wl-copy").arg("--clear").stdout(Stdio::null()).stderr(Stdio::null()).status();
}

/// Copies and/or inserts `text` according to settings. Returns a warning string on partial failure.
/// With "keep in clipboard" off, the clipboard is cleared again after inserting.
pub fn deliver(s: &Settings, kb: &mut Option<VirtualKb>, text: &str) -> Result<Option<String>, String> {
    let mut warn = None;
    let keep = s.copy_to_clipboard;
    let mut typed = false;

    if s.auto_insert && s.insert_method == "type" {
        if let Some(k) = kb.as_mut() {
            typed = crate::typing::type_text(k, text)?;
        }
        if !typed {
            warn = Some("Tippen nicht moeglich, eingefuegt stattdessen".to_string());
        }
    }
    let paste = s.auto_insert && !typed;
    if keep || paste {
        wl_copy_with(text, !keep)?;
    }
    if paste {
        std::thread::sleep(Duration::from_millis(60));
        match kb.as_mut() {
            Some(k) => k.paste(s.paste_shortcut == "ctrl+shift+v")?,
            None => {
                return Ok(Some("Einfuegen nicht moeglich (uinput), Text liegt in der Zwischenablage".to_string()));
            }
        }
    }
    if !keep && paste {
        std::thread::sleep(Duration::from_millis(350)); // let the target app read the clipboard first
        wl_clear();
    }
    Ok(warn)
}

pub fn wl_paste() -> Result<String, String> {
    let out = Command::new("wl-paste")
        .arg("-n")
        .output()
        .map_err(|e| format!("wl-paste fehlt: {e}"))?;
    Ok(String::from_utf8_lossy(&out.stdout).to_string())
}
