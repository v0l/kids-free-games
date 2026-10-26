use super::{Stage, DT};
use crate::games::paper::PaperPlay;
use crate::games::Play;
use kg_core::engine::{Dir, Input, View};
use kg_core::games::paper;
use kg_core::profile;

fn walk(p: &mut PaperPlay, view: &View, moves: &[(Dir, usize)]) {
    for (d, ticks) in moves {
        p.update(&Input::key(*d), view, DT);
        for _ in 0..(*ticks as f32 * paper::STEP / DT) as usize {
            p.update(&Input::default(), view, DT);
        }
    }
}

fn board(scores: &[(&str, usize, i64)]) {
    let me = profile::current_id();
    for (name, avatar, score) in scores {
        let p = profile::create(name, *avatar).unwrap();
        profile::select(&p.id);
        profile::submit(paper::GAME, *score, profile::Better::Higher);
    }
    if let Some(me) = me {
        profile::select(&me);
    }
}

pub fn setup(n: u32, view: &View) -> Stage {
    kg_core::save::reset_for_test();
    let mia = profile::create("Mia", 2).unwrap();
    profile::select(&mia.id);
    let level = match n {
        154 => 5,
        155 => 12,
        157 => 3,
        159 => 12,
        _ => 1,
    };
    if level > 1 {
        profile::unlock(paper::GAME, level);
        profile::set_level(paper::GAME, level);
    }
    if n == 156 {
        board(&[("Leo", 5, 4012), ("Ava", 9, 2008), ("Sam", 3, 1004)]);
    }
    let mut p = PaperPlay::new();
    let lap = [
        (Dir::Right, 6),
        (Dir::Down, 6),
        (Dir::Left, 6),
        (Dir::Up, 6),
    ];
    match n {
        150 => {
            for _ in 0..40 {
                p.update(&Input::default(), view, DT);
            }
        }
        151 => {
            p.update(&Input::key(Dir::Right), view, DT);
            walk(&mut p, view, &[(Dir::Right, 1)]);
        }
        152 => {
            p.update(&Input::key(Dir::Right), view, DT);
            walk(&mut p, view, &[(Dir::Right, 5), (Dir::Down, 4)]);
        }
        153 => {
            p.update(&Input::key(Dir::Right), view, DT);
            walk(&mut p, view, &lap);
            for _ in 0..8 {
                p.update(&Input::default(), view, DT);
            }
        }
        155 => {
            p.update(&Input::key(Dir::Right), view, DT);
            walk(&mut p, view, &[(Dir::Right, 3), (Dir::Down, 2)]);
        }
        154 => {
            p.update(&Input::key(Dir::Right), view, DT);
            walk(&mut p, view, &lap);
            walk(
                &mut p,
                view,
                &[(Dir::Right, 3), (Dir::Down, 10), (Dir::Right, 9)],
            );
        }
        156 => {
            p.update(&Input::key(Dir::Right), view, DT);
            p.g.players[0].alive = false;
            for _ in 0..90 {
                p.update(&Input::default(), view, DT);
            }
        }
        157 | 159 => {
            if n == 159 {
                p.g = paper::Paper::with_arena(paper::arena::MAX_LEVEL, 0);
            }
            p.update(&Input::key(Dir::Right), view, DT);
            for i in 0..p.g.owner.len() {
                if i % 7 != 0 {
                    p.g.owner[i] = Some(0);
                }
            }
            p.g.elapsed = 40.0;
            for _ in 0..70 {
                p.update(&Input::default(), view, DT);
            }
        }
        _ => {
            p.g.open_select();
            for _ in 0..40 {
                p.update(&Input::default(), view, DT);
            }
        }
    }
    kg_core::sfx::take_cues();
    Stage::Game(Box::new(p))
}

#[cfg(test)]
pub fn check() {
    use paper::State as S;
    for v in [View::ipad_landscape(), View::ipad_portrait()] {
        let get = |n: u32| super::game::<PaperPlay>(setup(n, &v));
        assert!(!get(150).g.started);
        let t = get(151);
        assert!(t.g.started && t.g.training && t.g.players[0].trail.is_empty());
        assert!(!get(152).g.players[0].trail.is_empty());
        assert!(get(153).g.grabbed);
        assert_eq!(get(154).g.arena.level, 5);
        assert_eq!(get(155).g.players.len(), 5);
        assert_eq!(get(156).g.state, S::Lost);
        assert_eq!(get(157).g.state, S::Won);
        assert_eq!(get(158).g.state, S::Select);
        assert_eq!(get(159).g.state, S::Won);
    }
}
