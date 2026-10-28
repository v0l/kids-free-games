use crate::engine::*;
use crate::feel::{self, Impact};
use crate::fx::approach;
use crate::fx::*;
use crate::geom::*;
use crate::sfx::{self, Sfx};
use glam::{vec2, vec3, Vec2, Vec3};

pub use super::helix::{levels_button, select_layout, SelectLayout};

pub const GAME: &str = "runner";
pub const MAX_LEVEL: u32 = 30;
pub const LANE_W: f32 = 1.6;
pub const HEARTS: u32 = 3;
pub const RUNWAY: f32 = 36.0;
pub const FINISH_RUN: f32 = 14.0;
pub const AHEAD: f32 = 120.0;
pub const GRAVITY: f32 = 26.0;
pub const JUMP_V: f32 = 9.6;
pub const SLIDE_TIME: f32 = 0.75;
pub const HURDLE_H: f32 = 0.6;
pub const TRAIN_LEN: f32 = 9.0;
pub const ROW_LEN: f32 = 0.8;
pub const HIT_REACH: f32 = 0.95;
pub const COIN_REACH: f32 = 0.9;
pub const INVULN: f32 = 1.8;
pub const WIN_DELAY: f32 = 1.2;
pub const STAR_START: f32 = 0.3;
pub const STAR_GAP: f32 = 0.25;
pub const COIN_SCORE: u32 = 10;

static NEXT_SERIAL: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Barrier,
    Hurdle,
    Overhead,
    Train,
}

impl Kind {
    pub fn len(self) -> f32 {
        if self == Kind::Train {
            TRAIN_LEN
        } else {
            ROW_LEN
        }
    }

    pub fn needs_action(self) -> bool {
        matches!(self, Kind::Hurdle | Kind::Overhead)
    }
}

#[derive(Clone, Debug)]
pub struct Row {
    pub at: f32,
    pub lanes: [Option<Kind>; 3],
    pub hit: bool,
    pub hit_t: f32,
    pub hit_lanes: [bool; 3],
}

impl Row {
    pub fn len(&self) -> f32 {
        self.lanes
            .iter()
            .flatten()
            .map(|k| k.len())
            .fold(0.0, f32::max)
    }

    pub fn blocked(&self) -> usize {
        self.lanes.iter().flatten().count()
    }

    pub fn open(&self) -> [bool; 3] {
        [
            self.lanes[0].is_none(),
            self.lanes[1].is_none(),
            self.lanes[2].is_none(),
        ]
    }

    pub fn kind(&self) -> Kind {
        self.lanes
            .iter()
            .flatten()
            .next()
            .copied()
            .unwrap_or(Kind::Barrier)
    }

