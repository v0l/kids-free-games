use crate::engine::draw_circle;
use crate::engine::*;
use crate::feel::{self, Impact};
use crate::fx::*;
use crate::learn::{choices, hud_bottom, tall, ui_scale, PRAISE};
use crate::profile;
use crate::sfx::{self, Sfx};
use crate::ui::BODY;
use macroquad::prelude::*;

mod art {
    crate::sprites!("mathpop");
}

pub const KEY: &str = "mathpop";
pub const LEVEL_NAMES: [&str; 9] = [
    "Adding to 5",
    "Adding to 10",
    "Adding to 20",
    "Taking away to 10",
    "Taking away to 20",
    "Mixed up",
    "Times 2, 5, 10",
    "Times tables",
    "Maths whizz",
];
pub const QUESTIONS: u32 = 5;
pub const DOWN_SLIPS: u32 = 3;
pub const BALLOONS: usize = 4;
const BALLOON_COLORS: [(&str, Color); 6] = [
    ("balloon_red", Color::new(0.95, 0.25, 0.3, 1.0)),
    ("balloon_orange", Color::new(1.0, 0.6, 0.15, 1.0)),
    ("balloon_yellow", Color::new(1.0, 0.85, 0.2, 1.0)),
    ("balloon_green", Color::new(0.3, 0.8, 0.35, 1.0)),
    ("balloon_blue", Color::new(0.25, 0.55, 0.95, 1.0)),
    ("balloon_purple", Color::new(0.65, 0.4, 0.9, 1.0)),
];
const LEAVE_TIME: f32 = 1.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Op {
    Add,
    Sub,
    Mul,
}

impl Op {
    pub fn symbol(self) -> &'static str {
        match self {
            Op::Add => "+",
            Op::Sub => "−",
            Op::Mul => "×",
        }
    }
}

#[derive(Clone, Debug)]
pub struct Question {
    pub a: i32,
    pub b: i32,
    pub op: Op,
    pub answer: i32,
    pub options: Vec<i32>,
}

fn operands(level: usize) -> (i32, i32, Op) {
    match level {
        0 => {
            let a = rand::gen_range(1, 5);
            (a, rand::gen_range(1, 6 - a), Op::Add)
        }
        1 => {
            let a = rand::gen_range(1, 10);
            let low = (6 - a).max(1);
            (a, rand::gen_range(low, 11 - a), Op::Add)
        }
        2 => {
            let a = rand::gen_range(2, 19);
            let low = (11 - a).max(1);
            (a, rand::gen_range(low, 21 - a), Op::Add)
        }
        3 => {
            let a = rand::gen_range(2, 11);
            (a, rand::gen_range(1, a), Op::Sub)
        }
        4 => {
            let a = rand::gen_range(11, 21);
            (a, rand::gen_range(1, a), Op::Sub)
        }
        5 => operands(rand::gen_range(1, 5)),
        6 => {
            let a = [2, 5, 10][rand::gen_range(0, 3)];
            let b = rand::gen_range(1, 11);
            if rand::gen_range(0, 2) == 0 {
                (a, b, Op::Mul)
            } else {
                (b, a, Op::Mul)
            }
        }
        7 => (rand::gen_range(2, 11), rand::gen_range(1, 11), Op::Mul),
        _ => operands([2, 4, 7][rand::gen_range(0, 3)]),
    }
}

fn solve(a: i32, b: i32, op: Op) -> i32 {
    match op {
        Op::Add => a + b,
        Op::Sub => a - b,
        Op::Mul => a * b,
    }
}

impl Question {
    pub fn generate(level: usize) -> Self {
        let (a, b, op) = operands(level);
        Self::from_parts(a, b, op)
    }

    pub fn from_parts(a: i32, b: i32, op: Op) -> Self {
        let answer = solve(a, b, op);
        let mut pool: Vec<i32> = (1..=4).flat_map(|d| [answer - d, answer + d]).collect();
        match op {
            Op::Add => pool.extend([a - b, answer + 10, answer - 10]),
            Op::Sub => pool.extend([a + b, answer + 10]),
            Op::Mul => pool.extend([(a - 1) * b, (a + 1) * b, a * (b - 1), a * (b + 1), a + b]),
        }
        pool.retain(|v| *v >= 1);
        Self {
            a,
            b,
            op,
            answer,
            options: choices(answer, pool, BALLOONS),
        }
    }

    pub fn starter() -> Self {
        Self::from_parts(rand::gen_range(1, 3), rand::gen_range(1, 3), Op::Add)
    }

    pub fn fresh(level: usize, previous: Option<&Question>) -> Self {
        let mut q = Self::generate(level);
        for _ in 0..8 {
            let same = previous.is_some_and(|p| (p.a, p.b, p.op) == (q.a, q.b, q.op));
            if !same {
                break;
            }
            q = Self::generate(level);
        }
        q
    }

    pub fn text(&self, solved: bool) -> String {
        let end = if solved {
            self.answer.to_string()
        } else {
            "?".to_string()
        };
        format!("{} {} {} = {}", self.a, self.op.symbol(), self.b, end)
    }

    pub fn slot_of_answer(&self) -> usize {
        self.options
            .iter()
            .position(|v| *v == self.answer)
            .unwrap_or(0)
    }
}

const WIDEST_QUESTION_EM: f32 = 6.6;
const QUESTION_PAD_EM: f32 = 0.6;

pub fn question_text_size(view: &View) -> f32 {
    (88.0 * ui_scale(view)).min((view.w - 40.0) / (WIDEST_QUESTION_EM + QUESTION_PAD_EM))
}

pub fn question_rect(view: &View) -> Rect {
    let size = question_text_size(view);
    let w = (size * (WIDEST_QUESTION_EM + QUESTION_PAD_EM))
        .max(360.0)
        .min(view.w - 20.0);
    Rect::new((view.w - w) / 2.0, hud_bottom(view) + 24.0, w, size * 1.25)
}

fn grid(view: &View) -> (usize, Rect) {
    let top = question_rect(view).bottom() + 30.0;
    let cols = if tall(view) { 2 } else { BALLOONS };
    (cols, Rect::new(0.0, top, view.w, view.h - 40.0 - top))
}

pub fn balloon_radius(view: &View) -> f32 {
    let (cols, area) = grid(view);
    let rows = BALLOONS.div_ceil(cols) as f32;
    let cap = if tall(view) { f32::MAX } else { 86.0 };
    (area.w / cols as f32 * 0.36)
        .min(area.h / rows / 4.0)
        .min(cap)
}

