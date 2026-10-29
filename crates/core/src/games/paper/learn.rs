use super::pilot::Pilot;
use super::world::{spread_spots, World};
use crate::brain::train::Env;
use crate::brain::Net;
use crate::rng as rand;
use std::f32::consts::PI;

pub use super::world::FEATURES;

const BOTS: usize = 10;
const FRAME: f32 = 1.0 / 60.0;
const PEAK_WEIGHT: f32 = 10.0;
const KILL_WEIGHT: f32 = 0.3;
const DEATH_WEIGHT: f32 = 0.1;
const HELD_WEIGHT: f32 = 10.0;
const OUT_WEIGHT: f32 = 0.3;
const RESPAWN_AFTER: f32 = 1.5;

#[derive(Clone, Debug)]
pub struct PaperEnv {
    pub secs: f32,
    pub scripted_player: bool,
    pub taught: std::cell::RefCell<Vec<super::world::Lesson>>,
}

impl Default for PaperEnv {
    fn default() -> Self {
        Self {
            secs: 80.0,
            scripted_player: false,
            taught: Default::default(),
        }
    }
}

pub type Choose<'a> = &'a mut dyn FnMut(usize, &[Vec<f32>]) -> Option<usize>;

pub struct Setup {
    pub lessons: bool,
    pub skill: f32,
    pub brains: bool,
    pub effort: Option<f32>,
}

impl PaperEnv {
    fn with_player(&self, seed: u64) -> bool {
        self.scripted_player && seed % 2 == 1
    }

    pub fn play(&self, seed: u64, setup: &Setup, choose: Choose) -> f32 {
        let with_player = self.with_player(seed);
        let mut w = World::new(setup.skill);
        w.brains = setup.brains;
        w.effort = setup.effort;
        if setup.lessons {
            w.lessons = Some(vec![]);
        }
        let spots = spread_spots(BOTS);
        let first = usize::from(with_player);
        if with_player {
            w.add_player(spots[0], 0.0, false);
            w.players[0].speed = super::arena::PLAYER_SPEED;
        }
        for s in &spots[first..] {
            w.add_player(*s, rand::gen_range(-PI, PI), true);
        }
        let mut pilot = Pilot::new(9, true);
        let (mut kills, mut deaths, mut peaks, mut lives) = (0.0f32, 0.0f32, 0.0f32, 0.0f32);
        let mut dead_for = 0.0;
        let (mut held, mut out) = (0.0f32, 0.0f32);
        for _ in 0..(self.secs / FRAME) as usize {
            if with_player {
                if w.players[0].alive {
                    dead_for = 0.0;
                    if let Some(a) = pilot.decide_in(&w) {
                        w.players[0].want = a;
                    }
                } else {
                    dead_for += FRAME;
                    if dead_for > RESPAWN_AFTER {
                        if let Some(at) = w.free_spot(100) {
                            w.respawn(0, at);
                            w.players[0].speed = super::arena::PLAYER_SPEED;
                            pilot = Pilot::new(9, true);
                        }
                    }
                }
            }
            w.advance_with(FRAME, choose);
            w.respawn_bots(FRAME);
            for d in w.deaths.drain(..) {
                if d.victim >= first {
                    deaths += 1.0;
                    peaks += w.players[d.victim].peak;
                    lives += 1.0;
                }
                if d.killer.is_some_and(|k| k >= first) {
                    kills += 1.0;
                }
            }
            w.claims.clear();
            for id in first..w.players.len() {
                held += w.share(id);
                if w.players[id].alive && w.players[id].out() {
                    out += 1.0;
                }
            }
        }
        self.taught.replace(w.lessons.take().unwrap_or_default());
        for p in w.players.iter().skip(first).filter(|p| p.alive) {
            peaks += p.peak;
            lives += 1.0;
        }
        let n = (w.players.len() - first) as f32;
        let ticks = (self.secs / FRAME).floor() * n;
        PEAK_WEIGHT * peaks / lives.max(1.0)
            + HELD_WEIGHT * held / ticks
            + OUT_WEIGHT * out / ticks
            + (KILL_WEIGHT * kills - DEATH_WEIGHT * deaths) / n
    }

    pub fn fitness(&self, setup: &Setup, seeds: &[u64]) -> f32 {
        let mut total = 0.0;
        for &s in seeds {
            rand::srand(s ^ 0x5bd1_e995);
            total += self.play(s, setup, &mut |_, _| None);
        }
        total / seeds.len().max(1) as f32
    }
}

impl Env for PaperEnv {
    fn episode(&mut self, seed: u64, policy: &mut dyn FnMut(&[Vec<f32>]) -> usize) -> f32 {
        let setup = Setup {
            lessons: false,
            skill: 1.0,
            brains: false,
            effort: None,
        };
        self.play(seed, &setup, &mut |_, o| Some(policy(o)))
    }
}

pub fn lessons(env: &PaperEnv, seeds: &[u64]) -> Vec<super::world::Lesson> {
    let mut all = vec![];
    for &s in seeds {
        rand::srand(s);
        let setup = Setup {
            lessons: true,
            skill: 1.0,
            brains: false,
            effort: None,
        };
        env.play(s, &setup, &mut |_, _| None);
        all.append(&mut env.taught.borrow_mut());
    }
    all
}

pub fn imitate(net: &mut Net, lessons: &[super::world::Lesson], epochs: usize, rate: f32) {
    let (n_in, n_h) = (net.sizes[0], net.sizes[1]);
    let w1 = n_in * n_h;
    let b1 = w1;
    let w2 = w1 + n_h;
    let b2 = w2 + n_h;
    for _ in 0..epochs {
        for (opts, want) in lessons {
            let hidden: Vec<Vec<f32>> = opts
                .iter()
                .map(|f| {
                    (0..n_h)
                        .map(|h| {
                            let row = &net.weights[h * n_in..(h + 1) * n_in];
                            (row.iter().zip(f).map(|(a, b)| a * b).sum::<f32>()
                                + net.weights[b1 + h])
                                .tanh()
                        })
                        .collect()
                })
                .collect();
            let scores: Vec<f32> = hidden
                .iter()
                .map(|h| {
                    h.iter()
                        .enumerate()
                        .map(|(k, v)| v * net.weights[w2 + k])
                        .sum::<f32>()
                        + net.weights[b2]
                })
                .collect();
            let top = scores.iter().copied().fold(f32::MIN, f32::max);
            let exps: Vec<f32> = scores.iter().map(|s| (s - top).exp()).collect();
            let total: f32 = exps.iter().sum();
            for (o, f) in opts.iter().enumerate() {
                let g = exps[o] / total - if o == *want { 1.0 } else { 0.0 };
                for (h, &a) in hidden[o].iter().enumerate() {
                    let back = g * net.weights[w2 + h] * (1.0 - a * a);
                    net.weights[w2 + h] -= rate * g * a;
                    net.weights[b1 + h] -= rate * back;
                    for (i, x) in f.iter().enumerate() {
                        net.weights[h * n_in + i] -= rate * back * x;
                    }
                }
                net.weights[b2] -= rate * g;
            }
        }
    }
}
