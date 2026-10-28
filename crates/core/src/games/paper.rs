pub mod arena;
pub mod learn;
pub mod pilot;
pub mod rl;
pub mod world;

use crate::engine::*;
use crate::feel::{self, Impact};
use crate::fx::*;
use crate::geom::*;
use crate::profile::{self, Better};
use crate::sfx::{self, Say, Sfx};
use glam::{vec2, Vec2};
pub use world::{
    cell_at, cell_center, cell_of, idx, in_grid, Cause, Death, Player, World, CELL, MAX_PLAYERS, N,
    RES, SIZE, START_R,
};

pub const GAME: &str = "paper";
pub const TOP: f32 = 96.0;
pub const RESPAWN_WAIT: f32 = 0.9;
pub const FEED_LIFE: f32 = 3.2;
pub const FEED_MAX: usize = 3;
const MIN_CELLS: f32 = 17.0;
const MAX_CELLS: f32 = 46.0;
const THREAT_RANGE: f32 = 26.0;
const STEER_DEAD_MM: f32 = 2.5;
const MIN_CLAIM_PCT: f32 = 0.05;

pub const COLORS: [(u8, u8, u8); 12] = [
    (40, 170, 230),
    (236, 64, 60),
    (250, 200, 30),
    (140, 200, 50),
    (232, 120, 170),
    (255, 150, 30),
    (170, 80, 200),
    (40, 196, 180),
    (60, 90, 170),
    (250, 130, 105),
    (150, 160, 165),
    (236, 66, 130),
];

pub const HEADS: [&str; 12] = [
    "head0", "head1", "head2", "head3", "head4", "head5", "head6", "head7", "head8", "head9",
    "head10", "head11",
];

