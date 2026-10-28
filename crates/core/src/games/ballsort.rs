pub mod level;
pub mod solve;

use crate::engine::*;
use crate::feel::{self, Impact};
use crate::fx::*;
use crate::geom::*;
use crate::sfx::{self, Say, Sfx};
use glam::{vec2, vec3, Vec2, Vec3};
use solve::CAP;

pub use level::MAX_LEVEL;

pub const GAME: &str = "ballsort";
pub const GRID_COLS: usize = 6;
pub const BALL_D: f32 = 0.84;
pub const TUBE_R: f32 = 0.5;
pub const BASE: f32 = 0.14;
pub const TUBE_H: f32 = CAP as f32 * BALL_D + 0.3;
pub const HOVER: f32 = TUBE_H + 0.55;
pub const GRAVITY: f32 = 26.0;
pub const SPACING: f32 = 1.55;
pub const ROW_Z: f32 = 3.1;
pub const PITCH: f32 = 0.92;
pub const FOV: f32 = 0.7;
pub const WIN_DELAY: f32 = 1.3;
pub const STAR_START: f32 = 0.3;
pub const STAR_GAP: f32 = 0.25;
pub const HUD_TOP: f32 = 100.0;
pub const HUD_BOTTOM: f32 = 150.0;
pub const HELPER_AFTER_FAILS: u32 = 2;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ball {
    pub id: u8,
    pub color: u8,
    pub revealed: bool,
}

pub type Tubes = Vec<Vec<Ball>>;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum State {
    Select,
    Playing,
    Won,
}

pub fn ball_color(c: u8) -> Color {
    const PALETTE: [(u8, u8, u8); 10] = [
        (235, 55, 60),
        (255, 150, 35),
        (255, 214, 40),
        (150, 215, 50),
        (35, 170, 85),
        (45, 205, 225),
        (55, 105, 240),
        (150, 85, 225),
        (255, 115, 185),
        (150, 95, 55),
    ];
    let p = PALETTE[c as usize % PALETTE.len()];
    Color::from_rgba(p.0, p.1, p.2, 255)
}

#[derive(Clone, Copy, Debug)]
pub struct Cam {
    pub eye: Vec3,
    pub target: Vec3,
}

impl Default for Cam {
    fn default() -> Self {
        Self {
            eye: vec3(0.0, 8.0, 8.0),
            target: Vec3::ZERO,
        }
    }
}

impl Cam {
    pub fn distance(&self) -> f32 {
        (self.eye - self.target).length()
    }

    pub fn project(&self, view: &View, p: Vec3) -> Vec2 {
        let f = view.full();
        let fwd = (self.target - self.eye).normalize();
        let right = fwd.cross(Vec3::Y).normalize();
        let up = right.cross(fwd);
        let d = p - self.eye;
        let depth = d.dot(fwd).max(0.01);
        let k = 1.0 / (FOV / 2.0).tan();
        let n = vec2(
            d.dot(right) * k / (depth * f.w / f.h),
            d.dot(up) * k / depth,
        );
        vec2(f.x + (n.x * 0.5 + 0.5) * f.w, f.y + (0.5 - n.y * 0.5) * f.h)
    }

    fn at(target: Vec3, dist: f32) -> Self {
        Self {
            eye: target + vec3(0.0, PITCH.sin(), PITCH.cos()) * dist,
            target,
        }
    }
}

pub fn layout(n: usize, landscape: bool) -> Vec<Vec2> {
    let rows = if landscape {
        if n <= 7 {
            1
        } else {
            2
        }
    } else {
        n.div_ceil(5).clamp(1, 3)
    };
    let cols = n.div_ceil(rows);
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let (r, c) = (i / cols, i % cols);
        let in_row = (n - r * cols).min(cols);
        let x = (c as f32 - (in_row as f32 - 1.0) / 2.0) * SPACING;
        let z = (r as f32 - (rows as f32 - 1.0) / 2.0) * ROW_Z;
        out.push(vec2(x, z));
    }
    out
}

pub fn fit(view: &View, spots: &[Vec2]) -> Cam {
    let region = Rect::new(
        20.0,
        HUD_TOP,
        view.w - 40.0,
        (view.h - HUD_TOP - HUD_BOTTOM).max(100.0),
    );
    let (mut lo, mut hi) = (vec2(f32::MAX, f32::MAX), vec2(f32::MIN, f32::MIN));
    for s in spots {
        lo = lo.min(*s);
        hi = hi.max(*s);
    }
    let pad = TUBE_R + 0.25;
    let corners: Vec<Vec3> = [lo.x - pad, hi.x + pad]
        .iter()
        .flat_map(|x| {
            [lo.y - pad, hi.y + pad]
                .into_iter()
                .flat_map(move |z| [0.0, HOVER + 0.5].into_iter().map(move |y| vec3(*x, y, z)))
        })
        .collect();
    let mid = vec3((lo.x + hi.x) / 2.0, (HOVER) / 2.0, (lo.y + hi.y) / 2.0);
    let mut target = mid;
    let mut dist = 20.0;
    for _ in 0..4 {
        let (mut a, mut b) = (3.0f32, 160.0f32);
        for _ in 0..28 {
            let m = (a + b) / 2.0;
            let cam = Cam::at(target, m);
            let ok = corners.iter().all(|c| {
                let p = cam.project(view, *c);
                p.x >= region.x
                    && p.x <= region.right()
                    && p.y >= region.y
                    && p.y <= region.bottom()
            });
            if ok {
                b = m;
            } else {
                a = m;
            }
        }
        dist = b;
        let cam = Cam::at(target, dist);
        let (mut pl, mut ph) = (vec2(f32::MAX, f32::MAX), vec2(f32::MIN, f32::MIN));
        for c in &corners {
            let p = cam.project(view, *c);
            pl = pl.min(p);
            ph = ph.max(p);
        }
        let off = region.center() - (pl + ph) / 2.0;
        let f = view.full();
        let per_px = 2.0 * dist * (FOV / 2.0).tan() / f.h;
        let fwd = (cam.target - cam.eye).normalize();
        let right = fwd.cross(Vec3::Y).normalize();
        let up = right.cross(fwd);
        target += right * (-off.x * per_px) + up * (off.y * per_px);
    }
    Cam::at(target, dist)
}

