use kg_core::games::paper::world::{spread_spots, wrap_angle, Cause, World};
use kg_core::rng as rand;

#[derive(Clone, Copy)]
enum Who {
    Plan,
    Es(f32),
    Rl(f32),
}

fn run(label: &str, who: Who) {
    let (mut own, mut other, mut alive_secs) = (0u32, 0u32, 0.0f32);
    let (mut loops, mut windows, mut home_secs) = (0u32, 0u32, 0.0f32);
    for seed in 0..8 {
        rand::srand(seed + 1);
        let mut w = World::new(1.0);
        w.brains = matches!(who, Who::Es(_));
        if let Who::Es(e) = who {
            w.effort = Some(e);
        }
        for s in spread_spots(10) {
            w.add_player(s, rand::gen_range(-3.1, 3.1), true);
        }
        if let Who::Rl(e) = who {
            for p in w.players.iter_mut() {
                p.rl = Some(kg_core::brain::Effort::new(e));
            }
        }
        let mut turned = [0.0f32; 10];
        let mut last: Vec<f32> = w.players.iter().map(|p| p.angle).collect();
        let mut window = [0usize; 10];
        for _ in 0..60 * 60usize {
            w.advance(1.0 / 60.0);
            w.respawn_bots(1.0 / 60.0);
            if let Who::Rl(e) = who {
                for p in w.players.iter_mut().filter(|p| p.rl.is_none()) {
                    p.rl = Some(kg_core::brain::Effort::new(e));
                }
            }
            for (i, p) in w.players.iter().enumerate() {
                let home = p.alive && !p.out() && w.owner_at(p.pos) == Some(i as u8);
                if p.alive {
                    alive_secs += 1.0 / 60.0;
                }
                if home {
                    home_secs += 1.0 / 60.0;
                    turned[i] += wrap_angle(p.angle - last[i]);
                    window[i] += 1;
                    if window[i] >= 120 {
                        windows += 1;
                        if turned[i].abs() >= std::f32::consts::TAU * 0.75 {
                            loops += 1;
                        }
                        turned[i] = 0.0;
                        window[i] = 0;
                    }
                } else {
                    turned[i] = 0.0;
                    window[i] = 0;
                }
                last[i] = p.angle;
            }
            for d in w.deaths.drain(..) {
                match d.cause {
                    Cause::OwnTrail => own += 1,
                    _ => other += 1,
                }
            }
            w.claims.clear();
        }
    }
    let per_min = |n: u32| n as f32 / (alive_secs / 60.0);
    println!(
        "{label:<38} on own land {:>3.0}% of the time, circling there {:>4.1}% of it   self-crashes {:.2}/min   other deaths {:.2}/min",
        home_secs / alive_secs.max(1.0) * 100.0,
        loops as f32 / windows.max(1) as f32 * 100.0,
        per_min(own),
        per_min(other)
    );
}

fn main() {
    run("hand-written plan", Who::Plan);
    run("ES brain effort 0.1 (fresh profile)", Who::Es(0.1));
    run("ES brain effort 0.5", Who::Es(0.5));
    run("ES brain effort 1.0", Who::Es(1.0));
    run("RL policy effort 0.3", Who::Rl(0.3));
    run("RL policy effort 1.0", Who::Rl(1.0));
}