pub fn color(id: usize) -> Color {
    let (r, g, b) = COLORS[id % COLORS.len()];
    Color::from_rgba(r, g, b, 255)
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum State {
    Ready,
    Playing,
    Dead,
}

#[derive(Clone, Debug, PartialEq)]
pub struct FeedLine {
    pub text: String,
    pub color: Color,
    pub mine: bool,
    pub age: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Run {
    pub peak: f32,
    pub kills: u32,
    pub time: f32,
    pub cause: Option<Cause>,
    pub killer: Option<&'static str>,
}

pub struct Paper {
    pub w: World,
    pub state: State,
    pub clock: f32,
    pub since: f32,
    pub zoom: f32,
    pub cam: Vec2,
    pub particles: Particles,
    pub popups: Popups,
    pub shake: Shake,
    pub bump: Bump,
    pub feed: Vec<FeedLine>,
    pub run: Run,
    pub best: i64,
    pub rank: Option<usize>,
    pub new_best: bool,
    pub lives: u32,
    pub skill: f32,
    pub milestone: Option<i64>,
    pub steer: Option<f32>,
    last_claim_say: f32,
}

impl Paper {
    pub fn new() -> Self {
        let best = profile::personal_best(GAME).unwrap_or(0);
        Self::with_best(best)
    }

    pub fn with_best(best: i64) -> Self {
        let skill = arena::skill(best);
        let mut w = World::new(skill);
        let n = arena::bots_for(skill);
        let spots = world::spread_spots(n + 1);
        w.add_player(spots[0], 0.0, false);
        for s in &spots[1..] {
            let a = crate::rng::gen_range(-std::f32::consts::PI, std::f32::consts::PI);
            w.add_player(*s, a, true);
        }
        w.players[0].speed = 0.0;
        let start = w.players[0].pos;
        let mut g = Self {
            w,
            state: State::Ready,
            clock: 0.0,
            since: 0.0,
            zoom: MIN_CELLS,
            cam: start,
            particles: Particles::default(),
            popups: Popups::default(),
            shake: Shake::default(),
            bump: Bump::default(),
            feed: vec![],
            run: Run {
                peak: 0.0,
                kills: 0,
                time: 0.0,
                cause: None,
                killer: None,
            },
            best,
            rank: None,
            new_best: false,
            lives: 0,
            skill,
            milestone: arena::next_milestone(best.max(0)),
            steer: None,
            last_claim_say: -10.0,
        };
        g.zoom = g.target_zoom();
        g
    }

    pub fn me(&self) -> &Player {
        &self.w.players[0]
    }

    pub fn share(&self, id: usize) -> f32 {
        self.w.share(id)
    }

    pub fn my_rank(&self) -> usize {
        self.w
            .ranking()
            .iter()
            .position(|i| *i == 0)
            .unwrap_or(self.w.players.len())
            + 1
    }

    pub fn target_zoom(&self) -> f32 {
        let s = self.share(0).max(0.0);
        MIN_CELLS + (MAX_CELLS - MIN_CELLS) * (s / 0.4).min(1.0).sqrt()
    }

    pub fn cell_px(&self, view: &View) -> f32 {
        let short = view.w.min(view.h - TOP * 0.5);
        short / self.zoom
    }

    pub fn origin(&self, view: &View) -> Vec2 {
        let cell = self.cell_px(view);
        vec2(view.w / 2.0, (view.h + TOP * 0.5) / 2.0) - self.cam * cell
    }

    pub fn to_screen(&self, view: &View, world: Vec2) -> Vec2 {
        self.origin(view) + world * self.cell_px(view)
    }

    pub fn threats(&self, view: &View) -> Vec<(usize, Vec2, f32)> {
        let cell = self.cell_px(view);
        let half = vec2(view.w, view.h) / cell / 2.0;
        let me = self.me().look();
        self.w
            .players
            .iter()
            .enumerate()
            .skip(1)
            .filter(|(_, p)| p.alive)
            .filter_map(|(id, p)| {
                let d = p.look() - self.cam;
                let off =
                    d.x.abs() > half.x * 0.92 || d.y.abs() > (half.y - TOP / cell * 0.5) * 0.92;
                let dist = p.look().distance(me);
                (off && dist < THREAT_RANGE).then_some((id, d.normalize_or_zero(), dist))
            })
            .collect()
    }

    fn read_input(&self, input: &Input, view: &View) -> Option<f32> {
        let keys = vec2(
            (input.right as i32 - input.left as i32) as f32,
            (input.down_key as i32 - input.up as i32) as f32,
        );
        let keys = if keys == Vec2::ZERO {
            input.key_dir.map_or(Vec2::ZERO, |d| d.delta().as_vec2())
        } else {
            keys
        };
        if keys != Vec2::ZERO {
            return Some(world::angle_of(keys));
        }
        if input.down {
            let head = self.to_screen(view, self.me().pos);
            let d = input.pos - head;
            if d.length() > STEER_DEAD_MM * input.mm() {
                return Some(world::angle_of(d));
            }
        }
        input.swipe.map(|d| world::angle_of(d.delta().as_vec2()))
    }

    pub fn aim(&mut self, angle: f32) {
        self.steer = Some(angle);
        self.w.players[0].want = angle;
    }

    fn begin(&mut self) {
        self.state = State::Playing;
        self.w.players[0].speed = arena::PLAYER_SPEED;
        self.since = 0.0;
        sfx::say(Say::Go);
        sfx::play(Sfx::Whoosh);
    }

    pub fn respawn_me(&mut self) {
        let at = self
            .w
            .free_spot(200)
            .or_else(|| self.w.free_spot(400))
            .unwrap_or(Vec2::splat(SIZE / 2.0));
        self.w.respawn(0, at);
        self.steer = None;
        self.cam = self.w.players[0].pos;
        self.zoom = self.target_zoom();
        self.state = State::Playing;
        self.since = 0.0;
        self.lives += 1;
        self.new_best = false;
        self.rank = None;
        self.run = Run {
            peak: 0.0,
            kills: 0,
            time: 0.0,
            cause: None,
            killer: None,
        };
        sfx::play(Sfx::Pop);
        sfx::say(Say::Go);
    }

    fn die(&mut self, d: &Death) {
        self.state = State::Dead;
        self.since = 0.0;
        self.run.peak = self.run.peak.max(self.me().peak);
        self.run.kills = self.me().kills;
        self.run.cause = Some(d.cause);
        self.run.killer = d.killer.map(|k| self.w.players[k].name);
        let score = arena::score(self.run.peak);
        self.new_best = score > self.best;
        self.rank = profile::submit(GAME, score, Better::Higher);
        if self.new_best {
            self.best = score;
            profile::set(
                GAME,
                "kills_best",
                (self.run.kills as i64).max(profile::get(GAME, "kills_best")),
            );
        }
        sfx::play(Sfx::Lose);
        sfx::say(Say::Oops);
        feel::impact_tinted(Impact::Large, color(0));
        self.shake.add(0.8);
    }

    fn feed_line(&mut self, text: String, color: Color, mine: bool) {
        self.feed.insert(
            0,
            FeedLine {
                text,
                color,
                mine,
                age: 0.0,
            },
        );
        self.feed.truncate(FEED_MAX);
    }

    fn react(&mut self) {
        for d in std::mem::take(&mut self.w.deaths) {
            let col = color(d.victim);
            self.particles.emit(
                d.at,
                &[col, darker(col, 0.75), WHITE],
                Burst {
                    count: 26,
                    speed: (3.0, 9.0),
                    size: (0.12, 0.32),
                    life: (0.4, 0.9),
                    gravity: 0.0,
                    drag: 3.0,
                    shape: Shape::Square,
                    ..Default::default()
                },
            );
            let killer = d.killer.map(|k| self.w.players[k].name);
            if d.victim == 0 {
                let text = match (killer, d.cause) {
                    (Some(k), Cause::HeadOn) => format!("You bumped into {k}"),
                    (Some(k), _) => format!("{k} got you"),
                    (None, _) => "You hit your own trail".into(),
                };
                self.feed_line(text, col, true);
                self.die(&d);
            } else if d.killer == Some(0) {
                self.feed_line(format!("You eliminated {}", d.name), color(0), true);
                sfx::play(Sfx::Crunch);
                sfx::say(Say::Gotcha);
                feel::impact(Impact::Medium);
                self.shake.add(0.35);
                self.bump.kick();
                self.popups.add(d.at - vec2(0.0, 1.0), "+1", GOLD, 1.4);
            } else {
                let text = match killer {
                    Some(k) => format!("{k} eliminated {}", d.name),
                    None => format!("{} crashed", d.name),
                };
                self.feed_line(text, col, false);
                if self.me().alive && d.at.distance(self.me().look()) < 14.0 {
                    sfx::play(Sfx::Hit);
                }
            }
        }
        for c in std::mem::take(&mut self.w.claims) {
            if c.id != 0 {
                continue;
            }
            let pct = c.cells as f32 / self.w.total() as f32 * 100.0;
            if pct < MIN_CLAIM_PCT {
                continue;
            }
            self.popups.add(
                c.at - vec2(0.0, 2.4),
                format!("+{pct:.1}%"),
                WHITE,
                1.2 + (pct / 2.0).min(0.8),
            );
            let step = ((pct * 2.0) as usize).min(7);
            sfx::play_step(Sfx::Coin, step);
            self.bump.kick();
            self.shake.add(0.1 + (pct / 10.0).min(0.25));
            self.particles.emit(
                c.at,
                &[color(0), WHITE, GOLD],
                Burst {
                    count: 10 + (c.cells / 8).min(30),
                    speed: (3.0, 10.0),
                    size: (0.15, 0.35),
                    gravity: 0.0,
                    drag: 2.5,
                    shape: Shape::Star,
                    ..Default::default()
                },
            );
            if c.taken > 0 {
                sfx::play(Sfx::Slice);
            }
            if pct >= 1.5 && self.clock - self.last_claim_say > 6.0 {
                self.last_claim_say = self.clock;
                sfx::say(Say::Land);
            }
        }
        let tenths = arena::score(self.share(0));
        if let Some(m) = self.milestone.filter(|m| tenths >= *m && self.me().alive) {
            self.milestone = arena::next_milestone(tenths);
            if m > self.best.max(0) {
                sfx::play(Sfx::LevelUp);
                self.popups.add(
                    self.me().look() - vec2(0.0, 2.5),
                    format!("{}%!", m / 10),
                    GOLD,
                    2.2,
                );
            }
        }
    }

    fn follow(&mut self, dt: f32) {
        let want = self.target_zoom();
        self.zoom = approach(self.zoom, want, 1.5, dt);
        if self.me().alive {
            let target = self.me().look() + self.me().heading() * 1.2;
            let k = 1.0 - (-dt * 8.0).exp();
            self.cam += (target - self.cam) * k;
        }
    }

    fn tick(&mut self, input: &Input, view: &View, dt: f32) {
        if let Some(a) = self.read_input(input, view) {
            self.aim(a);
        }
        self.run.time += dt;
        self.w.advance(dt);
        self.w.respawn_bots(dt);
        self.run.peak = self.run.peak.max(self.me().peak);
    }
}

impl Default for Paper {
    fn default() -> Self {
        Self::new()
    }
}

pub fn play_again_button(view: &View) -> Rect {
    let w = 340.0f32.min(view.w - 60.0);
    Rect::new(view.w / 2.0 - w / 2.0, view.h - 150.0, w, 96.0)
}

pub fn death_panel(view: &View) -> Rect {
    let w = 560.0f32.min(view.w - 40.0);
    let h = 400.0f32.min(view.h - 290.0);
    Rect::new(
        view.w / 2.0 - w / 2.0,
        (view.h - 150.0 - h) / 2.0 + 40.0,
        w,
        h,
    )
}

impl Game for Paper {
    fn update(&mut self, input: &Input, view: &View, dt: f32) {
        self.clock += dt;
        self.since += dt;
        self.particles.update(dt);
        for p in &mut self.popups.list {
            p.age += dt;
        }
        self.popups.list.retain(|p| p.age < p.life);
        self.shake.update(dt);
        self.bump.update(dt);
        for f in &mut self.feed {
            f.age += dt;
        }
        self.feed.retain(|f| f.age < FEED_LIFE);
        match self.state {
            State::Ready => {
                if let Some(a) = self.read_input(input, view) {
                    self.begin();
                    self.aim(a);
                } else if input.tap() {
                    self.begin();
                } else {
                    self.w.advance(dt * 0.5);
                    self.w.respawn_bots(dt);
                }
            }
            State::Playing => self.tick(input, view, dt),
            State::Dead => {
                self.w.advance(dt);
                self.w.respawn_bots(dt);
                let ready = self.since >= RESPAWN_WAIT;
                let tapped = (input.pressed && play_again_button(view).contains(input.pos))
                    || input.action
                    || input.key_dir.is_some();
                if ready && tapped {
                    self.respawn_me();
                }
            }
        }
        self.react();
        self.follow(dt);
    }
}

#[cfg(test)]
mod tests;
