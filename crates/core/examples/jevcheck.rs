use kg_core::engine::{Game, View};
use kg_core::games::flappy::{Flappy, GAME};
use kg_core::jev::{yalr_key, Player};

fn main() {
    kg_core::save::use_memory_only();
    kg_core::profile::create("Jev", 4);
    kg_core::profile::set(GAME, "passed", 1);
    let mut p = Player::new(yalr_key().expect("no yalr key"));
    let mut g = Flappy::new();
    let v = View::ipad_landscape();
    let start = std::time::Instant::now();
    let mut last = start;
    while start.elapsed().as_secs() < 40 {
        std::thread::sleep(std::time::Duration::from_millis(16));
        let dt = last.elapsed().as_secs_f32().min(0.05) * p.clock();
        last = std::time::Instant::now();
        let input = p.drive(&g, &v, dt);
        g.update(&input, &v, dt);
    }
    println!("calls {} runs {} best {}", p.calls, p.runs, p.best);
    for (l, _) in p.lines(g.score) {
        println!("{l}");
    }
}
