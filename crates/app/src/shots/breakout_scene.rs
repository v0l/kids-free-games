use super::{Stage, DT};
use crate::games::breakout::BreakoutPlay;
use crate::games::Play;
use glam::vec2;
use kg_core::engine::{Input, View};
use kg_core::games::breakout;
use kg_core::profile;

fn rally(p: &mut BreakoutPlay, view: &View, frames: usize) {
    for _ in 0..frames {
        let input = Input {
            pos: vec2(p.g.ball.x + 15.0 * (p.g.clock * 1.7).sin(), view.h - 40.0),
            down: true,
            ..Default::default()
        };
        p.update(&input, view, DT);
        if p.g.state != breakout::State::Playing {
            break;
        }
    }
}

fn launch_ball(p: &mut BreakoutPlay, view: &View) {
    p.update(&Input::default(), view, DT);
    p.update(&Input::press_at(vec2(10.0, 10.0)), view, 0.5);
    p.update(
        &Input {
            released: true,
            ..Default::default()
        },
        view,
        0.1,
    );
}

pub fn setup(n: u32, view: &View) -> Stage {
    kg_core::save::reset_for_test();
    let mia = profile::create("Mia", 2).unwrap();
    profile::select(&mia.id);
    let key = breakout::KEY;
    let level = match n {
        72 => 7,
        76..=78 => breakout::LEVELS as u32,
        75 => 5,
        _ => 1,
    };
    if n >= 72 && n != 75 || n == 79 {
        profile::unlock(key, breakout::LEVELS as u32);
        for l in 1..level.max(4) {
            profile::award_stars(key, l, 1 + (l % 3) as u8);
        }
        profile::set(key, "score.1", 480);
        profile::set(key, "score.2", 910);
    }
    if n == 73 {
        profile::set_level(key, 4);
    }
    let mut p = BreakoutPlay::new();
    if n != 73 {
        p.g.start_level(level, *view);
    }
    p.update(&Input::default(), view, DT);
    match n {
        70 | 73 | 76 | 79 => {
            for _ in 0..60 {
                p.update(&Input::default(), view, DT);
            }
        }
        71 => {
            launch_ball(&mut p, view);
            rally(&mut p, view, 60 * 7);
        }
        72 | 77 => {
            launch_ball(&mut p, view);
            rally(&mut p, view, 60 * 9);
            p.g.combo = 4;
        }
        74 | 78 => {
            if n == 78 {
                board(&[("Leo", 5, 1400), ("Ava", 9, 900)]);
                p = BreakoutPlay::new();
                p.g.start_level(level, *view);
            }
            launch_ball(&mut p, view);
            let last = p.g.bricks.iter().rposition(|b| b.is_some()).unwrap();
            for slot in p.g.bricks.iter_mut().take(last) {
                *slot = None;
            }
            let target = p.g.bricks[last].unwrap();
            p.g.ball = vec2(target.center().x, target.bottom() + 40.0);
            p.g.vel = vec2(0.0, -400.0);
            for _ in 0..60 {
                p.update(&Input::default(), view, DT);
            }
            for _ in 0..40 {
                p.update(&Input::default(), view, DT);
            }
        }
        75 => {
            board(&[("Leo", 5, 700), ("Ava", 9, 420), ("Sam", 3, 90)]);
            p = BreakoutPlay::new();
            p.g.start_level(level, *view);
            launch_ball(&mut p, view);
            p.g.lives = 1;
            p.g.score = 240;
            p.g.ball = vec2(30.0, view.h + 5.0);
            p.g.vel = vec2(0.0, 600.0);
            p.g.paddle_x = view.w - 200.0;
            for _ in 0..80 {
                p.update(&Input::default(), view, DT);
            }
        }
        _ => {}
    }
    kg_core::sfx::take_cues();
    Stage::Game(Box::new(p))
}

#[cfg(test)]
pub fn check() {
    use breakout::State as B;
    for v in [View::ipad_landscape(), View::ipad_portrait()] {
        let get = |n: u32| super::game::<BreakoutPlay>(setup(n, &v));
        assert_eq!(get(70).g.state, B::Serve);
        let mid = get(71);
        assert!(mid.g.state == B::Playing && mid.g.cleared() > 0);
        let seven = get(72);
        assert!(seven.g.level == 7 && seven.g.combo >= 2);
        assert_eq!(get(73).g.state, B::Select);
        assert_eq!(get(74).g.state, B::Cleared);
        let lost = get(75);
        assert!(lost.g.state == B::Lost && lost.g.rank.is_some());
        assert_eq!(get(76).g.level, breakout::LEVELS as u32);
        assert_eq!(get(77).g.state, B::Playing);
        assert_eq!(get(78).g.state, B::Cleared);
        assert!(get(79).g.first_win());
    }
}

fn board(scores: &[(&str, usize, i64)]) {
    let me = profile::current_id();
    for (name, avatar, score) in scores {
        let p = profile::create(name, *avatar).unwrap();
        profile::select(&p.id);
        profile::submit(breakout::KEY, *score, profile::Better::Higher);
    }
    if let Some(me) = me {
        profile::select(&me);
    }
}
