use kg_core::brain::conv::Policy;
use kg_core::brain::Effort;
use kg_core::games::clash::rl::{observe, training_spec, TURNS};
use kg_core::games::clash::sim::{wrap_angle, Event, World};

fn run(p: Option<&Policy>) -> (f32, f32, f32, f32) {
    let (mut near, mut total, mut eggs_near, mut eggs) = (0.0, 0.0, 0.0, 0.0);
    let (mut deaths_near, mut deaths) = (0.0f32, 0.0f32);
    for seed in 0..24u64 {
        kg_core::rng::srand(seed);
        let (spec, _) = training_spec(seed * 5);
        let mut w = World::new(spec, "Me", 0);
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
            let h = w.snakes[0].head();
            let edge = h.x.min(h.y).min(w.size - h.x).min(w.size - h.y);
            for e in std::mem::take(&mut w.events) {
                if let Event::Eaten { player: true, .. } = e {
                    deaths += 1.0;
                    if edge < 400.0 {
                        deaths_near += 1.0;
                    }
                }
            }
            if !w.player_alive() {
                break;
            }
            total += 1.0;
            if edge < 400.0 {
                near += 1.0;
            }
            for e in &w.eggs {
                let d = e
                    .pos
                    .x
                    .min(e.pos.y)
                    .min(w.size - e.pos.x)
                    .min(w.size - e.pos.y);
                eggs += e.value as f32;
                if d < 400.0 {
                    eggs_near += e.value as f32;
                }
            }
        }
    }
    (
        near / total,
        eggs_near / eggs,
        deaths_near / deaths.max(1.0),
        deaths,
    )
}

fn main() {
    let area = 1.0 - (2200.0f32 / 3000.0).powi(2);
    println!(
        "share of the arena within 400 of a wall: {:.0}%",
        area * 100.0
    );
    let (n, e, d, _) = run(None);
    println!("bot:   time near wall {:.0}%, egg value near wall {:.0}%, deaths that were near wall {:.0}%", n * 100.0, e * 100.0, d * 100.0);
    if let Some(path) = std::env::args().nth(1) {
        let p = Policy::from_bytes(&std::fs::read(path).unwrap()).unwrap();
        let (n, e, d, _) = run(Some(&p));
        println!("model: time near wall {:.0}%, egg value near wall {:.0}%, deaths that were near wall {:.0}%", n * 100.0, e * 100.0, d * 100.0);
    }
}
