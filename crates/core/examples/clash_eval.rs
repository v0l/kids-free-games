use kg_core::brain::conv::Policy;
use kg_core::brain::Effort;
use kg_core::games::clash::rl::{evaluate, Pilot, Report, GAME_CAP};

fn show(name: &str, r: &Report) {
    println!(
        "{name}: won {:.0}% level {:.1} lived {:.0}s died {:.0}% kills {:.1} stalled {:.0}% circling {:.1}%",
        r.wins * 100.0,
        r.level, r.secs, r.deaths * 100.0, r.kills, r.stall * 100.0, r.circling * 100.0
    );
}

fn main() {
    let path = std::env::args().nth(1).expect("kgb path");
    let p = Policy::from_bytes(&std::fs::read(path).unwrap()).expect("valid kgb");
    show("hand bot", &evaluate(&Pilot::Bot, 64, GAME_CAP));
    for e in [1.0, 0.5, 0.0] {
        show(
            &format!("model effort {e}"),
            &evaluate(&Pilot::Policy(&p, Effort::new(e)), 64, GAME_CAP),
        );
    }
}
