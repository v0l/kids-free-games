use crate::engine::*;
use crate::feel::{self, Impact};
use crate::fx::*;
use crate::geom::*;
use crate::learn::*;
use crate::rng as rand;
use crate::sfx::{self, Sfx};
use glam::{vec2, Vec2};

pub const GAME: &str = "pattern";
pub const LEVELS: usize = 8;
pub const COLOURS: [&str; 5] = ["red", "blue", "yellow", "green", "purple"];
pub const SHAPES: [&str; 4] = ["star", "heart", "moon", "diamond"];
pub const HOP_SECS: f32 = 0.22;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Token {
    Colour(u8),
    Shape(u8),
    Number(i32),
    Pair(u8, u8),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Family {
    Colours,
    Shapes,
    Numbers,
    Pairs,
}

#[derive(Clone, Copy, Debug)]
pub struct Spec {
    pub name: &'static str,
    pub family: Family,
    pub units: &'static [&'static [usize]],
    pub steps: &'static [i32],
    pub pool: usize,
    pub options: usize,
}

pub const SPECS: [Spec; LEVELS] = [
    Spec {
        name: "Red and blue",
        family: Family::Colours,
        units: &[&[0, 1]],
        steps: &[],
        pool: 3,
        options: 2,
    },
    Spec {
        name: "Two and one",
        family: Family::Colours,
        units: &[&[0, 0, 1], &[0, 1, 1]],
        steps: &[],
        pool: 4,
        options: 3,
    },
    Spec {
        name: "Three colours",
        family: Family::Colours,
        units: &[&[0, 1, 2]],
        steps: &[],
        pool: 5,
        options: 3,
    },
    Spec {
        name: "Shapes",
        family: Family::Shapes,
        units: &[&[0, 1], &[0, 0, 1], &[0, 1, 2]],
        steps: &[],
        pool: 4,
        options: 3,
    },
    Spec {
        name: "Count on",
        family: Family::Numbers,
        units: &[],
        steps: &[1],
        pool: 0,
        options: 3,
    },
    Spec {
        name: "Count by 2s",
        family: Family::Numbers,
        units: &[],
        steps: &[2],
        pool: 0,
        options: 3,
    },
    Spec {
        name: "Count by 5s",
        family: Family::Numbers,
        units: &[],
        steps: &[5, 10, 3],
        pool: 0,
        options: 4,
    },
    Spec {
        name: "Shape and colour",
        family: Family::Pairs,
        units: &[&[0, 1], &[0, 0, 1], &[0, 1, 2]],
        steps: &[],
        pool: 4,
        options: 4,
    },
];

pub fn level_name(level: usize) -> String {
    SPECS[level.min(LEVELS - 1)].name.to_string()
}

pub fn choice_count(level: usize) -> usize {
    SPECS[level.min(LEVELS - 1)].options
}

pub fn target_secs(level: usize) -> f32 {
    6.0 + level.min(LEVELS - 1) as f32 * 0.6
}

pub fn stars_score(s: i64) -> String {
    format!("{s} stars")
}

fn shuffled<T>(mut v: Vec<T>) -> Vec<T> {
    for i in (1..v.len()).rev() {
        v.swap(i, rand::gen_range(0, i + 1));
    }
    v
}

#[derive(Clone, Debug)]
pub struct Question {
    pub level: usize,
    pub seq: Vec<Token>,
    pub period: usize,
    pub options: Vec<Token>,
}

impl Question {
    pub fn generate(level: usize) -> Self {
        let spec = SPECS[level];
        match spec.family {
            Family::Numbers => Self::numbers(level, spec),
            _ => Self::symbols(level, spec),
        }
    }

    fn numbers(level: usize, spec: Spec) -> Self {
        let step = spec.steps[rand::gen_range(0, spec.steps.len())];
        let start = match step {
            1 => rand::gen_range(1, 10),
            2 => 2 * rand::gen_range(1, 6),
            _ => step * rand::gen_range(1, 4),
        };
        let len = if level >= 6 { 6 } else { 5 };
        let seq: Vec<Token> = (0..len)
            .map(|i| Token::Number(start + step * i as i32))
            .collect();
        let answer = start + step * (len as i32 - 1);
        let pool: Vec<i32> = [step, 1, 2, step * 2, 3]
            .iter()
            .flat_map(|d| [answer - d, answer + d])
            .filter(|v| *v > 0 && *v != answer)
            .collect();
        let mut pool = pool;
        let options = choices(answer, pool.drain(..).collect(), spec.options)
            .into_iter()
            .map(Token::Number)
            .collect();
        Self {
            level,
            seq,
            period: 0,
            options,
        }
    }

