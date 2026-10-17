use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, Sample, SizedSample};
use std::io::Cursor;
use std::sync::{mpsc, Arc, Mutex};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter};

pub const TARGET_RATE: u32 = 16_000;

pub struct Recorder {
    stop: mpsc::Sender<()>,
    join: std::thread::JoinHandle<Result<(Vec<f32>, u32), String>>,
    pub started: Instant,
}

impl Recorder {
    pub fn stop(self) -> Result<(Vec<f32>, u32), String> {
        let _ = self.stop.send(());
        self.join
            .join()
            .map_err(|_| "Audio-Thread abgestuerzt".to_string())?
    }
}

pub fn start(app: AppHandle, max_seconds: u32, device: String) -> Result<Recorder, String> {
    let (stop_tx, stop_rx) = mpsc::channel::<()>();
    let (ready_tx, ready_rx) = mpsc::channel::<Result<(), String>>();
    let join = std::thread::spawn(move || run(app, max_seconds, device, stop_rx, ready_tx));
    match ready_rx.recv_timeout(Duration::from_secs(4)) {
        Ok(Ok(())) => Ok(Recorder { stop: stop_tx, join, started: Instant::now() }),
        Ok(Err(e)) => Err(e),
        Err(_) => Err("Mikrofon-Start Timeout".into()),
    }
}

fn run(
    app: AppHandle,
    max_seconds: u32,
    device: String,
    stop_rx: mpsc::Receiver<()>,
    ready: mpsc::Sender<Result<(), String>>,
) -> Result<(Vec<f32>, u32), String> {
    let setup = || -> Result<(cpal::Stream, Arc<Mutex<Vec<f32>>>, u32), String> {
        let host = cpal::default_host();
        // "" = built-in microphone (via PipeWire node), "system" = whatever the system default is
        match (device.as_str(), builtin_source()) {
            ("", Some(node)) => std::env::set_var("PIPEWIRE_NODE", node),
            _ => std::env::remove_var("PIPEWIRE_NODE"),
        }
        let named = if device.is_empty() || device == "system" {
            None
        } else {
            host.input_devices()
                .ok()
                .and_then(|mut d| d.find(|x| x.name().map(|n| n == device).unwrap_or(false)))
        };
        let dev = named
            .or_else(|| host.default_input_device())
            .ok_or("Kein Mikrofon gefunden")?;
        let cfg = dev.default_input_config().map_err(|e| e.to_string())?;
        let rate = cfg.sample_rate().0;
        let channels = cfg.channels() as usize;
        let buf = Arc::new(Mutex::new(Vec::<f32>::with_capacity(rate as usize * 30)));
        let scfg: cpal::StreamConfig = cfg.clone().into();
        let stream = match cfg.sample_format() {
            cpal::SampleFormat::F32 => build::<f32>(&dev, &scfg, channels, rate, buf.clone(), app.clone()),
            cpal::SampleFormat::I16 => build::<i16>(&dev, &scfg, channels, rate, buf.clone(), app.clone()),
            cpal::SampleFormat::U16 => build::<u16>(&dev, &scfg, channels, rate, buf.clone(), app.clone()),
            f => Err(format!("Nicht unterstuetztes Sample-Format: {f:?}")),
        }?;
        stream.play().map_err(|e| e.to_string())?;
        Ok((stream, buf, rate))
    };

    match setup() {
        Err(e) => {
            let _ = ready.send(Err(e.clone()));
            Err(e)
        }
        Ok((stream, buf, rate)) => {
            let _ = ready.send(Ok(()));
            let _ = stop_rx.recv_timeout(Duration::from_secs(max_seconds.max(5) as u64));
            drop(stream);
            let data = std::mem::take(&mut *buf.lock().unwrap());
            Ok((data, rate))
        }
    }
}