    pub fn needs_action(&self) -> bool {
        self.blocked() == 3 && self.kind().needs_action()
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Coin {
    pub at: f32,
    pub lane: usize,
    pub h: f32,
    pub taken: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum State {
    Select,
    Ready,
    Playing,
    Dead,
    Won,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Coach {
    Dodge,
    Jump,
    Slide,
}

pub struct Runner {
    pub serial: u64,
    pub rows: Vec<Row>,
    pub coins_list: Vec<Coin>,
    pub length: f32,
    pub dist: f32,
    pub speed: f32,
    pub lane: i32,
    pub x: f32,
    pub lean: f32,
    pub y: f32,
    pub vy: f32,
    pub slide_t: f32,
    pub squash: f32,
    pub hearts: u32,
    pub hits: u32,
    pub invuln: f32,
    pub stumble_t: f32,
    pub coins: u32,
    pub combo: u32,
    pub combo_t: f32,
    pub score: u32,
    pub best: Best,
    pub level: u32,
    pub fails: u32,
    pub state: State,
    pub stars: u8,
    pub rank: Option<usize>,
    pub t: f32,
    pub end_t: f32,
    pub run_t: f32,
    pub kick: f32,
    pub taught_dodge: bool,
    pub taught_jump: bool,
    pub taught_slide: bool,
    pub fx: Particles,
    pub shake: Shake,
    pub pops: Popups,
    pub bump: Bump,
    pub flash_hit: f32,
    pub intro: f32,
    pub finale: f32,
    queued_jump: f32,
    slide_on_land: bool,
}

pub fn kinds_for(level: u32) -> &'static [Kind] {
    use Kind::*;
    match level {
        0..=2 => &[Barrier],
        3 => &[Hurdle],
        4 => &[Barrier, Hurdle],
        5 => &[Overhead],
        6 => &[Barrier, Hurdle, Overhead],
        7 => &[Train],
        8 => &[Barrier, Hurdle, Overhead, Train],
        _ => &[Barrier, Hurdle, Overhead, Train],
    }
}

pub fn intro_level(kind: Kind) -> u32 {
    match kind {
        Kind::Barrier => 1,
        Kind::Hurdle => 3,
        Kind::Overhead => 5,
        Kind::Train => 7,
    }
}

pub fn length_of(level: u32) -> f32 {
    260.0 + 40.0 * level.clamp(1, MAX_LEVEL) as f32
}

pub fn speed_of(level: u32, fails: u32) -> f32 {
    let base = (8.0 + level.clamp(1, MAX_LEVEL) as f32 * 0.3).min(15.5);
    base * (1.0 - 0.06 * fails.min(3) as f32)
}

pub fn gap_secs(level: u32, fails: u32) -> f32 {
    (2.1 - 0.045 * level as f32).max(0.95) + 0.3 * fails.min(3) as f32
}

pub fn double_rate(level: u32, fails: u32) -> f32 {
    if level < 3 {
        return 0.0;
    }
    ((level - 2) as f32 * 0.06 - fails as f32 * 0.15).clamp(0.0, 0.6)
}

pub fn forced_rate(level: u32, fails: u32) -> f32 {
    if level < 6 {
        return 0.0;
    }
    ((level - 5) as f32 * 0.02 - fails as f32 * 0.05).clamp(0.0, 0.25)
}

pub fn stars_for(hits: u32) -> u8 {
    match hits {
        0 => 3,
        1 => 2,
        _ => 1,
    }
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

pub fn lane_x(lane: i32) -> f32 {
    lane as f32 * LANE_W
}

pub fn at_world(dist: f32, x: f32, h: f32) -> Vec3 {
    vec3(x, h, -dist)
}

fn reachable(prev: [bool; 3], now: [bool; 3]) -> bool {
    (0..3).any(|a| prev[a] && (0..3).any(|b| now[b] && (a as i32 - b as i32).abs() <= 1))
}

fn pick_lanes(count: usize, prev: [bool; 3], force_all: bool) -> [bool; 3] {
    if force_all {
        return [true; 3];
    }
    for _ in 0..40 {
        let mut blocked = [false; 3];
        let mut left = count;
        while left > 0 {
            let k = crate::rng::gen_range(0, 3);
            if !blocked[k] {
                blocked[k] = true;
                left -= 1;
            }
        }
        let open = [!blocked[0], !blocked[1], !blocked[2]];
        if reachable(prev, open) {
            return blocked;
        }
    }
    let k = (0..3).find(|k| prev[*k]).unwrap_or(1);
    let mut blocked = [true; 3];
    blocked[k] = false;
    blocked[(k + 1) % 3] = false;
    blocked
}

fn pick_kind(level: u32) -> Kind {
    let kinds = kinds_for(level);
    kinds[crate::rng::gen_range(0, kinds.len())]
}

pub fn build_rows(
    level: u32,
    fails: u32,
    teach_jump: bool,
    teach_slide: bool,
) -> (Vec<Row>, Vec<Coin>) {
    let length = length_of(level);
    let spacing = speed_of(level, fails) * gap_secs(level, fails);
    let mut rows: Vec<Row> = Vec::new();
    let mut coins = Vec::new();
    let mut at = RUNWAY;
    let mut prev_open = [true; 3];
    let mut taught = (!teach_jump || level < 3, !teach_slide || level < 5);
    while at < length - 24.0 {
        let kind = pick_kind(level);
        let first_hurdle = kind == Kind::Hurdle && !taught.0;
        let first_overhead = kind == Kind::Overhead && !taught.1;
        let forced = kind.needs_action()
            && (first_hurdle
                || first_overhead
                || crate::rng::gen_range(0.0, 1.0) < forced_rate(level, fails));
        if first_hurdle {
            taught.0 = true;
        }
        if first_overhead {
            taught.1 = true;
        }
        let count = if crate::rng::gen_range(0.0, 1.0) < double_rate(level, fails) {
            2
        } else {
            1
        };
        let blocked = pick_lanes(count, prev_open, forced);
        let mut lanes = [None; 3];
        for k in 0..3 {
            if blocked[k] {
                lanes[k] = Some(kind);
            }
        }
        let row = Row {
            at,
            lanes,
            hit: false,
            hit_t: 0.0,
            hit_lanes: [false; 3],
        };
        let open = if forced { [true; 3] } else { row.open() };
        if kind == Kind::Hurdle && !forced {
            if let Some(k) = (0..3).find(|k| blocked[*k]) {
                for (i, h) in [1.0, 1.7, 1.0].into_iter().enumerate() {
                    coins.push(Coin {
                        at: at - 1.6 + i as f32 * 2.0,
                        lane: k,
                        h,
                        taken: false,
                    });
                }
            }
        }
        let len = row.len();
        rows.push(row);
        let next = at + len + spacing;
        let lane = (0..3).find(|k| open[*k] && !blocked[*k]).unwrap_or(1);
        let n = (((next - at - len) / 2.2) as usize).clamp(0, 6);
        if crate::rng::gen_range(0.0, 1.0) < 0.8 {
            for i in 0..n.saturating_sub(1) {
                coins.push(Coin {
                    at: at + len + 2.0 + i as f32 * 2.0,
                    lane,
                    h: 0.8,
                    taken: false,
                });
            }
        }
        prev_open = open;
        at = next;
    }
    coins.sort_by(|a: &Coin, b: &Coin| a.at.total_cmp(&b.at));
    (rows, coins)
}

pub fn row_signature(rows: &[Row]) -> Vec<[u8; 3]> {
    rows.iter()
        .map(|r| {
            let f = |k: Option<Kind>| match k {
                None => 0,
                Some(Kind::Barrier) => 1,
                Some(Kind::Hurdle) => 2,
                Some(Kind::Overhead) => 3,
                Some(Kind::Train) => 4,
            };
            [f(r.lanes[0]), f(r.lanes[1]), f(r.lanes[2])]
        })
        .collect()
}

#[derive(Clone, Copy, Debug)]
pub struct Cam {
    pub eye: Vec3,
    pub target: Vec3,
    pub fov: f32,
}

impl Cam {
    pub fn new(g: &Runner, view: &View) -> Self {
        let f = view.full();
        let aspect = f.w / f.h;
        let back: f32 = 5.6;
        let rise: f32 = 3.1;
        let reach = (back * back + rise * rise).sqrt();
        let tan_h = 3.7 / (reach * aspect);
        let fast = (g.speed / 16.0).clamp(0.0, 1.0);
        let fov = 2.0 * tan_h.max(0.4).atan() + fast * 0.07 + g.kick * 0.05;
        let sh = g.shake.offset();
        let slide = (g.slide_t / SLIDE_TIME).clamp(0.0, 1.0);
        let mut eye = vec3(
            g.x * 0.6 + sh.x * 0.012,
            rise + g.y * 0.35 - slide * 0.3 + sh.y * 0.012,
            -g.dist + back,
        );
        let mut target = vec3(g.x * 0.85, 1.0 + g.y * 0.2, -g.dist - 8.0);
        eye += vec3((g.t * 0.7).sin() * 1.8, -1.1, -1.4) * g.intro;
        target = target.lerp(vec3(g.x, 1.2, -g.dist - 2.0), g.intro);
        let front = vec3(g.x + 2.2, 1.9, -g.dist - 5.2);
        let chest = vec3(g.x, 1.1, -g.dist);
        eye = eye.lerp(front, g.finale);
        target = target.lerp(chest, g.finale);
        Self { eye, target, fov }
    }

    pub fn project(&self, view: &View, p: Vec3) -> Option<Vec2> {
        let f = view.full();
        let fwd = (self.target - self.eye).normalize();
        let right = fwd.cross(Vec3::Y).normalize();
        let up = right.cross(fwd);
        let d = p - self.eye;
        let depth = d.dot(fwd);
        if depth < 0.1 {
            return None;
        }
        let k = 1.0 / (self.fov / 2.0).tan();
        let n = vec2(
            d.dot(right) * k / (depth * f.w / f.h),
            d.dot(up) * k / depth,
        );
        Some(vec2(
            f.x + (n.x * 0.5 + 0.5) * f.w,
            f.y + (0.5 - n.y * 0.5) * f.h,
        ))
    }
}

impl Default for Runner {
    fn default() -> Self {
        Self::new()
    }
}

impl Runner {
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
        let taught_dodge = crate::profile::get(GAME, "taught_dodge") == 1;
        let taught_jump = crate::profile::get(GAME, "taught_jump") == 1;
        let taught_slide = crate::profile::get(GAME, "taught_slide") == 1;
        let (rows, coins_list) = build_rows(level, fails, !taught_jump, !taught_slide);
        Self {
            serial: NEXT_SERIAL.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
            rows,
            coins_list,
            length: length_of(level),
            dist: 0.0,
            speed: 0.0,
            lane: 0,
            x: 0.0,
            lean: 0.0,
            y: 0.0,
            vy: 0.0,
            slide_t: 0.0,
            squash: 0.0,
            hearts: HEARTS,
            hits: 0,
            invuln: 0.0,
            stumble_t: 0.0,
            coins: 0,
            combo: 0,
            combo_t: 0.0,
            score: 0,
            best: Best::default(),
            level,
            fails,
            state: State::Ready,
            stars: 0,
            rank: None,
            t: 0.0,
            end_t: 0.0,
            run_t: 0.0,
            kick: 0.0,
            taught_dodge,
            taught_jump,
            taught_slide,
            fx: Particles::default(),
            shake: Shake::default(),
            pops: Popups::default(),
            bump: Bump::default(),
            flash_hit: 0.0,
            intro: 1.0,
            finale: 0.0,
            queued_jump: 0.0,
            slide_on_land: false,
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

    pub fn target_speed(&self) -> f32 {
        speed_of(self.level, self.fails)
    }

    pub fn grounded(&self) -> bool {
        self.y <= 0.001 && self.vy <= 0.0
    }

    pub fn sliding(&self) -> bool {
        self.slide_t > 0.0 && self.y <= 0.05
    }

    pub fn progress(&self) -> f32 {
        (self.dist / self.length).clamp(0.0, 1.0)
    }

    pub fn player_world(&self) -> Vec3 {
        at_world(self.dist, self.x, self.y)
    }

    pub fn player_screen(&self, view: &View) -> Vec2 {
        Cam::new(self, view)
            .project(view, self.player_world() + Vec3::Y * 0.7)
            .unwrap_or(view.center())
    }

    pub fn blinking(&self) -> bool {
        self.invuln > 0.0 && ((self.invuln * 14.0) as i32) % 2 == 0
    }

    pub fn rows_left(&self) -> usize {
        self.rows
            .iter()
            .filter(|r| r.at + r.len() > self.dist)
            .count()
    }

    pub fn coach(&self) -> Option<Coach> {
        if self.state != State::Playing {
            return None;
        }
        if !self.taught_dodge && self.level <= 2 {
            return Some(Coach::Dodge);
        }
        let next = self
            .rows
            .iter()
            .find(|r| r.at + r.len() > self.dist && !r.hit)?;
        if next.at - self.dist > 45.0 || !next.needs_action() {
            return None;
        }
        match next.kind() {
            Kind::Hurdle if !self.taught_jump => Some(Coach::Jump),
            Kind::Overhead if !self.taught_slide => Some(Coach::Slide),
            _ => None,
        }
    }

    fn teach(&mut self, key: &str) {
        let flag = match key {
            "taught_dodge" => &mut self.taught_dodge,
            "taught_jump" => &mut self.taught_jump,
            _ => &mut self.taught_slide,
        };
        if !*flag {
            *flag = true;
            crate::profile::set(GAME, key, 1);
        }
    }

    fn feet_dust(&mut self, view: &View, n: usize) {
        let at = self.player_screen(view) + vec2(0.0, 40.0);
        self.fx.emit(
            at,
            &[Color::from_rgba(255, 240, 210, 255)],
            Burst {
                count: n,
                speed: (40.0, 160.0),
                size: (4.0, 9.0),
                life: (0.25, 0.5),
                gravity: -60.0,
                spread: std::f32::consts::PI,
                angle: -std::f32::consts::FRAC_PI_2,
                ..Default::default()
            },
        );
    }

    fn change_lane(&mut self, d: i32, view: &View) {
        let to = self.lane + d;
        if !(-1..=1).contains(&to) {
            self.x += d as f32 * 0.18;
            sfx::play(Sfx::Tick);
            return;
        }
        self.lane = to;
        self.lean = d as f32;
        self.kick = 1.0;
        self.teach("taught_dodge");
        sfx::play(Sfx::Whoosh);
        self.feet_dust(view, 4);
    }

    fn jump(&mut self, view: &View) {
        if !self.grounded() {
            self.queued_jump = 0.18;
            return;
        }
        self.vy = JUMP_V;
        self.y = 0.001;
        self.slide_t = 0.0;
        self.squash = -1.0;
        self.teach("taught_jump");
        sfx::play(Sfx::Jump);
        self.feet_dust(view, 6);
    }

    fn slide(&mut self) {
        if !self.grounded() {
            self.vy = self.vy.min(-14.0);
            self.slide_on_land = true;
            return;
        }
        self.start_slide();
    }

    fn start_slide(&mut self) {
        self.slide_t = SLIDE_TIME;
        self.slide_on_land = false;
        self.teach("taught_slide");
        sfx::play(Sfx::Swish);
    }

    fn steer(&mut self, input: &Input, view: &View) {
        match input.dir() {
            Some(Dir::Left) => self.change_lane(-1, view),
            Some(Dir::Right) => self.change_lane(1, view),
            Some(Dir::Up) => self.jump(view),
            Some(Dir::Down) => self.slide(),
            None => {}
        }
    }

    fn body(&mut self, dt: f32, view: &View) {
        let want = lane_x(self.lane);
        self.x = approach(self.x, want, 16.0, dt);
        self.lean = approach(self.lean, 0.0, 9.0, dt);
        self.kick = approach(self.kick, 0.0, 6.0, dt);
        self.queued_jump = (self.queued_jump - dt).max(0.0);
        self.slide_t = (self.slide_t - dt).max(0.0);
        self.squash = approach(self.squash, 0.0, 12.0, dt);
        if self.y > 0.0 || self.vy > 0.0 {
            self.vy -= GRAVITY * dt;
            self.y += self.vy * dt;
            if self.y <= 0.0 {
                self.y = 0.0;
                self.vy = 0.0;
                self.squash = 1.0;
                self.feet_dust(view, 5);
                if self.slide_on_land {
                    self.start_slide();
                } else if self.queued_jump > 0.0 {
                    self.queued_jump = 0.0;
                    self.jump(view);
                }
            }
        }
    }

    fn collides(&self, kind: Kind) -> bool {
        match kind {
            Kind::Barrier | Kind::Train => true,
            Kind::Hurdle => self.y < HURDLE_H,
            Kind::Overhead => !self.sliding(),
        }
    }

    fn check_rows(&mut self, view: &View) {
        if self.invuln > 0.0 {
            return;
        }
        let d = self.dist;
        let mut crash = None;
        let mut lanes = [false; 3];
        for (i, r) in self.rows.iter().enumerate() {
            if r.hit || d + 0.3 < r.at || d - 0.3 > r.at + r.len() {
                continue;
            }
            for (k, kind) in r.lanes.iter().enumerate() {
                if let Some(kind) = kind {
                    let dx = (self.x - lane_x(k as i32 - 1)).abs();
                    if dx < HIT_REACH && self.collides(*kind) {
                        crash = Some((i, *kind));
                        lanes[k] = true;
                    }
                }
            }
        }
        if let Some((i, kind)) = crash {
            self.rows[i].hit = true;
            self.rows[i].hit_t = self.t;
            self.rows[i].hit_lanes = lanes;
            self.stumble(kind, view);
        }
    }

    fn stumble(&mut self, kind: Kind, view: &View) {
        self.hearts = self.hearts.saturating_sub(1);
        self.hits += 1;
        self.invuln = INVULN;
        self.stumble_t = 0.6;
        self.combo = 0;
        self.flash_hit = 1.0;
        self.shake.add(0.55);
        sfx::play(Sfx::Hit);
        let at = self.player_screen(view);
        let color = match kind {
            Kind::Train => Color::from_rgba(120, 180, 255, 255),
            _ => Color::from_rgba(255, 140, 60, 255),
        };
        self.fx.emit(
            at,
            &[color, WHITE],
            Burst {
                count: 22,
                ..Default::default()
            },
        );
        if self.hearts == 0 {
            self.die();
        } else {
            feel::impact(Impact::Medium);
            self.pops.add(
                at + vec2(0.0, -80.0),
                "Oops!",
                Color::from_rgba(255, 120, 90, 255),
                48.0,
            );
        }
    }

    fn die(&mut self) {
        self.score = self.coins * COIN_SCORE + self.dist as u32;
        sfx::play(Sfx::Lose);
        if self.score > self.best.0 && self.best.0 > 0 {
            sfx::say(sfx::Say::Best);
        }
        feel::impact(Impact::Large);
        self.state = State::Dead;
        self.end_t = 0.0;
        self.fails += 1;
        self.rank = self.best.submit(GAME, self.score);
    }

    fn collect(&mut self, view: &View) {
        let d = self.dist;
        let (x, y) = (self.x, self.y);
        let mut got = 0;
        for c in self.coins_list.iter_mut() {
            if c.taken || (c.at - d).abs() > 0.7 {
                continue;
            }
            let dx = (x - lane_x(c.lane as i32 - 1)).abs();
            if dx < COIN_REACH && c.h > y - 0.3 && c.h < y + 1.6 {
                c.taken = true;
                got += 1;
            }
        }
        for _ in 0..got {
            self.coins += 1;
            self.combo += 1;
            self.combo_t = 1.4;
            self.bump.kick();
            sfx::play_step(Sfx::Coin, (self.combo as usize - 1).min(7));
            let at = self.player_screen(view) + vec2(0.0, -30.0);
            self.fx.sparkle(at, Color::from_rgba(255, 220, 60, 255));
        }
        if got > 0 && self.combo > 0 && self.combo.is_multiple_of(10) {
            let at = self.player_screen(view) + vec2(0.0, -110.0);
            self.pops.add(
                at,
                format!("{} coins!", self.combo),
                Color::from_rgba(255, 220, 60, 255),
                46.0,
            );
        }
    }

    fn finish(&mut self, view: &View) {
        self.state = State::Won;
        self.end_t = 0.0;
        self.stars = stars_for(self.hits);
        self.score = self.coins * COIN_SCORE + self.length as u32 + self.hearts * 50;
        crate::profile::award_stars(GAME, self.level, self.stars);
        let skip = if self.stars == 3 { 2 } else { 1 };
        crate::profile::unlock(GAME, (self.level + skip).min(MAX_LEVEL));
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

    pub fn shows_results(&self) -> bool {
        self.state == State::Dead && self.end_t > 0.7
            || self.state == State::Won && self.end_t > WIN_DELAY
    }

    pub fn autopilot(&self, react: f32) -> Input {
        let next = self
            .rows
            .iter()
            .find(|r| r.at + r.len() > self.dist - 0.4 && !r.hit);
        let Some(r) = next else {
            return Input::default();
        };
        let secs = (r.at - self.dist) / self.speed.max(1.0);
        if secs > react {
            return Input::default();
        }
        let mine = (self.lane + 1) as usize;
        if r.lanes[mine].is_none() {
            return Input::default();
        }
        if r.needs_action() {
            let go = match r.kind() {
                Kind::Hurdle => secs < 0.3,
                _ => secs < 0.22,
            };
            if !go {
                return Input::default();
            }
            return Input::key(if r.kind() == Kind::Hurdle {
                Dir::Up
            } else {
                Dir::Down
            });
        }
        let open = r.open();
        let best = (0..3)
            .filter(|k| open[*k])
            .min_by_key(|k| (*k as i32 - mine as i32).abs());
        match best {
            Some(k) if k < mine => Input::key(Dir::Left),
            Some(k) if k > mine => Input::key(Dir::Right),
            _ => Input::default(),
        }
    }
}

impl Game for Runner {
    fn update(&mut self, input: &Input, view: &View, dt: f32) {
        self.t += dt;
        self.fx.update(dt);
        self.shake.update(dt);
        self.pops.update(dt);
        self.bump.update(dt);
        self.flash_hit = (self.flash_hit - dt * 3.0).max(0.0);
        let ready = if self.state == State::Ready { 1.0 } else { 0.0 };
        self.intro = approach(self.intro, ready, 3.0, dt);
        let won = if self.state == State::Won { 1.0 } else { 0.0 };
        self.finale = approach(self.finale, won, 2.2, dt);
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
                self.body(dt, view);
                if input.tapped_in(levels_button(view)) {
                    sfx::play(Sfx::Tap);
                    self.open_select();
                    return;
                }
                if input.pressed || input.action {
                    self.state = State::Playing;
                    sfx::say(sfx::Say::Go);
                }
            }
            State::Playing => {
                self.run_t += dt;
                self.steer(input, view);
                self.body(dt, view);
                self.invuln = (self.invuln - dt).max(0.0);
                self.stumble_t = (self.stumble_t - dt).max(0.0);
                self.combo_t -= dt;
                if self.combo_t <= 0.0 {
                    self.combo = 0;
                }
                let ramp = (self.run_t / 2.5).clamp(0.25, 1.0);
                let slow = if self.stumble_t > 0.0 { 0.45 } else { 1.0 };
                self.speed = approach(self.speed, self.target_speed() * ramp * slow, 6.0, dt);
                self.dist += self.speed * dt;
                self.check_rows(view);
                self.collect(view);
                self.score = self.coins * COIN_SCORE + self.dist as u32;
                if self.state == State::Playing && self.dist >= self.length {
                    self.finish(view);
                }
            }
            State::Dead | State::Won => {
                let before = self.end_t;
                self.end_t += dt;
                self.body(dt, view);
                self.invuln = (self.invuln - dt).max(0.0);
                if self.state == State::Won {
                    let limit = self.length + FINISH_RUN;
                    self.speed = approach(
                        self.speed,
                        if self.dist < limit { 6.0 } else { 0.0 },
                        3.0,
                        dt,
                    );
                    self.dist = (self.dist + self.speed * dt).min(limit);
                    for k in 0..self.stars as usize {
                        let at = WIN_DELAY + STAR_START + STAR_GAP * k as f32;
                        if before < at && self.end_t >= at {
                            sfx::play_step(Sfx::Coin, k * 2);
                        }
                    }
                } else {
                    self.speed = approach(self.speed, 0.0, 5.0, dt);
                    self.dist += self.speed * dt;
                }
                let go = self.end_t > 0.8 && (input.pressed || input.action);
                if go {
                    if input.pressed && levels_button(view).contains(input.pos) {
                        sfx::play(Sfx::Tap);
                        self.open_select();
                    } else if self.state == State::Won {
                        self.enter((self.level + 1).min(MAX_LEVEL));
                    } else {
                        self.enter(self.level);
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup() {
        crate::save::reset_for_test();
        crate::rng::srand(7);
        crate::profile::create("Mia", 0);
    }

    fn run_bot(level: u32, react: f32, max_secs: f32) -> Runner {
        let view = View::ipad_landscape();
        let mut g = Runner::with_level(level);
        let dt = 1.0 / 60.0;
        g.update(&Input::press_at(view.center()), &view, dt);
        let mut t = 0.0;
        while g.state == State::Playing && t < max_secs {
            let input = g.autopilot(react);
            g.update(&input, &view, dt);
            t += dt;
        }
        g
    }

    #[test]
    fn the_first_levels_hold_only_walls_and_one_lane_is_always_open() {
        setup();
        for level in 1..=2 {
            for _ in 0..20 {
                let g = Runner::with_level(level);
                assert!(!g.rows.is_empty());
                for r in &g.rows {
                    assert_eq!(r.blocked(), 1, "level {level}");
                    assert_eq!(r.kind(), Kind::Barrier);
                }
            }
        }
    }

    #[test]
    fn each_new_obstacle_arrives_alone_and_is_then_mixed_in() {
        setup();
        for (level, only) in [(3, Kind::Hurdle), (5, Kind::Overhead), (7, Kind::Train)] {
            assert_eq!(kinds_for(level), &[only]);
            assert_eq!(intro_level(only), level);
            assert!(!kinds_for(level - 1).contains(&only));
            assert!(kinds_for(level + 1).contains(&only));
            assert!(kinds_for(level + 1).len() > 1);
        }
        assert_eq!(kinds_for(30).len(), 4);
    }

    #[test]
    fn speed_and_length_rise_and_gaps_shrink_with_the_level() {
        let mut last = (0.0, 0.0, 9.0);
        for level in 1..=MAX_LEVEL {
            let now = (speed_of(level, 0), length_of(level), gap_secs(level, 0));
            assert!(now.0 >= last.0 && now.1 > last.1 && now.2 <= last.2);
            last = now;
        }
        assert!(speed_of(20, 0) > speed_of(1, 0) * 1.5);
        assert!(gap_secs(20, 0) < gap_secs(1, 0));
        assert!(double_rate(10, 0) > double_rate(3, 0));
        assert_eq!(double_rate(2, 0), 0.0);
    }

    #[test]
    fn dying_makes_the_same_level_gentler() {
        assert!(speed_of(9, 2) < speed_of(9, 0));
        assert!(gap_secs(9, 2) > gap_secs(9, 0));
        assert!(double_rate(9, 2) < double_rate(9, 0));
        assert!(forced_rate(12, 3) < forced_rate(12, 0));
        setup();
        let mut g = Runner::with_level(9);
        g.state = State::Playing;
        g.hearts = 1;
        g.stumble(Kind::Barrier, &View::ipad_landscape());
        assert_eq!(g.state, State::Dead);
        assert_eq!(g.fails, 1);
        g.enter(9);
        assert_eq!(g.fails, 1);
        g.enter(10);
        assert_eq!(g.fails, 0);
    }

    #[test]
    fn every_row_leaves_a_way_through_within_one_lane_of_the_last() {
        setup();
        for level in [1, 3, 5, 7, 8, 12, 20, 30] {
            for _ in 0..10 {
                let g = Runner::with_level(level);
                let mut prev = [true; 3];
                for r in &g.rows {
                    assert!(r.blocked() < 3 || r.needs_action(), "level {level}");
                    let open = if r.needs_action() {
                        [true; 3]
                    } else {
                        r.open()
                    };
                    assert!(reachable(prev, open), "level {level}");
                    prev = open;
                }
            }
        }
    }

    #[test]
    fn rows_leave_room_to_react_at_every_level() {
        setup();
        for level in [1, 6, 15, 30] {
            let g = Runner::with_level(level);
            let floor = speed_of(level, 0) * 0.9;
            for w in g.rows.windows(2) {
                assert!(w[1].at - (w[0].at + w[0].len()) >= floor, "level {level}");
            }
            assert!(g.rows[0].at >= RUNWAY);
        }
    }

    #[test]
    fn the_first_hurdle_and_overhead_force_the_move_that_is_taught() {
        setup();
        for (level, kind) in [(3, Kind::Hurdle), (5, Kind::Overhead)] {
            let g = Runner::with_level(level);
            let first = g.rows.iter().find(|r| r.kind() == kind).unwrap();
            assert_eq!(first.blocked(), 3);
            assert!(first.needs_action());
        }
        crate::profile::set(GAME, "taught_jump", 1);
        crate::rng::srand(3);
        let g = Runner::with_level(3);
        assert!(g.rows.iter().all(|r| r.blocked() < 3));
    }

    #[test]
    fn a_new_layout_every_time_you_play_a_level() {
        setup();
        let mut seen = std::collections::HashSet::new();
        for _ in 0..40 {
            seen.insert(row_signature(&Runner::with_level(6).rows));
        }
        assert_eq!(seen.len(), 40);
    }

    #[test]
    fn a_careful_child_wins_level_one_with_three_stars_and_it_is_saved() {
        setup();
        let g = run_bot(1, 1.2, 120.0);
        assert_eq!(g.state, State::Won);
        assert_eq!(g.hits, 0);
        assert_eq!(g.stars, 3);
        assert_eq!(crate::profile::stars(GAME, 1), 3);
        assert_eq!(crate::profile::level(GAME), 2);
        assert_eq!(crate::profile::unlocked(GAME), 3);
        assert!(g.coins > 5);
        assert!(g.rank.is_some());
        assert_eq!(crate::profile::board(GAME).len(), 1);
    }

    #[test]
    fn a_strong_player_clears_every_band_without_a_scratch() {
        setup();
        for level in [2, 3, 4, 5, 6, 7, 8, 10, 15, 20, 25, 30] {
            let g = run_bot(level, 1.3, 400.0);
            assert_eq!(g.state, State::Won, "level {level}");
            assert_eq!(g.hits, 0, "level {level}");
        }
    }

    #[test]
    fn a_slow_reacting_child_still_gets_through_the_early_levels() {
        setup();
        for level in 1..=2 {
            let g = run_bot(level, 0.45, 120.0);
            assert_eq!(g.state, State::Won, "level {level}");
            assert!(g.stars >= 1);
        }
    }

    #[test]
    fn a_player_who_never_moves_crashes_out_of_a_hard_level() {
        setup();
        let view = View::ipad_landscape();
        let mut g = Runner::with_level(12);
        g.update(&Input::press_at(view.center()), &view, 1.0 / 60.0);
        for _ in 0..60 * 120 {
            g.update(&Input::default(), &view, 1.0 / 60.0);
        }
        assert_eq!(g.state, State::Dead);
        assert_eq!(g.fails, 1);
        assert_eq!(g.rank, Some(0));
    }

    #[test]
    fn a_hit_costs_a_heart_and_grants_a_moment_of_safety() {
        setup();
        let view = View::ipad_landscape();
        let mut g = Runner::with_level(1);
        g.state = State::Playing;
        g.rows[0].lanes = [None, Some(Kind::Barrier), None];
        g.dist = g.rows[0].at - 0.1;
        g.speed = 8.0;
        g.update(&Input::default(), &view, 0.05);
        assert_eq!(g.hearts, HEARTS - 1);
        assert!(g.invuln > 1.0);
        g.rows[1].lanes = [None, Some(Kind::Barrier), None];
        g.dist = g.rows[1].at - 0.1;
        g.update(&Input::default(), &view, 0.05);
        assert_eq!(g.hearts, HEARTS - 1);
    }

    #[test]
    fn a_jump_clears_a_hurdle_and_a_slide_clears_an_overhead() {
        setup();
        let view = View::ipad_landscape();
        let mut g = Runner::with_level(5);
        g.state = State::Playing;
        g.speed = 10.0;
        g.rows[0].lanes = [None, Some(Kind::Hurdle), None];
        g.dist = g.rows[0].at - 2.6;
        g.update(&Input::key(Dir::Up), &view, 1.0 / 60.0);
        for _ in 0..30 {
            g.update(&Input::default(), &view, 1.0 / 60.0);
        }
        assert_eq!(g.hits, 0);
        let mut g = Runner::with_level(5);
        g.state = State::Playing;
        g.speed = 10.0;
        g.rows[0].lanes = [None, Some(Kind::Overhead), None];
        g.dist = g.rows[0].at - 1.5;
        g.update(&Input::key(Dir::Down), &view, 1.0 / 60.0);
        for _ in 0..30 {
            g.update(&Input::default(), &view, 1.0 / 60.0);
        }
        assert_eq!(g.hits, 0);
        let mut g = Runner::with_level(5);
        g.state = State::Playing;
        g.speed = 10.0;
        g.rows[0].lanes = [None, Some(Kind::Overhead), None];
        g.dist = g.rows[0].at - 1.5;
        g.update(&Input::key(Dir::Up), &view, 1.0 / 60.0);
        for _ in 0..40 {
            g.update(&Input::default(), &view, 1.0 / 60.0);
        }
        assert_eq!(g.hits, 1);
    }

    #[test]
    fn swiping_changes_lanes_and_stops_at_the_edge() {
        setup();
        let view = View::ipad_landscape();
        let mut g = Runner::with_level(1);
        g.state = State::Playing;
        g.update(&Input::key(Dir::Left), &view, 1.0 / 60.0);
        assert_eq!(g.lane, -1);
        g.update(&Input::key(Dir::Left), &view, 1.0 / 60.0);
        assert_eq!(g.lane, -1);
        for _ in 0..30 {
            g.update(&Input::default(), &view, 1.0 / 60.0);
        }
        assert!((g.x - lane_x(-1)).abs() < 0.05);
        g.update(&Input::key(Dir::Right), &view, 1.0 / 60.0);
        g.update(&Input::key(Dir::Right), &view, 1.0 / 60.0);
        assert_eq!(g.lane, 1);
    }

    #[test]
    fn coins_are_picked_up_in_the_lane_you_run_and_ladder_the_sound() {
        setup();
        let view = View::ipad_landscape();
        let mut g = Runner::with_level(1);
        g.state = State::Playing;
        g.speed = 8.0;
        g.rows.clear();
        g.coins_list = vec![
            Coin {
                at: 1.0,
                lane: 1,
                h: 0.8,
                taken: false,
            },
            Coin {
                at: 1.5,
                lane: 1,
                h: 0.8,
                taken: false,
            },
            Coin {
                at: 1.5,
                lane: 0,
                h: 0.8,
                taken: false,
            },
        ];
        g.dist = 0.9;
        sfx::take_pending();
        for _ in 0..30 {
            g.update(&Input::default(), &view, 1.0 / 60.0);
        }
        assert_eq!(g.coins, 2);
        assert!(!g.coins_list[2].taken);
        assert!(g.score >= 2 * COIN_SCORE);
    }

    #[test]
    fn the_coach_teaches_dodging_first_then_each_new_move_once() {
        setup();
        let view = View::ipad_landscape();
        let mut g = Runner::with_level(1);
        g.state = State::Playing;
        assert_eq!(g.coach(), Some(Coach::Dodge));
        g.update(&Input::key(Dir::Right), &view, 1.0 / 60.0);
        assert_eq!(g.coach(), None);
        assert_eq!(crate::profile::get(GAME, "taught_dodge"), 1);
        let mut g = Runner::with_level(3);
        g.state = State::Playing;
        g.dist = g.rows.iter().find(|r| r.needs_action()).unwrap().at - 20.0;
        assert_eq!(g.coach(), Some(Coach::Jump));
        g.update(&Input::key(Dir::Up), &view, 1.0 / 60.0);
        assert_eq!(g.coach(), None);
        assert_eq!(crate::profile::get(GAME, "taught_jump"), 1);
        assert!(Runner::with_level(3).taught_jump);
    }

    #[test]
    fn a_perfect_clear_skips_a_level_ahead_and_a_messy_one_does_not() {
        setup();
        let view = View::ipad_landscape();
        let mut g = Runner::with_level(4);
        g.state = State::Playing;
        g.hits = 1;
        g.dist = g.length;
        g.update(&Input::default(), &view, 1.0 / 60.0);
        assert_eq!(g.state, State::Won);
        assert_eq!(g.stars, 2);
        assert_eq!(crate::profile::unlocked(GAME), 5);
        let mut g = Runner::with_level(5);
        g.state = State::Playing;
        g.dist = g.length;
        g.update(&Input::default(), &view, 1.0 / 60.0);
        assert_eq!(g.stars, 3);
        assert_eq!(crate::profile::unlocked(GAME), 7);
    }

    #[test]
    fn stars_follow_the_hits() {
        assert_eq!(stars_for(0), 3);
        assert_eq!(stars_for(1), 2);
        assert_eq!(stars_for(2), 1);
    }

    #[test]
    fn a_new_game_resumes_at_the_level_reached_and_progress_is_per_player() {
        setup();
        crate::profile::unlock(GAME, 5);
        crate::profile::set_level(GAME, 5);
        assert_eq!(Runner::new().level, 5);
        crate::profile::create("Leo", 3);
        assert_eq!(Runner::new().level, 1);
        let mia = crate::profile::list()
            .into_iter()
            .find(|p| p.name == "Mia")
            .unwrap();
        crate::profile::select(&mia.id);
        assert_eq!(Runner::new().level, 5);
    }

    #[test]
    fn level_select_opens_only_unlocked_levels() {
        setup();
        crate::profile::unlock(GAME, 3);
        let view = View::ipad_landscape();
        let l = select_layout(&view);
        assert_eq!(l.cells.len(), MAX_LEVEL as usize);
        assert_eq!(MAX_LEVEL, super::super::helix::MAX_LEVEL);
        let mut g = Runner::new();
        g.open_select();
        g.update(&Input::press_at(l.cells[7].center()), &view, 0.016);
        assert_eq!(g.state, State::Select);
        g.update(&Input::press_at(l.cells[1].center()), &view, 0.016);
        assert_eq!(g.state, State::Ready);
        assert_eq!(g.level, 2);
    }

    #[test]
    fn dying_posts_the_score_once() {
        setup();
        let view = View::ipad_landscape();
        let mut g = Runner::with_level(5);
        g.state = State::Playing;
        g.hearts = 1;
        g.coins = 4;
        g.stumble(Kind::Barrier, &view);
        assert_eq!(g.rank, Some(0));
        for _ in 0..60 {
            g.update(&Input::default(), &view, 0.05);
        }
        assert_eq!(crate::profile::board(GAME).len(), 1);
    }

    #[test]
    fn the_camera_keeps_all_three_lanes_on_screen_in_both_orientations() {
        setup();
        for view in [View::ipad_landscape(), View::ipad_portrait()] {
            let mut g = Runner::with_level(1);
            g.state = State::Playing;
            for lane in -1..=1 {
                g.lane = lane;
                g.x = lane_x(lane);
                let cam = Cam::new(&g, &view);
                let f = view.full();
                for ahead in [0.0, 6.0] {
                    let p = cam
                        .project(&view, at_world(g.dist + ahead, lane_x(lane), 0.0))
                        .unwrap();
                    assert!(
                        p.x > f.x + f.w * 0.05 && p.x < f.right() - f.w * 0.05,
                        "{p:?}"
                    );
                    assert!(p.y > f.y && p.y < f.bottom(), "{p:?}");
                }
                let me = g.player_screen(&view);
                assert!(me.y > f.h * 0.4 && me.y < f.h * 0.9, "{me:?}");
            }
            g.x = 0.0;
            let left = Cam::new(&g, &view)
                .project(&view, at_world(0.0, -LANE_W * 1.5, 0.0))
                .unwrap();
            let right = Cam::new(&g, &view)
                .project(&view, at_world(0.0, LANE_W * 1.5, 0.0))
                .unwrap();
            assert!(
                left.x > 0.0 && right.x < view.full().right(),
                "{left:?} {right:?} {:?}",
                view.full()
            );
        }
    }
}
