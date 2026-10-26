pub mod arena;

use crate::engine::*;
use crate::feel::{self, Impact};
use crate::fx::*;
use crate::geom::*;
use crate::profile::{self, Better};
use crate::rng as rand;
use crate::sfx::{self, Sfx};
use arena::{Arena, MAX_LEVEL};
use glam::{ivec2, vec2, IVec2, Vec2};
use std::collections::VecDeque;

pub const GAME: &str = "paper";

pub const GW: i32 = 60;
pub const GH: i32 = 44;
pub const STEP: f32 = 0.11;
pub const START_R: i32 = 2;
const RESPAWN: f32 = 3.0;
const HUNT_RANGE: i32 = 10;
const TOP: f32 = 100.0;
const END_DELAY: f32 = 0.5;
const VISIBLE_CELLS: f32 = 28.0;

pub const COLORS: [(u8, u8, u8); 5] = [
    (30, 167, 225),
    (230, 75, 75),
    (255, 204, 0),
    (128, 190, 31),
    (255, 140, 200),
];

pub const NAMES: [&str; 5] = ["You", "Zippy", "Bloop", "Mango", "Pip"];

pub fn name(id: usize) -> &'static str {
    NAMES[id % NAMES.len()]
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Death {
    pub victim: usize,
    pub killer: Option<usize>,
    pub at: IVec2,
}

pub fn claimed_cells(seen: &[Option<u8>], owner: &[Option<u8>]) -> Vec<usize> {
    seen.iter()
        .zip(owner)
        .enumerate()
        .filter(|(_, (a, b))| a != b && b.is_some())
        .map(|(i, _)| i)
        .collect()
}

pub fn ripple_delay(distance: f32) -> f32 {
    (distance * 0.02).min(0.5)
}

pub fn tile_lift(age: f32) -> f32 {
    if age < 0.0 {
        0.0
    } else {
        ease_out_back(age / 0.35)
    }
}

#[derive(Clone, Debug)]
pub struct Player {
    pub pos: IVec2,
    pub prev: IVec2,
    pub dir: Dir,
    pub next_dir: Option<Dir>,
    pub trail: Vec<IVec2>,
    pub alive: bool,
    pub respawn: f32,
    pub bot: bool,
    bot_steps: i32,
    bot_turns: i32,
    pace_acc: f32,
}

