use super::{Stage, DT};
use crate::games::merge::MergePlay;
use crate::games::Play;
use kg_core::engine::{Dir, Input, View};
use kg_core::games::merge::Screen;
use kg_core::profile;

fn board(scores: &[(&str, usize, i64)]) {
    let me = profile::current_id();
    for (name, avatar, score) in scores {
        let p = profile::create(name, *avatar).unwrap();
        profile::select(&p.id);
        profile::submit(kg_core::games::merge::GAME, *score, profile::Better::Higher);
    }
    if let Some(me) = me {
        profile::select(&me);
    }
}

pub fn setup(n: u32, view: &View) -> Stage {
    use kg_core::games::merge as m;
    kg_core::save::reset_for_test();
    let mia = profile::create("Mia", 2).unwrap();
    profile::select(&mia.id);
    if n != 40 {
        profile::set(m::GAME, "learned", 1);
    }
    let unlock = match n {
        41 | 49 => 4,
        42 | 46 | 48 => 8,
        44 => 5,
        40 => 1,
        _ => 3,
    };
    profile::unlock(m::GAME, unlock);
    for (level, stars) in [(1, 3), (2, 2), (3, 3), (4, 1)] {
        if level < unlock {
            profile::award_stars(m::GAME, level, stars);
        }
    }
    let level = match n {
        41 => 4,
        42 => 8,
        44 => 4,
        45 => 3,
        46 => 8,
        47 => 3,
        49 => 4,
        _ => 1,
    };
    profile::set_level(m::GAME, level);
    let mut p = MergePlay::new();
    let settle = |p: &mut MergePlay, frames: usize| {
        for _ in 0..frames {
            p.update(&Input::default(), view, DT);
        }
    };
    match n {
        40 => settle(&mut p, 40),
        41 => {
            p.g.board = [[2, 4, 8, 0], [0, 16, 4, 2], [0, 0, 32, 4], [0, 0, 2, 2]];
            settle(&mut p, 30);
        }
        42 => {
            p.g.board = [
                [2048, 512, 64, 8],
                [256, 128, 32, 4],
                [16, 8, 4, 2],
                [0, 0, 0, 2],
            ];
            p.g.score = 18420;
            p.g.t = 1.0;
            settle(&mut p, 60);
        }
        43 => {
            p.g.board = [[2, 2, 4, 0], [0, 4, 0, 4], [8, 0, 8, 2], [0, 0, 0, 0]];
            p.g.begin(4);
            p.g.board = [[2, 2, 4, 0], [0, 4, 0, 4], [8, 0, 8, 2], [0, 0, 0, 0]];
            p.g.play(Dir::Left);
            p.update(&Input::default(), view, 0.05);
        }
        44 => {
            p.g.screen = Screen::Select;
            settle(&mut p, 30);
        }
        45 | 46 => {
            if n == 46 {
                p.g.begin(8);
            }
            let top = p.g.target() / 2;
            p.g.board = [[top, top, 0, 0], [0; 4], [0; 4], [0; 4]];
            p.g.play(Dir::Left);
            settle(&mut p, 120);
            settle(&mut p, 40);
        }
        47 => {
            board(&[("Leo", 5, 480), ("Ava", 9, 220)]);
            p = MergePlay::new();
            p.g.board = [[0, 4, 8, 16], [16, 4, 8, 32], [4, 8, 16, 8], [8, 16, 4, 16]];
            p.g.score = 300;
            p.g.play(Dir::Left);
            settle(&mut p, 150);
        }
        48 => {
            p.g.begin(2);
            p.g.board = [[0, 4, 8, 0], [8, 4, 8, 0], [4, 8, 4, 0], [0; 4]];
            p.g.play(Dir::Left);
            p.update(&Input::default(), view, 0.2);
            settle(&mut p, 12);
        }
        _ => settle(&mut p, 30),
    }
    kg_core::sfx::take_cues();
    Stage::Game(Box::new(p))
}

#[cfg(test)]
pub fn check() {
    use kg_core::games::merge::Screen;
    for v in [View::ipad_landscape(), View::ipad_portrait()] {
        let get = |n: u32| super::game::<MergePlay>(setup(n, &v));
        let teach = get(40);
        assert!(!teach.g.learned && teach.g.moves == 0 && teach.g.level == 1);
        assert_eq!(get(41).g.level, 4);
        assert!(get(42).g.max_tile() >= 2048);
        assert!(get(43).g.sliding());
        assert_eq!(get(44).g.screen, Screen::Select);
        let done = get(45);
        assert!(done.g.screen == Screen::Complete && done.g.stars_earned == 3);
        let last = get(46);
        assert!(last.g.screen == Screen::Complete && last.g.level == 8);
        let lost = get(47);
        assert!(lost.g.screen == Screen::Lost && lost.g.rank.is_some_and(|r| r > 0));
        let fresh = get(48);
        assert!(fresh.g.popups.list.iter().any(|p| p.text == "Fresh start!"));
        let goal = get(49);
        assert!(goal.g.learned && goal.g.moves == 0 && goal.g.level == 4);
    }
}
