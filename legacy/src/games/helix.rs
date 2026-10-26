use crate::engine::*;
use crate::feel::{self, Impact};
use crate::fx::approach as feel_approach;
use crate::fx::*;
use crate::sfx::{self, Sfx};
use macroquad::prelude::*;
use std::f32::consts::TAU;

pub const SEGMENTS: usize = 12;
pub const FLOOR_GAP: f32 = 170.0;
pub const GRAVITY: f32 = 1700.0;
pub const BOUNCE: f32 = -640.0;
pub const BALL_R: f32 = 18.0;
pub const SMASH_FLOORS: u32 = 3;
pub const GAME: &str = "helix";
pub const MAX_LEVEL: u32 = 30;
pub const GRID_COLS: usize = 6;
const DRAG_DEADZONE_MM: f32 = 1.0;
const DRAG_TURN: f32 = 0.012;
const DRAG_EASE: f32 = 28.0;
const WIN_DELAY: f32 = 0.8;
const STAR_START: f32 = 0.3;
const STAR_GAP: f32 = 0.25;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Seg {
    Solid,
    Gap,
    Danger,
}

#[derive(Clone, Debug)]
pub struct Floor {
    pub segs: [Seg; SEGMENTS],
    pub broken: f32,
    pub hue: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum State {
    Select,
    Ready,
    Playing,
    Dead,
    Won,
}

pub struct Helix {
    pub floors: Vec<Floor>,
    pub angle: f32,
    pub y: f32,
    pub vy: f32,
    pub passed: usize,
    pub streak: u32,
    pub score: u32,
    pub best: Best,
    pub level: u32,
    pub state: State,
    pub bounces: u32,
    pub stars: u8,
    pub rank: Option<usize>,
    pub fails: u32,
    taught: bool,
    cam: f32,
    t: f32,
    squash: f32,
    splats: Vec<(usize, f32, f32)>,
    fx: Particles,
    shake: Shake,
    pops: Popups,
    bump: Bump,
    end_t: f32,
    last_x: Option<f32>,
    pending: f32,
    engaged: bool,
}

pub fn ball_angle() -> f32 {
    std::f32::consts::FRAC_PI_2
}

pub fn segment_at(angle: f32, rotation: f32) -> usize {
    let local = (angle - rotation).rem_euclid(TAU);
    ((local / TAU * SEGMENTS as f32) as usize).min(SEGMENTS - 1)
}

pub fn floor_count(level: u32) -> usize {
    (6 + level as usize * 2).min(36)
}

pub fn gap_len_range(level: u32, fails: u32) -> (usize, usize) {
    let (lo, hi) = match level {
        0..=2 => (4, 4),
        3..=5 => (3, 4),
        _ => (2, 3),
    };
    let ease = fails.min(2) as usize;
    (lo + ease, (hi + ease).min(5))
}

pub fn danger_rate(level: u32, fails: u32) -> f32 {
    (level.saturating_sub(2) as f32 * 0.3 - fails as f32 * 0.8).clamp(0.0, 4.0)
}

pub fn reached() -> u32 {
    crate::profile::unlocked(GAME).clamp(1, MAX_LEVEL)
}

pub fn start_level() -> u32 {
    match crate::profile::level(GAME) {
        0 => 1,
        n => n.clamp(1, reached()),
    }
}

pub fn max_shift(level: u32) -> i32 {
    (2 + level as i32 / 2).min(6)
}

pub fn stars_for(bounces: u32, floors: usize) -> u8 {
    let floors = floors as u32;
    if bounces <= floors {
        3
    } else if bounces <= floors * 2 {
        2
    } else {
        1
    }
}

fn gap_start(segs: &[Seg; SEGMENTS]) -> Option<usize> {
    (0..SEGMENTS).find(|k| segs[*k] == Seg::Gap && segs[(k + SEGMENTS - 1) % SEGMENTS] != Seg::Gap)
}

fn make_floor(index: usize, level: u32, fails: u32, prev_gap: Option<usize>) -> Floor {
    let mut segs = [Seg::Solid; SEGMENTS];
    let gap_at = match prev_gap {
        Some(p) => {
            let shift = max_shift(level);
            (p as i32 + rand::gen_range(-shift, shift + 1)).rem_euclid(SEGMENTS as i32) as usize
        }
        None => rand::gen_range(0, SEGMENTS),
    };
    let (lo, hi) = gap_len_range(level, fails);
    let gap_len = rand::gen_range(lo, hi + 1);
    for k in 0..gap_len {
        segs[(gap_at + k) % SEGMENTS] = Seg::Gap;
    }
    if index >= 2 {
        let rate = danger_rate(level, fails);
        let dangers = rate.floor() as usize + usize::from(rand::gen_range(0.0, 1.0) < rate.fract());
        for _ in 0..dangers.min(5) {
            let k = rand::gen_range(0, SEGMENTS);
            if segs[k] == Seg::Solid {
                segs[k] = Seg::Danger;
            }
        }
    }
    Floor {
        segs,
        broken: 0.0,
        hue: rand::gen_range(0.0, 1.0),
    }
}

impl Default for Helix {
    fn default() -> Self {
        Self::new()
    }
}

impl Helix {
    pub fn new() -> Self {
        let mut g = Self::with_level(start_level());
        g.best = Best::load(GAME);
        g
    }

    pub fn with_level(level: u32) -> Self {
        Self::build(level, 0)
    }

