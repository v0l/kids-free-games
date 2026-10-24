use crate::engine::*;
use crate::fx::*;
use macroquad::prelude::*;

pub const COLS: i32 = 9;
pub const HOP_TIME: f32 = 0.12;
pub const EAGLE_TIME: f32 = 7.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Lane {
    Grass,
    Road { speed: f32, gap: f32 },
    River { speed: f32, gap: f32 },
    Rail { timer: f32 },
}

#[derive(Clone, Debug)]
pub struct Row {
    pub lane: Lane,
    pub things: Vec<f32>,
    pub trees: Vec<i32>,
    pub len: f32,
    pub hue: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum State {
    Ready,
    Playing,
    Dead,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Death {
    Car,
    Water,
    Train,
    Eagle,
    Edge,
}

pub struct Crossy {
    pub rows: Vec<Row>,
    pub x: f32,
    pub row: i32,
    pub from: Vec2,
    pub hop: f32,
    pub facing: Dir,
    pub state: State,
    pub death: Option<Death>,
    pub score: u32,
    pub best: Best,
    pub idle: f32,
    cam: f32,
    t: f32,
    fx: Particles,
    shake: Shake,
    bump: Bump,
    dead_t: f32,
}

fn lane_for(index: i32) -> Lane {
    if index < 3 {
        return Lane::Grass;
    }
    let level = (index as f32 / 40.0).min(1.0);
    let dir = if rand::gen_range(0, 2) == 0 {
        1.0
    } else {
        -1.0
    };
    match rand::gen_range(0, 10) {
        0..=2 => Lane::Grass,
        3..=6 => Lane::Road {
            speed: dir * rand::gen_range(1.6, 2.6 + level * 2.0),
            gap: rand::gen_range(3.2, 5.5 - level),
        },
        7..=8 => Lane::River {
            speed: dir * rand::gen_range(0.9, 1.6 + level),
            gap: rand::gen_range(3.0, 4.2),
        },
        _ => Lane::Rail {
            timer: rand::gen_range(2.0, 5.0),
        },
    }
}

impl Row {
    fn make(index: i32) -> Self {
        let lane = lane_for(index);
        let mut row = Row {
            lane,
            things: vec![],
            trees: vec![],
            len: 1.0,
            hue: rand::gen_range(0.0, 1.0),
        };
        match lane {
            Lane::Grass if index > 0 => {
                for c in 0..COLS {
                    if c != COLS / 2 && rand::gen_range(0, 6) == 0 {
                        row.trees.push(c);
                    }
                }
            }
            Lane::Road { gap, .. } => {
                row.len = if rand::gen_range(0, 4) == 0 { 2.0 } else { 1.2 };
                let mut x = rand::gen_range(-2.0, 0.0);
                while x < COLS as f32 + 4.0 {
                    row.things.push(x);
                    x += gap + row.len;
                }
            }
            Lane::River { gap, .. } => {
                row.len = rand::gen_range(2.0, 3.5f32).round();
                let mut x = rand::gen_range(-2.0, 0.0);
                while x < COLS as f32 + 4.0 {
                    row.things.push(x);
                    x += gap + row.len - 1.0;
                }
            }
            _ => {}
        }
        row
    }

    fn span() -> f32 {
        COLS as f32 + 8.0
    }

    pub fn step(&mut self, dt: f32) -> bool {
        let mut train_arrived = false;
        match &mut self.lane {
            Lane::Road { speed, .. } | Lane::River { speed, .. } => {
                for x in &mut self.things {
                    *x += *speed * dt;
                    if *speed > 0.0 && *x > COLS as f32 + 4.0 {
                        *x -= Self::span();
                    } else if *speed < 0.0 && *x + self.len < -4.0 {
                        *x += Self::span();
                    }
                }
            }
            Lane::Rail { timer } => {
                *timer -= dt;
                if self.things.is_empty() && *timer <= 0.0 {
                    self.things.push(-14.0);
                    train_arrived = true;
                }
                for x in &mut self.things {
                    *x += 34.0 * dt;
                }
                if self.things.first().is_some_and(|x| *x > COLS as f32 + 2.0) {
                    self.things.clear();
                    *timer = rand::gen_range(3.0, 6.0);
                }
            }
            Lane::Grass => {}
        }
        train_arrived
    }

    pub fn warning(&self) -> bool {
        matches!(self.lane, Lane::Rail { timer } if self.things.is_empty() && timer < 1.2)
    }

    pub fn hits(&self, x: f32) -> bool {
        let c = x + 0.5;
        match self.lane {
            Lane::Road { .. } => self.things.iter().any(|t| {
                c + CHICKEN_HALF > *t + CAR_INSET && c - CHICKEN_HALF < *t + self.len - CAR_INSET
            }),
            Lane::Rail { .. } => self
                .things
                .iter()
                .any(|t| c + CHICKEN_HALF > *t && c - CHICKEN_HALF < *t + TRAIN_LEN),
            _ => false,
        }
    }

    pub fn log_under(&self, x: f32) -> Option<usize> {
        let c = x + 0.5;
        self.things.iter().position(|t| c > *t && c < *t + self.len)
    }
}

const ROW_PX: f32 = 72.0;
const CHICKEN_HALF: f32 = 0.3;
const CAR_INSET: f32 = 0.06;
const TRAIN_LEN: f32 = 12.0;

impl Default for Crossy {
    fn default() -> Self {
        Self::new()
    }
}

impl Crossy {
    pub fn new() -> Self {
        let mut g = Self {
            rows: vec![],
            x: (COLS / 2) as f32,
            row: 0,
            from: vec2((COLS / 2) as f32, 0.0),
            hop: 1.0,
            facing: Dir::Up,
            state: State::Ready,
            death: None,
            score: 0,
            best: Best::default(),
            idle: 0.0,
            cam: 0.0,
            t: 0.0,
            fx: Particles::default(),
            shake: Shake::default(),
            bump: Bump::default(),
            dead_t: 0.0,
        };
        g.ensure_rows();
        g
    }

    fn ensure_rows(&mut self) {
        while (self.rows.len() as i32) < self.row + 20 {
            let i = self.rows.len() as i32;
            self.rows.push(Row::make(i));
        }
    }

    pub fn blocked(&self, col: i32, row: i32) -> bool {
        row < 0
            || self
                .rows
                .get(row as usize)
                .is_some_and(|r| r.trees.contains(&col))
    }

    pub fn try_hop(&mut self, d: Dir) -> bool {
        if self.hop < 1.0 || self.state == State::Dead {
            return false;
        }
        self.facing = d;
        let delta = d.delta();
        let target_col = (self.x.round() as i32 + delta.x).clamp(0, COLS - 1);
        let target_row = self.row - delta.y;
        if self.blocked(target_col, target_row) {
            return false;
        }
        self.from = vec2(self.x, self.row as f32);
        self.row = target_row;
        let on_log = matches!(self.rows[self.row as usize].lane, Lane::River { .. });
        self.x = if on_log && delta.x == 0 {
            self.x
        } else {
            target_col as f32
        };
        if !on_log {
            self.x = self.x.round();
        }
        self.hop = 0.0;
        self.idle = 0.0;
        self.ensure_rows();
        if self.row as u32 > self.score {
            self.score = self.row as u32;
            self.bump.kick();
        }
        true
    }

    fn die(&mut self, how: Death) {
        if self.state == State::Dead {
            return;
        }
        self.state = State::Dead;
        self.death = Some(how);
        self.dead_t = 0.0;
        self.best.offer(self.score);
        self.shake.add(if how == Death::Train { 0.9 } else { 0.55 });
    }

    pub fn step(&mut self, dt: f32) {
        for (i, r) in self.rows.iter_mut().enumerate() {
            if r.step(dt) && (i as i32 - self.row).abs() < 8 {
                self.shake.add(0.15);
            }
        }
        if self.state != State::Playing {
            return;
        }
        self.hop = (self.hop + dt / HOP_TIME).min(1.0);
        self.idle += dt;
        let row = &self.rows[self.row as usize];
        if let Lane::River { speed, .. } = row.lane {
            if self.hop >= 1.0 {
                if row.log_under(self.x).is_some() {
                    self.x += speed * dt;
                    if self.x < -0.4 || self.x > COLS as f32 - 0.6 {
                        self.die(Death::Edge);
                    }
                } else {
                    self.die(Death::Water);
                }
            }
        }
        let row = &self.rows[self.row as usize];
        if row.hits(self.x) {
            let how = if matches!(row.lane, Lane::Rail { .. }) {
                Death::Train
            } else {
                Death::Car
            };
            self.die(how);
        }
        if self.idle > EAGLE_TIME {
            self.die(Death::Eagle);
        }
    }

    fn screen_y(&self, row: f32, view: &View) -> f32 {
        view.h - 160.0 - (row - self.cam) * ROW_PX
    }

    fn col_x(&self, view: &View) -> (f32, f32) {
        let cell = ((view.w - 40.0) / COLS as f32).min(ROW_PX * 1.4);
        (cell, (view.w - cell * COLS as f32) / 2.0)
    }

    fn player_pos(&self) -> Vec2 {
        let k = ease_out_cubic(self.hop);
        self.from.lerp(vec2(self.x, self.row as f32), k)
    }
}

impl Game for Crossy {
    fn update(&mut self, input: &Input, view: &View, dt: f32) {
        self.t += dt;
        self.fx.update(dt);
        self.shake.update(dt);
        self.bump.update(dt);
        let target_cam = (self.row as f32 - 2.0).max(self.cam);
        self.cam = approach(self.cam, target_cam, 6.0, dt);
        match self.state {
            State::Ready => {
                self.step(dt);
                if input.tap() || input.dir().is_some() {
                    self.state = State::Playing;
                    self.try_hop(input.dir().unwrap_or(Dir::Up));
                }
            }
            State::Playing => {
                let d = input.dir().or(
                    if input.released
                        && input.swipe.is_none()
                        && (input.pos - input.prev_pos).length() < 20.0
                    {
                        Some(Dir::Up)
                    } else {
                        None
                    },
                );
                if let Some(d) = d {
                    if self.try_hop(d) {
                        let (cell, ox) = self.col_x(view);
                        let p = self.player_pos();
                        self.fx.puff(
                            vec2(ox + (p.x + 0.5) * cell, self.screen_y(p.y, view)),
                            Color::from_rgba(240, 240, 220, 255),
                        );
                    }
                }
                if self.row as f32 + 2.5 < self.cam {
                    self.die(Death::Eagle);
                }
                let before = self.state;
                self.step(dt);
                if before == State::Playing && self.state == State::Dead {
                    let (cell, ox) = self.col_x(view);
                    let p = self.player_pos();
                    let at = vec2(ox + (p.x + 0.5) * cell, self.screen_y(p.y, view));
                    match self.death {
                        Some(Death::Water) => self.fx.emit(
                            at,
                            &[Color::from_rgba(120, 200, 255, 255), WHITE],
                            Burst {
                                count: 24,
                                gravity: 900.0,
                                ..Default::default()
                            },
                        ),
                        _ => self.fx.emit(
                            at,
                            &[WHITE, Color::from_rgba(255, 220, 80, 255)],
                            Burst {
                                count: 30,
                                shape: Shape::Square,
                                ..Default::default()
                            },
                        ),
                    }
                }
            }
            State::Dead => {
                self.dead_t += dt;
                self.step(dt);
                if self.dead_t > 0.8 && input.tap() {
                    let best = self.best;
                    *self = Self::new();
                    self.best = best;
                }
            }
        }
    }

    fn draw(&self, view: &View) {
        clear_background(Color::from_rgba(120, 200, 90, 255));
        let shake = self.shake.offset();
        let (cell, ox) = self.col_x(view);
        let ox = ox + shake.x;
        let first = (self.cam - 4.0).floor().max(0.0) as usize;
        let last = (self.cam + view.h / ROW_PX + 2.0) as usize;
        for i in (first..last.min(self.rows.len())).rev() {
            let r = &self.rows[i];
            let y = self.screen_y(i as f32, view) + shake.y;
            let top = y - ROW_PX / 2.0;
            match r.lane {
                Lane::Grass => {
                    let c = if i % 2 == 0 {
                        Color::from_rgba(140, 215, 100, 255)
                    } else {
                        Color::from_rgba(128, 205, 92, 255)
                    };
                    draw_rectangle(view.full().x, top, view.full().w, ROW_PX, c);
                    for k in 0..6 {
                        let gx = ((i * 37 + k * 71) % 100) as f32 / 100.0 * view.w;
                        draw_circle(gx, top + 12.0 + (k * 13 % 40) as f32, 3.0, darker(c, 0.85));
                    }
                }
                Lane::Road { .. } => {
                    draw_rectangle(
                        view.full().x,
                        top,
                        view.full().w,
                        ROW_PX,
                        Color::from_rgba(80, 84, 96, 255),
                    );
                    let next_road = self
                        .rows
                        .get(i + 1)
                        .is_some_and(|n| matches!(n.lane, Lane::Road { .. }));
                    if next_road {
                        for k in 0..((view.w / 60.0) as i32 + 1) {
                            draw_rectangle(
                                k as f32 * 60.0 + 10.0,
                                top - 3.0,
                                32.0,
                                6.0,
                                Color::from_rgba(230, 230, 230, 200),
                            );
                        }
                    }
                }
                Lane::River { .. } => {
                    draw_rectangle(
                        view.full().x,
                        top,
                        view.full().w,
                        ROW_PX,
                        Color::from_rgba(80, 170, 240, 255),
                    );
                    for k in 0..((view.w / 50.0) as i32 + 2) {
                        let wx = (k as f32 * 50.0 + self.t * 20.0 + i as f32 * 13.0)
                            .rem_euclid(view.w + 50.0)
                            - 25.0;
                        draw_rectangle(
                            wx,
                            top + 20.0 + (k % 3) as f32 * 14.0,
                            18.0,
                            3.0,
                            Color::new(1.0, 1.0, 1.0, 0.35),
                        );
                    }
                }
                Lane::Rail { .. } => {
                    draw_rectangle(
                        view.full().x,
                        top,
                        view.full().w,
                        ROW_PX,
                        Color::from_rgba(150, 130, 110, 255),
                    );
                    for k in 0..((view.w / 26.0) as i32 + 1) {
                        draw_rectangle(
                            k as f32 * 26.0,
                            top + 14.0,
                            12.0,
                            ROW_PX - 28.0,
                            Color::from_rgba(110, 80, 60, 255),
                        );
                    }
                    draw_rectangle(
                        view.full().x,
                        top + 22.0,
                        view.full().w,
                        5.0,
                        Color::from_rgba(190, 190, 200, 255),
                    );
                    draw_rectangle(
                        view.full().x,
                        top + ROW_PX - 27.0,
                        view.full().w,
                        5.0,
                        Color::from_rgba(190, 190, 200, 255),
                    );
                    let lit = r.warning() && (self.t * 8.0).sin() > 0.0;
                    let px = ox - 30.0;
                    draw_rectangle(px, top - 20.0, 6.0, 46.0, DARKGRAY);
                    draw_circle(
                        px + 3.0,
                        top - 22.0,
                        10.0,
                        if lit {
                            RED
                        } else {
                            Color::from_rgba(90, 30, 30, 255)
                        },
                    );
                }
            }
            for c in &r.trees {
                let tx = ox + (*c as f32 + 0.5) * cell;
                soft_shadow(vec2(tx, y + 18.0), cell * 0.4, 10.0, 0.5);
                draw_rectangle(tx - 6.0, y, 12.0, 18.0, Color::from_rgba(120, 80, 50, 255));
                draw_circle(tx, y - 8.0, cell * 0.36, Color::from_rgba(50, 150, 70, 255));
                draw_circle(
                    tx - 6.0,
                    y - 16.0,
                    cell * 0.22,
                    Color::from_rgba(80, 180, 90, 255),
                );
            }
            match r.lane {
                Lane::Road { speed, .. } => {
                    for t in &r.things {
                        let x = ox + t * cell;
                        let w = r.len * cell;
                        let body = hsv(r.hue, 0.65, 0.95);
                        let rr = Rect::new(x + 4.0, y - 22.0, w - 8.0, 44.0);
                        round_rect(
                            Rect::new(rr.x, rr.y + 8.0, rr.w, rr.h),
                            10.0,
                            Color::new(0.0, 0.0, 0.0, 0.25),
                        );
                        round_rect(rr, 10.0, body);
                        let cab = if speed > 0.0 {
                            Rect::new(rr.x + rr.w * 0.45, rr.y + 6.0, rr.w * 0.35, rr.h - 12.0)
                        } else {
                            Rect::new(rr.x + rr.w * 0.2, rr.y + 6.0, rr.w * 0.35, rr.h - 12.0)
                        };
                        round_rect(cab, 6.0, Color::from_rgba(200, 235, 255, 255));
                        gloss(rr, 10.0);
                        let fx = if speed > 0.0 {
                            rr.x + rr.w - 4.0
                        } else {
                            rr.x + 4.0
                        };
                        draw_circle(fx, rr.y + 10.0, 4.0, Color::from_rgba(255, 250, 180, 255));
                        draw_circle(
                            fx,
                            rr.y + rr.h - 10.0,
                            4.0,
                            Color::from_rgba(255, 250, 180, 255),
                        );
                    }
                }
                Lane::River { .. } => {
                    for t in &r.things {
                        let x = ox + t * cell;
                        let w = r.len * cell;
                        let rr = Rect::new(x + 3.0, y - 22.0, w - 6.0, 44.0);
                        round_rect(rr, 20.0, Color::from_rgba(140, 90, 55, 255));
                        round_rect(
                            Rect::new(rr.x, rr.y, rr.w, 16.0),
                            8.0,
                            Color::from_rgba(170, 115, 70, 255),
                        );
                        for k in 1..(r.len as i32 * 2) {
                            draw_line(
                                rr.x + k as f32 * cell / 2.0,
                                rr.y + 20.0,
                                rr.x + k as f32 * cell / 2.0 + 10.0,
                                rr.y + 30.0,
                                2.0,
                                Color::from_rgba(110, 70, 40, 255),
                            );
                        }
                    }
                }
                Lane::Rail { .. } => {
                    for t in &r.things {
                        let x = ox + t * cell;
                        let rr = Rect::new(x, y - 26.0, TRAIN_LEN * cell, 52.0);
                        round_rect(rr, 10.0, Color::from_rgba(220, 60, 60, 255));
                        for k in 0..11 {
                            draw_rectangle(
                                rr.x + 12.0 + k as f32 * cell,
                                rr.y + 10.0,
                                cell * 0.6,
                                16.0,
                                Color::from_rgba(200, 235, 255, 255),
                            );
                        }
                        gloss(rr, 10.0);
                    }
                }
                _ => {}
            }
            if i as i32 == self.row {
                self.draw_chicken(view, cell, ox, shake);
            }
        }
        self.fx.draw();
        score_text(
            &self.score.to_string(),
            view.w / 2.0,
            54.0,
            72.0 * self.bump.scale(0.3),
            1.0,
        );
        text_shadow(
            &format!("Best {}", self.best.0),
            view.w - 100.0,
            40.0,
            30.0,
            WHITE,
        );
        if self.idle > EAGLE_TIME - 2.5 && self.state == State::Playing {
            let k = (self.idle - (EAGLE_TIME - 2.5)) / 2.5;
            text_shadow(
                "Hurry up!",
                view.w / 2.0,
                120.0,
                40.0 * (1.0 + (self.t * 10.0).sin() * 0.05 * k),
                Color::from_rgba(255, 90, 90, 255),
            );
        }
        match self.state {
            State::Ready => {
                let a = 0.6 + (self.t * 3.0).sin() * 0.4;
                text_shadow(
                    "Tap to hop, swipe to turn",
                    view.w / 2.0,
                    view.h * 0.3,
                    40.0,
                    with_alpha(WHITE, a),
                );
            }
            State::Dead if self.dead_t > 0.6 => {
                let why = match self.death {
                    Some(Death::Car) => "Squashed by a car!",
                    Some(Death::Water) => "Splash!",
                    Some(Death::Train) => "Choo choo!",
                    Some(Death::Eagle) | None => "The eagle got you!",
                    Some(Death::Edge) => "Floated away!",
                };
                let s = format!("Score {}   Best {}", self.score, self.best.0);
                overlay(view, "Oh no!", &[why, &s, "Tap to hop again"]);
            }
            _ => {}
        }
    }
}

impl Crossy {
    fn draw_chicken(&self, view: &View, cell: f32, ox: f32, shake: Vec2) {
        let p = self.player_pos();
        let arc = (self.hop * std::f32::consts::PI).sin() * 22.0;
        let c = vec2(
            ox + (p.x + 0.5) * cell,
            self.screen_y(p.y, view) + shake.y - arc,
        );
        let flat = self.state == State::Dead
            && matches!(self.death, Some(Death::Car) | Some(Death::Train));
        if self.state == State::Dead && self.death == Some(Death::Water) {
            return;
        }
        soft_shadow(
            vec2(c.x, c.y + 22.0 + arc),
            20.0 * (1.0 - arc / 60.0),
            7.0,
            0.5,
        );
        let (sx, sy) = if flat {
            (1.6, 0.25)
        } else {
            let s = 1.0 - (self.hop * std::f32::consts::PI).sin() * 0.12;
            (s, 2.0 - s)
        };
        let w = 34.0 * sx;
        let h = 38.0 * sy;
        round_rect(
            Rect::new(c.x - w / 2.0, c.y - h / 2.0, w, h),
            10.0 * sx.min(sy),
            WHITE,
        );
        round_rect(
            Rect::new(c.x - w / 2.0, c.y + h * 0.1, w, h * 0.4),
            8.0 * sx.min(sy),
            Color::from_rgba(235, 235, 235, 255),
        );
        if flat {
            return;
        }
        draw_rectangle(
            c.x - 5.0,
            c.y - h / 2.0 - 8.0,
            10.0,
            10.0,
            Color::from_rgba(240, 60, 60, 255),
        );
        let look = match self.facing {
            Dir::Left => -1.0,
            Dir::Right => 1.0,
            _ => 0.0,
        };
        let up = if self.facing == Dir::Down { 6.0 } else { -4.0 };
        for s in [-1.0, 1.0] {
            draw_circle(c.x + s * 8.0 + look * 4.0, c.y + up - 6.0, 4.0, BLACK);
        }
        draw_triangle(
            vec2(c.x + look * 10.0 - 6.0, c.y + up),
            vec2(c.x + look * 10.0 + 6.0, c.y + up),
            vec2(c.x + look * 18.0, c.y + up + 8.0),
            Color::from_rgba(255, 160, 40, 255),
        );
    }
}

pub fn icon(r: Rect) {
    draw_rectangle(
        r.x,
        r.y,
        r.w,
        r.h * 0.33,
        Color::from_rgba(140, 215, 100, 255),
    );
    draw_rectangle(
        r.x,
        r.y + r.h * 0.33,
        r.w,
        r.h * 0.34,
        Color::from_rgba(80, 84, 96, 255),
    );
    draw_rectangle(
        r.x,
        r.y + r.h * 0.67,
        r.w,
        r.h * 0.33,
        Color::from_rgba(140, 215, 100, 255),
    );
    round_rect(
        Rect::new(r.x + r.w * 0.05, r.y + r.h * 0.38, r.w * 0.4, r.h * 0.22),
        6.0,
        Color::from_rgba(80, 160, 250, 255),
    );
    let c = vec2(r.x + r.w * 0.65, r.y + r.h * 0.78);
    round_rect(
        Rect::new(c.x - r.w * 0.12, c.y - r.h * 0.14, r.w * 0.24, r.h * 0.26),
        6.0,
        WHITE,
    );
    draw_rectangle(
        c.x - 3.0,
        c.y - r.h * 0.2,
        6.0,
        6.0,
        Color::from_rgba(240, 60, 60, 255),
    );
    draw_triangle(
        vec2(c.x - 4.0, c.y),
        vec2(c.x + 4.0, c.y),
        vec2(c.x, c.y + 6.0),
        Color::from_rgba(255, 160, 40, 255),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    fn flat_world() -> Crossy {
        let mut g = Crossy::new();
        for r in &mut g.rows {
            *r = Row {
                lane: Lane::Grass,
                things: vec![],
                trees: vec![],
                len: 1.0,
                hue: 0.0,
            };
        }
        g.state = State::Playing;
        g
    }

    fn settle(g: &mut Crossy) {
        for _ in 0..12 {
            g.step(1.0 / 60.0);
        }
    }

    #[test]
    fn hopping_forward_scores() {
        let mut g = flat_world();
        assert!(g.try_hop(Dir::Up));
        settle(&mut g);
        assert!(g.try_hop(Dir::Up));
        assert_eq!(g.score, 2);
        assert_eq!(g.row, 2);
    }

    #[test]
    fn going_back_does_not_score_again() {
        let mut g = flat_world();
        g.try_hop(Dir::Up);
        settle(&mut g);
        g.try_hop(Dir::Down);
        settle(&mut g);
        g.try_hop(Dir::Up);
        assert_eq!(g.score, 1);
    }

    #[test]
    fn cannot_hop_mid_air() {
        let mut g = flat_world();
        assert!(g.try_hop(Dir::Up));
        assert!(!g.try_hop(Dir::Up));
    }

    #[test]
    fn trees_block_movement() {
        let mut g = flat_world();
        let col = g.x as i32;
        g.rows[1].trees.push(col);
        assert!(!g.try_hop(Dir::Up));
        assert_eq!(g.row, 0);
    }

    #[test]
    fn car_hits_chicken() {
        let mut g = flat_world();
        g.rows[1] = Row {
            lane: Lane::Road {
                speed: 2.0,
                gap: 4.0,
            },
            things: vec![g.x - 0.5],
            trees: vec![],
            len: 1.2,
            hue: 0.0,
        };
        g.try_hop(Dir::Up);
        settle(&mut g);
        assert_eq!(g.state, State::Dead);
        assert_eq!(g.death, Some(Death::Car));
    }

    fn road_with_car_at(t: f32) -> Row {
        Row {
            lane: Lane::Road {
                speed: 0.0,
                gap: 4.0,
            },
            things: vec![t],
            trees: vec![],
            len: 1.2,
            hue: 0.0,
        }
    }

    #[test]
    fn car_that_passed_behind_the_chicken_does_not_hit() {
        let r = road_with_car_at(2.9);
        assert!(
            !r.hits(4.0),
            "car drawn at 2.9..4.1 is clear of a chicken in cell 4"
        );
    }

    #[test]
    fn car_nose_touching_the_chicken_hits() {
        let r = road_with_car_at(4.55);
        assert!(
            r.hits(4.0),
            "car drawn from 4.55 overlaps a chicken centred at 4.5"
        );
    }

    #[test]
    fn standing_on_the_drawn_part_of_a_log_is_safe() {
        let r = Row {
            lane: Lane::River {
                speed: 0.0,
                gap: 4.0,
            },
            things: vec![4.6],
            trees: vec![],
            len: 2.0,
            hue: 0.0,
        };
        assert!(
            r.log_under(4.2).is_some(),
            "chicken centred at 4.7 stands on a log drawn 4.6..6.6"
        );
        assert!(
            r.log_under(3.9).is_none(),
            "chicken centred at 4.4 is in the water before the log"
        );
        assert!(
            r.log_under(6.3).is_none(),
            "chicken centred at 6.8 is past the log end at 6.6"
        );
    }

    #[test]
    fn water_without_log_drowns_and_log_carries() {
        let mut g = flat_world();
        g.rows[1] = Row {
            lane: Lane::River {
                speed: 1.0,
                gap: 4.0,
            },
            things: vec![-10.0],
            trees: vec![],
            len: 2.0,
            hue: 0.0,
        };
        g.try_hop(Dir::Up);
        settle(&mut g);
        assert_eq!(g.death, Some(Death::Water));

        let mut g = flat_world();
        let x = g.x;
        g.rows[1] = Row {
            lane: Lane::River {
                speed: 1.0,
                gap: 4.0,
            },
            things: vec![x - 1.0],
            trees: vec![],
            len: 3.0,
            hue: 0.0,
        };
        g.try_hop(Dir::Up);
        settle(&mut g);
        assert_eq!(g.state, State::Playing);
        assert!(g.x > x);
    }

    #[test]
    fn standing_still_too_long_brings_the_eagle() {
        let mut g = flat_world();
        for _ in 0..((EAGLE_TIME + 0.5) * 60.0) as usize {
            g.step(1.0 / 60.0);
        }
        assert_eq!(g.death, Some(Death::Eagle));
    }

    #[test]
    fn train_arrives_after_its_timer() {
        let mut r = Row {
            lane: Lane::Rail { timer: 0.5 },
            things: vec![],
            trees: vec![],
            len: 1.0,
            hue: 0.0,
        };
        assert!(!r.warning() || r.things.is_empty());
        let mut arrived = false;
        for _ in 0..60 {
            arrived |= r.step(1.0 / 60.0);
        }
        assert!(arrived);
        assert!(r.hits(-14.0 + 34.0 * 0.5 + 1.0) || !r.things.is_empty());
    }

    #[test]
    fn start_rows_are_safe_grass() {
        for _ in 0..20 {
            let g = Crossy::new();
            for r in &g.rows[..3] {
                assert_eq!(r.lane, Lane::Grass);
            }
            assert!(!g.rows[0].trees.contains(&(COLS / 2)));
        }
    }
}
