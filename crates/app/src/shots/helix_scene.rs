use super::{Stage, DT};
use crate::games::helix::HelixPlay;
use crate::games::Play;
use glam::vec2;
use kg_core::engine::{Input, View};
use kg_core::games::helix::*;
use kg_core::profile;
use std::f32::consts::{PI, TAU};

fn board(scores: &[(&str, usize, i64)]) {
    let me = profile::current_id();
    for (name, avatar, score) in scores {
        let q = profile::create(name, *avatar).unwrap();
        profile::select(&q.id);
        profile::submit(KEY, *score, profile::Better::Higher);
    }
    if let Some(me) = me {
        profile::select(&me);
    }
}

const KEY: &str = GAME;

fn steer(g: &Helix) -> Input {
    let i = g.passed.min(g.floors.len() - 1);
    let segs = g.floors[i].segs;
    let start = gap_start(&segs).unwrap();
    let len = segs.iter().filter(|s| **s == Seg::Gap).count();
    let mid = (start as f32 + len as f32 / 2.0) / SEGMENTS as f32 * TAU;
    let want = (ball_angle() - mid).rem_euclid(TAU);
    let mut d = want - g.angle.rem_euclid(TAU);
    if d > PI {
        d -= TAU;
    }
    if d < -PI {
        d += TAU;
    }
    Input {
        left: d < -0.05,
        right: d > 0.05,
        ..Default::default()
    }
}

fn play_until(p: &mut HelixPlay, view: &View, floors: usize) {
    p.update(&Input::press_at(vec2(500.0, 500.0)), view, DT);
    for _ in 0..60 * 120 {
        if p.g.state != State::Playing || p.g.passed >= floors {
            break;
        }
        let input = steer(&p.g);
        p.update(&input, view, DT);
    }
}

fn die(p: &mut HelixPlay, view: &View) {
    let i = p.g.passed;
    let k = segment_at(ball_angle(), p.g.angle);
    p.g.floors[i].segs[k] = Seg::Danger;
    for _ in 0..180 {
        p.update(&Input::default(), view, DT);
        if p.g.state == State::Dead {
            break;
        }
    }
    for _ in 0..40 {
        p.update(&Input::default(), view, DT);
    }
}

fn win(p: &mut HelixPlay, view: &View, floors: usize) {
    p.update(&Input::press_at(vec2(500.0, 500.0)), view, DT);
    for _ in 0..60 * 600 {
        if p.g.state != State::Playing {
            break;
        }
        let input = steer(&p.g);
        p.update(&input, view, DT);
    }
    let _ = floors;
    for _ in 0..(60.0 * (WIN_DELAY + 1.6)) as usize {
        p.update(&Input::default(), view, DT);
    }
}

pub fn setup(n: u32, view: &View) -> Stage {
    kg_core::save::reset_for_test();
    let mia = profile::create("Mia", 2).unwrap();
    profile::select(&mia.id);
    let level = match n {
        114 => 9,
        115 => 12,
        118 => 3,
        119 => 30,
        _ => 1,
    };
    profile::unlock(KEY, if n == 110 { 9 } else { level });
    if n == 110 {
        profile::award_stars(KEY, 1, 3);
        profile::award_stars(KEY, 2, 2);
        profile::award_stars(KEY, 3, 1);
        profile::set_level(KEY, 4);
    }
    if n != 112 {
        profile::set(KEY, "taught", 1);
    }
    let mut p = HelixPlay::new();
    p.g = Helix::with_level(level);
    p.g.best = kg_core::engine::Best(120);
    match n {
        110 => {
            p.g.open_select();
            for _ in 0..30 {
                p.update(&Input::default(), view, DT);
            }
        }
        111 => {
            for _ in 0..40 {
                p.update(&Input::default(), view, DT);
            }
        }
        112 => {
            p.update(&Input::press_at(vec2(500.0, 500.0)), view, DT);
            for _ in 0..14 {
                p.update(&Input::default(), view, DT);
            }
        }
        113 | 114 | 119 => {
            let to = if n == 113 { 5 } else { 4 };
            play_until(&mut p, view, to);
            for _ in 0..8 {
                let input = steer(&p.g);
                p.update(&input, view, DT);
            }
        }
        115 => {
            play_until(&mut p, view, 3);
            p.g.streak = SMASH_FLOORS + 2;
            for _ in 0..6 {
                let input = steer(&p.g);
                p.update(&input, view, DT);
            }
        }
        116 => {
            board(&[("Leo", 5, 90), ("Ava", 9, 40), ("Sam", 3, 12)]);
            play_until(&mut p, view, 3);
            die(&mut p, view);
        }
        117 | 118 => {
            if n == 118 {
                p.g.bounces = 100;
            }
            win(&mut p, view, 0);
        }
        _ => {}
    }
    kg_core::sfx::take_cues();
    Stage::Game(Box::new(p))
}

#[cfg(test)]
pub fn check() {
    for v in [View::ipad_landscape(), View::ipad_portrait()] {
        let get = |n: u32| super::game::<HelixPlay>(setup(n, &v));
        assert_eq!(get(110).g.state, State::Select);
        let ready = get(111);
        assert_eq!(ready.g.state, State::Ready);
        let first = get(112);
        assert!(first.g.state == State::Playing && !first.g.taught);
        assert!(get(113).g.passed >= 5);
        let red = get(114);
        assert!(red.g.floors.iter().any(|f| f.segs.contains(&Seg::Danger)));
        assert!(get(115).g.streak >= SMASH_FLOORS);
        let dead = get(116);
        assert!(dead.over() && dead.g.rank.is_some());
        assert_eq!(get(117).g.state, State::Won);
        assert_eq!(get(117).g.stars, 3);
        assert!(get(118).g.stars < 3);
        assert!(get(119).g.level == 30);
    }
}
