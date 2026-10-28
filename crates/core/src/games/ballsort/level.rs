use super::solve::{self, Tube, CAP};

pub const MAX_LEVEL: u32 = 30;
pub const COLORS: usize = 10;
pub const EMPTY_TUBES: usize = 2;
pub const HIDDEN_FROM: u32 = 17;
pub const SOLVE_LIMIT: usize = 400_000;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Spec {
    pub colors: usize,
    pub swaps: usize,
    pub hidden: bool,
}

impl Spec {
    pub fn tubes(&self) -> usize {
        self.colors + EMPTY_TUBES
    }

    pub fn balls(&self) -> usize {
        self.colors * CAP
    }
}

pub fn spec(level: u32) -> Spec {
    let level = level.clamp(1, MAX_LEVEL);
    if level < HIDDEN_FROM {
        let colors = (3 + (level as usize - 1) / 2).min(COLORS);
        let balls = colors * CAP;
        let swaps = (balls as f32 * (0.28 + 0.07 * level as f32)).round() as usize;
        Spec {
            colors,
            swaps,
            hidden: false,
        }
    } else {
        let k = (level - HIDDEN_FROM) as usize;
        let colors = (5 + k * 5 / (MAX_LEVEL - HIDDEN_FROM) as usize).min(COLORS);
        let balls = colors * CAP;
        let swaps = (balls as f32 * (1.0 + 0.04 * k as f32)).round() as usize;
        Spec {
            colors,
            swaps,
            hidden: true,
        }
    }
}

struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (self.0 >> 33) ^ (self.0 >> 13)
    }

    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
}

pub fn layout_of(level: u32, attempt: u32) -> Vec<Tube> {
    let s = spec(level);
    let mut rng = Lcg(level as u64 * 7919 + attempt as u64 * 104729 + 17);
    let mut tubes: Vec<Vec<u8>> = (0..s.tubes())
        .map(|t| {
            if t < s.colors {
                vec![t as u8; CAP]
            } else {
                Vec::new()
            }
        })
        .collect();
    let filled = s.colors;
    for _ in 0..s.swaps {
        let (a, b) = (rng.below(filled), rng.below(filled));
        if a == b {
            continue;
        }
        let (i, j) = (rng.below(CAP), rng.below(CAP));
        let x = tubes[a][i];
        tubes[a][i] = tubes[b][j];
        tubes[b][j] = x;
    }
    let mut order: Vec<usize> = (0..s.tubes()).collect();
    for i in (1..order.len()).rev() {
        order.swap(i, rng.below(i + 1));
    }
    order.iter().map(|k| solve::pack(&tubes[*k])).collect()
}

pub fn disorder(tubes: &[Tube]) -> usize {
    tubes
        .iter()
        .map(|t| {
            let balls = solve::unpack(*t);
            match balls.first() {
                Some(b) => balls.iter().filter(|c| *c != b).count(),
                None => 0,
            }
        })
        .sum()
}

pub struct Level {
    pub number: u32,
    pub tubes: Vec<Tube>,
    pub par: usize,
    pub hidden: bool,
}

pub fn make(level: u32, variant: u32) -> Level {
    let level = level.clamp(1, MAX_LEVEL);
    let s = spec(level);
    let floor = (s.colors * 2 / 3).max(2);
    let mut best: Option<Level> = None;
    for attempt in 0..40 {
        let tubes = layout_of(level, variant * 100 + attempt);
        if solve::solved(&tubes) || disorder(&tubes) < floor {
            continue;
        }
        if let Some(path) = solve::solve(&tubes, SOLVE_LIMIT) {
            best = Some(Level {
                number: level,
                tubes,
                par: path.len(),
                hidden: s.hidden,
            });
            break;
        }
    }
    best.unwrap_or_else(|| fallback(level, s))
}

