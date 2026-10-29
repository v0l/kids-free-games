use super::{Stage, DT};
use crate::games::runner::RunnerPlay;
use crate::games::Play;
use kg_core::engine::{Input, View};
use kg_core::games::runner::*;
use kg_core::profile;

fn board(scores: &[(&str, usize, i64)]) {
    let me = profile::current_id();
    for (name, avatar, score) in scores {
        let q = profile::create(name, *avatar).unwrap();
        profile::select(&q.id);
        profile::submit(GAME, *score, profile::Better::Higher);
    }
    if let Some(me) = me {
        profile::select(&me);
    }
}

fn start(p: &mut RunnerPlay, view: &View) {
    p.update(&Input::press_at(view.center()), view, DT);
}

fn run(p: &mut RunnerPlay, view: &View, frames: usize) {
    for _ in 0..frames {
        if p.g.state != State::Playing {
            break;
        }
        let input = p.g.autopilot(1.2);
        p.update(&input, view, DT);
    }
}

fn run_to(p: &mut RunnerPlay, view: &View, dist: f32) {
    for _ in 0..60 * 600 {
        if p.g.state != State::Playing || p.g.dist >= dist {
            break;
        }
        let input = p.g.autopilot(1.2);
        p.update(&input, view, DT);
    }
}

fn settle(p: &mut RunnerPlay, view: &View, frames: usize) {
    for _ in 0..frames {
        p.update(&Input::default(), view, DT);
    }
}

fn before_row(p: &mut RunnerPlay, view: &View, want: Kind, gap: f32) {
    let at = p.g.rows.iter().find(|r| r.kind() == want).unwrap().at;
    run_to(p, view, at - gap);
}