pub fn balloon_rest(view: &View, slot: usize) -> Vec2 {
    let (cols, area) = grid(view);
    let rows = BALLOONS.div_ceil(cols);
    let r = balloon_radius(view);
    let (col, row) = (slot % cols, slot / cols);
    let cell = vec2(area.w / cols as f32, area.h / rows as f32);
    let amp = ((cell.y - 3.2 * r) / 2.0 * 0.9).clamp(0.0, 70.0);
    let stagger = [-1.0, 0.6, -0.5, 1.0][slot % 4];
    vec2(
        cell.x * (col as f32 + 0.5),
        area.y + cell.y * (row as f32 + 0.5) - r * 0.6 + stagger * amp,
    )
}

#[derive(Clone, Debug)]
struct Balloon {
    value: i32,
    color: usize,
    wobble: f32,
    tried: bool,
    popped: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tap {
    Right,
    Wrong,
    Reveal,
    Again,
    Ignored,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Screen {
    Play,
    Levels,
    Done(f32),
    Final(f32),
}

pub fn stars_for(slips: u32) -> u8 {
    match slips {
        0 => 3,
        1 | 2 => 2,
        _ => 1,
    }
}

pub struct MathPop {
    pub level: usize,
    pub answered: u32,
    pub slips: u32,
    pub slip_run: u32,
    pub wrong: u32,
    pub solved: bool,
    pub rank: Option<usize>,
    teach: bool,
    screen: Screen,
    finished: Option<u8>,
    pub question: Question,
    balloons: Vec<Balloon>,
    leave: Option<f32>,
    asked: f32,
    streak_bump: Bump,
    question_bump: Bump,
    particles: Particles,
    popups: Popups,
    press: f32,
    t: f32,
}

impl MathPop {
    pub fn new() -> Self {
        let teach = profile::get(KEY, "played") == 0;
        let last = LEVEL_NAMES.len() - 1;
        let level = (profile::level(KEY) as usize)
            .min(profile::unlocked(KEY) as usize)
            .min(last);
        let mut g = Self {
            level,
            answered: 0,
            slips: 0,
            slip_run: 0,
            wrong: 0,
            solved: false,
            rank: None,
            teach,
            screen: Screen::Play,
            finished: None,
            question: Question::fresh(level, None),
            balloons: vec![],
            leave: None,
            asked: 0.0,
            streak_bump: Bump::default(),
            question_bump: Bump::default(),
            particles: Particles::default(),
            popups: Popups::default(),
            press: 0.0,
            t: 0.0,
        };
        if teach && level == 0 {
            g.question = Question::starter();
        }
        g.load_balloons();
        g
    }

    pub fn is_final_level(&self) -> bool {
        self.level + 1 == LEVEL_NAMES.len()
    }

    pub fn start_level(&mut self, level: usize) {
        self.level = level.min(LEVEL_NAMES.len() - 1);
        profile::set_level(KEY, self.level as u32);
        self.answered = 0;
        self.slips = 0;
        self.slip_run = 0;
        self.finished = None;
        self.rank = None;
        self.screen = Screen::Play;
        self.next_question();
    }

    fn load_balloons(&mut self) {
        let offset = rand::gen_range(0, BALLOON_COLORS.len());
        self.balloons = self
            .question
            .options
            .iter()
            .enumerate()
            .map(|(i, v)| Balloon {
                value: *v,
                color: (offset + i * 5) % BALLOON_COLORS.len(),
                wobble: 0.0,
                tried: false,
                popped: false,
            })
            .collect();
        self.wrong = 0;
        self.solved = false;
        self.leave = None;
        self.asked = 0.0;
        self.question_bump.kick();
    }

    fn next_question(&mut self) {
        self.question = Question::fresh(self.level, Some(&self.question));
        self.load_balloons();
    }

    fn revealed(&self) -> bool {
        self.wrong >= 2
    }

    fn complete_level(&mut self) {
        let stars = stars_for(self.slips);
        profile::award_stars(KEY, self.level as u32, stars);
        if !self.is_final_level() {
            profile::unlock(KEY, self.level as u32 + 1);
        } else {
            let total = profile::total_stars(KEY) as i64;
            if profile::personal_best(KEY).is_none_or(|b| total > b) {
                self.rank = profile::submit(KEY, total, profile::Better::Higher);
            }
        }
        if !self.is_final_level() {
            profile::set_level(KEY, self.level as u32 + 1);
        }
        self.finished = Some(stars);
    }

    fn advance(&mut self, view: &View) {
        if self.finished.take().is_some() {
            self.screen = if self.is_final_level() {
                Screen::Final(0.0)
            } else {
                Screen::Done(0.0)
            };
            return;
        }
        if self.slip_run >= DOWN_SLIPS && self.level > 0 {
            self.start_level(self.level - 1);
            self.popups.add(
                vec2(view.w / 2.0, view.h * 0.36),
                format!("Let's practise {}", LEVEL_NAMES[self.level]),
                WHITE,
                44.0,
            );
            return;
        }
        self.next_question();
    }

    pub fn pick(&mut self, slot: usize) -> Tap {
        let Some(b) = self.balloons.get(slot) else {
            return Tap::Ignored;
        };
        if b.popped || self.solved {
            return Tap::Ignored;
        }
        if b.value == self.question.answer {
            self.solved = true;
            self.balloons[slot].popped = true;
            self.leave = Some(0.0);
            self.answered += 1;
            if self.wrong > 0 {
                self.slips += 1;
                self.slip_run += 1;
            } else {
                self.slip_run = 0;
            }
            if self.teach {
                self.teach = false;
                profile::set(KEY, "played", 1);
            }
            if self.answered >= QUESTIONS {
                self.complete_level();
            }
            return Tap::Right;
        }
        if self.balloons[slot].tried {
            self.balloons[slot].wobble = 1.0;
            return Tap::Again;
        }
        self.balloons[slot].tried = true;
        self.balloons[slot].wobble = 1.0;
        self.wrong += 1;
        if self.revealed() {
            Tap::Reveal
        } else {
            Tap::Wrong
        }
    }

    fn center(&self, view: &View, slot: usize) -> Vec2 {
        let rest = balloon_rest(view, slot);
        let rise = ease_out_cubic((self.asked - slot as f32 * 0.1) / 0.9);
        let bob = (self.t * 1.6 + slot as f32 * 1.7).sin() * 8.0;
        let lift = self.leave.map_or(0.0, |l| l * l * (view.h + 400.0));
        let wob = (self.balloons[slot].wobble * 38.0).sin() * 14.0 * self.balloons[slot].wobble;
        vec2(
            rest.x + wob,
            rest.y + bob + (1.0 - rise) * (view.h - rest.y + 220.0) - lift,
        )
    }

    fn react(&mut self, view: &View, slot: usize, tap: Tap) {
        let c = self.center(view, slot);
        let color = BALLOON_COLORS[self.balloons[slot].color].1;
        let gold = Color::from_rgba(255, 220, 70, 255);
        match tap {
            Tap::Right => {
                let cleared = self.finished.is_some();
                sfx::play(Sfx::Pop);
                sfx::play(if cleared { Sfx::Win } else { Sfx::Right });
                if !cleared {
                    sfx::play_step(Sfx::Right, self.answered as usize * 2);
                }
                if cleared {
                    feel::impact_tinted(Impact::Large, Color::from_rgba(255, 230, 120, 255));
                }
                self.particles.emit(
                    c,
                    &[color, WHITE, gold],
                    Burst {
                        count: 26,
                        shape: Shape::Circle,
                        size: (5.0, 11.0),
                        ..Default::default()
                    },
                );
                self.particles.sparkle(c, gold);
                self.streak_bump.kick();
                sfx::say(if cleared && self.is_final_level() {
                    sfx::Say::Congrats
                } else if cleared {
                    sfx::Say::LevelUp
                } else if self.wrong == 0 {
                    sfx::Say::Great
                } else {
                    sfx::Say::Correct
                });
                let word = if self.wrong == 0 {
                    PRAISE[rand::gen_range(0, PRAISE.len())]
                } else {
                    "That's it!"
                };
                self.popups.add(c, word, gold, 60.0);
                if cleared {
                    let mid = vec2(view.w / 2.0, view.h * 0.36);
                    self.particles.confetti(view);
                    let title = if self.is_final_level() {
                        "You did it!"
                    } else {
                        "Level done!"
                    };
                    self.popups.add(mid, title, gold, 88.0);
                }
            }
            Tap::Wrong => {
                sfx::play(Sfx::Wrong);
                self.popups
                    .add(c, "Try again", Color::from_rgba(255, 170, 90, 255), 48.0);
            }
            Tap::Reveal => {
                sfx::play(Sfx::Wrong);
                let text = format!("It's {}", self.question.answer);
                self.popups
                    .add(c, text, Color::from_rgba(255, 170, 90, 255), 48.0);
            }
            Tap::Again | Tap::Ignored => {}
        }
    }

    fn play_input(&mut self, input: &Input, view: &View) {
        if input.tapped_in(levels_button(view)) {
            sfx::play(Sfx::Click);
            self.screen = Screen::Levels;
            return;
        }
        if self.leave.is_some() || !input.pressed {
            return;
        }
        let r = balloon_radius(view) * 1.1;
        let hit = (0..self.balloons.len()).find(|i| {
            !self.balloons[*i].popped && (input.pos - self.center(view, *i)).length() <= r.max(50.0)
        });
        if let Some(slot) = hit {
            let tap = self.pick(slot);
            self.react(view, slot, tap);
        }
    }

    fn levels_input(&mut self, input: &Input, view: &View) {
        if !input.pressed {
            return;
        }
        let l = levels_layout(view);
        if input.tapped_in(l.back) {
            sfx::play(Sfx::Click);
            if self.answered >= QUESTIONS {
                self.start_level(self.level);
            }
            self.screen = Screen::Play;
            return;
        }
        let open = profile::unlocked(KEY) as usize;
        for (i, tile) in l.tiles.iter().enumerate() {
            if tile.contains(input.pos) && i <= open {
                sfx::play(Sfx::Click);
                self.start_level(i);
                return;
            }
        }
    }

    fn done_input(&mut self, input: &Input, view: &View, age: f32) {
        if !input.pressed || age < 0.45 {
            return;
        }
        let l = done_layout(view);
        if input.tapped_in(l.next) {
            sfx::play(Sfx::Click);
            self.start_level(self.level + 1);
        } else if input.tapped_in(l.again) {
            sfx::play(Sfx::Click);
            self.start_level(self.level);
        } else if input.tapped_in(l.levels) {
            sfx::play(Sfx::Click);
            self.screen = Screen::Levels;
        }
    }

    fn final_input(&mut self, input: &Input, age: f32) {
        if input.pressed && age > 0.8 {
            self.start_level(self.level);
        }
    }
}

impl Default for MathPop {
    fn default() -> Self {
        Self::new()
    }
}

pub struct LevelsLayout {
    pub panel: Rect,
    pub tiles: Vec<Rect>,
    pub back: Rect,
}

pub fn levels_layout(view: &View) -> LevelsLayout {
    let gap = 14.0;
    let pw = (view.w - 40.0).min(780.0);
    let tw = (pw - 80.0 - gap * 2.0) / 3.0;
    let th = ((view.h - 60.0 - 270.0 - gap * 3.0) / 3.0).clamp(80.0, tw * 0.95);
    let ph = 120.0 + 3.0 * (th + gap) + 150.0;
    let c = view.center() + vec2(0.0, 20.0);
    let panel = Rect::new(c.x - pw / 2.0, c.y - ph / 2.0, pw, ph);
    let tiles = (0..LEVEL_NAMES.len())
        .map(|i| {
            Rect::new(
                panel.x + 40.0 + (i % 3) as f32 * (tw + gap),
                panel.y + 120.0 + (i / 3) as f32 * (th + gap),
                tw,
                th,
            )
        })
        .collect();
    let back = Rect::new(c.x - 150.0, panel.bottom() - 100.0, 300.0, 78.0);
    LevelsLayout { panel, tiles, back }
}

pub struct DoneLayout {
    pub panel: Rect,
    pub next: Rect,
    pub again: Rect,
    pub levels: Rect,
}

pub fn done_layout(view: &View) -> DoneLayout {
    let pw = 580.0f32.min(view.w - 40.0);
    let ph = 470.0;
    let c = view.center() + vec2(0.0, 20.0);
    let panel = Rect::new(c.x - pw / 2.0, c.y - ph / 2.0, pw, ph);
    let next = Rect::new(c.x - 170.0, panel.y + 285.0, 340.0, 80.0);
    let again = Rect::new(c.x - 215.0, panel.y + 380.0, 200.0, 60.0);
    let levels = Rect::new(c.x + 15.0, panel.y + 380.0, 200.0, 60.0);
    DoneLayout {
        panel,
        next,
        again,
        levels,
    }
}

pub fn levels_button(view: &View) -> Rect {
    let k = ui_scale(view);
    let w = 150.0 * k;
    Rect::new(view.w - 30.0 - w, 18.0, w, 46.0 * k)
}

fn plural_stars(n: i64) -> String {
    format!("{n} stars")
}

impl Game for MathPop {
    fn update(&mut self, input: &Input, view: &View, dt: f32) {
        self.t += dt;
        self.streak_bump.update(dt);
        self.question_bump.update(dt);
        self.press = approach(
            self.press,
            if input.down && levels_button(view).contains(input.pos) {
                1.0
            } else {
                0.0
            },
            20.0,
            dt,
        );
        self.particles.update(dt);
        self.popups.update(dt);
        match self.screen {
            Screen::Play => {
                self.asked += dt;
                for b in &mut self.balloons {
                    b.wobble = (b.wobble - dt * 1.6).max(0.0);
                }
                if let Some(l) = &mut self.leave {
                    *l += dt / LEAVE_TIME;
                    if *l >= 1.0 {
                        self.advance(view);
                    }
                } else {
                    self.play_input(input, view);
                }
            }
            Screen::Levels => self.levels_input(input, view),
            Screen::Done(age) => {
                self.screen = Screen::Done(age + dt);
                self.done_input(input, view, age);
            }
            Screen::Final(age) => {
                self.screen = Screen::Final(age + dt);
                self.final_input(input, age);
            }
        }
    }

    fn draw(&self, view: &View) {
        self.draw_backdrop(view);
        self.draw_hud(view);
        self.draw_question(view);
        let r = balloon_radius(view);
        let nudge = self.teach && self.asked > 6.0;
        for (i, b) in self.balloons.iter().enumerate() {
            if b.popped {
                continue;
            }
            let c = self.center(view, i);
            let hint = (self.revealed() || nudge) && b.value == self.question.answer;
            if hint {
                let pulse = (self.t * 6.0).sin();
                draw_circle(
                    c.x,
                    c.y,
                    r * (1.3 + 0.06 * pulse),
                    Color::new(1.0, 0.95, 0.4, 0.4 + 0.15 * pulse),
                );
            }
            draw_balloon(c, r, b, self.t + i as f32);
        }
        self.particles.draw();
        self.popups.draw();
        if self.teach && self.screen == Screen::Play && !self.solved {
            let pulse = 1.0 + (self.t * 4.0).sin() * 0.03;
            crate::ui::hint(
                view,
                "Pop the balloon with the answer!",
                vec2(view.w / 2.0, view.h - 56.0),
                pulse,
            );
        }
        match self.screen {
            Screen::Levels => self.draw_levels(view),
            Screen::Done(age) => self.draw_done(view, age),
            Screen::Final(_) => {
                let total = profile::total_stars(KEY);
                let line = format!("{total} of {} stars", LEVEL_NAMES.len() * 3);
                results(
                    view,
                    &crate::ui::Results {
                        game: KEY,
                        title: "You did it!",
                        line: &line,
                        rank: self.rank,
                        action: "Play again",
                        format: plural_stars,
                    },
                );
            }
            _ => {}
        }
    }
}

const GOLD: Color = Color::new(1.0, 0.82, 0.24, 1.0);
const PINK: Color = Color::new(0.92, 0.39, 0.67, 1.0);

fn star_row(c: Vec2, r: f32, gap: f32, have: u8, total: u8, pop: f32) {
    for i in 0..total {
        let x = c.x + (i as f32 - (total as f32 - 1.0) / 2.0) * gap;
        let lit = i < have;
        let k = if lit { pop } else { 1.0 };
        star(
            vec2(x, c.y + 2.0),
            r * 1.08 * k,
            0.0,
            Color::new(0.1, 0.05, 0.25, 0.3),
        );
        star(
            vec2(x, c.y),
            r * k,
            0.0,
            if lit {
                GOLD
            } else {
                Color::new(0.75, 0.72, 0.82, 0.8)
            },
        );
    }
}

impl MathPop {
    fn draw_hud(&self, view: &View) {
        let k = ui_scale(view);
        let label = format!("Level {}: {}", self.level + 1, LEVEL_NAMES[self.level]);
        let tw = text_size(&label, 28.0 * k).width + 44.0 * k;
        let chip = Rect::new(HUD_LEFT, 18.0, tw, 46.0 * k);
        round_rect(
            Rect::new(chip.x, chip.y + 4.0, chip.w, chip.h),
            chip.h / 2.0,
            Color::new(0.0, 0.0, 0.0, 0.18),
        );
        round_rect(chip, chip.h / 2.0, with_alpha(WHITE, 0.94));
        text_center(
            &label,
            chip.center().x,
            chip.center().y,
            28.0 * k,
            darker(PINK, 0.6),
        );
        let b = levels_button(view);
        crate::ui::painted_button(b, "Levels", "blue", self.press, 1.0);
        let bar_w = (view.w - 180.0).min(520.0 * k);
        let gap = 8.0 * k;
        let n = QUESTIONS;
        let seg = (bar_w - gap * (n as f32 - 1.0)) / n as f32;
        let x0 = view.w / 2.0 - bar_w / 2.0;
        for j in 0..n {
            let r = Rect::new(
                x0 + j as f32 * (seg + gap),
                chip.y + chip.h + 12.0,
                seg,
                18.0 * k,
            );
            let filled = j < self.answered;
            let last = filled && j + 1 == self.answered;
            let sc = if last {
                self.streak_bump.scale(0.35)
            } else {
                1.0
            };
            let rr = Rect::new(
                r.x + r.w * (1.0 - sc) / 2.0,
                r.y + r.h * (1.0 - sc) / 2.0,
                r.w * sc,
                r.h * sc,
            );
            round_rect(
                Rect::new(rr.x, rr.y + 3.0, rr.w, rr.h),
                rr.h / 2.0,
                Color::new(0.0, 0.0, 0.0, 0.15),
            );
            round_rect(
                rr,
                rr.h / 2.0,
                if filled { GOLD } else { with_alpha(WHITE, 0.5) },
            );
            if filled {
                gloss(rr, rr.h / 2.0);
            }
        }
    }

    fn draw_levels(&self, view: &View) {
        let l = levels_layout(view);
        fill_screen(view, Color::new(0.06, 0.04, 0.16, 0.55));
        crate::ui::painted_panel(l.panel, WHITE);
        crate::ui::ribbon(
            view,
            "Pick a level",
            vec2(view.w / 2.0, l.panel.y + 8.0),
            self.t,
        );
        let open = profile::unlocked(KEY) as usize;
        for (i, r) in l.tiles.iter().enumerate() {
            let locked = i > open;
            let current = i == self.level;
            let base = if locked {
                Color::from_rgba(200, 196, 214, 255)
            } else if current {
                Color::from_rgba(255, 214, 90, 255)
            } else {
                Color::from_rgba(255, 150, 200, 255)
            };
            round_rect(
                Rect::new(r.x, r.y + 5.0, r.w, r.h),
                16.0,
                darker(base, 0.65),
            );
            round_rect(*r, 16.0, base);
            text_outline(
                &(i + 1).to_string(),
                r.center().x,
                r.y + r.h * 0.33,
                r.h * 0.36,
                WHITE,
                with_alpha(darker(base, 0.45), if locked { 0.6 } else { 1.0 }),
            );
            if locked {
                padlock(vec2(r.center().x, r.y + r.h * 0.74), r.h * 0.2);
            } else {
                star_row(
                    vec2(r.center().x, r.y + r.h * 0.76),
                    r.h * 0.12,
                    r.h * 0.3,
                    profile::stars(KEY, i as u32),
                    3,
                    1.0,
                );
            }
        }
        text_center(
            LEVEL_NAMES[self.level],
            view.w / 2.0,
            l.back.y - 26.0,
            30.0,
            BODY,
        );
        crate::ui::painted_button(l.back, "Back", "green", 0.0, 1.0);
    }

    fn draw_done(&self, view: &View, age: f32) {
        let l = done_layout(view);
        let fade = ease_out_cubic(age / 0.2);
        fill_screen(view, Color::new(0.06, 0.04, 0.16, 0.5 * fade));
        let s = ease_out_back(age / 0.32).max(0.01);
        let c = l.panel.center();
        let p = Rect::new(
            c.x - l.panel.w * s / 2.0,
            c.y - l.panel.h * s / 2.0,
            l.panel.w * s,
            l.panel.h * s,
        );
        crate::ui::painted_panel(p, WHITE);
        crate::ui::ribbon(view, "Level done!", vec2(c.x, p.y + 8.0 * s), age);
        let name = format!("{}: {}", self.level + 1, LEVEL_NAMES[self.level]);
        text_center(&name, c.x, p.y + 100.0 * s, 32.0 * s, BODY);
        let have = stars_for(self.slips);
        let pop = |i: u8| ease_out_back(((age - 0.25 - i as f32 * 0.18) / 0.3).clamp(0.0, 1.0));
        for i in 0..3u8 {
            let x = c.x + (i as f32 - 1.0) * 100.0 * s;
            let lit = i < have;
            let k = if lit { pop(i) } else { 1.0 };
            let r = 46.0 * s * k;
            star(
                vec2(x, p.y + 190.0 * s + 3.0),
                r * 1.05,
                0.0,
                Color::new(0.1, 0.05, 0.25, 0.3),
            );
            star(
                vec2(x, p.y + 190.0 * s),
                r,
                0.0,
                if lit {
                    GOLD
                } else {
                    Color::new(0.75, 0.72, 0.82, 0.8)
                },
            );
        }
        let a = ((age - 0.3) / 0.2).clamp(0.0, 1.0);
        let pulse = 1.0 + (age * 4.5).sin() * 0.03;
        let scaled = |r: Rect, k: f32| {
            let cc = r.center();
            Rect::new(cc.x - r.w * k / 2.0, cc.y - r.h * k / 2.0, r.w * k, r.h * k)
        };
        crate::ui::painted_button(scaled(l.next, pulse), "Next level", "green", 0.0, a);
        crate::ui::painted_button(l.again, "Again", "orange", 0.0, a);
        crate::ui::painted_button(l.levels, "Levels", "blue", 0.0, a);
    }

    fn draw_backdrop(&self, view: &View) {
        let f = view.full();
        let side = f.w.max(f.h);
        art::sprite(
            "bg",
            Rect::new(f.x + (f.w - side) / 2.0, f.y + f.h - side, side, side),
        );
        for i in 0..5usize {
            let span = view.w + 500.0;
            let x = (i as f32 * 0.23 * span + self.t * (7.0 + i as f32 * 3.0)) % span - 250.0;
            let y = hud_bottom(view) + 20.0 + (i as f32 * 0.37 % 1.0) * view.h * 0.35;
            crate::ui::cloud(
                i * 2,
                vec2(x, y),
                150.0 + (i % 3) as f32 * 50.0,
                with_alpha(WHITE, 0.9),
            );
        }
        self.draw_bunnies(view);
    }

    fn mood(&self) -> &'static str {
        if self.leave.is_some() {
            "up"
        } else if self.balloons.iter().any(|b| b.wobble > 0.3) {
            "sad"
        } else {
            "stand"
        }
    }

    fn draw_bunnies(&self, view: &View) {
        let h = 150.0 * ui_scale(view);
        let hop = match self.leave {
            Some(l) => (l * 14.0).sin().abs() * h * 0.2,
            None => 0.0,
        };
        let names = match self.mood() {
            "up" => ["bunny_up", "pink_up"],
            "sad" => ["bunny_sad", "pink_sad"],
            _ => ["bunny", "pink"],
        };
        let y = view.h - h * 0.5 - 10.0 - hop;
        for (i, name) in names.iter().enumerate() {
            let side = if i == 0 { 1.0 } else { -1.0 };
            let x = if i == 0 { 70.0 } else { view.w - 70.0 };
            art::sprite_at(
                name,
                vec2(x, y),
                h,
                crate::art::Draw {
                    flip_x: side < 0.0,
                    ..Default::default()
                },
            );
        }
    }

    fn draw_question(&self, view: &View) {
        let solved = self.solved;
        let text = self.question.text(solved);
        let size = question_text_size(view);
        let pop = ease_out_back(self.question_bump.t).max(0.2);
        let base = question_rect(view);
        let c = base.center();
        let r = Rect::new(
            c.x - base.w * pop / 2.0,
            c.y - base.h * pop / 2.0,
            base.w * pop,
            base.h * pop,
        );
        crate::ui::painted_panel(r, WHITE);
        let ink = if solved {
            Color::from_rgba(60, 170, 90, 255)
        } else {
            Color::from_rgba(70, 60, 130, 255)
        };
        text_center(&text, c.x, c.y, size * pop, ink);
    }
}

fn draw_balloon(c: Vec2, r: f32, b: &Balloon, t: f32) {
    let (sprite, color) = BALLOON_COLORS[b.color];
    let fade = if b.tried { 0.5 } else { 1.0 };
    let squash = 1.0 + 0.06 * (b.wobble * 30.0).sin() * b.wobble;
    let ry = r / squash;
    let height = ry * 3.1;
    let width = height * art::size(sprite).x / art::size(sprite).y * squash;
    let rect = Rect::new(c.x - width / 2.0, c.y - ry * 1.0, width, height);
    art::sprite_ex(
        sprite,
        rect,
        crate::art::Draw {
            tint: with_alpha(
                if b.tried {
                    Color::new(0.8, 0.8, 0.85, 1.0)
                } else {
                    WHITE
                },
                fade,
            ),
            rotation: (t * 1.3).sin() * 0.05,
            pivot: vec2(0.5, 0.0),
            ..Default::default()
        },
    );
    text_outline(
        &b.value.to_string(),
        c.x,
        c.y - ry * 0.08,
        r * 0.95,
        with_alpha(WHITE, fade),
        with_alpha(darker(color, 0.55), fade),
    );
}

fn padlock(c: Vec2, r: f32) {
    let ink = Color::from_rgba(120, 112, 150, 255);
    draw_circle_lines(c.x, c.y - r * 0.55, r * 0.5, r * 0.18, ink);
    round_rect(
        Rect::new(c.x - r * 0.7, c.y - r * 0.3, r * 1.4, r * 1.0),
        r * 0.2,
        ink,
    );
}

pub fn icon(r: Rect) {
    crate::ui::icon("mathpop", r);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mathpop_sprites_all_exist() {
        let have = art::names();
        for n in [
            "bg",
            "balloon_red",
            "balloon_orange",
            "balloon_yellow",
            "balloon_green",
            "balloon_blue",
            "balloon_purple",
            "bunny",
            "bunny_up",
            "bunny_sad",
            "pink",
            "pink_up",
            "pink_sad",
        ] {
            assert!(have.iter().any(|h| h == n), "missing {n}");
        }
    }

    fn fresh_player() {
        crate::save::reset_for_test();
        crate::profile::create("Mia", 0);
    }

    fn pop_right(g: &mut MathPop, v: &View) {
        let slot = g.question.slot_of_answer();
        let tap = g.pick(slot);
        g.react(v, slot, tap);
    }

    fn pop_wrong(g: &mut MathPop, v: &View) -> Tap {
        let slot = wrong_slot(g);
        let tap = g.pick(slot);
        g.react(v, slot, tap);
        tap
    }

    fn settle(g: &mut MathPop, v: &View, secs: f32) {
        for _ in 0..(secs * 60.0) as usize {
            g.update(&Input::default(), v, 1.0 / 60.0);
        }
    }

    fn clear_level(g: &mut MathPop, v: &View, miss_first: bool) {
        for n in 0..QUESTIONS {
            if miss_first && n == 0 {
                pop_wrong(g, v);
            }
            pop_right(g, v);
            settle(g, v, 1.2);
            if g.screen != Screen::Play {
                return;
            }
        }
    }

    #[test]
    fn right_answers_are_praised_aloud_and_wrong_ones_are_not() {
        fresh_player();
        let v = View::ipad_landscape();
        let mut g = MathPop::new();
        pop_wrong(&mut g, &v);
        assert_eq!(crate::sfx::take_voice(), None);
        pop_right(&mut g, &v);
        assert_eq!(crate::sfx::take_voice(), Some(sfx::Say::Correct));
    }

    #[test]
    fn first_try_answer_says_great_and_clearing_a_level_says_so() {
        fresh_player();
        let v = View::ipad_landscape();
        let mut g = MathPop::new();
        pop_right(&mut g, &v);
        assert_eq!(crate::sfx::take_voice(), Some(sfx::Say::Great));
        g.answered = QUESTIONS - 1;
        g.next_question();
        pop_right(&mut g, &v);
        assert_eq!(crate::sfx::take_voice(), Some(sfx::Say::LevelUp));
    }

    #[test]
    fn bunnies_cheer_after_a_pop_and_wince_after_a_miss() {
        fresh_player();
        let mut g = MathPop::new();
        assert_eq!(g.mood(), "stand");
        let wrong = wrong_slot(&g);
        g.pick(wrong);
        assert_eq!(g.mood(), "sad");
        let slot = g.question.slot_of_answer();
        g.pick(slot);
        assert_eq!(g.mood(), "up");
    }

    fn assert_sound(q: &Question) {
        assert_eq!(q.answer, solve(q.a, q.b, q.op));
        assert_eq!(q.options.len(), BALLOONS);
        assert_eq!(q.options.iter().filter(|v| **v == q.answer).count(), 1);
        let mut sorted = q.options.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), BALLOONS, "{q:?}");
        assert!(q.options.iter().all(|v| *v >= 0), "{q:?}");
        assert!(q.answer >= 1);
    }

    #[test]
    fn every_level_makes_sound_questions() {
        rand::srand(7);
        for level in 0..LEVEL_NAMES.len() {
            for _ in 0..400 {
                assert_sound(&Question::generate(level));
            }
        }
    }

    #[test]
    fn edge_questions_still_find_four_distinct_choices() {
        for (a, b, op) in [
            (1, 1, Op::Add),
            (3, 2, Op::Sub),
            (3, 1, Op::Sub),
            (2, 1, Op::Mul),
            (10, 10, Op::Mul),
            (18, 2, Op::Add),
            (20, 19, Op::Sub),
        ] {
            for _ in 0..50 {
                assert_sound(&Question::from_parts(a, b, op));
            }
        }
    }

    #[test]
    fn adding_levels_stay_inside_their_limits() {
        rand::srand(11);
        for _ in 0..400 {
            let q = Question::generate(0);
            assert!(q.op == Op::Add && q.a >= 1 && q.b >= 1 && q.answer <= 5);
            let q = Question::generate(1);
            assert!(q.op == Op::Add && (6..=10).contains(&q.answer) && q.b >= 1);
            let q = Question::generate(2);
            assert!(q.op == Op::Add && (11..=20).contains(&q.answer) && q.b >= 1);
        }
    }

    #[test]
    fn taking_away_never_goes_negative() {
        rand::srand(13);
        for _ in 0..400 {
            let q = Question::generate(3);
            assert!(q.op == Op::Sub && q.b < q.a && q.answer >= 1 && q.a <= 10);
            let q = Question::generate(4);
            assert!(q.op == Op::Sub && q.b < q.a && q.answer >= 1 && (11..=20).contains(&q.a));
        }
    }

    #[test]
    fn mixed_level_uses_every_adding_and_taking_away_op() {
        rand::srand(17);
        let ops: Vec<Op> = (0..300).map(|_| Question::generate(5).op).collect();
        assert!(ops.contains(&Op::Add) && ops.contains(&Op::Sub));
        assert!(!ops.contains(&Op::Mul));
    }

    #[test]
    fn times_tables_stay_up_to_ten() {
        rand::srand(19);
        for _ in 0..400 {
            let q = Question::generate(7);
            assert!(q.op == Op::Mul && (2..=10).contains(&q.a) && (1..=10).contains(&q.b));
        }
    }

    #[test]
    fn the_next_question_differs_from_the_last() {
        rand::srand(23);
        let mut q = Question::fresh(0, None);
        for _ in 0..200 {
            let next = Question::fresh(0, Some(&q));
            assert_ne!((q.a, q.b, q.op), (next.a, next.b, next.op));
            q = next;
        }
    }

    #[test]
    fn question_text_hides_then_shows_the_answer() {
        let q = Question::from_parts(7, 5, Op::Add);
        assert_eq!(q.text(false), "7 + 5 = ?");
        assert_eq!(q.text(true), "7 + 5 = 12");
        assert_eq!(q.options[q.slot_of_answer()], 12);
    }

    fn wrong_slot(g: &MathPop) -> usize {
        (0..BALLOONS)
            .find(|i| *i != g.question.slot_of_answer() && !g.balloons[*i].tried)
            .unwrap()
    }

    #[test]
    fn tapping_the_right_balloon_pops_it_and_starts_leaving() {
        fresh_player();
        let mut g = MathPop::new();
        let slot = g.question.slot_of_answer();
        assert_eq!(g.pick(slot), Tap::Right);
        assert!(g.balloons[slot].popped);
        assert!(g.leave.is_some());
        assert_eq!(g.answered, 1);
    }

    #[test]
    fn a_wrong_tap_wobbles_and_a_second_one_shows_the_answer() {
        fresh_player();
        let mut g = MathPop::new();
        let first = wrong_slot(&g);
        assert_eq!(g.pick(first), Tap::Wrong);
        assert_eq!(g.balloons[first].wobble, 1.0);
        assert!(!g.revealed());
        let second = wrong_slot(&g);
        assert_eq!(g.pick(second), Tap::Reveal);
        assert!(g.revealed());
        assert!(g.leave.is_none());
    }

    #[test]
    fn tapping_during_the_exit_is_ignored() {
        fresh_player();
        let v = View::ipad_landscape();
        let mut g = MathPop::new();
        let slot = g.question.slot_of_answer();
        g.pick(slot);
        g.asked = 5.0;
        let other = wrong_slot(&g);
        let at = g.center(&v, other);
        g.update(&Input::press_at(at), &v, 1.0 / 60.0);
        assert_eq!(g.wrong, 0);
    }

    #[test]
    fn a_touch_on_a_balloon_picks_it() {
        fresh_player();
        let v = View::ipad_portrait();
        let mut g = MathPop::new();
        g.asked = 5.0;
        let slot = g.question.slot_of_answer();
        let at = g.center(&v, slot);
        g.update(&Input::press_at(at), &v, 1.0 / 60.0);
        assert!(g.solved);
    }

    #[test]
    fn a_new_player_starts_with_a_tiny_sum_and_a_hint_until_the_first_pop() {
        fresh_player();
        let mut g = MathPop::new();
        assert_eq!(g.level, 0);
        assert!(g.teach);
        assert!(g.question.answer <= 4 && g.question.op == Op::Add);
        pop_right(&mut g, &View::ipad_landscape());
        assert!(!g.teach);
        assert_eq!(profile::get(KEY, "played"), 1);
        assert!(!MathPop::new().teach);
    }

    #[test]
    fn nothing_is_lost_before_the_first_success() {
        fresh_player();
        let v = View::ipad_landscape();
        let mut g = MathPop::new();
        for _ in 0..4 {
            pop_wrong(&mut g, &v);
            if g.revealed() {
                break;
            }
        }
        assert!(g.revealed());
        assert_eq!(g.level, 0);
        assert_eq!(g.screen, Screen::Play);
    }

    #[test]
    fn five_pops_clear_a_level_award_stars_and_unlock_the_next() {
        fresh_player();
        let v = View::ipad_landscape();
        let mut g = MathPop::new();
        clear_level(&mut g, &v, false);
        assert!(matches!(g.screen, Screen::Done(_)));
        assert_eq!(profile::stars(KEY, 0), 3);
        assert_eq!(profile::unlocked(KEY), 1);
        assert_eq!(profile::level(KEY), 1);
    }

    #[test]
    fn stars_fall_with_the_slips_and_never_below_one() {
        assert_eq!(stars_for(0), 3);
        assert_eq!(stars_for(1), 2);
        assert_eq!(stars_for(2), 2);
        assert_eq!(stars_for(5), 1);
        fresh_player();
        let v = View::ipad_landscape();
        let mut g = MathPop::new();
        clear_level(&mut g, &v, true);
        assert_eq!(profile::stars(KEY, 0), 2);
    }

    #[test]
    fn a_better_run_raises_stars_but_a_worse_one_keeps_them() {
        fresh_player();
        let v = View::ipad_landscape();
        let mut g = MathPop::new();
        clear_level(&mut g, &v, false);
        g.start_level(0);
        clear_level(&mut g, &v, true);
        assert_eq!(profile::stars(KEY, 0), 3);
    }

    #[test]
    fn a_returning_child_resumes_at_their_level_not_the_easiest() {
        fresh_player();
        let v = View::ipad_landscape();
        let mut g = MathPop::new();
        clear_level(&mut g, &v, false);
        let g = MathPop::new();
        assert_eq!(g.level, 1);
        assert!(g.question.answer >= 2);
        assert!(!g.teach);
    }

    #[test]
    fn progress_is_kept_per_player() {
        fresh_player();
        let v = View::ipad_landscape();
        let mut g = MathPop::new();
        clear_level(&mut g, &v, false);
        crate::profile::create("Leo", 3);
        assert_eq!(MathPop::new().level, 0);
        assert_eq!(profile::unlocked(KEY), 0);
    }

    #[test]
    fn a_saved_level_past_what_is_unlocked_is_clamped() {
        fresh_player();
        profile::set_level(KEY, 6);
        assert_eq!(MathPop::new().level, 0);
        profile::unlock(KEY, 6);
        assert_eq!(MathPop::new().level, 6);
    }

    #[test]
    fn three_shaky_answers_in_a_row_drop_back_one_level() {
        fresh_player();
        let v = View::ipad_landscape();
        profile::unlock(KEY, 3);
        profile::set(KEY, "played", 1);
        let mut g = MathPop::new();
        g.start_level(3);
        for _ in 0..DOWN_SLIPS {
            pop_wrong(&mut g, &v);
            pop_right(&mut g, &v);
            settle(&mut g, &v, 1.2);
        }
        assert_eq!(g.level, 2);
        assert_eq!(g.answered, 0);
        assert_eq!(profile::level(KEY), 2);
        assert_eq!(profile::unlocked(KEY), 3);
    }

    #[test]
    fn a_clean_answer_breaks_the_run_of_slips() {
        fresh_player();
        let v = View::ipad_landscape();
        profile::unlock(KEY, 3);
        let mut g = MathPop::new();
        g.start_level(3);
        for _ in 0..DOWN_SLIPS - 1 {
            pop_wrong(&mut g, &v);
            pop_right(&mut g, &v);
            settle(&mut g, &v, 1.2);
        }
        pop_right(&mut g, &v);
        settle(&mut g, &v, 1.2);
        pop_wrong(&mut g, &v);
        pop_right(&mut g, &v);
        settle(&mut g, &v, 1.2);
        assert_eq!(g.level, 3);
    }

    #[test]
    fn level_one_never_drops_below_itself() {
        fresh_player();
        let v = View::ipad_landscape();
        let mut g = MathPop::new();
        for _ in 0..5 {
            pop_wrong(&mut g, &v);
            pop_right(&mut g, &v);
            settle(&mut g, &v, 1.2);
        }
        assert_eq!(g.level, 0);
    }

    #[test]
    fn content_steps_up_with_each_level() {
        rand::srand(5);
        let top = |level: usize| {
            (0..300)
                .map(|_| {
                    let q = Question::generate(level);
                    q.a.max(q.b).max(q.answer)
                })
                .max()
                .unwrap()
        };
        assert!(top(0) < top(1) && top(1) < top(2));
        assert!(top(3) <= 10 && top(4) > 10);
        assert!(top(6) <= 100 && top(7) <= 100);
        let twos: Vec<i32> = (0..300)
            .map(|_| Question::generate(6))
            .flat_map(|q| [q.a, q.b])
            .collect();
        assert!(twos.iter().all(|v| [2, 5, 10].contains(v) || *v <= 10));
    }

    #[test]
    fn the_level_picker_only_opens_unlocked_levels() {
        fresh_player();
        let v = View::ipad_landscape();
        profile::unlock(KEY, 2);
        let mut g = MathPop::new();
        g.screen = Screen::Levels;
        let l = levels_layout(&v);
        g.update(&Input::press_at(l.tiles[5].center()), &v, 1.0 / 60.0);
        assert_eq!(g.screen, Screen::Levels);
        g.update(&Input::press_at(l.tiles[2].center()), &v, 1.0 / 60.0);
        assert_eq!(g.screen, Screen::Play);
        assert_eq!(g.level, 2);
        assert_eq!(profile::level(KEY), 2);
    }

    #[test]
    fn the_levels_button_opens_the_picker_and_back_returns() {
        fresh_player();
        let v = View::ipad_landscape();
        let mut g = MathPop::new();
        g.update(&Input::press_at(levels_button(&v).center()), &v, 1.0 / 60.0);
        assert_eq!(g.screen, Screen::Levels);
        let l = levels_layout(&v);
        g.update(&Input::press_at(l.back.center()), &v, 1.0 / 60.0);
        assert_eq!(g.screen, Screen::Play);
    }

    #[test]
    fn backing_out_of_the_picker_after_a_cleared_level_restarts_it() {
        fresh_player();
        let v = View::ipad_landscape();
        let mut g = MathPop::new();
        clear_level(&mut g, &v, false);
        g.screen = Screen::Levels;
        let l = levels_layout(&v);
        g.update(&Input::press_at(l.back.center()), &v, 1.0 / 60.0);
        assert_eq!((g.screen, g.answered), (Screen::Play, 0));
        pop_right(&mut g, &v);
        assert_eq!(g.finished, None);
    }

    #[test]
    fn the_done_screen_buttons_go_next_again_or_to_levels() {
        fresh_player();
        let v = View::ipad_landscape();
        let l = done_layout(&v);
        for (r, want_level, want_screen) in
            [(l.next, 1, false), (l.again, 0, false), (l.levels, 0, true)]
        {
            crate::save::reset_for_test();
            crate::profile::create("Mia", 0);
            let mut g = MathPop::new();
            clear_level(&mut g, &v, false);
            settle(&mut g, &v, 1.0);
            g.update(&Input::press_at(r.center()), &v, 1.0 / 60.0);
            assert_eq!(g.level, want_level);
            assert_eq!(g.screen == Screen::Levels, want_screen);
            assert_eq!(g.answered, if want_screen { QUESTIONS } else { 0 });
        }
    }

    #[test]
    fn clearing_the_last_level_submits_stars_to_the_board() {
        fresh_player();
        let v = View::ipad_landscape();
        let last = LEVEL_NAMES.len() - 1;
        profile::unlock(KEY, last as u32);
        let mut g = MathPop::new();
        g.start_level(last);
        clear_level(&mut g, &v, false);
        assert!(matches!(g.screen, Screen::Final(_)));
        assert_eq!(g.rank, Some(0));
        assert_eq!(profile::board(KEY)[0].score, 3);
    }

    #[test]
    fn level_names_match_the_questions_they_ask() {
        assert_eq!(LEVEL_NAMES.len(), 9);
        rand::srand(29);
        for level in 0..LEVEL_NAMES.len() {
            for _ in 0..50 {
                let q = Question::generate(level);
                assert_eq!(q.answer, solve(q.a, q.b, q.op));
            }
        }
    }

    fn phone() -> View {
        View::fit(1080.0, 2400.0)
    }

    #[test]
    fn balloons_are_big_touch_targets_and_fit_on_screen() {
        for v in [
            View::ipad_landscape(),
            View::ipad_portrait(),
            View::fit(390.0, 844.0),
            View::fit(1920.0, 1080.0),
            phone(),
        ] {
            let r = balloon_radius(&v);
            assert!(r * 2.0 >= 90.0 * 1.5);
            let top = question_rect(&v).bottom();
            for slot in 0..BALLOONS {
                let c = balloon_rest(&v, slot);
                assert!(c.x - r >= 0.0 && c.x + r <= v.w, "{v:?}");
                assert!(c.y - r >= top && c.y + r * 2.3 <= v.h, "{v:?}");
            }
            let (cols, _) = grid(&v);
            for slot in 1..BALLOONS {
                if slot % cols != 0 {
                    let gap = balloon_rest(&v, slot).x - balloon_rest(&v, slot - 1).x;
                    assert!(gap > r * 1.8);
                }
            }
        }
    }

    #[test]
    fn on_a_tall_phone_the_balloons_use_most_of_the_height() {
        let v = phone();
        assert!(v.h > 2000.0);
        let r = balloon_radius(&v);
        assert!(r >= 140.0, "{r}");
        let ys: Vec<f32> = (0..BALLOONS).map(|i| balloon_rest(&v, i).y).collect();
        let top = ys.iter().cloned().fold(f32::MAX, f32::min) - r;
        let bottom = ys.iter().cloned().fold(f32::MIN, f32::max) + r * 2.3;
        assert!(bottom - top >= v.h * 0.5, "{top} {bottom} of {}", v.h);
        assert!(question_rect(&v).h >= 150.0);
        assert!(question_text_size(&v) >= 120.0);
    }
}
