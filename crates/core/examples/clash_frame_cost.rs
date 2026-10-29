use kg_core::games::clash::{
    rating, rl,
    sim::{Spec, World},
};
use std::time::Instant;
fn main() {
    kg_core::save::use_memory_only();
    for r in [rating::FLOOR, rating::START, rating::TOP] {
        kg_core::rng::srand(3);
        let mut spec = Spec::survival(10, false);
        spec.rival = rl::shipped_arc();
        spec.rival_share = 1.0;
        spec.effort = rating::effort(r);
        let mut w = World::new(spec, "Me", 0);
        w.snakes[0].shield = 1e9;
        let t = Instant::now();
        let frames = 60 * 20;
        for _ in 0..frames {
            w.step(1.0 / 60.0, Some(0.3));
            w.events.clear();
        }
        let ms = t.elapsed().as_secs_f64() * 1000.0 / frames as f64;
        println!(
            "rating {r}: {:.2} ms per 60 fps frame with {} snakes",
            ms,
            w.snakes_left()
        );
    }
}
