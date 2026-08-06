//! Numbers, not WAVs: how much the gain actually moves within one note.
use fibonacci_dsp::{Breath, RatioMode};
const SR: f32 = 48_000.0;
fn main() {
    for &rate in &[8.0_f32, 4.0, 1.618, 0.5] {
        print!("{rate:>6.3} hz notes | ");
        for &field in &[0.25_f32, 0.5, 0.75, 1.0] {
            let mut b = Breath::new(SR, RatioMode::Golden);
            let step = (SR / rate) as usize;
            for _ in 0..4 {
                b.trigger();
                for _ in 0..step { b.tick(110.0, field, 0.5, 0.5); }
            }
            b.trigger();
            let (mut lo, mut hi) = (f32::MAX, f32::MIN);
            for _ in 0..step {
                let g = b.tick(110.0, field, 0.5, 0.5).gain;
                lo = lo.min(g); hi = hi.max(g);
            }
            print!("field {field:.2}: {:.2} dB   ", 20.0 * (hi / lo).log10());
        }
        println!();
    }
}