pub fn slot_y(slot: usize) -> f32 {
    BASE + BALL_D * (slot as f32 + 0.5)
}

#[derive(Clone, Debug)]
pub struct Flight {
    pub ball: Ball,
    pub to_tube: usize,
    pub start: Vec3,
    pub end: Vec3,
    pub t: f32,
    pub arc: f32,
    pub fall: f32,
    pub bounce: f32,
    pub lift: f32,
    pub landed: bool,
    pub completes: bool,
}

impl Flight {
    pub fn new(ball: Ball, to_tube: usize, start: Vec3, end: Vec3, completes: bool) -> Self {
        let dist = vec2(end.x - start.x, end.z - start.z).length();
        let drop = (HOVER - end.y).max(0.1);
        let fall = (2.0 * drop / GRAVITY).sqrt();
        Self {
            ball,
            to_tube,
            start,
            end,
            t: 0.0,
            arc: (0.2 + dist * 0.055).clamp(0.2, 0.5),
            fall,
            bounce: 2.0 * 0.28 * GRAVITY * fall / GRAVITY,
            lift: 0.35 + dist * 0.12,
            landed: false,
            completes,
        }
    }

    pub fn total(&self) -> f32 {
        self.arc + self.fall + self.bounce
    }

    pub fn impact_time(&self) -> f32 {
        self.arc + self.fall
    }

    pub fn pose(&self) -> (Vec3, f32) {
        let t = self.t;
        if t < self.arc {
            let s = t / self.arc;
            let e = s * s * (3.0 - 2.0 * s);
            let y = self.start.y + 4.0 * self.lift * s * (1.0 - s);
            let stretch = (4.0 * s * (1.0 - s)).min(1.0) * 0.12;
            return (
                vec3(
                    self.start.x + (self.end.x - self.start.x) * e,
                    y,
                    self.start.z + (self.end.z - self.start.z) * e,
                ),
                -stretch,
            );
        }
        let tau = t - self.arc;
        if tau < self.fall {
            let y = HOVER - 0.5 * GRAVITY * tau * tau;
            return (vec3(self.end.x, y.max(self.end.y), self.end.z), -0.1);
        }
        let tb = tau - self.fall;
        let v0 = 0.28 * GRAVITY * self.fall;
        let y = self.end.y + (v0 * tb - 0.5 * GRAVITY * tb * tb).max(0.0);
        let squash = 0.32 * (-tb * 16.0).exp();
        (vec3(self.end.x, y, self.end.z), squash)
    }
}

pub struct BallPose {
    pub ball: Ball,
    pub pos: Vec3,
    pub squash: f32,
}

static NEXT_SERIAL: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

pub struct BallSort {
    pub serial: u64,
    pub state: State,
    pub level: u32,
    pub variant: u32,
    pub par: usize,
    pub hidden: bool,
    pub tubes: Tubes,
    pub start: Tubes,
    pub history: Vec<Tubes>,
    pub flights: Vec<Flight>,
    pub selected: Option<usize>,
    pub lifts: Vec<f32>,
    pub glow: Vec<f32>,
    pub done: Vec<bool>,
    pub moves: usize,
    pub undos: u32,
    pub fails: u32,
    pub stars: u8,
    pub score: u32,
    pub best: Best,
    pub rank: Option<usize>,
    pub spots: Vec<Vec2>,
    pub cam: Cam,
    pub cam_key: (f32, f32, usize),
    pub hint: Option<(usize, usize)>,
    pub hint_dead_end: bool,
    pub idle: f32,
    pub stuck: bool,
    pub pending_win: bool,
    pub taught: bool,
    pub t: f32,
    pub end_t: f32,
    pub fx: Particles,
    pub shake: Shake,
    pub pops: Popups,
    pub bump: Bump,
    pub completed_now: usize,
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

pub fn stars_for(moves: usize, par: usize, undos: u32, hidden: bool) -> u8 {
    let slack = if hidden { par * 2 + 4 } else { par * 3 / 2 + 2 };
    if undos == 0 && moves <= slack {
        3
    } else if moves <= par * 2 + 6 {
        2
    } else {
        1
    }
}

pub fn score_for(level: u32, stars: u8) -> u32 {
    level * 60 + stars as u32 * 40
}

impl Default for BallSort {
    fn default() -> Self {
        Self::new()
    }
}

impl BallSort {
    pub fn new() -> Self {
        let mut g = Self::blank();
        g.best = Best::load(GAME);
        g.enter(start_level());
        g
    }

    fn blank() -> Self {
        Self {
            serial: 0,
            state: State::Playing,
            level: 1,
            variant: 0,
            par: 0,
            hidden: false,
            tubes: vec![],
            start: vec![],
            history: vec![],
            flights: vec![],
            selected: None,
            lifts: vec![],
            glow: vec![],
            done: vec![],
            moves: 0,
            undos: 0,
            fails: 0,
            stars: 0,
            score: 0,
            best: Best::default(),
            rank: None,
            spots: vec![],
            cam: Cam::default(),
            cam_key: (0.0, 0.0, 0),
            hint: None,
            hint_dead_end: false,
            idle: 0.0,
            stuck: false,
            pending_win: false,
            taught: crate::profile::get(GAME, "taught") == 1,
            t: 0.0,
            end_t: 0.0,
            fx: Particles::default(),
            shake: Shake::default(),
            pops: Popups::default(),
            bump: Bump::default(),
            completed_now: 0,
        }
    }

