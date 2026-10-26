use super::{Stage, DT};
use crate::games::stack::StackPlay;
use crate::games::Play;
use glam::vec2;
use kg_core::engine::{Input, View};
use kg_core::games::stack;
use kg_core::profile;

fn board(scores: &[(&str, usize, i64)]) {
    let me = profile::current_id();
    for (name, avatar, score) in scores {
        let q = profile::create(name, *avatar).unwrap();
        profile::select(&q.id);
        profile::submit(stack::KEY, *score, profile::Better::Higher);
    }
    if let Some(me) = me {
        profile::select(&me);
    }
}

fn stack_drops(p: &mut StackPlay, count: u32, offset: f32) {
    for _ in 0..count {
        let top = *p.g.tower.last().unwrap();
        p.g.moving.x = top.x + offset;
        p.g.drop_block();
    }
}

fn stack_bot(p: &mut StackPlay, view: &View, frames: usize) {
    for _ in 0..frames {
        let near = (p.g.moving.x - p.g.tower.last().unwrap().x).abs() < 14.0;
        let input = if near {
            Input::press_at(vec2(500.0, 500.0))
        } else {
            Input::default()
        };
        p.update(&input, view, DT);
        if p.g.state != stack::State::Playing {
            break;
        }
    }
}

pub fn setup(n: u32, view: &View) -> Stage {
    kg_core::save::reset_for_test();
    let mia = profile::create("Mia", 2).unwrap();
    profile::select(&mia.id);
    if n != 60 {
        profile::set(stack::KEY, "taught", 1);
    }
    let level = match n {
        62 | 63 => 4,
        64 | 68 => 3,
        65 => 2,
        67 => 6,
        _ => 1,
    };
    profile::unlock(stack::KEY, if n == 66 { 8 } else { level });
    let mut p = StackPlay::new();
    p.g = stack::Stack::for_level(level);
    match n {
        60 => {
            for _ in 0..40 {
                p.update(&Input::default(), view, DT);
            }
        }
        61 => {
            stack_drops(&mut p, 2, 0.0);
            stack_drops(&mut p, 1, 40.0);
            p.g.moving.x = -60.0;
            for _ in 0..30 {
                p.update(&Input::default(), view, DT);
            }
        }
        62 => {
            stack_drops(&mut p, 4, 0.0);
            stack_drops(&mut p, 5, 35.0);
            for _ in 0..150 {
                p.update(&Input::default(), view, DT);
            }
        }
        63 => {
            stack_drops(&mut p, 4, 0.0);
            p.update(&Input::default(), view, DT);
        }
        64 | 68 => {
            if n == 64 {
                board(&[("Leo", 5, 30), ("Ava", 9, 18), ("Sam", 3, 7)]);
            } else {
                board(&[("Leo", 5, 3), ("Ava", 9, 1)]);
            }
            p.g = stack::Stack::for_level(3);
            stack_drops(&mut p, if n == 64 { 4 } else { 6 }, 20.0);
            p.g.moving.x = 400.0;
            p.g.drop_block();
            for _ in 0..120 {
                p.update(&Input::default(), view, DT);
            }
        }
        65 => {
            let goal = p.g.goal;
            stack_drops(&mut p, goal, 0.0);
            for _ in 0..70 {
                p.update(&Input::default(), view, DT);
            }
        }
        66 => {
            profile::award_stars(stack::KEY, 1, 3);
            profile::award_stars(stack::KEY, 2, 2);
            profile::award_stars(stack::KEY, 3, 1);
            p.g.picking = true;
            for _ in 0..30 {
                p.update(&Input::default(), view, DT);
            }
        }
        _ => {
            let goal = p.g.goal;
            stack_drops(&mut p, goal - 2, 0.0);
            for _ in 0..150 {
                p.update(&Input::default(), view, DT);
            }
            stack_bot(&mut p, view, 20);
        }
    }
    kg_core::sfx::take_cues();
    Stage::Game(Box::new(p))
}

#[cfg(test)]
pub fn check() {
    for v in [View::ipad_landscape(), View::ipad_portrait()] {
        let get = |n: u32| super::game::<StackPlay>(setup(n, &v));
        let first = get(60);
        assert!(!first.g.taught && first.g.tower.len() == 1);
        assert!(get(61).g.tower.len() >= 3);
        assert!(get(62).g.tower.len() >= 8);
        assert!(get(63).g.streak >= 3);
        let over = get(64);
        assert!(over.g.shows_results() && over.g.rank.is_some_and(|r| r > 0));
        assert_eq!(get(65).g.state, stack::State::Won);
        assert!(get(66).g.picking);
        let near = get(67);
        assert!(near.g.score + 2 >= near.g.goal);
        assert_eq!(get(68).g.rank, Some(0));
    }
}
