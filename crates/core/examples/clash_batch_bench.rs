use kg_core::brain::learn::{Batch, Learner, Tick};
use kg_core::games::clash::rl::Arena;
use std::time::Instant;

fn main() {
    let threads: usize = std::env::args()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(5);
    if let Some(path) = std::env::args().nth(2).filter(|p| !p.is_empty()) {
        let p = kg_core::brain::conv::Policy::from_bytes(&std::fs::read(path).unwrap()).unwrap();
        kg_core::games::clash::rl::set_rivals(vec![p], 0.5);
    }
    if std::env::args().nth(3).is_some() {
        kg_core::games::clash::rl::set_seats(1, 0.25);
    }
    let envs: Vec<Arena> = (0..128)
        .map(|i| Arena::new(1000 + i * 7919, 600.0))
        .collect();
    let mut b = Batch::new(envs, threads);
    let n = b.agents();
    let spec = Arena::spec();
    let mut grid = vec![0.0; n * spec.channels * spec.side * spec.side];
    let mut sc = vec![0.0; n * spec.scalars];
    let mut ticks: Vec<Tick> = (0..n)
        .map(|_| Tick {
            reward: 0.0,
            done: false,
        })
        .collect();
    let acts: Vec<usize> = (0..n).map(|k| k % 5).collect();
    let t = Instant::now();
    let mut steps = 0;
    while t.elapsed().as_secs_f32() < 5.0 {
        b.step(&acts, &mut ticks);
        b.write_obs(&mut grid, &mut sc);
        steps += n;
    }
    let seats: usize = b.envs.iter().map(|e| e.seats().len()).sum();
    println!("seats {seats}");
    println!(
        "{threads} threads: {:.0} agent-steps/s",
        steps as f64 / t.elapsed().as_secs_f64()
    );
}
