use kg_core::brain::Effort;
use kg_core::games::paper::rl::{evaluate, shipped, Pilot};

fn main() {
    kg_core::save::use_memory_only();
    let p = shipped().expect("policy");
    let b = evaluate(&Pilot::Bot, 160, None);
    println!(
        "bot      won {:.0}% outlived {:.0}% peak {:.1}%",
        b.wins * 100.0,
        b.outlived * 100.0,
        b.peak * 100.0
    );
    for e in [0.0f32, 0.25, 0.5, 0.75, 0.9, 1.0] {
        let r = evaluate(&Pilot::Policy(p, Effort::new(e)), 160, None);
        println!(
            "effort {e:.2} won {:.0}% outlived {:.0}% peak {:.1}% circling {:.1}%",
            r.wins * 100.0,
            r.outlived * 100.0,
            r.peak * 100.0,
            r.circling * 100.0
        );
    }
}
