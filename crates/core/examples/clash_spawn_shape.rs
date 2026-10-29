use kg_core::games::clash::rl::training_spec;
use kg_core::games::clash::sim::World;
fn main() {
    let mut straight = 0;
    let mut total = 0;
    for seed in 0..10u64 {
        kg_core::rng::srand(seed);
        let (spec, _) = training_spec(seed * 5 + 4);
        let mut w = World::new(spec, "Me", 0);
        let mut seen: std::collections::HashSet<u32> = w.snakes.iter().map(|s| s.id).collect();
        for _ in 0..3000 {
            let a = w.autopilot();
            w.step(1.0 / 30.0, Some(a));
            w.events.clear();
            for s in &w.snakes {
                if seen.insert(s.id) && s.segs.len() > 20 {
                    total += 1;
                    let a = (s.segs[1] - s.segs[0]).normalize();
                    let b = (s.segs[s.segs.len() - 1] - s.segs[s.segs.len() - 2]).normalize();
                    if a.dot(b) > 0.999 {
                        straight += 1;
                        if straight < 4 {
                            println!(
                                "straight: level {} segs {} head {:?} age {}",
                                s.level(),
                                s.segs.len(),
                                s.segs[0],
                                s.age
                            );
                        }
                    }
                }
            }
        }
    }
    println!("spawned snakes over 20 segments: {total}, perfectly straight: {straight}");
}
