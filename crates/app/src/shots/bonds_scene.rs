use super::{Stage, DT};
use crate::games::bonds::BondsPlay;
use crate::games::Play;
use kg_core::engine::{Input, View};
use kg_core::games::bonds::{self, Question, Report};
use kg_core::profile;

fn board(scores: &[(&str, usize, i64)]) {
    let me = profile::current_id();
    for (name, avatar, score) in scores {
        let q = profile::create(name, *avatar).unwrap();
        profile::select(&q.id);
        profile::submit(bonds::GAME, *score, profile::Better::Higher);
    }
    if let Some(me) = me {
        profile::select(&me);
    }
}

fn run(p: &mut BondsPlay, view: &View, frames: usize) {
    for _ in 0..frames {
        p.update(&Input::default(), view, DT);
    }
}

fn at_level(p: &mut BondsPlay, level: usize, target: i32, a: i32) {
    p.g.start_level(level);
    p.g.question = Question::from_pair(level, target, a);
    p.g.load();
}

pub fn setup(n: u32, view: &View) -> Stage {
    kg_core::save::reset_for_test();
    let mia = profile::create("Mia", 2).unwrap();
    profile::select(&mia.id);
    if n != 290 {
        profile::set(bonds::GAME, "played", 1);
        profile::unlock(bonds::GAME, 6);
        profile::award_stars(bonds::GAME, 0, 3);
        profile::award_stars(bonds::GAME, 1, 2);
    }
    let mut p = BondsPlay::new();
    match n {
        290 => {
            at_level(&mut p, 0, 5, 3);
            run(&mut p, view, 130);
        }
        291 => {
            at_level(&mut p, 1, 10, 6);
            run(&mut p, view, 40);
        }
        292 => {
            at_level(&mut p, 1, 10, 6);
            run(&mut p, view, 30);
            let wrong = (0..bonds::OPTIONS)
                .find(|s| *s != p.g.question.slot_of_answer())
                .unwrap();
            let t = p.g.pick(wrong);
            p.g.react(view, wrong, t);
            run(&mut p, view, 20);
        }
        293 => {
            at_level(&mut p, 1, 10, 6);
            run(&mut p, view, 30);
            let slot = p.g.question.slot_of_answer();
            let t = p.g.pick(slot);
            p.g.react(view, slot, t);
            run(&mut p, view, 30);
        }
        294 => {
            at_level(&mut p, 3, 20, 14);
            run(&mut p, view, 40);
        }
        295 => {
            at_level(&mut p, 5, 100, 30);
            run(&mut p, view, 40);
        }
        296 => {
            p.g.mode = bonds::Mode::Levels;
            run(&mut p, view, 30);
        }
        297 => {
            p.g.report = Some(Report::Level { level: 1, stars: 2 });
            p.g.summary = "7 of 8 right first time   4.1 s each".into();
            p.g.mode = bonds::Mode::Report(0.0);
            run(&mut p, view, 50);
        }
        _ => {
            board(&[("Leo", 5, 21), ("Ava", 9, 18), ("Sam", 3, 12)]);
            let rank = profile::submit(bonds::GAME, 20, profile::Better::Higher);
            p.g.report = Some(Report::Cleared { rank });
            p.g.mode = bonds::Mode::Report(0.0);
            run(&mut p, view, 80);
        }
    }
    kg_core::sfx::take_cues();
    Stage::Game(Box::new(p))
}

#[cfg(test)]
pub fn check() {
    for v in [View::ipad_landscape(), View::ipad_portrait()] {
        let get = |n: u32| super::game::<BondsPlay>(setup(n, &v));
        assert!(get(290).g.first_time);
        assert_eq!(get(291).g.ladder.level, 1);
        assert_eq!(get(292).g.attempt.wrong, 1);
        assert!(get(293).g.attempt.solved);
        assert_eq!(get(294).g.ladder.level, 3);
        assert_eq!(get(295).g.ladder.level, 5);
        assert_eq!(get(296).g.mode, bonds::Mode::Levels);
        assert!(matches!(get(297).g.mode, bonds::Mode::Report(_)));
        assert!(matches!(get(298).g.report, Some(Report::Cleared { .. })));
    }
}
