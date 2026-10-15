//! "Type" insertion: types text through the virtual keyboard using the user's real XKB layout.
use crate::output::VirtualKb;
use std::collections::HashMap;
use std::sync::OnceLock;
use xkbcommon::xkb;

const KEY_LEFTSHIFT: u16 = 42;
const KEY_RIGHTALT: u16 = 100;
const KEY_ENTER: u16 = 28;
const KEY_TAB: u16 = 15;

/// char -> (evdev keycode, shift, altgr)
type KeyMap = HashMap<char, (u16, bool, bool)>;

fn layout_from_system() -> (String, String) {
    // KDE
    if let Some(home) = std::env::var_os("HOME") {
        let p = std::path::Path::new(&home).join(".config/kxkbrc");
        if let Ok(txt) = std::fs::read_to_string(p) {
            let get = |k: &str| {
                txt.lines()
                    .find_map(|l| l.strip_prefix(&format!("{k}=")).map(|v| v.split(',').next().unwrap_or("").to_string()))
                    .unwrap_or_default()
            };
            let layout = get("LayoutList");
            if !layout.is_empty() {
                return (layout, get("VariantList"));
            }
        }
    }
    if let Ok(l) = std::env::var("XKB_DEFAULT_LAYOUT") {
        return (l.split(',').next().unwrap_or("us").to_string(), String::new());
    }
    let lang = std::env::var("LANG").unwrap_or_default();
    if lang.starts_with("de") { ("de".into(), String::new()) } else { ("us".into(), String::new()) }
}

fn build_map() -> Option<KeyMap> {
    let (layout, variant) = layout_from_system();
    let ctx = xkb::Context::new(xkb::CONTEXT_NO_FLAGS);
    let keymap = xkb::Keymap::new_from_names(&ctx, "", "", &layout, &variant, None, xkb::COMPILE_NO_FLAGS)?;
    let mut map: KeyMap = HashMap::new();
    let (min, max) = (keymap.min_keycode().raw(), keymap.max_keycode().raw());
    for raw in min..=max {
        let kc = xkb::Keycode::new(raw);
        if raw < 8 {
            continue;
        }
        let evdev = (raw - 8) as u16;
        let levels = keymap.num_levels_for_key(kc, 0).min(4);
        for lvl in 0..levels {
            let (shift, altgr) = (lvl & 1 == 1, lvl & 2 == 2);
            for sym in keymap.key_get_syms_by_level(kc, 0, lvl) {
                let s = xkb::keysym_to_utf8(*sym);
                let mut it = s.chars().filter(|c| !c.is_control());
                if let (Some(c), None) = (it.next(), it.next()) {
                    // keep the plainest way to type each char (fewest modifiers)
                    let cost = |v: &(u16, bool, bool)| v.1 as u8 + v.2 as u8;
                    let cand = (evdev, shift, altgr);
                    if map.get(&c).map(|old| cost(&cand) < cost(old)).unwrap_or(true) {
                        map.insert(c, cand);
                    }
                }
            }
        }
    }
    map.insert(' ', (57, false, false));
    Some(map)
}

fn keymap() -> Option<&'static KeyMap> {
    static M: OnceLock<Option<KeyMap>> = OnceLock::new();
    M.get_or_init(build_map).as_ref()
}

/// Types `text`. Returns Ok(false) when a character cannot be typed on this layout
/// (nothing was typed in that case, so the caller can fall back to pasting).
pub fn type_text(kb: &mut VirtualKb, text: &str) -> Result<bool, String> {
    let Some(map) = keymap() else { return Ok(false) };
    for c in text.chars() {
        if c != '\n' && c != '\t' && !map.contains_key(&c) {
            return Ok(false);
        }
    }
    for c in text.chars() {
        match c {
            '\n' => kb.tap(KEY_ENTER)?,
            '\t' => kb.tap(KEY_TAB)?,
            _ => {
                let (code, shift, altgr) = map[&c];
                if shift { kb.press(KEY_LEFTSHIFT, true)?; }
                if altgr { kb.press(KEY_RIGHTALT, true)?; }
                kb.tap(code)?;
                if altgr { kb.press(KEY_RIGHTALT, false)?; }
                if shift { kb.press(KEY_LEFTSHIFT, false)?; }
            }
        }
    }
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Manual: focus a text field, then `cargo test typing_live -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn typing_live() {
        std::thread::sleep(std::time::Duration::from_secs(6));
        let mut kb = VirtualKb::new().expect("uinput");
        std::thread::sleep(std::time::Duration::from_millis(600));
        let ok = type_text(&mut kb, "Hallo Welt, äöü ÄÖÜ ß! 1+2=3 @ € (x) \"q\" ~ #\nZeile 2").unwrap();
        println!("typed={ok}");
    }
}
