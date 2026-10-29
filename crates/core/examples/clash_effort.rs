use kg_core::brain::conv::Policy;
use kg_core::brain::Effort;
use kg_core::games::clash::rl::training_spec;
use kg_core::games::clash::sim::{Event, World};
use std::sync::Arc;

fn games(p: &Arc<Policy>, me: f32, them: f32) -> (f32, f32, f32) {
    let runs: Vec<(f32, f32, f32)> = std::thread::scope(|s| {
        (0..48u64)
            .map(|seed| {
                let p = p.clone();
                s.spawn(move || {
                    kg_core::rng::srand(90_000 + seed);
                    let (mut spec, _) = training_spec(seed * 5);
                    spec.rival = Some(p.clone());
                    spec.rival_share = 1.0;
                    spec.effort = Effort::new(them);
                    let mut w = World::new(spec, "Me", 0);
                    for s in w.snakes.iter_mut().skip(1) {
                        s.rival = Some(p.clone());
                        s.effort = Effort::new(them);
                    }
                    w.snakes[0].player = false;
                    w.snakes[0].rival = Some(p.clone());
                    w.snakes[0].effort = Effort::new(me);
                    let me_id = w.snakes[0].id;
                    let (mut peak, mut kills) = (0u32, 0.0f32);
                    for _ in 0..(600 * 30) {
                        w.step(1.0 / 30.0, None);
                        for e in std::mem::take(&mut w.events) {
                            if let Event::Eaten { by: Some(b), .. } = e {
                                if b == me_id {
                                    kills += 1.0;
                                }
                            }
                        }
                        let alive = w.snakes.iter().any(|s| s.id == me_id && s.alive);
                        if !alive {
                            return (peak as f32, kills, 0.0);
                        }
                        peak = peak.max(w.snakes[0].level());
                        if w.snakes_left() == 1 {
                            return (peak as f32, kills, 1.0);
                        }
                    }
                    (peak as f32, kills, 0.0)
                })
            })
            .collect::<Vec<_>>()
            .into_iter()
            .map(|h| h.join().unwrap())
            .collect()
    });
    let n = runs.len() as f32;
    (
        runs.iter().map(|r| r.0).sum::<f32>() / n,
        runs.iter().map(|r| r.1).sum::<f32>() / n,
        runs.iter().map(|r| r.2).sum::<f32>() / n,
    )
}

fn main() {
    let path = std::env::args().nth(1).expect("kgb");
    let p = Arc::new(Policy::from_bytes(&std::fs::read(path).unwrap()).unwrap());
    kg_core::save::use_memory_only();
    for them in [0.0, 0.5, 1.0] {
        for me in [0.0, 0.25, 0.5, 0.75, 1.0] {
            let (lvl, k, w) = games(&p, me, them);
            println!(
                "opponents {them:.2} | me {me:.2}: level {lvl:.1} kills {k:.1} won {:.0}%",
                w * 100.0
            );
        }
    }
}
