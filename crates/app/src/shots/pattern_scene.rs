use super::{Stage, DT};
use crate::games::pattern::PatternPlay;
use crate::games::Play;
use kg_core::engine::{Input, View};
use kg_core::games::pattern::{self, Report};
use kg_core::profile;

fn board(scores: &[(&str, usize, i64)]) {
    let me = profile::current_id();
    for (name, avatar, score) in scores {
        let q = profile::create(name, *avatar).unwrap();
        profile::select(&q.id);
        profile::submit(pattern::GAME, *score, profile::Better::Higher);
    }
    if let Some(me) = me {
        profile::select(&me);
    }
}

fn run(p: &mut PatternPlay, view: &View, frames: usize) {
    for _ in 0..frames {
        p.update(&Input::default(), view, DT);
    }
}

fn press_slot(p: &mut PatternPlay, view: &View, slot: usize) {
    let rects = pattern::button_rects(view, p.g.question.options.len());
    p.update(&Input::press_at(rects[slot].center()), view, DT);
}

fn right(p: &mut PatternPlay, view: &View) {
    let slot = p.g.question.slot_of_answer();
    press_slot(p, view, slot);
}

fn wrong(p: &mut PatternPlay, view: &View) {
    let right = p.g.question.slot_of_answer();
    let slot = (0..p.g.question.options.len())
        .find(|i| *i != right)
        .unwrap();
    press_slot(p, view, slot);
}

pub fn setup(n: u32, view: &View) -> Stage {
    kg_core::save::reset_for_test();
    let mia = profile::create("Mia", 2).unwrap();
    profile::select(&mia.id);
    let veteran = n != 270;
    if veteran {
        profile::set(pattern::GAME, "played", 1);
        profile::unlock(pattern::GAME, 4);
        profile::award_stars(pattern::GAME, 0, 3);
        profile::award_stars(pattern::GAME, 1, 2);
        profile::award_stars(pattern::GAME, 2, 3);
        profile::award_stars(pattern::GAME, 3, 2);
    }
    match n {
        272 => profile::set_level(pattern::GAME, 3),
        273 => profile::set_level(pattern::GAME, 4),
        274 => {
            profile::unlock(pattern::GAME, 7);
            profile::set_level(pattern::GAME, 7);
        }
        _ => {}
    }
    let mut p = PatternPlay::new();
    match n {
        270 | 272 | 273 | 274 => run(&mut p, view, 60),
        271 => {
            run(&mut p, view, 60);
            right(&mut p, view);
            run(&mut p, view, 24);
        }
        275 => {
            run(&mut p, view, 60);
            wrong(&mut p, view);
            run(&mut p, view, 10);
        }
        276 => {
            p.g.mode = pattern::Mode::Levels;
            run(&mut p, view, 30);
        }
        277 => {
            p.g.report = Some(Report::Level { level: 2, stars: 2 });
            p.g.summary = "4 of 4 right first time   6.1 s each".into();
            p.g.mode = pattern::Mode::Report(0.0);
            run(&mut p, view, 50);
        }
        _ => {
            board(&[("Leo", 5, 41), ("Ava", 9, 33), ("Sam", 3, 20)]);
            let rank = profile::submit(pattern::GAME, 36, profile::Better::Higher);
            p.g.report = Some(Report::Cleared { rank });
            p.g.mode = pattern::Mode::Report(0.0);
            run(&mut p, view, 80);
        }
    }
    kg_core::sfx::take_cues();
    Stage::Game(Box::new(p))
}

#[cfg(test)]
pub fn check() {
    for v in [View::ipad_landscape(), View::ipad_portrait()] {
        let get = |n: u32| super::game::<PatternPlay>(setup(n, &v));
        assert!(get(270).g.first_time);
        assert!(get(271).g.celebrate.is_some());
        assert_eq!(get(272).g.ladder.level, 3);
        assert_eq!(get(273).g.ladder.level, 4);
        assert_eq!(get(274).g.ladder.level, 7);
        assert_eq!(get(275).g.attempt.wrong, 1);
        assert_eq!(get(276).g.mode, pattern::Mode::Levels);
        assert!(matches!(get(277).g.mode, pattern::Mode::Report(_)));
        assert!(matches!(get(278).g.report, Some(Report::Cleared { .. })));
    }
}
