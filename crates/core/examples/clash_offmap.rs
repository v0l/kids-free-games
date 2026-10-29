use kg_core::games::clash::rl::training_spec;
use kg_core::games::clash::sim::World;
fn main() {
    let (mut bad, mut frames, mut first) = (0usize, 0usize, None);
    for seed in 0..20u64 {
        kg_core::rng::srand(seed);
        let (spec, _) = training_spec(seed * 5 + 4);
        let mut w = World::new(spec, "Me", 0);
        for f in 0..3000 {
            let a = w.autopilot();
            w.step(1.0 / 30.0, Some(a));
            w.events.clear();
            frames += 1;
            for s in w.snakes.iter().filter(|s| s.alive) {
                let out = s
                    .segs
                    .iter()
                    .filter(|p| p.x < 0.0 || p.y < 0.0 || p.x > w.size || p.y > w.size)
                    .count();
                if out > 0 {
                    bad += 1;
                    if first.is_none() {
                        let k = s
                            .segs
                            .iter()
                            .position(|p| p.x < 0.0 || p.y < 0.0 || p.x > w.size || p.y > w.size)
                            .unwrap();
                        println!(
                            "seg {k} at {:?}, prev {:?}, head {:?}, stun {}, shield {}",
                            s.segs[k],
                            s.segs[k.saturating_sub(1)],
                            s.segs[0],
                            s.stun,
                            s.shield
                        );
                        first = Some((seed, f, s.level(), s.segs.len(), out, s.age));
                    }
                }
            }
        }
    }
    println!("snake-frames with segments off the map: {bad} of {frames} frames; first {first:?} (seed, frame, level, segs, off, age)");
}
