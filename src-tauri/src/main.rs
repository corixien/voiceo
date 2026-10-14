#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod audio;
mod config;
mod groq;
mod hotkey;
mod output;
mod setup;

use config::{HistoryItem, Settings};
use serde_json::json;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tauri::menu::{Menu, MenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, State, WindowEvent};

const MIN_RECORDING_MS: u128 = 300;

pub struct AppState {
    settings: Mutex<Settings>,
    dir: PathBuf,
    recorder: Mutex<Option<audio::Recorder>>,
    busy: AtomicBool,
    capture: hotkey::Capture,
    keyboard: Mutex<Option<output::VirtualKb>>,
    keyboard_err: Mutex<Option<String>>,
    keyboards: AtomicUsize,
    history: Mutex<Vec<HistoryItem>>,
    http: reqwest::blocking::Client,
}

impl AppState {
    fn settings_path(&self) -> PathBuf {
        self.dir.join("settings.json")
    }
    fn history_path(&self) -> PathBuf {
        self.dir.join("history.json")
    }
}

fn off_runtime<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> T {
    std::thread::spawn(f).join().expect("worker thread")
}

/// Starts hotkey listeners and the virtual keyboard if not yet available. Safe to call repeatedly.
fn init_input(app: &AppHandle) -> bool {
    let st = app.state::<AppState>();
    if st.keyboards.load(Ordering::SeqCst) == 0 {
        let handle = app.clone();
        let h2 = app.clone();
        let n = hotkey::spawn(
            move || h2.state::<AppState>().settings.lock().unwrap().hotkey.clone(),
            st.capture.clone(),
            move |pressed| on_hotkey(&handle, pressed),
        );
        st.keyboards.store(n, Ordering::SeqCst);
    }
    let mut kb = st.keyboard.lock().unwrap();
    if kb.is_none() {
        match output::VirtualKb::new() {
            Ok(k) => {
                *kb = Some(k);
                *st.keyboard_err.lock().unwrap() = None;
            }
            Err(e) => *st.keyboard_err.lock().unwrap() = Some(e),
        }
    }
    st.keyboards.load(Ordering::SeqCst) > 0 && kb.is_some()
}

fn emit_state(app: &AppHandle, state: &str, message: Option<&str>) {
    let _ = app.emit("voiceo://state", json!({ "state": state, "message": message }));
}

fn show_overlay(app: &AppHandle) {
    let Some(w) = app.get_webview_window("overlay") else { return };
    if let Ok(Some(m)) = w.primary_monitor() {
        let sf = m.scale_factor();
        let (mw, mh) = (m.size().width as f64, m.size().height as f64);
        let (ow, oh) = (264.0 * sf, 64.0 * sf);
        let x = m.position().x as f64 + (mw - ow) / 2.0;
        let y = m.position().y as f64 + mh - oh - 72.0 * sf;
        let _ = w.set_position(PhysicalPosition::new(x as i32, y as i32));
    }
    let _ = w.show();
}

fn hide_overlay_later(app: &AppHandle, ms: u64) {
    let app = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(ms));
        let st = app.state::<AppState>();
        if !st.busy.load(Ordering::SeqCst) && st.recorder.lock().unwrap().is_none() {
            if let Some(w) = app.get_webview_window("overlay") {
                let _ = w.hide();
            }
        }
    });
}

fn start_recording(app: &AppHandle) {
    let st = app.state::<AppState>();
    if st.busy.load(Ordering::SeqCst) || st.recorder.lock().unwrap().is_some() {
        return;
    }
    let s = st.settings.lock().unwrap().clone();
    if s.api_key.trim().is_empty() {
        emit_state(app, "error", Some("Kein Groq API-Key gesetzt"));
        if s.overlay {
            show_overlay(app);
            hide_overlay_later(app, 2600);
        }
        return;
    }
    match audio::start(app.clone(), s.max_seconds) {
        Ok(rec) => {
            *st.recorder.lock().unwrap() = Some(rec);
            if s.overlay {
                show_overlay(app);
            }
            emit_state(app, "listening", None);
        }
        Err(e) => {
            emit_state(app, "error", Some(&e));
            if s.overlay {
                show_overlay(app);
                hide_overlay_later(app, 2600);
            }
        }
    }
}

