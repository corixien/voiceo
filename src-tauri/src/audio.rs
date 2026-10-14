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

pub fn start(app: AppHandle, max_seconds: u32) -> Result<Recorder, String> {
    let (stop_tx, stop_rx) = mpsc::channel::<()>();
    let (ready_tx, ready_rx) = mpsc::channel::<Result<(), String>>();
    let join = std::thread::spawn(move || run(app, max_seconds, stop_rx, ready_tx));
    match ready_rx.recv_timeout(Duration::from_secs(4)) {
        Ok(Ok(())) => Ok(Recorder { stop: stop_tx, join, started: Instant::now() }),
        Ok(Err(e)) => Err(e),
        Err(_) => Err("Mikrofon-Start Timeout".into()),
    }
}

fn run(
    app: AppHandle,
    max_seconds: u32,
    stop_rx: mpsc::Receiver<()>,
    ready: mpsc::Sender<Result<(), String>>,
) -> Result<(Vec<f32>, u32), String> {
    let setup = || -> Result<(cpal::Stream, Arc<Mutex<Vec<f32>>>, u32), String> {
        let host = cpal::default_host();
        let dev = host
            .default_input_device()
            .ok_or("Kein Mikrofon gefunden")?;
        let cfg = dev.default_input_config().map_err(|e| e.to_string())?;
        let rate = cfg.sample_rate().0;
        let channels = cfg.channels() as usize;
        let buf = Arc::new(Mutex::new(Vec::<f32>::with_capacity(rate as usize * 30)));
        let scfg: cpal::StreamConfig = cfg.clone().into();
        let stream = match cfg.sample_format() {
            cpal::SampleFormat::F32 => build::<f32>(&dev, &scfg, channels, buf.clone(), app.clone()),
            cpal::SampleFormat::I16 => build::<i16>(&dev, &scfg, channels, buf.clone(), app.clone()),
            cpal::SampleFormat::U16 => build::<u16>(&dev, &scfg, channels, buf.clone(), app.clone()),
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
            {
                let mut b = buf.lock().unwrap();
                for frame in data.chunks(ch) {
                    let m = frame.iter().map(|&s| f32::from_sample(s)).sum::<f32>() / ch as f32;
                    sum_sq += m * m;
                    n += 1;
                    b.push(m);
                }
            }
            if n > 0 && last.elapsed() >= Duration::from_millis(40) {
                last = Instant::now();
                let _ = app.emit("voiceo://level", (sum_sq / n as f32).sqrt());
            }
        },
        |e| eprintln!("audio error: {e}"),
        None,
    )
    .map_err(|e| e.to_string())
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

/// Trim leading/trailing silence (20 ms windows) keeping 250 ms padding.
pub fn trim_silence(samples: &[f32]) -> Vec<f32> {
    let win = (TARGET_RATE as usize) / 50;
    if samples.len() < win * 4 {
        return samples.to_vec();
    }
    let p = peak(samples);
    let thr = (p * 0.04).max(0.004);
    let loud = |w: &[f32]| (w.iter().map(|s| s * s).sum::<f32>() / w.len() as f32).sqrt() > thr;
    let wins: Vec<bool> = samples.chunks(win).map(loud).collect();
    let first = wins.iter().position(|&b| b);
    let last = wins.iter().rposition(|&b| b);
    match (first, last) {
        (Some(f), Some(l)) => {
            let pad = 12; // 12 * 20ms = 240ms
            let a = f.saturating_sub(pad) * win;
            let b = (((l + 1 + pad) * win)).min(samples.len());
            samples[a..b].to_vec()
        }
        _ => samples.to_vec(),
    }
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
