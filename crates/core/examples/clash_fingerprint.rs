use kg_core::games::clash::rl::training_spec;
use kg_core::games::clash::sim::World;

fn main() {
    let mut h: u64 = 0xcbf29ce484222325;
    let mut mix = |v: u64| {
        h ^= v;
        h = h.wrapping_mul(0x100000001b3);
    };
    for seed in 0..6u64 {
        kg_core::rng::srand(seed);
        let (spec, _) = training_spec(seed);
        let mut w = World::new(spec, "Me", 0);
        for _ in 0..(60 * 30) {
            let a = w.autopilot();
            w.step(1.0 / 30.0, Some(a));
            for e in std::mem::take(&mut w.events) {
                mix(format!("{e:?}").len() as u64);
            }
        }
        for s in &w.snakes {
            mix(s.xp as u64);
            mix(s.segs.len() as u64);
            mix(s.head().x.to_bits() as u64);
            mix(s.alive as u64);
        }
        mix(w.eggs.len() as u64);
    }
    println!("{h:016x}");
}