fn stop_recording(app: &AppHandle) {
    let st = app.state::<AppState>();
    let Some(rec) = st.recorder.lock().unwrap().take() else { return };
    let too_short = rec.started.elapsed().as_millis() < MIN_RECORDING_MS;
    let result = rec.stop();
    if too_short {
        emit_state(app, "idle", None);
        hide_overlay_later(app, 0);
        return;
    }
    let (samples, rate) = match result {
        Ok(r) => r,
        Err(e) => {
            emit_state(app, "error", Some(&e));
            hide_overlay_later(app, 2600);
            return;
        }
    };
    st.busy.store(true, Ordering::SeqCst);
    emit_state(app, "thinking", None);
    let app = app.clone();
    std::thread::spawn(move || {
        let t0 = Instant::now();
        let res = run_pipeline(&app, samples, rate);
        let st = app.state::<AppState>();
        match res {
            Ok(Some((raw, text, warn))) => {
                let (mode, limit) = {
                    let s = st.settings.lock().unwrap();
                    (s.mode.clone(), s.history_limit)
                };
                let item = HistoryItem {
                    ts: SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0),
                    raw,
                    text: text.clone(),
                    mode,
                    ms: t0.elapsed().as_millis() as u64,
                };
                {
                    let mut h = st.history.lock().unwrap();
                    h.insert(0, item);
                    h.truncate(limit.max(1));
                    config::save_history(&st.history_path(), &h);
                }
                let _ = app.emit("voiceo://history", ());
                st.busy.store(false, Ordering::SeqCst);
                match warn {
                    Some(w) => emit_state(&app, "error", Some(&w)),
                    None => emit_state(&app, "done", Some(&text)),
                }
                hide_overlay_later(&app, 1100);
            }
            Ok(None) => {
                st.busy.store(false, Ordering::SeqCst);
                emit_state(&app, "idle", None);
                hide_overlay_later(&app, 0);
            }
            Err(e) => {
                st.busy.store(false, Ordering::SeqCst);
                emit_state(&app, "error", Some(&e));
                hide_overlay_later(&app, 3000);
            }
        }
    });
}

const HALLUCINATIONS: &[&str] = &[
    "untertitel",
    "amara.org",
    "thanks for watching",
    "thank you for watching",
    "danke fürs zuschauen",
    "vielen dank fürs zuschauen",
    "subtitles by",
    "subscribe to",
];

/// Returns (raw, final text, optional warning).
fn run_pipeline(
    app: &AppHandle,
    samples: Vec<f32>,
    rate: u32,
) -> Result<Option<(String, String, Option<String>)>, String> {
    let st = app.state::<AppState>();
    let s = st.settings.lock().unwrap().clone();

    let mut pcm = audio::resample(&samples, rate);
    if audio::peak(&pcm) < s.silence_threshold {
        return Ok(None);
    }
    if s.trim_silence {
        pcm = audio::trim_silence(&pcm);
    }
    let wav = audio::to_wav(&pcm)?;
    let raw = groq::transcribe(&st.http, &s, wav)?;
    let lower = raw.to_lowercase();
    if raw.is_empty() || (raw.chars().count() < 90 && HALLUCINATIONS.iter().any(|h| lower.contains(h))) {
        return Ok(None);
    }

    let words = raw.split_whitespace().count() as u32;
    let mut text = raw.clone();
    let mut warn = None;
    if s.cleanup && s.mode != "raw" && words >= s.min_words_cleanup {
        match groq::cleanup(&st.http, &s, &raw) {
            Ok(Some(t)) => text = t,
            Ok(None) => {}
            Err(e) => warn = Some(format!("Cleanup fehlgeschlagen, Rohtext genutzt ({e})")),
        }
    }

    let mut kb = st.keyboard.lock().unwrap();
    let deliver_warn = output::deliver(&s, &mut kb, &text)?;
    Ok(Some((raw, text, deliver_warn.or(warn))))
}

fn on_hotkey(app: &AppHandle, pressed: bool) {
    let st = app.state::<AppState>();
    let mode = st.settings.lock().unwrap().hotkey_mode.clone();
    let recording = st.recorder.lock().unwrap().is_some();
    match (mode.as_str(), pressed) {
        ("toggle", true) => {
            if recording {
                stop_recording(app)
            } else {
                start_recording(app)
            }
        }
        ("toggle", false) => {}
        (_, true) => start_recording(app),
        (_, false) => stop_recording(app),
    }
}

// ---------- commands ----------

#[tauri::command]
fn get_settings(state: State<AppState>) -> Settings {
    state.settings.lock().unwrap().clone()
}

#[tauri::command]
fn save_settings(state: State<AppState>, settings: Settings) -> Result<(), String> {
    settings.save(&state.settings_path())?;
    *state.settings.lock().unwrap() = settings;
    Ok(())
}

#[tauri::command]
fn key_labels(codes: Vec<u16>) -> Vec<String> {
    codes.into_iter().map(hotkey::label).collect()
}

