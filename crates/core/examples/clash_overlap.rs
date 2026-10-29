use kg_core::games::clash::rl::training_spec;
use kg_core::games::clash::sim::World;
fn main() {
    let (mut frames, mut shielded, mut open) = (0, 0, 0);
    for seed in 0..10u64 {
        kg_core::rng::srand(seed);
        let (spec, _) = training_spec(seed * 5 + 4);
        let mut w = World::new(spec, "Me", 0);
        for _ in 0..3000 {
            let a = w.autopilot();
            w.step(1.0 / 30.0, Some(a));
            w.events.clear();
            frames += 1;
            for (i, s) in w.snakes.iter().enumerate().filter(|(_, s)| s.alive) {
                for (j, o) in w.snakes.iter().enumerate() {
                    if i == j || !o.alive {
                        continue;
                    }
                    let r = s.radius() * 0.5 + o.radius() * 0.5;
                    let hit = o.segs.iter().skip(2).any(|p| (*p - s.head()).length() < r);
                    if hit {
                        if s.shield > 0.0 || o.shield > 0.0 {
                            shielded += 1;
                        } else {
                            open += 1;
                        }
                    }
                }
            }
        }
    }
    println!("{frames} frames: head deep inside another body {shielded} times with a shield, {open} without");
}
