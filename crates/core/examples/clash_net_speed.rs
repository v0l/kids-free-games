use kg_core::brain::conv::Policy;
use kg_core::games::clash::rl::{observe, Arena};
use std::time::Instant;

fn main() {
    let path = std::env::args().nth(1).expect("kgb");
    let p = Policy::from_bytes(&std::fs::read(path).unwrap()).unwrap();
    let a = Arena::new(7, 600.0);
    let n = 2000;
    let t = Instant::now();
    let mut sink = 0usize;
    for _ in 0..n {
        sink += std::hint::black_box(observe(&a.w, 0)).grid.len();
    }
    let look = t.elapsed().as_secs_f64() * 1e6 / n as f64;
    let o = observe(&a.w, 0);
    let t = Instant::now();
    let mut s = 0.0;
    for _ in 0..n {
        s += std::hint::black_box(p.logits(std::hint::black_box(&o)))[0];
    }
    let net = t.elapsed().as_secs_f64() * 1e6 / n as f64;
    println!(
        "{} weights: observe {look:.0} us, network {net:.0} us per decision ({sink} {s:.0})",
        p.param_count()
    );
}
