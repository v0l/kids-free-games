use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashSet};

pub const CAP: usize = 4;

pub type Tube = u16;
pub type Move = (u8, u8);

pub fn pack(colors: &[u8]) -> Tube {
    colors
        .iter()
        .enumerate()
        .fold(0, |a, (i, c)| a | ((*c as u16 + 1) << (i * 4)))
}

pub fn unpack(t: Tube) -> Vec<u8> {
    (0..len(t))
        .map(|i| ((t >> (i * 4)) & 15) as u8 - 1)
        .collect()
}

pub fn len(t: Tube) -> usize {
    (0..CAP).take_while(|i| (t >> (i * 4)) & 15 != 0).count()
}

fn top(t: Tube) -> u16 {
    match len(t) {
        0 => 0,
        n => (t >> ((n - 1) * 4)) & 15,
    }
}

fn bottom(t: Tube) -> u16 {
    t & 15
}

fn run(t: Tube) -> usize {
    let b = bottom(t);
    (0..len(t)).take_while(|i| (t >> (i * 4)) & 15 == b).count()
}

pub fn mono(t: Tube) -> bool {
    len(t) > 0 && run(t) == len(t)
}

pub fn complete(t: Tube) -> bool {
    len(t) == CAP && mono(t)
}

pub fn solved(state: &[Tube]) -> bool {
    state.iter().all(|t| len(*t) == 0 || complete(*t))
}

pub fn can_pour(from: Tube, to: Tube) -> bool {
    len(from) > 0 && len(to) < CAP && (len(to) == 0 || top(to) == top(from))
}

pub fn pour(state: &mut [Tube], from: usize, to: usize) {
    let n = len(state[from]);
    let ball = top(state[from]);
    state[from] &= !(15 << ((n - 1) * 4));
    state[to] |= ball << (len(state[to]) * 4);
}

pub fn legal_moves(state: &[Tube]) -> Vec<Move> {
    let mut out = Vec::new();
    for (a, ta) in state.iter().enumerate() {
        if len(*ta) == 0 {
            continue;
        }
        let mut empty_used = false;
        for (b, tb) in state.iter().enumerate() {
            if a == b || !can_pour(*ta, *tb) {
                continue;
            }
            if len(*tb) == 0 {
                if mono(*ta) || empty_used {
                    continue;
                }
                empty_used = true;
            }
            out.push((a as u8, b as u8));
        }
    }
    out
}

pub fn any_move(state: &[Tube]) -> bool {
    state.iter().enumerate().any(|(a, ta)| {
        state
            .iter()
            .enumerate()
            .any(|(b, tb)| a != b && can_pour(*ta, *tb))
    })
}

fn disorder(state: &[Tube]) -> usize {
    state
        .iter()
        .map(|t| len(*t) - if len(*t) == 0 { 0 } else { run(*t) })
        .sum()
}

fn key(state: &[Tube]) -> Vec<Tube> {
    let mut k = state.to_vec();
    k.sort_unstable();
    k
}

struct Node {
    state: Vec<Tube>,
    parent: usize,
    mv: Move,
    g: usize,
}

pub fn solve(start: &[Tube], limit: usize) -> Option<Vec<Move>> {
    if solved(start) {
        return Some(Vec::new());
    }
    let mut nodes = vec![Node {
        state: start.to_vec(),
        parent: usize::MAX,
        mv: (0, 0),
        g: 0,
    }];
    let mut seen: HashSet<Vec<Tube>> = HashSet::new();
    seen.insert(key(start));
    let mut open = BinaryHeap::new();
    open.push(Reverse((disorder(start) * 4, 0usize)));
    while let Some(Reverse((_, at))) = open.pop() {
        if nodes.len() > limit {
            return None;
        }
        let state = nodes[at].state.clone();
        let g = nodes[at].g;
        for (a, b) in legal_moves(&state) {
            let mut next = state.clone();
            pour(&mut next, a as usize, b as usize);
            if !seen.insert(key(&next)) {
                continue;
            }
            let done = solved(&next);
            let prio = disorder(&next) * 4 + g + 1;
            nodes.push(Node {
                state: next,
                parent: at,
                mv: (a, b),
                g: g + 1,
            });
            let id = nodes.len() - 1;
            if done {
                let mut path = Vec::new();
                let mut n = id;
                while nodes[n].parent != usize::MAX {
                    path.push(nodes[n].mv);
                    n = nodes[n].parent;
                }
                path.reverse();
                return Some(path);
            }
            open.push(Reverse((prio, id)));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn packing_round_trips_and_measures() {
        let t = pack(&[3, 3, 5]);
        assert_eq!(unpack(t), vec![3, 3, 5]);
        assert_eq!(len(t), 3);
        assert!(!mono(t));
        assert!(mono(pack(&[2, 2])));
        assert!(complete(pack(&[1, 1, 1, 1])));
        assert_eq!(len(0), 0);
    }

    #[test]
    fn pouring_moves_the_top_ball_only_onto_a_match_or_an_empty_tube() {
        let mut s = vec![pack(&[0, 1]), pack(&[1]), pack(&[]), pack(&[0, 1, 0, 1])];
        assert!(can_pour(s[0], s[1]));
        assert!(can_pour(s[0], s[2]));
        assert!(!can_pour(s[0], s[3]));
        assert!(!can_pour(s[2], s[0]));
        pour(&mut s, 0, 1);
        assert_eq!(unpack(s[0]), vec![0]);
        assert_eq!(unpack(s[1]), vec![1, 1]);
    }

    #[test]
    fn the_solver_finishes_a_small_puzzle_with_legal_moves() {
        let start = vec![
            pack(&[0, 1, 0, 1]),
            pack(&[1, 0, 1, 0]),
            pack(&[]),
            pack(&[]),
        ];
        let path = solve(&start, 50_000).unwrap();
        let mut s = start.clone();
        for (a, b) in &path {
            assert!(can_pour(s[*a as usize], s[*b as usize]));
            pour(&mut s, *a as usize, *b as usize);
        }
        assert!(solved(&s));
    }

    #[test]
    fn an_unsolvable_position_is_reported_as_such() {
        let start = vec![pack(&[0, 1, 0, 1]), pack(&[1, 0, 1, 0])];
        assert!(solve(&start, 50_000).is_none());
    }

    #[test]
    fn a_solved_position_needs_no_moves() {
        let s = vec![pack(&[0, 0, 0, 0]), pack(&[])];
        assert_eq!(solve(&s, 10).unwrap().len(), 0);
    }
}