#[tauri::command(async)]
fn capture_hotkey(state: State<AppState>) -> Result<Vec<u16>, String> {
    hotkey::wait_capture(&state.capture, Duration::from_secs(12))
}

#[tauri::command(async)]
fn test_api(state: State<AppState>, key: String) -> Result<String, String> {
    let http = state.http.clone();
    let key = key.trim().to_string();
    let n = off_runtime(move || groq::check_key(&http, &key))?;
    Ok(format!("Verbunden, {n} Modelle verfuegbar"))
}

#[tauri::command(async)]
fn run_setup(app: AppHandle) -> Result<bool, String> {
    off_runtime(setup::run)?;
    for _ in 0..10 {
        std::thread::sleep(Duration::from_millis(400));
        if init_input(&app) {
            return Ok(true);
        }
    }
    Ok(false)
}

#[tauri::command]
fn get_history(state: State<AppState>) -> Vec<HistoryItem> {
    state.history.lock().unwrap().clone()
}

#[tauri::command]
fn clear_history(state: State<AppState>) {
    let mut h = state.history.lock().unwrap();
    h.clear();
    config::save_history(&state.history_path(), &h);
}

#[tauri::command]
fn system_status(state: State<AppState>) -> serde_json::Value {
    json!({
        "keyboards": state.keyboards.load(Ordering::SeqCst),
        "uinput": state.keyboard.lock().unwrap().is_some(),
        "uinput_error": state.keyboard_err.lock().unwrap().clone(),
        "wl_copy": output::wl_copy_available(),
        "wtype": output::wtype_available(),
        "api_key": !state.settings.lock().unwrap().api_key.trim().is_empty(),
    })
}

fn main() {
    // XWayland: lets the overlay position itself and stay on top; hotkeys use evdev anyway.
    if std::env::var_os("GDK_BACKEND").is_none() {
        std::env::set_var("GDK_BACKEND", "x11");
    }
    if std::env::var_os("WEBKIT_DISABLE_DMABUF_RENDERER").is_none() {
        std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1");
    }

    tauri::Builder::default()
        .setup(|app| {
            let dir = app.path().app_config_dir()?;
            std::fs::create_dir_all(&dir)?;
            let settings = Settings::load(&dir.join("settings.json"));
            let history = config::load_history(&dir.join("history.json"));
            let (kb, kb_err) = match output::VirtualKb::new() {
                Ok(k) => (Some(k), None),
                Err(e) => (None, Some(e)),
            };
            let capture: hotkey::Capture = Arc::new(Mutex::new(None));
            app.manage(AppState {
                settings: Mutex::new(settings),
                dir,
                recorder: Mutex::new(None),
                busy: AtomicBool::new(false),
                capture: capture.clone(),
                keyboard: Mutex::new(kb),
                keyboard_err: Mutex::new(kb_err),
                keyboards: AtomicUsize::new(0),
                history: Mutex::new(history),
                http: groq::client(),
            });

            let handle = app.handle().clone();
            std::thread::spawn(move || {
                if init_input(&handle) {
                    return;
                }
                // first run: ask once for the one-time system setup (polkit prompt)
                let marker = handle.state::<AppState>().dir.join("setup-attempted");
                if marker.exists() {
                    return;
                }
                let _ = std::fs::write(&marker, "1");
                if setup::run().is_ok() {
                    for _ in 0..10 {
                        std::thread::sleep(Duration::from_millis(400));
                        if init_input(&handle) {
                            break;
                        }
                    }
                }
                let _ = handle.emit("voiceo://setup", ());
            });

            // tray
            let show = MenuItem::with_id(app, "show", "Einstellungen", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "Beenden", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show, &quit])?;
            TrayIconBuilder::new()
                .icon(app.default_window_icon().cloned().ok_or("icon")?)
                .tooltip("Voiceo")
                .menu(&menu)
                .on_menu_event(|app, ev| match ev.id.as_ref() {
                    "show" => {
                        if let Some(w) = app.get_webview_window("main") {
                            let _ = w.show();
                            let _ = w.set_focus();
                        }
                    }
                    "quit" => app.exit(0),
                    _ => {}
                })
                .build(app)?;
            Ok(())
        })
        .on_window_event(|window, event| {
            if window.label() == "main" {
                if let WindowEvent::CloseRequested { api, .. } = event {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            get_settings,
            save_settings,
            key_labels,
            capture_hotkey,
            test_api,
            get_history,
            clear_history,
            system_status,
            run_setup
        ])
        .run(tauri::generate_context!())
        .expect("error while running voiceo");
}