fn build<T>(
    dev: &cpal::Device,
    cfg: &cpal::StreamConfig,
    ch: usize,
    rate: u32,
    buf: Arc<Mutex<Vec<f32>>>,
    app: AppHandle,
) -> Result<cpal::Stream, String>
where
    T: SizedSample,
    f32: FromSample<T>,
{
    let mut last = Instant::now();
    dev.build_input_stream(
        cfg,
        move |data: &[T], _| {
            let mut sum_sq = 0f32;
            let mut n = 0usize;
            let mut window: Vec<f32> = Vec::new();
            {
                let mut b = buf.lock().unwrap();
                for frame in data.chunks(ch) {
                    let m = frame.iter().map(|&s| f32::from_sample(s)).sum::<f32>() / ch as f32;
                    sum_sq += m * m;
                    n += 1;
                    b.push(m);
                }
                if last.elapsed() >= Duration::from_millis(20) {
                    window = b[b.len().saturating_sub(1024)..].to_vec();
                }
            }
            if n > 0 && !window.is_empty() {
                last = Instant::now();
                let bands = bands(&window, rate);
                let _ = app.emit(
                    "voiceo://audio",
                    serde_json::json!({ "rms": (sum_sq / n as f32).sqrt(), "bands": bands }),
                );
            }
        },
        |e| eprintln!("audio error: {e}"),
        None,
    )
    .map_err(|e| e.to_string())
}

pub const BAND_COUNT: usize = 16;

/// Goertzel magnitudes on 16 log-spaced bands (90 Hz - 6.5 kHz) for the overlay visualizer.
fn bands(win: &[f32], rate: u32) -> [f32; BAND_COUNT] {
    let n = win.len() as f32;
    let mut out = [0f32; BAND_COUNT];
    for (i, o) in out.iter_mut().enumerate() {
        let f = 90.0 * (6500.0f32 / 90.0).powf(i as f32 / (BAND_COUNT as f32 - 1.0));
        let w = 2.0 * std::f32::consts::PI * f / rate as f32;
        let coeff = 2.0 * w.cos();
        let (mut s1, mut s2) = (0f32, 0f32);
        for (k, &x) in win.iter().enumerate() {
            let hann = 0.5 - 0.5 * (2.0 * std::f32::consts::PI * k as f32 / (n - 1.0)).cos();
            let s = x * hann + coeff * s1 - s2;
            s2 = s1;
            s1 = s;
        }
        let p = s1 * s1 + s2 * s2 - coeff * s1 * s2;
        *o = p.max(0.0).sqrt() * 4.0 / n;
    }
    out
}

/// The laptop's own microphone: the PCI-attached PipeWire/Pulse source (USB and Bluetooth are skipped).
pub fn builtin_source() -> Option<String> {
    let out = std::process::Command::new("pactl").args(["list", "short", "sources"]).output().ok()?;
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter_map(|l| l.split('\t').nth(1))
        .find(|n| n.starts_with("alsa_input.pci-"))
        .map(String::from)
}

pub fn list_inputs() -> Vec<String> {
    cpal::default_host()
        .input_devices()
        .map(|d| d.filter_map(|x| x.name().ok()).filter(|n| n.starts_with("sysdefault:")).collect())
        .unwrap_or_default()
}

/// 80 Hz first-order high-pass (removes rumble and desk thumps).
pub fn highpass(samples: &mut [f32]) {
    let rc = 1.0 / (2.0 * std::f32::consts::PI * 80.0);
    let dt = 1.0 / TARGET_RATE as f32;
    let a = rc / (rc + dt);
    let (mut prev_x, mut prev_y) = (0f32, 0f32);
    for s in samples.iter_mut() {
        let x = *s;
        let y = a * (prev_y + x - prev_x);
        prev_x = x;
        prev_y = y;
        *s = y;
    }
}

/// Peak-normalize quiet recordings (max +24 dB) so Whisper gets a healthy level.
pub fn normalize(samples: &mut [f32]) {
    let p = peak(samples);
    if p > 0.0005 && p < 0.7 {
        let g = (0.85 / p).min(16.0);
        for s in samples.iter_mut() {
            *s *= g;
        }
    }
}

