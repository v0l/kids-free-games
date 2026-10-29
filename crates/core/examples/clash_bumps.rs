use kg_core::brain::conv::Policy;
use kg_core::brain::Effort;
use kg_core::games::clash::rl::{observe, training_spec, TURNS};
use kg_core::games::clash::sim::{wrap_angle, Event, World};

fn run(p: Option<&Policy>) -> (f32, f32, f32) {
    let (mut snake_bumps, mut repeat, mut secs) = (0.0, 0.0, 0.0);
    for seed in 0..24u64 {
        kg_core::rng::srand(seed);
        let (spec, _) = training_spec(seed * 5);
        let mut w = World::new(spec, "Me", 0);
        let mut last_bump = -10.0f32;
        for _ in 0..3000 {
            let want = match p {
                Some(p) => {
                    wrap_angle(w.snakes[0].angle + TURNS[p.choose(&observe(&w, 0), Effort::BEST)])
                }
                None => w.autopilot(),
            };
            for _ in 0..3 {
                w.step(1.0 / 30.0, Some(want));
            }
            for e in std::mem::take(&mut w.events) {
                if let Event::Bump { player: true, at } = e {
                    if !w.near_wall(at) {
                        snake_bumps += 1.0;
                        if w.t - last_bump < 2.0 {
                            repeat += 1.0;
                        }
                        last_bump = w.t;
                    }
                }
            }
            if !w.player_alive() {
                break;
            }
            secs += 0.1;
        }
    }
    (
        snake_bumps / secs * 60.0,
        repeat / snake_bumps.max(1.0),
        secs,
    )
}

fn main() {
    let (b, r, _) = run(None);
    println!(
        "bot:   {b:.1} snake bumps per minute, {:.0}% within 2s of the last",
        r * 100.0
    );
    if let Some(path) = std::env::args().nth(1) {
        let p = Policy::from_bytes(&std::fs::read(path).unwrap()).unwrap();
        let (b, r, _) = run(Some(&p));
        println!(
            "model: {b:.1} snake bumps per minute, {:.0}% within 2s of the last",
            r * 100.0
        );
    }
}