    fn build(level: u32, fails: u32) -> Self {
        let level = level.clamp(1, MAX_LEVEL);
        let mut floors: Vec<Floor> = Vec::new();
        let mut prev = None;
        for i in 0..floor_count(level) {
            let f = make_floor(i, level, fails, prev);
            prev = gap_start(&f.segs);
            floors.push(f);
        }
        let first_gap = gap_start(&floors[0].segs).unwrap();
        let turn = if level <= 2 { 0.25 } else { 0.5 };
        let rotation = ball_angle() - (first_gap as f32 + 0.5) / SEGMENTS as f32 * TAU + TAU * turn;
        floors[0].segs[segment_at(ball_angle(), rotation)] = Seg::Solid;
        Self {
            floors,
            angle: rotation,
            y: -60.0,
            vy: 0.0,
            passed: 0,
            streak: 0,
            score: 0,
            best: Best::default(),
            level,
            state: State::Ready,
            bounces: 0,
            stars: 0,
            rank: None,
            fails,
            taught: crate::profile::get(GAME, "taught") == 1,
            cam: 0.0,
            t: 0.0,
            squash: 0.0,
            splats: vec![],
            fx: Particles::default(),
            shake: Shake::default(),
            pops: Popups::default(),
            bump: Bump::default(),
            end_t: 0.0,
            last_x: None,
            pending: 0.0,
            engaged: false,
        }
    }

    pub fn enter(&mut self, level: u32) {
        let level = level.clamp(1, MAX_LEVEL);
        crate::profile::set_level(GAME, level);
        let fails = if level == self.level { self.fails } else { 0 };
        let best = self.best;
        *self = Self::build(level, fails);
        self.best = best;
    }

    pub fn open_select(&mut self) {
        self.state = State::Select;
        self.end_t = 0.0;
    }

    pub fn floor_y(i: usize) -> f32 {
        i as f32 * FLOOR_GAP
    }

    pub fn seg_under_ball(&self, i: usize) -> Seg {
        self.floors[i].segs[segment_at(ball_angle(), self.angle)]
    }

    pub fn physics(&mut self, dt: f32) -> Option<Seg> {
        let old = self.y;
        self.vy += GRAVITY * dt;
        self.y += self.vy * dt;
        if self.vy <= 0.0 {
            return None;
        }
        let i = self.passed;
        if i >= self.floors.len() {
            return None;
        }
        let fy = Self::floor_y(i) - BALL_R;
        if old <= fy && self.y >= fy {
            let seg = self.seg_under_ball(i);
            match seg {
                Seg::Gap => {
                    self.pass_floor();
                }
                _ if self.streak >= SMASH_FLOORS => {
                    sfx::play(Sfx::Boom);
                    feel::impact(Impact::Medium);
                    self.floors[i].broken = 0.001;
                    self.pass_floor();
                    self.streak = 0;
                    self.y = fy;
                    self.vy = BOUNCE * 0.6;
                    self.shake.add(0.5);
                    return Some(Seg::Gap);
                }
                Seg::Danger => {
                    self.y = fy;
                    return Some(Seg::Danger);
                }
                Seg::Solid => {
                    sfx::play(Sfx::Bounce);
                    self.bounces += 1;
                    self.y = fy;
                    self.vy = BOUNCE;
                    self.streak = 0;
                }
            }
            return Some(seg);
        }
        None
    }

    fn follow_finger(&mut self, input: &Input, dt: f32) {
        if input.down {
            if input.pressed {
                self.engaged = false;
                self.pending = 0.0;
            }
            if !self.engaged && (input.pos.x - input.start.x).abs() > DRAG_DEADZONE_MM * input.mm()
            {
                self.engaged = true;
                self.last_x = Some(input.pos.x);
            }
            if self.engaged {
                if let Some(lx) = self.last_x {
                    self.pending += (input.pos.x - lx) * DRAG_TURN;
                }
                self.last_x = Some(input.pos.x);
            }
        } else {
            self.last_x = None;
            self.engaged = false;
        }
        let left = feel_approach(self.pending, 0.0, DRAG_EASE, dt);
        self.angle += self.pending - left;
        self.pending = left;
    }

    fn finish_level(&mut self, view: &View) {
        self.state = State::Won;
        self.end_t = 0.0;
        self.stars = stars_for(self.bounces, self.floors.len());
        crate::profile::award_stars(GAME, self.level, self.stars);
        crate::profile::unlock(GAME, (self.level + 1).min(MAX_LEVEL));
        crate::profile::set_level(GAME, (self.level + 1).min(MAX_LEVEL));
        self.rank = self.best.submit(GAME, self.score);
        sfx::play(Sfx::Win);
        sfx::play(Sfx::LevelUp);
        sfx::say(if self.stars == 3 {
            sfx::Say::Congrats
        } else {
            sfx::Say::Win
        });
        feel::impact_tinted(Impact::Large, Color::from_rgba(255, 220, 60, 255));
        self.fx.confetti(view);
    }

    fn pass_floor(&mut self) {
        let i = self.passed;
        if self.floors[i].broken == 0.0 {
            self.floors[i].broken = 0.001;
        }
        self.passed += 1;
        self.streak += 1;
        let gained = self.streak * self.level;
        self.score += gained;
        self.bump.kick();
    }