impl Player {
    fn spawn(pos: IVec2, dir: Dir, bot: bool) -> Self {
        Self {
            pos,
            prev: pos,
            dir,
            next_dir: None,
            trail: vec![],
            alive: true,
            respawn: 0.0,
            bot,
            bot_steps: 0,
            bot_turns: 0,
            pace_acc: 0.0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum State {
    Playing,
    Lost,
    Won,
    Select,
}

pub struct Paper {
    pub owner: Vec<Option<u8>>,
    pub trail: Vec<Option<u8>>,
    pub players: Vec<Player>,
    pub state: State,
    pub deaths: Vec<Death>,
    pub timer: f32,
    pub clock: f32,
    seen: Vec<Option<u8>>,
    from: Vec<Option<u8>>,
    born: Vec<f32>,
    pub particles: Particles,
    pub screen_fx: Particles,
    pub popups: Popups,
    pub shake: Shake,
    pub bump: Bump,
    pub arena: Arena,
    pub started: bool,
    pub elapsed: f32,
    pub training: bool,
    pub grabbed: bool,
    pub rank: Option<usize>,
    pub stars: u8,
    pace: f32,
    aggro: f32,
    pub step: f32,
    fails: u32,
    pub since_end: f32,
    pub since_start: f32,
    halfway: bool,
    forgiven: u32,
    home: IVec2,
}

pub fn idx(p: IVec2) -> usize {
    (p.y * GW + p.x) as usize
}

pub fn in_bounds(p: IVec2) -> bool {
    p.x >= 0 && p.y >= 0 && p.x < GW && p.y < GH
}

impl Paper {
    pub fn empty() -> Self {
        Self {
            owner: vec![None; (GW * GH) as usize],
            trail: vec![None; (GW * GH) as usize],
            players: vec![],
            state: State::Playing,
            deaths: vec![],
            timer: 0.0,
            clock: 0.0,
            seen: vec![None; (GW * GH) as usize],
            from: vec![None; (GW * GH) as usize],
            born: vec![-10.0; (GW * GH) as usize],
            particles: Particles::default(),
            screen_fx: Particles::default(),
            popups: Popups::default(),
            shake: Shake::default(),
            bump: Bump::default(),
            arena: Arena::for_level(1, 0),
            started: true,
            elapsed: 0.0,
            training: false,
            grabbed: false,
            rank: None,
            stars: 0,
            pace: 1.0,
            aggro: 0.0,
            step: STEP,
            fails: 0,
            since_end: 0.0,
            since_start: 0.0,
            halfway: false,
            forgiven: 0,
            home: ivec2(GW / 2, GH / 2),
        }
    }

    pub fn new() -> Self {
        let level = profile::level(GAME).clamp(1, MAX_LEVEL);
        Self::with_arena(level, profile::get(GAME, "fails").max(0) as u32)
    }

    pub fn with_arena(level: u32, fails: u32) -> Self {
        let arena = Arena::for_level(level, fails);
        let mut g = Self::empty();
        g.arena = arena;
        g.pace = arena.pace;
        g.aggro = arena.aggro;
        g.step = STEP / arena.tempo;
        g.fails = fails;
        g.started = false;
        g.training = arena.level == 1 && profile::stars(GAME, 1) == 0;
        let spots = [
            ivec2(GW - 11, GH - 10),
            ivec2(10, 9),
            ivec2(GW - 11, 9),
            ivec2(10, GH - 10),
        ];
        g.add_player(g.home, Dir::Right, false);
        for s in spots.iter().take(arena.bots) {
            g.add_player(*s, Dir::Up, true);
        }
        profile::set_level(GAME, arena.level);
        profile::unlock(GAME, arena.level);
        g
    }

    pub fn add_player(&mut self, at: IVec2, dir: Dir, bot: bool) -> usize {
        let id = self.players.len();
        if id == 0 {
            self.home = at;
        }
        self.players.push(Player::spawn(at, dir, bot));
        self.claim_square(id, at);
        self.seen = self.owner.clone();
        id
    }

    fn claim_square(&mut self, id: usize, at: IVec2) {
        for dy in -START_R..=START_R {
            for dx in -START_R..=START_R {
                let p = at + ivec2(dx, dy);
                if in_bounds(p) {
                    self.owner[idx(p)] = Some(id as u8);
                }
            }
        }
    }

    pub fn share(&self, id: usize) -> f32 {
        let n = self.owner.iter().filter(|o| **o == Some(id as u8)).count();
        n as f32 / self.owner.len() as f32
    }

    pub fn kill(&mut self, id: usize) {
        self.kill_by(id, None);
    }

    pub fn kill_by(&mut self, id: usize, killer: Option<usize>) {
        if !self.players[id].alive {
            return;
        }
        if id == 0 && self.training && !self.grabbed {
            self.forgive();
            return;
        }
        let p = &mut self.players[id];
        self.deaths.push(Death {
            victim: id,
            killer,
            at: p.pos,
        });
        p.alive = false;
        p.respawn = RESPAWN;
        p.trail.clear();
        for c in self.trail.iter_mut().chain(self.owner.iter_mut()) {
            if *c == Some(id as u8) {
                *c = None;
            }
        }
    }

    fn forgive(&mut self) {
        for c in self.trail.iter_mut() {
            if *c == Some(0) {
                *c = None;
            }
        }
        let home = self.home;
        self.claim_square(0, home);
        let p = &mut self.players[0];
        p.trail.clear();
        p.pos = home;
        p.prev = home;
        p.dir = Dir::Right;
        p.next_dir = None;
        self.forgiven += 1;
    }

    pub fn close_loop(&mut self, id: usize) {
        let me = Some(id as u8);
        let trail = std::mem::take(&mut self.players[id].trail);
        for t in &trail {
            self.trail[idx(*t)] = None;
            self.owner[idx(*t)] = me;
        }
        let mut outside = vec![false; self.owner.len()];
        let mut q = VecDeque::new();
        for x in 0..GW {
            for y in 0..GH {
                if (x == 0 || y == 0 || x == GW - 1 || y == GH - 1)
                    && self.owner[idx(ivec2(x, y))] != me
                {
                    outside[idx(ivec2(x, y))] = true;
                    q.push_back(ivec2(x, y));
                }
            }
        }
        while let Some(p) = q.pop_front() {
            for d in Dir::ALL {
                let n = p + d.delta();
                if in_bounds(n) && !outside[idx(n)] && self.owner[idx(n)] != me {
                    outside[idx(n)] = true;
                    q.push_back(n);
                }
            }
        }
        let mut victims = vec![];
        for ((owner, trail), out) in self.owner.iter_mut().zip(&self.trail).zip(&outside) {
            if !out && *owner != me {
                *owner = me;
                if let Some(other) = trail {
                    victims.push(*other as usize);
                }
            }
        }
        for v in victims {
            self.kill_by(v, (v != id).then_some(id));
        }
    }

    pub fn tick(&mut self) {
        let mut moving = vec![true; self.players.len()];
        for (id, go) in moving.iter_mut().enumerate() {
            let p = &mut self.players[id];
            if !p.bot || !p.alive {
                continue;
            }
            p.pace_acc += self.pace;
            if p.pace_acc < 1.0 {
                *go = false;
                p.prev = p.pos;
                continue;
            }
            p.pace_acc -= 1.0;
            self.bot_think(id);
        }
        for (id, go) in moving.into_iter().enumerate() {
            if !self.players[id].alive || !go {
                continue;
            }
            let p = &mut self.players[id];
            if let Some(nd) = p.next_dir.take() {
                if nd != p.dir.opposite() || p.trail.is_empty() {
                    p.dir = nd;
                }
            }
            let target = p.pos + p.dir.delta();
            p.prev = p.pos;
            if !in_bounds(target) {
                continue;
            }
            p.pos = target;
            let me = Some(id as u8);
            if let Some(owner) = self.trail[idx(target)] {
                self.kill_by(owner as usize, (owner as usize != id).then_some(id));
                if owner as usize == id {
                    continue;
                }
            }
            if self.owner[idx(target)] == me {
                if !self.players[id].trail.is_empty() {
                    self.close_loop(id);
                }
            } else {
                self.trail[idx(target)] = me;
                self.players[id].trail.push(target);
            }
        }
        for a in 0..self.players.len() {
            for b in (a + 1)..self.players.len() {
                let (pa, pb) = (&self.players[a], &self.players[b]);
                if pa.alive && pb.alive && pa.pos == pb.pos {
                    let a_home = self.owner[idx(pa.pos)] == Some(a as u8);
                    let b_home = self.owner[idx(pb.pos)] == Some(b as u8);
                    if !a_home {
                        self.kill_by(a, Some(b));
                    }
                    if !b_home {
                        self.kill_by(b, Some(a));
                    }
                }
            }
        }
    }

    fn safe(&self, id: usize, d: Dir) -> bool {
        let p = &self.players[id];
        let n = p.pos + d.delta();
        in_bounds(n)
            && self.trail[idx(n)] != Some(id as u8)
            && !(d == p.dir.opposite() && !p.trail.is_empty())
    }

    fn hunt_dir(&self, id: usize) -> Option<Dir> {
        let prey = self.players.first().filter(|p| p.alive && !p.bot)?;
        let me = &self.players[id];
        if self.aggro <= 0.0 || me.trail.len() > 14 || rand::gen_range(0.0, 1.0) >= self.aggro {
            return None;
        }
        let target = prey
            .trail
            .iter()
            .copied()
            .min_by_key(|t| (*t - me.pos).abs().element_sum())?;
        let delta = target - me.pos;
        if delta.abs().element_sum() > HUNT_RANGE {
            return None;
        }
        let mut order = vec![];
        if delta.x != 0 {
            order.push((
                delta.x.abs(),
                if delta.x < 0 { Dir::Left } else { Dir::Right },
            ));
        }
        if delta.y != 0 {
            order.push((delta.y.abs(), if delta.y < 0 { Dir::Up } else { Dir::Down }));
        }
        order.sort_by_key(|(far, _)| -far);
        order
            .into_iter()
            .map(|(_, d)| d)
            .find(|d| self.safe(id, *d))
    }

    fn bot_think(&mut self, id: usize) {
        if let Some(d) = self.hunt_dir(id) {
            self.players[id].next_dir = Some(d);
            return;
        }
        let me = Some(id as u8);
        let pos = self.players[id].pos;
        let home = self.owner[idx(pos)] == me;
        let trail_len = self.players[id].trail.len() as i32;
        let dir = self.players[id].dir;
        let mut want = dir;
        if home {
            let p = &mut self.players[id];
            p.bot_turns = 0;
            p.bot_steps = rand::gen_range(4, 9);
            if rand::gen_range(0, 4) == 0 {
                want = Dir::ALL[rand::gen_range(0, 4)];
            }
        } else if trail_len > 14 || self.players[id].bot_turns >= 2 {
            want = self.dir_home(id).unwrap_or(dir);
        } else {
            let p = &mut self.players[id];
            p.bot_steps -= 1;
            if p.bot_steps <= 0 {
                p.bot_turns += 1;
                p.bot_steps = rand::gen_range(4, 9);
                want = if rand::gen_range(0, 2) == 0 {
                    turn_left(dir)
                } else {
                    turn_right(dir)
                };
            }
        }
        if !self.safe(id, want) {
            want = [dir, turn_left(dir), turn_right(dir), dir.opposite()]
                .into_iter()
                .find(|d| self.safe(id, *d))
                .unwrap_or(dir);
        }
        self.players[id].next_dir = Some(want);
    }

    fn dir_home(&self, id: usize) -> Option<Dir> {
        let me = Some(id as u8);
        let start = self.players[id].pos;
        let mut seen = vec![false; self.owner.len()];
        let mut first: Vec<Option<Dir>> = vec![None; self.owner.len()];
        let mut q = VecDeque::new();
        seen[idx(start)] = true;
        q.push_back(start);
        while let Some(p) = q.pop_front() {
            if self.owner[idx(p)] == me && p != start {
                return first[idx(p)];
            }
            for d in Dir::ALL {
                let n = p + d.delta();
                if !in_bounds(n) || seen[idx(n)] || self.trail[idx(n)] == me {
                    continue;
                }
                if p == start && !self.safe(id, d) {
                    continue;
                }
                seen[idx(n)] = true;
                first[idx(n)] = first[idx(p)].or(Some(d));
                q.push_back(n);
            }
        }
        None
    }

    fn respawn_bots(&mut self, dt: f32) {
        for id in 1..self.players.len() {
            if !self.players[id].bot || self.players[id].alive {
                continue;
            }
            self.players[id].respawn -= dt;
            if self.players[id].respawn > 0.0 {
                continue;
            }
            for _ in 0..40 {
                let at = ivec2(rand::gen_range(4, GW - 4), rand::gen_range(4, GH - 4));
                let clear = (-START_R - 1..=START_R + 1).all(|dy| {
                    (-START_R - 1..=START_R + 1).all(|dx| {
                        let c = idx(at + ivec2(dx, dy));
                        self.owner[c].is_none() && self.trail[c].is_none()
                    })
                });
                if clear {
                    self.players[id] = Player::spawn(at, Dir::ALL[rand::gen_range(0, 4)], true);
                    self.claim_square(id, at);
                    break;
                }
            }
        }
    }

    fn react(&mut self, cell: f32) {
        let center = |p: IVec2| (p.as_vec2() + vec2(0.5, 0.5)) * cell;
        for d in std::mem::take(&mut self.deaths) {
            let at = center(d.at);
            let col = color(d.victim);
            self.particles.emit(
                at,
                &[col, darker(col, 0.8), WHITE],
                Burst {
                    count: 28,
                    size: (cell * 0.15, cell * 0.4),
                    shape: Shape::Square,
                    ..Default::default()
                },
            );
            if d.victim == 0 {
                sfx::play(Sfx::Hit);
                feel::impact(Impact::Large);
                self.shake.add(0.9);
            } else if d.killer == Some(0) {
                sfx::play(Sfx::Crunch);
                feel::impact(Impact::Medium);
                self.shake.add(0.35);
                self.bump.kick();
                self.popups.add(
                    at - vec2(0.0, cell),
                    format!("Got {}!", name(d.victim)),
                    Color::from_rgba(255, 220, 80, 255),
                    cell * 1.3,
                );
            } else {
                self.popups.add(
                    at - vec2(0.0, cell),
                    format!("{} is out", name(d.victim)),
                    Color::from_rgba(230, 230, 240, 255),
                    cell * 0.9,
                );
            }
        }
        let claimed = claimed_cells(&self.seen, &self.owner);
        let mut gained = [0usize; 8];
        for &i in &claimed {
            let o = self.owner[i].unwrap_or(0) as usize;
            let c = ivec2(i as i32 % GW, i as i32 / GW);
            let head = self.players.get(o).map_or(c, |p| p.pos);
            let dist = (c - head).as_vec2().length();
            self.from[i] = self.seen[i];
            self.born[i] = self.clock + ripple_delay(dist);
            if o < gained.len() {
                gained[o] += 1;
            }
        }
        self.seen.clone_from(&self.owner);
        for (id, n) in gained.iter().enumerate() {
            if *n == 0 || id >= self.players.len() {
                continue;
            }
            let at = center(self.players[id].pos);
            let col = color(id);
            if id == 0 {
                let pct = *n as f32 / self.owner.len() as f32 * 100.0;
                if pct >= 0.1 {
                    self.grabbed = true;
                    sfx::play(Sfx::Coin);
                    self.popups.add(
                        at - vec2(0.0, cell * 1.4),
                        format!("+{pct:.1}%"),
                        WHITE,
                        cell * 1.5,
                    );
                    self.shake.add(0.12 + (pct / 12.0).min(0.3));
                    self.bump.kick();
                    self.particles.emit(
                        at,
                        &[col, WHITE, Color::from_rgba(255, 230, 120, 255)],
                        Burst {
                            count: 12 + (*n).min(160) / 6,
                            shape: Shape::Star,
                            size: (cell * 0.2, cell * 0.45),
                            gravity: 200.0,
                            ..Default::default()
                        },
                    );
                }
            } else if *n > 40 {
                self.particles.puff(at, col);
            }
        }
    }

    pub fn cell_age(&self, i: usize) -> f32 {
        self.clock - self.born[i]
    }

    pub fn shown_owner(&self, i: usize) -> Option<u8> {
        if self.owner[i].is_some() && self.cell_age(i) < 0.0 {
            self.from[i]
        } else {
            self.owner[i]
        }
    }

    pub fn layout(&self, view: &View) -> (f32, Vec2) {
        let top = TOP;
        let area = vec2(view.w - 20.0, view.h - top - 10.0);
        let fit = (area.x / GW as f32).min(area.y / GH as f32);
        let cell = fit.max(view.w.min(view.h) / VISIBLE_CELLS).floor();
        let board = vec2(GW as f32, GH as f32) * cell;
        let center = vec2(10.0, top) + area / 2.0;
        let p = &self.players[0];
        let head = (p
            .prev
            .as_vec2()
            .lerp(p.pos.as_vec2(), (self.timer / self.step).clamp(0.0, 1.0))
            + 0.5)
            * cell;
        let axis = |board: f32, area: f32, head: f32, center: f32, lo: f32| {
            if board <= area {
                center - board / 2.0
            } else {
                (center - head).clamp(lo + area - board, lo)
            }
        };
        let origin = vec2(
            axis(board.x, area.x, head.x, center.x, 10.0),
            axis(board.y, area.y, head.y, center.y, top),
        );
        (cell, origin)
    }
}

impl Default for Paper {
    fn default() -> Self {
        Self::new()
    }
}

fn turn_left(d: Dir) -> Dir {
    match d {
        Dir::Up => Dir::Left,
        Dir::Left => Dir::Down,
        Dir::Down => Dir::Right,
        Dir::Right => Dir::Up,
    }
}

fn turn_right(d: Dir) -> Dir {
    turn_left(d).opposite()
}

pub fn color(id: usize) -> Color {
    let (r, g, b) = COLORS[id % COLORS.len()];
    Color::from_rgba(r, g, b, 255)
}

pub const GRASS_OUTER: Color = Color::new(0.45, 0.72, 0.38, 1.0);
pub const HEADS: [&str; 5] = ["head0", "head1", "head2", "head3", "head4"];
pub const TUFTS: [&str; 2] = ["tuft0", "tuft1"];
pub const GRASS_TILE_CELLS: i32 = 10;
pub const GRASS_TILE_CELLS_Y: i32 = 11;

pub fn tuft_at(x: i32, y: i32) -> Option<usize> {
    let h = (x.wrapping_mul(73856093) ^ y.wrapping_mul(19349663)) as u32;
    let h = h ^ (h >> 13);
    if h.is_multiple_of(23) {
        Some((h >> 5) as usize % 9)
    } else {
        None
    }
}
impl Game for Paper {
    fn update(&mut self, input: &Input, view: &View, dt: f32) {
        self.clock += dt;
        self.particles.update(dt);
        self.screen_fx.update(dt);
        self.popups.update(dt);
        self.shake.update(dt);
        self.bump.update(dt);
        match self.state {
            State::Select => return self.update_select(input, view, dt),
            State::Won | State::Lost => return self.update_end(input, view, dt),
            State::Playing => {}
        }
        if !self.started {
            if input.tapped_in(arenas_button(view)) {
                self.open_select();
            } else if input.dir().is_some() || input.tap() {
                self.started = true;
                sfx::say(sfx::Say::Go);
            } else {
                return;
            }
        }
        if let Some(d) = input.dir() {
            self.players[0].next_dir = Some(d);
        }
        self.since_start += dt;
        self.elapsed += dt;
        self.timer += dt;
        let forgiven = self.forgiven;
        while self.timer >= self.step {
            self.timer -= self.step;
            self.tick();
            self.respawn_bots(self.step);
        }
        let (cell, _) = self.layout(view);
        if self.forgiven > forgiven {
            let at = (self.home.as_vec2() + vec2(0.5, -1.0)) * cell;
            self.popups.add(
                at,
                "Oops! Try again",
                Color::from_rgba(255, 220, 80, 255),
                cell * 1.2,
            );
            sfx::play(Sfx::Wrong);
        }
        self.react(cell);
        let share = self.share(0);
        if !self.halfway && share >= self.arena.target / 2.0 && share < self.arena.target {
            self.halfway = true;
            sfx::play(Sfx::Score);
            sfx::say(sfx::Say::Great);
            self.popups.add(
                vec2(view.w / 2.0 - cell * 8.0, cell * 6.0),
                "Halfway there!",
                WHITE,
                cell * 1.6,
            );
        }
        if !self.players[0].alive {
            self.finish_loss();
        } else if share >= self.arena.target {
            self.finish_win(view);
        }
    }
}

pub fn arenas_button(view: &View) -> Rect {
    Rect::new(view.w / 2.0 - 110.0, view.h - 175.0, 220.0, 64.0)
}

pub fn win_arenas_button(view: &View) -> Rect {
    let r = win_panel(view);
    Rect::new(
        r.x + r.w / 2.0 + 10.0,
        r.y + r.h - 96.0,
        r.w / 2.0 - 40.0,
        70.0,
    )
}

pub fn win_next_button(view: &View) -> Rect {
    let r = win_panel(view);
    Rect::new(r.x + 30.0, r.y + r.h - 96.0, r.w / 2.0 - 40.0, 70.0)
}

pub fn lost_arenas_button(view: &View) -> Rect {
    Rect::new(view.w / 2.0 - 100.0, view.h - 64.0, 200.0, 52.0)
}

pub fn win_panel(view: &View) -> Rect {
    let w = 560.0f32.min(view.w - 40.0);
    let h = 440.0;
    let c = view.center() + vec2(0.0, 20.0);
    Rect::new(c.x - w / 2.0, c.y - h / 2.0, w, h)
}

pub fn select_back_button(view: &View) -> Rect {
    Rect::new(view.w / 2.0 - 100.0, view.h - 110.0, 200.0, 64.0)
}

pub fn select_cells(view: &View) -> Vec<Rect> {
    let cols = if view.w >= view.h { 4 } else { 3 };
    let rows = (MAX_LEVEL as usize).div_ceil(cols);
    let top = 176.0;
    let area = vec2(view.w - 60.0, view.h - top - 120.0);
    let gap = 16.0;
    let size = ((area.x - gap * (cols - 1) as f32) / cols as f32)
        .min((area.y - gap * (rows - 1) as f32) / rows as f32)
        .min(190.0);
    let total = vec2(
        size * cols as f32 + gap * (cols - 1) as f32,
        size * rows as f32 + gap * (rows - 1) as f32,
    );
    let o = vec2(view.w / 2.0 - total.x / 2.0, top + (area.y - total.y) / 2.0);
    (0..MAX_LEVEL as usize)
        .map(|i| {
            Rect::new(
                o.x + (i % cols) as f32 * (size + gap),
                o.y + (i / cols) as f32 * (size + gap),
                size,
                size,
            )
        })
        .collect()
}

pub fn unlocked_arenas() -> u32 {
    profile::unlocked(GAME).clamp(1, MAX_LEVEL)
}

impl Paper {
    pub fn open_select(&mut self) {
        self.state = State::Select;
        self.since_end = 0.0;
    }

    fn update_select(&mut self, input: &Input, view: &View, dt: f32) {
        self.since_end += dt;
        if !input.pressed {
            return;
        }
        if input.tapped_in(select_back_button(view)) {
            sfx::play(Sfx::Click);
            self.restart_at(profile::level(GAME).max(1), 0);
            return;
        }
        let open = unlocked_arenas();
        for (i, r) in select_cells(view).iter().enumerate() {
            let level = i as u32 + 1;
            if r.contains(input.pos) {
                if level <= open {
                    sfx::play(Sfx::Pop);
                    self.restart_at(level, 0);
                } else {
                    sfx::play(Sfx::Wrong);
                }
                return;
            }
        }
    }

    fn update_end(&mut self, input: &Input, view: &View, dt: f32) {
        self.since_end += dt;
        if self.since_end < END_DELAY || !input.tap() {
            return;
        }
        let level = self.arena.level;
        let won = self.state == State::Won;
        let again = if won && level >= MAX_LEVEL {
            level
        } else if won {
            level + 1
        } else {
            level
        };
        let button = if won {
            win_arenas_button(view)
        } else {
            lost_arenas_button(view)
        };
        if input.pressed && button.contains(input.pos) {
            sfx::play(Sfx::Click);
            self.open_select();
        } else {
            let fails = if won { 0 } else { self.fails + 1 };
            self.restart_at(again, fails);
        }
    }

    fn restart_at(&mut self, level: u32, fails: u32) {
        let clock = self.clock;
        *self = Self::with_arena(level, fails);
        self.clock = clock;
    }

    fn finish_loss(&mut self) {
        self.state = State::Lost;
        self.since_end = 0.0;
        sfx::play(Sfx::Lose);
        sfx::say(sfx::Say::GameOver);
        let score = arena::score(self.arena.level, self.share(0), false);
        self.rank = profile::submit(GAME, score, Better::Higher);
        profile::set(GAME, "fails", self.fails as i64 + 1);
    }

    fn finish_win(&mut self, view: &View) {
        let level = self.arena.level;
        self.state = State::Won;
        self.since_end = 0.0;
        self.stars = self.arena.stars(self.elapsed);
        let next = arena::next_level(level);
        profile::award_stars(GAME, level, self.stars);
        profile::unlock(GAME, next);
        profile::set_level(GAME, next);
        profile::set(GAME, "fails", 0);
        let score = arena::score(level, self.share(0), true);
        self.rank = profile::submit(GAME, score, Better::Higher);
        sfx::play(Sfx::Win);
        sfx::say(if level >= MAX_LEVEL {
            sfx::Say::Congrats
        } else {
            sfx::Say::Win
        });
        feel::impact_tinted(Impact::Large, color(0));
        self.screen_fx.confetti(view);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn solo() -> Paper {
        let mut g = Paper::empty();
        g.add_player(ivec2(10, 10), Dir::Right, false);
        g
    }

    fn walk(g: &mut Paper, moves: &[(Dir, usize)]) {
        for (d, n) in moves {
            g.players[0].next_dir = Some(*d);
            for _ in 0..*n {
                g.tick();
            }
        }
    }

    #[test]
    fn paper_sprites_all_exist() {
        let b = std::fs::read(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../assets/art/paper.kga"
        ))
        .unwrap();
        let have: Vec<String> = crate::art::parse(&b)
            .unwrap()
            .frames
            .keys()
            .cloned()
            .collect();
        for n in HEADS
            .iter()
            .chain(&TUFTS)
            .chain(&["grass", "fence", "tile"])
        {
            assert!(have.iter().any(|h| h == n), "paper sheet lacks {n}");
        }
        assert_eq!(HEADS.len(), COLORS.len());
    }

    #[test]
    fn grass_tufts_are_sparse_and_stable() {
        let all: Vec<_> = (0..GW)
            .flat_map(|x| (0..GH).map(move |y| tuft_at(x, y)))
            .collect();
        let n = all.iter().filter(|t| t.is_some()).count();
        assert!(n > 40 && n < (GW * GH) as usize / 8, "{n}");
        assert_eq!(tuft_at(7, 9), tuft_at(7, 9));
    }

    #[test]
    fn losing_plays_the_sad_jingle() {
        let v = View::ipad_landscape();
        let mut g = solo();
        crate::sfx::take_pending();
        g.kill(0);
        g.update(&Input::default(), &v, 0.01);
        assert_eq!(g.state, State::Lost);
        assert!(crate::sfx::take_pending().contains(&Sfx::Lose));
    }

    #[test]
    fn closing_a_loop_claims_the_inside() {
        let mut g = solo();
        let before = g.owner.iter().filter(|o| o.is_some()).count();
        assert_eq!(before, 25);
        walk(
            &mut g,
            &[
                (Dir::Right, 6),
                (Dir::Down, 6),
                (Dir::Left, 6),
                (Dir::Up, 4),
            ],
        );
        assert!(g.players[0].trail.is_empty());
        assert_eq!(g.owner[idx(ivec2(15, 15))], Some(0));
        assert_eq!(g.owner[idx(ivec2(14, 13))], Some(0));
        assert!(g.owner.iter().filter(|o| o.is_some()).count() > 50);
        assert!(g.trail.iter().all(|t| t.is_none()));
    }

    #[test]
    fn crossing_your_own_trail_kills_you() {
        let mut g = solo();
        walk(
            &mut g,
            &[
                (Dir::Right, 6),
                (Dir::Down, 3),
                (Dir::Left, 2),
                (Dir::Up, 4),
            ],
        );
        assert!(!g.players[0].alive);
        assert!(g.owner.iter().all(|o| o.is_none()));
    }

    #[test]
    fn cutting_an_enemy_trail_kills_the_enemy() {
        let mut g = solo();
        let bot = g.add_player(ivec2(30, 10), Dir::Down, false);
        for _ in 0..5 {
            g.players[bot].next_dir = Some(Dir::Down);
            g.tick();
        }
        assert!(g.trail[idx(ivec2(30, 14))] == Some(bot as u8));
        g.players[0].pos = ivec2(28, 14);
        g.players[0].dir = Dir::Right;
        for _ in 0..2 {
            g.players[bot].next_dir = Some(Dir::Down);
            g.players[0].next_dir = Some(Dir::Right);
            g.tick();
        }
        assert!(!g.players[bot].alive);
        assert!(g.players[0].alive);
        assert!(g.owner.iter().all(|o| *o != Some(bot as u8)));
    }

    #[test]
    fn camera_keeps_player_on_screen_on_phone_and_ipad() {
        for v in [
            View::fit(1080.0, 2400.0),
            View::ipad_landscape(),
            View::ipad_portrait(),
        ] {
            let mut g = Paper::new();
            for at in [
                ivec2(0, 0),
                ivec2(GW - 1, GH - 1),
                ivec2(GW - 1, 0),
                ivec2(30, 20),
            ] {
                g.players[0].pos = at;
                g.players[0].prev = at;
                let (cell, origin) = g.layout(&v);
                assert!(cell >= v.w.min(v.h) / VISIBLE_CELLS - 1.0);
                let head = origin + at.as_vec2() * cell;
                assert!(
                    head.x >= 0.0 && head.x + cell <= v.w && head.y >= 64.0 && head.y + cell <= v.h,
                    "{at} at {head} in {v:?}"
                );
            }
        }
    }

    #[test]
    fn walls_stop_instead_of_kill() {
        let mut g = solo();
        walk(&mut g, &[(Dir::Up, 30)]);
        assert!(g.players[0].alive);
        assert_eq!(g.players[0].pos.y, 0);
    }

    #[test]
    fn bots_survive_and_grow_alone() {
        rand::srand(7);
        let mut g = Paper::empty();
        g.add_player(ivec2(2, 2), Dir::Right, false);
        let bot = g.add_player(ivec2(30, 22), Dir::Up, true);
        let start = g.share(bot);
        let mut deaths = 0;
        for _ in 0..1500 {
            g.players[0].next_dir = Some(Dir::Down);
            g.tick();
            if !g.players[bot].alive {
                deaths += 1;
                g.respawn_bots(10.0);
            }
        }
        assert!(deaths <= 2, "bot died {deaths} times");
        assert!(g.share(bot) > start * 3.0, "bot share {}", g.share(bot));
    }

    #[test]
    fn killing_a_bot_credits_the_player() {
        let mut g = solo();
        let bot = g.add_player(ivec2(30, 10), Dir::Down, false);
        for _ in 0..5 {
            g.players[bot].next_dir = Some(Dir::Down);
            g.tick();
        }
        g.players[0].pos = ivec2(28, 14);
        g.players[0].dir = Dir::Right;
        for _ in 0..2 {
            g.players[bot].next_dir = Some(Dir::Down);
            g.players[0].next_dir = Some(Dir::Right);
            g.tick();
        }
        assert_eq!(g.deaths.len(), 1);
        assert_eq!(g.deaths[0].victim, bot);
        assert_eq!(g.deaths[0].killer, Some(0));
    }

    #[test]
    fn crossing_your_own_trail_has_no_killer() {
        let mut g = solo();
        walk(
            &mut g,
            &[
                (Dir::Right, 6),
                (Dir::Down, 3),
                (Dir::Left, 2),
                (Dir::Up, 4),
            ],
        );
        assert_eq!(g.deaths.len(), 1);
        assert_eq!(g.deaths[0].victim, 0);
        assert_eq!(g.deaths[0].killer, None);
    }

    #[test]
    fn claimed_cells_lists_only_new_ownership() {
        let seen = vec![None, Some(0), Some(1), None];
        let owner = vec![Some(0), Some(0), Some(0), None];
        assert_eq!(claimed_cells(&seen, &owner), vec![0, 2]);
    }

    #[test]
    fn ripple_travels_outward_and_is_capped() {
        assert_eq!(ripple_delay(0.0), 0.0);
        assert!(ripple_delay(10.0) > ripple_delay(3.0));
        assert_eq!(ripple_delay(1000.0), 0.5);
    }

    #[test]
    fn tile_rises_after_its_delay() {
        assert_eq!(tile_lift(-0.1), 0.0);
        assert!(tile_lift(0.2) > 0.5);
        assert!((tile_lift(5.0) - 1.0).abs() < 1e-4);
    }

    #[test]
    fn closing_a_loop_pops_a_percentage_and_ripples() {
        let mut g = solo();
        let cell = 20.0;
        g.react(cell);
        assert!(g.popups.list.is_empty());
        walk(
            &mut g,
            &[
                (Dir::Right, 6),
                (Dir::Down, 6),
                (Dir::Left, 6),
                (Dir::Up, 4),
            ],
        );
        g.react(cell);
        assert!(g
            .popups
            .list
            .iter()
            .any(|p| p.text.starts_with('+') && p.text.ends_with('%')));
        let far = idx(ivec2(15, 15));
        g.clock = 0.0;
        assert_eq!(g.owner[far], Some(0));
        assert!(!g.particles.list.is_empty());
        assert!(g.shake.active());
        assert!(g.born[far] > g.born[idx(ivec2(11, 10))] - 1e-6);
        assert_eq!(g.shown_owner(far), None);
        g.clock = 2.0;
        assert_eq!(g.shown_owner(far), Some(0));
    }

    #[test]
    fn player_death_shakes_the_screen() {
        let mut g = solo();
        walk(
            &mut g,
            &[
                (Dir::Right, 6),
                (Dir::Down, 3),
                (Dir::Left, 2),
                (Dir::Up, 4),
            ],
        );
        g.react(20.0);
        assert!(g.shake.active());
        assert!(g.deaths.is_empty());
        assert!(!g.particles.list.is_empty());
    }

    fn fresh_player() {
        crate::save::reset_for_test();
        crate::profile::create("Mia", 0);
    }

    fn start(g: &mut Paper, v: &View) {
        g.update(&Input::press_at(v.center()), v, 0.01);
        assert!(g.started);
    }

    fn run(g: &mut Paper, v: &View, secs: f32) {
        for _ in 0..(secs * 60.0) as usize {
            g.update(&Input::default(), v, 1.0 / 60.0);
        }
    }

    fn box_loop(g: &mut Paper) {
        walk(
            g,
            &[
                (Dir::Right, 6),
                (Dir::Down, 6),
                (Dir::Left, 6),
                (Dir::Up, 4),
            ],
        );
    }

    fn self_cross(g: &mut Paper) {
        walk(
            g,
            &[
                (Dir::Left, 12),
                (Dir::Down, 3),
                (Dir::Right, 2),
                (Dir::Up, 3),
            ],
        );
    }

    fn nearest_trail(g: &Paper, id: usize) -> i32 {
        let me = g.players[id].pos;
        g.players[0]
            .trail
            .iter()
            .map(|t| (*t - me).abs().element_sum())
            .min()
            .unwrap()
    }

    #[test]
    fn a_new_player_starts_in_a_gentle_first_arena() {
        fresh_player();
        let g = Paper::new();
        assert_eq!(g.arena.level, 1);
        assert!(g.training);
        assert!(!g.started);
        assert_eq!(g.players.len(), 2);
        assert!(g.arena.target <= 0.1);
        assert!(g.step > STEP);
    }

    #[test]
    fn the_game_waits_for_the_first_swipe() {
        fresh_player();
        let v = View::ipad_landscape();
        let mut g = Paper::new();
        run(&mut g, &v, 3.0);
        assert_eq!(g.players[0].pos, g.home);
        assert_eq!(g.elapsed, 0.0);
        g.update(
            &Input {
                swipe: Some(Dir::Right),
                ..Default::default()
            },
            &v,
            1.0 / 60.0,
        );
        run(&mut g, &v, 1.0);
        assert!(g.players[0].pos.x > g.home.x);
    }

    #[test]
    fn training_forgives_a_mistake_and_sends_you_home() {
        fresh_player();
        let mut g = Paper::new();
        self_cross(&mut g);
        assert!(g.players[0].alive);
        assert_eq!(g.forgiven, 1);
        assert_eq!(g.players[0].pos, g.home);
        assert!(g.players[0].trail.is_empty());
        assert!(g.trail.iter().all(|t| *t != Some(0)));
        assert!(g.deaths.is_empty());
        assert!(g.owner[idx(g.home)] == Some(0));
    }

    #[test]
    fn training_ends_after_the_first_grab() {
        fresh_player();
        let mut g = Paper::new();
        box_loop(&mut g);
        g.react(20.0);
        assert!(g.grabbed);
        self_cross(&mut g);
        assert!(!g.players[0].alive);
    }

    #[test]
    fn later_arenas_do_not_forgive() {
        fresh_player();
        let mut g = Paper::with_arena(2, 0);
        assert!(!g.training);
        self_cross(&mut g);
        assert!(!g.players[0].alive);
        crate::profile::award_stars(GAME, 1, 1);
        let mut again = Paper::with_arena(1, 0);
        assert!(!again.training);
        self_cross(&mut again);
        assert!(!again.players[0].alive);
    }

    #[test]
    fn a_slow_looper_survives_the_first_ten_seconds() {
        for seed in 0..8 {
            rand::srand(seed);
            fresh_player();
            crate::profile::award_stars(GAME, 1, 1);
            let v = View::ipad_landscape();
            let mut g = Paper::new();
            start(&mut g, &v);
            let plan = [Dir::Right, Dir::Down, Dir::Left, Dir::Up];
            for f in 0..600 {
                let swipe = (f % 60 == 0).then(|| plan[(f / 60) % 4]);
                g.update(
                    &Input {
                        swipe,
                        ..Default::default()
                    },
                    &v,
                    1.0 / 60.0,
                );
            }
            assert_eq!(g.state, State::Playing, "seed {seed}");
        }
    }

    #[test]
    fn first_arena_bots_are_slow_and_peaceful() {
        fresh_player();
        let mut g = Paper::new();
        let bot = 1;
        let mut moves = 0;
        for _ in 0..100 {
            let before = g.players[bot].pos;
            g.tick();
            if g.players[bot].pos != before {
                moves += 1;
            }
        }
        assert!(moves < 55 && moves > 25, "{moves}");
        assert_eq!(g.aggro, 0.0);
    }

    #[test]
    fn deeper_arenas_have_more_and_faster_bots() {
        fresh_player();
        let shallow = Paper::with_arena(1, 0);
        let deep = Paper::with_arena(MAX_LEVEL, 0);
        assert!(deep.players.len() > shallow.players.len() + 2);
        assert!(deep.pace > shallow.pace);
        assert!(deep.step < shallow.step);
        assert!(deep.aggro > shallow.aggro);
        assert!(deep.arena.target > shallow.arena.target);
    }

    #[test]
    fn aggressive_bots_chase_your_trail() {
        rand::srand(3);
        let mut g = Paper::empty();
        g.add_player(ivec2(10, 10), Dir::Right, false);
        let bot = g.add_player(ivec2(30, 10), Dir::Up, true);
        g.aggro = 1.0;
        for _ in 0..8 {
            g.players[0].next_dir = Some(Dir::Right);
            g.tick();
        }
        g.players[bot].pos = ivec2(16, 16);
        g.players[bot].prev = ivec2(16, 16);
        g.players[bot].dir = Dir::Left;
        let before = nearest_trail(&g, bot);
        g.hunt_dir(bot).expect("hunting");
        g.tick();
        assert!(nearest_trail(&g, bot) < before);
    }

    #[test]
    fn peaceful_bots_never_hunt() {
        let mut g = Paper::empty();
        g.add_player(ivec2(10, 10), Dir::Right, false);
        let bot = g.add_player(ivec2(14, 14), Dir::Up, true);
        for _ in 0..8 {
            g.players[0].next_dir = Some(Dir::Right);
            g.tick();
        }
        g.aggro = 0.0;
        assert!(g.hunt_dir(bot).is_none());
    }

    #[test]
    fn winning_saves_stars_unlocks_and_resumes_at_the_next_arena() {
        fresh_player();
        let v = View::ipad_landscape();
        let mut g = Paper::new();
        start(&mut g, &v);
        for c in 0..(GW * GH) as usize {
            if c % 7 != 0 {
                g.owner[c] = Some(0);
            }
        }
        g.update(&Input::default(), &v, 0.2);
        assert_eq!(g.state, State::Won);
        assert!(g.stars >= 1);
        assert_eq!(profile::stars(GAME, 1), g.stars);
        assert_eq!(profile::unlocked(GAME), 2);
        assert_eq!(profile::level(GAME), 2);
        assert_eq!(g.rank, Some(0));
        let next = Paper::new();
        assert_eq!(next.arena.level, 2);
        assert!(next.arena.target > g.arena.target);
    }

    #[test]
    fn progress_is_kept_per_player() {
        fresh_player();
        profile::unlock(GAME, 5);
        profile::set_level(GAME, 5);
        assert_eq!(Paper::new().arena.level, 5);
        crate::profile::create("Leo", 1);
        assert_eq!(Paper::new().arena.level, 1);
        assert_eq!(unlocked_arenas(), 1);
    }

    #[test]
    fn losing_keeps_the_arena_and_eases_it_after_repeats() {
        fresh_player();
        profile::set_level(GAME, 4);
        profile::unlock(GAME, 4);
        let v = View::ipad_landscape();
        let first = Paper::new();
        let mut g = first;
        start(&mut g, &v);
        g.kill(0);
        g.update(&Input::default(), &v, 0.1);
        assert_eq!(g.state, State::Lost);
        assert_eq!(profile::get(GAME, "fails"), 1);
        let base = Arena::for_level(4, 0);
        for _ in 0..3 {
            g.since_end = 1.0;
            g.update(&Input::press_at(vec2(5.0, 5.0)), &v, 0.01);
            assert_eq!(g.arena.level, 4);
            start(&mut g, &v);
            g.kill(0);
            g.update(&Input::default(), &v, 0.1);
        }
        assert!(g.fails >= 2);
        assert!(g.arena.target < base.target);
        assert!(g.arena.pace < base.pace);
        let resumed = Paper::new();
        assert!(resumed.arena.target < base.target);
        assert_eq!(resumed.arena.level, 4);
    }

    #[test]
    fn a_win_after_struggling_resets_the_help() {
        fresh_player();
        profile::set(GAME, "fails", 5);
        let v = View::ipad_landscape();
        let mut g = Paper::new();
        start(&mut g, &v);
        for c in 0..g.owner.len() {
            g.owner[c] = Some(0);
        }
        g.update(&Input::default(), &v, 0.2);
        assert_eq!(g.state, State::Won);
        assert_eq!(profile::get(GAME, "fails"), 0);
    }

    #[test]
    fn the_end_screen_waits_a_moment_before_taking_a_tap() {
        fresh_player();
        let v = View::ipad_landscape();
        let mut g = Paper::with_arena(2, 0);
        start(&mut g, &v);
        g.kill(0);
        g.update(&Input::default(), &v, 0.1);
        assert_eq!(g.state, State::Lost);
        g.update(&Input::press_at(vec2(5.0, 5.0)), &v, 0.1);
        assert_eq!(g.state, State::Lost);
        g.since_end = 1.0;
        g.update(&Input::press_at(vec2(5.0, 5.0)), &v, 0.1);
        assert_eq!(g.state, State::Playing);
    }

    #[test]
    fn the_arena_picker_only_opens_unlocked_arenas() {
        fresh_player();
        profile::unlock(GAME, 3);
        let v = View::ipad_landscape();
        let cells = select_cells(&v);
        assert_eq!(cells.len(), MAX_LEVEL as usize);
        for r in &cells {
            assert!(r.x >= 0.0 && r.x + r.w <= v.w && r.y >= 0.0 && r.y + r.h <= v.h);
        }
        let mut g = Paper::new();
        g.open_select();
        g.update(&Input::press_at(cells[6].center()), &v, 0.01);
        assert_eq!(g.state, State::Select);
        g.update(&Input::press_at(cells[2].center()), &v, 0.01);
        assert_eq!(g.state, State::Playing);
        assert_eq!(g.arena.level, 3);
        assert_eq!(profile::level(GAME), 3);
    }

    #[test]
    fn the_picker_fits_phones_and_portrait_ipads() {
        for v in [View::fit(1080.0, 2400.0), View::ipad_portrait()] {
            let cells = select_cells(&v);
            for r in &cells {
                assert!(r.x >= 0.0 && r.x + r.w <= v.w && r.y >= 150.0 && r.y + r.h <= v.h - 100.0);
            }
        }
    }

    #[test]
    fn scores_ride_the_leaderboard() {
        fresh_player();
        let v = View::ipad_landscape();
        let mut g = Paper::with_arena(2, 0);
        start(&mut g, &v);
        g.kill(0);
        g.update(&Input::default(), &v, 0.1);
        assert_eq!(g.rank, Some(0));
        assert_eq!(profile::board(GAME).len(), 1);
    }

    #[test]
    fn every_bot_has_a_name() {
        for id in 1..5 {
            assert!(!name(id).is_empty());
            assert_ne!(name(id), name(0));
        }
    }
}
