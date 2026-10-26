use super::{Stage, DT};
use crate::games::snake::SnakePlay;
use crate::games::Play;
use glam::{ivec2, IVec2};
use kg_core::engine::{Dir, Input, View};
use kg_core::games::snake::{self, State, STAGE_APPLES};
use kg_core::profile;

fn heading(p: &SnakePlay) -> Option<Dir> {
    let g = &p.g;
    let head = g.body[0];
    let size = g.size;
    let wrapped = |d: IVec2| ivec2(d.x.rem_euclid(size.x), d.y.rem_euclid(size.y));
    let free = |d: Dir| {
        let at = wrapped(head + d.delta());
        d != g.dir.opposite() && !g.body.iter().take(g.body.len() - 1).any(|b| *b == at)
    };
    let gap = |d: Dir| {
        let at = wrapped(head + d.delta());
        let dx = (g.food.x - at.x).abs();
        let dy = (g.food.y - at.y).abs();
        dx.min(size.x - dx) + dy.min(size.y - dy)
    };
    Dir::ALL
        .into_iter()
        .filter(|d| free(*d))
        .min_by_key(|d| gap(*d))
}

fn steer(p: &mut SnakePlay, view: &View, frames: usize) {
    for _ in 0..frames {
        let input = match heading(p) {
            Some(d) if p.g.state == State::Playing && d != p.g.dir => Input::key(d),
            _ => Input::default(),
        };
        p.update(&input, view, DT);
        if p.g.state == State::Dead {
            break;
        }
    }
}

fn start(p: &mut SnakePlay, view: &View) {
    p.update(&Input::key(Dir::Right), view, DT);
}

fn until_score(p: &mut SnakePlay, view: &View, score: u32) {
    for _ in 0..60 * 600 {
        if p.g.score >= score || p.g.state == State::Dead {
            break;
        }
        steer(p, view, 1);
    }
}

fn die(p: &mut SnakePlay, view: &View) {
    p.g.moves = snake::GRACE_MOVES + 100;
    p.g.wrap = false;
    let (w, h) = (p.g.size.x, p.g.size.y);
    let row = h / 2;
    p.g.body = (0..p.g.body.len().max(4) as i32)
        .map(|i| ivec2(w - 1 - i, row))
        .collect();
    p.g.dir = Dir::Right;
    p.g.queued.clear();
    p.g.food = ivec2(1, 1);
    for _ in 0..150 {
        p.update(&Input::default(), view, DT);
    }
}

fn board(scores: &[(&str, usize, i64)]) {
    let me = profile::current_id();
    for (name, avatar, score) in scores {
        let q = profile::create(name, *avatar).unwrap();
        profile::select(&q.id);
        profile::submit(snake::GAME, *score, profile::Better::Higher);
    }
    if let Some(me) = me {
        profile::select(&me);
    }
}

pub fn setup(n: u32, view: &View) -> Stage {
    kg_core::save::reset_for_test();
    let mia = profile::create("Mia", 2).unwrap();
    profile::select(&mia.id);
    if n >= 32 {
        let prior = if n == 37 { 3 } else { 12 };
        profile::submit(snake::GAME, prior, profile::Better::Higher);
        profile::set(snake::GAME, "stage", 3);
    }
    let mut p = SnakePlay::new();
    match n {
        30 => {
            for _ in 0..40 {
                p.update(&Input::default(), view, DT);
            }
        }
        31 => {
            start(&mut p, view);
            steer(&mut p, view, 50);
        }
        32 => {
            start(&mut p, view);
            until_score(&mut p, view, 6);
            steer(&mut p, view, 20);
        }
        33 => {
            start(&mut p, view);
            until_score(&mut p, view, STAGE_APPLES);
            steer(&mut p, view, 12);
        }
        34 => {
            start(&mut p, view);
            p.g.score = STAGE_APPLES * 3;
            until_score(&mut p, view, STAGE_APPLES * 3 + 10);
            steer(&mut p, view, 10);
        }
        35 => {
            start(&mut p, view);
            until_score(&mut p, view, 4);
            let head = p.g.body[0];
            p.g.body = (0..7).map(|i| ivec2(head.x - i, head.y)).collect();
            p.g.dir = Dir::Right;
            p.g.queued.clear();
            p.g.food = ivec2(0, 0);
            p.g.moves = 0;
            for d in [Dir::Down, Dir::Left] {
                p.update(&Input::key(d), view, DT);
                steer_blind(&mut p, view);
            }
            p.update(&Input::key(Dir::Up), view, DT);
            steer_blind(&mut p, view);
        }
        36 => {
            board(&[("Leo", 5, 23), ("Ava", 9, 14), ("Sam", 3, 6)]);
            p = SnakePlay::new();
            start(&mut p, view);
            until_score(&mut p, view, 8);
            die(&mut p, view);
        }
        _ => {
            board(&[("Leo", 5, 2), ("Ava", 9, 1)]);
            p = SnakePlay::new();
            start(&mut p, view);
            until_score(&mut p, view, 6);
            die(&mut p, view);
        }
    }
    kg_core::sfx::take_cues();
    Stage::Game(Box::new(p))
}

fn steer_blind(p: &mut SnakePlay, view: &View) {
    let step = (p.g.step_time() / DT).ceil() as usize;
    for _ in 0..step {
        p.update(&Input::default(), view, DT);
    }
}

#[cfg(test)]
pub fn check() {
    for v in [View::ipad_landscape(), View::ipad_portrait()] {
        let get = |n: u32| super::game::<SnakePlay>(setup(n, &v));
        assert_eq!(get(30).g.state, State::Ready);
        let first = get(31);
        assert!(first.g.state == State::Playing && first.g.score == 0);
        assert!(get(32).g.score >= 6);
        assert!(get(33).g.stage() == 2);
        assert!(get(34).g.score >= STAGE_APPLES * 3 + 10);
        assert!(get(35).g.state == State::Playing);
        let over = get(36);
        assert!(over.g.shows_results() && over.g.rank.is_some_and(|r| r > 0));
        assert_eq!(get(37).g.rank, Some(0));
    }
}
