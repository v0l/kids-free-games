use super::{Stage, DT};
use crate::games::hotel::HotelPlay;
use crate::games::Play;
use glam::vec2;
use kg_core::engine::{Input, View};
use kg_core::games::hotel::layout::{room_center, Build, Layout};
use kg_core::games::hotel::sim::Sim;
use kg_core::games::hotel::{Hotel, Mode};
use kg_core::profile;

fn sim_for(n: u32, number: u32, built: Vec<bool>, secs: u32) -> Sim {
    let mut sim = Sim::new(number, &built, 0, 42 + n as u64);
    for _ in 0..secs * 30 {
        sim.autopilot(1.0 / 30.0);
        sim.step(1.0 / 30.0);
        if sim.complete() {
            break;
        }
    }
    sim.events.clear();
    sim
}

fn all_but_last(number: u32) -> Vec<bool> {
    let n = Layout::new(number).pads.len();
    let mut b = vec![true; n];
    b[n - 1] = false;
    b
}

fn settle(p: &mut HotelPlay, view: &View, frames: usize) {
    for _ in 0..frames {
        p.update(&Input::default(), view, DT);
    }
}

pub fn setup(n: u32, view: &View) -> Stage {
    kg_core::save::reset_for_test();
    let mia = profile::create("Mia", 2).unwrap();
    profile::select(&mia.id);
    let mut p = match n {
        120 => {
            let mut h = Hotel::with(Sim::new(1, &[], 0, 1));
            h.learned_move = false;
            HotelPlay::with(h)
        }
        121 => HotelPlay::with(Hotel::with(sim_for(n, 1, vec![], 14))),
        122 => HotelPlay::with(Hotel::with(sim_for(n, 1, vec![], 240))),
        123 => {
            let lay = Layout::new(3);
            let upto = lay.pads.iter().position(|p| p.build == Build::Pool).unwrap();
            let built: Vec<bool> = (0..lay.pads.len()).map(|i| i <= upto).collect();
            let mut sim = sim_for(n, 3, built, 120);
            sim.player.pos = vec2(0.0, 2.0);
            HotelPlay::with(Hotel::with(sim))
        }
        124 => HotelPlay::with(Hotel::with(sim_for(n, 4, vec![], 700))),
        125..=128 => {
            let number = n - 124;
            let mut h = Hotel::with(sim_for(n, number, all_but_last(number), 70));
            h.overview();
            HotelPlay::with(h)
        }
        129 => {
            let mut sim = sim_for(n, 3, all_but_last(3), 160);
            sim.add_swimmers(3);
            let pool = sim.lay.pool;
            let mut h = Hotel::with(sim);
            h.look_at(vec2(pool.x + pool.w / 2.0, pool.y + pool.h / 2.0 + 1.5), 1.6);
            HotelPlay::with(h)
        }
        130 => {
            let lay = Layout::new(4);
            let built: Vec<bool> = lay
                .pads
                .iter()
                .map(|p| !matches!(p.build, Build::Decor(0, _) | Build::Next))
                .collect();
            let mut sim = Sim::new(4, &built, 999, 1);
            sim.level = sim.lay.max_level;
            sim.player.pos = vec2(-0.4, -1.0);
            let mut h = Hotel::with(sim);
            h.look_at(room_center(0) + vec2(1.5, 0.5), 1.4);
            HotelPlay::with(h)
        }
        131 => {
            let mut h = Hotel::with(Sim::new(1, &[], 0, 1));
            let need = h.sim.xp_needed();
            h.sim.gain_xp(need, glam::Vec2::ZERO);
            h.react();
            let mut p = HotelPlay::with(h);
            settle(&mut p, view, 20);
            return finish(p);
        }
        132 => {
            let n_pads = Layout::new(1).pads.len();
            let mut sim = Sim::new(1, &all_but_last(1), 900, 1);
            sim.level = sim.lay.max_level;
            sim.player.pos = sim.lay.pads[n_pads - 1].pos;
            let mut p = HotelPlay::with(Hotel::with(sim));
            for _ in 0..200 {
                p.update(&Input::default(), view, DT);
                if matches!(p.h.mode, Mode::Moving(t) if t > 1.0) {
                    break;
                }
            }
            return finish(p);
        }
        _ => {
            let n_pads = Layout::new(4).pads.len();
            let mut sim = Sim::new(4, &all_but_last(4), 2000, 1);
            sim.level = sim.lay.max_level;
            sim.player.pos = sim.lay.pads[n_pads - 1].pos;
            let mut p = HotelPlay::with(Hotel::with(sim));
            for _ in 0..200 {
                p.update(&Input::default(), view, DT);
                if matches!(p.h.mode, Mode::Done(t) if t > 1.0) {
                    break;
                }
            }
            return finish(p);
        }
    };
    if n != 120 {
        p.h.learned_move = true;
    }
    settle(&mut p, view, 60);
    finish(p)
}

fn finish(p: HotelPlay) -> Stage {
    kg_core::sfx::take_cues();
    Stage::Game(Box::new(p))
}

#[cfg(test)]
pub fn check() {
    for v in [View::ipad_landscape(), View::ipad_portrait()] {
        let get = |n: u32| super::game::<HotelPlay>(setup(n, &v));
        let first = get(120);
        assert!(!first.h.learned_move && first.h.sim.number() == 1);
        assert!(!get(121).h.sim.guests.is_empty());
        assert!(get(122).h.sim.built.iter().any(|b| *b));
        assert!(get(123).h.sim.has(Build::Pool));
        assert_eq!(get(124).h.sim.number(), 4);
        for n in 125..=128 {
            let p = get(n);
            assert_eq!(p.h.sim.number(), n - 124);
            assert!(p.h.zoom < 1.0);
        }
        let swim = get(129);
        assert!(swim.h.sim.guests.iter().any(|g| matches!(g.stage, kg_core::games::hotel::sim::Stage::Swim(..))));
        let decor = get(130);
        assert!(decor.h.sim.visible_pads().iter().any(|&i| matches!(decor.h.sim.lay.pads[i].build, Build::Decor(0, _))));
        assert!(matches!(get(131).h.mode, Mode::LevelUp(2, _)));
        let moving = get(132);
        assert!(matches!(moving.h.mode, Mode::Moving(_)) && moving.over());
        let done = get(133);
        assert!(matches!(done.h.mode, Mode::Done(_)) && done.over());
    }
}

