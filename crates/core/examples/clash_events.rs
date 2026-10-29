use kg_core::games::clash::rl::training_spec;
use kg_core::games::clash::sim::{Event, World};

fn main() {
    let (mut cut, mut eaten, mut bump_snake, mut bump_wall, mut equal) = (0, 0, 0, 0, 0);
    for seed in 0..10u64 {
        kg_core::rng::srand(seed);
        let (spec, _) = training_spec(seed);
        let mut w = World::new(spec, "Me", 0);
        for _ in 0..(120 * 30) {
            let a = w.autopilot();
            w.step(1.0 / 30.0, Some(a));
            for e in std::mem::take(&mut w.events) {
                match e {
                    Event::Cut { .. } => cut += 1,
                    Event::Eaten { .. } => eaten += 1,
                    Event::Bump { at, .. } if w.near_wall(at) => bump_wall += 1,
                    Event::Bump { .. } => bump_snake += 1,
                    _ => {}
                }
            }
            if !w.player_alive() {
                break;
            }
        }
        let lv: Vec<u32> = w
            .snakes
            .iter()
            .filter(|s| s.alive)
            .map(|s| s.level())
            .collect();
        let mut s = lv.clone();
        s.sort();
        s.dedup();
        equal += lv.len() - s.len();
    }
    println!("per 10 games: cuts {cut} eaten {eaten} snake bumps {bump_snake} wall bumps {bump_wall}; snakes sharing a level at the end {equal}");
}