/// Box-filter resample to 16 kHz mono.
pub fn resample(samples: &[f32], rate: u32) -> Vec<f32> {
    if rate == TARGET_RATE || samples.is_empty() {
        return samples.to_vec();
    }
    let ratio = rate as f64 / TARGET_RATE as f64;
    let out_len = (samples.len() as f64 / ratio) as usize;
    let mut out = Vec::with_capacity(out_len);
    for i in 0..out_len {
        let start = (i as f64 * ratio) as usize;
        let end = (((i + 1) as f64 * ratio) as usize).min(samples.len()).max(start + 1);
        let slice = &samples[start..end];
        out.push(slice.iter().sum::<f32>() / slice.len() as f32);
    }
    out
}

pub fn peak(samples: &[f32]) -> f32 {
    samples.iter().fold(0f32, |m, s| m.max(s.abs()))
}

/// Inclusive VAD: trims leading/trailing silence and shortens pauses longer than 1 s.
/// Everything between speech stays untouched so quiet word endings are never clipped.
/// Returns None when there is no real speech. (Rust port of tools/eval/evalkit.py `vad_soft`.)
pub fn vad_soft(samples: &[f32]) -> Option<Vec<f32>> {
    const LO_MUL: f32 = 2.2;
    const LO_MIN: f32 = 0.0025;
    let win = TARGET_RATE as usize / 50; // 20 ms
    if samples.len() < win * 5 {
        return None;
    }
    let energy: Vec<f32> = samples
        .chunks(win)
        .map(|w| (w.iter().map(|s| s * s).sum::<f32>() / w.len() as f32).sqrt())
        .collect();
    let n = energy.len();
    let mut sorted = energy.clone();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let thr = (sorted[n / 10] * LO_MUL).max(LO_MIN);
    let voiced: Vec<usize> = energy.iter().enumerate().filter(|(_, e)| **e > thr).map(|(i, _)| i).collect();
    if voiced.len() < 10 {
        return None;
    }
    let merge = 35; // 700 ms
    let mut segs: Vec<(usize, usize)> = vec![];
    let (mut s0, mut prev) = (voiced[0], voiced[0]);
    for &i in &voiced[1..] {
        if i - prev > merge {
            segs.push((s0, prev));
            s0 = i;
        }
        prev = i;
    }
    segs.push((s0, prev));
    segs.retain(|(a, b)| b - a + 1 >= 4);
    if segs.is_empty() {
        return None;
    }
    let pad = 15; // 300 ms
    let (long_gap, keep_gap) = (TARGET_RATE as usize, (TARGET_RATE as f32 * 0.4) as usize);
    let mut out: Vec<f32> = vec![];
    let mut last_end: Option<usize> = None;
    for (a, b) in segs {
        let mut a0 = a.saturating_sub(pad) * win;
        let b0 = ((b + 1 + pad).min(n) * win).min(samples.len());
        if let Some(le) = last_end {
            if a0 < le {
                a0 = le;
            }
            if a0 > le {
                let gap = a0 - le;
                out.extend_from_slice(&samples[le..le + if gap > long_gap { keep_gap } else { gap }]);
            }
        }
        out.extend_from_slice(&samples[a0..b0]);
        last_end = Some(b0);
    }
    Some(out)
}

pub fn to_wav(samples: &[f32]) -> Result<Vec<u8>, String> {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: TARGET_RATE,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut cur = Cursor::new(Vec::<u8>::new());
    {
        let mut w = hound::WavWriter::new(&mut cur, spec).map_err(|e| e.to_string())?;
        for &s in samples {
            let v = (s.clamp(-1.0, 1.0) * i16::MAX as f32) as i16;
            w.write_sample(v).map_err(|e| e.to_string())?;
        }
        w.finalize().map_err(|e| e.to_string())?;
    }
    Ok(cur.into_inner())
}
