use kg_core::brain::Effort;
use kg_core::games::paper::rl::{observe, shipped, DECIDE, TURNS};
use kg_core::games::paper::world::{wrap_angle, SUB};
use kg_core::rng as rand;

fn main() {
    let policy = shipped().expect("policy");
    kg_core::save::use_memory_only();
    for rl in [false, true] {
        let (mut stall_total, mut longest, mut regions, mut empty_reach) =
            (0.0f32, 0.0f32, 0.0f32, 0.0f32);
        let mut gain_secs = 0.0f32;
        let (mut loops, mut loops_out, mut windows) = (0u32, 0u32, 0u32);
        let (mut big_secs, mut big_stall) = (0.0f32, 0.0f32);
        let seeds = 12;
        let secs = 120.0;
        for seed in 0..seeds {
            rand::srand(5_000 + seed);
            let mut g = kg_core::games::paper::Paper::with_best(0);
            g.state = kg_core::games::paper::State::Playing;
            g.w.players[0].speed = kg_core::games::paper::arena::PLAYER_SPEED;
            let mut w = g.w;
            if !rl {
                w.players[0].bot = true;
            }
            let mut last_land = w.counts[0];
            let mut since_gain = 0.0f32;
            let mut worst = 0.0f32;
            let mut touched = [false; 16];
            let mut empty_seen = [false; 16];
            let mut turned = 0.0f32;
            let mut last_angle = w.players[0].angle;
            let mut window = 0usize;
            let ticks = (DECIDE / SUB).round() as usize;
            for _ in 0..(secs / DECIDE) as usize {
                if !w.players[0].alive {
                    break;
                }
                if rl {
                    let o = observe(&w, 0);
                    let a = policy.choose(&o, Effort::BEST);
                    let p = &mut w.players[0];
                    p.want = wrap_angle(p.angle + TURNS[a]);
                }
                for _ in 0..ticks {
                    w.step_with(&mut |_, _| None);
                    w.deaths.clear();
                    w.claims.clear();
                }
                w.respawn_bots(DECIDE);
                let a0 = w.players[0].angle;
                turned += wrap_angle(a0 - last_angle);
                last_angle = a0;
                window += 1;
                if window as f32 * DECIDE >= 3.0 {
                    windows += 1;
                    if turned.abs() >= std::f32::consts::TAU {
                        loops += 1;
                        if w.players[0].out() {
                            loops_out += 1;
                        }
                    }
                    turned = 0.0;
                    window = 0;
                }
                if w.share(0) > 0.08 {
                    big_secs += DECIDE;
                    if since_gain > 6.0 {
                        big_stall += DECIDE;
                    }
                }
                let land = w.counts[0];
                if land > last_land {
                    gain_secs += DECIDE;
                    since_gain = 0.0;
                } else {
                    since_gain += DECIDE;
                    worst = worst.max(since_gain);
                }
                last_land = land;
                for q in 0..16 {
                    let (by, bx) = (q / 4, q % 4);
                    let mut mine = 0u32;
                    let mut anyone = 0u32;
                    for y in by * 4..by * 4 + 4 {
                        for x in bx * 4..bx * 4 + 4 {
                            let b = &w.block_owner[y * 16 + x];
                            mine += b[0] as u32;
                            anyone += b.iter().map(|v| *v as u32).sum::<u32>();
                        }
                    }
                    if mine > 0 {
                        touched[q] = true;
                    }
                    if anyone == 0 {
                        empty_seen[q] = true;
                    }
                }
            }
            stall_total += worst;
            longest = longest.max(worst);
            regions += touched.iter().filter(|t| **t).count() as f32;
            let empties = empty_seen.iter().filter(|e| **e).count().max(1) as f32;
            empty_reach +=
                (0..16).filter(|q| empty_seen[*q] && touched[*q]).count() as f32 / empties;
        }
        let n = seeds as f32;
        println!(
            "{}: full circles in {:.1}% of 3 s windows ({} of {} while out drawing a trail); once over 8% land, {:.0}% of the time is >6 s since the last capture",
            if rl { "RL policy" } else { "old bot  " },
            loops as f32 / windows.max(1) as f32 * 100.0,
            loops_out,
            loops,
            big_stall / big_secs.max(0.01) * 100.0
        );
        println!(
            "{}: longest stretch without new land {:.1}s on average (worst {:.1}s), gaining land {:.0}% of the time, owns land in {:.1} of 16 map quarters, reached {:.0}% of quarters that were empty",
            if rl { "RL policy" } else { "old bot  " },
            stall_total / n,
            longest,
            gain_secs / (n * secs) * 100.0,
            regions / n,
            empty_reach / n * 100.0
        );
    }
}