    fn ring(&self) -> (f32, f32) {
        (150.0, 60.0)
    }
}

impl Game for Helix {
    fn update(&mut self, input: &Input, view: &View, dt: f32) {
        self.t += dt;
        self.fx.update(dt);
        self.shake.update(dt);
        self.pops.update(dt);
        self.bump.update(dt);
        self.squash = (self.squash - dt * 5.0).max(0.0);
        for f in &mut self.floors {
            if f.broken > 0.0 {
                f.broken += dt;
            }
        }
        self.follow_finger(input, dt);
        if input.left {
            self.angle -= 3.5 * dt;
        }
        if input.right {
            self.angle += 3.5 * dt;
        }
        let target = self.y - view.h * 0.35;
        if target > self.cam {
            self.cam = approach(self.cam, target, 10.0, dt);
        }
        match self.state {
            State::Select => {
                self.end_t += dt;
                if input.pressed {
                    let l = select_layout(view);
                    if l.play.contains(input.pos) {
                        sfx::play(Sfx::Tap);
                        self.enter(start_level());
                    } else if let Some(n) = l.cells.iter().position(|c| c.contains(input.pos)) {
                        let n = n as u32 + 1;
                        if n <= reached() {
                            sfx::play(Sfx::Tap);
                            self.enter(n);
                        } else {
                            sfx::play(Sfx::Wrong);
                        }
                    }
                }
            }
            State::Ready => {
                if input.tapped_in(levels_button(view)) {
                    sfx::play(Sfx::Tap);
                    self.open_select();
                    return;
                }
                self.y = -60.0 + (self.t * 4.0).sin().abs() * -40.0;
                if input.pressed || input.action {
                    self.state = State::Playing;
                    sfx::say(sfx::Say::Go);
                    self.vy = 0.0;
                }
            }
            State::Playing => {
                let before = self.passed;
                let ball_screen = vec2(view.w / 2.0, self.y - self.cam + view.h * 0.15);
                match self.physics(dt) {
                    Some(Seg::Solid) => {
                        self.squash = 1.0;
                        self.splats.push((
                            self.passed,
                            segment_at(ball_angle(), self.angle) as f32,
                            self.angle,
                        ));
                        self.fx.emit(
                            ball_screen + vec2(0.0, BALL_R),
                            &[Color::from_rgba(255, 120, 60, 255)],
                            Burst {
                                count: 8,
                                speed: (60.0, 180.0),
                                size: (3.0, 6.0),
                                spread: 2.0,
                                angle: -std::f32::consts::FRAC_PI_2,
                                ..Default::default()
                            },
                        );
                    }
                    Some(Seg::Danger) => {
                        sfx::play(Sfx::Hit);
                        sfx::play(Sfx::Lose);
                        if self.score > self.best.0 && self.best.0 > 0 {
                            sfx::say(sfx::Say::Best);
                        }
                        feel::impact(Impact::Large);
                        self.state = State::Dead;
                        self.end_t = 0.0;
                        self.fails += 1;
                        self.rank = self.best.submit(GAME, self.score);
                        self.shake.add(0.7);
                        self.fx.emit(
                            ball_screen,
                            &[Color::from_rgba(255, 120, 60, 255), WHITE],
                            Burst {
                                count: 30,
                                ..Default::default()
                            },
                        );
                    }
                    _ => {}
                }
                if self.passed > before {
                    if !self.taught {
                        self.taught = true;
                        crate::profile::set(GAME, "taught", 1);
                    }
                    sfx::play(Sfx::Swish);
                    let col = hsv(self.floors[before].hue, 0.6, 1.0);
                    self.fx.emit(
                        ball_screen,
                        &[col, darker(col, 0.8)],
                        Burst {
                            count: 18,
                            shape: Shape::Square,
                            speed: (200.0, 420.0),
                            gravity: 900.0,
                            ..Default::default()
                        },
                    );
                    if self.passed * 2 == self.floors.len() {
                        self.pops
                            .add(ball_screen + vec2(0.0, -70.0), "Halfway!", WHITE, 52.0);
                        sfx::play(Sfx::Score);
                    }
                    if self.streak >= 2 {
                        self.pops.add(
                            ball_screen + vec2(70.0, -30.0),
                            format!("+{}", self.streak * self.level),
                            Color::from_rgba(255, 220, 60, 255),
                            40.0,
                        );
                    }
                }
                if self.passed >= self.floors.len() {
                    self.finish_level(view);
                }
            }
            State::Dead | State::Won => {
                let before = self.end_t;
                self.end_t += dt;
                if self.state == State::Won {
                    self.physics(dt);
                    self.y = self.y.min(Self::floor_y(self.floors.len()) + 60.0);
                    for k in 0..self.stars as usize {
                        let at = WIN_DELAY + STAR_START + STAR_GAP * k as f32;
                        if before < at && self.end_t >= at {
                            sfx::play_step(Sfx::Coin, k * 2);
                        }
                    }
                }
                if self.end_t > 0.7 && input.pressed {
                    if levels_button(view).contains(input.pos) {
                        sfx::play(Sfx::Tap);
                        self.open_select();
                    } else if self.state == State::Won {
                        let next = (self.level + 1).min(MAX_LEVEL);
                        self.enter(next);
                    } else {
                        self.enter(self.level);
                    }
                } else if self.end_t > 0.7 && input.action {
                    let next = if self.state == State::Won {
                        (self.level + 1).min(MAX_LEVEL)
                    } else {
                        self.level
                    };
                    self.enter(next);
                }
            }
        }
    }

