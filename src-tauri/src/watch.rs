//! Bridge to the AT-SPI helper (python3 + gi). Best-effort: if python/AT-SPI is missing, learning
//! from edited text is simply unavailable and the manual learn key still works.
use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::Mutex;

const HELPER: &str = include_str!("../data/atspi_helper.py");

pub struct Watcher {
    stdin: Mutex<ChildStdin>,
    _child: Child,
}

impl Watcher {
    pub fn send(&self, msg: Value) {
        if let Ok(mut w) = self.stdin.lock() {
            let _ = writeln!(w, "{msg}");
            let _ = w.flush();
        }
    }
    pub fn track(&self, text: &str) {
        self.send(json!({"cmd": "track", "text": text}));
    }
    pub fn flush(&self) {
        self.send(json!({"cmd": "flush"}));
    }
    pub fn submit(&self) {
        self.send(json!({"cmd": "submit"}));
    }
}

/// Switches on the session's accessibility flag so apps (Qt, Firefox, Electron) expose text fields.
fn enable_accessibility() {
    let _ = Command::new("gdbus")
        .args([
            "call", "--session", "--dest", "org.a11y.Bus", "--object-path", "/org/a11y/bus",
            "--method", "org.freedesktop.DBus.Properties.Set", "org.a11y.Status", "IsEnabled", "<true>",
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
}

/// `on_final(original, final_text)` is called from the reader thread.
pub fn spawn(dir: &Path, on_final: impl Fn(String, String) + Send + 'static) -> Option<Watcher> {
    enable_accessibility();
    let path = dir.join("atspi_helper.py");
    std::fs::write(&path, HELPER).ok()?;
    let mut child = Command::new("python3")
        .arg(&path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let stdin = child.stdin.take()?;
    let stdout = child.stdout.take()?;
    std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines().map_while(Result::ok) {
            let Ok(v) = serde_json::from_str::<Value>(&line) else { continue };
            if v["event"] == "final" {
                let o = v["original"].as_str().unwrap_or("").to_string();
                let f = v["final"].as_str().unwrap_or("").to_string();
                if !o.is_empty() && !f.is_empty() {
                    on_final(o, f);
                }
            }
        }
    });
    Some(Watcher { stdin: Mutex::new(stdin), _child: child })
}
