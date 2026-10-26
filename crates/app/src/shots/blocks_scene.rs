use super::{Stage, DT};
use crate::games::blocks::BlocksPlay;
use crate::games::Play;
use glam::vec2;
use kg_core::engine::{Input, View};
use kg_core::profile;

fn blocks_board(p: &mut BlocksPlay, rows: usize, gaps: &[usize]) {
    use kg_core::games::blocks::{H, W};
    for i in 0..rows {
        let y = H - 1 - i;
        for x in 0..W {
            if !gaps.contains(&x) && (i % 3 != 2 || x < 7) {
                p.g.cells[y][x] = Some(((x + i * 2) % 7) as u8);
            }
        }
    }
}

pub fn setup(n: u32, view: &View) -> Stage {
    use kg_core::games::blocks::{self as b, Piece, H, W};
    kg_core::save::reset_for_test();
    let mia = profile::create("Mia", 2).unwrap();
    if n != 121 && n != 127 {
        profile::set(b::GAME, "cleared", 1);
    }
    profile::select(&mia.id);
    let mut p = BlocksPlay::new();
    let go = Input::press_at(vec2(500.0, 500.0));
    let rest = Input::default();
    match n {
        120 => {
            for _ in 0..40 {
                p.update(&rest, view, DT);
            }
        }
        121 => {
            p.update(&go, view, DT);
            for _ in 0..60 {
                p.update(&rest, view, DT);
            }
        }
        122 => {
            p.update(&go, view, DT);
            blocks_board(&mut p, 7, &[4]);
            p.g.lines = 7;
            p.g.score = 640;
            p.g.piece = Piece { kind: 5, rot: 0, x: 3, y: 3 };
            for _ in 0..30 {
                p.update(&rest, view, DT);
            }
        }
        123 | 124 => {
            p.update(&go, view, DT);
            let tall = n == 124;
            if tall {
                p.g.lines = b::LINES_PER_LEVEL - 1;
                for i in 0..4 {
                    for x in 0..W - 1 {
                        p.g.cells[H - 1 - i][x] = Some(((x + i) % 7) as u8);
                    }
                }
                p.g.piece = Piece { kind: 0, rot: 1, x: W as i32 - 4, y: 8 };
            } else {
                blocks_board(&mut p, 3, &[]);
                for x in 0..W {
                    p.g.cells[H - 4][x] = None;
                }
                p.g.cells[H - 1] = [Some(1); W];
                p.g.cells[H - 2] = [Some(4); W];
                p.g.piece = Piece { kind: 3, rot: 0, x: 3, y: 2 };
            }
            p.g.hard_drop();
            for _ in 0..if tall { 28 } else { 14 } {
                p.update(&rest, view, DT);
            }
        }
        127 => {
            p.update(&go, view, DT);
            blocks_board(&mut p, H, &[]);
            p.g.cells[0][3..6].fill(None);
            p.g.piece = Piece { kind: 5, rot: 0, x: 3, y: -1 };
            p.g.hard_drop();
            for _ in 0..12 {
                p.update(&rest, view, DT);
            }
        }
        _ => {
            if n == 125 {
                for (name, avatar, score) in [("Leo", 5, 2300), ("Ava", 9, 1400), ("Sam", 3, 600)] {
                    let q = profile::create(name, avatar).unwrap();
                    profile::select(&q.id);
                    profile::submit(b::GAME, score, profile::Better::Higher);
                }
                profile::select(&mia.id);
                p = BlocksPlay::new();
            } else {
                let q = profile::create("Leo", 5).unwrap();
                profile::select(&q.id);
                profile::submit(b::GAME, 300, profile::Better::Higher);
                profile::select(&mia.id);
                p = BlocksPlay::new();
            }
            p.update(&go, view, DT);
            p.g.lines = 12;
            p.g.score = if n == 125 { 980 } else { 3400 };
            blocks_board(&mut p, H, &[]);
            p.g.cells[0][3..6].fill(None);
            p.g.piece = Piece { kind: 5, rot: 0, x: 3, y: -1 };
            p.g.hard_drop();
            for _ in 0..90 {
                p.update(&rest, view, DT);
            }
        }
    }
    kg_core::sfx::take_cues();
    Stage::Game(Box::new(p))
}

#[cfg(test)]
pub fn check() {
    use kg_core::games::blocks::State as S;
    for v in [View::ipad_landscape(), View::ipad_portrait()] {
        let get = |n: u32| super::game::<BlocksPlay>(setup(n, &v));
        assert_eq!(get(120).g.state, S::Ready);
        let hint = get(121);
        assert!(hint.g.state == S::Playing && hint.g.hint_text().is_some());
        assert!(get(122).g.stack_height() >= 5);
        assert!(get(123).g.clearing.is_some());
        let big = get(124);
        assert!(big.g.clearing.is_some() || big.g.lines >= 4);
        let over = get(125);
        assert!(over.g.shows_results() && over.g.rank.is_some_and(|r| r > 0));
        assert_eq!(get(126).g.rank, Some(0));
        assert!(get(127).g.rescues >= 1);
    }
}