    fn draw(&self, view: &View) {
        draw_backdrop(view, self.level);
        if self.state == State::Select {
            draw_clouds(view, self.cam, self.t);
            draw_select(self, view);
            return;
        }
        let sh = self.shake.offset();
        let cx = view.w / 2.0 + sh.x;
        let to_screen = |y: f32| y - self.cam + view.h * 0.15 + sh.y;
        draw_clouds(view, self.cam, self.t);
        let (outer, inner) = self.ring();
        let pole_w = inner * 1.1;
        draw_pole(cx, view, pole_w);
        for (i, f) in self.floors.iter().enumerate() {
            let y = to_screen(Self::floor_y(i));
            if y < -100.0 || y > view.h + 100.0 {
                continue;
            }
            let fall = f.broken * f.broken * 900.0;
            let fade = (1.0 - f.broken * 1.5).max(0.0);
            if f.broken > 0.0 && fade <= 0.0 {
                continue;
            }
            for k in 0..SEGMENTS {
                let seg = f.segs[k];
                if seg == Seg::Gap {
                    continue;
                }
                let a0 = self.angle + k as f32 / SEGMENTS as f32 * TAU;
                let a1 = a0 + TAU / SEGMENTS as f32;
                let mid = (a0 + a1) / 2.0;
                let push = if f.broken > 0.0 {
                    vec2(mid.cos() * fall * 0.6, fall)
                } else {
                    Vec2::ZERO
                };
                let name = if seg == Seg::Danger {
                    "tile_red"
                } else {
                    floor_tile(f.hue)
                };
                draw_ring_segment(&Ring {
                    centre: vec2(cx, y) + push,
                    a0,
                    a1,
                    outer,
                    inner,
                    thick: 18.0,
                    flat: 0.32,
                    tile: name,
                    light: 0.75 + 0.25 * mid.sin().max(-0.2),
                    alpha: fade,
                });
            }
            for (fi, seg, at) in &self.splats {
                if *fi == i && f.broken == 0.0 {
                    let a = self.angle - at + (*seg + 0.5) / SEGMENTS as f32 * TAU;
                    let r = (outer + inner) / 2.0;
                    art::sprite_at(
                        "splat",
                        vec2(cx + a.cos() * r, y + a.sin() * r * 0.32),
                        24.0,
                        crate::art::Draw {
                            tint: Color::from_rgba(255, 255, 255, 235),
                            ..Default::default()
                        },
                    );
                }
            }
        }
        if self.state != State::Dead {
            let by = to_screen(self.y);
            let bx = cx + ball_angle().cos() * (outer + inner) / 2.0;
            let bz = by + ball_angle().sin() * (outer + inner) / 2.0 * 0.32;
            let sq = self.squash * 0.35;
            let stretch = (self.vy.abs() / 1200.0).min(0.25) * (1.0 - self.squash);
            if self.streak >= SMASH_FLOORS && self.state == State::Playing {
                for k in 0..6 {
                    art::sprite_at(
                        "ball",
                        vec2(bx, bz - k as f32 * 10.0),
                        BALL_R * 2.0 * (1.0 - k as f32 * 0.12),
                        crate::art::Draw {
                            tint: Color::new(1.0, 0.6, 0.2, 0.3),
                            ..Default::default()
                        },
                    );
                }
            }
            let w = BALL_R * 2.2 * (1.0 + sq - stretch * 0.5);
            let h = BALL_R * 2.2 * (1.0 - sq + stretch);
            art::sprite_ex(
                "ball",
                Rect::new(bx - w / 2.0, bz - h / 2.0, w, h),
                crate::art::Draw {
                    ..Default::default()
                },
            );
        } else {
            let by = to_screen(self.y);
            let bx = cx + ball_angle().cos() * (outer + inner) / 2.0;
            let bz = by + (outer + inner) / 2.0 * 0.32;
            art::sprite(
                "splat",
                Rect::new(
                    bx - BALL_R * 1.8,
                    bz + 4.0 - BALL_R * 0.9,
                    BALL_R * 3.6,
                    BALL_R * 1.8,
                ),
            );
        }
        self.fx.draw();
        self.pops.draw();
        let progress = self.passed as f32 / self.floors.len() as f32;
        let bar = Rect::new(view.w / 2.0 - 180.0, 24.0, 360.0, 26.0);
        crate::ui::nine("panel", bar, 14.0, bar.h / 64.0, WHITE);
        crate::ui::nine(
            "btn_yellow",
            Rect::new(bar.x, bar.y, (bar.w * progress).max(bar.h), bar.h),
            18.0,
            bar.h / 64.0,
            WHITE,
        );
        for (k, x) in [
            (self.level, bar.x - 26.0),
            (self.level + 1, bar.x + bar.w + 26.0),
        ] {
            let done = x < view.w / 2.0 || progress >= 1.0;
            crate::ui::sprite_at(
                if done { "round_yellow" } else { "round_white" },
                vec2(x, bar.y + 13.0),
                46.0,
                Default::default(),
            );
            text_outline(
                &k.to_string(),
                x,
                bar.y + 11.0,
                26.0,
                WHITE,
                if done {
                    Color::from_rgba(150, 90, 0, 255)
                } else {
                    crate::ui::INK
                },
            );
        }
        let left = self.floors.len() - self.passed;
        let goal = match left {
            0 => "You made it!".to_string(),
            1 => "1 floor to go".to_string(),
            n => format!("{n} floors to go"),
        };
        text_shadow(&goal, view.w / 2.0, 72.0, 24.0, WHITE);
        big_score(view, &self.score.to_string(), 128.0, self.bump.t);
        chip(view, Side::Left, "LEVEL", &self.level.to_string(), 1.0);
        chip(view, Side::Right, "BEST", &self.best.0.to_string(), 1.0);
        let pulse = 1.0 + (self.t * 3.0).sin() * 0.03;
        match self.state {
            State::Ready => {
                crate::ui::hint(
                    view,
                    "Tap, then drag sideways to spin the tower",
                    vec2(view.w / 2.0, view.h * 0.8),
                    pulse,
                );
                draw_levels_button(view);
            }
            State::Playing if !self.taught => crate::ui::hint(
                view,
                "Line up the hole to drop through!",
                vec2(view.w / 2.0, view.h * 0.8),
                pulse,
            ),
            State::Dead if self.end_t > 0.5 => {
                let line = format!("Level {}, score {}", self.level, self.score);
                results(
                    view,
                    &crate::ui::Results {
                        game: GAME,
                        title: "Splat!",
                        line: &line,
                        rank: self.rank,
                        action: "Try again",
                        format: crate::ui::plain_score,
                    },
                );
                draw_levels_button(view);
            }
            State::Won if self.end_t > WIN_DELAY => {
                draw_win(self, view);
                draw_levels_button(view);
            }
            _ => {}
        }
    }
}

pub struct SelectLayout {
    pub cells: Vec<Rect>,
    pub play: Rect,
}

pub fn select_layout(view: &View) -> SelectLayout {
    let rows = (MAX_LEVEL as usize).div_ceil(GRID_COLS);
    let gap = 10.0;
    let cell = (((view.w - 120.0) / GRID_COLS as f32) - gap)
        .min(((view.h - 330.0) / rows as f32) - gap)
        .clamp(50.0, 110.0);
    let gw = GRID_COLS as f32 * (cell + gap) - gap;
    let gh = rows as f32 * (cell + gap) - gap;
    let total = gh + 24.0 + 76.0;
    let top = ((view.h - total) / 2.0 + 50.0).max(170.0);
    let left = (view.w - gw) / 2.0;
    let cells = (0..MAX_LEVEL as usize)
        .map(|i| {
            Rect::new(
                left + (i % GRID_COLS) as f32 * (cell + gap),
                top + (i / GRID_COLS) as f32 * (cell + gap),
                cell,
                cell,
            )
        })
        .collect();
    let pw = 320.0;
    SelectLayout {
        cells,
        play: Rect::new((view.w - pw) / 2.0, top + gh + 24.0, pw, 76.0),
    }
}

pub fn levels_button(view: &View) -> Rect {
    Rect::new(view.w / 2.0 - 95.0, view.h - 70.0, 190.0, 54.0)
}

fn draw_levels_button(view: &View) {
    crate::ui::painted_button(levels_button(view), "Levels", "blue", 0.0, 1.0);
}

fn draw_select(g: &Helix, view: &View) {
    crate::ui::ribbon(view, "Pick a level", vec2(view.w / 2.0, 84.0), g.t);
    chip(
        view,
        Side::Right,
        "STARS",
        &crate::profile::total_stars(GAME).to_string(),
        1.0,
    );
    let l = select_layout(view);
    let open = reached();
    let now = start_level();
    for (i, r) in l.cells.iter().enumerate() {
        let n = i as u32 + 1;
        let locked = n > open;
        let tint = if locked {
            Color::new(0.55, 0.55, 0.62, 0.8)
        } else if n == now {
            Color::from_rgba(255, 224, 120, 255)
        } else {
            WHITE
        };
        let pulse = if n == now {
            1.0 + (g.t * 4.0).sin() * 0.03
        } else {
            1.0
        };
        let rr = Rect::new(
            r.center().x - r.w * pulse / 2.0,
            r.center().y - r.h * pulse / 2.0,
            r.w * pulse,
            r.h * pulse,
        );
        crate::ui::card(rr, tint);
        let ink = if locked {
            Color::new(0.4, 0.38, 0.5, 0.7)
        } else {
            crate::ui::INK
        };
        text_center(
            &n.to_string(),
            r.center().x,
            r.y + r.h * 0.38,
            r.h * 0.4,
            ink,
        );
        if !locked {
            let earned = crate::profile::stars(GAME, n);
            for k in 0..3u8 {
                crate::ui::sprite_at(
                    if k < earned {
                        "star_yellow"
                    } else {
                        "star_white"
                    },
                    vec2(
                        r.center().x + (k as f32 - 1.0) * r.w * 0.26,
                        r.y + r.h * 0.74,
                    ),
                    r.h * 0.24,
                    crate::art::Draw {
                        tint: if k < earned {
                            WHITE
                        } else {
                            Color::new(0.6, 0.6, 0.7, 0.6)
                        },
                        ..Default::default()
                    },
                );
            }
        }
    }
    let label = format!("Play level {now}");
    crate::ui::painted_button(l.play, &label, "green", 0.0, 1.0);
}

fn draw_win(g: &Helix, view: &View) {
    let age = (g.end_t - WIN_DELAY).max(0.0);
    let fade = ease_out_cubic(age / 0.2);
    fill_screen(view, Color::new(0.06, 0.04, 0.16, 0.5 * fade));
    let pw = 560.0f32.min(view.w - 40.0);
    let ph = 420.0;
    let s = ease_out_back(age / 0.32).max(0.01);
    let c = view.center() + vec2(0.0, 10.0 + (1.0 - ease_out_cubic(age / 0.32)) * 90.0);
    let r = Rect::new(c.x - pw * s / 2.0, c.y - ph * s / 2.0, pw * s, ph * s);
    crate::ui::painted_panel(r, WHITE);
    crate::ui::ribbon(view, "Hooray!", vec2(c.x, r.y + 8.0 * s), age);
    let a = |d: f32| ((age - d) / 0.2).clamp(0.0, 1.0);
    let line = format!("Level {} done!", g.level);
    text_center(
        &line,
        c.x,
        r.y + 100.0 * s,
        36.0 * s,
        with_alpha(crate::ui::BODY, a(0.12)),
    );
    for k in 0..3usize {
        let earned = k < g.stars as usize;
        let pop = if earned {
            ease_out_back(((age - STAR_START - STAR_GAP * k as f32) / 0.3).clamp(0.0, 1.0))
        } else {
            a(STAR_START)
        };
        let tint = if earned {
            WHITE
        } else {
            Color::new(0.4, 0.4, 0.46, 0.85)
        };
        let lift = if k == 1 { -14.0 } else { 0.0 };
        crate::ui::sprite_at(
            "p_star",
            vec2(c.x + (k as f32 - 1.0) * 120.0 * s, r.y + (205.0 + lift) * s),
            100.0 * s * pop,
            crate::art::Draw {
                tint,
                ..Default::default()
            },
        );
    }
    let score = format!("Score {}   Best {}", g.score, g.best.0);
    text_center(
        &score,
        c.x,
        r.y + 292.0 * s,
        30.0 * s,
        with_alpha(crate::ui::BODY, a(0.5)),
    );
    let label = if g.level >= MAX_LEVEL {
        "Play again"
    } else {
        "Next level"
    };
    let by = r.y + r.h - 56.0 * s;
    let pulse = 1.0 + (age * 4.5).sin() * 0.03;
    let bw = (crate::outline::measure(FONT_BYTES, label, 30.0) + 110.0) * s * pulse;
    crate::ui::painted_button(
        Rect::new(c.x - bw / 2.0, by - 10.0 * s, bw, 78.0 * s * pulse),
        label,
        "green",
        0.0,
        a(0.6),
    );
}

mod art {
    crate::sprites!("helix");

