use kg_core::brain::train::{self, Config};
use kg_core::brain::Net;
use kg_core::games::paper::learn::{imitate, lessons, PaperEnv, FEATURES};

fn main() {
    let secs = std::env::var("SECS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(60.0);
    let scripted_player = std::env::var("PLAYER").is_ok_and(|v| v == "1");
    let cfg = Config::from_env();
    let make = || PaperEnv {
        secs,
        scripted_player,
        ..PaperEnv::default()
    };
    let start = match std::env::var("RESUME") {
        Ok(path) => Net::from_bytes(&std::fs::read(path).expect("read brain")).expect("brain"),
        Err(_) => {
            kg_core::rng::srand(cfg.seed);
            let mut sizes = vec![FEATURES];
            sizes.extend(&cfg.hidden);
            sizes.push(1);
            let mut net = Net::random(&sizes, 1.0);
            let seeds: Vec<u64> = (0..12).collect();
            let taught = lessons(&make(), &seeds);
            eprintln!("{} lessons", taught.len());
            imitate(&mut net, &taught, 30, 0.01);
            net
        }
    };
    let report = train::train_from(make, start, &cfg);
    train::save(&report, "assets/brains/paper.kgb");
}
