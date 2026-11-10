use crate::engine::View;
use crate::geom::Rect;
use crate::rng as rand;
use glam::{vec2, Vec2};

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

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum LevelsAt {
    None,
    Top,
    Goal,
}

#[derive(Clone, Copy, Debug)]
pub struct HudSpec<'a> {
    pub title: &'a str,
    pub levels: LevelsAt,
    pub stars: u32,
    pub progress: f32,
    pub goal: &'a str,
}

#[derive(Clone, Debug)]
pub struct Hud {
    pub bar: crate::sizing::TopBar,
    pub chip: Rect,
    pub title: crate::sizing::Fitted,
    pub levels: Option<Rect>,
    pub stars: Vec<Vec2>,
    pub star_r: f32,
    pub progress: Option<Rect>,
    pub goal: Option<Rect>,
    pub bottom: f32,
}

impl Hud {
    pub fn fit_goal(
        &self,
        view: &View,
        text: &str,
        full_width: bool,
    ) -> (Rect, crate::sizing::Fitted) {
        let r = self.goal.unwrap_or_default();
        let r = if full_width {
            let m = self.bar.home.x;
            Rect::new(m, r.y, view.w - 2.0 * m, r.h)
        } else {
            r
        };
        let k = ui_scale(view);
        (r, crate::sizing::fit_text(view, text, 26.0 * k, r.w, r.h))
    }
}

pub const LEVELS_LABEL: &str = "Levels";

pub fn learn_hud(view: &View, spec: &HudSpec) -> Hud {
    let k = ui_scale(view);
    let gap = view.gap().min(16.0);
    let home = view.home();
    let star_r = (20.0 * k).min(home.h * 0.32);
    let star_step = star_r * 2.2;
    let stars_w = spec.stars as f32 * star_step;
    let levels_top = (spec.levels == LevelsAt::Top)
        .then(|| crate::sizing::top_button(view, LEVELS_LABEL, 150.0));
    let pad = 18.0 * k.min(1.2);
    let fits_beside = |reserve: f32| {
        let bar = crate::sizing::top_bar(view, levels_top, reserve);
        crate::sizing::fit_size(spec.title, 28.0 * k, bar.title.w - 2.0 * pad) >= 28.0 * k * 0.8
    };
    let stars_beside = levels_top.is_none() && spec.stars > 0 && fits_beside(stars_w + gap * 0.5);
    let reserve = if stars_beside {
        stars_w + gap * 0.5
    } else {
        0.0
    };
    let bar = crate::sizing::top_bar(view, levels_top, reserve);
    let title = crate::sizing::fit_text(
        view,
        spec.title,
        28.0 * k,
        bar.title.w - 2.0 * pad,
        bar.title.h * 0.9,
    );
    let chip_w = (title.width() + 2.0 * pad).min(bar.title.w);
    let chip = Rect::new(bar.title.x, bar.title.y, chip_w, bar.title.h);
    let mut y = bar.bottom() + gap * 0.6;
    let star_y = if spec.stars > 0 && !stars_beside {
        let c = y + star_r;
        y = c + star_r + gap * 0.6;
        c
    } else {
        bar.home.center().y
    };
    let star_right = view.w - home.x - star_r;
    let stars = (0..spec.stars)
        .map(|i| vec2(star_right - (spec.stars - 1 - i) as f32 * star_step, star_y))
        .collect();
    let progress = (spec.progress > 0.0).then(|| {
        let bar_w = (view.w - 2.0 * home.x).min(520.0 * k);
        if let Some(lv) = levels_top {
            let x0 = chip.x + chip.w + gap;
            let room = lv.x - gap - x0;
            if room >= bar_w.min(300.0 * k) {
                let w = room.min(bar_w);
                let h = (spec.progress * k).min(bar.title.h);
                return Rect::new(x0 + (room - w) / 2.0, bar.title.center().y - h / 2.0, w, h);
            }
        }
        let r = Rect::new(view.w / 2.0 - bar_w / 2.0, y, bar_w, spec.progress * k);
        y = r.y + r.h + gap * 0.6;
        r
    });
    let mut levels = levels_top;
    let goal = (!spec.goal.is_empty()).then(|| {
        let goal_levels = spec.levels == LevelsAt::Goal;
        let button = goal_levels.then(|| {
            let b = crate::sizing::top_button(view, LEVELS_LABEL, 170.0);
            Rect::new(b.x, y, b.w, b.h)
        });
        let end = button.map_or(view.w - home.x, |b| b.x - gap);
        let w = end - home.x;
        let one = 26.0 * k * crate::sizing::LINE_H;
        let f = crate::sizing::fit_text(view, spec.goal, 26.0 * k, w, one * 2.0);
        let h = f.height().max(button.map_or(0.0, |b| b.h));
        let r = Rect::new(home.x, y, w, h);
        if let Some(b) = button {
            levels = Some(Rect::new(b.x, y + (h - b.h) / 2.0, b.w, b.h));
        }
        y = r.y + r.h + gap * 0.6;
        r
    });
    Hud {
        bar,
        chip,
        title,
        levels,
        stars,
        star_r,
        progress,
        goal,
        bottom: y,
    }
}

pub fn hud_problems(view: &View, what: &str, spec: &HudSpec, goals: &[&str]) -> Vec<String> {
    use crate::sizing::*;
    let h = learn_hud(view, spec);
    let mut out = vec![];
    let mut rects = vec![h.bar.home, h.chip];
    rects.extend(h.levels);
    for c in &h.stars {
        rects.push(Rect::new(
            c.x - h.star_r,
            c.y - h.star_r,
            2.0 * h.star_r,
            2.0 * h.star_r,
        ));
    }
    rects.extend(h.progress);
    rects.extend(h.goal);
    if let Err(e) = check_apart(what, &rects) {
        out.push(e);
    }
    for r in &rects {
        if let Err(e) = check_inside(view, what, *r) {
            out.push(e);
        }
    }
    let mut buttons = vec![h.bar.home];
    buttons.extend(h.levels);
    if let Err(e) = check_buttons(view, what, &buttons) {
        out.push(e);
    }
    if let Err(e) = check_fitted(&format!("{what} title"), &h.title, h.chip) {
        out.push(e);
    }
    if h.title.size < min_text(view).min(28.0) - 0.01 {
        out.push(format!("{what}: title shrank to {:.1}", h.title.size));
    }
    if h.title.lines.len() > 1
        && !spec.title.contains(':')
        && spec.title.split_whitespace().count() <= 2
    {
        out.push(format!("{what}: short title {:?} wrapped", spec.title));
    }
    for g in goals {
        for full in [false, true] {
            if full && spec.levels == LevelsAt::Goal {
                continue;
            }
            let (r, f) = h.fit_goal(view, g, full && spec.levels != LevelsAt::Goal);
            if let Err(e) = check_fitted(&format!("{what} goal"), &f, r) {
                out.push(e);
            }
        }
    }
    out
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