    pub fn bytes() -> &'static [u8] {
        SHEET_BYTES
    }
}

thread_local! {
    static TILES: std::cell::RefCell<Option<(Texture2D, crate::art::Index)>> =
        const { std::cell::RefCell::new(None) };
}

pub fn floor_tile(hue: f32) -> &'static str {
    ["tile_green", "tile_blue", "tile_pink", "tile_grey"][(hue.rem_euclid(1.0) * 4.0) as usize % 4]
}

pub struct Ring {
    pub centre: Vec2,
    pub a0: f32,
    pub a1: f32,
    pub outer: f32,
    pub inner: f32,
    pub thick: f32,
    pub flat: f32,
    pub tile: &'static str,
    pub light: f32,
    pub alpha: f32,
}

fn with_tiles(f: impl FnOnce(&Texture2D, &crate::art::Index)) {
    TILES.with(|t| {
        let mut t = t.borrow_mut();
        let (tex, index) = t.get_or_insert_with(|| {
            let index = crate::art::parse(art::bytes()).expect("bad helix sheet");
            let (at, len) = index.pages[0];
            let tex = crate::art::upload(&art::bytes()[at..at + len]);
            (tex, index)
        });
        f(tex, index);
    });
}

fn textured_quad(tex: &Texture2D, pts: [Vec2; 4], uvs: [Vec2; 4], color: Color) {
    use macroquad::models::{draw_mesh, Mesh, Vertex};
    let vertices = (0..4)
        .map(|i| Vertex::new2(vec3(pts[i].x, pts[i].y, 0.0), uvs[i], color))
        .collect();
    draw_mesh(&Mesh {
        vertices,
        indices: vec![0, 1, 2, 0, 2, 3],
        texture: Some(tex.clone()),
    });
}

