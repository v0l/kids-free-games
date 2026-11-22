use super::{Stage, DT};
use crate::games::build::BuildPlay;
use crate::games::Play;
use kg_core::engine::{Input, View};
use kg_core::games::build::*;
use kg_core::profile;

fn run(p: &mut BuildPlay, view: &View, frames: usize) {
    for _ in 0..frames {
        p.update(&Input::default(), view, DT);
    }
}

fn at_level(p: &mut BuildPlay, level: usize, word: &'static str, gaps: Vec<usize>) {
    p.g.start_level(level);
    p.g.load(Question::with_gaps(level, word, gaps));
}

pub fn setup(n: u32, view: &View) -> Stage {
    kg_core::save::reset_for_test();
    let mia = profile::create("Mia", 2).unwrap();
    profile::select(&mia.id);
    if n != 290 {
        profile::set(GAME, "played", 1);
        profile::unlock(GAME, 4);
        profile::award_stars(GAME, 0, 3);
        profile::award_stars(GAME, 1, 2);
    }
    let mut p = BuildPlay::new();
    match n {
        290 => run(&mut p, view, 40),
        291 => {
            at_level(&mut p, 1, "pig", vec![2]);
            run(&mut p, view, 30);
        }
        292 => {
            at_level(&mut p, 3, "lamp", vec![0, 2]);
            let a = p.g.question.answer_at(0);
            let t = p.g.question.tiles.iter().position(|c| *c == a).unwrap();
            p.g.drop_tile(view, t);
            run(&mut p, view, 30);
        }
        293 => {
            p.g.mode = Mode::Levels;
            run(&mut p, view, 60);
        }
        294 => {
            p.g.report = Some(Report::Cleared { rank: Some(0) });
            p.g.mode = Mode::Report(0.0);
            run(&mut p, view, 110);
        }
        _ => {
            at_level(&mut p, 2, "sun", vec![1]);
            let want = p.g.question.answer_at(0);
            let wrong: Vec<usize> = (0..p.g.used.len())
                .filter(|i| p.g.question.tiles[*i] != want)
                .collect();
            p.g.drop_tile(view, wrong[0]);
            p.g.drop_tile(view, wrong[1]);
            run(&mut p, view, 30);
        }
    }
    kg_core::sfx::take_cues();
    Stage::Game(Box::new(p))
}

#[cfg(test)]
pub fn check() {
    for v in [View::ipad_landscape(), View::ipad_portrait()] {
        let get = |n: u32| super::game::<BuildPlay>(setup(n, &v));
        assert!(get(290).g.first_time);
        assert_eq!(get(291).g.question.pattern(), "pi_");
        assert_eq!(get(292).g.filled, 1);
        assert_eq!(get(293).g.mode, Mode::Levels);
        assert!(get(294).over());
        assert!(get(295).g.glow().is_some());
    }
}
