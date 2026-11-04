use kg_core::games::clash::rl::{self, ACTIONS, DECIDE, FRAME, TURNS};
use kg_core::games::clash::sim::{wrap_angle, World};
use kg_core::rng as rand;
use std::io::Write;

fn main() {
    kg_core::save::use_memory_only();
    let want: usize = std::env::args()
        .nth(1)
        .and_then(|v| v.parse().ok())
        .unwrap_or(60_000);
    let out = std::env::args()
        .nth(2)
        .unwrap_or("/tmp/kgrl/clash_demos.bin".into());
    let threads = 16;
    let per = want / threads;
    let parts: Vec<(Vec<u8>, f32)> = std::thread::scope(|s| {
        let hs: Vec<_> = (0..threads)
            .map(|t| {
                s.spawn(move || {
                    let mut buf = vec![];
                    let mut top = 0.0f32;
                    let mut n = 0;
                    let mut seed = 900_000 + t as u64 * 100_003;
                    while n < per {
                        rand::srand(seed);
                        let (spec, _) = rl::training_spec(seed);
                        seed += 1;
                        let mut w = World::new(spec, "Demo", 0);
                        for _ in 0..(rl::GAME_CAP / DECIDE) as usize {
                            if !w.player_alive() || w.won() || n >= per {
                                break;
                            }
                            let o = rl::observe(&w, 0);
                            let want = w.autopilot();
                            let turn = wrap_angle(want - w.snakes[0].angle);
                            let a = (0..ACTIONS)
                                .min_by(|x, y| {
                                    (TURNS[*x] - turn)
                                        .abs()
                                        .total_cmp(&(TURNS[*y] - turn).abs())
                                })
                                .unwrap();
                            top = o.grid.iter().fold(top, |m, v| m.max(*v));
                            buf.extend(
                                o.grid
                                    .iter()
                                    .map(|v| (v.clamp(0.0, 1.0) * 255.0 + 0.5) as u8),
                            );
                            for v in &o.scalars {
                                buf.extend(v.to_le_bytes());
                            }
                            buf.push(a as u8);
                            n += 1;
                            for _ in 0..(DECIDE / FRAME).round() as usize {
                                w.step(FRAME, Some(want));
                                w.events.clear();
                            }
                        }
                    }
                    (buf, top)
                })
            })
            .collect();
        hs.into_iter().map(|h| h.join().unwrap()).collect()
    });
    let top = parts.iter().fold(0.0f32, |m, p| m.max(p.1));
    let mut f = std::fs::File::create(&out).unwrap();
    for p in &parts {
        f.write_all(&p.0).unwrap();
    }
    println!(
        "{} samples, grid {} scalars {}, max grid value {top} -> {out}",
        per * threads,
        rl::grid_len(),
        rl::SCALARS
    );
}
