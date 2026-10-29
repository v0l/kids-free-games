use kg_core::games::clash::{
    rating, rl,
    sim::{Event, Spec, World},
};
fn main() {
    kg_core::save::use_memory_only();
    let mut shown = 0;
    let (mut bumps, mut hits) = (0, 0);
    let mut stunned_me = 0;
    for seed in 0..12u64 {
        kg_core::rng::srand(seed);
        let mut spec = Spec::arena(30, false);
        spec.rival = rl::shipped_arc();
        spec.rival_share = 1.0;
        spec.effort = rating::effort(rating::START);
        let mut w = World::new(spec, "Me", 0);
        w.snakes[0].xp = kg_core::games::clash::sim::xp_for_level(32);
        w.snakes[0].shield = 0.0;
        w.layout_tail(0);
        for _ in 0..60 * 90 {
            let snap: Vec<(u32, u32, f32, f32)> = w
                .snakes
                .iter()
                .map(|s| (s.id, s.level(), s.shield, s.stun))
                .collect();
            let my_head = w.snakes[0].head();
            let a = w.autopilot();
            w.step(1.0 / 60.0, Some(a));
            for e in std::mem::take(&mut w.events) {
                if let Event::Bump { at, player: true } = e {
                    if w.near_wall(at) {
                        continue;
                    }
                    bumps += 1;
                    let me = snap[0];
                    let near = w
                        .snakes
                        .iter()
                        .skip(1)
                        .filter(|s| s.alive)
                        .map(|s| {
                            (
                                s.segs
                                    .iter()
                                    .enumerate()
                                    .map(|(k, p)| ((*p - at).length(), k))
                                    .fold((f32::MAX, 0), |a, b| if b.0 < a.0 { b } else { a }),
                                s,
                            )
                        })
                        .min_by(|a, b| a.0 .0.total_cmp(&b.0 .0));
                    if let Some(((d, k), o)) = near {
                        let ob = snap.iter().find(|x| x.0 == o.id).copied().unwrap();
                        if ob.1 < me.1 && ob.2 <= 0.0 && me.2 <= 0.0 {
                            hits += 1;
                            let moved = (w.snakes[0].head() - my_head).length();
                            if w.snakes[0].stun > 0.3 {
                                stunned_me += 1;
                            }
                            let small = w
                                .snakes
                                .iter()
                                .find(|s| s.id == ob.0)
                                .map(|s| s.stun)
                                .unwrap_or(0.0);
                            if shown < 6 {
                                println!("  small snake stun now {small:.2}");
                                println!(
                                    "  my head moved {moved:.0}, my stun {:.2}",
                                    w.snakes[0].stun
                                );
                                shown += 1;
                                println!("me lv {} shield {:.2} | other lv {} shield {:.2} seg {k} dist {d:.0} other stun {:.2}", me.1, me.2, ob.1, ob.2, ob.3);
                            }
                        }
                    }
                }
            }
            if !w.player_alive() {
                break;
            }
        }
    }
    println!("player bumps {bumps}, of which off an open smaller snake {hits}, of which I was knocked back {stunned_me}");
}
