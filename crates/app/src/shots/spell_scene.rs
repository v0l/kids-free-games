use super::{Stage, DT};
use crate::games::spell::SpellPlay;
use crate::games::Play;
use kg_core::engine::{Input, View};
use kg_core::games::spell::{layout, Screen, KEY};
use kg_core::profile;

fn tap(p: &mut SpellPlay, view: &View, pick: impl Fn(&SpellPlay, usize) -> bool) {
    let l = layout(view, p.g.board.word.len(), p.g.board.tiles.len());
    let i = (0..p.g.board.tiles.len())
        .find(|i| p.g.board.used[*i].is_none() && pick(p, *i))
        .unwrap();
    p.update(&Input::press_at(l.tiles[i].center()), view, DT);
}

fn right(p: &SpellPlay, i: usize) -> bool {
    Some(p.g.board.tiles[i]) == p.g.board.needed()
}

fn wrong(p: &SpellPlay, i: usize) -> bool {
    !right(p, i)
}

fn settle(p: &mut SpellPlay, view: &View, frames: usize) {
    for _ in 0..frames {
        p.update(&Input::default(), view, DT);
    }
}

pub fn setup(n: u32, view: &View) -> Stage {
    kg_core::save::reset_for_test();
    let mia = profile::create("Mia", 2).unwrap();
    profile::select(&mia.id);
    let level = match n {
        161 | 162 => 2,
        165 => 3,
        167 => 8,
        _ => 0,
    };
    if n != 160 {
        profile::set(KEY, "played", 1);
    }
    let top = if n == 164 { 4 } else { level };
    for l in 0..=top {
        profile::unlock(KEY, l as u32);
        profile::award_stars(KEY, l as u32, if l % 2 == 0 { 3 } else { 2 });
    }
    profile::set_level(KEY, level as u32);
    let mut p = SpellPlay::new();
    p.g.start_level(level);
    settle(&mut p, view, 30);
    match n {
        161 => {
            tap(&mut p, view, right);
            settle(&mut p, view, 5);
            tap(&mut p, view, wrong);
            settle(&mut p, view, 4);
        }
        162 => {
            for _ in 0..3 {
                tap(&mut p, view, wrong);
                settle(&mut p, view, 20);
            }
        }
        163 => {
            for _ in 0..p.g.board.word.len() {
                tap(&mut p, view, right);
                settle(&mut p, view, 8);
            }
            settle(&mut p, view, 20);
        }
        164 => p.g.screen = Screen::Levels,
        165 => {
            p.g.slips = 1;
            p.g.screen = Screen::Done(0.0);
            settle(&mut p, view, 50);
        }
        166 => {
            p.g.rank = Some(0);
            p.g.screen = Screen::Final(0.0);
            settle(&mut p, view, 50);
        }
        _ => {}
    }
    kg_core::sfx::take_cues();
    Stage::Game(Box::new(p))
}

#[cfg(test)]
pub fn check() {
    for v in [View::ipad_landscape(), View::ipad_portrait()] {
        let get = |n: u32| super::game::<SpellPlay>(setup(n, &v));
        let first = get(160);
        assert!(first.g.teach && first.g.board.filled == 0);
        assert_eq!(get(161).g.board.filled, 1);
        assert!(get(162).g.glow().is_some());
        assert!(matches!(
            get(163).g.phase,
            kg_core::games::spell::Phase::Won(_)
        ));
        assert_eq!(get(164).g.screen, Screen::Levels);
        assert!(matches!(get(165).g.screen, Screen::Done(_)));
        assert!(matches!(get(166).g.screen, Screen::Final(_)));
        assert!(get(167).g.board.word.len() >= 6);
    }
}
