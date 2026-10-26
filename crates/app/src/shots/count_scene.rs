use super::{Stage, DT};
use crate::games::count::CountPlay;
use crate::games::Play;
use kg_core::engine::{Input, View};
use kg_core::games::count::{self, Report};
use kg_core::profile;

fn board(scores: &[(&str, usize, i64)]) {
    let me = profile::current_id();
    for (name, avatar, score) in scores {
        let q = profile::create(name, *avatar).unwrap();
        profile::select(&q.id);
        profile::submit(count::GAME, *score, profile::Better::Higher);
    }
    if let Some(me) = me {
        profile::select(&me);
    }
}

fn run(p: &mut CountPlay, view: &View, frames: usize) {
    for _ in 0..frames {
        p.update(&Input::default(), view, DT);
    }
}

fn wrong(p: &mut CountPlay, view: &View, times: usize) {
    let right = p.g.question.slot_of_answer();
    let rects = count::button_rects(view, p.g.question.options.len());
    for slot in (0..rects.len()).filter(|i| *i != right).take(times) {
        p.update(&Input::press_at(rects[slot].center()), view, DT);
    }
}

fn right(p: &mut CountPlay, view: &View) {
    let rects = count::button_rects(view, p.g.question.options.len());
    let slot = p.g.question.slot_of_answer();
    p.update(&Input::press_at(rects[slot].center()), view, DT);
}

pub fn setup(n: u32, view: &View) -> Stage {
    kg_core::save::reset_for_test();
    let mia = profile::create("Mia", 2).unwrap();
    profile::select(&mia.id);
    let veteran = !matches!(n, 140 | 149);
    if veteran {
        profile::set(count::GAME, "played", 1);
        profile::unlock(count::GAME, 3);
        profile::award_stars(count::GAME, 0, 3);
        profile::award_stars(count::GAME, 1, 2);
        profile::award_stars(count::GAME, 2, 3);
    }
    if n == 144 {
        profile::set_level(count::GAME, 5);
        profile::unlock(count::GAME, 5);
    }
    if n == 148 {
        profile::set_level(count::GAME, 3);
    }
    let mut p = CountPlay::new();
    match n {
        140 => run(&mut p, view, 50),
        141 => {
            run(&mut p, view, 40);
            p.g.question = count::Question::with_count(p.g.ladder.level, 7);
            p.g.load();
            run(&mut p, view, 40);
            let (points, _) = count::scatter(7, p.g.question.seed, count::play_area(view));
            for i in [3, 0, 5] {
                p.update(&Input::press_at(points[i]), view, DT);
                run(&mut p, view, 3);
            }
            run(&mut p, view, 6);
        }
        142 => {
            run(&mut p, view, 40);
            right(&mut p, view);
            run(&mut p, view, 25);
        }
        143 => {
            run(&mut p, view, 40);
            wrong(&mut p, view, 2);
            run(&mut p, view, 20);
        }
        144 => run(&mut p, view, 50),
        145 => {
            p.g.mode = count::Mode::Levels;
            run(&mut p, view, 30);
        }
        146 => {
            p.g.report = Some(Report::Level { level: 2, stars: 2 });
            p.g.summary = "7 of 8 first try, about 6 seconds each".into();
            p.g.mode = count::Mode::Report(0.0);
            run(&mut p, view, 50);
        }
        147 => {
            board(&[("Leo", 5, 41), ("Ava", 9, 33), ("Sam", 3, 20)]);
            let rank = profile::submit(count::GAME, 36, profile::Better::Higher);
            p.g.report = Some(Report::Cleared { rank });
            p.g.mode = count::Mode::Report(0.0);
            run(&mut p, view, 80);
        }
        148 => {
            run(&mut p, view, 40);
            p.g.ladder.streak = 3;
            right(&mut p, view);
            run(&mut p, view, 12);
        }
        _ => {
            run(&mut p, view, 40);
            wrong(&mut p, view, 1);
            run(&mut p, view, 10);
        }
    }
    kg_core::sfx::take_cues();
    Stage::Game(Box::new(p))
}

#[cfg(test)]
pub fn check() {
    for v in [View::ipad_landscape(), View::ipad_portrait()] {
        let get = |n: u32| super::game::<CountPlay>(setup(n, &v));
        assert!(get(140).g.first_time);
        assert_eq!(get(141).g.ticked(), 3);
        assert!(get(142).g.celebrate.is_some());
        assert!(get(143).g.attempt.revealed());
        assert_eq!(get(144).g.ladder.level, 5);
        assert_eq!(get(145).g.mode, count::Mode::Levels);
        assert!(matches!(get(146).g.mode, count::Mode::Report(_)));
        assert!(matches!(get(147).g.report, Some(Report::Cleared { .. })));
        assert_eq!(get(148).g.ladder.level, 4);
        assert_eq!(get(149).g.attempt.wrong, 1);
    }
}
