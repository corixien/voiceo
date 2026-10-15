//! Global hotkey via evdev (works on Wayland; needs membership in the `input` group).
use evdev::{Device, EventType, Key};
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::time::Duration;

pub type Capture = Arc<Mutex<Option<mpsc::Sender<Vec<u16>>>>>;

struct Shared {
    pressed: Mutex<HashSet<u16>>,
    active: [AtomicBool; 2],
}

pub fn keyboards() -> Vec<(std::path::PathBuf, Device)> {
    evdev::enumerate()
        .filter(|(_, d)| {
            d.name().map(|n| n != "voiceo-virtual-kb").unwrap_or(true)
                && d.supported_keys()
                    .map(|k| k.contains(Key::KEY_A) && k.contains(Key::KEY_ENTER))
                    .unwrap_or(false)
        })
        .collect()
}

/// Starts (once) a rescanning listener: new keyboards are picked up every 2 s.
/// `get_hotkeys` is re-read on every event so settings changes apply instantly.
/// `on_event(idx, true)` = hotkey `idx` pressed, `false` = released.
pub fn spawn(
    get_hotkeys: impl Fn() -> Vec<Vec<u16>> + Send + Sync + 'static,
    capture: Capture,
    on_event: impl Fn(usize, bool) + Send + Sync + 'static,
    on_key: impl Fn(u16) + Send + Sync + 'static,
    count: Arc<AtomicUsize>,
) {
    static STARTED: AtomicBool = AtomicBool::new(false);
    if STARTED.swap(true, Ordering::SeqCst) {
        return;
    }
    let shared = Arc::new(Shared {
        pressed: Mutex::new(HashSet::new()),
        active: [AtomicBool::new(false), AtomicBool::new(false)],
    });
    let get_hotkeys: Arc<dyn Fn() -> Vec<Vec<u16>> + Send + Sync> = Arc::new(get_hotkeys);
    let on_event: Arc<dyn Fn(usize, bool) + Send + Sync> = Arc::new(on_event);
    let on_key: Arc<dyn Fn(u16) + Send + Sync> = Arc::new(on_key);
    let known: Arc<Mutex<HashSet<PathBuf>>> = Arc::new(Mutex::new(HashSet::new()));

    std::thread::spawn(move || loop {
        for (path, dev) in keyboards() {
            if !known.lock().unwrap().insert(path.clone()) {
                continue;
            }
            count.fetch_add(1, Ordering::SeqCst);
            read_device(
                path,
                dev,
                shared.clone(),
                capture.clone(),
                get_hotkeys.clone(),
                on_event.clone(),
                on_key.clone(),
                known.clone(),
                count.clone(),
            );
        }
        std::thread::sleep(Duration::from_secs(2));
    });
}

#[allow(clippy::too_many_arguments)]
fn read_device(
    path: PathBuf,
    mut dev: Device,
    shared: Arc<Shared>,
    capture: Capture,
    get_hotkeys: Arc<dyn Fn() -> Vec<Vec<u16>> + Send + Sync>,
    on_event: Arc<dyn Fn(usize, bool) + Send + Sync>,
    on_key: Arc<dyn Fn(u16) + Send + Sync>,
    known: Arc<Mutex<HashSet<PathBuf>>>,
    count: Arc<AtomicUsize>,
) {
    std::thread::spawn(move || {
        'outer: loop {
            let events = match dev.fetch_events() {
                Ok(e) => e,
                Err(e) => {
                    eprintln!("[voiceo] device {} read error: {e}", path.display());
                    break 'outer; // device unplugged
                }
            };
            for ev in events {
                if ev.event_type() != EventType::KEY || ev.value() == 2 {
                    continue;
                }
                let code = ev.code();
                let down = ev.value() == 1;
                let mut pressed = shared.pressed.lock().unwrap();
                if down {
                    pressed.insert(code);
                } else {
                    pressed.remove(&code);
                }

                // capture mode: record the biggest combo until everything is released
                if let Some(tx) = capture.lock().unwrap().as_ref() {
                    if down {
                        let _ = tx.send(vec![code]); // single key: first key pressed wins
                    }
                    continue;
                }

                if down {
                    on_key(code);
                }
                let hotkeys = get_hotkeys();
                let states: Vec<bool> = hotkeys
                    .iter()
                    .map(|hk| !hk.is_empty() && hk.iter().all(|k| pressed.contains(k)))
                    .collect();
                drop(pressed);
                for (idx, all_down) in states.into_iter().enumerate().take(2) {
                    let was_active = shared.active[idx].load(Ordering::SeqCst);
                    if all_down && !was_active {
                        shared.active[idx].store(true, Ordering::SeqCst);
                        on_event(idx, true);
                    } else if !all_down && was_active {
                        shared.active[idx].store(false, Ordering::SeqCst);
                        on_event(idx, false);
                    }
                }
            }
        }
        known.lock().unwrap().remove(&path);
        count.fetch_sub(1, Ordering::SeqCst);
    });
}

pub fn wait_capture(capture: &Capture, timeout: Duration) -> Result<Vec<u16>, String> {
    let (tx, rx) = mpsc::channel();
    *capture.lock().unwrap() = Some(tx);
    let res = rx.recv_timeout(timeout);
    *capture.lock().unwrap() = None;
    res.map_err(|_| "Keine Taste erkannt".to_string())
}

pub fn label(code: u16) -> String {
    let raw = format!("{:?}", Key::new(code));
    let n = raw.trim_start_matches("KEY_");
    match n {
        "LEFTCTRL" => "Ctrl links".into(),
        "RIGHTCTRL" => "Ctrl rechts".into(),
        "LEFTALT" => "Alt links".into(),
        "RIGHTALT" => "Alt Gr".into(),
        "LEFTSHIFT" => "Shift links".into(),
        "RIGHTSHIFT" => "Shift rechts".into(),
        "LEFTMETA" => "Super links".into(),
        "RIGHTMETA" => "Super rechts".into(),
        "SPACE" => "Leertaste".into(),
        "CAPSLOCK" => "Feststelltaste".into(),
        "COMPOSE" => "Menue".into(),
        "SCROLLLOCK" => "Rollen".into(),
        "PAUSE" => "Pause".into(),
        other if other.starts_with("UNKNOWN") || other.contains('(') => format!("Taste {code}"),
        other => {
            let mut c = other.chars();
            match c.next() {
                Some(f) => f.to_uppercase().collect::<String>() + &c.as_str().to_lowercase(),
                None => String::new(),
            }
        }
    }
}