    pub fn with_level(level: u32) -> Self {
        let mut g = Self::blank();
        g.enter(level);
        g
    }

    fn seen_key(level: u32) -> String {
        format!("seen.{level}")
    }

    pub fn enter(&mut self, level: u32) {
        let level = level.clamp(1, MAX_LEVEL);
        crate::profile::set_level(GAME, level);
        if level != self.level {
            self.fails = 0;
        }
        self.level = level;
        self.variant = crate::profile::get(GAME, &Self::seen_key(level)).max(0) as u32;
        let lv = level::make(level, self.variant);
        self.par = lv.par;
        self.hidden = lv.hidden;
        let mut id = 0u8;
        self.start = lv
            .tubes
            .iter()
            .map(|t| {
                let balls = solve::unpack(*t);
                let n = balls.len();
                balls
                    .iter()
                    .enumerate()
                    .map(|(k, c)| {
                        id += 1;
                        Ball {
                            id: id - 1,
                            color: *c,
                            revealed: !lv.hidden || k + 1 == n,
                        }
                    })
                    .collect()
            })
            .collect();
        self.reset_board();
        self.state = State::Playing;
    }

    fn reset_board(&mut self) {
        self.serial = NEXT_SERIAL.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        self.tubes = self.start.clone();
        if self.fails >= HELPER_AFTER_FAILS {
            self.tubes.push(Vec::new());
        }
        let n = self.tubes.len();
        self.history.clear();
        self.flights.clear();
        self.selected = None;
        self.lifts = vec![0.0; n];
        self.glow = vec![0.0; n];
        self.done = vec![false; n];
        self.moves = 0;
        self.undos = 0;
        self.stars = 0;
        self.hint = None;
        self.hint_dead_end = false;
        self.idle = 0.0;
        self.stuck = false;
        self.pending_win = false;
        self.end_t = 0.0;
        self.completed_now = 0;
    }

    pub fn restart(&mut self) {
        self.fails += 1;
        self.reset_board();
        sfx::play(Sfx::Whoosh);
    }

    pub fn open_select(&mut self) {
        self.state = State::Select;
        self.end_t = 0.0;
    }

    pub fn packed(&self) -> Vec<solve::Tube> {
        self.tubes
            .iter()
            .map(|t| solve::pack(&t.iter().map(|b| b.color).collect::<Vec<_>>()))
            .collect()
    }

    pub fn sorted_tubes(&self) -> usize {
        self.tubes
            .iter()
            .filter(|t| t.len() == CAP && t.iter().all(|b| b.color == t[0].color))
            .count()
    }

    pub fn total_colors(&self) -> usize {
        level::spec(self.level).colors
    }

    pub fn stars_now(&self) -> u8 {
        stars_for(self.moves, self.par, self.undos, self.hidden)
    }

    pub fn ensure_cam(&mut self, view: &View) {
        let key = (view.w, view.h, self.tubes.len());
        if self.cam_key == key {
            return;
        }
        self.cam_key = key;
        self.spots = layout(self.tubes.len(), view.w >= view.h);
        self.cam = fit(view, &self.spots);
    }

    pub fn spot(&self, tube: usize) -> Vec2 {
        self.spots.get(tube).copied().unwrap_or(Vec2::ZERO)
    }

    pub fn slot_pos(&self, tube: usize, slot: usize) -> Vec3 {
        let s = self.spot(tube);
        vec3(s.x, slot_y(slot), s.y)
    }

    pub fn hover_pos(&self, tube: usize) -> Vec3 {
        let s = self.spot(tube);
        vec3(s.x, HOVER, s.y)
    }

    pub fn balls(&self) -> Vec<BallPose> {
        let mut out = Vec::new();
        for (ti, tube) in self.tubes.iter().enumerate() {
            let top = tube.len().saturating_sub(1);
            for (si, b) in tube.iter().enumerate() {
                if self.flights.iter().any(|f| f.ball.id == b.id) {
                    continue;
                }
                let mut pos = self.slot_pos(ti, si);
                if si == top {
                    let k = ease_out_cubic(self.lifts[ti]);
                    pos.y += (HOVER - pos.y) * k;
                }
                if self.state == State::Won {
                    let phase = ti as f32 * 0.11 + si as f32 * 0.05;
                    let a = ((self.end_t - phase) * 9.0).clamp(0.0, std::f32::consts::PI);
                    pos.y += a.sin() * 0.4;
                }
                out.push(BallPose {
                    ball: *b,
                    pos,
                    squash: 0.0,
                });
            }
        }
        for f in &self.flights {
            let (pos, squash) = f.pose();
            out.push(BallPose {
                ball: f.ball,
                pos,
                squash,
            });
        }
        out
    }