pub fn draw_ring_segment(r: &Ring) {
    with_tiles(|tex, index| {
        let Some(f) = index.frames.get(r.tile) else {
            return;
        };
        let (tw, th) = (tex.width(), tex.height());
        let uv = |u: f32, v: f32| {
            let (u, v) = (0.14 + u * 0.72, 0.14 + v * 0.72);
            vec2(
                (f.src.x + 0.5 + (f.src.w - 1.0) * u) / tw,
                (f.src.y + 0.5 + (f.src.h - 1.0) * v) / th,
            )
        };
        let steps = 4;
        let top = Color::new(r.light, r.light, r.light, r.alpha);
        let side = Color::new(r.light * 0.7, r.light * 0.7, r.light * 0.7, r.alpha);
        let p =
            |a: f32, rad: f32, dy: f32| r.centre + vec2(a.cos() * rad, a.sin() * rad * r.flat + dy);
        for s in 0..steps {
            let k0 = s as f32 / steps as f32;
            let k1 = (s + 1) as f32 / steps as f32;
            let b0 = r.a0 + (r.a1 - r.a0) * k0;
            let b1 = r.a0 + (r.a1 - r.a0) * k1;
            if b0.sin() + b1.sin() > -0.5 {
                textured_quad(
                    tex,
                    [
                        p(b0, r.outer, 0.0),
                        p(b1, r.outer, 0.0),
                        p(b1, r.outer, r.thick),
                        p(b0, r.outer, r.thick),
                    ],
                    [uv(k0, 0.8), uv(k1, 0.8), uv(k1, 1.0), uv(k0, 1.0)],
                    side,
                );
            }
            textured_quad(
                tex,
                [
                    p(b0, r.inner, 0.0),
                    p(b1, r.inner, 0.0),
                    p(b1, r.outer, 0.0),
                    p(b0, r.outer, 0.0),
                ],
                [uv(k0, 0.0), uv(k1, 0.0), uv(k1, 0.8), uv(k0, 0.8)],
                top,
            );
        }
    });
}

fn draw_pole(cx: f32, view: &View, width: f32) {
    let f = view.full();
    art::sprite("pole", Rect::new(cx - width / 2.0, f.y, width, f.h));
}

fn draw_backdrop(view: &View, level: u32) {
    let f = view.full();
    let s = art::size("bg");
    let k = (f.w / s.x).max(f.h / s.y);
    let (w, h) = (s.x * k, s.y * k);
    let hue = (level as f32 * 0.07).rem_euclid(1.0);
    art::sprite_ex(
        "bg",
        Rect::new(f.x + (f.w - w) / 2.0, f.y + (f.h - h) / 2.0, w, h),
        crate::art::Draw {
            tint: hsv(hue, 0.12, 1.0),
            ..Default::default()
        },
    );
}

fn draw_clouds(view: &View, cam: f32, t: f32) {
    for i in 0..6 {
        let depth = 0.08 + (i % 3) as f32 * 0.05;
        let span = view.w + 400.0;
        let x = (i as f32 * 233.0 + t * (5.0 + depth * 40.0)).rem_euclid(span) - 200.0;
        let y = ((i * 131 % 700) as f32 - cam * depth).rem_euclid(view.h + 200.0) - 100.0;
        art::sprite_w(
            if i % 2 == 0 { "cloud1" } else { "cloud2" },
            vec2(x, y),
            150.0 + depth * 900.0,
            crate::art::Draw {
                tint: Color::new(1.0, 1.0, 1.0, 0.85),
                ..Default::default()
            },
        );
    }
}

