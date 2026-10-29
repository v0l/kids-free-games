use kg_core::games::clash::{
    rating, rl,
    sim::{Event, Spec, World},
};
fn main() {
    kg_core::save::use_memory_only();
    let (mut frames, mut shielded_snake_frames, mut snake_frames) = (0usize, 0usize, 0usize);
    let (
        mut cuts,
        mut bumps_lower_open,
        mut bumps_shielded,
        mut bumps_equal,
        mut bumps_bigger_tail,
    ) = (0, 0, 0, 0, 0);
    for seed in 0..8u64 {
        kg_core::rng::srand(seed);
        let mut spec = Spec::arena(10, false);
        spec.rival = rl::shipped_arc();
        spec.rival_share = 1.0;
        spec.effort = rating::effort(rating::START);
        let mut w = World::new(spec, "Me", 0);
        for _ in 0..60 * 120 {
            let before: Vec<(u32, u32, bool)> = w
                .snakes
                .iter()
                .map(|s| (s.id, s.level(), s.shield > 0.0))
                .collect();
            let a = w.autopilot();
            w.step(1.0 / 60.0, Some(a));
            frames += 1;
            for s in w.snakes.iter().filter(|s| s.alive) {
                snake_frames += 1;
                if s.shield > 0.0 {
                    shielded_snake_frames += 1;
                }
            }
            for e in std::mem::take(&mut w.events) {
                match e {
                    Event::Cut { .. } => cuts += 1,
                    Event::Bump { at, player: true } if !w.near_wall(at) => {
                        let me = before[0];
                        let other = w.snakes.iter().skip(1).filter(|s| s.alive).min_by(|a, b| {
                            a.segs
                                .iter()
                                .map(|p| (*p - at).length())
                                .fold(f32::MAX, f32::min)
                                .total_cmp(
                                    &b.segs
                                        .iter()
                                        .map(|p| (*p - at).length())
                                        .fold(f32::MAX, f32::min),
                                )
                        });
                        if let Some(o) = other {
                            let o_before = before
                                .iter()
                                .find(|b| b.0 == o.id)
                                .copied()
                                .unwrap_or((o.id, o.level(), o.shield > 0.0));
                            if me.2 || o_before.2 {
                                bumps_shielded += 1;
                            } else if o_before.1 == me.1 {
                                bumps_equal += 1;
                            } else if o_before.1 < me.1 {
                                bumps_lower_open += 1;
                                println!("lower bump: me lv {} | other lv {} | me stunned {} other stunned {}", me.1, o_before.1, w.snakes[0].stun > 0.0, o.stun > 0.0);
                            } else {
                                bumps_bigger_tail += 1;
                            }
                        }
                    }
                    _ => {}
                }
            }
            if !w.player_alive() {
                break;
            }
        }
    }
    println!(
        "{frames} frames; snakes shielded {:.0}% of the time",
        100.0 * shielded_snake_frames as f32 / snake_frames as f32
    );
    println!("player bumps: shielded {bumps_shielded}, equal level {bumps_equal}, LOWER and open {bumps_lower_open}, bigger tail {bumps_bigger_tail}; cuts {cuts}");
}
