use super::{Stage, DT};
use crate::games::jumper::JumperPlay;
use crate::games::Play;
use glam::vec2;
use kg_core::engine::{Input, View};
use kg_core::games::jumper;
use kg_core::profile;

fn climb(p: &mut JumperPlay, view: &View, frames: usize, until: u32) {
    let hold = |left: bool| Input {
        down: true,
        pos: vec2(if left { 10.0 } else { view.w - 10.0 }, 300.0),
        ..Default::default()
    };
    for _ in 0..frames {
        let g = &p.g;
        let falling = g.vel.y > 0.0;
        let target = g
            .plats
            .iter()
            .filter(|q| !q.broken && q.kind != jumper::Kind::Crumble)
            .filter(|q| {
                if falling {
                    q.top() > g.pos.y + jumper::HERO_R && q.top() < g.pos.y + 200.0
                } else {
                    q.top() < g.pos.y - 10.0
                }
            })
            .min_by(|a, b| {
                (a.top() - g.pos.y)
                    .abs()
                    .total_cmp(&(b.top() - g.pos.y).abs())
            })
            .map(|q| q.x + jumper::PLAT_W / 2.0);
        let input = match target {
            Some(x) if (x - g.pos.x).abs() > 20.0 => hold(x < g.pos.x),
            _ => Input::default(),
        };
        p.update(&input, view, DT);
        if p.g.state != jumper::State::Playing || p.g.score >= until {
            break;
        }
    }
}

fn teleport(g: &mut jumper::Jumper, view: &View, score: u32) {
    g.score = score;
    g.cam = -(score as f32) * 10.0;
    g.milestone = score / jumper::MILESTONE;
    g.stage = jumper::stage_for(score);
    let y = g.cam + view.h * 0.7;
    g.plats = vec![jumper::Plat {
        x: g.pos.x - jumper::PLAT_W / 2.0,
        y,
        kind: jumper::Kind::Solid,
        broken: false,
    }];
    g.top = y;
    g.pos.y = y - jumper::HERO_R - 4.0;
    g.vel.y = 100.0;
    g.rescues = 2;
}

fn fall(p: &mut JumperPlay, view: &View) {
    p.g.rescues = 0;
    p.g.plats.clear();
    p.g.pos.y = p.g.cam + view.h + 200.0;
    for _ in 0..80 {
        p.update(&Input::default(), view, DT);
    }
}

fn jumper_board(scores: &[(&str, usize, i64)]) {
    let me = profile::current_id();
    for (name, avatar, score) in scores {
        let p = profile::create(name, *avatar).unwrap();
        profile::select(&p.id);
        profile::submit(jumper::KEY, *score, profile::Better::Higher);
    }
    if let Some(me) = me {
        profile::select(&me);
    }
}

pub fn setup(n: u32, view: &View) -> Stage {
    kg_core::save::reset_for_test();
    let mia = profile::create("Mia", 2).unwrap();
    profile::select(&mia.id);
    if n != 81 {
        profile::set(jumper::KEY, "runs", 5);
    }
    if n == 86 {
        jumper_board(&[("Leo", 5, 640), ("Ava", 9, 410), ("Sam", 3, 120)]);
    }
    if n == 87 {
        jumper_board(&[("Leo", 5, 90), ("Ava", 9, 60)]);
    }
    let mut p = JumperPlay::new();
    let tap = Input::press_at(vec2(100.0, 100.0));
    match n {
        80 => {
            for _ in 0..40 {
                p.update(&Input::default(), view, DT);
            }
        }
        81 => {
            p.update(&tap, view, DT);
            for _ in 0..20 {
                p.update(&Input::default(), view, DT);
            }
        }
        82 => {
            p.update(&tap, view, DT);
            climb(&mut p, view, 60 * 60, 130);
        }
        83 => {
            p.update(&tap, view, DT);
            teleport(&mut p.g, view, jumper::STAGES[0].0 - 6);
            climb(&mut p, view, 60 * 20, jumper::STAGES[0].0);
            climb(&mut p, view, 25, u32::MAX);
        }
        84 => {
            p.update(&tap, view, DT);
            teleport(&mut p.g, view, 560);
            climb(&mut p, view, 60 * 12, u32::MAX);
        }
        85 => {
            p.update(&tap, view, DT);
            climb(&mut p, view, 60 * 60, 40);
            let at = p.g.pos.x;
            let y = p.g.cam + view.h * 0.75;
            p.g.plats.clear();
            p.g.plats.push(jumper::Plat {
                x: at - jumper::PLAT_W / 2.0,
                y,
                kind: jumper::Kind::Spring,
                broken: false,
            });
            p.g.pos.y = y - jumper::SPRING_H - jumper::HERO_R - 60.0;
            p.g.vel.y = 400.0;
            for _ in 0..14 {
                p.update(&Input::default(), view, DT);
            }
        }
        _ => {
            p.update(&tap, view, DT);
            climb(&mut p, view, 60 * 60, if n == 86 { 150 } else { 120 });
            fall(&mut p, view);
        }
    }
    kg_core::sfx::take_cues();
    Stage::Game(Box::new(p))
}

#[cfg(test)]
pub fn check() {
    for v in [View::ipad_landscape(), View::ipad_portrait()] {
        let get = |n: u32| super::game::<JumperPlay>(setup(n, &v));
        assert_eq!(get(80).g.state, jumper::State::Ready);
        assert!(get(81).g.state == jumper::State::Playing && get(81).g.shows_steer_hint());
        let mid = get(82);
        assert!(mid.g.state == jumper::State::Playing && mid.g.score >= 100);
        assert!(get(83).g.score >= jumper::STAGES[0].0);
        assert!(get(84).g.score >= 500);
        let spring = get(85);
        assert!(spring
            .g
            .plats
            .iter()
            .any(|q| spring.g.boing_age(q).is_some()));
        let over = get(86);
        assert!(over.g.state == jumper::State::Fallen && over.g.rank.is_some_and(|r| r > 0));
        assert_eq!(get(87).g.rank, Some(0));
    }
}
