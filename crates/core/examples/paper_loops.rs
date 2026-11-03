use kg_core::brain::conv::Policy;
use kg_core::brain::Effort;
use kg_core::games::paper::rl::{self, DECIDE};
use kg_core::games::paper::world::{SUB, N};
use kg_core::games::paper::{Paper, State};
use kg_core::rng as rand;

fn main() {
    kg_core::save::use_memory_only();
    kg_core::profile::create("Brain", 5);
    let path = std::env::args().nth(1).unwrap_or("/tmp/kgrl/paper_pop.kgb.live".into());
    let bot = path == "bot";
    let p = if bot {
        Policy::from_bytes(&std::fs::read("/tmp/kgrl/paper_pop.kgb.live").unwrap()).filter(rl::fits).expect("fits")
    } else {
        Policy::from_bytes(&std::fs::read(&path).unwrap()).filter(rl::fits).expect("fits")
    };
    rl::use_policy(p.clone());
    let (mut claims, mut cells, mut far, mut outs, mut secs, mut land) = (0usize, 0usize, 0.0f32, 0usize, 0.0f32, 0.0f32);
    for seed in 0..8u64 {
        rand::srand(seed + 7);
        let mut g = Paper::with_best(0);
        g.state = State::Playing;
        g.w.players[0].speed = kg_core::games::paper::arena::PLAYER_SPEED;
        g.w.players[0].manual = !bot;
        if bot {
            g.w.players[0].bot = true;
            g.w.players[0].tune = kg_core::games::paper::arena::tuning(kg_core::games::paper::arena::PERSONAS[1], 1.0);
        }
        let mut reach = 0.0f32;
        let mut start = g.w.players[0].pos;
        let mut was_out = false;
        for _ in 0..(120.0 / DECIDE) as usize {
            if !g.w.players[0].alive { break; }
            if !bot {
                g.w.rl_steer(0, &p, Effort::BEST);
            }
            for _ in 0..(DECIDE / SUB).round() as usize {
                g.w.step_with(&mut |_, _| None);
                for c in std::mem::take(&mut g.w.claims) {
                    if c.id == 0 { claims += 1; cells += c.cells; }
                }
                g.w.deaths.clear();
            }
            secs += DECIDE;
            let me = &g.w.players[0];
            if me.out() {
                if !was_out { start = me.pos; reach = 0.0; }
                reach = reach.max(me.pos.distance(start));
            } else if was_out {
                far += reach; outs += 1;
            }
            was_out = me.out();
        }
        land += g.w.share(0);
    }
    let per = (N * N) as f32;
    println!("trips/min {:.1}, mean claim {:.2}% of map, mean farthest point per trip {:.1} units, land after 2min {:.1}%",
        outs as f32 / secs * 60.0, cells as f32 / claims.max(1) as f32 / per * 100.0, far / outs.max(1) as f32, land / 8.0 * 100.0);
}