    pub fn tube_at(&self, view: &View, p: Vec2) -> Option<usize> {
        let mut best: Option<(usize, f32, f32)> = None;
        for i in 0..self.tubes.len() {
            let s = self.spot(i);
            let bottom = self.cam.project(view, vec3(s.x, 0.0, s.y));
            let top = self.cam.project(view, vec3(s.x, HOVER + 0.3, s.y));
            let edge = self.cam.project(view, vec3(s.x + TUBE_R, 0.0, s.y));
            let half = ((edge.x - bottom.x).abs() * 1.3).max(30.0);
            let (y0, y1) = (top.y.min(bottom.y) - 8.0, top.y.max(bottom.y) + 14.0);
            if p.y < y0 || p.y > y1 {
                continue;
            }
            let t = ((p.y - top.y) / (bottom.y - top.y).abs().max(1.0)).clamp(0.0, 1.0);
            let cx = top.x + (bottom.x - top.x) * t;
            let dx = (p.x - cx).abs();
            if dx > half {
                continue;
            }
            let better = match best {
                None => true,
                Some((_, bd, bz)) => dx < bd - 0.5 * half || (dx <= bd + 0.5 * half && s.y > bz),
            };
            if better {
                best = Some((i, dx, s.y));
            }
        }
        best.map(|b| b.0)
    }

    pub fn can_pour(&self, from: usize, to: usize) -> bool {
        from != to
            && !self.tubes[from].is_empty()
            && self.tubes[to].len() < CAP
            && self.tubes[to]
                .last()
                .is_none_or(|b| b.color == self.tubes[from].last().unwrap().color)
    }

    fn complete(&self, t: usize) -> bool {
        let tube = &self.tubes[t];
        tube.len() == CAP && tube.iter().all(|b| b.color == tube[0].color)
    }

    pub fn solved(&self) -> bool {
        self.tubes
            .iter()
            .all(|t| t.is_empty() || (t.len() == CAP && t.iter().all(|b| b.color == t[0].color)))
    }

    pub fn pour(&mut self, from: usize, to: usize) -> bool {
        if !self.can_pour(from, to) {
            return false;
        }
        self.history.push(self.tubes.clone());
        let start = self.hover_pos(from);
        let mut ball = self.tubes[from].pop().unwrap();
        ball.revealed = true;
        if let Some(b) = self.tubes[from].last_mut() {
            b.revealed = true;
        }
        let slot = self.tubes[to].len();
        self.tubes[to].push(ball);
        self.moves += 1;
        self.lifts[from] = 0.0;
        self.selected = None;
        let completes = self.complete(to);
        self.flights.push(Flight::new(
            ball,
            to,
            start,
            self.slot_pos(to, slot),
            completes,
        ));
        sfx::play(Sfx::Swish);
        self.hint = None;
        self.hint_dead_end = false;
        self.idle = 0.0;
        self.stuck = false;
        if self.solved() {
            self.pending_win = true;
        } else if !solve::any_move(&self.packed()) {
            self.stuck = true;
            self.fails += 1;
        }
        true
    }

    pub fn undo(&mut self) -> bool {
        let Some(prev) = self.history.pop() else {
            return false;
        };
        self.tubes = prev;
        self.flights.clear();
        self.selected = None;
        self.lifts.iter_mut().for_each(|l| *l = 0.0);
        self.moves = self.moves.saturating_sub(1);
        self.undos += 1;
        self.pending_win = false;
        self.stuck = false;
        self.hint = None;
        self.hint_dead_end = false;
        self.idle = 0.0;
        sfx::play(Sfx::Whoosh);
        true
    }

    pub fn compute_hint(&mut self) {
        let state = self.packed();
        match solve::solve(&state, 60_000) {
            Some(path) if !path.is_empty() => {
                self.hint = Some((path[0].0 as usize, path[0].1 as usize));
                self.hint_dead_end = false;
            }
            _ => {
                self.hint = None;
                self.hint_dead_end = true;
            }
        }
    }

    pub fn glow_of(&self, tube: usize) -> (Color, f32) {
        let pulse = 0.55 + 0.45 * (self.t * 5.0).sin();
        if self.selected == Some(tube) {
            return (Color::from_rgba(255, 235, 120, 255), 0.85);
        }
        if let Some((a, b)) = self.hint {
            if tube == a {
                return (Color::from_rgba(120, 255, 150, 255), pulse);
            }
            if tube == b {
                return (Color::from_rgba(120, 220, 255, 255), pulse);
            }
        }
        if self.done[tube] {
            return (GOLD, 0.45 + self.glow[tube] * 0.5);
        }
        (WHITE, 0.0)
    }

    fn select(&mut self, i: usize) {
        self.selected = Some(i);
        sfx::play(Sfx::Click);
    }

    fn tap_tube(&mut self, i: usize) {
        self.hint = None;
        self.idle = 0.0;
        match self.selected {
            None => {
                if self.tubes[i].is_empty() || self.complete(i) {
                    sfx::play(Sfx::Tick);
                } else {
                    self.select(i);
                }
            }
            Some(s) if s == i => {
                self.selected = None;
                sfx::play(Sfx::Tick);
            }
            Some(s) => {
                if !self.pour(s, i) {
                    sfx::play(Sfx::Wrong);
                    self.shake.add(0.15);
                    let at = self.cam.project(&self.last_view(), self.hover_pos(i));
                    self.pops
                        .add(at, "Not here", Color::from_rgba(255, 140, 140, 255), 34.0);
                    if self.tubes[i].is_empty() || self.complete(i) {
                        self.selected = None;
                    } else {
                        self.select(i);
                    }
                }
            }
        }
    }

    fn last_view(&self) -> View {
        let mut v = View::ipad_landscape();
        v.w = self.cam_key.0;
        v.h = self.cam_key.1;
        v.scale = 1.0;
        v
    }

