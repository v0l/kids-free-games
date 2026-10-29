use kg_core::games::paper::rl::Arena;
use std::time::Instant;

fn main() {
    let mut a = Arena::new(4, 600.0);
    let t = Instant::now();
    let n = 2000;
    for k in 0..n {
        a.step((k / 7) % 5);
    }
    let per = t.elapsed().as_secs_f64() / n as f64;
    println!(
        "arena step {:.0} us, {:.0} decisions/s/core",
        per * 1e6,
        1.0 / per
    );
}
