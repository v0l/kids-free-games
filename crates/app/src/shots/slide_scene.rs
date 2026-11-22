use super::{Stage, DT};
use crate::games::slide::SlidePlay;
use crate::games::Play;
use kg_core::engine::{Input, View};
use kg_core::games::slide::*;
use kg_core::profile;

fn run(p: &mut SlidePlay, view: &View, frames: usize) {
    for _ in 0..frames {
        p.update(&Input::default(), view, DT);
    }
}

fn step(p: &mut SlidePlay, view: &View) {
    p.g.compute_hint();
    let Some(i) = p.g.hint else { return };
    let at = cell_rect(view, p.g.n, p.g.cell_pos(i)).center();
    p.update(&Input::press_at(at), view, DT);
    run(p, view, 30);
}

pub fn setup(n: u32, view: &View) -> Stage {
    kg_core::save::reset_for_test();
    let mia = profile::create("Mia", 2).unwrap();
    profile::select(&mia.id);
    let fresh = n == 340 || n == 347;
    if !fresh {
        profile::set(GAME, "taught", 1);
        profile::unlock(GAME, if n == 348 { MAX_LEVEL } else { 5 });
        profile::award_stars(GAME, 1, 3);
        profile::award_stars(GAME, 2, 2);
        profile::award_stars(GAME, 3, 3);
        if n == 348 {
            for l in 4..=8 {
                profile::award_stars(GAME, l, (l % 3 + 1) as u8);
            }
        }
    }
    let level = match n {
        342 => 7,
        343 => 10,
        347 => 1,
        _ => 3,
    };
    if n == 344 {
        let other = profile::create("Leo", 5).unwrap();
        profile::select(&other.id);
        profile::submit(GAME, 400, profile::Better::Higher);
        profile::select(&mia.id);
    }
    profile::set_level(GAME, level);
    let mut p = SlidePlay::new();
    p.g = Slide::with_level(level);
    run(&mut p, view, 20);
    match n {
        340 | 347 => run(&mut p, view, 40),
        341 => {
            step(&mut p, view);
            step(&mut p, view);
        }
        342 | 343 => step(&mut p, view),
        344 => {
            while p.g.state == State::Playing && p.g.moves < 200 {
                step(&mut p, view);
            }
            run(&mut p, view, (60.0 * (WIN_DELAY + 1.6)) as usize);
        }
        345 | 348 => {
            p.g.open_select();
            run(&mut p, view, 30);
        }
        346 => {
            let b = buttons(view);
            p.update(&Input::press_at(b.peek.center()), view, DT);
            run(&mut p, view, 10);
        }
        _ => {}
    }
    kg_core::sfx::take_cues();
    Stage::Game(Box::new(p))
}

#[cfg(test)]
pub fn check() {
    for v in [View::ipad_landscape(), View::ipad_portrait()] {
        let get = |n: u32| super::game::<SlidePlay>(setup(n, &v));
        assert_eq!(get(340).g.moves, 0);
        assert!(get(341).g.moves >= 1);
        assert_eq!(get(342).g.n, 4);
        assert_eq!(get(343).g.n, 5);
        let won = get(344);
        assert!(won.over(), "{:?}", won.g.state);
        assert!(won.g.rank.is_some());
        assert_eq!(won.g.stars, 3);
        assert_eq!(get(345).g.state, State::Select);
        assert!(get(346).g.peek > 0.0);
        assert_eq!(get(347).g.level, 1);
        assert_eq!(get(348).g.state, State::Select);
    }
}
