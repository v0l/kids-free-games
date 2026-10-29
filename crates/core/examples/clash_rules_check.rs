use kg_core::games::clash::{
    rating, rl,
    sim::{Event, Spec, World, NECK},
};
fn main() {
    kg_core::save::use_memory_only();
    let mut bad = std::collections::BTreeMap::<String, usize>::new();
    let mut total = 0;
    for seed in 0..16u64 {
        kg_core::rng::srand(seed);
        let mut spec = Spec::arena(10, false);
        spec.rival = rl::shipped_arc();
        spec.rival_share = 1.0;
        spec.effort = rating::effort(rating::START);
        let mut w = World::new(spec, "Me", 0);
        for _ in 0..60 * 120 {
            let before: std::collections::HashMap<u32, (u32, bool, usize)> = w
                .snakes
                .iter()
                .map(|s| (s.id, (s.level(), s.shield > 0.0, s.segs.len())))
                .collect();
            let a = w.autopilot();
            w.step(1.0 / 60.0, Some(a));
            for e in std::mem::take(&mut w.events) {
                let (kind, victim, by) = match e {
                    Event::Cut { victim, by, .. } => ("cut", victim, Some(by)),
                    Event::Eaten { victim, by, .. } => ("eaten", victim, by),
                    _ => continue,
                };
                total += 1;
                let v = before[&victim];
                if v.1 {
                    *bad.entry(format!("{kind} while shielded")).or_default() += 1;
                    let now = w.snakes.iter().find(|s| s.id == victim);
                    println!(
                        "shielded {kind}: victim before lv {} shield {} | now shield {:?}",
                        v.0,
                        v.1,
                        now.map(|s| s.shield)
                    );
                }
                if let Some(b) = by.and_then(|b| before.get(&b)) {
                    if b.0 <= v.0 {
                        println!(
                            "{kind}: by lv {} shield {} | victim lv {} shield {}",
                            b.0, b.1, v.0, v.1
                        );
                        *bad.entry(format!("{kind} by a snake not bigger ({} vs {})", b.0, v.0))
                            .or_default() += 1;
                    }
                }
                let _ = NECK;
            }
            if !w.player_alive() {
                break;
            }
        }
    }
    println!("{total} cuts and kills; rule breaks: {bad:?}");
}