    fn land(&mut self, f: &Flight, view: &View) {
        let at = self
            .cam
            .project(view, vec3(f.end.x, f.end.y - BALL_D * 0.4, f.end.z));
        let col = ball_color(f.ball.color);
        self.fx.emit(
            at,
            &[col, WHITE],
            Burst {
                count: 7,
                speed: (60.0, 160.0),
                size: (3.0, 6.0),
                life: (0.25, 0.5),
                gravity: 500.0,
                spread: 2.4,
                angle: -std::f32::consts::FRAC_PI_2,
                ..Default::default()
            },
        );
        sfx::play(Sfx::Bounce);
        self.shake.add(0.05);
        if f.completes && !self.done[f.to_tube] {
            self.done[f.to_tube] = true;
            self.glow[f.to_tube] = 1.0;
            self.completed_now += 1;
            let top = self.cam.project(view, vec3(f.end.x, TUBE_H * 0.7, f.end.z));
            self.fx.sparkle(top, GOLD);
            self.fx.emit(
                top,
                &[GOLD, WHITE, col],
                Burst {
                    count: 22,
                    shape: Shape::Star,
                    speed: (140.0, 330.0),
                    size: (6.0, 11.0),
                    ..Default::default()
                },
            );
            self.pops
                .add(top + vec2(0.0, -40.0), "Sorted!", WHITE, 40.0);
            sfx::play_step(Sfx::Coin, self.completed_now.saturating_sub(1));
            if !self.pending_win {
                sfx::say(Say::Correct);
                feel::impact_tinted(Impact::Medium, GOLD);
                self.shake.add(0.2);
            }
            self.bump.kick();
        }
    }

    fn finish_level(&mut self, view: &View) {
        self.state = State::Won;
        self.end_t = 0.0;
        self.stars = stars_for(self.moves, self.par, self.undos, self.hidden);
        crate::profile::award_stars(GAME, self.level, self.stars);
        let skip = if self.stars == 3 { 2 } else { 1 };
        crate::profile::unlock(GAME, (self.level + skip).min(MAX_LEVEL));
        crate::profile::set_level(GAME, (self.level + 1).min(MAX_LEVEL));
        crate::profile::set(GAME, &Self::seen_key(self.level), self.variant as i64 + 1);
        crate::profile::set(GAME, "taught", 1);
        self.taught = true;
        self.fails = 0;
        self.score += score_for(self.level, self.stars);
        self.rank = self.best.submit(GAME, self.score);
        sfx::play(Sfx::Win);
        sfx::play(Sfx::LevelUp);
        sfx::say(if self.stars == 3 {
            Say::Congrats
        } else {
            Say::Win
        });
        feel::impact_tinted(Impact::Large, Color::from_rgba(255, 220, 60, 255));
        self.fx.confetti(view);
        self.shake.add(0.5);
        self.bump.kick();
    }

    fn advance_flights(&mut self, view: &View, dt: f32) {
        let mut landed = Vec::new();
        for f in &mut self.flights {
            let before = f.t;
            f.t += dt;
            if !f.landed && f.t >= f.impact_time() {
                f.landed = true;
                landed.push(f.clone());
            }
            if before < f.arc {
                let (pos, _) = f.pose();
                let at = self.cam.project(view, pos);
                self.fx.emit(
                    at,
                    &[ball_color(f.ball.color).with_alpha(0.8)],
                    Burst {
                        count: 1,
                        speed: (5.0, 25.0),
                        size: (3.0, 6.0),
                        life: (0.25, 0.45),
                        gravity: 0.0,
                        ..Default::default()
                    },
                );
            }
        }
        self.flights.retain(|f| f.t < f.total());
        for f in landed {
            self.land(&f, view);
        }
    }

    fn animate(&mut self, dt: f32) {
        for (i, l) in self.lifts.iter_mut().enumerate() {
            let want = if self.selected == Some(i) { 1.0 } else { 0.0 };
            *l = approach(*l, want, 18.0, dt);
        }
        for g in &mut self.glow {
            *g = approach(*g, 0.0, 2.5, dt);
        }
    }

    fn press(&mut self, input: &Input, view: &View) {
        if !input.pressed {
            return;
        }
        let b = buttons(view);
        self.idle = 0.0;
        self.hint = None;
        if b.undo.contains(input.pos) {
            if !self.undo() {
                sfx::play(Sfx::Tick);
            }
        } else if b.restart.contains(input.pos) {
            self.restart();
        } else if b.hint.contains(input.pos) {
            sfx::play(Sfx::Magic);
            self.compute_hint();
        } else if b.levels.contains(input.pos) {
            sfx::play(Sfx::Tap);
            self.open_select();
        } else if let Some(i) = self.tube_at(view, input.pos) {
            self.tap_tube(i);
        } else if self.selected.is_some() {
            self.selected = None;
        }
    }

    pub fn can_skip(&self) -> bool {
        self.stars == 3 && self.level + 2 <= MAX_LEVEL
    }

