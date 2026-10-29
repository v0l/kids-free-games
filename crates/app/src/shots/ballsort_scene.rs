use super::{Stage, DT};
use crate::games::ballsort::BallSortPlay;
use crate::games::Play;
use glam::vec3;
use kg_core::engine::{Input, View};
use kg_core::games::ballsort::*;
use kg_core::profile;

const KEY: &str = GAME;

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

fn tap(p: &mut BallSortPlay, view: &View, tube: usize) {
    let s = p.g.spot(tube);
    let at = p.g.cam.project(view, vec3(s.x, TUBE_H * 0.5, s.y));
    p.update(&Input::press_at(at), view, DT);
    p.update(&Input::default(), view, DT);
}

fn pour_next(p: &mut BallSortPlay, view: &View) -> bool {
    let Some(&(a, b)) = solve::solve(&p.g.packed(), 400_000)
        .as_ref()
        .and_then(|s| s.first())
    else {
        return false;
    };
    tap(p, view, a as usize);
    tap(p, view, b as usize);
    for _ in 0..90 {
        p.update(&Input::default(), view, DT);
    }
    true
}

fn win(p: &mut BallSortPlay, view: &View) {
    while p.g.state == State::Playing && pour_next(p, view) {}
    for _ in 0..(60.0 * (WIN_DELAY + 1.6)) as usize {
        p.update(&Input::default(), view, DT);
    }
}

pub fn setup(n: u32, view: &View) -> Stage {
    kg_core::save::reset_for_test();
    let mia = profile::create("Mia", 2).unwrap();
    profile::select(&mia.id);
    let level = match n {
        254 => 9,
        255 => 17,
        256 => 24,
        259 => 30,
        _ => 1,
    };
    profile::unlock(KEY, if n == 250 { 9 } else { level.max(1) });
    if n == 250 {
        profile::award_stars(KEY, 1, 3);
        profile::award_stars(KEY, 2, 2);
        profile::award_stars(KEY, 3, 1);
        profile::set_level(KEY, 4);
    }
    if n != 251 {
        profile::set(KEY, "taught", 1);
    }
    let mut p = BallSortPlay::new();
    p.g = BallSort::with_level(level);
    p.g.best = kg_core::engine::Best(300);
    p.update(&Input::default(), view, DT);
    match n {
        250 => {
            p.g.open_select();
            for _ in 0..30 {
                p.update(&Input::default(), view, DT);
            }
        }
        251 => {
            for _ in 0..30 {
                p.update(&Input::default(), view, DT);
            }
        }
        252 => {
            let s = solve::solve(&p.g.packed(), 100_000).unwrap();
            tap(&mut p, view, s[0].0 as usize);
            for _ in 0..30 {
                p.update(&Input::default(), view, DT);
            }
        }
        253 | 254 | 255 | 256 | 259 => {
            for _ in 0..2 {
                pour_next(&mut p, view);
            }
            let s = solve::solve(&p.g.packed(), 400_000).unwrap();
            tap(&mut p, view, s[0].0 as usize);
            tap(&mut p, view, s[0].1 as usize);
            for _ in 0..22 {
                p.update(&Input::default(), view, DT);
            }
        }
        257 => {
            board(&[("Leo", 5, 700), ("Ava", 9, 400), ("Sam", 3, 120)]);
            win(&mut p, view);
        }
        258 => {
            p.g.undos = 3;
            win(&mut p, view);
        }
        _ => {}
    }
    kg_core::sfx::take_cues();
    Stage::Game(Box::new(p))
}

#[cfg(test)]
pub fn check() {
    for v in [View::ipad_landscape(), View::ipad_portrait()] {
        let get = |n: u32| super::game::<BallSortPlay>(setup(n, &v));
        assert_eq!(get(250).g.state, State::Select);
        assert_eq!(get(251).g.moves, 0);
        assert!(get(252).g.selected.is_some());
        assert!(get(253).g.moves >= 2);
        assert!(get(254).g.level == 9);
        assert!(get(255).g.hidden);
        assert!(get(256).g.tubes.len() >= 9);
        let won = get(257);
        assert!(won.over(), "{:?} {}", won.g.state, won.g.end_t);
        assert!(won.g.rank.is_some());
        assert_eq!(won.g.stars, 3);
        assert!(get(258).g.stars < 3);
        assert_eq!(get(259).g.level, 30);
    }
}
