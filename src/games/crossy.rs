use crate::engine::*;
use crate::feel::{self, Impact};
use crate::fx::*;
use crate::sfx::{self, Sfx};
use macroquad::prelude::*;

pub const COLS: i32 = 9;
pub const HOP_TIME: f32 = 0.12;
pub const EAGLE_TIME: f32 = 7.0;
pub const EAGLE_EASY: f32 = 12.0;
pub const STAGE_ROWS: i32 = 10;
pub const GAME: &str = "crossy";
const RAIL_WARN: f32 = 1.8;
pub const MAX_CHECKPOINT: i32 = 4;
const BANNER_TIME: f32 = 2.4;

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
    pub start_row: i32,
    pub top: i32,
    pub gentle: bool,
    pub ease: u32,
    pub rank: Option<usize>,
    pub oops: f32,
    cheer: Option<i32>,
    banner: Option<(i32, f32)>,
    cam: f32,
    t: f32,
    fx: Particles,
    shake: Shake,
    bump: Bump,
    dead_t: f32,
}

pub fn saved_stage() -> i32 {
    crate::profile::get(GAME, "stage").clamp(0, 99) as i32
}

pub fn zone_name(stage: i32) -> &'static str {
    match stage {
        0 => "Quiet roads",
        1 => "Busy roads",
        2 => "River time",
        3 => "Train tracks",
        _ => "Rush hour",
    }
}

pub fn stage_of(row: i32) -> i32 {
    row / STAGE_ROWS
}

fn ramp(s: i32, from: f32, to: f32, over: f32) -> f32 {
    from + (to - from) * (s as f32 / over).clamp(0.0, 1.0)
}

fn max_run(s: i32) -> usize {
    match s {
        0..=2 => 2,
        3..=4 => 3,
        _ => 4,
    }
}

fn random_dir() -> f32 {
    if rand::gen_range(0, 2) == 0 {
        1.0
    } else {
        -1.0
    }
}

pub fn road_speed_range(s: i32) -> (f32, f32) {
    (ramp(s, 0.7, 1.6, 4.0), ramp(s, 1.0, 4.6, 8.0))
}

pub fn road_gap_range(s: i32) -> (f32, f32) {
    (ramp(s, 6.5, 3.2, 6.0), ramp(s, 8.0, 4.5, 6.0))
}

fn calm(ease: u32) -> f32 {
    1.0 - 0.1 * ease as f32
}

fn road_lane(s: i32, ease: u32) -> Lane {
    let (lo, hi) = road_speed_range(s);
    let (glo, ghi) = road_gap_range(s);
    Lane::Road {
        speed: random_dir() * rand::gen_range(lo, hi) * calm(ease),
        gap: rand::gen_range(glo, ghi) + 0.4 * ease as f32,
    }
}

fn river_lane(s: i32, ease: u32) -> Lane {
    Lane::River {
        speed: random_dir()
            * rand::gen_range(ramp(s, 0.5, 0.9, 4.0), ramp(s, 0.9, 2.6, 8.0))
            * calm(ease),
        gap: rand::gen_range(ramp(s, 2.0, 3.0, 5.0), ramp(s, 2.6, 4.2, 5.0)),
    }
}

fn pick(weights: [i32; 4]) -> usize {
    let mut n = rand::gen_range(0, weights.iter().sum::<i32>());
    for (i, w) in weights.iter().enumerate() {
        if n < *w {
            return i;
        }
        n -= w;
    }
    0
}

pub fn lane_for(index: i32, run: usize, ease: u32) -> Lane {
    let s = stage_of(index);
    let at = index % STAGE_ROWS;
    if s == 0 {
        return if index == 3 || index == 7 {
            road_lane(0, ease)
        } else {
            Lane::Grass
        };
    }
    if at < 2 {
        return Lane::Grass;
    }
    match (s, at) {
        (1, 2 | 3) => return road_lane(s, ease),
        (2, 2) => return river_lane(s, ease),
        (3, 2) => {
            return Lane::Rail {
                timer: rand::gen_range(2.5, 4.0),
            }
        }
        _ => {}
    }
    if run >= max_run(s) {
        return Lane::Grass;
    }
    let weights = match s {
        1 => [5, 5, 0, 0],
        2 => [4, 4, 2, 0],
        _ => [3, 4, 2, 1],
    };
    match pick(weights) {
        0 => Lane::Grass,
        1 => road_lane(s, ease),
        2 => river_lane(s, ease),
        _ => Lane::Rail {
            timer: rand::gen_range(2.0, 5.0),
        },
    }
}

