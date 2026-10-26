use crate::engine::View;
use quad_rand as rand;

pub const UP_STREAK: u32 = 4;
pub const DOWN_SLIPS: u32 = 3;
pub const PRAISE: [&str; 6] = [
    "Great!",
    "Yay!",
    "Super!",
    "Nice!",
    "Well done!",
    "Awesome!",
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    Same,
    Up,
    Down,
    Cleared,
}

#[derive(Clone, Debug)]
pub struct Ladder {
    pub level: usize,
    pub levels: usize,
    pub streak: u32,
    pub slips: u32,
    pub stars: usize,
}

impl Ladder {
    pub fn new(levels: usize) -> Self {
        Self {
            level: 0,
            levels,
            streak: 0,
            slips: 0,
            stars: 0,
        }
    }

    pub fn right(&mut self) -> Step {
        self.slips = 0;
        self.streak += 1;
        if self.streak < UP_STREAK {
            return Step::Same;
        }
        self.streak = 0;
        if self.level + 1 < self.levels {
            self.level += 1;
            self.stars = self.stars.max(self.level);
            Step::Up
        } else {
            self.stars = self.levels;
            Step::Cleared
        }
    }

    pub fn slip(&mut self) -> Step {
        self.streak = 0;
        self.slips += 1;
        if self.slips >= DOWN_SLIPS && self.level > 0 {
            self.slips = 0;
            self.level -= 1;
            Step::Down
        } else {
            Step::Same
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tap {
    Right(Step),
    Wrong,
    Reveal,
    Again,
    Ignored,
}

#[derive(Clone, Debug)]
pub struct Attempt {
    pub tried: Vec<bool>,
    pub wrong: u32,
    pub solved: bool,
}

impl Attempt {
    pub fn new(options: usize) -> Self {
        Self {
            tried: vec![false; options],
            wrong: 0,
            solved: false,
        }
    }

    pub fn revealed(&self) -> bool {
        self.wrong >= 2
    }

    pub fn choose(&mut self, slot: usize, correct: bool, ladder: &mut Ladder) -> Tap {
        if self.solved || slot >= self.tried.len() {
            return Tap::Ignored;
        }
        if correct {
            self.solved = true;
            let step = if self.wrong > 0 {
                ladder.slip()
            } else {
                ladder.right()
            };
            return Tap::Right(step);
        }
        if self.tried[slot] {
            return Tap::Again;
        }
        self.tried[slot] = true;
        self.wrong += 1;
        if self.revealed() {
            Tap::Reveal
        } else {
            Tap::Wrong
        }
    }
}

pub fn pick_distinct(pool: &mut Vec<i32>, take: usize) -> Vec<i32> {
    pool.sort_unstable();
    pool.dedup();
    let mut picks = vec![];
    while picks.len() < take && !pool.is_empty() {
        picks.push(pool.swap_remove(rand::gen_range(0, pool.len())));
    }
    picks
}

pub fn choices(correct: i32, pool: Vec<i32>, total: usize) -> Vec<i32> {
    let mut pool: Vec<i32> = pool.into_iter().filter(|v| *v != correct).collect();
    let mut options = pick_distinct(&mut pool, total.saturating_sub(1));
    options.insert(rand::gen_range(0, options.len() + 1), correct);
    options
}

pub fn ui_scale(view: &View) -> f32 {
    (view.h / view.w).clamp(1.0, 1.6)
}

pub fn tall(view: &View) -> bool {
    view.h > view.w
}

pub fn hud_bottom(view: &View) -> f32 {
    30.0 + 64.0 * ui_scale(view)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_streak_of_right_answers_climbs_one_level() {
        let mut l = Ladder::new(5);
        for _ in 0..UP_STREAK - 1 {
            assert_eq!(l.right(), Step::Same);
        }
        assert_eq!(l.right(), Step::Up);
        assert_eq!((l.level, l.streak, l.stars), (1, 0, 1));
    }

    #[test]
    fn a_miss_breaks_the_streak() {
        let mut l = Ladder::new(5);
        l.right();
        l.right();
        l.slip();
        assert_eq!(l.streak, 0);
        for _ in 0..UP_STREAK - 1 {
            assert_eq!(l.right(), Step::Same);
        }
        assert_eq!(l.level, 0);
    }

    #[test]
    fn three_misses_in_a_row_step_down_but_never_below_zero() {
        let mut l = Ladder::new(5);
        l.level = 2;
        assert_eq!(l.slip(), Step::Same);
        assert_eq!(l.slip(), Step::Same);
        assert_eq!(l.slip(), Step::Down);
        assert_eq!(l.level, 1);
        l.level = 0;
        for _ in 0..10 {
            assert_eq!(l.slip(), Step::Same);
        }
        assert_eq!(l.level, 0);
    }

    #[test]
    fn a_right_answer_forgives_earlier_misses() {
        let mut l = Ladder::new(5);
        l.level = 3;
        l.slip();
        l.slip();
        l.right();
        assert_eq!(l.slip(), Step::Same);
        assert_eq!(l.level, 3);
    }

    #[test]
    fn stars_keep_after_stepping_down_and_top_level_clears() {
        let mut l = Ladder::new(2);
        for _ in 0..UP_STREAK {
            l.right();
        }
        assert_eq!((l.level, l.stars), (1, 1));
        for _ in 0..DOWN_SLIPS {
            l.slip();
        }
        assert_eq!((l.level, l.stars), (0, 1));
        for _ in 0..UP_STREAK {
            l.right();
        }
        let mut last = Step::Same;
        for _ in 0..UP_STREAK {
            last = l.right();
        }
        assert_eq!(last, Step::Cleared);
        assert_eq!((l.level, l.stars), (1, 2));
    }

    #[test]
    fn first_wrong_tap_is_gentle_and_second_reveals() {
        let mut l = Ladder::new(5);
        let mut a = Attempt::new(4);
        assert_eq!(a.choose(0, false, &mut l), Tap::Wrong);
        assert!(!a.revealed());
        assert_eq!(a.choose(0, false, &mut l), Tap::Again);
        assert_eq!(a.wrong, 1);
        assert_eq!(a.choose(1, false, &mut l), Tap::Reveal);
        assert!(a.revealed());
    }

    #[test]
    fn solving_after_a_slip_does_not_count_as_right() {
        let mut l = Ladder::new(5);
        let mut a = Attempt::new(4);
        a.choose(0, false, &mut l);
        assert_eq!(a.choose(2, true, &mut l), Tap::Right(Step::Same));
        assert_eq!((l.streak, l.slips), (0, 1));
        assert_eq!(a.choose(3, true, &mut l), Tap::Ignored);
    }

    #[test]
    fn solving_first_try_builds_the_streak() {
        let mut l = Ladder::new(5);
        let mut a = Attempt::new(3);
        assert_eq!(a.choose(1, true, &mut l), Tap::Right(Step::Same));
        assert_eq!(l.streak, 1);
        assert_eq!(a.choose(9, false, &mut l), Tap::Ignored);
    }

    #[test]
    fn choices_hold_the_answer_exactly_once() {
        for correct in 0..30 {
            let pool: Vec<i32> = (correct - 4..=correct + 4).collect();
            let options = choices(correct, pool, 4);
            assert_eq!(options.len(), 4);
            assert_eq!(options.iter().filter(|v| **v == correct).count(), 1);
            let mut sorted = options.clone();
            sorted.sort_unstable();
            sorted.dedup();
            assert_eq!(sorted.len(), 4);
        }
    }

    #[test]
    fn pick_distinct_never_repeats() {
        for _ in 0..50 {
            let mut pool = vec![3, 3, 4, 5, 5, 6, 7];
            let picks = pick_distinct(&mut pool, 4);
            assert_eq!(picks.len(), 4);
            let mut sorted = picks.clone();
            sorted.sort_unstable();
            sorted.dedup();
            assert_eq!(sorted.len(), 4);
        }
    }
}
