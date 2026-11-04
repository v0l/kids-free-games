use kg_core::brain::conv::Policy;
use kg_core::brain::Effort;
use kg_core::games::paper::rl::{evaluate, Pilot, Report};

fn fitness(r: &Report) -> f32 {
    r.wins + r.outlived + 5.0 * r.peak - r.circling
}

fn main() {
    kg_core::save::use_memory_only();
    let bot = evaluate(&Pilot::Bot, 20, None);
    println!(
        "bot fitness {:.3} outlived {:.3} kills/min {:.3} stolen {:.4}",
        fitness(&bot),
        bot.outlived,
        bot.kills / bot.secs.max(1.0) * 60.0,
        bot.stolen
    );
    for path in std::env::args().skip(1) {
        let p = Policy::from_bytes(&std::fs::read(&path).unwrap()).unwrap();
        let r = evaluate(&Pilot::Policy(&p, Effort::BEST), 20, None);
        let pass = fitness(&r) > fitness(&bot)
            && r.outlived > bot.outlived
            && r.circling < 0.25
            && r.kills / r.secs.max(1.0) >= bot.kills / bot.secs.max(1.0)
            && r.stolen >= bot.stolen;
        println!(
            "{} {path}: fitness {:.3} outlived {:.3} kills/min {:.3} stolen {:.4} circling {:.3}",
            if pass { "PASS" } else { "fail" },
            fitness(&r),
            r.outlived,
            r.kills / r.secs.max(1.0) * 60.0,
            r.stolen,
            r.circling
        );
    }
}
