//! One-click system setup: grants the logged-in user access to keyboards (hotkey) and
//! /dev/uinput (auto-paste) via a udev `uaccess` rule. Needs one polkit password prompt, no re-login.
use std::process::{Command, Stdio};

const SCRIPT: &str = r#"set -e
echo uinput > /etc/modules-load.d/voiceo-uinput.conf
modprobe uinput || true
cat > /etc/udev/rules.d/70-voiceo.rules <<'EOF'
KERNEL=="uinput", SUBSYSTEM=="misc", TAG+="uaccess", OPTIONS+="static_node=uinput"
SUBSYSTEM=="input", KERNEL=="event*", ENV{ID_INPUT_KEYBOARD}=="1", TAG+="uaccess"
EOF
udevadm control --reload-rules
udevadm trigger --subsystem-match=input --subsystem-match=misc
udevadm settle || true
"#;

pub fn run() -> Result<(), String> {
    let out = Command::new("pkexec")
        .args(["bash", "-c", SCRIPT])
        .stdin(Stdio::null())
        .output()
        .map_err(|e| format!("pkexec nicht startbar: {e}"))?;
    if out.status.success() {
        Ok(())
    } else {
        let code = out.status.code().unwrap_or(-1);
        if code == 126 || code == 127 {
            Err("Einrichtung abgebrochen".into())
        } else {
            Err(format!("Einrichtung fehlgeschlagen: {}", String::from_utf8_lossy(&out.stderr).trim()))
        }
    }
}