    fn hint_delay(&self) -> f32 {
        if self.level <= 3 && !self.taught {
            3.0
        } else {
            14.0
        }
    }
}

impl Game for BallSort {
    fn update(&mut self, input: &Input, view: &View, dt: f32) {
        self.ensure_cam(view);
        self.t += dt;
        self.fx.update(dt);
        self.shake.update(dt);
        self.pops.update(dt);
        self.bump.update(dt);
        self.animate(dt);
        self.advance_flights(view, dt);
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
            State::Playing => {
                if self.pending_win && self.flights.is_empty() {
                    self.finish_level(view);
                    return;
                }
                if !self.pending_win {
                    self.press(input, view);
                    if input.active() {
                        self.idle = 0.0;
                    } else {
                        self.idle += dt;
                        if self.idle > self.hint_delay() && self.hint.is_none() && !self.stuck {
                            self.compute_hint();
                        }
                    }
                }
            }
            State::Won => {
                self.end_t += dt;
                for k in 0..self.stars as usize {
                    let at = WIN_DELAY + STAR_START + STAR_GAP * k as f32;
                    if self.end_t - dt < at && self.end_t >= at {
                        sfx::play_step(Sfx::Coin, k * 2);
                    }
                }
                if self.end_t > WIN_DELAY + 0.7 && input.pressed {
                    let w = win_layout(view, self.can_skip());
                    if w.levels.contains(input.pos) {
                        sfx::play(Sfx::Tap);
                        self.open_select();
                    } else if self.can_skip() && w.skip.contains(input.pos) {
                        sfx::play(Sfx::Tap);
                        self.enter(self.level + 2);
                    } else {
                        self.enter((self.level + 1).min(MAX_LEVEL));
                    }
                }
            }
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Buttons {
    pub undo: Rect,
    pub restart: Rect,
    pub hint: Rect,
    pub levels: Rect,
}

pub fn buttons(view: &View) -> Buttons {
    let w = 190.0;
    let h = 62.0;
    let gap = 16.0;
    let total = w * 4.0 + gap * 3.0;
    let x = (view.w - total) / 2.0;
    let y = view.h - h - 18.0;
    let at = |i: f32| Rect::new(x + i * (w + gap), y, w, h);
    Buttons {
        undo: at(0.0),
        restart: at(1.0),
        hint: at(2.0),
        levels: at(3.0),
    }
}

pub struct WinLayout {
    pub skip: Rect,
    pub levels: Rect,
}

pub fn win_layout(view: &View, skip: bool) -> WinLayout {
    let y = (view.h / 2.0 + 272.0).min(view.h - 66.0);
    let (w, gap) = (250.0, 16.0);
    let x = (view.w - w * 2.0 - gap) / 2.0;
    WinLayout {
        skip: Rect::new(x, y, w, 62.0),
        levels: if skip {
            Rect::new(x + w + gap, y, w, 62.0)
        } else {
            Rect::new((view.w - w) / 2.0, y, w, 62.0)
        },
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

pub const SPRITES: [&str; 2] = ["bg", "table"];

#[cfg(test)]
mod tests {
    use super::*;

    const DT: f32 = 1.0 / 60.0;

    fn setup() {
        crate::save::reset_for_test();
        crate::profile::create("Mia", 0);
    }

    fn views() -> [View; 2] {
        [View::ipad_landscape(), View::ipad_portrait()]
    }

    fn tap_tube(g: &mut BallSort, v: &View, tube: usize) {
        let s = g.spot(tube);
        let at = g.cam.project(v, vec3(s.x, TUBE_H * 0.5, s.y));
        g.update(&Input::press_at(at), v, DT);
        g.update(&Input::default(), v, DT);
    }

    fn settle(g: &mut BallSort, v: &View) {
        for _ in 0..200 {
            if g.flights.is_empty() {
                break;
            }
            g.update(&Input::default(), v, DT);
        }
    }

    fn play_path(g: &mut BallSort, v: &View, mistakes: usize) {
        let mut wasted = 0;
        while g.state == State::Playing {
            let path = solve::solve(&g.packed(), 400_000).expect("solvable");
            let Some(&(a, b)) = path.first() else { break };
            if wasted < mistakes {
                wasted += 1;
                let from = (0..g.tubes.len())
                    .find(|i| !g.tubes[*i].is_empty())
                    .unwrap();
                if let Some(to) = (0..g.tubes.len()).find(|t| g.can_pour(from, *t)) {
                    tap_tube(g, v, from);
                    tap_tube(g, v, to);
                    settle(g, v);
                    continue;
                }
            }
            tap_tube(g, v, a as usize);
            tap_tube(g, v, b as usize);
            settle(g, v);
            for _ in 0..4 {
                g.update(&Input::default(), v, DT);
            }
        }
    }

    #[test]
    fn tapping_two_tubes_pours_one_ball_and_it_lands_in_the_slot() {
        setup();
        for v in views() {
            let mut g = BallSort::with_level(1);
            g.ensure_cam(&v);
            let (a, b) = {
                let path = solve::solve(&g.packed(), 100_000).unwrap();
                (path[0].0 as usize, path[0].1 as usize)
            };
            let (na, nb) = (g.tubes[a].len(), g.tubes[b].len());
            tap_tube(&mut g, &v, a);
            assert_eq!(g.selected, Some(a));
            tap_tube(&mut g, &v, b);
            assert_eq!((g.tubes[a].len(), g.tubes[b].len()), (na - 1, nb + 1));
            assert_eq!(g.moves, 1);
            assert_eq!(g.flights.len(), 1);
            settle(&mut g, &v);
            assert!(g.flights.is_empty());
        }
    }

    #[test]
    fn a_ball_only_pours_onto_the_same_colour_or_an_empty_tube() {
        setup();
        let v = View::ipad_landscape();
        let mut g = BallSort::with_level(1);
        g.ensure_cam(&v);
        let (a, b) = (0..g.tubes.len())
            .flat_map(|a| (0..g.tubes.len()).map(move |b| (a, b)))
            .find(|(a, b)| {
                a != b
                    && !g.tubes[*a].is_empty()
                    && !g.tubes[*b].is_empty()
                    && g.tubes[*a].last().unwrap().color != g.tubes[*b].last().unwrap().color
            })
            .unwrap();
        let before = g.tubes.clone();
        tap_tube(&mut g, &v, a);
        tap_tube(&mut g, &v, b);
        assert_eq!(g.tubes, before);
        assert_eq!(g.moves, 0);
    }

    #[test]
    fn undo_restores_the_board_and_costs_the_third_star() {
        setup();
        let v = View::ipad_landscape();
        let mut g = BallSort::with_level(2);
        g.ensure_cam(&v);
        let before = g.tubes.clone();
        let path = solve::solve(&g.packed(), 100_000).unwrap();
        tap_tube(&mut g, &v, path[0].0 as usize);
        tap_tube(&mut g, &v, path[0].1 as usize);
        assert_ne!(g.tubes, before);
        assert!(g.undo());
        assert_eq!(g.tubes, before);
        assert_eq!((g.moves, g.undos), (0, 1));
        assert!(g.flights.is_empty());
        assert!(!g.undo());
        play_path(&mut g, &v, 0);
        assert_eq!(g.state, State::Won);
        assert!(g.stars < 3, "undo used, got {}", g.stars);
    }

    #[test]
    fn a_strong_player_clears_every_level_with_three_stars() {
        setup();
        for v in views() {
            for l in [1, 5, 10, 17, 22] {
                let mut g = BallSort::with_level(l);
                play_path(&mut g, &v, 0);
                assert_eq!(g.state, State::Won, "level {l}");
                assert_eq!(g.stars, 3, "level {l}");
                assert_eq!(crate::profile::stars(GAME, l), 3);
            }
        }
    }

    #[test]
    fn a_strong_player_is_offered_a_skip_ahead_after_three_stars() {
        setup();
        let v = View::ipad_landscape();
        let mut g = BallSort::with_level(1);
        play_path(&mut g, &v, 0);
        assert_eq!(g.stars, 3);
        assert_eq!(reached(), 3);
        let mut h = BallSort::with_level(2);
        play_path(&mut h, &v, 0);
        assert_eq!(reached(), 4);
    }

    #[test]
    fn a_weak_player_who_pours_wastefully_still_finishes_and_progresses() {
        setup();
        let v = View::ipad_landscape();
        let mut g = BallSort::with_level(3);
        play_path(&mut g, &v, 30);
        assert_eq!(g.state, State::Won);
        assert!(g.stars >= 1 && g.stars < 3, "{}", g.stars);
        assert_eq!(reached(), 4);
        assert_eq!(crate::profile::level(GAME), 4);
    }

    #[test]
    fn finishing_a_level_saves_stars_unlocks_and_posts_the_score() {
        setup();
        let v = View::ipad_landscape();
        let mut g = BallSort::new();
        assert_eq!(g.level, 1);
        play_path(&mut g, &v, 0);
        assert!(crate::profile::stars(GAME, 1) >= 1);
        assert!(g.rank.is_some());
        assert!(g.score > 0);
        assert!(crate::profile::personal_best(GAME).unwrap_or(0) > 0);
        let fresh = BallSort::new();
        assert_eq!(fresh.level, 2, "resumes where the child left off");
    }

    #[test]
    fn the_same_level_replayed_is_a_new_puzzle() {
        setup();
        let v = View::ipad_landscape();
        let mut g = BallSort::with_level(2);
        let first = g.start.clone();
        play_path(&mut g, &v, 0);
        g.enter(2);
        assert_ne!(first, g.start);
    }

    #[test]
    fn hidden_balls_show_only_the_top_ball_and_reveal_as_they_are_uncovered() {
        setup();
        let v = View::ipad_landscape();
        let mut g = BallSort::with_level(level::HIDDEN_FROM);
        assert!(g.hidden);
        for t in &g.tubes {
            for (i, b) in t.iter().enumerate() {
                assert_eq!(b.revealed, i + 1 == t.len());
            }
        }
        g.ensure_cam(&v);
        let path = solve::solve(&g.packed(), 400_000).unwrap();
        let (a, b) = (path[0].0 as usize, path[0].1 as usize);
        tap_tube(&mut g, &v, a);
        tap_tube(&mut g, &v, b);
        if let Some(top) = g.tubes[a].last() {
            assert!(top.revealed);
        }
    }

    #[test]
    fn early_levels_have_no_hidden_balls_and_five_tubes() {
        setup();
        let g = BallSort::with_level(1);
        assert_eq!(g.tubes.len(), 5);
        assert!(g.tubes.iter().flatten().all(|b| b.revealed));
    }

    #[test]
    fn restarting_after_a_stall_gives_a_helper_tube_after_repeated_failures() {
        setup();
        let mut g = BallSort::with_level(6);
        let n = g.tubes.len();
        g.restart();
        assert_eq!(g.tubes.len(), n);
        g.restart();
        assert_eq!(g.tubes.len(), n + 1);
        assert_eq!(g.moves, 0);
        g.enter(7);
        assert_eq!(g.tubes.len(), level::spec(7).tubes());
    }

    #[test]
    fn a_dead_end_is_noticed_and_the_hint_says_so() {
        setup();
        let v = View::ipad_landscape();
        let mut g = BallSort::with_level(1);
        g.ensure_cam(&v);
        g.tubes = vec![
            vec![
                Ball {
                    id: 0,
                    color: 0,
                    revealed: true,
                },
                Ball {
                    id: 1,
                    color: 1,
                    revealed: true,
                },
                Ball {
                    id: 2,
                    color: 0,
                    revealed: true,
                },
                Ball {
                    id: 3,
                    color: 1,
                    revealed: true,
                },
            ],
            vec![
                Ball {
                    id: 4,
                    color: 1,
                    revealed: true,
                },
                Ball {
                    id: 5,
                    color: 0,
                    revealed: true,
                },
                Ball {
                    id: 6,
                    color: 1,
                    revealed: true,
                },
                Ball {
                    id: 7,
                    color: 0,
                    revealed: true,
                },
            ],
        ];
        g.lifts = vec![0.0; 2];
        g.glow = vec![0.0; 2];
        g.done = vec![false; 2];
        g.compute_hint();
        assert!(g.hint.is_none() && g.hint_dead_end);
    }

    #[test]
    fn the_hint_points_at_a_move_that_still_solves_the_puzzle() {
        setup();
        let mut g = BallSort::with_level(4);
        g.compute_hint();
        let (a, b) = g.hint.unwrap();
        assert!(g.can_pour(a, b));
        let mut s = g.packed();
        solve::pour(&mut s, a, b);
        assert!(solve::solve(&s, 400_000).is_some());
    }

    #[test]
    fn an_idle_child_on_the_first_level_is_shown_a_hint_soon_and_a_veteran_later() {
        setup();
        let v = View::ipad_landscape();
        let mut g = BallSort::with_level(1);
        for _ in 0..(3.2 / DT) as usize {
            g.update(&Input::default(), &v, DT);
        }
        assert!(g.hint.is_some());
        crate::profile::set(GAME, "taught", 1);
        let mut h = BallSort::with_level(1);
        for _ in 0..(5.0 / DT) as usize {
            h.update(&Input::default(), &v, DT);
        }
        assert!(h.hint.is_none());
    }

    #[test]
    fn flights_land_exactly_in_their_slot_after_an_arc_and_a_fall() {
        let f = Flight::new(
            Ball {
                id: 0,
                color: 0,
                revealed: true,
            },
            1,
            vec3(0.0, HOVER, 0.0),
            vec3(3.0, slot_y(2), 0.0),
            false,
        );
        let mut g = f.clone();
        g.t = 0.0;
        assert!((g.pose().0 - f.start).length() < 1e-4);
        g.t = f.arc;
        let (p, _) = g.pose();
        assert!((p.x - 3.0).abs() < 1e-3 && (p.y - HOVER).abs() < 1e-2);
        g.t = f.impact_time();
        assert!((g.pose().0.y - f.end.y).abs() < 1e-2);
        g.t = f.total();
        assert!((g.pose().0.y - f.end.y).abs() < 1e-2);
        let mut last = HOVER + 1.0;
        for k in 0..20 {
            g.t = f.arc + f.fall * k as f32 / 20.0;
            let y = g.pose().0.y;
            assert!(y <= last + 1e-4, "gravity only pulls down");
            last = y;
        }
    }

    #[test]
    fn the_camera_keeps_every_tube_inside_the_playfield_in_both_orientations() {
        for v in views() {
            for n in 5..=12 {
                let spots = layout(n, v.w >= v.h);
                let cam = fit(&v, &spots);
                for s in &spots {
                    for y in [0.0, HOVER] {
                        for dx in [-TUBE_R, TUBE_R] {
                            let p = cam.project(&v, vec3(s.x + dx, y, s.y));
                            assert!(p.x > 0.0 && p.x < v.w, "n {n} x {p:?}");
                            assert!(
                                p.y > HUD_TOP - 5.0 && p.y < v.h - HUD_BOTTOM + 5.0,
                                "n {n} y {p:?}"
                            );
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn tubes_are_big_enough_to_tap_with_a_finger() {
        for v in views() {
            let mut g = BallSort::with_level(MAX_LEVEL);
            g.ensure_cam(&v);
            let n = g.tubes.len();
            for i in 0..n {
                let s = g.spot(i);
                let a = g.cam.project(&v, vec3(s.x - TUBE_R, 0.5, s.y));
                let b = g.cam.project(&v, vec3(s.x + TUBE_R, 0.5, s.y));
                let mm = 9.0 * FALLBACK_MM / 6.0;
                assert!(
                    (b.x - a.x).abs() >= mm * 4.0,
                    "tube {i} width {}",
                    (b.x - a.x).abs()
                );
                let mid = g.cam.project(&v, vec3(s.x, TUBE_H * 0.5, s.y));
                assert_eq!(g.tube_at(&v, mid), Some(i), "tube {i}");
            }
        }
    }

    #[test]
    fn a_dead_end_marks_the_board_as_stuck() {
        setup();
        let v = View::ipad_landscape();
        let mut g = BallSort::with_level(1);
        g.ensure_cam(&v);
        let ball = |id, color| Ball {
            id,
            color,
            revealed: true,
        };
        g.tubes = vec![
            vec![ball(0, 0), ball(1, 1), ball(2, 0), ball(3, 1)],
            vec![ball(4, 1), ball(5, 0), ball(6, 1)],
        ];
        g.lifts = vec![0.0; 2];
        g.glow = vec![0.0; 2];
        g.done = vec![false; 2];
        g.history.clear();
        assert!(g.pour(0, 1));
        assert!(!g.stuck || !solve::any_move(&g.packed()));
        assert!(g.undo());
    }

    #[test]
    fn level_stars_for_the_level_picker_come_from_the_player_profile() {
        setup();
        crate::profile::award_stars(GAME, 3, 2);
        crate::profile::unlock(GAME, 4);
        assert_eq!(crate::profile::stars(GAME, 3), 2);
        assert_eq!(reached(), 4);
        let l = select_layout(&View::ipad_portrait());
        assert_eq!(l.cells.len(), MAX_LEVEL as usize);
    }

    #[test]
    fn star_rules_reward_clean_play_and_never_withhold_a_star() {
        assert_eq!(stars_for(10, 10, 0, false), 3);
        assert_eq!(stars_for(10, 10, 1, false), 2);
        assert_eq!(stars_for(40, 10, 0, false), 1);
        assert!(stars_for(500, 10, 5, true) >= 1);
        assert!(stars_for(20, 10, 0, true) == 3);
    }
}
