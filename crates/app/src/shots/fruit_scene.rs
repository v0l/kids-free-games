use super::{Stage, DT};
use crate::games::fruit::FruitPlay;
use crate::games::Play;
use glam::vec2;
use kg_core::engine::{Input, View};
use kg_core::games::fruit;
use kg_core::profile;

fn board(scores: &[(&str, usize, i64)]) {
    let me = profile::current_id();
    for (name, avatar, score) in scores {
        let q = profile::create(name, *avatar).unwrap();
        profile::select(&q.id);
        profile::submit(fruit::GAME, *score, profile::Better::Higher);
    }
    if let Some(me) = me {
        profile::select(&me);
    }
}

fn fruit_thing(kind: fruit::Kind, at: glam::Vec2, vel: glam::Vec2, r: f32) -> fruit::Thing {
    fruit::Thing {
        pos: at,
        vel,
        r,
        kind,
        spin: 1.5,
    }
}

pub fn setup(n: u32, view: &View) -> Stage {
    kg_core::save::reset_for_test();
    let mia = profile::create("Mia", 2).unwrap();
    profile::select(&mia.id);
    let tap = Input::press_at(vec2(100.0, 100.0));
    let mut p = FruitPlay::new();
    let mid = vec2(view.w / 2.0, view.h * 0.45);
    let still = glam::Vec2::ZERO;
    match n {
        50 => {
            for _ in 0..40 {
                p.update(&Input::default(), view, DT);
            }
        }
        51 => {
            p.update(&tap, view, DT);
            p.g.things
                .push(fruit_thing(fruit::Kind::Fruit(0), mid, still, 46.0));
            p.g.things.push(fruit_thing(
                fruit::Kind::Fruit(3),
                mid + vec2(160.0, -60.0),
                still,
                44.0,
            ));
            p.update(&Input::default(), view, DT);
        }
        52 => {
            p.update(&tap, view, DT);
            profile::set_level(fruit::GAME, 6);
            p.g = fruit::Fruit::new();
            p.update(&tap, view, DT);
            p.g.best.0 = 12;
            p.g.sliced = 14;
            p.g.score = 17;
            for (i, f) in [2usize, 1, 5].into_iter().enumerate() {
                p.g.things.push(fruit_thing(
                    fruit::Kind::Fruit(f),
                    mid + vec2(i as f32 * 110.0 - 110.0, 0.0),
                    still,
                    42.0,
                ));
            }
            p.g.things.push(fruit_thing(
                fruit::Kind::Fruit(4),
                mid + vec2(-200.0, 160.0),
                still,
                40.0,
            ));
            p.g.slice(mid + vec2(-260.0, 10.0), mid + vec2(260.0, -10.0));
            for _ in 0..10 {
                p.update(&Input::default(), view, DT);
            }
            p.g.things.push(fruit_thing(
                fruit::Kind::Fruit(0),
                mid + vec2(-200.0, 160.0),
                still,
                42.0,
            ));
            p.g.blade = (0..8)
                .map(|i| {
                    (
                        mid + vec2(-220.0 + i as f32 * 50.0, 150.0 - i as f32 * 14.0),
                        0.02 * (i + 1) as f32,
                    )
                })
                .collect();
        }
        53 => {
            p.update(&tap, view, DT);
            p.g.start_level = 2;
            p.g.lives = 2;
            p.g.sliced = fruit::FRUIT_PER_LEVEL * 2 - 1;
            p.g.things
                .push(fruit_thing(fruit::Kind::Fruit(0), mid, still, 44.0));
            p.g.slice(mid + vec2(-80.0, 0.0), mid + vec2(80.0, 0.0));
            for _ in 0..12 {
                p.update(&Input::default(), view, DT);
            }
        }
        54 => {
            profile::set_level(fruit::GAME, 8);
            p = FruitPlay::new();
            p.update(&tap, view, DT);
            p.g.best.0 = 30;
            p.g.things
                .push(fruit_thing(fruit::Kind::Bomb, mid, still, 34.0));
            p.g.things.push(fruit_thing(
                fruit::Kind::Fruit(2),
                mid + vec2(150.0, -40.0),
                still,
                42.0,
            ));
            for _ in 0..20 {
                p.g.things[0].pos = mid;
                p.update(&Input::default(), view, DT);
            }
        }
        55 => {
            profile::set_level(fruit::GAME, 8);
            p = FruitPlay::new();
            p.update(&tap, view, DT);
            p.g.things
                .push(fruit_thing(fruit::Kind::Bomb, mid, still, 34.0));
            p.g.slice(mid + vec2(-80.0, 0.0), mid + vec2(80.0, 0.0));
            for _ in 0..6 {
                p.update(&Input::default(), view, DT);
            }
        }
        57 => {
            profile::set_level(fruit::GAME, 8);
            p = FruitPlay::new();
            p.update(&tap, view, DT);
            p.g.score = 9;
            p.g.things
                .push(fruit_thing(fruit::Kind::Bomb, mid, still, 34.0));
            p.g.slice(mid + vec2(-80.0, 0.0), mid + vec2(80.0, 0.0));
            for _ in 0..80 {
                p.update(&Input::default(), view, DT);
            }
        }
        _ => {
            board(&[("Leo", 5, 23), ("Ava", 9, 14), ("Sam", 3, 6)]);
            p = FruitPlay::new();
            p.g.start_level = 2;
            p.update(&tap, view, DT);
            p.g.score = 17;
            p.g.sliced = 13;
            p.g.lives = 0;
            p.update(&Input::default(), view, DT);
            p.g.flash = 0.0;
            for _ in 0..90 {
                p.update(&Input::default(), view, DT);
            }
        }
    }
    kg_core::sfx::take_cues();
    Stage::Game(Box::new(p))
}

#[cfg(test)]
pub fn check() {
    for v in [View::ipad_landscape(), View::ipad_portrait()] {
        let get = |n: u32| super::game::<FruitPlay>(setup(n, &v));
        assert_eq!(get(50).g.state, fruit::State::Ready);
        assert_eq!(get(51).g.things.len(), 2);
        assert!(get(52)
            .g
            .popups
            .list
            .iter()
            .any(|p| p.text.contains("Combo")));
        assert!(get(53)
            .g
            .popups
            .list
            .iter()
            .any(|p| p.text.starts_with("Level")));
        assert!(get(54).g.show_bomb_hint() || get(54).g.bomb_hint_time > 0.0);
        assert_eq!(get(55).g.lives, 0);
        let over = get(56);
        assert!(over.g.shows_results() && over.g.rank.is_some());
        assert!(get(57).over());
    }
}
