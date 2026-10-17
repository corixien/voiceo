//! Dev tool: RNNoise denoising of a 16 kHz mono wav (used by tools/eval to measure its effect).
use nnnoiseless::DenoiseState;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mut r = hound::WavReader::open(&args[1]).expect("open");
    let spec = r.spec();
    let x: Vec<f32> = r.samples::<i16>().map(|s| s.unwrap() as f32).collect();
    // 16k -> 48k (linear)
    let mut up = Vec::with_capacity(x.len() * 3);
    for i in 0..x.len() {
        let a = x[i];
        let b = *x.get(i + 1).unwrap_or(&a);
        for k in 0..3 {
            up.push(a + (b - a) * (k as f32 / 3.0));
        }
    }
    let mut st = DenoiseState::new();
    let mut out48 = vec![];
    let mut buf = [0f32; DenoiseState::FRAME_SIZE];
    for chunk in up.chunks(DenoiseState::FRAME_SIZE) {
        let mut inp = [0f32; DenoiseState::FRAME_SIZE];
        inp[..chunk.len()].copy_from_slice(chunk);
        st.process_frame(&mut buf, &inp);
        out48.extend_from_slice(&buf[..chunk.len()]);
    }
    // 48k -> 16k (average of 3)
    let out: Vec<i16> = out48.chunks(3).map(|c| (c.iter().sum::<f32>() / c.len() as f32).clamp(-32768.0, 32767.0) as i16).collect();
    let mut w = hound::WavWriter::create(&args[2], spec).expect("create");
    for s in out {
        w.write_sample(s).unwrap();
    }
    w.finalize().unwrap();
}