pub fn icon(r: Rect) {
    crate::ui::icon("helix", r);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn above(g: &mut Helix, floor: usize, seg: Seg) {
        let k = segment_at(ball_angle(), g.angle);
        g.floors[floor].segs[k] = seg;
        g.passed = floor;
        g.y = Helix::floor_y(floor) - BALL_R - 5.0;
        g.vy = 400.0;
        g.state = State::Playing;
    }

    #[test]
    fn helix_tile_page_decodes_whatever_its_format() {
        let index = crate::art::parse(art::bytes()).unwrap();
        let (at, len) = index.pages[0];
        assert!(crate::art::decode_page(&art::bytes()[at..at + len]).is_some());
    }

    #[test]
    fn helix_sprites_all_exist() {
        let have = art::names();
        for n in [
            "tile_green",
            "tile_pink",
            "tile_blue",
            "tile_grey",
            "tile_red",
            "ball",
            "pole",
            "splat",
            "cloud1",
            "cloud2",
            "bg",
        ] {
            assert!(have.iter().any(|h| h == n), "missing {n}");
        }
        for k in 0..8 {
            let name = floor_tile(k as f32 / 8.0);
            assert!(have.iter().any(|h| h == name), "missing {name}");
        }
    }

    #[test]
    fn hue_picks_a_safe_tile_that_is_never_red() {
        for k in 0..100 {
            assert_ne!(floor_tile(k as f32 / 100.0), "tile_red");
        }
        assert_eq!(floor_tile(1.0), floor_tile(0.0));
    }

    #[test]
    fn segment_lookup_wraps() {
        assert_eq!(segment_at(0.01, 0.0), 0);
        assert_eq!(segment_at(TAU - 0.01, 0.0), SEGMENTS - 1);
        assert_eq!(segment_at(0.01, TAU), 0);
        assert_eq!(segment_at(0.01, -TAU / SEGMENTS as f32), 1);
    }

    #[test]
    fn solid_floor_bounces() {
        let mut g = Helix::new();
        above(&mut g, 2, Seg::Solid);
        assert_eq!(g.physics(0.05), Some(Seg::Solid));
        assert_eq!(g.vy, BOUNCE);
        assert_eq!(g.passed, 2);
    }

    #[test]
    fn gap_lets_ball_through_and_scores() {
        let mut g = Helix::new();
        above(&mut g, 2, Seg::Gap);
        g.physics(0.05);
        assert_eq!(g.passed, 3);
        assert!(g.score > 0);
    }

    #[test]
    fn dying_plays_the_sad_jingle() {
        let mut g = Helix::new();
        above(&mut g, 2, Seg::Danger);
        crate::sfx::take_pending();
        g.update(&Input::default(), &View::ipad_landscape(), 0.05);
        assert_eq!(g.state, State::Dead);
        assert!(crate::sfx::take_pending().contains(&Sfx::Lose));
    }

    #[test]
    fn red_segment_is_deadly() {
        let mut g = Helix::new();
        above(&mut g, 2, Seg::Danger);
        assert_eq!(g.physics(0.05), Some(Seg::Danger));
    }

    #[test]
    fn falling_through_several_floors_smashes_red() {
        let mut g = Helix::new();
        above(&mut g, 3, Seg::Danger);
        g.streak = SMASH_FLOORS;
        assert_ne!(g.physics(0.05), Some(Seg::Danger));
        assert_eq!(g.passed, 4);
    }

    #[test]
    fn start_floor_is_safe_under_the_ball() {
        for _ in 0..30 {
            let g = Helix::new();
            assert_eq!(g.seg_under_ball(0), Seg::Solid);
            assert!(g.floors[0].segs.contains(&Seg::Gap));
            assert!(!g.floors[0].segs.contains(&Seg::Danger));
        }
    }

    #[test]
    fn every_floor_has_a_way_down() {
        for level in 1..8 {
            let g = Helix::with_level(level);
            for f in &g.floors {
                assert!(f.segs.contains(&Seg::Gap));
            }
        }
    }

    fn setup() {
        crate::save::reset_for_test();
        crate::profile::create("Mia", 0);
    }

    fn bot_plays(g: &mut Helix, view: &View, max_secs: f32) {
        let dt = 1.0 / 60.0;
        let mut t = 0.0;
        let tap = Input::press_at(view.center());
        g.update(&tap, view, dt);
        while g.state == State::Playing && t < max_secs {
            let i = g.passed.min(g.floors.len() - 1);
            let segs = g.floors[i].segs;
            let start = gap_start(&segs).unwrap();
            let len = segs.iter().filter(|s| **s == Seg::Gap).count();
            let mid = (start as f32 + len as f32 / 2.0) / SEGMENTS as f32 * TAU;
            let want = (ball_angle() - mid).rem_euclid(TAU);
            let have = g.angle.rem_euclid(TAU);
            let mut d = want - have;
            if d > std::f32::consts::PI {
                d -= TAU;
            }
            if d < -std::f32::consts::PI {
                d += TAU;
            }
            let input = Input {
                left: d < -0.05,
                right: d > 0.05,
                ..Default::default()
            };
            g.update(&input, view, dt);
            t += dt;
        }
    }

    #[test]
    fn the_first_levels_have_no_red_and_wide_holes() {
        setup();
        for level in 1..=2 {
            for _ in 0..20 {
                let g = Helix::with_level(level);
                for f in &g.floors {
                    assert!(!f.segs.contains(&Seg::Danger));
                    let gaps = f.segs.iter().filter(|s| **s == Seg::Gap).count();
                    assert!(gaps >= 4, "level {level} hole of {gaps}");
                }
            }
        }
    }

    #[test]
    fn level_one_is_short() {
        assert!(floor_count(1) <= 8);
        assert!(floor_count(10) > floor_count(1));
    }

    #[test]
    fn red_arrives_after_the_first_levels_and_grows() {
        assert_eq!(danger_rate(1, 0), 0.0);
        assert_eq!(danger_rate(2, 0), 0.0);
        assert!(danger_rate(5, 0) > 0.0);
        assert!(danger_rate(12, 0) > danger_rate(5, 0));
        assert!(gap_len_range(8, 0).1 < gap_len_range(1, 0).0 + 1);
        assert!(gap_len_range(8, 0).0 < gap_len_range(1, 0).0);
    }

    #[test]
    fn red_actually_shows_up_on_later_levels() {
        setup();
        let reds: usize = (0..10)
            .map(|_| {
                Helix::with_level(8)
                    .floors
                    .iter()
                    .flat_map(|f| f.segs)
                    .filter(|s| *s == Seg::Danger)
                    .count()
            })
            .sum();
        assert!(reds > 20);
    }

    #[test]
    fn dying_makes_the_same_level_gentler() {
        assert!(danger_rate(9, 1) < danger_rate(9, 0));
        assert!(gap_len_range(9, 2).0 > gap_len_range(9, 0).0);
        setup();
        let mut g = Helix::with_level(9);
        above(&mut g, 3, Seg::Danger);
        g.update(&Input::default(), &View::ipad_landscape(), 0.05);
        assert_eq!(g.fails, 1);
        g.enter(9);
        assert_eq!(g.fails, 1);
        g.enter(10);
        assert_eq!(g.fails, 0);
    }

    #[test]
    fn holes_on_neighbouring_floors_stay_within_reach() {
        setup();
        for level in [1, 2, 6, 20] {
            let g = Helix::with_level(level);
            let limit = max_shift(level);
            for pair in g.floors.windows(2) {
                let a = gap_start(&pair[0].segs).unwrap() as i32;
                let b = gap_start(&pair[1].segs).unwrap() as i32;
                let d = (b - a).rem_euclid(SEGMENTS as i32);
                let d = d.min(SEGMENTS as i32 - d);
                assert!(d <= limit);
            }
        }
    }

    #[test]
    fn stars_reward_fewer_bounces() {
        assert_eq!(stars_for(4, 8), 3);
        assert_eq!(stars_for(8, 8), 3);
        assert_eq!(stars_for(12, 8), 2);
        assert_eq!(stars_for(40, 8), 1);
    }

    #[test]
    fn a_child_who_lines_up_the_hole_wins_level_one_and_it_is_saved() {
        setup();
        let view = View::ipad_landscape();
        let mut g = Helix::new();
        assert_eq!(g.level, 1);
        bot_plays(&mut g, &view, 60.0);
        assert_eq!(g.state, State::Won);
        assert!(g.stars >= 1);
        assert_eq!(crate::profile::unlocked(GAME), 2);
        assert_eq!(crate::profile::level(GAME), 2);
        assert_eq!(crate::profile::stars(GAME, 1), g.stars);
        assert!(g.rank.is_some());
        assert_eq!(crate::profile::board(GAME).len(), 1);
    }

    #[test]
    fn a_new_game_resumes_at_the_level_reached() {
        setup();
        crate::profile::unlock(GAME, 5);
        crate::profile::set_level(GAME, 5);
        let g = Helix::new();
        assert_eq!(g.level, 5);
        assert_eq!(g.floors.len(), floor_count(5));
    }

    #[test]
    fn progress_is_kept_per_player() {
        setup();
        crate::profile::unlock(GAME, 4);
        crate::profile::set_level(GAME, 4);
        crate::profile::create("Leo", 3);
        assert_eq!(Helix::new().level, 1);
        let mia = crate::profile::list()
            .into_iter()
            .find(|p| p.name == "Mia")
            .unwrap();
        crate::profile::select(&mia.id);
        assert_eq!(Helix::new().level, 4);
    }

    #[test]
    fn level_select_only_opens_unlocked_levels() {
        setup();
        crate::profile::unlock(GAME, 3);
        let view = View::ipad_landscape();
        let l = select_layout(&view);
        assert_eq!(l.cells.len(), MAX_LEVEL as usize);
        let mut g = Helix::new();
        g.open_select();
        g.update(&Input::press_at(l.cells[7].center()), &view, 0.016);
        assert_eq!(g.state, State::Select);
        g.update(&Input::press_at(l.cells[1].center()), &view, 0.016);
        assert_eq!(g.state, State::Ready);
        assert_eq!(g.level, 2);
        g.open_select();
        g.update(&Input::press_at(l.play.center()), &view, 0.016);
        assert_eq!(g.state, State::Ready);
        assert_eq!(g.level, 2);
    }

    #[test]
    fn level_select_fits_on_portrait_and_landscape() {
        for view in [View::ipad_landscape(), View::ipad_portrait()] {
            let l = select_layout(&view);
            for c in &l.cells {
                assert!(c.x >= 0.0 && c.right() <= view.w);
                assert!(c.y >= 0.0 && c.bottom() <= view.h);
            }
            assert!(l.play.bottom() <= view.h);
            assert!(l.play.y > l.cells.last().unwrap().bottom());
        }
    }

    #[test]
    fn the_hint_stays_until_the_first_floor_is_passed() {
        setup();
        let view = View::ipad_landscape();
        let mut g = Helix::new();
        assert!(!g.taught);
        above(&mut g, 2, Seg::Gap);
        g.update(&Input::default(), &view, 0.05);
        assert!(g.taught);
        assert_eq!(crate::profile::get(GAME, "taught"), 1);
        assert!(Helix::new().taught);
    }

    #[test]
    fn dying_posts_the_score_once_and_gives_a_rank() {
        setup();
        let view = View::ipad_landscape();
        let mut g = Helix::with_level(5);
        g.score = 12;
        above(&mut g, 3, Seg::Danger);
        g.update(&Input::default(), &view, 0.05);
        assert_eq!(g.state, State::Dead);
        assert_eq!(g.rank, Some(0));
        for _ in 0..30 {
            g.update(&Input::default(), &view, 0.05);
        }
        assert_eq!(crate::profile::board(GAME).len(), 1);
    }

    #[test]
    fn picking_an_earlier_level_is_what_a_restart_resumes() {
        setup();
        crate::profile::unlock(GAME, 6);
        crate::profile::set_level(GAME, 6);
        let mut g = Helix::new();
        g.enter(2);
        assert_eq!(Helix::new().level, 2);
        assert_eq!(crate::profile::unlocked(GAME), 6);
    }

    fn drag(g: &mut Helix, view: &View, from: f32, to: f32, frames: usize) {
        let mut i = Input::press_at(vec2(from, 300.0));
        i.mm = 4.0;
        g.update(&i, view, 1.0 / 60.0);
        for k in 1..=frames {
            let x = from + (to - from) * k as f32 / frames as f32;
            let mut m = Input {
                pos: vec2(x, 300.0),
                prev_pos: vec2(x, 300.0),
                start: vec2(from, 300.0),
                down: true,
                mm: 4.0,
                ..Default::default()
            };
            m.travel = (x - from).abs();
            g.update(&m, view, 1.0 / 60.0);
        }
    }

    #[test]
    fn a_wobbling_finger_under_a_millimetre_does_not_spin_the_tower() {
        setup();
        let view = View::ipad_landscape();
        let mut g = Helix::new();
        g.state = State::Playing;
        let before = g.angle;
        drag(&mut g, &view, 400.0, 403.0, 6);
        assert_eq!(g.angle, before);
    }

    #[test]
    fn a_real_drag_turns_the_tower_smoothly_and_settles() {
        setup();
        let view = View::ipad_landscape();
        let mut g = Helix::new();
        g.state = State::Playing;
        let before = g.angle;
        drag(&mut g, &view, 400.0, 520.0, 12);
        let mid = g.angle - before;
        assert!(mid > 0.5);
        let steady = Input::default();
        for _ in 0..30 {
            g.update(&steady, &view, 1.0 / 60.0);
        }
        let done = g.angle - before;
        assert!(done > mid);
        assert!(done > 1.2 && done < 1.5);
    }
}
