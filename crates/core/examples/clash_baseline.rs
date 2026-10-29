use kg_core::games::clash::rl::{evaluate, Pilot, GAME_CAP};
fn main() {
    let t = std::time::Instant::now();
    let r = evaluate(&Pilot::Bot, 16, GAME_CAP);
    println!(
        "bot: level {:.1} lived {:.0}s died {:.0}% kills {:.1} ({:.1}s wall)",
        r.level,
        r.secs,
        r.deaths * 100.0,
        r.kills,
        t.elapsed().as_secs_f32()
    );
}
