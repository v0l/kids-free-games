use kg_core::brain::conv::{Layer, Obs};
use kg_core::games::paper::rl::{observe, shipped, Arena};
use std::time::Instant;

fn main() {
    let a = Arena::new(1, 60.0);
    let p = shipped().expect("policy");
    let o = observe(&a.w, 0);
    let n = 500;
    let t = Instant::now();
    let mut sink = 0.0;
    for _ in 0..n {
        sink += std::hint::black_box(p.logits(std::hint::black_box(&o)))[0];
    }
    println!(
        "whole net: {:.1} us ({} weights) {sink:.0}",
        t.elapsed().as_secs_f64() * 1e6 / n as f64,
        p.param_count()
    );
    let mut side = p.grid;
    let mut ch = p.channels;
    for l in &p.trunk {
        if let Layer::Conv {
            cout,
            k,
            stride,
            pad,
            ..
        } = l
        {
            let out = (side + 2 * pad - k) / stride + 1;
            let macs = cout * out * out * ch * k * k;
            println!(
                "  conv {ch}->{cout} {side}x{side} -> {out}x{out}: {:.2} M multiply-adds",
                macs as f64 / 1e6
            );
            side = out;
            ch = *cout;
        }
    }
    let _ = Obs::default();
}
