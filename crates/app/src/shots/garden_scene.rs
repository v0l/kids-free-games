use super::{Stage, DT};
use crate::games::garden::GardenPlay;
use crate::games::Play;
use kg_core::engine::{Input, View};
use kg_core::games::garden::{self, *};
use kg_core::profile;

fn run(p: &mut GardenPlay, view: &View, frames: usize) {
    for _ in 0..frames {
        p.update(&Input::default(), view, DT);
    }
}

fn pick(p: &mut GardenPlay, view: &View, right: bool) {
    let want = p.g.q.slot_of_answer();
    let slot = if right {
        want
    } else {
        (0..OPTIONS).find(|i| *i != want && !p.g.tried[*i]).unwrap()
    };
    p.update(&Input::press_at(seed_rects(view)[slot].center()), view, DT);
}

fn fact(p: &mut GardenPlay, level: usize, stage: usize, a: i32, b: i32) {
    p.g.start_level(level);
    p.g.stage = stage;
    p.g.q = Question::new(Fact { table: a, other: b }, level, stage, false, false);
    p.g.cam_rows = a as f32;
    p.g.cam_cols = b as f32;
}

fn veteran(beds: usize) {
    profile::set(GAME, "played", 1);
    profile::unlock(GAME, beds as u32);
    for l in 0..beds {
        profile::set(GAME, &format!("bed.{l}"), 1);
        profile::award_stars(GAME, l as u32, 3 - (l % 2) as u8);
    }
}

pub fn setup(n: u32, view: &View) -> Stage {
    kg_core::save::reset_for_test();
    let mia = profile::create("Mia", 2).unwrap();
    profile::select(&mia.id);
    if !matches!(n, 260 | 261) {
        veteran(if n == 267 { 11 } else { 4 });
    }
    profile::set_level(garden::GAME, if n == 267 { 11 } else { 3 });
    let mut p = GardenPlay::new();
    match n {
        260 => run(&mut p, view, 70),
        261 => {
            fact(&mut p, 0, 0, 3, 4);
            p.g.grow_age = 0.35;
            p.g.shown = 1.0;
            run(&mut p, view, 40);
        }
        262 => {
            fact(&mut p, 3, 0, 4, 6);
            run(&mut p, view, 70);
        }
        263 => {
            fact(&mut p, 3, 0, 4, 6);
            run(&mut p, view, 40);
            pick(&mut p, view, true);
            run(&mut p, view, 20);
        }
        264 => {
            fact(&mut p, 5, 1, 8, 7);
            run(&mut p, view, 20);
            p.g.ask_t = 10.0;
            run(&mut p, view, 60);
        }
        265 => {
            fact(&mut p, 6, 2, 6, 9);
            run(&mut p, view, 40);
            pick(&mut p, view, false);
            run(&mut p, view, 40);
        }
        266 => {
            p.g.mode = Mode::Beds;
            run(&mut p, view, 80);
        }
        267 => {
            p.g.report = Some(Report::Cleared { rank: Some(0) });
            p.g.mode = Mode::Report(0.0);
            run(&mut p, view, 110);
        }
        268 => {
            fact(&mut p, 2, 2, 5, 9);
            p.g.run = 9;
            p.g.stage_run = 9;
            run(&mut p, view, 40);
            pick(&mut p, view, true);
            run(&mut p, view, 20);
        }
        _ => {
            p.g.report = Some(Report::Round { stars: 2 });
            p.g.summary = "7 of 8 right first time".into();
            p.g.mode = Mode::Report(0.0);
            run(&mut p, view, 60);
        }
    }
    kg_core::sfx::take_cues();
    Stage::Game(Box::new(p))
}

#[cfg(test)]
pub fn check() {
    for v in [View::ipad_landscape(), View::ipad_portrait()] {
        let get = |n: u32| super::game::<GardenPlay>(setup(n, &v));
        assert!(get(260).g.first_time);
        assert_eq!(get(261).g.q.rows() * get(261).g.q.cols(), 12);
        assert_eq!(get(262).g.stage, 0);
        assert!(get(263).g.solved.is_some());
        assert_eq!(get(264).g.stage, 1);
        assert_eq!(get(265).g.wrong, 1);
        assert_eq!(get(266).g.mode, Mode::Beds);
        assert!(get(267).over());
        assert!(get(268).g.solved.is_some());
        assert!(matches!(get(269).g.mode, Mode::Report(_)));
    }
}
