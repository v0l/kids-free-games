use super::{Stage, DT};
use crate::games::trains::TrainsPlay;
use crate::games::Play;
use kg_core::engine::{Input, View};
use kg_core::games::trains::{self, Phase, Question, Report};
use kg_core::profile;

fn board(scores: &[(&str, usize, i64)]) {
    let me = profile::current_id();
    for (name, avatar, score) in scores {
        let q = profile::create(name, *avatar).unwrap();
        profile::select(&q.id);
        profile::submit(trains::GAME, *score, profile::Better::Higher);
    }
    if let Some(me) = me {
        profile::select(&me);
    }
}

fn run(p: &mut TrainsPlay, view: &View, frames: usize) {
    for _ in 0..frames {
        p.update(&Input::default(), view, DT);
    }
}

fn settle(p: &mut TrainsPlay, view: &View) {
    let mut guard = 0;
    while p.g.phase == Phase::Arrive && guard < 600 {
        p.update(&Input::default(), view, DT);
        guard += 1;
    }
    run(p, view, 20);
}

fn at_level(p: &mut TrainsPlay, level: usize, a: i32, b: i32) {
    p.g.start_level(level);
    p.g.question = Question::from_pair(level, a, b);
    p.g.riders = vec![trains::Rider::Aboard; p.g.question.riders()];
    p.g.looks = (0..p.g.riders.len() as u32)
        .map(|i| trains::rider_look(31, i as usize))
        .collect();
    p.g.spots = vec![0; p.g.riders.len()];
    p.g.phase = Phase::Arrive;
    p.g.phase_t = 0.0;
}

fn remove(p: &mut TrainsPlay, n: usize) {
    for i in (0..p.g.riders.len()).rev().take(n) {
        p.g.tap_rider(i);
    }
}

pub fn setup(n: u32, view: &View) -> Stage {
    kg_core::save::reset_for_test();
    let mia = profile::create("Mia", 2).unwrap();
    profile::select(&mia.id);
    if n != 220 {
        profile::set(trains::GAME, "played", 1);
        profile::unlock(trains::GAME, 7);
        profile::award_stars(trains::GAME, 0, 3);
        profile::award_stars(trains::GAME, 1, 2);
        profile::award_stars(trains::GAME, 2, 3);
    }
    let mut p = TrainsPlay::new();
    match n {
        220 => {
            at_level(&mut p, 0, 5, 2);
            settle(&mut p, view);
            run(&mut p, view, 100);
        }
        221 => {
            at_level(&mut p, 1, 8, 3);
            settle(&mut p, view);
            remove(&mut p, 2);
            run(&mut p, view, 12);
        }
        222 => {
            at_level(&mut p, 1, 9, 4);
            settle(&mut p, view);
            remove(&mut p, 4);
            run(&mut p, view, 50);
        }
        223 => {
            at_level(&mut p, 2, 8, 3);
            settle(&mut p, view);
            run(&mut p, view, 80);
        }
        224 => {
            at_level(&mut p, 3, 17, 9);
            settle(&mut p, view);
            run(&mut p, view, 30);
        }
        225 => {
            at_level(&mut p, 4, 12, 5);
            settle(&mut p, view);
            run(&mut p, view, 30);
        }
        226 => {
            at_level(&mut p, 7, 63, 28);
            settle(&mut p, view);
            let wrong = (0..trains::OPTIONS)
                .find(|s| *s != p.g.question.slot_of_answer())
                .unwrap();
            let t = p.g.pick(wrong);
            p.g.react(view, wrong, t);
            run(&mut p, view, 20);
        }
        227 => {
            at_level(&mut p, 1, 7, 3);
            settle(&mut p, view);
            remove(&mut p, 3);
            run(&mut p, view, 40);
            let slot = p.g.question.slot_of_answer();
            let t = p.g.pick(slot);
            p.g.react(view, slot, t);
            run(&mut p, view, 40);
        }
        228 => {
            p.g.mode = trains::Mode::Levels;
            run(&mut p, view, 30);
        }
        _ => {
            board(&[("Leo", 5, 21), ("Ava", 9, 18), ("Sam", 3, 12)]);
            let rank = profile::submit(trains::GAME, 20, profile::Better::Higher);
            p.g.report = Some(Report::Cleared { rank });
            p.g.mode = trains::Mode::Report(0.0);
            run(&mut p, view, 80);
        }
    }
    kg_core::sfx::take_cues();
    Stage::Game(Box::new(p))
}

#[cfg(test)]
pub fn check() {
    for v in [View::ipad_landscape(), View::ipad_portrait()] {
        let get = |n: u32| super::game::<TrainsPlay>(setup(n, &v));
        assert!(get(220).g.first_time);
        assert_eq!(get(220).g.phase, Phase::Remove);
        assert_eq!(get(221).g.removed(), 2);
        assert_eq!(get(222).g.phase, Phase::Choose);
        assert!(get(223).g.hide_k() > 0.9);
        assert_eq!(get(224).g.ladder.level, 3);
        assert_eq!(get(225).g.ladder.level, 4);
        assert_eq!(get(226).g.attempt.wrong, 1);
        assert!(get(227).g.attempt.solved);
        assert_eq!(get(228).g.mode, trains::Mode::Levels);
        assert!(matches!(get(229).g.report, Some(Report::Cleared { .. })));
    }
}
