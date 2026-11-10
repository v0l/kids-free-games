use kg_core::brain::conv::Policy;
use kg_core::brain::Effort;
use kg_core::games::paper::rl::{self, DECIDE};
use kg_core::games::paper::world::{RES, SUB};
use kg_core::rng as rand;

fn area(pts: &[glam::Vec2]) -> f32 {
    let mut a = 0.0;
    for i in 0..pts.len() {
        let (p, q) = (pts[i], pts[(i + 1) % pts.len()]);
        a += p.x * q.y - q.x * p.y;
    }
    (a * 0.5).abs()
}

fn main() {
    kg_core::save::use_memory_only();
    let path = std::env::args()
        .nth(1)
        .unwrap_or("/tmp/kgrl/paper_pop.kgb.live".into());
    let p = Policy::from_bytes(&std::fs::read(&path).unwrap())
        .filter(rl::fits)
        .expect("fits");
    let (mut claims, mut short) = (0, 0);
    for seed in 0..40u64 {
        let mut w = rl::field(seed + 11, rl::skill_of(seed), false);
        rand::srand(seed);
        for _ in 0..(120.0 / DECIDE) as usize {
            if !w.players[0].alive {
                break;
            }
            w.rl_steer(0, &p, Effort::BEST);
            for _ in 0..(DECIDE / SUB).round() as usize {
                let trail = w.players[0].trail_points();
                let enemy: Vec<u32> = w.counts.clone();
                w.step_with(&mut |_, _| None);
                w.deaths.clear();
                for c in std::mem::take(&mut w.claims) {
                    if c.id != 0 || trail.len() < 10 {
                        continue;
                    }
                    claims += 1;
                    let want = area(&trail) * (RES * RES) as f32;
                    let got = c.cells as f32;
                    if want > 400.0 && got < want * 0.5 {
                        short += 1;
                        let lost: Vec<i64> = enemy
                            .iter()
                            .zip(&w.counts)
                            .skip(1)
                            .map(|(a, b)| *a as i64 - *b as i64)
                            .filter(|d| *d != 0)
                            .collect();
                        let inside = |q: glam::Vec2| {
                            let mut c = false;
                            for i in 0..trail.len() {
                                let (a, b) = (trail[i], trail[(i + 1) % trail.len()]);
                                if (a.y > q.y) != (b.y > q.y)
                                    && q.x < (b.x - a.x) * (q.y - a.y) / (b.y - a.y) + a.x
                                {
                                    c = !c;
                                }
                            }
                            c
                        };
                        let mut kinds = [0usize; 4];
                        let n = kg_core::games::paper::world::N;
                        for i in 0..w.owner.len() {
                            let cell = glam::ivec2(i as i32 % n, i as i32 / n);
                            if !inside(kg_core::games::paper::world::cell_center(cell)) {
                                continue;
                            }
                            let k = match (w.owner[i], w.trail[i]) {
                                (Some(0), _) => 0,
                                (Some(_), _) => 1,
                                (None, Some(_)) => 2,
                                (None, None) => 3,
                            };
                            kinds[k] += 1;
                        }
                        if short == 1 {
                            let lo = w.players[0]
                                .trail
                                .iter()
                                .fold(glam::Vec2::splat(1e9), |a, b| a.min(*b));
                            let hi = trail.iter().fold(glam::Vec2::splat(-1e9), |a, b| a.max(*b));
                            let mut blo = glam::IVec2::splat(99);
                            let mut bhi = glam::IVec2::splat(-1);
                            for (b, c) in w.block_owner.iter().enumerate() {
                                if c[0] > 0 {
                                    let v = glam::ivec2((b % 16) as i32, (b / 16) as i32);
                                    blo = blo.min(v);
                                    bhi = bhi.max(v);
                                }
                            }
                            let tlo = trail.iter().fold(glam::Vec2::splat(1e9), |a, b| a.min(*b));
                            println!("   trail box {tlo} .. {hi} (units), own-land blocks {blo}..{bhi} block={} units {}", kg_core::games::paper::world::BLOCK, kg_core::games::paper::world::BLOCK as f32 / RES as f32);
                            let _ = lo;
                        }
                        if short <= 6 {
                            println!(
                                "   inside after: mine {} others {} trail {} empty {}",
                                kinds[0], kinds[1], kinds[2], kinds[3]
                            );
                        }
                        if short <= 6 {
                            println!("seed {seed}: loop area {:.0} cells, filled {got:.0}, taken from others {}, enemy loss {lost:?}", want, c.taken);
                        }
                    }
                }
            }
        }
    }
    println!("{short} of {claims} loops filled under half their area");
}