pub fn setup(n: u32, view: &View) -> Stage {
    kg_core::save::reset_for_test();
    kg_core::rng::srand(n as u64 + 3);
    let mia = profile::create("Mia", 2).unwrap();
    profile::select(&mia.id);
    let level = match n {
        203 => 3,
        204 => 8,
        207 => 6,
        208 => 12,
        209 => 30,
        210 => 22,
        _ => 1,
    };
    profile::unlock(GAME, if n == 200 { 9 } else { level.max(8) });
    if n == 200 {
        profile::award_stars(GAME, 1, 3);
        profile::award_stars(GAME, 2, 2);
        profile::award_stars(GAME, 3, 1);
        profile::set_level(GAME, 4);
    }
    if n != 202 && n != 203 {
        profile::set(GAME, "taught_dodge", 1);
        profile::set(GAME, "taught_jump", 1);
        profile::set(GAME, "taught_slide", 1);
    }
    let mut p = RunnerPlay::new();
    p.g = Runner::with_level(level);
    p.g.best = kg_core::engine::Best(900);
    match n {
        200 => {
            p.g.open_select();
            settle(&mut p, view, 30);
        }
        201 => settle(&mut p, view, 50),
        202 => {
            start(&mut p, view);
            run(&mut p, view, 140);
        }
        203 => {
            start(&mut p, view);
            before_row(&mut p, view, Kind::Hurdle, 12.0);
            settle(&mut p, view, 4);
        }
        204 => {
            start(&mut p, view);
            before_row(&mut p, view, Kind::Train, 14.0);
            settle(&mut p, view, 4);
        }
        205 => {
            start(&mut p, view);
            run_to(&mut p, view, 90.0);
            p.g.coins = 17;
            p.g.combo = 5;
            p.g.combo_t = 1.0;
        }
        206 => {
            start(&mut p, view);
            run_to(&mut p, view, 70.0);
            let r =
                p.g.rows
                    .iter()
                    .find(|r| r.at > p.g.dist + 8.0)
                    .unwrap()
                    .clone();
            let lane = p.g.lane;
            let at = r.at;
            let row = p.g.rows.iter_mut().find(|x| x.at == at).unwrap();
            row.lanes = [None; 3];
            row.lanes[(lane + 1) as usize] = Some(Kind::Barrier);
            p.g.dist = at - 1.0;
            for _ in 0..14 {
                p.update(&Input::default(), view, DT);
            }
        }
        207 => {
            start(&mut p, view);
            before_row(&mut p, view, Kind::Overhead, 2.5);
            run(&mut p, view, 10);
            p.update(&Input::key(kg_core::engine::Dir::Down), view, DT);
            settle(&mut p, view, 6);
        }
        208 => {
            board(&[("Leo", 5, 1500), ("Ava", 9, 700), ("Sam", 3, 120)]);
            start(&mut p, view);
            run_to(&mut p, view, 60.0);
            p.g.hearts = 1;
            settle(&mut p, view, 600);
        }
        209 => {
            start(&mut p, view);
            let end = p.g.length;
            run_to(&mut p, view, end + 1.0);
            settle(&mut p, view, 60 * 3);
        }
        210 => {
            start(&mut p, view);
            settle(&mut p, view, 200);
            let at = p.g.dist + 12.0;
            p.g.rows
                .retain(|r| r.at + r.len() < p.g.dist - 1.0 || r.at > at + 12.0);
            p.g.coins_list
                .retain(|c| c.at < p.g.dist - 1.0 || c.at > at + 12.0);
            let mut lanes = [None; 3];
            lanes[1] = Some(Kind::Hurdle);
            p.g.rows.push(Row {
                at,
                lanes,
                hit: false,
                hit_t: 0.0,
                hit_lanes: [false; 3],
            });
            for (c, h) in jump_arc(at + ROW_LEN / 2.0, p.g.speed) {
                p.g.coins_list.push(Coin {
                    at: c,
                    lane: 1,
                    h,
                    taken: false,
                });
            }
            p.g.rows.sort_by(|a, b| a.at.total_cmp(&b.at));
            p.g.coins_list.sort_by(|a, b| a.at.total_cmp(&b.at));
            let air = 2.0 * JUMP_V / GRAVITY;
            let takeoff = at + ROW_LEN / 2.0 - p.g.speed * air / 2.0;
            while p.g.dist < takeoff - 3.0 {
                settle(&mut p, view, 1);
            }
        }
        _ => {}
    }
    kg_core::sfx::take_cues();
    Stage::Game(Box::new(p))
}

#[cfg(test)]
pub fn check() {
    for v in [View::ipad_landscape(), View::ipad_portrait()] {
        let get = |n: u32| super::game::<RunnerPlay>(setup(n, &v));
        assert_eq!(get(200).g.state, State::Select);
        assert_eq!(get(201).g.state, State::Ready);
        let first = get(202);
        assert!(first.g.state == State::Playing && first.g.coach().is_some());
        assert!(get(203).g.rows.iter().any(|r| r.kind() == Kind::Hurdle));
        assert!(get(204).g.dist > 5.0);
        assert!(get(205).g.combo >= 5);
        assert!(get(206).g.hits == 1, "{}", get(206).g.hits);
        assert!(get(207).g.sliding());
        let dead = get(208);
        assert!(dead.over() && dead.g.rank.is_some(), "{:?}", dead.g.state);
        let won = get(209);
        assert_eq!(won.g.state, State::Won);
        assert_eq!(won.g.stars, 3);
        let mut arc = get(210);
        let ahead = |g: &Runner| {
            g.coins_list
                .iter()
                .filter(|c| !c.taken && c.at > g.dist)
                .count()
        };
        assert!(ahead(&arc.g) >= 3);
        let before = arc.g.coins;
        arc.update(&Input::key(kg_core::engine::Dir::Up), &v, DT);
        for _ in 0..120 {
            arc.update(&Input::default(), &v, DT);
        }
        assert_eq!(arc.g.hits, 0);
        assert!(arc.g.coins >= before + 3, "{} -> {}", before, arc.g.coins);
    }
}