    fn symbols(level: usize, spec: Spec) -> Self {
        let unit = spec.units[rand::gen_range(0, spec.units.len())];
        let kinds = unit.iter().max().map_or(1, |m| m + 1);
        let period = unit.len();
        let extra = rand::gen_range(1, period + 1);
        let len = period * 2 + extra;
        let (symbols, everything) = Self::alphabet(spec, kinds);
        let seq: Vec<Token> = (0..len).map(|i| symbols[unit[i % period]]).collect();
        let answer = seq[len - 1];
        let mut near: Vec<Token> = symbols.iter().copied().filter(|t| *t != answer).collect();
        near = shuffled(near);
        let mut far: Vec<Token> = everything
            .into_iter()
            .filter(|t| *t != answer && !near.contains(t))
            .collect();
        far = shuffled(far);
        let mut options: Vec<Token> = near
            .into_iter()
            .chain(far)
            .take(spec.options.saturating_sub(1))
            .collect();
        options.insert(rand::gen_range(0, options.len() + 1), answer);
        Self {
            level,
            seq,
            period,
            options,
        }
    }

    fn alphabet(spec: Spec, kinds: usize) -> (Vec<Token>, Vec<Token>) {
        match spec.family {
            Family::Colours => {
                let order = shuffled((0..COLOURS.len() as u8).collect::<Vec<_>>());
                let all: Vec<Token> = order
                    .iter()
                    .take(spec.pool)
                    .map(|c| Token::Colour(*c))
                    .collect();
                (all[..kinds].to_vec(), all)
            }
            Family::Shapes => {
                let order = shuffled((0..SHAPES.len() as u8).collect::<Vec<_>>());
                let all: Vec<Token> = order
                    .iter()
                    .take(spec.pool)
                    .map(|c| Token::Shape(*c))
                    .collect();
                (all[..kinds].to_vec(), all)
            }
            _ => {
                let colours = shuffled((0..COLOURS.len() as u8).collect::<Vec<_>>());
                let shapes = shuffled((0..SHAPES.len() as u8).collect::<Vec<_>>());
                let symbols: Vec<Token> = (0..kinds)
                    .map(|i| Token::Pair(colours[i], shapes[i]))
                    .collect();
                let mut all = symbols.clone();
                for s in &symbols {
                    if let Token::Pair(c, sh) = *s {
                        all.push(Token::Pair(c, shapes[kinds]));
                        all.push(Token::Pair(colours[kinds], sh));
                    }
                }
                (symbols, all)
            }
        }
    }

    pub fn fresh(level: usize, previous: Option<&Question>) -> Self {
        let mut q = Self::generate(level);
        for _ in 0..12 {
            if previous.is_none_or(|p| p.seq != q.seq) {
                break;
            }
            q = Self::generate(level);
        }
        q
    }

    pub fn shown(&self) -> &[Token] {
        &self.seq[..self.seq.len() - 1]
    }

    pub fn answer(&self) -> Token {
        self.seq[self.seq.len() - 1]
    }

    pub fn slot_of_answer(&self) -> usize {
        self.options
            .iter()
            .position(|t| *t == self.answer())
            .unwrap_or(0)
    }
}

pub fn prompt_size(view: &View) -> f32 {
    62.0 * ui_scale(view)
}

pub const HUD_TITLE: &str = "Shape and colour";
pub const HUD_GOAL: &str = "Get 4 right in a row to reach Shape and colour";

pub fn hud(view: &View) -> crate::learn::Hud {
    crate::learn::learn_hud(
        view,
        &crate::learn::HudSpec {
            title: HUD_TITLE,
            levels: crate::learn::LevelsAt::Goal,
            stars: LEVELS as u32,
            progress: 18.0,
            goal: HUD_GOAL,
        },
    )
}

pub fn prompt_y(view: &View) -> f32 {
    hud(view).bottom + prompt_size(view) * 0.7
}

pub fn levels_button(view: &View) -> Rect {
    hud(view).levels.unwrap_or_default()
}

pub fn level_tiles(view: &View) -> Vec<Rect> {
    let n = LEVELS;
    let cols = if tall(view) { 2 } else { 4 };
    let rows = n.div_ceil(cols);
    let gap = 24.0;
    let top = 150.0;
    let bottom = view.h - 190.0;
    let w = ((view.w - 80.0 - gap * (cols as f32 - 1.0)) / cols as f32).min(300.0);
    let h = ((bottom - top - gap * (rows as f32 - 1.0)) / rows as f32).min(230.0);
    let left = (view.w - w * cols as f32 - gap * (cols as f32 - 1.0)) / 2.0;
    let up = top + (bottom - top - h * rows as f32 - gap * (rows as f32 - 1.0)) / 2.0;
    (0..n)
        .map(|i| {
            Rect::new(
                left + (i % cols) as f32 * (w + gap),
                up + (i / cols) as f32 * (h + gap),
                w,
                h,
            )
        })
        .collect()
}

