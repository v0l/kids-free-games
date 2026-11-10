use kg_core::brain::conv::Policy;
use kg_core::brain::Effort;
use kg_core::games::paper::rl::{self, DECIDE, TURNS};
use kg_core::games::paper::world::SUB;
use kg_core::rng as rand;

fn main() {
    kg_core::save::use_memory_only();
    let path = std::env::args()
        .nth(1)
        .unwrap_or("/tmp/kgrl/paper_pop.kgb.live".into());
    let p = Policy::from_bytes(&std::fs::read(&path).unwrap())
        .filter(rl::fits)
        .expect("fits");
    for greedy in [false, true] {
        let (mut flips, mut turns, mut n) = (0, 0, 0);
        for seed in 0..8u64 {
            let mut w = rl::field(seed + 3, 0.5, false);
            rand::srand(seed);
            let mut last = 2usize;
            for _ in 0..(90.0 / DECIDE) as usize {
                if !w.players[0].alive {
                    break;
                }
                let o = rl::observe(&w, 0);
                let a = if greedy {
                    let l = p.logits(&o);
                    (0..l.len()).max_by(|x, y| l[*x].total_cmp(&l[*y])).unwrap()
                } else {
                    p.choose(&o, Effort::BEST)
                };
                let me = &mut w.players[0];
                me.want = kg_core::games::paper::world::wrap_angle(me.angle + TURNS[a]);
                if a != 2 {
                    turns += 1;
                }
                if a != 2 && last != 2 && (a < 2) != (last < 2) {
                    flips += 1;
                }
                last = a;
                n += 1;
                for _ in 0..(DECIDE / SUB).round() as usize {
                    w.step_with(&mut |_, _| None);
                    w.deaths.clear();
                    w.claims.clear();
                }
            }
        }
        println!(
            "{}: turning {:.0}% of decisions, left-right flips {:.0}% of decisions",
            if greedy { "greedy " } else { "sampled" },
            turns as f32 / n as f32 * 100.0,
            flips as f32 / n as f32 * 100.0
        );
    }
}
