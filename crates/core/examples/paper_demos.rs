use kg_core::games::paper::arena;
use kg_core::games::paper::rl::{self, ACTIONS, DECIDE, TURNS};
use kg_core::games::paper::world::{spread_spots, wrap_angle, World, SUB};
use kg_core::rng as rand;
use std::io::Write;

fn world(seed: u64) -> World {
    rand::srand(seed);
    let skill = rl::skill_of(seed);
    let mut w = World::new(skill);
    w.brains = false;
    let spots = spread_spots(arena::bots_for(skill) + 1);
    w.add_player(spots[0], rand::gen_range(-3.1, 3.1), true);
    for s in &spots[1..] {
        w.add_player(*s, rand::gen_range(-3.1, 3.1), true);
    }
    let p = &mut w.players[0];
    p.tune = arena::tuning(arena::PERSONAS[1], 1.0);
    p.speed = arena::PLAYER_SPEED;
    w
}

fn main() {
    kg_core::save::use_memory_only();
    let want: usize = std::env::args().nth(1).and_then(|v| v.parse().ok()).unwrap_or(60_000);
    let out = std::env::args().nth(2).unwrap_or("/tmp/kgrl/paper_demos.bin".into());
    let threads = 16;
    let per = want / threads;
    let parts: Vec<Vec<u8>> = std::thread::scope(|s| {
        let hs: Vec<_> = (0..threads)
            .map(|t| {
                s.spawn(move || {
                    let mut buf = vec![];
                    let mut n = 0;
                    let mut seed = 500_000 + t as u64 * 100_003;
                    while n < per {
                        let mut w = world(seed);
                        seed += 1;
                        for _ in 0..(rl::GAME_CAP / DECIDE) as usize {
                            if !w.players[0].alive || n >= per {
                                break;
                            }
                            let o = rl::observe(&w, 0);
                            let before = w.players[0].angle;
                            for _ in 0..(DECIDE / SUB).round() as usize {
                                w.step_with(&mut |_, _| None);
                                w.deaths.clear();
                                w.claims.clear();
                            }
                            let turned = wrap_angle(w.players[0].angle - before);
                            let a = (0..ACTIONS)
                                .min_by(|x, y| {
                                    (TURNS[*x] * 0.9 - turned)
                                        .abs()
                                        .total_cmp(&(TURNS[*y] * 0.9 - turned).abs())
                                })
                                .unwrap();
                            buf.extend(o.grid.iter().map(|v| (v * 255.0 + 0.5) as u8));
                            for v in &o.scalars {
                                buf.extend(v.to_le_bytes());
                            }
                            buf.push(a as u8);
                            n += 1;
                        }
                    }
                    buf
                })
            })
            .collect();
        hs.into_iter().map(|h| h.join().unwrap()).collect()
    });
    let mut f = std::fs::File::create(&out).unwrap();
    for p in parts {
        f.write_all(&p).unwrap();
    }
    println!("{} samples, grid {} scalars {} -> {out}", per * threads, rl::grid_len(), rl::SCALARS);
}
