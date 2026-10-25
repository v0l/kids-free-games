use crate::engine::*;
use crate::feel::{self, Impact};
use crate::fx::*;
use crate::learn::*;
use crate::sfx::{self, Sfx};
use macroquad::prelude::*;

mod art {
    crate::sprites!("count");
}

pub const GAME: &str = "count";
pub const RANGES: [(i32, i32); 6] = [(1, 3), (1, 5), (3, 8), (5, 10), (8, 15), (11, 20)];
pub const MAX_COUNT: i32 = 20;
const TICK_BOUNCE: f32 = 0.45;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Thing {
    Apple,
    Star,
    Fish,
    Duck,
    Frog,
    Pig,
    Bee,
}

impl Thing {
    pub const ALL: [Thing; 7] = [
        Thing::Apple,
        Thing::Star,
        Thing::Fish,
        Thing::Duck,
        Thing::Frog,
        Thing::Pig,
        Thing::Bee,
    ];

    pub fn sprite(self) -> &'static str {
        match self {
            Thing::Apple => "apple",
            Thing::Star => "star",
            Thing::Fish => "fish",
            Thing::Duck => "duck",
            Thing::Frog => "frog",
            Thing::Pig => "pig",
            Thing::Bee => "bee",
        }
    }

    pub fn plural(self) -> &'static str {
        match self {
            Thing::Apple => "apples",
            Thing::Star => "stars",
            Thing::Fish => "fish",
            Thing::Duck => "ducks",
            Thing::Frog => "frogs",
            Thing::Pig => "pigs",
            Thing::Bee => "bees",
        }
    }
}

pub fn level_name(level: usize) -> String {
    format!("Count to {}", RANGES[level].1)
}

pub fn choice_count(level: usize) -> usize {
    if level < 3 {
        3
    } else {
        4
    }
}

pub fn target_secs(level: usize) -> f32 {
    2.5 + RANGES[level.min(RANGES.len() - 1)].1 as f32 * 0.35
}

fn stars_score(s: i64) -> String {
    format!("{s} stars")
}

#[derive(Clone, Debug)]
pub struct Question {
    pub thing: Thing,
    pub count: i32,
    pub options: Vec<i32>,
    pub seed: u32,
}

impl Question {
    pub fn generate(level: usize) -> Self {
        let (low, high) = RANGES[level];
        Self::with_count(level, rand::gen_range(low, high + 1))
    }

    pub fn with_count(level: usize, count: i32) -> Self {
        let pool: Vec<i32> = (1..=4)
            .flat_map(|d| [count - d, count + d])
            .filter(|v| (1..=MAX_COUNT.min(RANGES[level].1 + 2)).contains(v))
            .collect();
        Self {
            thing: Thing::ALL[rand::gen_range(0, Thing::ALL.len())],
            count,
            options: choices(count, pool, choice_count(level)),
            seed: rand::gen_range(0u32, u32::MAX),
        }
    }

    pub fn fresh(level: usize, previous: Option<&Question>) -> Self {
        let (low, high) = RANGES[level];
        let counts: Vec<i32> = (low..=high)
            .filter(|c| previous.is_none_or(|p| p.count != *c))
            .collect();
        match counts.len() {
            0 => Self::generate(level),
            n => Self::with_count(level, counts[rand::gen_range(0, n)]),
        }
    }

    pub fn slot_of_answer(&self) -> usize {
        self.options
            .iter()
            .position(|v| *v == self.count)
            .unwrap_or(0)
    }
}

fn mix(seed: u32, k: u32) -> u32 {
    let mut x = seed ^ k.wrapping_mul(0x9E37_79B1);
    x ^= x >> 15;
    x = x.wrapping_mul(0x85EB_CA6B);
    x ^= x >> 13;
    x = x.wrapping_mul(0xC2B2_AE35);
    x ^ (x >> 16)
}

fn unit(x: u32) -> f32 {
    (x >> 8) as f32 / 16_777_216.0
}

pub fn scatter(count: usize, seed: u32, area: Rect) -> (Vec<Vec2>, f32) {
    let cols = ((count as f32 * area.w / area.h).sqrt().ceil() as usize).clamp(1, count.max(1));
    let rows = count.div_ceil(cols).max(1);
    let cols = count.div_ceil(rows).max(1);
    let (cw, ch) = (area.w / cols as f32, area.h / rows as f32);
    let radius = (cw.min(ch) * 0.5 * 0.8).min(70.0 * (area.h / area.w).clamp(1.0, 1.6));
    let mut cells: Vec<usize> = (0..cols * rows).collect();
    cells.sort_by_key(|c| mix(seed, *c as u32));
    cells.truncate(count);
    let slack = vec2((cw / 2.0 - radius).max(0.0), (ch / 2.0 - radius).max(0.0));
    let points = cells
        .iter()
        .map(|c| {
            let center = vec2(
                area.x + (c % cols) as f32 * cw + cw / 2.0,
                area.y + (c / cols) as f32 * ch + ch / 2.0,
            );
            let jitter = vec2(
                unit(mix(seed, *c as u32 + 1000)) * 2.0 - 1.0,
                unit(mix(seed, *c as u32 + 2000)) * 2.0 - 1.0,
            );
            center + jitter * slack * 0.9
        })
        .collect();
    (points, radius)
}

