use super::sim::Sim;
use crate::brain::train::Env;
use crate::brain::{Effort, Net};
use std::sync::Arc;

pub const EPISODE_SECS: f32 = 300.0;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Score {
    pub earned: u32,
    pub served: u32,
    pub angry: u32,
}

impl Score {
    pub fn fitness(self) -> f32 {
        self.earned as f32 * 0.1 + self.served as f32 - self.angry as f32 * 3.0
    }
}

pub fn staffed_sim(number: u32, seed: u64) -> Sim {
    let pads = super::layout::Layout::new(number).pads.len();
    let mut built = vec![true; pads];
    built[pads - 1] = false;
    for (i, p) in super::layout::Layout::new(number).pads.iter().enumerate() {
        if p.build == super::layout::Build::Clerk {
            built[i] = false;
        }
    }
    Sim::new(number, &built, 0, seed)
}

pub fn hotel_for(seed: u64) -> u32 {
    1 + (seed % 4) as u32
}

pub fn play(sim: &mut Sim, secs: f32, choose: super::sim::Chooser) -> Score {
    let dt = 1.0 / 30.0;
    let mut t = 0.0;
    while t < secs {
        sim.autopilot(dt);
        sim.step_with(dt, choose);
        sim.events.clear();
        t += dt;
    }
    Score {
        earned: sim.earned + sim.piles.iter().sum::<u32>(),
        served: sim.served,
        angry: sim.angry,
    }
}

pub fn play_net(
    number: u32,
    seed: u64,
    secs: f32,
    net: Option<&Arc<Net>>,
    effort: Effort,
) -> Score {
    crate::rng::srand(seed ^ 0xabcd);
    let mut sim = staffed_sim(number, seed);
    sim.effort = Some(effort);
    match net {
        Some(net) => {
            let net = net.clone();
            play(&mut sim, secs, &mut |e, options| {
                let scores: Vec<f32> = options.iter().map(|o| net.score(o)).collect();
                Some(crate::brain::pick_soft(&scores, e))
            })
        }
        None => play(&mut sim, secs, &mut |_, _| None),
    }
}

#[derive(Default)]
pub struct HotelEnv;

impl Env for HotelEnv {
    fn episode(&mut self, seed: u64, policy: &mut dyn FnMut(&[Vec<f32>]) -> usize) -> f32 {
        let mut sim = staffed_sim(hotel_for(seed), seed);
        sim.effort = Some(Effort::BEST);
        play(&mut sim, EPISODE_SECS, &mut |_, options| Some(policy(options))).fitness()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::games::hotel::sim::{Job, Sim};

    const SEEDS: std::ops::Range<u64> = 100..132;

    fn totals(net: Option<&Arc<Net>>, effort: f32) -> (Score, f32) {
        let mut sum = Score::default();
        let mut fit = 0.0;
        for s in SEEDS {
            let sc = play_net(hotel_for(s), s, EPISODE_SECS, net, Effort::new(effort));
            sum.earned += sc.earned;
            sum.served += sc.served;
            sum.angry += sc.angry;
            fit += sc.fitness();
        }
        (sum, fit / SEEDS.count() as f32)
    }

    fn shipped() -> Arc<Net> {
        Sim::new(1, &[], 0, 1).net.expect("assets/brains/hotel.kgb loads")
    }

    #[test]
    fn trained_staff_beat_the_first_found_rule_over_the_same_seeds() {
        let net = shipped();
        let (rules, rf) = totals(None, 1.0);
        let (brain, bf) = totals(Some(&net), 1.0);
        println!("rules {rules:?} {rf}\nbrain {brain:?} {bf}");
        assert!(brain.earned > rules.earned, "{brain:?} vs {rules:?}");
        assert!(brain.angry < rules.angry, "{brain:?} vs {rules:?}");
    }

    #[test]
    fn staff_fitness_rises_with_effort() {
        let net = shipped();
        let fit: Vec<f32> = [0.0, 0.5, 1.0]
            .iter()
            .map(|e| totals(Some(&net), *e).1)
            .collect();
        println!("{fit:?}");
        assert!(fit[0] < fit[1] && fit[1] < fit[2], "{fit:?}");
    }

    #[test]
    fn no_staff_member_idles_past_their_think_time_while_a_job_is_open() {
        let net = shipped();
        for effort in [0.0f32, 0.5, 1.0] {
            let effort = Effort::new(effort);
            let limit = 2.0 * effort.think_every(crate::games::hotel::sim::THINK_FASTEST) + 0.5;
            let mut sim = staffed_sim(4, 3);
            sim.effort = Some(effort);
            let mut idle_for = [0.0f32; 8];
            let dt = 1.0 / 30.0;
            let n2 = net.clone();
            for _ in 0..(EPISODE_SECS * 30.0) as usize {
                sim.step_with(dt, &mut |e, options| {
                    let scores: Vec<f32> = options.iter().map(|o| n2.score(o)).collect();
                    Some(crate::brain::pick_soft(&scores, e))
                });
                sim.events.clear();
                for (k, st) in sim.staff.iter().enumerate() {
                    if sim.job_open_for(k) && st.job == Job::Idle && st.path.is_empty() {
                        idle_for[k] += dt;
                        assert!(idle_for[k] < limit, "staff {k} idle {} s at effort {effort:?}", idle_for[k]);
                    } else {
                        idle_for[k] = 0.0;
                    }
                }
            }
        }
    }
}
