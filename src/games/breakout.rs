use crate::engine::*;
use crate::feel::{self, Impact};
use crate::fx::*;
use crate::profile;
use crate::sfx::{self, Sfx};
use macroquad::prelude::*;

pub const KEY: &str = "breakout";
pub const ROWS: usize = 6;
pub const COLS: usize = 10;
pub const PADDLE_EASE: f32 = 30.0;
pub const BALL_R: f32 = 11.0;
pub const PADDLE_W: f32 = 170.0;
pub const PADDLE_H: f32 = 27.0;
pub const LIVES: u32 = 3;
pub const LEVELS: usize = 12;
pub const FORGIVING_LEVELS: u32 = 2;
pub const MAX_ASSIST: u32 = 3;
const TAP_DELAY: f32 = 0.7;
const LAUNCH_DELAY: f32 = 0.35;

pub const LAYOUTS: [[&str; ROWS]; LEVELS] = [
    [
        "..........",
        "..........",
        "...2222...",
        "..333333..",
        "..........",
        "..........",
    ],
    [
        "..........",
        "..111111..",
        "..222222..",
        "..333333..",
        "..........",
        "..........",
    ],
    [
        "..........",
        "0.0.0.0.0.",
        ".1.1.1.1.1",
        "2.2.2.2.2.",
        ".3.3.3.3.3",
        "..........",
    ],
    [
        "....00....",
        "...1111...",
        "..222222..",
        ".33333333.",
        "4444444444",
        "..........",
    ],
    [
        "0000000000",
        "..........",
        "2222222222",
        "..........",
        "4444444444",
        "..........",
    ],
    [
        "..00..00..",
        ".00000000.",
        ".00000000.",
        "..000000..",
        "...0000...",
        "....00....",
    ],
    [
        "0000..0000",
        "1111..1111",
        "2222..2222",
        "3333..3333",
        "4444..4444",
        "..........",
    ],
    [
        "...0000...",
        "..111111..",
        ".22222222.",
        ".33333333.",
        "..444444..",
        "...5555...",
    ],
    [
        "0.0.0.0.0.",
        ".1.1.1.1.1",
        "2.2.2.2.2.",
        ".3.3.3.3.3",
        "4.4.4.4.4.",
        ".5.5.5.5.5",
    ],
    [
        "0000000000",
        "1........1",
        "2..3333..2",
        "3..4444..3",
        "4........4",
        "5555555555",
    ],
    [
        "0000000000",
        "1111111111",
        "2222222222",
        "3333333333",
        "4444444444",
        "..........",
    ],
    [
        "0000000000",
        "1111111111",
        "2222222222",
        "3333333333",
        "4444444444",
        "5555555555",
    ],
];

pub fn layout_tints(level: u32) -> Vec<Option<usize>> {
    let rows = &LAYOUTS[(level.clamp(1, LEVELS as u32) - 1) as usize];
    rows.iter()
        .flat_map(|r| {
            r.chars()
                .map(|c| c.to_digit(10).map(|d| d as usize % ROWS))
                .collect::<Vec<_>>()
        })
        .collect()
}

pub fn stars_for(misses: u32) -> u8 {
    3u32.saturating_sub(misses).max(1) as u8
}

pub fn base_speed(level: u32) -> f32 {
    300.0 + 22.0 * (level.max(1) - 1) as f32
}

pub fn base_paddle_w(level: u32) -> f32 {
    (260.0 - 9.0 * (level.max(1) - 1) as f32).max(160.0)
}

pub fn level_tiles(view: &View) -> Vec<Rect> {
    let cols = if view.w >= view.h { 4 } else { 3 };
    let rows = LEVELS.div_ceil(cols);
    let gap = 18.0;
    let top = 150.0;
    let avail_w = view.w - 80.0;
    let avail_h = view.h - top - 40.0;
    let s = ((avail_w - gap * (cols - 1) as f32) / cols as f32)
        .min((avail_h - gap * (rows - 1) as f32) / rows as f32)
        .min(190.0);
    let gw = s * cols as f32 + gap * (cols - 1) as f32;
    let gh = s * rows as f32 + gap * (rows - 1) as f32;
    let x0 = (view.w - gw) / 2.0;
    let y0 = top + (avail_h - gh) / 2.0;
    (0..LEVELS)
        .map(|i| {
            Rect::new(
                x0 + (i % cols) as f32 * (s + gap),
                y0 + (i / cols) as f32 * (s + gap),
                s,
                s,
            )
        })
        .collect()
}

pub fn panel_rect(view: &View) -> Rect {
    let c = view.center() + vec2(0.0, 20.0);
    Rect::new(c.x - 290.0, c.y - 220.0, 580.0, 440.0)
}

pub fn next_button(view: &View) -> Rect {
    let p = panel_rect(view);
    Rect::new(p.x + p.w / 2.0 - 160.0, p.y + 268.0, 320.0, 78.0)
}

pub fn levels_button(view: &View) -> Rect {
    let p = panel_rect(view);
    Rect::new(p.x + p.w / 2.0 - 100.0, p.y + 356.0, 200.0, 64.0)
}

pub fn serve_levels_button(view: &View) -> Rect {
    Rect::new(view.w / 2.0 - 110.0, view.h * 0.74 - 32.0, 220.0, 64.0)
}

pub fn lost_levels_button(view: &View) -> Rect {
    let rows = profile::board(KEY).len().min(crate::ui::RESULT_ROWS);
    let panel_h = 230.0 + rows as f32 * 46.0 + if rows == 0 { 0.0 } else { 20.0 };
    let y = (view.h / 2.0 + 24.0 + panel_h / 2.0 + 30.0).min(view.h - 68.0);
    Rect::new(view.w / 2.0 - 100.0, y, 200.0, 58.0)
}

pub fn career_score() -> i64 {
    (1..=LEVELS as u32)
        .map(|l| profile::get(KEY, &format!("score.{l}")))
        .sum()
}
const TRAIL_LEN: usize = 14;
const COMBO_WINDOW: f32 = 1.2;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Event {
    BrickBroken {
        at: Vec2,
        row: usize,
        points: u32,
        combo: u32,
    },
    PaddleHit(Vec2),
    WallHit(Vec2),
    Lost(Vec2),
    LevelCleared,
}

pub fn combo_points(combo: u32) -> u32 {
    10 * combo.clamp(1, 8)
}

pub fn squash_scale(t: f32) -> Vec2 {
    let k = (1.0 - ease_out_cubic(t / 0.3)).max(0.0);
    vec2(1.0 + 0.25 * k, 1.0 - 0.45 * k)
}