pub fn play_button(view: &View) -> Rect {
    crate::sizing::bottom_button(view, "Play", 340.0, 100.0)
}

pub fn button_height(view: &View) -> f32 {
    view.kid_tap(124.0 * ui_scale(view))
}

pub fn path_area(view: &View) -> Rect {
    let top = prompt_y(view) + prompt_size(view) * 0.7;
    let bottom = view.h - button_height(view) - 80.0;
    Rect::new(40.0, top, view.w - 80.0, bottom - top)
}

pub fn hint_pos(view: &View, n: usize) -> Vec2 {
    let top = button_rects(view, n)[0].y;
    let half = 32.0 * view.text_k() * 1.05;
    vec2(view.w / 2.0, top - view.gap() - half)
}

pub fn button_rects(view: &View, n: usize) -> Vec<Rect> {
    let gap = 24.0;
    let h = button_height(view);
    let w = ((view.w - 80.0 - gap * (n as f32 - 1.0)) / n as f32).min(240.0 * ui_scale(view));
    let left = (view.w - w * n as f32 - gap * (n as f32 - 1.0)) / 2.0;
    (0..n)
        .map(|i| Rect::new(left + i as f32 * (w + gap), view.h - h - 40.0, w, h))
        .collect()
}

pub fn stone_cells(view: &View, n: usize) -> Vec<Rect> {
    let area = path_area(view);
    let gap = 14.0;
    let max = 170.0 * ui_scale(view);
    let one_row = ((area.w - gap * (n as f32 - 1.0)) / n as f32).min(max);
    let (rows, cols) = if one_row >= 90.0 * ui_scale(view).min(1.2) || n < 2 {
        (1, n)
    } else {
        (2, n.div_ceil(2))
    };
    let size = ((area.w - gap * (cols as f32 - 1.0)) / cols as f32)
        .min((area.h - gap * (rows as f32 - 1.0)) / rows as f32)
        .min(max);
    let top = area.y + (area.h - size * rows as f32 - gap * (rows as f32 - 1.0)) / 2.0;
    (0..n)
        .map(|i| {
            let (r, c) = (i / cols, i % cols);
            let in_row = if r + 1 == rows { n - r * cols } else { cols };
            let left = area.x + (area.w - size * in_row as f32 - gap * (in_row as f32 - 1.0)) / 2.0;
            Rect::new(
                left + c as f32 * (size + gap),
                top + r as f32 * (size + gap),
                size,
                size,
            )
        })
        .collect()
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Mode {
    Play,
    Levels,
    Report(f32),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Report {
    Level { level: usize, stars: u8 },
    Cleared { rank: Option<usize> },
}

pub struct Pattern {
    pub mode: Mode,
    pub misses: u32,
    pub first_time: bool,
    pub report: Option<Report>,
    pub ladder: Ladder,
    pub question: Question,
    pub attempt: Attempt,
    pub wobble: Vec<f32>,
    pub press: Vec<Bump>,
    pub celebrate: Option<f32>,
    pub streak_bump: Bump,
    pub prompt_bump: Bump,
    pub shake: Shake,
    pub particles: Particles,
    pub popups: Popups,
    pub t: f32,
    pub session: crate::mastery::Session,
    pub summary: String,
}

impl Pattern {
    pub fn new() -> Self {
        let top = LEVELS - 1;
        let unlocked = (crate::profile::unlocked(GAME) as usize).min(top);
        let level = (crate::profile::level(GAME) as usize).min(unlocked);
        let cleared = crate::profile::get(GAME, "cleared") > 0;
        let mut ladder = Ladder::new(LEVELS);
        ladder.level = level;
        ladder.stars = if cleared { LEVELS } else { unlocked };
        let question = Question::fresh(level, None);
        let mut g = Self {
            mode: Mode::Play,
            misses: 0,
            first_time: crate::profile::get(GAME, "played") == 0,
            report: None,
            ladder,
            attempt: Attempt::new(0),
            wobble: vec![],
            press: vec![],
            celebrate: None,
            question,
            streak_bump: Bump::default(),
            prompt_bump: Bump::default(),
            shake: Shake::default(),
            particles: Particles::default(),
            popups: Popups::default(),
            t: 0.0,
            session: crate::mastery::Session::default(),
            summary: String::new(),
        };
        g.load();
        g
    }

    pub fn start_level(&mut self, level: usize) {
        self.ladder.level = level.min(LEVELS - 1);
        self.ladder.streak = 0;
        self.ladder.slips = 0;
        self.misses = 0;
        self.session.reset();
        crate::profile::set_level(GAME, self.ladder.level as u32);
        self.mode = Mode::Play;
        self.next_question();
    }

    pub fn unlocked_level(&self) -> usize {
        (crate::profile::unlocked(GAME) as usize).min(LEVELS - 1)
    }

    fn finish_level(&mut self, level: usize) -> u8 {
        let stars = self.session.stars(target_secs(level));
        crate::profile::award_stars(GAME, level as u32, stars);
        self.summary = self.session.summary();
        self.session.reset();
        self.misses = 0;
        stars
    }

    fn record(&mut self, step: Step) {
        if self.attempt.wrong > 0 {
            self.misses += 1;
        }
        self.first_time = false;
        crate::profile::set(GAME, "played", 1);
        match step {
            Step::Up => {
                let level = self.ladder.level - 1;
                let stars = self.finish_level(level);
                crate::profile::unlock(GAME, self.ladder.level as u32);
                crate::profile::set_level(GAME, self.ladder.level as u32);
                self.report = Some(Report::Level { level, stars });
            }
            Step::Cleared => {
                let level = self.ladder.level;
                let stars = self.finish_level(level);
                if crate::profile::get(GAME, "cleared") == 0 {
                    crate::profile::set(GAME, "cleared", 1);
                    let total = crate::profile::total_stars(GAME) as i64;
                    let rank = crate::profile::submit(GAME, total, crate::profile::Better::Higher);
                    self.report = Some(Report::Cleared { rank });
                } else {
                    self.report = Some(Report::Level { level, stars });
                }
            }
            Step::Down => {
                self.misses = 0;
                self.session.reset();
                crate::profile::set_level(GAME, self.ladder.level as u32);
            }
            Step::Same => {}
        }
    }

    pub fn load(&mut self) {
        let options = self.question.options.len();
        self.attempt = Attempt::new(options);
        self.wobble = vec![0.0; options];
        self.press = (0..options).map(|_| Bump::default()).collect();
        self.celebrate = None;
        self.prompt_bump.kick();
        self.t = 0.0;
        self.session.start_question();
    }

    fn next_question(&mut self) {
        self.question = Question::fresh(self.ladder.level, Some(&self.question));
        self.load();
    }

    pub fn pick(&mut self, slot: usize) -> Tap {
        if self.celebrate.is_some() || slot >= self.question.options.len() {
            return Tap::Ignored;
        }
        let correct = self.question.options[slot] == self.question.answer();
        let level = self.ladder.level;
        let result = self.attempt.choose(slot, correct, &mut self.ladder);
        if let Tap::Right(step) = result {
            let answer = self.session.answer(self.attempt.wrong == 0, false);
            crate::mastery::record(GAME, level, answer, target_secs(level));
            self.record(step);
        }
        match result {
            Tap::Right(_) => self.celebrate = Some(0.0),
            Tap::Wrong | Tap::Reveal | Tap::Again => self.wobble[slot] = 1.0,
            Tap::Ignored => {}
        }
        if result != Tap::Ignored {
            self.press[slot].kick();
        }
        result
    }

    pub fn celebration_length(&self) -> f32 {
        self.question.seq.len() as f32 * HOP_SECS * 0.6 + 1.0
    }

    pub fn frog_stone(&self) -> usize {
        match self.celebrate {
            Some(t) => ((t / (HOP_SECS * 0.6)) as usize).min(self.question.seq.len() - 1),
            None => 0,
        }
    }

    pub fn react(&mut self, view: &View, slot: usize, tap: Tap) {
        let gold = Color::from_rgba(255, 220, 70, 255);
        let orange = Color::from_rgba(255, 170, 90, 255);
        let at = button_rects(view, self.question.options.len())[slot].center();
        match tap {
            Tap::Right(step) => {
                sfx::play(match step {
                    Step::Up | Step::Cleared => Sfx::Win,
                    _ => Sfx::Right,
                });
                if step == Step::Same {
                    sfx::play_step(Sfx::Right, self.ladder.streak as usize * 2);
                }
                if matches!(step, Step::Up | Step::Cleared) {
                    feel::impact_tinted(Impact::Large, Color::from_rgba(255, 230, 120, 255));
                }
                self.particles.sparkle(at, gold);
                self.particles.emit(
                    at,
                    &[gold, WHITE, hsv(0.55, 0.5, 1.0), hsv(0.9, 0.5, 1.0)],
                    Burst {
                        count: 30,
                        speed: (200.0, 520.0),
                        shape: Shape::Star,
                        size: (6.0, 12.0),
                        angle: -std::f32::consts::FRAC_PI_2,
                        spread: 2.4,
                        ..Default::default()
                    },
                );
                self.streak_bump.kick();
                sfx::say(if self.attempt.wrong == 0 {
                    sfx::Say::Great
                } else {
                    sfx::Say::Correct
                });
                let word = if self.attempt.wrong == 0 {
                    PRAISE[rand::gen_range(0, PRAISE.len())]
                } else {
                    "That's it!"
                };
                let mid = vec2(view.w / 2.0, view.h * 0.4);
                self.popups.add(mid, word, gold, 90.0);
                match step {
                    Step::Up => {
                        self.particles.confetti(view);
                        self.popups.add(
                            mid + vec2(0.0, 90.0),
                            format!("Level up: {}", level_name(self.ladder.level)),
                            WHITE,
                            50.0,
                        );
                    }
                    Step::Cleared => {
                        self.particles.confetti(view);
                        self.popups
                            .add(mid + vec2(0.0, 90.0), "Pattern master!", WHITE, 50.0);
                    }
                    Step::Down => self.popups.add(
                        mid + vec2(0.0, 90.0),
                        format!("Let's practise: {}", level_name(self.ladder.level)),
                        WHITE,
                        44.0,
                    ),
                    Step::Same => {}
                }
            }
            Tap::Wrong => {
                sfx::play(Sfx::Wrong);
                self.shake.add(0.25);
                self.popups
                    .add(at - vec2(0.0, 90.0), "Try again", orange, 48.0);
            }
            Tap::Reveal => {
                sfx::play(Sfx::Wrong);
                self.shake.add(0.25);
                self.popups
                    .add(at - vec2(0.0, 90.0), "This one!", orange, 48.0);
            }
            Tap::Again | Tap::Ignored => {}
        }
    }

    fn update_play(&mut self, input: &Input, view: &View, dt: f32) {
        let end = self.celebration_length();
        if self.celebrate.is_none() {
            self.session.tick(dt);
        }
        if let Some(t) = &mut self.celebrate {
            *t += dt;
            if *t >= end {
                self.next_question();
                if self.report.is_some() {
                    self.mode = Mode::Report(0.0);
                }
            }
        } else if input.pressed && levels_button(view).contains(input.pos) && !self.first_time {
            self.mode = Mode::Levels;
        } else if input.pressed {
            let buttons = button_rects(view, self.question.options.len());
            if let Some(slot) = buttons.iter().position(|r| r.contains(input.pos)) {
                let tap = self.pick(slot);
                self.react(view, slot, tap);
            }
        }
    }

    fn update_levels(&mut self, input: &Input, view: &View) {
        if !input.pressed {
            return;
        }
        if play_button(view).contains(input.pos) {
            self.mode = Mode::Play;
            return;
        }
        let unlocked = self.unlocked_level();
        let hit = level_tiles(view).iter().position(|r| r.contains(input.pos));
        if let Some(i) = hit.filter(|i| *i <= unlocked) {
            sfx::play(Sfx::Tick);
            self.start_level(i);
        }
    }

    fn update_report(&mut self, input: &Input, age: f32, dt: f32) {
        self.mode = Mode::Report(age + dt);
        if input.pressed && age > 0.8 {
            self.report = None;
            self.mode = Mode::Play;
            self.session.start_question();
        }
    }

    pub fn goal_text(&self) -> String {
        if self.ladder.level + 1 < LEVELS {
            format!(
                "Get {UP_STREAK} right in a row to reach {}",
                level_name(self.ladder.level + 1)
            )
        } else {
            format!("Get {UP_STREAK} right in a row to finish")
        }
    }
}

impl Default for Pattern {
    fn default() -> Self {
        Self::new()
    }
}

impl Game for Pattern {
    fn update(&mut self, input: &Input, view: &View, dt: f32) {
        self.t += dt;
        self.streak_bump.update(dt);
        self.prompt_bump.update(dt);
        for b in &mut self.press {
            b.update(dt);
        }
        for w in &mut self.wobble {
            *w = (*w - dt * 1.8).max(0.0);
        }
        match self.mode {
            Mode::Levels => self.update_levels(input, view),
            Mode::Report(age) => self.update_report(input, age, dt),
            Mode::Play => self.update_play(input, view, dt),
        }
        self.shake.update(dt);
        self.particles.update(dt);
        self.popups.update(dt);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn views() -> [View; 4] {
        [
            View::ipad_landscape(),
            View::ipad_portrait(),
            View::fit(390.0, 844.0),
            View::fit(1920.0, 1080.0),
        ]
    }

    fn fresh_player() {
        crate::save::reset_for_test();
        crate::profile::create("Mia", 0);
    }

    fn answer_right(g: &mut Pattern, v: &View) {
        let slot = g.question.slot_of_answer();
        let tap = g.pick(slot);
        g.react(v, slot, tap);
        g.celebrate = None;
        g.next_question();
    }

    fn answer_wrong_once(g: &mut Pattern) {
        let right = g.question.slot_of_answer();
        let wrong = (0..g.question.options.len()).find(|i| *i != right).unwrap();
        g.pick(wrong);
    }

    fn think(g: &mut Pattern, v: &View, secs: f32) {
        for _ in 0..(secs * 60.0) as usize {
            g.update(&Input::default(), v, 1.0 / 60.0);
        }
    }

    fn period_of(shown: &[Token]) -> usize {
        (1..=shown.len())
            .find(|p| (*p..shown.len()).all(|i| shown[i] == shown[i - p]))
            .unwrap()
    }

    #[test]
    fn every_level_has_a_pattern_the_shown_stones_give_away() {
        fresh_player();
        for level in 0..LEVELS {
            for _ in 0..200 {
                let q = Question::generate(level);
                let shown = q.shown();
                match SPECS[level].family {
                    Family::Numbers => {
                        let d = match (shown[1], shown[0]) {
                            (Token::Number(b), Token::Number(a)) => b - a,
                            _ => panic!("numbers expected"),
                        };
                        assert!(SPECS[level].steps.contains(&d), "level {level} step {d}");
                        for w in q.seq.windows(2) {
                            match (w[0], w[1]) {
                                (Token::Number(a), Token::Number(b)) => assert_eq!(b - a, d),
                                _ => panic!(),
                            }
                        }
                    }
                    _ => {
                        let p = period_of(shown);
                        assert_eq!(p, q.period, "level {level} {:?}", q.seq);
                        assert!(shown.len() >= 2 * p, "pattern shown twice, level {level}");
                        assert_eq!(q.answer(), q.seq[q.seq.len() - 1 - p]);
                    }
                }
            }
        }
    }

    #[test]
    fn options_hold_the_answer_once_with_no_duplicates() {
        fresh_player();
        for level in 0..LEVELS {
            for _ in 0..200 {
                let q = Question::generate(level);
                assert_eq!(q.options.len(), choice_count(level));
                assert_eq!(
                    q.options.iter().filter(|t| **t == q.answer()).count(),
                    1,
                    "level {level} {:?}",
                    q
                );
                for (i, a) in q.options.iter().enumerate() {
                    assert!(q.options[i + 1..].iter().all(|b| b != a), "{:?}", q);
                }
                assert_eq!(q.options[q.slot_of_answer()], q.answer());
            }
        }
    }

    #[test]
    fn levels_climb_from_ab_colours_to_shapes_with_colours() {
        assert_eq!(SPECS[0].units, &[&[0usize, 1][..]]);
        assert_eq!(SPECS[0].family, Family::Colours);
        assert_eq!(SPECS[0].options, 2);
        assert!(SPECS[1].units.iter().all(|u| u.len() == 3));
        assert!(SPECS[2].units.iter().all(|u| u == &[0, 1, 2]));
        assert_eq!(SPECS[3].family, Family::Shapes);
        let steps: Vec<i32> = (4..7).flat_map(|l| SPECS[l].steps.to_vec()).collect();
        assert!(steps.contains(&1) && steps.contains(&2) && steps.contains(&5));
        assert_eq!(SPECS[LEVELS - 1].family, Family::Pairs);
        for l in 1..LEVELS {
            assert!(SPECS[l].options >= SPECS[l - 1].options.min(3));
        }
    }

    #[test]
    fn pair_levels_use_distinct_colours_and_shapes_per_symbol_and_hard_decoys() {
        fresh_player();
        for _ in 0..300 {
            let q = Question::generate(LEVELS - 1);
            let mut colours = vec![];
            let mut shapes = vec![];
            for t in q.shown() {
                if let Token::Pair(c, s) = t {
                    if !colours.contains(c) {
                        colours.push(*c);
                    }
                    if !shapes.contains(s) {
                        shapes.push(*s);
                    }
                }
            }
            assert_eq!(colours.len(), shapes.len());
            assert!(q.options.iter().all(|t| matches!(t, Token::Pair(..))));
        }
    }

    #[test]
    fn the_next_pattern_differs_from_the_last() {
        fresh_player();
        for level in 0..LEVELS {
            let mut prev = Question::fresh(level, None);
            let mut same = 0;
            for _ in 0..100 {
                let q = Question::fresh(level, Some(&prev));
                if q.seq == prev.seq {
                    same += 1;
                }
                prev = q;
            }
            assert!(same <= 2, "level {level} repeated {same} times");
        }
    }

    #[test]
    fn the_stone_path_fits_every_screen_between_the_prompt_and_the_buttons() {
        for v in views() {
            for n in 4..=9 {
                let cells = stone_cells(&v, n);
                let area = path_area(&v);
                assert_eq!(cells.len(), n);
                for (i, c) in cells.iter().enumerate() {
                    assert!(
                        c.x >= area.x - 0.5 && c.right() <= area.right() + 0.5,
                        "{n} stones {c:?} outside {area:?}"
                    );
                    assert!(c.y >= area.y - 0.5 && c.bottom() <= area.bottom() + 0.5);
                    assert!(c.w >= 60.0, "stone too small {}", c.w);
                    assert!(cells[i + 1..].iter().all(|o| !o.overlaps(c)));
                }
            }
            assert!(path_area(&v).y >= hud(&v).bottom);
        }
    }

    #[test]
    fn answer_buttons_are_big_and_fit_on_screen() {
        for v in views() {
            for n in 2..=4 {
                for r in button_rects(&v, n) {
                    assert!(r.x >= 0.0 && r.right() <= v.w && r.bottom() <= v.h);
                    assert!(r.w >= v.touch() && r.h >= v.touch());
                }
            }
        }
    }

    #[test]
    fn the_right_stone_celebrates_then_moves_to_a_new_pattern() {
        fresh_player();
        let v = View::ipad_landscape();
        let mut g = Pattern::new();
        let before = g.question.seq.clone();
        let slot = g.question.slot_of_answer();
        let r = button_rects(&v, g.question.options.len())[slot];
        g.update(&Input::press_at(r.center()), &v, 1.0 / 60.0);
        assert!(g.celebrate.is_some());
        let len = g.celebration_length() + 0.2;
        think(&mut g, &v, len);
        assert!(g.celebrate.is_none());
        assert_ne!(g.question.seq, before);
        assert_eq!(g.ladder.streak, 1);
    }

    #[test]
    fn the_frog_hops_along_every_stone_while_celebrating() {
        fresh_player();
        let v = View::ipad_landscape();
        let mut g = Pattern::new();
        assert_eq!(g.frog_stone(), 0);
        let slot = g.question.slot_of_answer();
        g.pick(slot);
        let n = g.question.seq.len();
        let mut seen = vec![];
        while g.celebrate.is_some() && g.celebrate.unwrap() < g.celebration_length() - 0.05 {
            g.update(&Input::default(), &v, 1.0 / 60.0);
            if seen.last() != Some(&g.frog_stone()) {
                seen.push(g.frog_stone());
            }
        }
        assert_eq!(seen.last(), Some(&(n - 1)));
        assert!(seen.windows(2).all(|w| w[1] == w[0] + 1));
    }

    #[test]
    fn wrong_stones_shake_gently_then_show_the_answer() {
        fresh_player();
        let mut g = Pattern::new();
        let right = g.question.slot_of_answer();
        let wrong: Vec<usize> = (0..g.question.options.len())
            .filter(|i| *i != right)
            .collect();
        assert_eq!(g.pick(wrong[0]), Tap::Wrong);
        assert_eq!(g.pick(wrong[0]), Tap::Again);
        assert!(!g.attempt.revealed());
        assert_eq!(g.pick(wrong[0]), Tap::Again);
        let mut g = Pattern::new();
        g.question = Question::generate(1);
        g.load();
        let right = g.question.slot_of_answer();
        let wrong: Vec<usize> = (0..3).filter(|i| *i != right).collect();
        g.pick(wrong[0]);
        assert_eq!(g.pick(wrong[1]), Tap::Reveal);
        assert!(g.attempt.revealed());
    }

    #[test]
    fn four_right_answers_in_a_row_level_up() {
        fresh_player();
        let v = View::ipad_landscape();
        let mut g = Pattern::new();
        for i in 0..UP_STREAK {
            let slot = g.question.slot_of_answer();
            let tap = g.pick(slot);
            if i + 1 == UP_STREAK {
                assert_eq!(tap, Tap::Right(Step::Up));
            }
            g.react(&v, slot, tap);
            g.celebrate = None;
            g.next_question();
        }
        assert_eq!(g.ladder.level, 1);
        assert!(matches!(g.report, Some(Report::Level { level: 0, .. })));
    }

    #[test]
    fn the_first_level_has_two_stones_to_choose_and_a_hint_until_the_first_success() {
        fresh_player();
        let mut g = Pattern::new();
        assert!(g.first_time);
        assert_eq!(g.question.options.len(), 2);
        assert_eq!(g.question.period, 2);
        let v = View::ipad_landscape();
        answer_right(&mut g, &v);
        assert!(!g.first_time);
    }

    #[test]
    fn slow_clean_play_clears_a_level_with_two_stars_and_quick_with_three() {
        let v = View::ipad_landscape();
        fresh_player();
        let mut g = Pattern::new();
        for _ in 0..UP_STREAK {
            think(&mut g, &v, target_secs(0) + 4.0);
            answer_right(&mut g, &v);
        }
        assert_eq!(crate::profile::stars(GAME, 0), 2);
        fresh_player();
        let mut g = Pattern::new();
        for _ in 0..UP_STREAK {
            think(&mut g, &v, 1.0);
            answer_right(&mut g, &v);
        }
        assert_eq!(crate::profile::stars(GAME, 0), 3);
    }

    #[test]
    fn a_level_with_a_slip_earns_fewer_stars() {
        let v = View::ipad_landscape();
        fresh_player();
        let mut g = Pattern::new();
        answer_wrong_once(&mut g);
        answer_right(&mut g, &v);
        for _ in 0..UP_STREAK {
            think(&mut g, &v, 1.0);
            answer_right(&mut g, &v);
        }
        assert!(crate::profile::stars(GAME, 0) <= 2);
    }

    #[test]
    fn finishing_a_level_unlocks_the_next_and_saves_stars() {
        let v = View::ipad_landscape();
        fresh_player();
        let mut g = Pattern::new();
        assert_eq!(g.unlocked_level(), 0);
        for _ in 0..UP_STREAK {
            answer_right(&mut g, &v);
        }
        assert_eq!(g.unlocked_level(), 1);
        assert!(crate::profile::stars(GAME, 0) >= 1);
        assert_eq!(crate::profile::level(GAME), 1);
    }

    #[test]
    fn a_new_run_resumes_at_the_saved_level_for_that_player() {
        let v = View::ipad_landscape();
        fresh_player();
        let mut g = Pattern::new();
        for _ in 0..UP_STREAK {
            answer_right(&mut g, &v);
        }
        assert_eq!(Pattern::new().ladder.level, 1);
        let other = crate::profile::create("Leo", 1).unwrap();
        crate::profile::select(&other.id);
        assert_eq!(Pattern::new().ladder.level, 0);
    }

    #[test]
    fn three_slips_ease_back_a_level_and_the_save_follows() {
        let v = View::ipad_landscape();
        fresh_player();
        crate::profile::unlock(GAME, 2);
        let mut g = Pattern::new();
        g.start_level(2);
        for _ in 0..DOWN_SLIPS {
            answer_wrong_once(&mut g);
            let slot = g.question.slot_of_answer();
            let tap = g.pick(slot);
            g.react(&v, slot, tap);
            g.celebrate = None;
            g.next_question();
        }
        assert_eq!(g.ladder.level, 1);
        assert_eq!(crate::profile::level(GAME), 1);
    }

    #[test]
    fn the_level_picker_only_opens_unlocked_levels() {
        fresh_player();
        let v = View::ipad_landscape();
        let mut g = Pattern::new();
        g.mode = Mode::Levels;
        let tiles = level_tiles(&v);
        g.update(&Input::press_at(tiles[3].center()), &v, 1.0 / 60.0);
        assert_eq!(g.mode, Mode::Levels);
        crate::profile::unlock(GAME, 3);
        g.update(&Input::press_at(tiles[3].center()), &v, 1.0 / 60.0);
        assert_eq!(g.mode, Mode::Play);
        assert_eq!(g.ladder.level, 3);
    }

    #[test]
    fn clearing_the_top_level_posts_the_star_total_once() {
        fresh_player();
        let v = View::ipad_landscape();
        crate::profile::unlock(GAME, (LEVELS - 1) as u32);
        let mut g = Pattern::new();
        g.start_level(LEVELS - 1);
        for _ in 0..UP_STREAK {
            answer_right(&mut g, &v);
        }
        assert!(matches!(g.report, Some(Report::Cleared { .. })));
        assert_eq!(crate::profile::get(GAME, "cleared"), 1);
        g.report = None;
        for _ in 0..UP_STREAK {
            answer_right(&mut g, &v);
        }
        assert!(matches!(g.report, Some(Report::Level { .. })));
    }

    #[test]
    fn level_tiles_fit_on_every_screen_and_clear_the_buttons() {
        for v in views() {
            let tiles = level_tiles(&v);
            assert_eq!(tiles.len(), LEVELS);
            for (i, t) in tiles.iter().enumerate() {
                assert!(t.x >= 0.0 && t.right() <= v.w && t.y >= 0.0);
                assert!(t.bottom() <= play_button(&v).y, "{t:?}");
                assert!(tiles[i + 1..].iter().all(|o| !o.overlaps(t)));
            }
        }
    }

    #[test]
    fn the_goal_row_sits_between_the_hud_and_the_prompt() {
        for v in views() {
            let h = hud(&v);
            assert!(prompt_y(&v) - prompt_size(&v) * 0.6 >= h.bottom - 0.5);
        }
    }

    #[test]
    fn level_names_are_short_and_the_goal_fits_the_longest() {
        for l in 0..LEVELS {
            assert!(level_name(l).len() <= HUD_TITLE.len());
        }
        let g = Pattern::new();
        assert!(g.goal_text().len() <= HUD_GOAL.len());
    }
}
