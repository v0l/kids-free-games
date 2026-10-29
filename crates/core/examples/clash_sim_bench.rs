use kg_core::games::clash::rl::{observe, Arena};
use std::time::Instant;

fn main() {
    let secs: f64 = std::env::args()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(10.0);
    let mut arenas: Vec<Arena> = (0..16)
        .map(|i| Arena::new(1000 + i * 7919, 120.0))
        .collect();
    let start = Instant::now();
    let (mut steps, mut sim, mut look) = (0u64, 0.0f64, 0.0f64);
    let mut k = 0usize;
    while start.elapsed().as_secs_f64() < secs {
        for a in arenas.iter_mut() {
            let t = Instant::now();
            a.step(k % 5);
            sim += t.elapsed().as_secs_f64();
            let t = Instant::now();
            std::hint::black_box(observe(&a.w, 0));
            look += t.elapsed().as_secs_f64();
            steps += 1;
            k = k.wrapping_mul(1103515245).wrapping_add(12345) >> 3;
        }
    }
    println!(
        "{steps} decisions on one thread: {:.0}/s, step {:.1} us, observe {:.1} us",
        steps as f64 / start.elapsed().as_secs_f64(),
        sim / steps as f64 * 1e6,
        look / steps as f64 * 1e6
    );
}