pub fn brick_color(row: usize) -> Color {
    [
        Color::from_rgba(238, 107, 107, 255),
        Color::from_rgba(255, 144, 72, 255),
        Color::from_rgba(255, 220, 60, 255),
        Color::from_rgba(150, 215, 60, 255),
        Color::from_rgba(80, 190, 250, 255),
        Color::from_rgba(255, 170, 215, 255),
    ][row % ROWS]
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum State {
    Serve,
    Playing,
    Lost,
    Cleared,
    Select,
}

pub struct Breakout {
    pub bricks: Vec<Option<Rect>>,
    pub ball: Vec2,
    pub vel: Vec2,
    pub paddle_x: f32,
    pub lives: u32,
    pub score: u32,
    pub level: u32,
    pub state: State,
    pub combo: u32,
    pub events: Vec<Event>,
    pub tints: Vec<usize>,
    pub total: usize,
    pub misses: u32,
    pub assist: u32,
    pub rank: Option<usize>,
    pub age: f32,
    settled: bool,
    touched: bool,
    stale: u32,
    prev_state: State,
    size: Vec2,
    since_break: f32,
    paddle_age: f32,
    clock: f32,
    trail: Vec<Vec2>,
    pops: Vec<f32>,
    particles: Particles,
    popups: Popups,
    shake: Shake,
    bump: Bump,
}

impl Breakout {
    pub fn new() -> Self {
        let mut b = Self {
            bricks: vec![],
            ball: Vec2::ZERO,
            vel: Vec2::ZERO,
            paddle_x: 0.0,
            lives: LIVES,
            score: 0,
            level: 1,
            state: State::Serve,
            combo: 0,
            events: vec![],
            tints: vec![],
            total: 0,
            misses: 0,
            assist: 0,
            rank: None,
            age: 0.0,
            settled: false,
            touched: false,
            stale: 0,
            prev_state: State::Serve,
            size: Vec2::ZERO,
            since_break: 10.0,
            paddle_age: 10.0,
            clock: 0.0,
            trail: vec![],
            pops: vec![],
            particles: Particles::default(),
            popups: Popups::default(),
            shake: Shake::default(),
            bump: Bump::default(),
        };
        let open = profile::unlocked(KEY).clamp(1, LEVELS as u32);
        b.assist = profile::get(KEY, "assist").clamp(0, MAX_ASSIST as i64) as u32;
        b.level = profile::level(KEY).clamp(1, open);
        b.start_level(b.level, View::fit(MIN_W, MIN_H));
        if open >= 2 {
            b.state = State::Select;
            b.prev_state = State::Select;
        }
        b
    }

    pub fn forgiving(&self) -> bool {
        self.level <= FORGIVING_LEVELS
    }

    pub fn paddle_w(&self) -> f32 {
        base_paddle_w(self.level) * (1.0 + 0.15 * self.assist as f32)
    }

    fn paddle_h(&self) -> f32 {
        self.paddle_w() * PADDLE_H / PADDLE_W
    }

    pub fn start_lives(&self) -> u32 {
        LIVES + (self.assist >= 2) as u32
    }

    pub fn cleared(&self) -> usize {
        self.total - self.bricks.iter().flatten().count()
    }

    pub fn first_win(&self) -> bool {
        profile::unlocked(KEY) >= 2
    }

    pub fn start_level(&mut self, level: u32, view: View) {
        self.level = level.clamp(1, LEVELS as u32);
        self.score = 0;
        self.misses = 0;
        self.rank = None;
        self.settled = false;
        self.lives = self.start_lives();
        self.combo = 0;
        self.events.clear();
        if self.level > profile::level(KEY) {
            profile::set_level(KEY, self.level);
        }
        self.build(view);
        self.age = 0.0;
    }

    fn paddle_y(&self) -> f32 {
        self.size.y - 70.0
    }

    pub fn paddle_rect(&self) -> Rect {
        let w = self.paddle_w();
        Rect::new(self.paddle_x - w / 2.0, self.paddle_y(), w, self.paddle_h())
    }

    fn build(&mut self, view: View) {
        self.size = vec2(view.w, view.h);
        self.paddle_x = view.w / 2.0;
        let margin = 30.0;
        let bw = (view.w - margin * 2.0) / COLS as f32;
        let bh = 34.0;
        let layout = layout_tints(self.level);
        self.tints = layout.iter().map(|t| t.unwrap_or(0)).collect();
        self.bricks = (0..ROWS * COLS)
            .map(|i| {
                let (r, c) = (i / COLS, i % COLS);
                layout[i].map(|_| {
                    Rect::new(
                        margin + c as f32 * bw + 3.0,
                        90.0 + r as f32 * bh + 3.0,
                        bw - 6.0,
                        bh - 6.0,
                    )
                })
            })
            .collect();
        self.total = self.bricks.iter().flatten().count();
        self.pops = (0..ROWS * COLS)
            .map(|i| -((i % COLS + i / COLS) as f32) * 0.04)
            .collect();
        self.serve();
    }

    fn serve(&mut self) {
        self.state = State::Serve;
        self.touched = false;
        self.stale = 0;
        self.combo = 0;
        self.trail.clear();
        self.vel = Vec2::ZERO;
        self.ball = vec2(self.paddle_x, self.paddle_y() - BALL_R - 1.0);
    }

    pub fn speed(&self) -> f32 {
        let ramp = 1.0 + 0.2 * self.cleared() as f32 / self.total.max(1) as f32;
        base_speed(self.level) * ramp * (1.0 - 0.07 * self.assist as f32)
    }

    pub fn step(&mut self, dt: f32) {
        let steps = ((self.vel.length() * dt) / (BALL_R * 0.5)).ceil().max(1.0) as usize;
        let h = dt / steps as f32;
        for _ in 0..steps {
            self.substep(h);
            if self.state != State::Playing {
                return;
            }
        }
    }

    fn substep(&mut self, dt: f32) {
        self.ball += self.vel * dt;
        if self.ball.x < BALL_R {
            self.ball.x = BALL_R;
            self.vel.x = self.vel.x.abs();
            self.events.push(Event::WallHit(self.ball));
        }
        if self.ball.x > self.size.x - BALL_R {
            self.ball.x = self.size.x - BALL_R;
            self.vel.x = -self.vel.x.abs();
            self.events.push(Event::WallHit(self.ball));
        }
        if self.ball.y < 70.0 + BALL_R {
            self.ball.y = 70.0 + BALL_R;
            self.vel.y = self.vel.y.abs();
            self.events.push(Event::WallHit(self.ball));
        }
        let p = self.paddle_rect();
        if self.vel.y > 0.0 && circle_hits_rect(self.ball, BALL_R, p) {
            let k = ((self.ball.x - self.paddle_x) / (self.paddle_w() / 2.0)).clamp(-1.0, 1.0);
            let mut angle = k * 1.05;
            if self.stale >= 3 {
                angle += (self.stale as f32 * 2.3).sin() * 0.28;
            }
            if angle.abs() < 0.12 {
                angle = if self.vel.x < 0.0 { -0.12 } else { 0.12 };
            }
            self.stale += 1;
            self.vel = vec2(angle.sin(), -angle.cos()) * self.speed();
            self.ball.y = p.y - BALL_R;
            self.combo = 0;
            self.events.push(Event::PaddleHit(self.ball));
        }
        for (i, b) in self.bricks.iter_mut().enumerate() {
            let Some(r) = *b else { continue };
            if !circle_hits_rect(self.ball, BALL_R, r) {
                continue;
            }
            *b = None;
            self.stale = 0;
            self.combo += 1;
            let points = combo_points(self.combo);
            self.score += points;
            let c = r.center();
            self.events.push(Event::BrickBroken {
                at: c,
                row: self.tints[i],
                points,
                combo: self.combo,
            });
            let d = (self.ball - c) / vec2(r.w, r.h);
            if d.x.abs() > d.y.abs() {
                self.vel.x = self.vel.x.abs() * d.x.signum();
            } else {
                self.vel.y = self.vel.y.abs() * d.y.signum();
            }
            break;
        }
        if self.ball.y > self.size.y + BALL_R {
            self.events
                .push(Event::Lost(vec2(self.ball.x, self.size.y - 10.0)));
            self.misses += 1;
            if self.forgiving() {
                self.serve();
            } else {
                self.lives -= 1;
                if self.lives == 0 {
                    self.state = State::Lost;
                } else {
                    self.serve();
                }
            }
        } else if self.bricks.iter().all(|b| b.is_none()) {
            self.state = State::Cleared;
            self.events.push(Event::LevelCleared);
        }
    }

    fn settle(&mut self) {
        self.settled = true;
        match self.state {
            State::Cleared => {
                let stars = stars_for(self.misses);
                self.score += 100 * stars as u32;
                profile::award_stars(KEY, self.level, stars);
                let next = (self.level + 1).min(LEVELS as u32);
                profile::unlock(KEY, next);
                profile::set_level(KEY, next.max(profile::level(KEY)));
                self.assist = if self.misses == 0 {
                    0
                } else {
                    self.assist.saturating_sub(1)
                };
                let key = format!("score.{}", self.level);
                if self.score as i64 > profile::get(KEY, &key) {
                    profile::set(KEY, &key, self.score as i64);
                }
                self.rank = profile::submit(KEY, self.score as i64, profile::Better::Higher);
            }
            State::Lost => {
                self.assist = (self.assist + 1).min(MAX_ASSIST);
                self.rank = profile::submit(KEY, self.score as i64, profile::Better::Higher);
            }
            _ => {}
        }
        profile::set(KEY, "assist", self.assist as i64);
    }

    fn go_select(&mut self) {
        self.state = State::Select;
        self.prev_state = State::Select;
        self.age = 0.0;
        self.level = profile::level(KEY).clamp(1, profile::unlocked(KEY).clamp(1, LEVELS as u32));
        sfx::play(Sfx::Tap);
    }

    fn update_select(&mut self, input: &Input, view: &View) {
        if !input.pressed {
            return;
        }
        let open = profile::unlocked(KEY).clamp(1, LEVELS as u32);
        for (i, r) in level_tiles(view).iter().enumerate() {
            if r.contains(input.pos) && (i as u32) < open {
                sfx::play(Sfx::Pop);
                self.start_level(i as u32 + 1, *view);
                break;
            }
        }
    }

    fn react(&mut self, view: &View) {
        for e in std::mem::take(&mut self.events) {
            match e {
                Event::BrickBroken {
                    at,
                    row,
                    points,
                    combo,
                } => {
                    sfx::play_step(Sfx::Pop, combo.saturating_sub(1) as usize);
                    let col = brick_color(row);
                    self.particles.emit(
                        at,
                        &[col, darker(col, 0.8), WHITE],
                        Burst {
                            count: 14,
                            speed: (80.0, 320.0),
                            size: (3.0, 8.0),
                            shape: Shape::Square,
                            ..Default::default()
                        },
                    );
                    self.shake.add(0.12 + 0.03 * combo.min(6) as f32);
                    self.bump.kick();
                    self.since_break = 0.0;
                    if combo >= 2 {
                        self.popups.add(
                            at - vec2(0.0, 20.0),
                            format!("+{points}"),
                            hsv(0.12 - combo.min(8) as f32 * 0.015, 0.7, 1.0),
                            26.0 + combo.min(8) as f32 * 4.0,
                        );
                    }
                }
                Event::PaddleHit(at) => {
                    sfx::play(Sfx::Bounce);
                    self.paddle_age = 0.0;
                    self.particles.puff(vec2(at.x, at.y + BALL_R), WHITE);
                }
                Event::WallHit(at) => {
                    sfx::play(Sfx::Tick);
                    self.particles.emit(
                        at,
                        &[Color::from_rgba(200, 220, 255, 255)],
                        Burst {
                            count: 4,
                            speed: (40.0, 120.0),
                            size: (2.0, 4.0),
                            ..Default::default()
                        },
                    );
                }
                Event::Lost(at) => {
                    sfx::play(Sfx::Wrong);
                    if self.state == State::Lost {
                        sfx::play(Sfx::Lose);
                    }
                    feel::impact(Impact::Medium);
                    self.shake.add(0.7);
                    self.particles.emit(
                        at,
                        &[WHITE, Color::from_rgba(255, 120, 120, 255)],
                        Burst {
                            count: 22,
                            angle: -std::f32::consts::FRAC_PI_2,
                            spread: 2.4,
                            ..Default::default()
                        },
                    );
                }
                Event::LevelCleared => {
                    sfx::play(Sfx::Win);
                    sfx::say(sfx::Say::Win);
                    feel::impact_tinted(Impact::Large, Color::from_rgba(255, 210, 80, 255));
                    self.shake.add(0.4);
                    self.particles.confetti(view);
                }
            }
        }
    }
}

impl Default for Breakout {
    fn default() -> Self {
        Self::new()
    }
}

impl Game for Breakout {
    fn update(&mut self, input: &Input, view: &View, dt: f32) {
        self.clock += dt;
        self.age += dt;
        self.paddle_age += dt;
        self.since_break += dt;
        if self.since_break > COMBO_WINDOW && self.state == State::Playing {
            self.combo = 0;
        }
        for p in &mut self.pops {
            *p += dt;
        }
        self.particles.update(dt);
        self.popups.update(dt);
        self.shake.update(dt);
        self.bump.update(dt);
        if self.state != State::Playing {
            self.trail.clear();
        }
        if vec2(view.w, view.h) != self.size && self.state == State::Serve && self.cleared() == 0 {
            self.build(*view);
        }
        self.size = vec2(view.w, view.h);
        if input.down {
            self.paddle_x = approach(self.paddle_x, input.pos.x, PADDLE_EASE, dt);
        }
        if input.left {
            self.paddle_x -= 700.0 * dt;
        }
        if input.right {
            self.paddle_x += 700.0 * dt;
        }
        let half = self.paddle_w() / 2.0;
        self.paddle_x = self.paddle_x.clamp(half, view.w - half);
        match self.state {
            State::Select => self.update_select(input, view),
            State::Serve => {
                self.ball = vec2(self.paddle_x, self.paddle_y() - BALL_R - 1.0);
                if input.tapped_in(serve_levels_button(view)) && self.first_win() {
                    self.go_select();
                    return;
                }
                self.touched |= input.pressed;
                if ((input.released && self.touched) || input.action) && self.age >= LAUNCH_DELAY {
                    if self.score == 0 && self.misses == 0 {
                        sfx::say(sfx::Say::Go);
                    }
                    self.state = State::Playing;
                    self.vel = vec2(0.35, -1.0).normalize() * self.speed();
                }
            }
            State::Playing => {
                self.step(dt);
                self.trail.push(self.ball);
                if self.trail.len() > TRAIL_LEN {
                    self.trail.remove(0);
                }
            }
            State::Lost => {
                if input.tapped_in(lost_levels_button(view)) && self.first_win() && self.age >= 0.3
                {
                    self.go_select();
                } else if input.tap() && self.age >= TAP_DELAY {
                    self.start_level(self.level, *view);
                }
            }
            State::Cleared => {
                let last = self.level >= LEVELS as u32;
                if input.tapped_in(levels_button(view)) && !last && self.age >= 0.4 {
                    self.go_select();
                } else if input.tap() && self.age >= TAP_DELAY {
                    if last {
                        self.go_select();
                    } else {
                        self.start_level(self.level + 1, *view);
                    }
                } else {
                    for i in 0..stars_for(self.misses) as usize {
                        let t = 0.35 + 0.25 * i as f32;
                        if self.age - dt < t && self.age >= t {
                            sfx::play_step(Sfx::Coin, i * 2);
                        }
                    }
                }
            }
        }
        if self.state != self.prev_state {
            self.prev_state = self.state;
            self.age = 0.0;
        }
        if matches!(self.state, State::Lost | State::Cleared) && !self.settled {
            self.settle();
        }
        self.react(view);
    }

    fn draw(&self, view: &View) {
        draw_arena(view, self.clock);
        if self.state == State::Select {
            self.draw_select(view);
            return;
        }
        let off = self.shake.offset();
        set_view_camera(view, vec2(off.x, off.y));
        for (i, b) in self.bricks.iter().enumerate() {
            if let Some(r) = b {
                let age = self.pops[i];
                if age < 0.0 {
                    continue;
                }
                let k = ease_out_back(age / 0.35).max(0.01);
                let center = r.center();
                let r = Rect::new(
                    center.x - r.w * k / 2.0,
                    center.y - r.h * k / 2.0,
                    r.w * k,
                    r.h * k,
                );
                draw_brick(self.tints[i], r);
            }
        }
        for (i, p) in self.trail.iter().enumerate() {
            let k = (i + 1) as f32 / self.trail.len() as f32;
            art::sprite_at(
                "ball",
                *p,
                BALL_R * 2.0 * (0.3 + 0.6 * k),
                crate::art::Draw {
                    tint: Color::new(1.0, 1.0, 1.0, 0.28 * k),
                    ..Default::default()
                },
            );
        }
        let p = self.paddle_rect();
        let sq = squash_scale(self.paddle_age);
        let pc = p.center();
        let p = Rect::new(
            pc.x - p.w * sq.x / 2.0,
            p.y + p.h - p.h * sq.y,
            p.w * sq.x,
            p.h * sq.y,
        );
        soft_shadow(vec2(pc.x, p.y + p.h + 10.0), p.w * 0.45, 7.0, 0.6);
        art::sprite("paddle", p);
        let glow = 0.22 + 0.08 * (self.clock * 6.0).sin();
        art::sprite_at(
            "ball",
            self.ball,
            BALL_R * 4.0,
            crate::art::Draw {
                tint: Color::new(1.0, 1.0, 1.0, glow),
                ..Default::default()
            },
        );
        art::sprite_at("ball", self.ball, BALL_R * 2.3, Default::default());
        self.particles.draw();
        self.popups.draw();
        set_view_camera(view, Vec2::ZERO);
        vignette(view, 0.6);
        chip(
            view,
            Side::Left,
            "SCORE",
            &self.score.to_string(),
            self.bump.t,
        );
        text_outline(
            &format!("Level {}", self.level),
            view.w / 2.0,
            CHIP_Y + 16.0,
            30.0,
            Color::from_rgba(200, 200, 255, 255),
            INK,
        );
        self.draw_progress(view);
        if self.combo >= 2 && self.state == State::Playing {
            let pulse = 1.0 + 0.06 * (self.clock * 14.0).sin();
            text_outline(
                &format!("Combo x{}", self.combo),
                view.w / 2.0,
                70.0,
                28.0 * pulse,
                Color::from_rgba(255, 220, 90, 255),
                Color::from_rgba(40, 30, 60, 255),
            );
        }
        if self.forgiving() {
            self.draw_stars(view);
        }
        for i in 0..if self.forgiving() { 0 } else { self.lives } {
            let c = vec2(
                view.w - 30.0 - i as f32 * 36.0,
                35.0 + view.inset.top.min(0.0),
            );
            art::sprite_at(
                "ball",
                c + vec2(0.0, 2.0),
                28.0,
                crate::art::Draw {
                    tint: Color::new(0.0, 0.0, 0.0, 0.3),
                    ..Default::default()
                },
            );
            art::sprite_at("ball", c, 28.0, Default::default());
        }
        let learning = self.level == 1 && !self.first_win();
        match self.state {
            State::Serve => {
                let label = if learning {
                    "Drag to move, let go to launch"
                } else {
                    "Let go to launch"
                };
                let pulse = 1.0 + 0.04 * (self.clock * 4.0).sin();
                crate::ui::hint(view, label, vec2(view.w / 2.0, view.h * 0.62), pulse);
                if self.first_win() {
                    painted_button(serve_levels_button(view), "Levels", "blue", 0.0, 1.0);
                }
            }
            State::Playing => {
                if learning && self.cleared() == 0 {
                    crate::ui::hint(
                        view,
                        "Keep the ball up!",
                        vec2(view.w / 2.0, view.h * 0.62),
                        1.0 + 0.04 * (self.clock * 4.0).sin(),
                    );
                }
            }
            State::Lost => {
                let line = format!("Cleared {} of {} bricks", self.cleared(), self.total);
                results(
                    view,
                    &crate::ui::Results {
                        game: KEY,
                        title: "Nice try!",
                        line: &line,
                        rank: self.rank,
                        action: "Try again",
                        format: crate::ui::plain_score,
                    },
                );
                if self.first_win() {
                    painted_button(lost_levels_button(view), "Levels", "blue", 0.0, 1.0);
                }
            }
            State::Cleared => self.draw_cleared(view),
            State::Select => {}
        }
    }
}

impl Breakout {
    fn draw_progress(&self, view: &View) {
        let w = 240.0;
        let r = Rect::new(view.w / 2.0 - w / 2.0, CHIP_Y + 36.0, w, 14.0);
        round_rect(r, 7.0, Color::new(0.0, 0.0, 0.1, 0.55));
        let f = self.cleared() as f32 / self.total.max(1) as f32;
        if f > 0.0 {
            let fill = Rect::new(r.x + 2.0, r.y + 2.0, ((w - 4.0) * f).max(10.0), r.h - 4.0);
            round_rect(fill, fill.h / 2.0, Color::from_rgba(255, 200, 40, 255));
        }
    }

    fn draw_stars(&self, view: &View) {
        let have = stars_for(self.misses);
        for i in 0..3 {
            let on = (i as u8) < have;
            crate::ui::sprite_at(
                "p_star",
                vec2(view.w - 36.0 - i as f32 * 40.0, 36.0),
                36.0,
                crate::art::Draw {
                    tint: if on {
                        WHITE
                    } else {
                        Color::from_rgba(90, 70, 130, 140)
                    },
                    ..Default::default()
                },
            );
        }
    }

    fn draw_cleared(&self, view: &View) {
        let age = self.age;
        let fade = ease_out_cubic(age / 0.2);
        fill_screen(view, Color::new(0.06, 0.04, 0.16, 0.5 * fade));
        let s = ease_out_back(age / 0.32).max(0.01);
        let base = panel_rect(view);
        let c = base.center() + vec2(0.0, (1.0 - ease_out_cubic(age / 0.32)) * 90.0);
        let r = Rect::new(
            c.x - base.w * s / 2.0,
            c.y - base.h * s / 2.0,
            base.w * s,
            base.h * s,
        );
        crate::ui::painted_panel(r, WHITE);
        crate::ui::ribbon(view, "Level clear!", vec2(c.x, r.y + 8.0 * s), age);
        let earned = stars_for(self.misses);
        for i in 0..3u8 {
            let t = ((age - 0.3 - i as f32 * 0.25) / 0.3).clamp(0.0, 1.0);
            let on = i < earned;
            let k = if on { ease_out_back(t) } else { t };
            let rise = if i == 1 { -14.0 } else { 0.0 };
            crate::ui::sprite_at(
                "p_star",
                vec2(c.x + (i as f32 - 1.0) * 110.0 * s, r.y + (122.0 + rise) * s),
                (if i == 1 { 110.0 } else { 92.0 }) * s * k.max(0.01),
                crate::art::Draw {
                    tint: if on {
                        WHITE
                    } else {
                        Color::from_rgba(90, 70, 130, 140)
                    },
                    rotation: if on {
                        (age * 5.0 + i as f32).sin() * 0.05
                    } else {
                        0.0
                    },
                    ..Default::default()
                },
            );
        }
        let line = if self.rank == Some(0) {
            format!("New best! Score {}", self.score)
        } else {
            format!("Score {}", self.score)
        };
        let la = ((age - 0.3) / 0.3).clamp(0.0, 1.0);
        text_center(
            &line,
            c.x,
            r.y + 218.0 * s,
            36.0 * s,
            with_alpha(crate::ui::BODY, la),
        );
        let last = self.level >= LEVELS as u32;
        let label = if last { "All levels" } else { "Next level" };
        let pulse = 1.0 + (age * 4.5).sin() * 0.03;
        let place = |b: Rect, k: f32| {
            let at = c + (b.center() - base.center()) * s;
            Rect::new(
                at.x - b.w * s * k / 2.0,
                at.y - b.h * s * k / 2.0,
                b.w * s * k,
                b.h * s * k,
            )
        };
        crate::ui::painted_button(place(next_button(view), pulse), label, "green", 0.0, la);
        if !last {
            crate::ui::painted_button(place(levels_button(view), 1.0), "Levels", "blue", 0.0, la);
        }
    }

    fn draw_select(&self, view: &View) {
        fill_screen(view, Color::new(0.06, 0.04, 0.2, 0.45));
        crate::ui::ribbon(view, "Pick a level", vec2(view.w / 2.0, 100.0), self.clock);
        let open = profile::unlocked(KEY).clamp(1, LEVELS as u32);
        let next = profile::level(KEY).clamp(1, open);
        let tiles = level_tiles(view);
        for (i, r) in tiles.iter().enumerate() {
            let lvl = i as u32 + 1;
            let locked = lvl > open;
            let appear = ease_out_back(((self.age - i as f32 * 0.03) / 0.3).clamp(0.0, 1.0));
            let pulse = if lvl == next {
                1.0 + 0.04 * (self.clock * 5.0).sin()
            } else {
                1.0
            };
            let k = appear.max(0.01) * pulse;
            let c = r.center();
            let rr = Rect::new(c.x - r.w * k / 2.0, c.y - r.h * k / 2.0, r.w * k, r.h * k);
            let tint = if locked {
                Color::new(0.55, 0.55, 0.65, 1.0)
            } else if lvl == next {
                Color::from_rgba(255, 244, 190, 255)
            } else {
                WHITE
            };
            crate::ui::painted_panel(rr, tint);
            let inner = Rect::new(
                rr.x + rr.w * 0.14,
                rr.y + rr.h * 0.2,
                rr.w * 0.72,
                rr.h * 0.42,
            );
            let tints = layout_tints(lvl);
            let (cw, ch) = (inner.w / COLS as f32, inner.h / ROWS as f32);
            for (j, t) in tints.iter().enumerate() {
                if let Some(t) = t {
                    let col = if locked {
                        Color::new(0.5, 0.5, 0.58, 0.6)
                    } else {
                        brick_color(*t)
                    };
                    draw_rectangle(
                        inner.x + (j % COLS) as f32 * cw + 0.5,
                        inner.y + (j / COLS) as f32 * ch + 0.5,
                        cw - 1.0,
                        ch - 1.0,
                        col,
                    );
                }
            }
            text_outline(
                &lvl.to_string(),
                rr.center().x,
                rr.y + rr.h * 0.12,
                rr.h * 0.2,
                WHITE,
                INK,
            );
            if locked {
                crate::ui::sprite_at(
                    "icon_lock",
                    rr.center() + vec2(0.0, rr.h * 0.02),
                    rr.h * 0.3,
                    crate::art::Draw {
                        tint: Color::from_rgba(70, 60, 100, 230),
                        ..Default::default()
                    },
                );
            } else {
                let got = profile::stars(KEY, lvl);
                for j in 0..3u8 {
                    crate::ui::sprite_at(
                        "p_star",
                        vec2(
                            rr.center().x + (j as f32 - 1.0) * rr.w * 0.26,
                            rr.y + rr.h * 0.8,
                        ),
                        rr.h * 0.24,
                        crate::art::Draw {
                            tint: if j < got {
                                WHITE
                            } else {
                                Color::from_rgba(90, 70, 130, 120)
                            },
                            ..Default::default()
                        },
                    );
                }
            }
        }
        let total = profile::total_stars(KEY);
        chip(
            view,
            Side::Right,
            "STARS",
            &format!("{total}/{}", LEVELS * 3),
            self.bump.t,
        );
        let top = career_score();
        if top > 0 {
            chip(view, Side::Left, "SCORE", &top.to_string(), self.bump.t);
        }
    }
}

pub fn icon(r: Rect) {
    crate::ui::icon("breakout", r);
}

mod art {
    crate::sprites!("breakout");
}

use crate::ui::painted_button;

fn draw_brick(row: usize, r: Rect) {
    let grow = r.h * 0.09;
    art::sprite(
        &format!("brick{}", row % ROWS),
        Rect::new(r.x - grow, r.y - grow, r.w + grow * 2.0, r.h + grow * 2.0),
    );
}

fn draw_arena(view: &View, clock: f32) {
    let f = view.full();
    let s = art::size("bg");
    let k = (f.w / s.x).max(f.h / s.y);
    let (w, h) = (s.x * k, s.y * k);
    art::sprite(
        "bg",
        Rect::new(f.x + (f.w - w) / 2.0, f.y + (f.h - h) / 2.0, w, h),
    );
    for k in 0..14 {
        let x = (k as f32 * 197.0 + 40.0) % view.w;
        let y = (k as f32 * 131.0 + 20.0) % view.h;
        let tw = 0.5 + 0.5 * (clock * 1.5 + k as f32).sin();
        crate::ui::spark(
            vec2(x, y),
            6.0 + tw * 4.0,
            clock * 0.2,
            with_alpha(WHITE, 0.2 + 0.25 * tw),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fresh() {
        crate::save::reset_for_test();
        crate::profile::create("Mia", 0);
    }

    fn playing() -> Breakout {
        fresh();
        let mut b = Breakout::new();
        b.start_level(LEVELS as u32, View::fit(MIN_W, MIN_H));
        b.state = State::Playing;
        b
    }

    #[test]
    fn a_jittery_finger_does_not_jerk_the_paddle() {
        let mut b = playing();
        let v = View::fit(MIN_W, MIN_H);
        b.paddle_x = 400.0;
        b.update(
            &Input {
                pos: vec2(430.0, 600.0),
                down: true,
                ..Default::default()
            },
            &v,
            1.0 / 60.0,
        );
        assert!(b.paddle_x > 400.0 && b.paddle_x < 425.0, "{}", b.paddle_x);
        for _ in 0..15 {
            b.update(
                &Input {
                    pos: vec2(430.0, 600.0),
                    down: true,
                    ..Default::default()
                },
                &v,
                1.0 / 60.0,
            );
        }
        assert!((b.paddle_x - 430.0).abs() < 1.0, "{}", b.paddle_x);
    }

    #[test]
    fn breakout_sprites_all_exist() {
        let have = art::names();
        let mut wanted: Vec<String> = (0..ROWS).map(|r| format!("brick{r}")).collect();
        wanted.extend(["paddle", "ball", "bg"].map(String::from));
        for n in wanted {
            assert!(have.contains(&n), "missing {n}");
        }
    }

    #[test]
    fn paddle_sprite_is_as_wide_as_its_hitbox() {
        let s = art::size("paddle");
        let drawn_h = PADDLE_W * s.y / s.x;
        assert!((drawn_h - PADDLE_H).abs() < 2.0, "{drawn_h}");
    }

    #[test]
    fn losing_all_balls_plays_the_sad_jingle() {
        let mut b = playing();
        b.lives = 1;
        b.ball = vec2(10.0 + BALL_R, b.size.y - 5.0);
        b.paddle_x = b.size.x - PADDLE_W;
        b.vel = vec2(0.0, 600.0);
        b.step(0.1);
        crate::sfx::take_pending();
        b.react(&View::fit(MIN_W, MIN_H));
        assert!(crate::sfx::take_pending().contains(&Sfx::Lose));
    }

    #[test]
    fn ball_breaks_brick_and_bounces() {
        let mut b = playing();
        let brick = b.bricks[ROWS * COLS - 1].unwrap();
        b.ball = vec2(brick.center().x, brick.y + brick.h + BALL_R + 5.0);
        b.vel = vec2(0.0, -500.0);
        b.step(0.05);
        assert!(b.bricks[ROWS * COLS - 1].is_none());
        assert_eq!(b.score, 10);
        assert!(b.vel.y > 0.0);
    }

    #[test]
    fn fast_ball_does_not_tunnel_through_paddle() {
        let mut b = playing();
        b.ball = vec2(b.paddle_x, b.paddle_y() - 60.0);
        b.vel = vec2(0.0, 3000.0);
        b.step(1.0 / 30.0);
        assert!(b.vel.y < 0.0);
        assert_eq!(b.lives, LIVES);
    }

    #[test]
    fn edge_of_paddle_sends_ball_sideways() {
        let mut b = playing();
        b.ball = vec2(
            b.paddle_x + PADDLE_W / 2.0 - 2.0,
            b.paddle_y() - BALL_R + 1.0,
        );
        b.vel = vec2(0.0, 400.0);
        b.step(0.001);
        assert!(b.vel.x > 0.0 && b.vel.y < 0.0);
    }

    #[test]
    fn missing_the_ball_costs_a_life() {
        let mut b = playing();
        b.ball = vec2(10.0 + BALL_R, b.size.y - 5.0);
        b.paddle_x = b.size.x - PADDLE_W;
        b.vel = vec2(0.0, 600.0);
        b.step(0.1);
        assert_eq!(b.lives, LIVES - 1);
        assert_eq!(b.state, State::Serve);
    }

    #[test]
    fn consecutive_bricks_build_a_combo_until_the_paddle() {
        let mut b = playing();
        assert_eq!(combo_points(1), 10);
        assert_eq!(combo_points(3), 30);
        assert_eq!(combo_points(99), 80);
        let first = b.bricks[ROWS * COLS - 1].unwrap();
        b.ball = vec2(first.center().x, first.y + first.h + BALL_R + 5.0);
        b.vel = vec2(0.0, -500.0);
        b.step(0.05);
        assert_eq!(b.combo, 1);
        b.combo = 2;
        let second = b.bricks[ROWS * COLS - 2].unwrap();
        b.ball = vec2(second.center().x, second.y + second.h + BALL_R + 5.0);
        b.vel = vec2(0.0, -500.0);
        b.step(0.05);
        assert_eq!(b.combo, 3);
        assert_eq!(b.score, 10 + 30);
        b.ball = vec2(b.paddle_x, b.paddle_y() - BALL_R + 1.0);
        b.vel = vec2(0.0, 400.0);
        b.step(0.001);
        assert_eq!(b.combo, 0);
    }

    #[test]
    fn breaking_a_brick_emits_particles_and_a_combo_popup() {
        let mut b = playing();
        b.combo = 1;
        let brick = b.bricks[ROWS * COLS - 1].unwrap();
        b.ball = vec2(brick.center().x, brick.y + brick.h + BALL_R + 5.0);
        b.vel = vec2(0.0, -500.0);
        b.step(0.05);
        assert!(matches!(
            b.events.first(),
            Some(Event::BrickBroken {
                combo: 2,
                points: 20,
                ..
            })
        ));
        b.react(&View::fit(MIN_W, MIN_H));
        assert!(!b.particles.list.is_empty());
        assert_eq!(b.popups.list.len(), 1);
        assert!(b.shake.active());
        assert!(b.events.is_empty());
    }

    #[test]
    fn paddle_hit_squashes_then_recovers() {
        let s = squash_scale(0.0);
        assert!(s.x > 1.0 && s.y < 1.0);
        assert_eq!(squash_scale(0.5), vec2(1.0, 1.0));
        let mut b = playing();
        b.ball = vec2(b.paddle_x, b.paddle_y() - BALL_R + 1.0);
        b.vel = vec2(0.0, 400.0);
        b.step(0.001);
        assert!(matches!(b.events.last(), Some(Event::PaddleHit(_))));
        b.react(&View::fit(MIN_W, MIN_H));
        assert_eq!(b.paddle_age, 0.0);
    }

    #[test]
    fn clearing_the_last_brick_throws_confetti() {
        let mut b = playing();
        for slot in b.bricks.iter_mut().skip(1) {
            *slot = None;
        }
        let last = b.bricks[0].unwrap();
        b.ball = vec2(last.center().x, last.y + last.h + BALL_R + 5.0);
        b.vel = vec2(0.0, -500.0);
        b.step(0.05);
        assert_eq!(b.state, State::Cleared);
        assert!(b.events.contains(&Event::LevelCleared));
        b.react(&View::fit(MIN_W, MIN_H));
        assert!(b.particles.list.len() > 50);
    }

    #[test]
    fn trail_follows_the_ball_and_is_bounded() {
        let mut b = Breakout::new();
        b.state = State::Playing;
        b.vel = vec2(0.0, -1.0);
        let input = Input::default();
        let view = View::fit(MIN_W, MIN_H);
        for _ in 0..40 {
            b.update(&input, &view, 1.0 / 60.0);
        }
        assert_eq!(b.trail.len(), TRAIL_LEN);
        b.state = State::Lost;
        b.update(&input, &view, 1.0 / 60.0);
        assert!(b.trail.is_empty());
    }

    fn view() -> View {
        View::fit(MIN_W, MIN_H)
    }

    fn tap() -> Input {
        Input::press_at(vec2(10.0, 10.0))
    }

    fn follow(b: &Breakout, error: f32, t: f32) -> Input {
        let target = b.ball.x + error * (t * 1.7).sin();
        Input {
            pos: vec2(target, b.size.y - 40.0),
            down: true,
            ..Default::default()
        }
    }

    fn launch(b: &mut Breakout) {
        let v = view();
        b.update(&tap(), &v, 0.5);
        b.update(
            &Input {
                released: true,
                ..Default::default()
            },
            &v,
            0.1,
        );
        assert_eq!(b.state, State::Playing);
    }

    fn play_until_done(b: &mut Breakout, error: f32, seconds: f32) {
        let v = view();
        launch(b);
        let mut t = 0.0;
        while b.state == State::Playing && t < seconds {
            let input = follow(b, error, t);
            b.update(&input, &v, 1.0 / 60.0);
            t += 1.0 / 60.0;
        }
    }

    #[test]
    fn every_layout_is_a_full_grid_with_bricks() {
        for (i, l) in LAYOUTS.iter().enumerate() {
            assert!(
                l.iter().all(|r| r.chars().count() == COLS),
                "level {}",
                i + 1
            );
            assert!(!layout_tints(i as u32 + 1).iter().all(|t| t.is_none()));
        }
    }

    #[test]
    fn brick_counts_grow_with_the_level() {
        let counts: Vec<usize> = (1..=LEVELS as u32)
            .map(|l| layout_tints(l).iter().flatten().count())
            .collect();
        assert!(counts[0] <= 10, "{counts:?}");
        assert_eq!(*counts.last().unwrap(), ROWS * COLS);
        assert!(counts[LEVELS - 1] > counts[0] * 4);
    }

    #[test]
    fn first_level_is_slow_wide_and_forgiving() {
        fresh();
        let b = Breakout::new();
        assert_eq!(b.level, 1);
        assert!(b.forgiving());
        assert!(b.paddle_w() >= 250.0);
        assert!(b.speed() <= 320.0);
        assert!(b.speed() < 430.0);
    }

    #[test]
    fn difficulty_rises_level_by_level() {
        for l in 2..=LEVELS as u32 {
            assert!(base_speed(l) > base_speed(l - 1));
            assert!(base_paddle_w(l) <= base_paddle_w(l - 1));
        }
        assert!(base_paddle_w(LEVELS as u32) >= 160.0);
        assert!(base_speed(LEVELS as u32) <= 600.0);
    }

    #[test]
    fn ball_speeds_up_as_the_wall_empties() {
        let mut b = playing();
        let before = b.speed();
        for slot in b.bricks.iter_mut().take(ROWS * COLS / 2) {
            *slot = None;
        }
        assert!(b.speed() > before);
    }

    #[test]
    fn missing_on_early_levels_never_ends_the_game() {
        fresh();
        let mut b = Breakout::new();
        b.state = State::Playing;
        for _ in 0..10 {
            b.state = State::Playing;
            b.ball = vec2(10.0 + BALL_R, b.size.y - 5.0);
            b.paddle_x = b.size.x - b.paddle_w();
            b.vel = vec2(0.0, 600.0);
            b.step(0.1);
            assert_eq!(b.state, State::Serve);
        }
        assert_eq!(b.lives, LIVES);
        assert_eq!(b.misses, 10);
        assert_eq!(stars_for(b.misses), 1);
    }

    #[test]
    fn a_sloppy_player_clears_level_one() {
        fresh();
        let mut b = Breakout::new();
        play_until_done(&mut b, 90.0, 240.0);
        assert_eq!(b.state, State::Cleared);
    }

    #[test]
    fn a_steady_player_clears_every_level() {
        for level in 1..=LEVELS as u32 {
            fresh();
            let mut b = Breakout::new();
            b.start_level(level, view());
            play_until_done(&mut b, 40.0, 900.0);
            assert_eq!(b.state, State::Cleared, "level {level}");
        }
    }

    #[test]
    fn clearing_a_level_awards_stars_unlocks_the_next_and_submits() {
        fresh();
        let mut b = Breakout::new();
        play_until_done(&mut b, 20.0, 240.0);
        assert_eq!(b.state, State::Cleared);
        b.update(&Input::default(), &view(), 0.016);
        assert_eq!(profile::stars(KEY, 1), stars_for(b.misses));
        assert_eq!(profile::unlocked(KEY), 2);
        assert_eq!(profile::level(KEY), 2);
        assert!(b.rank.is_some());
        assert_eq!(profile::board(KEY).len(), 1);
        b.update(&Input::default(), &view(), 0.016);
        assert_eq!(profile::board(KEY).len(), 1);
    }

    #[test]
    fn three_stars_for_a_clean_clear_fewer_for_misses() {
        assert_eq!(stars_for(0), 3);
        assert_eq!(stars_for(1), 2);
        assert_eq!(stars_for(2), 1);
        assert_eq!(stars_for(9), 1);
    }

    #[test]
    fn a_better_run_keeps_the_best_stars() {
        fresh();
        profile::award_stars(KEY, 1, 3);
        profile::award_stars(KEY, 1, 1);
        assert_eq!(profile::stars(KEY, 1), 3);
    }

    #[test]
    fn next_level_tap_starts_the_next_level() {
        fresh();
        let mut b = Breakout::new();
        play_until_done(&mut b, 20.0, 240.0);
        let v = view();
        b.update(&Input::default(), &v, 1.0);
        b.update(&tap(), &v, 0.016);
        assert_eq!(b.level, 2);
        assert_eq!(b.state, State::Serve);
        assert_eq!(b.cleared(), 0);
    }

    #[test]
    fn a_new_player_starts_at_level_one_and_resumes_after_unlocking() {
        fresh();
        let b = Breakout::new();
        assert_eq!(b.state, State::Serve);
        assert_eq!(b.level, 1);
        profile::unlock(KEY, 4);
        profile::set_level(KEY, 4);
        let b = Breakout::new();
        assert_eq!(b.state, State::Select);
        assert_eq!(b.level, 4);
        crate::profile::create("Leo", 1);
        let b = Breakout::new();
        assert_eq!(b.state, State::Serve);
        assert_eq!(b.level, 1);
    }

    #[test]
    fn level_select_only_opens_unlocked_levels() {
        fresh();
        profile::unlock(KEY, 3);
        let mut b = Breakout::new();
        let v = view();
        let tiles = level_tiles(&v);
        assert_eq!(tiles.len(), LEVELS);
        b.update(&Input::press_at(tiles[5].center()), &v, 0.016);
        assert_eq!(b.state, State::Select);
        b.update(&Input::press_at(tiles[1].center()), &v, 0.016);
        assert_eq!(b.state, State::Serve);
        assert_eq!(b.level, 2);
        b.update(&Input::default(), &v, 0.5);
        let back = serve_levels_button(&v).center();
        assert_eq!(b.state, State::Serve);
        b.update(&Input::press_at(back), &v, 0.016);
        assert_eq!(b.state, State::Select);
    }

    #[test]
    fn tiles_fit_on_phone_and_tablet_shapes() {
        for v in [
            View::fit(1024.0, 768.0),
            View::fit(768.0, 1024.0),
            View::fit(390.0, 844.0),
            View::fit(844.0, 390.0),
        ] {
            for r in level_tiles(&v) {
                assert!(r.x >= 0.0 && r.right() <= v.w && r.y >= 100.0 && r.bottom() <= v.h);
            }
        }
    }

    #[test]
    fn revisiting_an_earlier_level_does_not_lose_your_place() {
        fresh();
        profile::unlock(KEY, 5);
        profile::set_level(KEY, 5);
        let mut b = Breakout::new();
        b.start_level(2, view());
        assert_eq!(profile::level(KEY), 5);
        assert_eq!(profile::unlocked(KEY), 5);
    }

    #[test]
    fn failing_adds_help_and_winning_takes_it_away() {
        fresh();
        let mut b = Breakout::new();
        b.start_level(5, view());
        let base_w = b.paddle_w();
        let base_v = b.speed();
        b.lives = 1;
        b.state = State::Playing;
        b.ball = vec2(10.0 + BALL_R, b.size.y - 5.0);
        b.paddle_x = b.size.x - b.paddle_w();
        b.vel = vec2(0.0, 600.0);
        b.step(0.1);
        b.update(&Input::default(), &view(), 0.016);
        assert_eq!(b.state, State::Lost);
        assert_eq!(b.assist, 1);
        b.update(&Input::default(), &view(), 1.0);
        b.update(&tap(), &view(), 0.016);
        assert_eq!(b.state, State::Serve);
        assert!(b.paddle_w() > base_w);
        assert!(b.speed() < base_v);
        assert_eq!(b.lives, LIVES);
        b.assist = 2;
        b.start_level(5, view());
        assert_eq!(b.lives, LIVES + 1);
    }

    #[test]
    fn help_is_capped_and_saved_for_next_time() {
        fresh();
        let mut b = Breakout::new();
        b.start_level(5, view());
        for _ in 0..6 {
            b.state = State::Lost;
            b.settled = false;
            b.update(&Input::default(), &view(), 0.016);
        }
        assert_eq!(b.assist, MAX_ASSIST);
        assert_eq!(Breakout::new().assist, MAX_ASSIST);
        assert!(b.paddle_w() <= base_paddle_w(5) * 1.5);
    }

    #[test]
    fn losing_does_not_throw_away_progress_or_reset_the_level() {
        fresh();
        profile::unlock(KEY, 4);
        profile::set_level(KEY, 4);
        let mut b = Breakout::new();
        b.start_level(4, view());
        b.state = State::Lost;
        b.update(&Input::default(), &view(), 0.016);
        b.update(&Input::default(), &view(), 1.0);
        b.update(&tap(), &view(), 0.016);
        assert_eq!(b.level, 4);
        assert_eq!(profile::unlocked(KEY), 4);
        assert_eq!(Breakout::new().level, 4);
    }

    #[test]
    fn losing_submits_the_score_once() {
        fresh();
        let mut b = Breakout::new();
        b.start_level(5, view());
        b.score = 120;
        b.state = State::Lost;
        b.update(&Input::default(), &view(), 0.016);
        b.update(&Input::default(), &view(), 0.016);
        assert_eq!(b.rank, Some(0));
        assert_eq!(profile::board(KEY).len(), 1);
    }

    #[test]
    fn releasing_without_a_press_does_not_launch() {
        fresh();
        let mut b = Breakout::new();
        b.update(
            &Input {
                released: true,
                ..Default::default()
            },
            &view(),
            1.0,
        );
        assert_eq!(b.state, State::Serve);
        launch(&mut b);
    }

    #[test]
    fn progress_bar_tracks_cleared_bricks() {
        let mut b = playing();
        assert_eq!(b.cleared(), 0);
        b.bricks[0] = None;
        b.bricks[1] = None;
        assert_eq!(b.cleared(), 2);
        assert_eq!(b.total, ROWS * COLS);
    }

    #[test]
    fn two_players_keep_separate_levels() {
        fresh();
        profile::unlock(KEY, 6);
        profile::set_level(KEY, 6);
        let mia = profile::current_id().unwrap();
        crate::profile::create("Leo", 1);
        assert_eq!(Breakout::new().level, 1);
        profile::select(&mia);
        assert_eq!(Breakout::new().level, 6);
    }
}
