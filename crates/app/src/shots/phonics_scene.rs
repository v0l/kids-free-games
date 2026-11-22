use super::{Stage, DT};
use crate::games::phonics::PhonicsPlay;
use crate::games::Play;
use kg_core::engine::{Input, View};
use kg_core::games::phonics::{self, Screen};
use kg_core::profile;

fn settle(p: &mut PhonicsPlay, view: &View, secs: f32) {
    for _ in 0..(secs / DT) as usize {
        p.update(&Input::default(), view, DT);
    }
}

fn pop(p: &mut PhonicsPlay, view: &View, right: bool) {
    let g = &p.g;
    let slot = (0..g.balloons.len())
        .find(|i| {
            let b = &g.balloons[*i];
            !b.popped && !b.tried && (b.value == g.question.target) == right
        })
        .unwrap();
    let at = g.center(view, slot);
    p.update(&Input::press_at(at), view, DT);
}

fn clear(p: &mut PhonicsPlay, view: &View) {
    while matches!(p.g.screen, Screen::Play) {
        settle(p, view, 1.0);
        pop(p, view, true);
        settle(p, view, 1.2);
    }
}

pub fn setup(n: u32, view: &View) -> Stage {
    kg_core::save::reset_for_test();
    let mia = profile::create("Mia", 2).unwrap();
    profile::select(&mia.id);
    let key = phonics::KEY;
    if n != 310 {
        profile::set(key, "played", 1);
    }
    match n {
        314 => {
            profile::unlock(key, 5);
            profile::set_level(key, 5);
        }
        315 => {
            profile::unlock(key, 3);
            profile::award_stars(key, 0, 3);
            profile::award_stars(key, 1, 2);
            profile::award_stars(key, 2, 1);
        }
        318 => {
            profile::unlock(key, 7);
            profile::set_level(key, 7);
            for l in 0..7 {
                profile::award_stars(key, l, 3);
            }
        }
        _ => {}
    }
    let mut p = PhonicsPlay::new();
    settle(&mut p, view, 1.2);
    match n {
        311 | 314 => {
            pop(&mut p, view, true);
            settle(&mut p, view, 1.2);
            pop(&mut p, view, false);
            settle(&mut p, view, 0.12);
        }
        312 => {
            pop(&mut p, view, false);
            settle(&mut p, view, 0.3);
            pop(&mut p, view, false);
            settle(&mut p, view, 0.8);
        }
        313 => {
            pop(&mut p, view, true);
            settle(&mut p, view, 0.35);
        }
        315 => {
            let b = phonics::levels_button(view).center();
            p.update(&Input::press_at(b), view, DT);
            settle(&mut p, view, 0.5);
        }
        316 => {
            clear(&mut p, view);
            settle(&mut p, view, 1.2);
        }
        318 => {
            clear(&mut p, view);
            settle(&mut p, view, 1.2);
        }
        _ => {}
    }
    kg_core::sfx::take_cues();
    Stage::Game(Box::new(p))
}

#[cfg(test)]
pub fn check() {
    for v in [View::ipad_landscape(), View::ipad_portrait()] {
        let get = |n: u32| super::game::<PhonicsPlay>(setup(n, &v));
        assert!(get(310).g.teach);
        assert!(get(311).g.balloons.iter().any(|b| b.tried));
        assert!(get(312).g.revealed());
        assert!(get(313).g.solved);
        assert_eq!(get(314).g.level, 5);
        assert_eq!(get(315).g.screen, Screen::Levels);
        assert!(matches!(get(316).g.screen, Screen::Done(_)));
        assert!(matches!(get(318).g.screen, Screen::Final(_)));
    }
}