fn fallback(level: u32, s: Spec) -> Level {
    let mut tubes: Vec<Tube> = (0..s.tubes())
        .map(|t| {
            if t < s.colors {
                solve::pack(&[t as u8; CAP])
            } else {
                0
            }
        })
        .collect();
    let a = solve::unpack(tubes[0]);
    let b = solve::unpack(tubes[1]);
    tubes[0] = solve::pack(&[a[0], a[1], a[2], b[0]]);
    tubes[1] = solve::pack(&[b[1], b[2], b[3], a[3]]);
    let par = solve::solve(&tubes, SOLVE_LIMIT).map_or(0, |p| p.len());
    Level {
        number: level,
        tubes,
        par,
        hidden: s.hidden,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_plan_starts_at_three_colours_and_five_tubes_and_ends_at_ten_colours() {
        let first = spec(1);
        assert_eq!((first.colors, first.tubes(), first.hidden), (3, 5, false));
        let last = spec(MAX_LEVEL);
        assert_eq!(last.colors, COLORS);
        assert!(last.hidden);
        assert!(spec(HIDDEN_FROM - 1).colors == COLORS && !spec(HIDDEN_FROM - 1).hidden);
    }

    #[test]
    fn hidden_balls_arrive_alone_after_the_colours_have_peaked() {
        assert!(spec(HIDDEN_FROM).colors < spec(HIDDEN_FROM - 1).colors);
        for l in 1..HIDDEN_FROM {
            assert!(!spec(l).hidden);
        }
        for l in HIDDEN_FROM..=MAX_LEVEL {
            assert!(spec(l).hidden);
        }
    }

    #[test]
    fn colours_and_scramble_never_fall_within_a_band() {
        for l in 2..HIDDEN_FROM {
            assert!(spec(l).colors >= spec(l - 1).colors);
            assert!(spec(l).swaps > spec(l - 1).swaps);
        }
        for l in HIDDEN_FROM + 1..=MAX_LEVEL {
            assert!(spec(l).colors >= spec(l - 1).colors);
            assert!(spec(l).swaps >= spec(l - 1).swaps);
        }
    }

    #[test]
    fn every_level_is_solvable_with_a_verified_solution() {
        for l in 1..=MAX_LEVEL {
            let lv = make(l, 0);
            let s = spec(l);
            assert_eq!(lv.tubes.len(), s.tubes(), "level {l}");
            let mut count = vec![0; COLORS];
            for t in &lv.tubes {
                assert!(solve::len(*t) <= CAP);
                for c in solve::unpack(*t) {
                    count[c as usize] += 1;
                }
            }
            for c in 0..s.colors {
                assert_eq!(count[c], CAP, "level {l} colour {c}");
            }
            assert!(!solve::solved(&lv.tubes), "level {l} starts finished");
            let path = solve::solve(&lv.tubes, SOLVE_LIMIT).unwrap_or_else(|| panic!("level {l}"));
            let mut state = lv.tubes.clone();
            for (a, b) in &path {
                assert!(solve::can_pour(state[*a as usize], state[*b as usize]));
                solve::pour(&mut state, *a as usize, *b as usize);
            }
            assert!(solve::solved(&state), "level {l}");
            assert_eq!(lv.par, path.len());
        }
    }

    #[test]
    fn a_level_is_the_same_every_time_it_is_met() {
        for l in [1, 9, 20] {
            assert_eq!(make(l, 0).tubes, make(l, 0).tubes);
            assert_ne!(make(l, 0).tubes, make(l, 1).tubes);
        }
    }

    #[test]
    fn disorder_and_solution_length_rise_from_band_to_band() {
        let band = |from: u32, to: u32| {
            let n = (to - from + 1) as f32;
            let (mut dis, mut par) = (0.0, 0.0);
            for l in from..=to {
                let lv = make(l, 0);
                dis += disorder(&lv.tubes) as f32;
                par += lv.par as f32;
            }
            (dis / n, par / n)
        };
        let a = band(1, 5);
        let b = band(6, 10);
        let c = band(11, 16);
        let d = band(17, 23);
        let e = band(24, 30);
        for (lo, hi) in [(a, b), (b, c), (d, e)] {
            assert!(hi.0 > lo.0, "disorder {lo:?} -> {hi:?}");
            assert!(hi.1 > lo.1, "par {lo:?} -> {hi:?}");
        }
    }

    #[test]
    fn the_first_level_needs_only_a_few_moves() {
        let lv = make(1, 0);
        assert!(lv.par <= 12, "{}", lv.par);
        assert!(lv.par >= 3);
    }

    #[test]
    fn replaying_a_level_thirty_times_never_repeats_the_puzzle() {
        for l in [3, 12, 24] {
            let mut seen = std::collections::HashSet::new();
            for v in 0..30 {
                let lv = make(l, v);
                let mut key = lv.tubes.clone();
                key.sort_unstable();
                assert!(seen.insert(key), "level {l} variant {v} repeats");
                assert!(solve::solve(&lv.tubes, SOLVE_LIMIT).is_some());
            }
        }
    }
}
