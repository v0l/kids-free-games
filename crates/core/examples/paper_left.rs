use kg_core::games::paper::{world::Cause, Paper, State};
fn main() {
    kg_core::save::use_memory_only();
    kg_core::profile::create("Brain", 5);
    for (seed, rl, best) in [
        (0u64, true, 0i64),
        (1, true, 0),
        (0, false, 0),
        (1, false, 0),
        (0, true, 120),
        (0, true, 250),
    ] {
        kg_core::rng::srand(seed);
        let mut g = Paper::with_best(best);
        if !rl {
            for p in g.w.players.iter_mut() {
                p.rl = None;
            }
            g.w.rl_effort = None;
        }
        g.state = State::Playing;
        g.w.players[0].speed = 0.0;
        let n = g.w.players.len() - 1;
        let mut causes = std::collections::BTreeMap::new();
        let mut at = vec![];
        for t in 0..60 * 120 {
            g.w.advance(1.0 / 60.0);
            for d in std::mem::take(&mut g.w.deaths) {
                let k = match (d.cause, d.killer.is_some()) {
                    (Cause::OwnTrail, _) => "own trail",
                    (_, true) => "killed by bot",
                    (Cause::HeadOn, _) => "head on",
                    _ => "other",
                };
                *causes.entry(k).or_insert(0) += 1;
            }
            g.w.claims.clear();
            if t % 1800 == 1799 {
                at.push(g.rivals_left());
            }
        }
        println!(
            "seed {seed} rl {rl} best {best}: {n} bots, left every 30s {at:?}, deaths {causes:?}"
        );
    }
}
