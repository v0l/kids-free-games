use kg_core::brain::train::{self, Config};
use kg_core::brain::{Effort, Net};
use kg_core::games::hotel::run::{hotel_for, play_net, HotelEnv, EPISODE_SECS};
use kg_core::games::hotel::sim::CREW_FEATURES;
use std::sync::Arc;

fn main() {
    let cfg = Config::from_env();
    let report = train::train(HotelEnv::default, CREW_FEATURES, &cfg);
    train::save(&report, "assets/brains/hotel.kgb");
    let net: Arc<Net> = Arc::new(report.net.clone());
    let seeds: Vec<u64> = (0..16).map(|k| 7_777_777 + k).collect();
    let rules: f32 = seeds
        .iter()
        .map(|s| play_net(hotel_for(*s), *s, EPISODE_SECS, None, Effort::BEST).fitness())
        .sum();
    eprintln!("first-found rules: fitness {:.2}", rules / seeds.len() as f32);
    for e in [0.0f32, 0.25, 0.5, 0.75, 1.0] {
        let total: f32 = seeds
            .iter()
            .map(|s| play_net(hotel_for(*s), *s, EPISODE_SECS, Some(&net), Effort::new(e)).fitness())
            .sum();
        eprintln!("full effort model e={e:.2}: fitness {:.2}", total / seeds.len() as f32);
    }
}