pub fn prompt_size(view: &View) -> f32 {
    62.0 * ui_scale(view)
}

pub fn goal_row_y(view: &View) -> f32 {
    hud_bottom(view) + 34.0
}

pub fn prompt_y(view: &View) -> f32 {
    hud_bottom(view) + 64.0 + prompt_size(view) * 0.7
}

pub fn levels_button(view: &View) -> Rect {
    let k = ui_scale(view);
    let w = 170.0 * k;
    let h = 46.0 * k;
    Rect::new(view.w - 30.0 - w, goal_row_y(view) - h / 2.0, w, h)
}

pub fn level_tiles(view: &View) -> Vec<Rect> {
    let n = RANGES.len();
    let cols = if tall(view) { 2 } else { 3 };
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
    Rect::new(view.w / 2.0 - 170.0, view.h - 150.0, 340.0, 100.0)
}

pub fn button_height(view: &View) -> f32 {
    124.0 * ui_scale(view)
}

pub fn play_area(view: &View) -> Rect {
    let top = prompt_y(view) + prompt_size(view) * 0.7;
    let bottom = view.h - button_height(view) - 80.0;
    Rect::new(50.0, top, view.w - 100.0, bottom - top)
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

pub struct Count {
    pub mode: Mode,
    pub misses: u32,
    pub first_time: bool,
    pub report: Option<Report>,
    pub ladder: Ladder,
    pub question: Question,
    pub attempt: Attempt,
    pub ticks: Vec<Option<u32>>,
    bounce: Vec<f32>,
    wobble: Vec<f32>,
    press: Vec<Bump>,
    celebrate: Option<f32>,
    streak_bump: Bump,
    prompt_bump: Bump,
    shake: Shake,
    particles: Particles,
    popups: Popups,
    t: f32,
    pub session: crate::mastery::Session,
    pub summary: String,
}

impl Count {
    pub fn new() -> Self {
        let top = RANGES.len() - 1;
        let unlocked = (crate::profile::unlocked(GAME) as usize).min(top);
        let level = (crate::profile::level(GAME) as usize).min(unlocked);
        let cleared = crate::profile::get(GAME, "cleared") > 0;
        let mut ladder = Ladder::new(RANGES.len());
        ladder.level = level;
        ladder.stars = if cleared { RANGES.len() } else { unlocked };
        let question = Question::fresh(level, None);
        let mut g = Self {
            mode: Mode::Play,
            misses: 0,
            first_time: crate::profile::get(GAME, "played") == 0,
            report: None,
            ladder,
            attempt: Attempt::new(0),
            ticks: vec![],
            bounce: vec![],
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
        self.ladder.level = level.min(RANGES.len() - 1);
        self.ladder.streak = 0;
        self.ladder.slips = 0;
        self.misses = 0;
        self.session.reset();
        crate::profile::set_level(GAME, self.ladder.level as u32);
        self.mode = Mode::Play;
        self.next_question();
    }

    pub fn unlocked_level(&self) -> usize {
        (crate::profile::unlocked(GAME) as usize).min(RANGES.len() - 1)
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

    fn load(&mut self) {
        let n = self.question.count as usize;
        let options = self.question.options.len();
        self.attempt = Attempt::new(options);
        self.ticks = vec![None; n];
        self.bounce = vec![TICK_BOUNCE; n];
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

    pub fn ticked(&self) -> u32 {
        self.ticks.iter().flatten().count() as u32
    }

    pub fn tick_object(&mut self, i: usize) -> bool {
        if self.celebrate.is_some() || self.ticks.get(i).is_none_or(|t| t.is_some()) {
            return false;
        }
        self.ticks[i] = Some(self.ticked() + 1);
        self.bounce[i] = 0.0;
        true
    }

    pub fn pick(&mut self, slot: usize) -> Tap {
        if self.celebrate.is_some() || slot >= self.question.options.len() {
            return Tap::Ignored;
        }
        let correct = self.question.options[slot] == self.question.count;
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

    fn celebration_length(&self) -> f32 {
        self.question.count as f32 * 0.05 + 1.0
    }

    fn react(&mut self, view: &View, slot: usize, tap: Tap) {
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
                match u8::try_from(self.question.count) {
                    Ok(n) if n <= 10 => sfx::say(sfx::Say::Number(n)),
                    _ => sfx::say(sfx::Say::Great),
                }
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
                            .add(mid + vec2(0.0, 90.0), "You can count to 20!", WHITE, 50.0);
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
                let text = format!("It's {}", self.question.count);
                self.popups.add(at - vec2(0.0, 90.0), text, orange, 48.0);
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
            } else {
                let q = &self.question;
                let (points, r) = scatter(q.count as usize, q.seed, play_area(view));
                let hit = points
                    .iter()
                    .position(|p| p.distance(input.pos) <= r * 1.15);
                if let Some(i) = hit {
                    if self.tick_object(i) {
                        sfx::play(Sfx::Tick);
                        if let Some(n) = self.ticks[i].filter(|n| *n <= 10) {
                            sfx::say(sfx::Say::Number(n as u8));
                        }
                        let col = hsv(i as f32 * 0.13, 0.5, 1.0);
                        self.particles.sparkle(points[i], col);
                    }
                }
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

    fn number_shown(&self, i: usize) -> Option<u32> {
        match self.celebrate {
            Some(t) if t > i as f32 * 0.05 => Some(i as u32 + 1),
            _ => self.ticks[i],
        }
    }
}

impl Default for Count {
    fn default() -> Self {
        Self::new()
    }
}

impl Game for Count {
    fn update(&mut self, input: &Input, view: &View, dt: f32) {
        self.t += dt;
        self.streak_bump.update(dt);
        self.prompt_bump.update(dt);
        for b in &mut self.press {
            b.update(dt);
        }
        for b in &mut self.bounce {
            *b = (*b + dt).min(TICK_BOUNCE);
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

    fn draw(&self, view: &View) {
        draw_backdrop(view, self.t);
        if self.mode == Mode::Levels {
            self.draw_levels(view);
            return;
        }
        hud(
            view,
            &self.ladder,
            &level_name(self.ladder.level),
            Color::from_rgba(250, 170, 60, 255),
            &self.streak_bump,
        );
        set_view_camera(view, self.shake.offset());
        self.draw_goal(view);
        self.draw_prompt(view);
        self.draw_things(view);
        self.draw_buttons(view);
        if self.first_time && self.mode == Mode::Play {
            let pulse = 1.0 + (self.t * 4.0).sin() * 0.03;
            let y = button_rects(view, self.question.options.len())[0].y - 36.0;
            crate::ui::hint(
                view,
                "Tap each one to count, then pick the number",
                vec2(view.w / 2.0, y),
                pulse,
            );
        }
        set_view_camera(view, Vec2::ZERO);
        self.particles.draw();
        self.popups.draw();
        if let Mode::Report(age) = self.mode {
            self.draw_report(view, age);
        }
    }
}

impl Count {
    pub fn goal_text(&self) -> String {
        if self.ladder.level + 1 < RANGES.len() {
            format!(
                "Get {UP_STREAK} right in a row to reach {}",
                level_name(self.ladder.level + 1)
            )
        } else {
            format!("Get {UP_STREAK} right in a row to finish")
        }
    }

    fn draw_goal(&self, view: &View) {
        let k = ui_scale(view);
        let y = goal_row_y(view);
        let button = levels_button(view);
        let room = if self.first_time {
            view.w - 2.0 * HUD_LEFT
        } else {
            button.x - 20.0 - HUD_LEFT
        };
        let text = self.goal_text();
        let size = (26.0 * k).min(room / text.len() as f32 * 2.0);
        let ink = Color::from_rgba(40, 30, 60, 255);
        let left = if self.first_time {
            view.w / 2.0
        } else {
            HUD_LEFT + room / 2.0
        };
        text_outline(&text, left, y, size, WHITE, ink);
        if !self.first_time {
            crate::ui::painted_button(button, "Levels", "blue", 0.0, 1.0);
        }
    }

    fn draw_levels(&self, view: &View) {
        let unlocked = self.unlocked_level();
        crate::ui::ribbon(view, "Pick a level", vec2(view.w / 2.0, 90.0), self.t);
        for (i, r) in level_tiles(view).iter().enumerate() {
            let open = i <= unlocked;
            let now = i == self.ladder.level;
            if now {
                let g = 8.0 + 3.0 * (self.t * 5.0).sin();
                round_rect(
                    Rect::new(r.x - g, r.y - g, r.w + g * 2.0, r.h + g * 2.0),
                    34.0,
                    Color::new(1.0, 0.95, 0.4, 0.8),
                );
            }
            let tint = if open { WHITE } else { with_alpha(WHITE, 0.55) };
            crate::ui::painted_panel(*r, tint);
            let (low, high) = RANGES[i];
            let label = if low == 1 {
                format!("1 to {high}")
            } else {
                format!("{low} to {high}")
            };
            let ink = Color::from_rgba(40, 30, 60, 255);
            if open {
                text_outline(
                    &label,
                    r.center().x,
                    r.y + r.h * 0.38,
                    r.h * 0.26,
                    WHITE,
                    Color::from_rgba(10, 70, 140, 255),
                );
                let earned = crate::profile::stars(GAME, i as u32);
                for j in 0..3u8 {
                    let c = vec2(
                        r.center().x + (j as f32 - 1.0) * r.h * 0.26,
                        r.y + r.h * 0.72,
                    );
                    let fill = if j < earned {
                        Color::from_rgba(255, 210, 60, 255)
                    } else {
                        Color::from_rgba(110, 90, 140, 150)
                    };
                    star(c, r.h * 0.11, 0.0, fill);
                }
            } else {
                crate::ui::sprite_at(
                    "icon_lock",
                    r.center() - vec2(0.0, r.h * 0.05),
                    r.h * 0.34,
                    crate::art::Draw {
                        tint: ink,
                        ..Default::default()
                    },
                );
                text_center(
                    &label,
                    r.center().x,
                    r.y + r.h * 0.8,
                    r.h * 0.14,
                    with_alpha(ink, 0.6),
                );
            }
        }
        let pb = play_button(view);
        crate::ui::painted_button(pb, "Play", "green", 0.0, 1.0);
    }

    fn draw_report(&self, view: &View, age: f32) {
        match self.report {
            Some(Report::Level { level, stars }) => {
                let title = format!("{} done!", level_name(level));
                let next = if level + 1 < RANGES.len() {
                    format!("Next: {}", level_name(level + 1))
                } else {
                    "Counting champion!".to_string()
                };
                overlay(view, &title, &["", &next, &self.summary]);
                let c = view.center() + vec2(0.0, 20.0);
                let ph = 150.0 + 3.0 * 64.0;
                let y = c.y - ph / 2.0 + 118.0;
                for j in 0..3u8 {
                    let k = ease_out_back(((age - 0.3 - j as f32 * 0.15) / 0.3).clamp(0.0, 1.0));
                    let fill = if j < stars {
                        Color::from_rgba(255, 210, 60, 255)
                    } else {
                        Color::from_rgba(150, 140, 170, 140)
                    };
                    star(vec2(c.x + (j as f32 - 1.0) * 90.0, y), 38.0 * k, 0.0, fill);
                }
            }
            Some(Report::Cleared { rank }) => {
                let total = crate::profile::total_stars(GAME);
                let line = format!("{total} stars earned");
                results(
                    view,
                    &crate::ui::Results {
                        game: GAME,
                        title: "You can count to 20!",
                        line: &line,
                        rank,
                        action: "Keep counting",
                        format: stars_score,
                    },
                );
            }
            None => {}
        }
    }

    fn draw_prompt(&self, view: &View) {
        let s = format!("How many {}?", self.question.thing.plural());
        let pop = ease_out_back(self.prompt_bump.t).max(0.2);
        text_outline(
            &s,
            view.w / 2.0,
            prompt_y(view),
            prompt_size(view) * pop,
            WHITE,
            Color::from_rgba(40, 30, 60, 255),
        );
    }

    fn draw_things(&self, view: &View) {
        let area = play_area(view);
        crate::ui::painted_panel(
            Rect::new(area.x - 24.0, area.y - 18.0, area.w + 48.0, area.h + 36.0),
            with_alpha(WHITE, 0.4),
        );
        let q = &self.question;
        let (points, r) = scatter(q.count as usize, q.seed, area);
        for (i, p) in points.iter().enumerate() {
            let k = (self.bounce[i] / TICK_BOUNCE).clamp(0.0, 1.0);
            let hop = (k * std::f32::consts::PI).sin() * r * 0.6;
            let party = self.celebrate.map_or(0.0, |t| {
                ((t * 9.0 - i as f32 * 0.7).sin().max(0.0)) * r * 0.25
            });
            let squash = 1.0 + 0.25 * (1.0 - ease_out_cubic(k)) * (k < 1.0) as i32 as f32;
            let c = *p - vec2(0.0, hop + party);
            soft_shadow(*p + vec2(0.0, r * 0.85), r * 0.7, r * 0.16, 0.6);
            let intro = ease_out_back((self.t * 2.0 - i as f32 * 0.04).clamp(0.0, 1.0));
            draw_thing(q.thing, c, r * squash * intro, self.t + i as f32);
            if let Some(n) = self.number_shown(i) {
                let b = r * 0.55;
                crate::ui::sprite_at(
                    "p_round",
                    c + vec2(0.0, r * 0.1),
                    b * 2.2,
                    Default::default(),
                );
                text_outline(
                    &n.to_string(),
                    c.x,
                    c.y + r * 0.08,
                    b * 1.2,
                    WHITE,
                    Color::from_rgba(10, 70, 140, 255),
                );
            }
        }
    }

    fn draw_buttons(&self, view: &View) {
        let rects = button_rects(view, self.question.options.len());
        for (i, (r, v)) in rects.iter().zip(&self.question.options).enumerate() {
            let tried = self.attempt.tried[i];
            let hint = self.attempt.revealed() && *v == self.question.count;
            let pop = self.press[i].scale(0.12);
            let wob = (self.wobble[i] * 36.0).sin() * 12.0 * self.wobble[i];
            let c = r.center() + vec2(wob, 0.0);
            let rr = Rect::new(
                c.x - r.w * pop / 2.0,
                c.y - r.h * pop / 2.0,
                r.w * pop,
                r.h * pop,
            );
            if hint {
                let pulse = (self.t * 6.0).sin();
                let g = 10.0 + 4.0 * pulse;
                round_rect(
                    Rect::new(rr.x - g, rr.y - g, rr.w + g * 2.0, rr.h + g * 2.0),
                    24.0,
                    Color::new(1.0, 0.95, 0.4, 0.7),
                );
            }
            let correct_now = self.attempt.solved && *v == self.question.count;
            let color = if correct_now {
                "green"
            } else {
                ["orange", "blue", "green", "orange"][i % 4]
            };
            let alpha = if tried { 0.45 } else { 1.0 };
            crate::ui::painted_button(rr, &v.to_string(), color, 0.0, alpha);
        }
    }
}

fn thing_height(name: &str, r: f32) -> f32 {
    let s = art::size(name);
    2.2 * r * s.y / s.x.max(s.y)
}

fn draw_thing(thing: Thing, c: Vec2, r: f32, t: f32) {
    let name = thing.sprite();
    let wiggle = match thing {
        Thing::Star => (t * 2.0).sin() * 0.15,
        Thing::Fish | Thing::Bee => (t * 5.0).sin() * 0.08,
        _ => 0.0,
    };
    art::sprite_at(
        name,
        c,
        thing_height(name, r),
        crate::art::Draw {
            rotation: wiggle,
            ..Default::default()
        },
    );
}

fn draw_backdrop(view: &View, t: f32) {
    let f = view.full();
    let side = f.w.max(f.h);
    art::sprite(
        "bg",
        Rect::new(f.x + (f.w - side) / 2.0, f.y + f.h - side, side, side),
    );
    for i in 0..5usize {
        let span = view.w + 500.0;
        let x = (i as f32 * 0.23 * span + t * (7.0 + i as f32 * 3.0)) % span - 250.0;
        let y = hud_bottom(view) + 10.0 + (i as f32 * 0.37 % 1.0) * view.h * 0.25;
        crate::ui::cloud(
            i * 2 + 1,
            vec2(x, y),
            150.0 + (i % 3) as f32 * 50.0,
            with_alpha(WHITE, 0.9),
        );
    }
}

pub fn icon(r: Rect) {
    crate::ui::icon("count", r);
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

    #[test]
    fn count_sprites_all_exist() {
        let have = art::names();
        for n in ["bg"]
            .into_iter()
            .chain(Thing::ALL.iter().map(|t| t.sprite()))
        {
            assert!(have.iter().any(|h| h == n), "missing {n}");
        }
    }

    #[test]
    fn every_thing_fits_inside_its_scatter_cell() {
        for t in Thing::ALL {
            let s = art::size(t.sprite());
            let h = thing_height(t.sprite(), 50.0);
            assert!(h <= 2.2 * 50.0 + 0.01);
            assert!(h * s.x / s.y <= 2.2 * 50.0 + 0.01);
        }
    }

    #[test]
    fn tapping_a_thing_says_its_number() {
        let v = View::ipad_landscape();
        let mut g = Count::new();
        let (points, _) = scatter(g.question.count as usize, g.question.seed, play_area(&v));
        g.update(&Input::press_at(points[0]), &v, 1.0 / 60.0);
        assert_eq!(crate::sfx::take_voice(), Some(sfx::Say::Number(1)));
    }

    #[test]
    fn the_right_button_says_the_total() {
        let v = View::ipad_landscape();
        let mut g = Count::new();
        g.question = Question::with_count(0, 4);
        g.load();
        let slot = g.question.slot_of_answer();
        let tap = g.pick(slot);
        g.react(&v, slot, tap);
        assert_eq!(crate::sfx::take_voice(), Some(sfx::Say::Number(4)));
    }

    #[test]
    fn counts_stay_inside_each_level_range() {
        rand::srand(5);
        for (level, (low, high)) in RANGES.iter().enumerate() {
            for _ in 0..300 {
                let q = Question::generate(level);
                assert!((*low..=*high).contains(&q.count), "{q:?}");
            }
        }
        assert_eq!(RANGES[0].0, 1);
        assert_eq!(RANGES[RANGES.len() - 1].1, MAX_COUNT);
    }

    #[test]
    fn choices_have_one_right_answer_and_no_duplicates() {
        rand::srand(9);
        for (level, (low, high)) in RANGES.iter().enumerate() {
            for count in *low..=*high {
                for _ in 0..30 {
                    let q = Question::with_count(level, count);
                    assert_eq!(q.options.len(), choice_count(level));
                    assert_eq!(q.options.iter().filter(|v| **v == count).count(), 1);
                    let mut sorted = q.options.clone();
                    sorted.sort_unstable();
                    sorted.dedup();
                    assert_eq!(sorted.len(), q.options.len(), "{q:?}");
                    assert!(q.options.iter().all(|v| (1..=MAX_COUNT).contains(v)));
                    assert!(q.options.iter().all(|v| *v <= high + 2), "{q:?}");
                    assert_eq!(q.options[q.slot_of_answer()], count);
                }
            }
        }
    }

    #[test]
    fn the_next_count_differs_from_the_last() {
        for seed in 0..40 {
            rand::srand(seed);
            let mut q = Question::fresh(0, None);
            for _ in 0..500 {
                let next = Question::fresh(0, Some(&q));
                assert_ne!(q.count, next.count, "seed {seed}");
                q = next;
            }
        }
    }

    #[test]
    fn scattered_things_never_overlap_and_stay_in_the_play_area() {
        for v in views() {
            let area = play_area(&v);
            for count in 1..=MAX_COUNT as usize {
                for seed in [1u32, 77, 4_000_000_000] {
                    let (points, r) = scatter(count, seed, area);
                    assert_eq!(points.len(), count);
                    for (i, p) in points.iter().enumerate() {
                        assert!(
                            p.x - r >= area.x - 0.01
                                && p.x + r <= area.x + area.w + 0.01
                                && p.y - r >= area.y - 0.01
                                && p.y + r <= area.y + area.h + 0.01,
                            "{count} in {v:?}"
                        );
                        for q in &points[i + 1..] {
                            assert!(p.distance(*q) >= 2.0 * r - 0.01, "{count} in {v:?}");
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn scatter_is_stable_for_a_seed_and_varies_between_seeds() {
        let area = play_area(&View::ipad_landscape());
        assert_eq!(scatter(9, 3, area).0, scatter(9, 3, area).0);
        assert_ne!(scatter(9, 3, area).0, scatter(9, 4, area).0);
    }

    #[test]
    fn on_a_tall_phone_everything_scales_up() {
        let v = View::fit(1080.0, 2400.0);
        for n in [3, 4] {
            for r in button_rects(&v, n) {
                assert!(r.w >= 140.0 && r.h >= 140.0, "{r:?}");
            }
        }
        assert!(prompt_size(&v) >= 90.0);
        assert!(hud_bottom(&v) >= 100.0);
        let area = play_area(&v);
        assert!(area.h >= v.h * 0.45, "{area:?}");
        let (_, r) = scatter(5, 1, area);
        assert!(r >= 90.0, "{r}");
        let (_, r) = scatter(20, 1, area);
        assert!(r >= 60.0, "{r}");
    }

    #[test]
    fn things_are_big_enough_to_tap_even_at_twenty() {
        for v in views() {
            let (_, r) = scatter(20, 1, play_area(&v));
            assert!(r >= 35.0, "{r} in {v:?}");
        }
    }

    #[test]
    fn answer_buttons_are_big_and_fit_on_screen() {
        for v in views() {
            for n in [3, 4] {
                let rects = button_rects(&v, n);
                for (i, r) in rects.iter().enumerate() {
                    assert!(r.w >= 90.0 && r.h >= 90.0, "{r:?}");
                    assert!(r.x >= 0.0 && r.x + r.w <= v.w && r.y + r.h <= v.h);
                    if i > 0 {
                        assert!(r.x >= rects[i - 1].x + rects[i - 1].w);
                    }
                }
                assert!(rects[0].y >= play_area(&v).y + play_area(&v).h);
            }
        }
    }

    #[test]
    fn ticking_numbers_things_in_the_order_tapped() {
        let mut g = Count::new();
        g.question = Question::with_count(0, 4);
        g.load();
        assert!(g.tick_object(2));
        assert!(g.tick_object(0));
        assert!(!g.tick_object(2));
        assert_eq!(g.ticks[2], Some(1));
        assert_eq!(g.ticks[0], Some(2));
        assert_eq!(g.ticked(), 2);
        assert!(!g.tick_object(999));
    }

    #[test]
    fn tapping_a_thing_in_the_play_area_ticks_it() {
        let v = View::ipad_landscape();
        let mut g = Count::new();
        let (points, _) = scatter(g.question.count as usize, g.question.seed, play_area(&v));
        g.update(&Input::press_at(points[0]), &v, 1.0 / 60.0);
        assert_eq!(g.ticks[0], Some(1));
        assert!(!g.particles.list.is_empty());
    }

    #[test]
    fn right_button_celebrates_then_moves_to_a_new_question() {
        let v = View::ipad_landscape();
        let mut g = Count::new();
        let before = g.question.clone();
        let slot = before.slot_of_answer();
        let at = button_rects(&v, before.options.len())[slot].center();
        g.update(&Input::press_at(at), &v, 1.0 / 60.0);
        assert!(g.celebrate.is_some());
        assert_eq!(g.ladder.streak, 1);
        assert!(!g.popups.list.is_empty());
        assert!(!g.tick_object(0));
        for _ in 0..240 {
            g.update(&Input::default(), &v, 1.0 / 60.0);
        }
        assert!(g.celebrate.is_none());
        assert_ne!(g.question.count, before.count);
        assert!(g.ticks.iter().all(|t| t.is_none()));
    }

    #[test]
    fn wrong_buttons_shake_gently_then_show_the_answer() {
        let v = View::ipad_landscape();
        let mut g = Count::new();
        let right = g.question.slot_of_answer();
        let wrongs: Vec<usize> = (0..g.question.options.len())
            .filter(|i| *i != right)
            .collect();
        let rects = button_rects(&v, g.question.options.len());
        g.update(&Input::press_at(rects[wrongs[0]].center()), &v, 1.0 / 60.0);
        assert!(g.shake.active());
        assert!(!g.attempt.revealed());
        g.update(&Input::press_at(rects[wrongs[1]].center()), &v, 1.0 / 60.0);
        assert!(g.attempt.revealed());
        assert!(g.celebrate.is_none());
        assert_eq!(g.ladder.level, 0);
    }

    #[test]
    fn counting_celebration_numbers_every_thing() {
        let mut g = Count::new();
        let slot = g.question.slot_of_answer();
        g.pick(slot);
        g.celebrate = Some(5.0);
        for i in 0..g.ticks.len() {
            assert_eq!(g.number_shown(i), Some(i as u32 + 1));
        }
    }

    #[test]
    fn four_right_answers_in_a_row_level_up() {
        let v = View::ipad_landscape();
        let mut g = Count::new();
        for k in 0..UP_STREAK {
            let slot = g.question.slot_of_answer();
            let tap = g.pick(slot);
            g.react(&v, slot, tap);
            if k + 1 == UP_STREAK {
                assert!(g.particles.list.iter().any(|p| p.shape == Shape::Confetti));
            }
            for _ in 0..240 {
                g.update(&Input::default(), &v, 1.0 / 60.0);
            }
        }
        assert_eq!(g.ladder.level, 1);
        assert!((RANGES[1].0..=RANGES[1].1).contains(&g.question.count));
    }

    fn fresh_player() {
        crate::save::reset_for_test();
        crate::profile::create("Mia", 0);
    }

    fn answer_right(g: &mut Count, v: &View) {
        let slot = g.question.slot_of_answer();
        let tap = g.pick(slot);
        g.react(v, slot, tap);
        for _ in 0..240 {
            g.update(&Input::default(), v, 1.0 / 60.0);
        }
        if matches!(g.mode, Mode::Report(_)) {
            g.update(&Input::press_at(vec2(1.0, 1.0)), v, 1.0 / 60.0);
            for _ in 0..60 {
                g.update(&Input::default(), v, 1.0 / 60.0);
            }
            g.update(&Input::press_at(vec2(1.0, 1.0)), v, 1.0 / 60.0);
        }
    }

    fn answer_wrong_once(g: &mut Count) {
        let wrong = (0..g.question.options.len())
            .find(|i| *i != g.question.slot_of_answer())
            .unwrap();
        g.pick(wrong);
    }

    fn think(g: &mut Count, v: &View, secs: f32) {
        for _ in 0..(secs * 60.0) as usize {
            g.update(&Input::default(), v, 1.0 / 60.0);
        }
    }

    #[test]
    fn slow_counting_clears_the_level_with_two_stars_and_quick_with_three() {
        let v = View::ipad_landscape();
        for (secs, want) in [(1.0, 3), (12.0, 2)] {
            fresh_player();
            let mut g = Count::new();
            for _ in 0..UP_STREAK {
                think(&mut g, &v, secs);
                let slot = g.question.slot_of_answer();
                let tap = g.pick(slot);
                g.react(&v, slot, tap);
                while g.celebrate.is_some() {
                    g.update(&Input::default(), &v, 1.0 / 60.0);
                }
            }
            assert_eq!(
                crate::profile::stars(GAME, 0),
                want,
                "{secs} s {}",
                g.summary
            );
            let b = crate::mastery::band(GAME, 0);
            assert_eq!(b.answers, UP_STREAK as i64);
            assert!((b.secs - secs).abs() < 0.2, "{}", b.secs);
        }
    }

    #[test]
    fn the_first_level_counts_to_three_with_a_hint_until_the_first_success() {
        fresh_player();
        rand::srand(3);
        let v = View::ipad_landscape();
        let mut g = Count::new();
        assert_eq!(g.ladder.level, 0);
        assert_eq!(RANGES[0], (1, 3));
        assert!(g.first_time);
        assert!(g.question.count <= 3);
        answer_right(&mut g, &v);
        assert!(!g.first_time);
    }

    #[test]
    fn ranges_climb_through_five_ten_and_twenty() {
        let highs: Vec<i32> = RANGES.iter().map(|r| r.1).collect();
        assert_eq!(highs, vec![3, 5, 8, 10, 15, 20]);
        assert!(RANGES
            .windows(2)
            .all(|w| w[0].1 < w[1].1 && w[0].0 <= w[1].0));
        assert!(choice_count(0) <= choice_count(RANGES.len() - 1));
    }

    #[test]
    fn finishing_a_level_unlocks_the_next_and_saves_stars() {
        fresh_player();
        let v = View::ipad_landscape();
        let mut g = Count::new();
        for _ in 0..UP_STREAK {
            answer_right(&mut g, &v);
        }
        assert_eq!(g.ladder.level, 1);
        assert_eq!(crate::profile::unlocked(GAME), 1);
        assert_eq!(crate::profile::level(GAME), 1);
        assert_eq!(crate::profile::stars(GAME, 0), 3);
    }

    #[test]
    fn a_level_with_a_slip_earns_fewer_stars() {
        fresh_player();
        let v = View::ipad_landscape();
        let mut g = Count::new();
        answer_wrong_once(&mut g);
        for _ in 0..UP_STREAK + 1 {
            answer_right(&mut g, &v);
        }
        assert_eq!(g.ladder.level, 1);
        assert_eq!(crate::profile::stars(GAME, 0), 2);
    }

    #[test]
    fn a_new_run_resumes_at_the_saved_level_for_that_player() {
        fresh_player();
        let v = View::ipad_landscape();
        let mut g = Count::new();
        for _ in 0..UP_STREAK * 2 {
            answer_right(&mut g, &v);
        }
        assert_eq!(g.ladder.level, 2);
        let again = Count::new();
        assert_eq!(again.ladder.level, 2);
        assert!((RANGES[2].0..=RANGES[2].1).contains(&again.question.count));
        assert!(!again.first_time);
        crate::profile::create("Leo", 3);
        let other = Count::new();
        assert_eq!(other.ladder.level, 0);
        assert!(other.first_time);
    }

    #[test]
    fn three_slips_ease_back_a_level_and_the_save_follows() {
        fresh_player();
        let v = View::ipad_landscape();
        let mut g = Count::new();
        for _ in 0..UP_STREAK {
            answer_right(&mut g, &v);
        }
        assert_eq!(g.ladder.level, 1);
        for _ in 0..DOWN_SLIPS {
            answer_wrong_once(&mut g);
            let slot = g.question.slot_of_answer();
            g.pick(slot);
            g.celebrate = None;
            g.next_question();
        }
        assert_eq!(g.ladder.level, 0);
        assert_eq!(crate::profile::level(GAME), 0);
        assert_eq!(crate::profile::unlocked(GAME), 1);
        assert_eq!(Count::new().ladder.level, 0);
    }

    #[test]
    fn the_level_picker_only_opens_unlocked_levels() {
        fresh_player();
        let v = View::ipad_landscape();
        let mut g = Count::new();
        for _ in 0..UP_STREAK {
            answer_right(&mut g, &v);
        }
        g.mode = Mode::Levels;
        let tiles = level_tiles(&v);
        g.update(&Input::press_at(tiles[4].center()), &v, 1.0 / 60.0);
        assert_eq!(g.mode, Mode::Levels);
        g.update(&Input::press_at(tiles[0].center()), &v, 1.0 / 60.0);
        assert_eq!(g.mode, Mode::Play);
        assert_eq!(g.ladder.level, 0);
        assert_eq!(crate::profile::level(GAME), 0);
        assert!((RANGES[0].0..=RANGES[0].1).contains(&g.question.count));
        assert_eq!(crate::profile::unlocked(GAME), 1);
    }

    #[test]
    fn the_levels_button_appears_after_the_first_success() {
        fresh_player();
        let v = View::ipad_landscape();
        let mut g = Count::new();
        let at = levels_button(&v).center();
        g.update(&Input::press_at(at), &v, 1.0 / 60.0);
        assert_eq!(g.mode, Mode::Play);
        answer_right(&mut g, &v);
        g.update(&Input::press_at(at), &v, 1.0 / 60.0);
        assert_eq!(g.mode, Mode::Levels);
    }

    #[test]
    fn clearing_the_top_level_posts_the_star_total_once() {
        fresh_player();
        let v = View::ipad_landscape();
        let mut g = Count::new();
        let last = RANGES.len() - 1;
        crate::profile::unlock(GAME, last as u32);
        g.start_level(last);
        for _ in 0..UP_STREAK {
            let slot = g.question.slot_of_answer();
            let tap = g.pick(slot);
            g.react(&v, slot, tap);
            g.celebrate = None;
            g.next_question();
        }
        assert_eq!(crate::profile::get(GAME, "cleared"), 1);
        assert!(matches!(g.report, Some(Report::Cleared { rank: Some(0) })));
        assert_eq!(crate::profile::board(GAME).len(), 1);
        assert_eq!(Count::new().ladder.stars, RANGES.len());
    }

    #[test]
    fn level_tiles_fit_on_every_screen_and_clear_the_buttons() {
        for v in views() {
            let tiles = level_tiles(&v);
            assert_eq!(tiles.len(), RANGES.len());
            for r in &tiles {
                assert!(r.x >= 0.0 && r.x + r.w <= v.w && r.y >= 100.0 && r.y + r.h <= v.h);
                assert!(r.w >= 150.0 && r.h >= 100.0, "{r:?} in {v:?}");
                assert!(r.y + r.h <= play_button(&v).y);
            }
            assert!(levels_button(&v).x + levels_button(&v).w <= v.w);
        }
    }

    #[test]
    fn the_goal_row_sits_between_the_hud_and_the_question() {
        for v in views() {
            assert!(goal_row_y(&v) > hud_bottom(&v));
            assert!(prompt_y(&v) - prompt_size(&v) * 0.5 > goal_row_y(&v) + 20.0);
            let area = play_area(&v);
            assert!(area.y > prompt_y(&v));
            assert!(area.h >= v.h * 0.3, "{area:?} in {v:?}");
        }
    }
}
