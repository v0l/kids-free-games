use super::{Stage, DT};
use crate::games::mathpop::MathPopPlay;
use crate::games::Play;
use kg_core::engine::{Input, View};
use kg_core::games::mathpop::{self, Screen};
use kg_core::profile;

fn mp_settle(p: &mut MathPopPlay, view: &View, secs: f32) {
    for _ in 0..(secs / DT) as usize {
        p.update(&Input::default(), view, DT);
    }
}

fn mp_pop(p: &mut MathPopPlay, view: &View, right: bool) {
    let g = &p.g;
    let slot = (0..g.balloons.len())
        .find(|i| {
            let b = &g.balloons[*i];
            !b.popped && !b.tried && (b.value == g.question.answer) == right
        })
        .unwrap();
    let at = g.center(view, slot);
    p.update(&Input::press_at(at), view, DT);
}

fn mp_clear(p: &mut MathPopPlay, view: &View, slow: bool) {
    while matches!(p.g.screen, Screen::Play) {
        mp_settle(p, view, if slow { 9.0 } else { 1.0 });
        mp_pop(p, view, true);
        mp_settle(p, view, 1.2);
    }
}

pub fn setup(n: u32, view: &View) -> Stage {
    kg_core::save::reset_for_test();
    let mia = profile::create("Mia", 2).unwrap();
    profile::select(&mia.id);
    let key = mathpop::KEY;
    if n != 130 {
        profile::set(key, "played", 1);
    }
    match n {
        134 => {
            profile::unlock(key, 7);
            profile::set_level(key, 6);
        }
        138 => {
            profile::unlock(key, 8);
            profile::set_level(key, 8);
            for l in 0..8 {
                profile::award_stars(key, l, 3);
            }
        }
        139 => {
            profile::unlock(key, 8);
            for l in 0..9 {
                profile::award_stars(key, l, (l % 3) as u8 + 1);
            }
        }
        135 => {
            profile::unlock(key, 3);
            profile::award_stars(key, 0, 3);
            profile::award_stars(key, 1, 2);
            profile::award_stars(key, 2, 1);
        }
        _ => {}
    }
    let mut p = MathPopPlay::new();
    mp_settle(&mut p, view, 1.2);
    match n {
        131 | 134 => {
            mp_pop(&mut p, view, true);
            mp_settle(&mut p, view, 1.2);
            mp_pop(&mut p, view, true);
            mp_settle(&mut p, view, 1.2);
            mp_pop(&mut p, view, false);
            mp_settle(&mut p, view, 0.12);
        }
        132 => {
            mp_pop(&mut p, view, false);
            mp_settle(&mut p, view, 0.3);
            mp_pop(&mut p, view, false);
            mp_settle(&mut p, view, 0.8);
        }
        133 => {
            mp_pop(&mut p, view, true);
            mp_settle(&mut p, view, 0.35);
        }
        135 | 139 => {
            let b = kg_core::games::mathpop::levels_button(view).center();
            p.update(&Input::press_at(b), view, DT);
            mp_settle(&mut p, view, 0.5);
        }
        136 | 137 => {
            mp_clear(&mut p, view, n == 137);
            mp_settle(&mut p, view, 1.2);
        }
        138 => {
            mp_clear(&mut p, view, false);
            mp_settle(&mut p, view, 1.2);
        }
        _ => {}
    }
    kg_core::sfx::take_cues();
    Stage::Game(Box::new(p))
}

#[cfg(test)]
pub fn check() {
    for v in [View::ipad_landscape(), View::ipad_portrait()] {
        let get = |n: u32| super::game::<MathPopPlay>(setup(n, &v));
        assert!(get(130).g.teach);
        assert!(get(131).g.balloons.iter().any(|b| b.tried));
        assert!(get(132).g.revealed());
        assert!(get(133).g.solved);
        assert_eq!(get(134).g.level, 6);
        assert_eq!(get(135).g.screen, Screen::Levels);
        assert!(matches!(get(136).g.screen, Screen::Done(_)));
        assert!(matches!(get(137).g.screen, Screen::Done(_)));
        assert!(matches!(get(138).g.screen, Screen::Final(_)));
        assert_eq!(get(139).g.screen, Screen::Levels);
    }
}
