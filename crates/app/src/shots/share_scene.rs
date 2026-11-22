use super::{Stage, DT};
use crate::games::share::SharePlay;
use crate::games::Play;
use kg_core::engine::{Input, View};
use kg_core::games::share::{self, *};
use kg_core::profile;

fn run(p: &mut SharePlay, view: &View, frames: usize) {
    for _ in 0..frames {
        p.update(&Input::default(), view, DT);
    }
}

fn at_level(p: &mut SharePlay, level: usize, total: i32, friends: i32) {
    p.g.start_level(level);
    p.g.question = Question::from_pair(level, total, friends);
    p.g.load();
}

pub fn setup(n: u32, view: &View) -> Stage {
    kg_core::save::reset_for_test();
    let mia = profile::create("Mia", 2).unwrap();
    profile::select(&mia.id);
    if n != 280 {
        profile::set(GAME, "played", 1);
        profile::unlock(GAME, 5);
        profile::award_stars(GAME, 0, 3);
        profile::award_stars(GAME, 1, 2);
    }
    let mut p = SharePlay::new();
    match n {
        280 => run(&mut p, view, 40),
        281 => {
            at_level(&mut p, 1, 12, 3);
            run(&mut p, view, 20);
            p.g.place(view, 0);
            p.g.place(view, 0);
            p.g.place(view, 1);
            run(&mut p, view, 20);
        }
        282 => {
            at_level(&mut p, 2, 15, 5);
            for i in 0..15 {
                p.g.place(view, i % 5);
            }
            run(&mut p, view, 30);
        }
        283 => {
            at_level(&mut p, 3, 40, 4);
            run(&mut p, view, 30);
        }
        284 => {
            at_level(&mut p, 4, 12, 3);
            run(&mut p, view, 30);
        }
        285 => {
            at_level(&mut p, 5, 13, 4);
            run(&mut p, view, 30);
            let right = p.g.question.slot_of_answer();
            p.g.pick(view, right);
            run(&mut p, view, 25);
        }
        286 => {
            p.g.mode = Mode::Levels;
            run(&mut p, view, 60);
        }
        287 => {
            p.g.report = Some(Report::Cleared { rank: Some(0) });
            p.g.mode = Mode::Report(0.0);
            run(&mut p, view, 110);
        }
        _ => {
            at_level(&mut p, 0, 8, 2);
            p.g.place(view, 0);
            p.g.place(view, 0);
            p.g.place(view, 0);
            p.g.place(view, 0);
            p.g.place(view, 0);
            run(&mut p, view, 30);
        }
    }
    kg_core::sfx::take_cues();
    Stage::Game(Box::new(p))
}

#[cfg(test)]
pub fn check() {
    for v in [View::ipad_landscape(), View::ipad_portrait()] {
        let get = |n: u32| super::game::<SharePlay>(setup(n, &v));
        assert!(get(280).g.first_time);
        assert_eq!(get(281).g.placed, vec![2, 1, 0]);
        assert_eq!(get(282).g.phase, share::Phase::Choose);
        assert_eq!(get(282).g.remaining(), 0);
        assert_eq!(get(283).g.phase, share::Phase::Choose);
        assert!(get(284).g.question.asks_friends());
        assert!(get(285).g.attempt.solved);
        assert_eq!(get(286).g.mode, Mode::Levels);
        assert!(get(287).over());
        assert_eq!(get(288).g.placed, vec![4, 0]);
    }
}
