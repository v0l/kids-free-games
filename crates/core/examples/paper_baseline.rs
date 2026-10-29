use kg_core::games::paper::rl::{evaluate, shipped, Pilot};

fn main() {
    kg_core::save::use_memory_only();
    println!("bands bot: {:?}", evaluate(&Pilot::Bot, 40, None));
    for skill in [0.0f32, 0.5, 1.0] {
        let r = evaluate(&Pilot::Bot, 32, Some(skill));
        println!("skill {skill} bot: {r:?}");
    }
    if let Some(p) = shipped() {
        let r = evaluate(&Pilot::Policy(p, kg_core::brain::Effort::BEST), 32, None);
        println!("shipped: {r:?}");
    }
}