impl Row {
    fn make(index: i32, run: usize, ease: u32) -> Self {
        let lane = lane_for(index, run, ease);
        let s = stage_of(index);
        let mut row = Row {
            lane,
            things: vec![],
            trees: vec![],
            len: 1.0,
            hue: rand::gen_range(0.0, 1.0),
        };
        match lane {
            Lane::Grass if index > 0 => {
                let checkpoint = index % STAGE_ROWS == 0;
                for c in 0..COLS {
                    let clear = checkpoint && (c - COLS / 2).abs() <= 1;
                    if c != COLS / 2 && !clear && rand::gen_range(0, 6) == 0 {
                        row.trees.push(c);
                    }
                }
            }
            Lane::Road { gap, .. } => {
                row.len = if rand::gen_range(0, 4) == 0 { 1.7 } else { 1.2 };
                let mut x = rand::gen_range(-2.0, 0.0);
                while x < COLS as f32 + 4.0 {
                    row.things.push(x);
                    x += gap + row.len;
                }
            }
            Lane::River { gap, .. } => {
                let t = (s - 2).max(0);
                row.len = rand::gen_range(ramp(t, 3.0, 2.0, 4.0), ramp(t, 4.0, 3.5, 4.0)).round();
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
        matches!(self.lane, Lane::Rail { timer } if self.things.is_empty() && timer < RAIL_WARN)
    }

    pub fn hits(&self, x: f32) -> bool {
        let c = x + 0.5;
        match self.lane {
            Lane::Road { .. } => self.things.iter().any(|t| {
                let (a, b) = car_span(*t, self.len);
                c + CHICKEN_HALF > a && c - CHICKEN_HALF < b
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
const HOPPER_H: f32 = 84.0;
const CHICKEN_HALF: f32 = 0.3;
const CAR_INSET: f32 = 0.05;
const CARS: [&str; 6] = [
    "car_red",
    "car_blue",
    "car_yellow",
    "car_green",
    "car_orange",
    "car_purple",
];

pub fn car_span(t: f32, len: f32) -> (f32, f32) {
    (t + CAR_INSET, t + len - CAR_INSET)
}

mod art {
    crate::sprites!("crossy");
}
const TRAIN_LEN: f32 = 12.0;

impl Default for Crossy {
    fn default() -> Self {
        Self::new()
    }
}

impl Crossy {
    pub fn new() -> Self {
        Self::starting_at(0)
    }

    pub fn starting_at(stage: i32) -> Self {
        let start_row = stage * STAGE_ROWS;
        let mut g = Self {
            rows: vec![],
            x: (COLS / 2) as f32,
            row: start_row,
            from: vec2((COLS / 2) as f32, start_row as f32),
            hop: 1.0,
            facing: Dir::Up,
            state: State::Ready,
            death: None,
            score: 0,
            best: Best::load(GAME),
            idle: 0.0,
            start_row,
            top: start_row,
            gentle: saved_stage() < 1,
            ease: crate::profile::get(GAME, "ease").clamp(0, 3) as u32,
            rank: None,
            oops: 0.0,
            cheer: None,
            banner: None,
            cam: (start_row as f32 - 2.0).max(0.0),
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
            let run = self
                .rows
                .iter()
                .rev()
                .take_while(|r| r.lane != Lane::Grass)
                .count();
            self.rows.push(Row::make(i, run, self.ease));
        }
    }

    pub fn checkpoint_offer(&self) -> Option<i32> {
        let n = saved_stage().min(MAX_CHECKPOINT);
        (n >= 1).then_some(n)
    }

    fn eagle_limit(&self) -> Option<f32> {
        if self.gentle {
            None
        } else if stage_of(self.row) < 1 {
            Some(EAGLE_EASY)
        } else {
            Some(EAGLE_TIME)
        }
    }

    fn bounce_back(&mut self) {
        let mut r = self.row - 1;
        let col = (self.x.round() as i32).clamp(0, COLS - 1);
        while r > 0
            && (self.rows[r as usize].lane != Lane::Grass
                || self.rows[r as usize].trees.contains(&col))
        {
            r -= 1;
        }
        self.from = vec2(self.x, self.row as f32);
        self.row = r.max(0);
        self.x = col as f32;
        self.hop = 0.0;
        self.idle = 0.0;
        self.oops = 2.5;
        self.shake.add(0.4);
        sfx::play(Sfx::Hit);
        feel::impact(Impact::Medium);
    }

    fn adjust_ease(&mut self) {
        if self.score < 8 {
            self.ease = (self.ease + 1).min(3);
        } else if self.score >= 20 {
            self.ease = self.ease.saturating_sub(1);
        }
        crate::profile::set(GAME, "ease", self.ease as i64);
    }

    pub fn coach(&self) -> Option<&'static str> {
        if !self.gentle || self.state != State::Playing {
            return None;
        }
        let ahead = self.rows.get(self.row as usize + 1)?;
        if matches!(ahead.lane, Lane::Road { .. }) {
            Some("Wait for a gap, then hop!")
        } else if self.score < 2 {
            Some("Tap to hop forward")
        } else {
            None
        }
    }

    pub fn goal_progress(&self) -> f32 {
        (self.top - stage_of(self.top) * STAGE_ROWS) as f32 / STAGE_ROWS as f32
    }

    pub fn goal_stage(&self) -> i32 {
        stage_of(self.top) + 1
    }

    pub fn checkpoint_button(&self, view: &View) -> Rect {
        Rect::new(view.w / 2.0 - 170.0, view.h * 0.42, 340.0, 84.0)
    }

    fn begin_at(&mut self, stage: i32, input: &Input) {
        *self = Self::starting_at(stage);
        self.state = State::Playing;
        sfx::say(sfx::Say::Go);
        self.try_hop(input.dir().unwrap_or(Dir::Up));
    }

    fn reach_checkpoint(&mut self) {
        let stage = stage_of(self.row);
        crate::profile::set(GAME, "stage", stage.max(saved_stage()) as i64);
        self.gentle = false;
        self.cheer = Some(stage);
        self.banner = Some((stage, 0.0));
        sfx::play(Sfx::LevelUp);
        sfx::say(if stage == 1 {
            sfx::Say::Great
        } else {
            sfx::Say::LevelUp
        });
        feel::impact_tinted(Impact::Large, Color::from_rgba(255, 210, 80, 255));
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
        if self.row > self.top {
            self.top = self.row;
            self.score = (self.row - self.start_row) as u32;
            self.bump.kick();
            if self.row % STAGE_ROWS == 0 && stage_of(self.row) > saved_stage() {
                self.reach_checkpoint();
            }
        }
        true
    }

    fn die(&mut self, how: Death) {
        if self.state == State::Dead {
            return;
        }
        sfx::play(match how {
            Death::Water => Sfx::Splash,
            Death::Train => Sfx::Boom,
            Death::Eagle => Sfx::Whoosh,
            _ => Sfx::Hit,
        });
        if self.gentle && how != Death::Eagle {
            self.bounce_back();
            return;
        }
        sfx::play(Sfx::Lose);
        if self.score > self.best.0 && self.best.0 > 0 {
            sfx::say(sfx::Say::Best);
        }
        self.state = State::Dead;
        self.death = Some(how);
        self.dead_t = 0.0;
        self.rank = self.best.submit(GAME, self.score);
        self.adjust_ease();
        self.shake.add(if how == Death::Train { 0.9 } else { 0.55 });
        feel::impact(Impact::Large);
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
        if self.eagle_limit().is_some_and(|limit| self.idle > limit) {
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
        self.oops = (self.oops - dt).max(0.0);
        if let Some((_, age)) = &mut self.banner {
            *age += dt;
            if *age > BANNER_TIME {
                self.banner = None;
            }
        }
        if self.cheer.take().is_some() {
            self.fx.confetti(view);
        }
        self.fx.update(dt);
        self.shake.update(dt);
        self.bump.update(dt);
        let target_cam = (self.row as f32 - 2.0).max(self.cam);
        self.cam = approach(self.cam, target_cam, 6.0, dt);
        match self.state {
            State::Ready => {
                self.step(dt);
                let offer = self.checkpoint_offer();
                if let Some(n) = offer.filter(|_| input.tapped_in(self.checkpoint_button(view))) {
                    self.begin_at(n, input);
                } else if input.tap() || input.dir().is_some() {
                    self.state = State::Playing;
                    sfx::say(sfx::Say::Go);
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
                        sfx::play(Sfx::Tap);
                        let (cell, ox) = self.col_x(view);
                        let p = self.player_pos();
                        self.fx.puff(
                            vec2(ox + (p.x + 0.5) * cell, self.screen_y(p.y, view)),
                            Color::from_rgba(240, 240, 220, 255),
                        );
                    }
                }
                if self.row as f32 + 2.5 < self.cam && !self.gentle {
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
                    *self = Self::new();
                }
            }
        }
    }

    fn draw(&self, view: &View) {
        let shake = self.shake.offset();
        let (cell, ox) = self.col_x(view);
        let ox = ox + shake.x;
        let first = (self.cam - 4.0).floor().max(0.0) as usize;
        let last = (self.cam + view.h / ROW_PX + 2.0) as usize;
        let f = view.full();
        let row0 = self.screen_y(0.0, view) + shake.y + ROW_PX / 2.0;
        tiled("grass", f, row0, f.y + f.h + ROW_PX - row0, WHITE);
        for i in (first..last.min(self.rows.len())).rev() {
            let r = &self.rows[i];
            let y = self.screen_y(i as f32, view) + shake.y;
            let top = y - ROW_PX / 2.0;
            let above_road = self
                .rows
                .get(i + 1)
                .is_some_and(|n| matches!(n.lane, Lane::Road { .. }));
            match r.lane {
                Lane::Grass => {
                    let k = if i % 2 == 0 { 1.0 } else { 0.9 };
                    tiled_offset("grass", f, top, ROW_PX, Color::new(k, k, k, 1.0), i);
                }
                Lane::Road { .. } => {
                    tiled_offset("road", f, top, ROW_PX, WHITE, i);
                    if !above_road {
                        tiled_strip("kerb", f, top, KERB_H, false);
                    } else {
                        for k in 0..((f.w / 60.0) as i32 + 2) {
                            fill_dash(f.x + k as f32 * 60.0 + 10.0, top - 3.0);
                        }
                    }
                    let below_is_road = i > 0 && matches!(self.rows[i - 1].lane, Lane::Road { .. });
                    if !below_is_road {
                        tiled_strip("kerb", f, top + ROW_PX - KERB_H, KERB_H, true);
                    }
                }
                Lane::River { .. } => {
                    tiled_offset("water", f, top, ROW_PX, WHITE, i);
                    for k in 0..((f.w / 50.0) as i32 + 2) {
                        let wx = (k as f32 * 50.0 + self.t * 20.0 + i as f32 * 13.0)
                            .rem_euclid(f.w + 50.0)
                            - 25.0
                            + f.x;
                        draw_rectangle(
                            wx,
                            top + 20.0 + (k % 3) as f32 * 14.0,
                            18.0,
                            3.0,
                            Color::new(1.0, 1.0, 1.0, 0.4),
                        );
                    }
                }
                Lane::Rail { .. } => {
                    tiled("rail", f, top, ROW_PX, WHITE);
                    let lit = r.warning() && (self.t * 8.0).sin() > 0.0;
                    let px = ox - 30.0;
                    crate::ui::sprite_at(
                        "round_white",
                        vec2(px, top + 6.0),
                        30.0,
                        crate::art::Draw {
                            tint: if lit {
                                RED
                            } else {
                                Color::from_rgba(120, 60, 60, 255)
                            },
                            ..Default::default()
                        },
                    );
                }
            }
            for c in &r.trees {
                let tx = ox + (*c as f32 + 0.5) * cell;
                soft_shadow(vec2(tx, y + 20.0), cell * 0.38, 9.0, 0.45);
                let (name, h) = match (*c as usize + i) % 3 {
                    0 => ("tree", ROW_PX * 1.7),
                    1 => ("bush", ROW_PX * 0.95),
                    _ => ("rock", ROW_PX * 0.8),
                };
                art::sprite_at(name, vec2(tx, y + 24.0 - h / 2.0), h, Default::default());
            }
            match r.lane {
                Lane::Road { speed, .. } => {
                    for (k, t) in r.things.iter().enumerate() {
                        let (a, b) = car_span(*t, r.len);
                        let cx = ox + (a + b) / 2.0 * cell;
                        let w = (b - a) * cell;
                        let name = vehicle(r, k);
                        let h = w * art::size(name).y / art::size(name).x;
                        soft_shadow(vec2(cx, y + 24.0), w * 0.42, 8.0, 0.4);
                        art::sprite_ex(
                            name,
                            Rect::new(cx - w / 2.0, y + 28.0 - h, w, h),
                            crate::art::Draw {
                                flip_x: speed < 0.0,
                                ..Default::default()
                            },
                        );
                    }
                }
                Lane::River { .. } => {
                    for t in &r.things {
                        let x = ox + t * cell;
                        let w = r.len * cell;
                        soft_shadow(vec2(x + w / 2.0, y + 22.0), w * 0.45, 8.0, 0.3);
                        art::nine(
                            "log",
                            Rect::new(x + 3.0, y - 28.0, w - 6.0, 56.0),
                            46.0,
                            0.5,
                            WHITE,
                        );
                    }
                }
                Lane::Rail { .. } => {
                    for t in &r.things {
                        for k in 0..6 {
                            let slot = 2.0 * cell;
                            let name = if k == 5 { "engine" } else { "wagon" };
                            let x = ox + (t + k as f32 * 2.0) * cell;
                            let h = if k == 5 { 118.0 } else { 92.0 };
                            let w = h * art::size(name).x / art::size(name).y;
                            let w = w.min(slot + 8.0);
                            art::sprite(name, Rect::new(x + slot - w, y + 30.0 - h, w, h));
                        }
                    }
                }
                _ => {}
            }
            if i as i32 == self.row {
                self.draw_chicken(view, cell, ox, shake);
            }
        }
        self.fx.draw();
        big_score(view, &self.score.to_string(), 60.0, self.bump.t);
        chip(view, Side::Right, "BEST", &self.best.0.to_string(), 1.0);
        self.draw_goal(view);
        if let Some(limit) = self.eagle_limit() {
            if self.idle > limit - 2.5 && self.state == State::Playing {
                let k = (self.idle - (limit - 2.5)) / 2.5;
                text_shadow(
                    "Hurry up!",
                    view.w / 2.0,
                    210.0,
                    40.0 * (1.0 + (self.t * 10.0).sin() * 0.05 * k),
                    Color::from_rgba(255, 90, 90, 255),
                );
            }
        }
        if let Some((stage, age)) = self.banner {
            let fade = (1.0 - (age - (BANNER_TIME - 0.5)) / 0.5).clamp(0.0, 1.0);
            let pop = ease_out_back((age / 0.35).min(1.0));
            let title = format!("Checkpoint {stage}");
            crate::ui::ribbon(view, &title, vec2(view.w / 2.0, view.h * 0.3), self.t);
            text_shadow(
                &format!("Next: {}", zone_name(stage)),
                view.w / 2.0,
                view.h * 0.3 + 70.0,
                36.0 * pop.max(0.01),
                with_alpha(WHITE, fade),
            );
        }
        match self.state {
            State::Ready => {
                let pulse = 1.0 + (self.t * 5.0).sin() * 0.05;
                crate::ui::hint(
                    view,
                    "Tap to hop, swipe to turn",
                    vec2(view.w / 2.0, view.h * 0.3),
                    pulse,
                );
                if let Some(n) = self.checkpoint_offer() {
                    crate::ui::painted_button(
                        self.checkpoint_button(view),
                        &format!("Start at checkpoint {n}"),
                        "blue",
                        0.0,
                        1.0,
                    );
                }
            }
            State::Playing if self.oops > 0.0 => {
                text_shadow(
                    "Oops! Try again",
                    view.w / 2.0,
                    view.h * 0.3,
                    46.0,
                    Color::from_rgba(255, 220, 90, 255),
                );
            }
            State::Playing if self.banner.is_none() => {
                if let Some(text) = self.coach() {
                    let pulse = 1.0 + (self.t * 5.0).sin() * 0.05;
                    crate::ui::hint(view, text, vec2(view.w / 2.0, view.h * 0.3), pulse);
                }
            }
            State::Dead if self.dead_t > 0.6 => {
                let why = match self.death {
                    Some(Death::Car) => "Squashed by a car!",
                    Some(Death::Water) => "Splash!",
                    Some(Death::Train) => "Choo choo!",
                    Some(Death::Eagle) | None => "The eagle got you!",
                    Some(Death::Edge) => "Floated away!",
                };
                let line = format!("{why}  Score {}", self.score);
                results(
                    view,
                    &crate::ui::Results {
                        game: GAME,
                        title: "Oh no!",
                        line: &line,
                        rank: self.rank,
                        action: "Hop again",
                        format: crate::ui::plain_score,
                    },
                );
            }
            _ => {}
        }
    }
}

impl Crossy {
    fn draw_goal(&self, view: &View) {
        let w = (view.w * 0.6).min(360.0);
        let bar = Rect::new(view.w / 2.0 - w / 2.0, 122.0, w, 18.0);
        round_rect(bar, 9.0, Color::new(0.0, 0.0, 0.0, 0.35));
        let fill = self.goal_progress().clamp(0.0, 1.0);
        if fill > 0.0 {
            let inner = Rect::new(bar.x + 2.0, bar.y + 2.0, (bar.w - 4.0) * fill, bar.h - 4.0);
            round_rect(inner, 7.0, Color::from_rgba(255, 200, 40, 255));
        }
        let flag = vec2(bar.x + bar.w, bar.y + bar.h / 2.0);
        crate::ui::sprite_ex(
            "p_star",
            Rect::new(flag.x - 20.0, flag.y - 20.0, 40.0, 40.0),
            Default::default(),
        );
        text_shadow(
            &format!("Reach checkpoint {}", self.goal_stage()),
            view.w / 2.0,
            165.0,
            26.0,
            WHITE,
        );
    }
}

pub fn vehicle(r: &Row, k: usize) -> &'static str {
    if r.len > 1.5 {
        ["bus", "truck"][((r.hue * 2.0) as usize + k) % 2]
    } else {
        CARS[((r.hue * CARS.len() as f32) as usize + k) % CARS.len()]
    }
}

const KERB_H: f32 = 15.0;

fn tiled(name: &str, f: Rect, top: f32, h: f32, tint: Color) {
    tiled_offset(name, f, top, h, tint, 0);
}

fn tiled_offset(name: &str, f: Rect, top: f32, h: f32, tint: Color, shift: usize) {
    let s = art::size(name);
    let w = h.max(ROW_PX) * s.x / s.y;
    let mut x = f.x - (shift as f32 * w * 0.37).rem_euclid(w);
    while x < f.x + f.w {
        art::sprite_ex(
            name,
            Rect::new(x, top, w + 1.0, h),
            crate::art::Draw {
                tint,
                ..Default::default()
            },
        );
        x += w;
    }
}

fn tiled_strip(name: &str, f: Rect, top: f32, h: f32, flip_y: bool) {
    let s = art::size(name);
    let w = h * s.x / s.y;
    let mut x = f.x;
    while x < f.x + f.w {
        art::sprite_ex(
            name,
            Rect::new(x, top, w + 1.0, h),
            crate::art::Draw {
                flip_y,
                ..Default::default()
            },
        );
        x += w;
    }
}

fn fill_dash(x: f32, y: f32) {
    draw_rectangle(x, y, 32.0, 6.0, Color::from_rgba(240, 240, 240, 210));
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
        let frame = if arc > 6.0 && !flat {
            "hopper_air"
        } else {
            "hopper"
        };
        let h = HOPPER_H * sy;
        let w = HOPPER_H * art::size(frame).x / art::size(frame).y * sx;
        let bottom = c.y + 26.0;
        art::sprite_ex(
            frame,
            Rect::new(c.x - w / 2.0, bottom - h, w, h),
            crate::art::Draw {
                flip_x: self.facing == Dir::Left,
                ..Default::default()
            },
        );
    }
}

pub fn icon(r: Rect) {
    crate::ui::icon("crossy", r);
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
        g.gentle = false;
        g
    }

    fn fresh_player() {
        crate::save::reset_for_test();
        crate::profile::create("Mia", 0);
    }

    fn tap_player(g: &mut Crossy, every: f32, seconds: f32) -> Option<f32> {
        let view = View::ipad_landscape();
        let dt = 1.0 / 60.0;
        let mut since = every;
        let mut t = 0.0;
        while t < seconds {
            since += dt;
            let mut input = Input::default();
            if since >= every {
                input.pressed = true;
                input.released = true;
                since = 0.0;
            }
            g.update(&input, &view, dt);
            t += dt;
            if g.state == State::Dead {
                return Some(t);
            }
        }
        None
    }

    fn settle(g: &mut Crossy) {
        for _ in 0..12 {
            g.step(1.0 / 60.0);
        }
    }

    #[test]
    fn crossy_sprites_all_exist() {
        let have = art::names();
        let mut names = vec![
            "grass",
            "road",
            "kerb",
            "water",
            "rail",
            "tree",
            "bush",
            "rock",
            "bus",
            "truck",
            "engine",
            "wagon",
            "log",
            "hopper",
            "hopper_air",
        ];
        names.extend(CARS);
        for n in names {
            assert!(have.iter().any(|h| h == n), "missing {n}");
        }
    }

    #[test]
    fn hitbox_follows_the_drawn_car_body() {
        let (a, b) = car_span(2.0, 1.2);
        assert!((a - 2.05).abs() < 1e-5 && (b - 3.15).abs() < 1e-5);
        let road = Row {
            lane: Lane::Road {
                speed: 1.0,
                gap: 4.0,
            },
            things: vec![2.0],
            trees: vec![],
            len: 1.2,
            hue: 0.0,
        };
        assert!(road.hits(2.0));
        assert!(!road.hits(3.5));
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
        for _ in 0..((EAGLE_EASY + 0.5) * 60.0) as usize {
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

    #[test]
    fn every_vehicle_the_lanes_pick_has_a_sprite() {
        let have = art::names();
        for hue in [0.0, 0.3, 0.6, 0.99] {
            for len in [1.2, 1.7] {
                for k in 0..12 {
                    let row = Row {
                        lane: Lane::Grass,
                        things: vec![],
                        trees: vec![],
                        len,
                        hue,
                    };
                    let name = vehicle(&row, k);
                    assert!(have.iter().any(|h| h == name), "missing {name}");
                }
            }
        }
    }

    #[test]
    fn a_child_tapping_once_a_second_survives_the_first_ten_seconds() {
        for seed in 0..40 {
            fresh_player();
            rand::srand(seed);
            let mut g = Crossy::new();
            assert!(g.gentle);
            assert_eq!(tap_player(&mut g, 1.0, 10.0), None, "seed {seed}");
            assert!(g.score >= 6, "seed {seed} score {}", g.score);
        }
    }

    #[test]
    fn a_blind_tapper_never_meets_the_eagle_while_learning() {
        fresh_player();
        let mut g = Crossy::new();
        assert_eq!(tap_player(&mut g, 100.0, 30.0), None);
        assert_ne!(g.death, Some(Death::Eagle));
    }

    #[test]
    fn a_hit_while_learning_hops_back_to_grass_instead_of_dying() {
        fresh_player();
        let mut g = Crossy::new();
        g.state = State::Playing;
        g.rows[3] = Row {
            lane: Lane::Road {
                speed: 0.0,
                gap: 4.0,
            },
            things: vec![g.x - 0.5],
            trees: vec![],
            len: 1.2,
            hue: 0.0,
        };
        g.row = 3;
        g.from = vec2(g.x, 3.0);
        g.hop = 1.0;
        g.step(1.0 / 60.0);
        assert_eq!(g.state, State::Playing);
        assert_eq!(g.rows[g.row as usize].lane, Lane::Grass);
        assert!(g.row < 3);
        assert!(g.oops > 0.0);
    }

    #[test]
    fn the_first_stage_is_grass_with_two_slow_roads() {
        for seed in 0..30 {
            fresh_player();
            rand::srand(seed);
            let g = Crossy::new();
            let roads: Vec<usize> = (0..STAGE_ROWS as usize)
                .filter(|i| matches!(g.rows[*i].lane, Lane::Road { .. }))
                .collect();
            assert_eq!(roads, vec![3, 7]);
            for i in &roads {
                if let Lane::Road { speed, gap } = g.rows[*i].lane {
                    assert!(speed.abs() <= 1.0 && gap >= 6.5);
                }
            }
            for r in &g.rows[..STAGE_ROWS as usize] {
                assert!(matches!(r.lane, Lane::Grass | Lane::Road { .. }));
            }
        }
    }

    #[test]
    fn new_hazards_arrive_stage_by_stage() {
        fresh_player();
        let kinds = |stage: i32| {
            let (mut river, mut rail) = (false, false);
            for seed in 0..40 {
                rand::srand(seed);
                for k in 0..STAGE_ROWS {
                    match lane_for(stage * STAGE_ROWS + k, 0, 0) {
                        Lane::River { .. } => river = true,
                        Lane::Rail { .. } => rail = true,
                        _ => {}
                    }
                }
            }
            (river, rail)
        };
        assert_eq!(kinds(0), (false, false));
        assert_eq!(kinds(1), (false, false));
        assert_eq!(kinds(2), (true, false));
        assert_eq!(kinds(3), (true, true));
    }

    #[test]
    fn cars_get_faster_and_closer_with_distance() {
        let mut last = road_speed_range(0);
        let mut last_gap = road_gap_range(0);
        for s in 1..10 {
            let (lo, hi) = road_speed_range(s);
            let (glo, ghi) = road_gap_range(s);
            assert!(lo >= last.0 && hi >= last.1);
            assert!(glo <= last_gap.0 && ghi <= last_gap.1);
            last = (lo, hi);
            last_gap = (glo, ghi);
        }
        assert!(road_speed_range(0).1 <= 1.0);
        assert!(road_speed_range(8).1 > 4.0);
    }

    #[test]
    fn busy_stages_never_chain_more_than_a_few_hazard_lanes() {
        fresh_player();
        for stage in 0..8 {
            rand::srand(stage as u64);
            let mut g = Crossy::new();
            g.rows.clear();
            g.row = (stage + 1) * STAGE_ROWS * 3;
            g.ensure_rows();
            let mut run = 0;
            for r in &g.rows {
                if r.lane == Lane::Grass {
                    run = 0;
                } else {
                    run += 1;
                    assert!(run <= 4, "stage {stage}");
                }
            }
        }
    }

    #[test]
    fn score_counts_rows_from_the_start_line() {
        fresh_player();
        crate::profile::set(GAME, "stage", 2);
        let mut g = Crossy::starting_at(2);
        g.state = State::Playing;
        g.gentle = false;
        for r in &mut g.rows {
            r.lane = Lane::Grass;
            r.trees.clear();
        }
        assert_eq!(g.row, 2 * STAGE_ROWS);
        assert!(g.try_hop(Dir::Up));
        assert_eq!(g.score, 1);
    }

    #[test]
    fn reaching_a_checkpoint_saves_and_ends_the_gentle_start() {
        fresh_player();
        let mut g = Crossy::new();
        g.state = State::Playing;
        for r in &mut g.rows {
            r.lane = Lane::Grass;
            r.trees.clear();
        }
        for _ in 0..STAGE_ROWS {
            settle(&mut g);
            g.idle = 0.0;
            assert!(g.try_hop(Dir::Up));
        }
        assert_eq!(saved_stage(), 1);
        assert!(!g.gentle);
        assert_eq!(g.score, STAGE_ROWS as u32);
    }

    #[test]
    fn checkpoint_progress_is_per_player() {
        fresh_player();
        crate::profile::set(GAME, "stage", 3);
        assert_eq!(Crossy::new().checkpoint_offer(), Some(3));
        crate::profile::create("Leo", 1);
        assert_eq!(saved_stage(), 0);
        assert_eq!(Crossy::new().checkpoint_offer(), None);
        assert!(Crossy::new().gentle);
    }

    #[test]
    fn restarting_from_a_checkpoint_begins_on_that_stage() {
        fresh_player();
        crate::profile::set(GAME, "stage", 2);
        let view = View::ipad_landscape();
        let mut g = Crossy::new();
        assert!(!g.gentle);
        assert_eq!(g.checkpoint_offer(), Some(2));
        let btn = g.checkpoint_button(&view);
        g.update(&Input::press_at(btn.center()), &view, 1.0 / 60.0);
        assert_eq!(g.state, State::Playing);
        assert!(g.row >= 2 * STAGE_ROWS);
        assert_eq!(g.start_row, 2 * STAGE_ROWS);
    }

    #[test]
    fn dying_early_eases_the_next_run_and_a_long_run_hardens_it() {
        fresh_player();
        let mut g = Crossy::new();
        g.gentle = false;
        g.state = State::Playing;
        g.score = 3;
        g.die(Death::Car);
        assert_eq!(Crossy::new().ease, 1);
        let mut g = Crossy::new();
        g.gentle = false;
        g.state = State::Playing;
        g.score = 30;
        g.die(Death::Car);
        assert_eq!(Crossy::new().ease, 0);
        let (_, easy) = (0, lane_for(25, 0, 3));
        if let Lane::Road { speed, .. } = easy {
            assert!(speed.abs() < road_speed_range(2).1);
        }
    }

    #[test]
    fn game_over_submits_the_score_and_remembers_the_best() {
        fresh_player();
        let mut g = Crossy::new();
        g.gentle = false;
        g.state = State::Playing;
        g.score = 12;
        g.die(Death::Car);
        assert_eq!(g.rank, Some(0));
        assert_eq!(Crossy::new().best.0, 12);
        assert_eq!(crate::profile::personal_best(GAME), Some(12));
    }

    #[test]
    fn trains_give_a_long_warning() {
        let r = Row {
            lane: Lane::Rail { timer: 1.7 },
            things: vec![],
            trees: vec![],
            len: 1.0,
            hue: 0.0,
        };
        assert!(r.warning());
    }
}
