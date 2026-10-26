use super::{Stage, DT};
use crate::games::memory::MemoryPlay;
use crate::games::Play;
use kg_core::engine::{Input, View};
use kg_core::games::memory::{Memory, KEY};
use kg_core::profile;

fn memory_pair(m: &Memory, k: usize) -> (usize, usize) {
    let mut ids: Vec<u8> = m.cards.iter().map(|c| c.0).collect();
    ids.sort();
    ids.dedup();
    let id = ids[k % ids.len()];
    let at: Vec<usize> = (0..m.cards.len()).filter(|i| m.cards[*i].0 == id).collect();
    (at[0], at[1])
}

fn memory_match(p: &mut MemoryPlay, pairs: usize) {
    for k in 0..pairs {
        let (a, b) = memory_pair(&p.g, k);
        p.g.pick(a);
        p.g.pick(b);
    }
}

fn memory_miss(p: &mut MemoryPlay, first: usize, second: usize) {
    let (a, _) = memory_pair(&p.g, first);
    let (b, _) = memory_pair(&p.g, second);
    p.g.pick(a);
    p.g.pick(b);
}

fn memory_wait(p: &mut MemoryPlay, view: &View, frames: usize) {
    for _ in 0..frames {
        p.update(&Input::default(), view, DT);
    }
}

pub fn setup(n: u32, view: &View) -> Stage {
    use kg_core::games::memory::MAX_LEVEL;
    kg_core::save::reset_for_test();
    let mia = profile::create("Mia", 2).unwrap();
    profile::select(&mia.id);
    let reach = |upto: u32| {
        for l in 1..upto {
            let mut m = Memory::for_level(l);
            for k in 0..m.pairs {
                let (a, b) = memory_pair(&m, k);
                m.pick(a);
                m.pick(b);
            }
        }
    };
    let mut p;
    match n {
        90 => {
            p = MemoryPlay::new();
            memory_wait(&mut p, view, 30);
        }
        91 => {
            reach(3);
            p = MemoryPlay::new();
            p.g.begin(3);
            memory_match(&mut p, 2);
            let (a, _) = memory_pair(&p.g, 3);
            p.g.pick(a);
            memory_wait(&mut p, view, 40);
        }
        92 => {
            reach(3);
            p = MemoryPlay::new();
            p.g.begin(3);
            memory_match(&mut p, 1);
            memory_miss(&mut p, 2, 3);
            memory_wait(&mut p, view, 14);
        }
        93 => {
            reach(2);
            p = MemoryPlay::new();
            p.g.begin(2);
            for _ in 0..3 {
                memory_miss(&mut p, 0, 1);
                p.g.tick(p.g.peek + 0.01);
            }
            let (a, _) = memory_pair(&p.g, 0);
            p.g.pick(a);
            memory_wait(&mut p, view, 40);
        }
        94 | 95 => {
            reach(4);
            p = MemoryPlay::new();
            let tiles = Memory::tiles(view);
            if n == 95 {
                p.update(&Input::press_at(tiles[4].center()), view, DT);
            }
            memory_wait(&mut p, view, 12);
        }
        96 | 97 => {
            reach(2);
            p = MemoryPlay::new();
            p.g.begin(2);
            if n == 97 {
                for k in 0..9 {
                    memory_miss(&mut p, k, k + 1);
                    p.g.tick(p.g.peek + 0.01);
                }
            }
            memory_match(&mut p, 4);
            memory_wait(&mut p, view, 130);
        }
        _ => {
            reach(MAX_LEVEL);
            p = MemoryPlay::new();
            p.g.begin(MAX_LEVEL);
            if n == 98 {
                memory_match(&mut p, 4);
                memory_miss(&mut p, 5, 6);
                memory_wait(&mut p, view, 20);
            } else {
                let me = profile::current_id();
                for (name, avatar, moves) in [("Leo", 5, 10), ("Ava", 9, 16), ("Sam", 3, 22)] {
                    let q = profile::create(name, avatar).unwrap();
                    profile::select(&q.id);
                    profile::submit(KEY, moves, profile::Better::Lower);
                }
                if let Some(me) = me {
                    profile::select(&me);
                }
                p = MemoryPlay::new();
                p.g.begin(MAX_LEVEL);
                for k in 0..3 {
                    memory_miss(&mut p, k, k + 1);
                    p.g.tick(p.g.peek + 0.01);
                }
                memory_match(&mut p, 10);
                memory_wait(&mut p, view, 150);
            }
        }
    }
    kg_core::sfx::take_cues();
    Stage::Game(Box::new(p))
}

#[cfg(test)]
pub fn check() {
    for v in [View::ipad_landscape(), View::ipad_portrait()] {
        use kg_core::games::memory::{Screen, MAX_LEVEL};
        let get = |n: u32| super::game::<MemoryPlay>(setup(n, &v));
        let first = get(90);
        assert!(first.g.teach && first.g.level == 1);
        assert_eq!(get(91).g.matched_pairs(), 2);
        assert!(get(92).g.wait > 0.0);
        assert!(get(93).g.helper().is_some());
        assert_eq!(get(94).g.screen, Screen::Select);
        assert!(!get(95).g.popups.list.is_empty());
        let clear = get(96);
        assert!(clear.g.won() && clear.g.stars == 3 && clear.g.celebrated >= 1.5);
        assert_eq!(get(97).g.stars, 1);
        let last = get(98);
        assert!(last.g.level == MAX_LEVEL && !last.g.won());
        let done = get(99);
        assert!(done.g.won() && done.g.rank.is_some_and(|r| r > 0));
    }
}
