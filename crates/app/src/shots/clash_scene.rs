use super::{Stage, DT};
use crate::games::clash::ClashPlay;
use crate::games::Play;
use kg_core::engine::{Input, View};
use kg_core::games::clash::sim::xp_for_level;
use kg_core::games::clash::{self, Mode, Screen};
use kg_core::profile;

fn board(scores: &[(&str, usize, i64)]) {
    let me = profile::current_id();
    for (name, avatar, score) in scores {
        let q = profile::create(name, *avatar).unwrap();
        profile::select(&q.id);
        profile::submit(clash::GAME, *score, profile::Better::Higher);
    }
    if let Some(me) = me {
        profile::select(&me);
    }
}

fn play(p: &mut ClashPlay, view: &View, frames: usize) {
    for _ in 0..frames {
        if p.g.screen == Screen::Playing && p.g.ending == 0.0 {
            p.g.steer = p.g.world.autopilot();
        }
        p.update(&Input::default(), view, DT);
    }
}

fn veteran() {
    profile::set(clash::GAME, "games", 6);
    profile::set(clash::GAME, "best_level", 9);
    profile::set(clash::GAME, "eaten", 12);
    profile::set(clash::GAME, "floor", 4);
}

fn grow(p: &mut ClashPlay, level: u32) {
    p.g.world.snakes[0].xp = xp_for_level(level);
}

pub fn setup(n: u32, view: &View) -> Stage {
    kg_core::save::reset_for_test();
    kg_core::rng::srand(n as u64 + 11);
    let mia = profile::create("Mia", 2).unwrap();
    profile::select(&mia.id);
    if n != 193 {
        veteran();
    }
    let mut p = ClashPlay::new();
    match n {
        190 => play(&mut p, view, 90),
        191 => {
            p.g.cycle_skin(-1);
            p.g.cycle_skin(-1);
            play(&mut p, view, 40);
        }
        192 => {
            p.g.screen = Screen::Tower;
            p.g.pick_floor = 5;
            play(&mut p, view, 40);
        }
        193 => {
            p.g.start(Mode::Arena, 1);
            play(&mut p, view, 60 * 3);
        }
        194 => {
            p.g.start(Mode::Arena, 1);
            grow(&mut p, 9);
            play(&mut p, view, 60 * 14);
        }
        195 => {
            p.g.start(Mode::Arena, 1);
            grow(&mut p, 5);
            play(&mut p, view, 120);
            grow(&mut p, 6);
            play(&mut p, view, 12);
        }
        196 => {
            p.g.start(Mode::Tower, 5);
            grow(&mut p, 7);
            play(&mut p, view, 60 * 6);
        }
        197 => {
            board(&[("Leo", 5, 14), ("Ava", 9, 11), ("Sam", 3, 6)]);
            p = ClashPlay::new();
            p.g.start(Mode::Arena, 1);
            grow(&mut p, 8);
            play(&mut p, view, 60);
            let head = p.g.world.player().head();
            p.g.world.snakes[0].shield = 0.0;
            let i = p.g.world.add_snake(head, 30, "Titan".into(), 9);
            p.g.world.snakes[i].xp = xp_for_level(30);
            play(&mut p, view, 60 * 4);
        }
        198 => {
            p.g.start(Mode::Tower, 1);
            grow(&mut p, 12);
            play(&mut p, view, 60 * 6);
        }
        _ => {
            p.g.start(Mode::Tower, 12);
            grow(&mut p, 13);
            play(&mut p, view, 60 * 12);
        }
    }
    kg_core::sfx::take_cues();
    Stage::Game(Box::new(p))
}

#[cfg(test)]
pub fn check() {
    for v in [View::ipad_landscape(), View::ipad_portrait()] {
        let get = |n: u32| super::game::<ClashPlay>(setup(n, &v));
        assert_eq!(get(190).g.screen, Screen::Title);
        let locked = get(191);
        assert!(!locked.g.skin_open(locked.g.skin));
        assert_eq!(get(192).g.screen, Screen::Tower);
        let first = get(193);
        assert!(first.g.screen == Screen::Playing && first.g.training);
        assert!(get(194).g.world.player().level() >= 9);
        assert!(get(195).g.world.player().level() >= 6);
        assert!(get(196).g.world.boss().is_some());
        let over = get(197);
        assert!(over.g.screen == Screen::Over && over.g.shows_results());
        let clear = get(198);
        assert!(clear.g.outcome.cleared && clear.g.screen == Screen::Over);
        assert!(get(199).g.mode == Mode::Tower);
    }
}
