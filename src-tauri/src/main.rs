#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod audio;
mod config;
mod glossary;
mod groq;
mod hotkey;
mod learn;
mod output;
mod setup;
mod typing;
mod watch;

use config::{HistoryItem, Settings};
use learn::Learned;
use serde_json::json;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tauri::menu::{Menu, MenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Emitter, Manager, State, WindowEvent};

const MIN_RECORDING_MS: u128 = 300;

pub struct AppState {
    settings: Mutex<Settings>,
    dir: PathBuf,
    recorder: Mutex<Option<audio::Recorder>>,
    busy: AtomicBool,
    capture: hotkey::Capture,
    keyboard: Mutex<Option<output::VirtualKb>>,
    keyboard_err: Mutex<Option<String>>,
    keyboards: Arc<AtomicUsize>,
    history: Mutex<Vec<HistoryItem>>,
    learned: Mutex<Vec<Learned>>,
    last_text: Mutex<String>,
    failed: Mutex<Option<(Vec<f32>, u32)>>,
    watcher: Mutex<Option<watch::Watcher>>,
    http: reqwest::blocking::Client,
}

impl AppState {
    fn settings_path(&self) -> PathBuf {
        self.dir.join("settings.json")
    }
    fn learned_path(&self) -> PathBuf {
        self.dir.join("learned.json")
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
    {
        let handle = app.clone();
        let h2 = app.clone();
        hotkey::spawn(
            move || {
                let s = h2.state::<AppState>();
                let s = s.settings.lock().unwrap();
                vec![s.hotkey.clone(), s.learn_hotkey.clone()]
            },
            st.capture.clone(),
            move |idx, pressed| {
                if idx == 0 {
                    on_hotkey(&handle, pressed)
                } else if !pressed {
                    let h = handle.clone();
                    std::thread::spawn(move || learn_from_selection(&h));
                }
            },
            {
                let h = app.clone();
                move |code| {
                    // Enter = message/URL submitted: finalize what the user did to the dictated text
                    if code == 28 || code == 96 {
                        if let Some(w) = h.state::<AppState>().watcher.lock().unwrap().as_ref() {
                            w.submit();
                        }
                    }
                }
            },
            st.keyboards.clone(),
        );
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
    if let Some(w) = app.get_webview_window("overlay") {
        let r = w.show();
        let _ = r;
    }
}

/// Plays the exit animation (state "idle"), then hides the overlay window.
fn hide_overlay_later(app: &AppHandle, ms: u64) {
    let app = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(ms));
        let st = app.state::<AppState>();
        let free = |st: &AppState| !st.busy.load(Ordering::SeqCst) && st.recorder.lock().unwrap().is_none();
        if !free(&st) {
            return;
        }
        emit_state(&app, "idle", None);
        std::thread::sleep(Duration::from_millis(650));
        if free(&st) {
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
    if let Some(w) = st.watcher.lock().unwrap().as_ref() {
        w.flush();
    }
    match audio::start(app.clone(), s.max_seconds, s.input_device.clone()) {
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
    process_audio(app, samples, rate);
}

/// Runs transcription + cleanup + insertion on a worker thread. On transient Groq/network
/// failures the audio is kept so the user can retry from the tray without speaking again.
fn process_audio(app: &AppHandle, samples: Vec<f32>, rate: u32) {
    let st = app.state::<AppState>();
    st.busy.store(true, Ordering::SeqCst);
    emit_state(app, "thinking", None);
    let app = app.clone();
    std::thread::spawn(move || {
        let t0 = Instant::now();
        let backup = samples.clone();
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
                eprintln!("pipeline error: {e}");
                st.busy.store(false, Ordering::SeqCst);
                let msg = if groq::is_transient(&e) {
                    *st.failed.lock().unwrap() = Some((backup, rate));
                    "Groq nicht erreichbar. Tray: Letztes Diktat wiederholen".to_string()
                } else {
                    e
                };
                emit_state(&app, "error", Some(&msg));
                hide_overlay_later(&app, 4500);
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
    if s.highpass {
        audio::highpass(&mut pcm);
    }
    if s.trim_silence {
        match audio::vad_soft(&pcm) {
            Some(p) => pcm = p,
            None => return Ok(None), // no real speech
        }
    }
    if s.normalize {
        audio::normalize(&mut pcm);
    }
    let wav = audio::to_wav(&pcm)?;
    let learned = st.learned.lock().unwrap().clone();
    let mut terms: Vec<String> = vec![];
    for t in learned.iter().map(|l| l.to.clone()).chain(s.dictionary.iter().cloned()) {
        if !terms.contains(&t) {
            terms.push(t);
        }
    }
    terms.reverse(); // newest first: the prompt budget favors recent terms
    let raw = groq::transcribe(&st.http, &s, wav, &terms)?;
    let lower = raw.to_lowercase();
    if raw.is_empty() || groq::is_prompt_echo(&raw) || (raw.chars().count() < 90 && HALLUCINATIONS.iter().any(|h| lower.contains(h))) {
        return Ok(None);
    }

    let words = raw.split_whitespace().count() as u32;
    let mut text = raw.clone();
    let mut warn = None;
    if s.cleanup && s.mode != "raw" && words >= s.min_words_cleanup {
        let hints = glossary::candidates(&raw, &terms);
        match groq::cleanup(&st.http, &s, &raw, &hints, &terms) {
            Ok(Some(t)) => text = t,
            Ok(None) => {}
            Err(e) => {
                eprintln!("cleanup error: {e}");
                warn = Some(format!("Cleanup fehlgeschlagen: {e}"));
            }
        }
    }

    text = learn::apply(&text, &learned);
    *st.last_text.lock().unwrap() = text.clone();

    let mut kb = st.keyboard.lock().unwrap();
    let deliver_warn = output::deliver(&s, &mut kb, &text)?;
    drop(kb);
    if s.auto_learn {
        if let Some(w) = st.watcher.lock().unwrap().as_ref() {
            w.track(&text);
        }
    }
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

fn flash(app: &AppHandle, state: &str, msg: &str, ms: u64) {
    if app.state::<AppState>().settings.lock().unwrap().overlay {
        show_overlay(app);
    }
    emit_state(app, state, Some(msg));
    hide_overlay_later(app, ms);
}

/// Compares dictated text with the user's final version; the LLM derives dictionary entries.
/// Returns the number of entries learned.
fn learn_from_edit(app: &AppHandle, original: &str, corrected: &str) -> usize {
    let st = app.state::<AppState>();
    let s = st.settings.lock().unwrap().clone();
    eprintln!("[voiceo] learn: original={original:?} final={corrected:?}");
    let norm = |t: &str| t.split_whitespace().collect::<Vec<_>>().join(" ");
    if norm(original) == norm(corrected) || s.api_key.trim().is_empty() {
        return 0;
    }
    let pairs = match groq::extract_learning(&st.http, &s, original, corrected) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("learn extraction failed: {e}");
            learn::diff_pairs(original, corrected)
        }
    };
    eprintln!("[voiceo] learn pairs: {pairs:?}");
    if pairs.is_empty() {
        return 0;
    }
    {
        let mut learned = st.learned.lock().unwrap();
        for (from, to) in &pairs {
            learn::add(&mut learned, from, to);
        }
        learn::save(&st.learned_path(), &learned);
    }
    let _ = app.emit("voiceo://learned", ());
    let (f, t) = &pairs[0];
    let more = if pairs.len() > 1 { format!(" +{}", pairs.len() - 1) } else { String::new() };
    flash(app, "learned", &format!("Gelernt: {f} → {t}{more}"), 2200);
    pairs.len()
}

/// Manual learn key: copies the current selection (the user's corrected text) and learns from it.
fn learn_from_selection(app: &AppHandle) {
    let st = app.state::<AppState>();
    let last = st.last_text.lock().unwrap().clone();
    if last.is_empty() {
        return flash(app, "error", "Noch kein Diktat zum Lernen", 2000);
    }
    let copied = {
        let mut kb = st.keyboard.lock().unwrap();
        match kb.as_mut() {
            Some(k) => k.copy(),
            None => Err("uinput fehlt".into()),
        }
    };
    if let Err(e) = copied {
        return flash(app, "error", &e, 2400);
    }
    std::thread::sleep(Duration::from_millis(220));
    let corrected = match output::wl_paste() {
        Ok(t) => t.trim().to_string(),
        Err(e) => return flash(app, "error", &e, 2400),
    };
    if learn_from_edit(app, &last, &corrected) == 0 {
        return flash(app, "error", "Keine Korrektur erkannt. Text markieren, dann Lern-Taste", 2600);
    }
    *st.last_text.lock().unwrap() = corrected;
}

// ---------- commands ----------

#[tauri::command]
fn get_learned(state: State<AppState>) -> Vec<Learned> {
    state.learned.lock().unwrap().clone()
}

#[tauri::command]
fn add_learned(state: State<AppState>, from: String, to: String) -> Vec<Learned> {
    let mut l = state.learned.lock().unwrap();
    learn::add(&mut l, from.trim(), to.trim());
    learn::save(&state.learned_path(), &l);
    l.clone()
}

#[tauri::command]
fn delete_learned(state: State<AppState>, from: String, to: String) -> Vec<Learned> {
    let mut l = state.learned.lock().unwrap();
    l.retain(|x| !(x.from == from && x.to == to));
    learn::save(&state.learned_path(), &l);
    l.clone()
}

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
    Ok(n.to_string())
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
fn list_inputs() -> Vec<String> {
    audio::list_inputs()
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
        "atspi": state.watcher.lock().unwrap().is_some(),
        "api_key": !state.settings.lock().unwrap().api_key.trim().is_empty(),
    })
}

/// Wayland resolves the app icon through a .desktop file matching the app id ("voiceo").
fn install_desktop_entry() {
    let Some(home) = std::env::var_os("HOME") else { return };
    let home = std::path::PathBuf::from(home);
    let Ok(exe) = std::env::current_exe() else { return };
    let apps = home.join(".local/share/applications");
    let icons = home.join(".local/share/icons/hicolor/512x512/apps");
    let _ = std::fs::create_dir_all(&apps);
    let _ = std::fs::create_dir_all(&icons);
    let _ = std::fs::write(icons.join("voiceo.png"), include_bytes!("../icons/icon.png"));
    let desktop = format!(
        "[Desktop Entry]\nType=Application\nName=Voiceo\nComment=Voice dictation powered by Groq\nExec={}\nIcon=voiceo\nTerminal=false\nCategories=Utility;AudioVideo;\nStartupWMClass=voiceo\n",
        exe.display()
    );
    let path = apps.join("voiceo.desktop");
    if std::fs::read_to_string(&path).map(|old| old != desktop).unwrap_or(true) {
        let _ = std::fs::write(&path, desktop);
        let _ = std::process::Command::new("update-desktop-database").arg(&apps).status();
    }
}

fn main() {
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
            let mut learned = learn::load(&dir.join("learned.json"));
            let mut settings = settings;
            if !settings.dictionary.is_empty() {
                // the old "custom terms" list now lives in the corrections list as plain terms
                for term in std::mem::take(&mut settings.dictionary) {
                    learn::add(&mut learned, "", &term);
                }
                learn::save(&dir.join("learned.json"), &learned);
                let _ = settings.save(&dir.join("settings.json"));
            }
            app.manage(AppState {
                settings: Mutex::new(settings),
                dir,
                recorder: Mutex::new(None),
                busy: AtomicBool::new(false),
                capture: capture.clone(),
                keyboard: Mutex::new(kb),
                keyboard_err: Mutex::new(kb_err),
                keyboards: Arc::new(AtomicUsize::new(0)),
                history: Mutex::new(history),
                learned: Mutex::new(learned),
                last_text: Mutex::new(String::new()),
                failed: Mutex::new(None),
                watcher: Mutex::new(None),
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

            // Overlay: wlr-layer-shell surface (never focused, anchored to the screen bottom).
            if let Some(w) = app.get_webview_window("overlay") {
                if let Ok(gw) = w.gtk_window() {
                    use gtk::prelude::{GtkWindowExt, WidgetExt};
                    use gtk_layer_shell::{Edge, KeyboardMode, Layer, LayerShell};
                    if gtk_layer_shell::is_supported() {
                        gw.init_layer_shell();
                        gw.set_layer(Layer::Overlay);
                        gw.set_keyboard_mode(KeyboardMode::None);
                        gw.set_anchor(Edge::Bottom, true);
                        gw.set_namespace("voiceo-overlay");
                        gw.set_exclusive_zone(-1);
                    }
                    gw.set_accept_focus(false);
                    gw.set_focus_on_map(false);
                    gw.set_skip_taskbar_hint(true);
                    gw.set_size_request(312, 173);
                    gw.set_default_size(312, 173);
                    gw.realize();
                    if let Some(gdk_win) = gw.window() {
                        gdk_win.set_pass_through(true);
                    }
                }
            }

            {
                let h = app.handle().clone();
                let _ = groq::NOTIFY.set(Box::new(move |msg| emit_state(&h, "thinking", Some(msg))));
            }
            install_desktop_entry();

            // AT-SPI watcher: learns from edits the user makes to dictated text
            if app.state::<AppState>().settings.lock().unwrap().auto_learn {
                let h = app.handle().clone();
                let dir = app.state::<AppState>().dir.clone();
                let watcher = watch::spawn(&dir, move |orig, fin| {
                    let h = h.clone();
                    std::thread::spawn(move || {
                        learn_from_edit(&h, &orig, &fin);
                    });
                });
                *app.state::<AppState>().watcher.lock().unwrap() = watcher;
            }

            // tray
            let show = MenuItem::with_id(app, "show", "Einstellungen", true, None::<&str>)?;
            let retry = MenuItem::with_id(app, "retry", "Letztes Diktat wiederholen", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "Beenden", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show, &retry, &quit])?;
            TrayIconBuilder::new()
                .icon(tauri::image::Image::from_bytes(include_bytes!("../icons/tray.png"))?)
                .tooltip("Voiceo")
                .menu(&menu)
                .on_menu_event(|app, ev| match ev.id.as_ref() {
                    "show" => {
                        if let Some(w) = app.get_webview_window("main") {
                            let _ = w.show();
                            let _ = w.set_focus();
                        }
                    }
                    "retry" => {
                        let st = app.state::<AppState>();
                        let failed = st.failed.lock().unwrap().take();
                        if let Some((samples, rate)) = failed {
                            if st.settings.lock().unwrap().overlay {
                                show_overlay(app);
                            }
                            process_audio(app, samples, rate);
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
            run_setup,
            get_learned,
            add_learned,
            delete_learned,
            list_inputs
        ])
        .run(tauri::generate_context!())
        .expect("error while running voiceo");
}
