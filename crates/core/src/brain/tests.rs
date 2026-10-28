use super::train::{self, Config, Env};
use super::*;

struct Nearest;

impl Env for Nearest {
    fn episode(&mut self, _seed: u64, policy: &mut dyn FnMut(&[Vec<f32>]) -> usize) -> f32 {
        let mut score = 0.0;
        for _ in 0..30 {
            let target = rand::gen_range(-1.0f32, 1.0);
            let opts: Vec<Vec<f32>> = (0..6)
                .map(|_| {
                    let x = rand::gen_range(-1.0f32, 1.0);
                    vec![(x - target).abs(), rand::gen_range(-1.0, 1.0)]
                })
                .collect();
            let k = policy(&opts);
            let best = opts.iter().map(|o| o[0]).fold(f32::MAX, f32::min);
            if (opts[k][0] - best).abs() < 1e-6 {
                score += 1.0;
            }
        }
        score / 30.0
    }
}

fn quick() -> Config {
    Config {
        hidden: vec![6],
        generations: 60,
        population: 32,
        seeds: 2,
        rate: 0.05,
        threads: 4,
        ..Config::default()
    }
}

#[test]
fn a_net_round_trips_through_bytes() {
    crate::rng::srand(1);
    let net = Net::random(&[5, 7, 1], 1.0);
    let back = Net::from_bytes(&net.to_bytes()).unwrap();
    assert_eq!(net, back);
    assert!(Net::from_bytes(b"KGB1junk").is_none());
    assert!(Net::from_bytes(&net.to_bytes()[..20]).is_none());
}

#[test]
fn eval_matches_a_hand_built_net() {
    let mut net = Net::new(&[2, 1, 1]);
    net.weights = vec![1.0, -1.0, 0.0, 2.0, 0.5];
    let x = (0.3f32 - 0.1).tanh() * 2.0 + 0.5;
    assert!((net.score(&[0.3, 0.1]) - x).abs() < 1e-6);
}

#[test]
fn training_learns_a_task_that_random_play_fails() {
    let report = train::train(|| Nearest, 2, &quick());
    let seeds: Vec<u64> = (100..140).collect();
    let learned = train::evaluate(&mut Nearest, &report.net, Effort::BEST, &seeds);
    let blank = Net::new(&[2, 6, 1]);
    let random = train::evaluate(&mut Nearest, &blank, Effort::new(0.0), &seeds);
    assert!(learned > 0.85, "learned {learned}");
    assert!(random < 0.35, "random {random}");
}

#[test]
fn more_effort_plays_better() {
    let report = train::train(|| Nearest, 2, &quick());
    let by: Vec<f32> = report.by_effort.iter().map(|(_, f)| *f).collect();
    for w in by.windows(2) {
        assert!(w[1] >= w[0] - 0.02, "{by:?}");
    }
    assert!(by[4] > by[0] + 0.3, "{by:?}");
}

#[test]
fn low_effort_thinks_less_often_and_sticks_with_its_last_choice() {
    let lazy = Effort::new(0.0);
    let keen = Effort::BEST;
    assert!(lazy.think_every(0.1) > keen.think_every(0.1) * 3.0);
    assert_eq!(keen.think_every(0.1), 0.1);
    assert_eq!(keen.stubborn(), 0.0);
    assert!(lazy.temperature() > 1.0);
    assert_eq!(keen.temperature(), 0.0);
}

#[test]
fn full_effort_always_takes_the_top_score() {
    crate::rng::srand(3);
    let scores = [0.1, 0.9, 0.3, 0.85];
    for _ in 0..200 {
        assert_eq!(pick_soft(&scores, Effort::BEST), 1);
    }
}
