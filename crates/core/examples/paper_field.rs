use kg_core::brain::conv::Policy;
use kg_core::games::paper::rl;
use kg_core::games::paper::world::{wrap_angle, Cause};
use kg_core::games::paper::{Paper, State};
use kg_core::rng as rand;

fn main() {
    kg_core::save::use_memory_only();
    kg_core::profile::create("Brain", 5);
    let path = std::env::args()
        .nth(1)
        .unwrap_or("/tmp/kgrl/paper_pop.kgb.live".into());
    let p = Policy::from_bytes(&std::fs::read(&path).unwrap())
        .filter(rl::fits)
        .expect("fits");
    rl::use_policy(p);
    for best in [0i64, 120, 400] {
        let (mut own, mut other, mut home, mut out, mut loops, mut alive_end, mut land) =
            (0, 0, 0.0f32, 0.0f32, 0, 0, 0.0f32);
        let seeds = 6u64;
        for seed in 0..seeds {
            rand::srand(seed + 50);
            let mut g = Paper::with_best(best);
            g.state = State::Playing;
            g.w.players[0].alive = false;
            let n = g.w.players.len();
            let mut turned = vec![0.0f32; n];
            let mut last: Vec<f32> = g.w.players.iter().map(|p| p.angle).collect();
            for t in 0..60 * 180 {
                g.w.advance(1.0 / 60.0);
                for d in std::mem::take(&mut g.w.deaths) {
                    if d.cause == Cause::OwnTrail {
                        own += 1
                    } else {
                        other += 1
                    }
                }
                g.w.claims.clear();
                for (i, p) in g.w.players.iter().enumerate().skip(1) {
                    if !p.alive {
                        continue;
                    }
                    if p.out() {
                        out += 1.0 / 60.0
                    } else {
                        home += 1.0 / 60.0
                    }
                    turned[i] += wrap_angle(p.angle - last[i]);
                    last[i] = p.angle;
                    if t % 180 == 179 {
                        if turned[i].abs() > std::f32::consts::TAU {
                            loops += 1
                        }
                        turned[i] = 0.0;
                    }
                }
            }
            alive_end += g.w.players.iter().skip(1).filter(|p| p.alive).count();
            land +=
                g.w.players
                    .iter()
                    .enumerate()
                    .skip(1)
                    .filter(|(_, p)| p.alive)
                    .map(|(i, _)| g.w.share(i))
                    .sum::<f32>();
        }
        let s = seeds as f32;
        println!("best {best}: deaths own {:.1} other {:.1} per game, alive at 3min {:.1}, land held {:.1}%, out {:.0}% of time, full circles every 3s {} ",
            own as f32 / s, other as f32 / s, alive_end as f32 / s, land / s * 100.0, out / (out + home) * 100.0, loops);
    }
}
