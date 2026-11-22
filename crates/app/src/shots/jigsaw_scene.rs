use super::{Stage, DT};
use crate::games::jigsaw::JigsawPlay;
use crate::games::Play;
use kg_core::engine::{Input, View};
use kg_core::games::jigsaw::{self, layout, Mode, Report};
use kg_core::profile;

fn run(p: &mut JigsawPlay, view: &View, frames: usize) {
    for _ in 0..frames {
        p.update(&Input::default(), view, DT);
    }
}

fn place(p: &mut JigsawPlay, view: &View, count: usize) {
    let lay = layout(view, p.g.level);
    for i in 0..count.min(p.g.pieces.len()) {
        let t = lay.cell_center(p.g.pieces[i].col, p.g.pieces[i].row);
        let from = p.g.pieces[i].pos;
        p.update(&Input::press_at(from), view, DT);
        let mut mv = Input::press_at(t);
        mv.pressed = false;
        p.update(&mv, view, DT);
        let mut up = Input::default();
        up.pos = t;
        up.released = true;
        p.update(&up, view, DT);
    }
    run(p, view, 30);
}

pub fn setup(n: u32, view: &View) -> Stage {
    kg_core::save::reset_for_test();
    let mia = profile::create("Mia", 2).unwrap();
    profile::select(&mia.id);
    let veteran = n != 320;
    if veteran {
        profile::set(jigsaw::GAME, "played", 1);
        profile::unlock(jigsaw::GAME, 4);
        profile::award_stars(jigsaw::GAME, 0, 3);
        profile::award_stars(jigsaw::GAME, 1, 2);
        profile::award_stars(jigsaw::GAME, 2, 3);
    }
    match n {
        323 => profile::set_level(jigsaw::GAME, 4),
        324 => {
            profile::unlock(jigsaw::GAME, 7);
            profile::set_level(jigsaw::GAME, 7);
        }
        _ => {}
    }
    let mut p = JigsawPlay::new();
    run(&mut p, view, 60);
    match n {
        321 | 323 | 324 => place(&mut p, view, 3),
        322 => {
            p.g.mode = Mode::Levels;
            run(&mut p, view, 30);
        }
        325 => {
            p.g.report = Some(Report::Level { level: 2, stars: 2 });
            p.g.mode = Mode::Report(0.0);
            run(&mut p, view, 50);
        }
        326 => {
            let rank = profile::submit(jigsaw::GAME, 20, profile::Better::Higher);
            p.g.report = Some(Report::Cleared { rank });
            p.g.mode = Mode::Report(0.0);
            run(&mut p, view, 80);
        }
        _ => {}
    }
    kg_core::sfx::take_cues();
    Stage::Game(Box::new(p))
}

#[cfg(test)]
pub fn check() {
    for v in [View::ipad_landscape(), View::ipad_portrait()] {
        let get = |n: u32| super::game::<JigsawPlay>(setup(n, &v));
        assert!(get(320).g.first_time);
        assert_eq!(get(321).g.placed_count(), 3);
        assert_eq!(get(322).g.mode, Mode::Levels);
        assert_eq!(get(323).g.level, 4);
        assert_eq!(get(324).g.level, 7);
        assert!(matches!(get(325).g.mode, Mode::Report(_)));
        assert!(matches!(get(326).g.report, Some(Report::Cleared { .. })));
    }
}
