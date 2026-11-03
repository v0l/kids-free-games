use kg_core::brain::Effort;
use kg_core::games::paper::world::{spread_spots, Cause, World};
use kg_core::rng as rand;

fn run(effort: f32, blunders: bool) -> (usize, usize, usize) {
    let (mut own, mut other, mut alive) = (0, 0, 0);
    for seed in 0..12u64 {
        rand::srand(seed + 100);
        let mut w = World::new(0.0);
        w.rl_effort = Some(effort);
        for s in spread_spots(9) {
            w.add_player(s, rand::gen_range(-3.1, 3.1), true);
        }
        if !blunders {
            for p in w.players.iter_mut() {
                p.blunder = (0, -1e9);
            }
        }
        for _ in 0..60 * 60 {
            w.advance(1.0 / 60.0);
            if !blunders {
                for p in w.players.iter_mut() {
                    p.blunder = (0, -1e9);
                }
            }
            for d in std::mem::take(&mut w.deaths) {
                if d.cause == Cause::OwnTrail {
                    own += 1;
                } else {
                    other += 1;
                }
            }
            w.claims.clear();
        }
        alive += w.players.iter().filter(|p| p.alive).count();
    }
    (own, other, alive)
}

fn main() {
    let _ = Effort::BEST;
    for e in [0.0f32, 0.5, 1.0] {
        for b in [true, false] {
            let (own, other, alive) = run(e, b);
            println!("effort {e:.1} blunders {b:5}: own trail {own:3} other {other:3} alive after 60s {alive:3} of 108");
        }
    }
}
