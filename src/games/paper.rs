use crate::engine::draw_circle;
use crate::engine::*;
use crate::feel::{self, Impact};
use crate::fx::*;
use crate::sfx::{self, Sfx};
use macroquad::prelude::*;
use std::collections::VecDeque;

pub const GW: i32 = 60;
pub const GH: i32 = 44;
pub const STEP: f32 = 0.11;
pub const START_R: i32 = 2;
const RESPAWN: f32 = 3.0;
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
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum State {
    Playing,
    Lost,
    Won,
}

pub struct Paper {
    pub owner: Vec<Option<u8>>,
    pub trail: Vec<Option<u8>>,
    pub players: Vec<Player>,
    pub state: State,
    pub best: f32,
    pub deaths: Vec<Death>,
    timer: f32,
    clock: f32,
    seen: Vec<Option<u8>>,
    from: Vec<Option<u8>>,
    born: Vec<f32>,
    particles: Particles,
    screen_fx: Particles,
    popups: Popups,
    shake: Shake,
    bump: Bump,
    start_best: f32,
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
            best: 0.0,
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
            start_best: 0.0,
        }
    }

    pub fn new() -> Self {
        let mut g = Self::empty();
        let spots = [
            ivec2(GW / 2, GH / 2),
            ivec2(10, 9),
            ivec2(GW - 11, 9),
            ivec2(10, GH - 10),
            ivec2(GW - 11, GH - 10),
        ];
        for (i, s) in spots.iter().enumerate() {
            g.add_player(*s, if i == 0 { Dir::Right } else { Dir::Up }, i != 0);
        }
        g
    }

    pub fn add_player(&mut self, at: IVec2, dir: Dir, bot: bool) -> usize {
        let id = self.players.len();
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
        let p = &mut self.players[id];
        if !p.alive {
            return;
        }
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
        for id in 0..self.players.len() {
            if self.players[id].bot && self.players[id].alive {
                self.bot_think(id);
            }
        }
        for id in 0..self.players.len() {
            if !self.players[id].alive {
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

    fn bot_think(&mut self, id: usize) {
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

    fn cell_age(&self, i: usize) -> f32 {
        self.clock - self.born[i]
    }

    fn shown_owner(&self, i: usize) -> Option<u8> {
        if self.owner[i].is_some() && self.cell_age(i) < 0.0 {
            self.from[i]
        } else {
            self.owner[i]
        }
    }

    fn layout(&self, view: &View) -> (f32, Vec2) {
        let top = 64.0;
        let area = vec2(view.w - 20.0, view.h - top - 10.0);
        let fit = (area.x / GW as f32).min(area.y / GH as f32);
        let cell = fit.max(view.w.min(view.h) / VISIBLE_CELLS).floor();
        let board = vec2(GW as f32, GH as f32) * cell;
        let center = vec2(10.0, top) + area / 2.0;
        let p = &self.players[0];
        let head = (p
            .prev
            .as_vec2()
            .lerp(p.pos.as_vec2(), (self.timer / STEP).clamp(0.0, 1.0))
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

fn color(id: usize) -> Color {
    let (r, g, b) = COLORS[id % COLORS.len()];
    Color::from_rgba(r, g, b, 255)
}

mod art {
    crate::sprites!("paper");
}

const GRASS_OUTER: Color = Color::new(0.45, 0.72, 0.38, 1.0);
const HEADS: [&str; 5] = ["head0", "head1", "head2", "head3", "head4"];
const TUFTS: [&str; 2] = ["tuft0", "tuft1"];
const GRASS_TILE_CELLS: i32 = 10;
const GRASS_TILE_CELLS_Y: i32 = 11;

pub fn tuft_at(x: i32, y: i32) -> Option<usize> {
    let h = (x.wrapping_mul(73856093) ^ y.wrapping_mul(19349663)) as u32;
    let h = h ^ (h >> 13);
    if h.is_multiple_of(23) {
        Some((h >> 5) as usize % 9)
    } else {
        None
    }
}

impl Paper {
    fn draw_lawn(&self, board: Vec2, origin: Vec2, view: &View) {
        let tiles = (GW / GRASS_TILE_CELLS, GH / GRASS_TILE_CELLS_Y);
        let size = vec2(board.x / tiles.0 as f32, board.y / tiles.1 as f32);
        let seen = Rect::new(-origin.x, -origin.y, view.w, view.h);
        for ty in 0..tiles.1 {
            for tx in 0..tiles.0 {
                let at = vec2(tx as f32 * size.x, ty as f32 * size.y);
                if at.x > seen.x + seen.w
                    || at.y > seen.y + seen.h
                    || at.x + size.x < seen.x
                    || at.y + size.y < seen.y
                {
                    continue;
                }
                art::sprite_ex(
                    "grass",
                    Rect::new(at.x, at.y, size.x + 0.5, size.y + 0.5),
                    crate::art::Draw {
                        flip_x: tx % 2 == 1,
                        flip_y: ty % 2 == 1,
                        ..Default::default()
                    },
                );
            }
        }
    }

    fn draw_fence(&self, board: Vec2, origin: Vec2, view: &View, cell: f32) {
        let ph = cell * 2.4;
        let pw = art::size("fence").x * ph / art::size("fence").y;
        let n = (board.x / pw).ceil() as usize;
        let draw = crate::art::Draw::default();
        let seen = Rect::new(
            -origin.x - pw,
            -origin.y - pw,
            view.w + pw * 2.0,
            view.h + pw * 2.0,
        );
        for k in 0..n {
            let x = k as f32 * pw + pw / 2.0;
            for y in [-ph * 0.35, board.y + ph * 0.05] {
                if seen.contains(vec2(x, y)) {
                    art::sprite_ex(
                        "fence",
                        Rect::new(x - pw / 2.0, y - ph / 2.0, pw + 1.0, ph),
                        draw,
                    );
                }
            }
        }
        let m = (board.y / pw).ceil() as usize;
        for k in 0..m {
            let y = k as f32 * pw + pw / 2.0;
            for x in [-ph * 0.35, board.x + ph * 0.35] {
                if seen.contains(vec2(x, y)) {
                    art::sprite_ex(
                        "fence",
                        Rect::new(x - pw / 2.0, y - ph / 2.0, pw + 1.0, ph),
                        crate::art::Draw {
                            rotation: std::f32::consts::FRAC_PI_2,
                            ..draw
                        },
                    );
                }
            }
        }
    }
}

impl Game for Paper {
    fn update(&mut self, input: &Input, view: &View, dt: f32) {
        if self.clock == 0.0 {
            sfx::say(sfx::Say::Go);
        }
        self.clock += dt;
        self.particles.update(dt);
        self.screen_fx.update(dt);
        self.popups.update(dt);
        self.shake.update(dt);
        self.bump.update(dt);
        if self.state != State::Playing {
            if input.tap() {
                let best = self.best;
                *self = Self::new();
                self.best = best;
                self.start_best = best;
            }
            return;
        }
        if let Some(d) = input.dir() {
            self.players[0].next_dir = Some(d);
        }
        self.timer += dt;
        while self.timer >= STEP {
            self.timer -= STEP;
            self.tick();
            self.respawn_bots(STEP);
        }
        let (cell, _) = self.layout(view);
        self.react(cell);
        let share = self.share(0);
        self.best = self.best.max(share);
        if !self.players[0].alive {
            self.state = State::Lost;
            sfx::play(Sfx::Lose);
            if self.start_best > 0.0 && self.best > self.start_best {
                sfx::say(sfx::Say::Best);
            }
        } else if share >= 0.5 {
            self.state = State::Won;
            sfx::play(Sfx::Win);
            sfx::say(sfx::Say::Win);
            self.screen_fx.confetti(view);
        }
    }

    fn draw(&self, view: &View) {
        clear_background(GRASS_OUTER);
        let (cell, origin) = self.layout(view);
        let shift = origin + self.shake.offset();
        set_view_camera(view, vec2(shift.x, shift.y));
        let board = vec2(cell * GW as f32, cell * GH as f32);
        draw_rectangle(5.0, 9.0, board.x, board.y, Color::new(0.1, 0.25, 0.1, 0.25));
        self.draw_lawn(board, origin, view);
        let x0 = ((-origin.x / cell).floor() as i32).max(0);
        let y0 = ((-origin.y / cell).floor() as i32).max(0);
        let x1 = (((view.w - origin.x) / cell).ceil() as i32).min(GW);
        let y1 = (((view.h - origin.y) / cell).ceil() as i32).min(GH);
        for y in y0..y1 {
            for x in x0..x1 {
                let i = idx(ivec2(x, y));
                let px = x as f32 * cell;
                let py = y as f32 * cell;
                if let Some(o) = self.shown_owner(i) {
                    let age = self.cell_age(i);
                    let pop = if age < 0.0 {
                        1.0
                    } else {
                        0.8 + 0.2 * tile_lift(age)
                    };
                    let d = (cell + 0.5) * pop;
                    let c = vec2(px + cell / 2.0, py + cell / 2.0);
                    draw_rectangle(
                        px,
                        py,
                        cell + 0.5,
                        cell + 0.5,
                        darker(color(o as usize), 0.75),
                    );
                    art::sprite_ex(
                        "tile",
                        Rect::new(c.x - d / 2.0, c.y - d / 2.0, d, d),
                        crate::art::Draw {
                            tint: color(o as usize),
                            ..Default::default()
                        },
                    );
                    if (0.0..0.3).contains(&age) {
                        draw_rectangle(
                            px,
                            py,
                            cell + 0.5,
                            cell + 0.5,
                            with_alpha(WHITE, 0.55 * (1.0 - age / 0.3)),
                        );
                    }
                } else {
                    if let Some(t) = tuft_at(x, y) {
                        let s = cell * (0.95 + 0.15 * (t % 3) as f32);
                        art::sprite_at(
                            TUFTS[t % TUFTS.len()],
                            vec2(px + cell * 0.5, py + cell * 0.55),
                            s,
                            crate::art::Draw {
                                tint: WHITE,
                                ..Default::default()
                            },
                        );
                    }
                }
                if let Some(t) = self.trail[i] {
                    let pulse = 0.5 + 0.5 * (self.clock * 10.0 - (x + y) as f32 * 0.6).sin();
                    let base = color(t as usize);
                    let d = cell * (0.62 + 0.1 * pulse);
                    let c = vec2(px + cell / 2.0, py + cell / 2.0);
                    draw_circle(
                        c.x,
                        c.y + d * 0.1,
                        d * 0.5,
                        with_alpha(darker(base, 0.5), 0.5),
                    );
                    draw_circle(
                        c.x,
                        c.y,
                        d * 0.5,
                        Color::from_vec(base.to_vec() * 0.55 + vec4(0.45, 0.45, 0.45, 0.0)),
                    );
                    draw_circle(
                        c.x - d * 0.14,
                        c.y - d * 0.16,
                        d * 0.17,
                        with_alpha(WHITE, 0.85),
                    );
                }
            }
        }
        self.draw_fence(board, origin, view, cell);
        let frac = (self.timer / STEP).clamp(0.0, 1.0);
        for (id, p) in self.players.iter().enumerate() {
            if !p.alive {
                continue;
            }
            let at = p.prev.as_vec2().lerp(p.pos.as_vec2(), frac);
            let bob = (self.clock * 9.0 + id as f32).sin() * cell * 0.06;
            let c = at * cell + vec2(cell / 2.0, cell / 2.0 + bob);
            let s = cell * 0.95;
            let col = color(id);
            soft_shadow(c + vec2(0.0, s * 0.95), s * 1.0, s * 0.35, 0.8);
            let lean = match p.dir {
                Dir::Left => -0.18,
                Dir::Right => 0.18,
                _ => 0.0,
            };
            art::sprite_at(
                HEADS[id % HEADS.len()],
                c,
                s * 2.0,
                crate::art::Draw {
                    rotation: lean,
                    ..Default::default()
                },
            );
            let tag = if id == 0 { "YOU" } else { name(id) };
            text_outline(
                tag,
                c.x,
                c.y - s - 16.0,
                if id == 0 { 26.0 } else { 22.0 },
                WHITE,
                darker(col, 0.5),
            );
        }
        self.particles.draw();
        self.popups.draw();
        set_view_camera(view, Vec2::ZERO);
        let pct = self.share(0) * 100.0;
        let f = view.full();
        let bar = Rect::new(f.x - 8.0, f.y - 8.0, f.w + 16.0, 62.0 - f.y + 8.0);
        gradient_rect(
            bar,
            Color::from_rgba(70, 150, 60, 255),
            Color::from_rgba(40, 110, 50, 255),
        );
        draw_rectangle(
            bar.x,
            bar.y + bar.h,
            bar.w,
            4.0,
            Color::from_rgba(25, 70, 35, 255),
        );
        draw_rectangle(
            bar.x,
            bar.y + bar.h - 2.0,
            bar.w,
            2.0,
            Color::from_rgba(150, 220, 110, 255),
        );
        let pill = Rect::new(HUD_LEFT, 4.0, 220.0, 56.0);
        crate::ui::painted_button(pill, "", "blue", 0.0, 1.0);
        score_text(
            &format!("{pct:.1}%"),
            pill.x + pill.w / 2.0,
            32.0,
            34.0,
            self.bump.t,
        );
        let mut x = view.w - 20.0;
        for id in (1..self.players.len()).rev() {
            let s = format!("{:.0}%", self.share(id) * 100.0);
            let w = text_size(&s, 24.0).width + 60.0;
            x -= w + 8.0;
            let chip = Rect::new(x, 12.0, w, 40.0);
            let alive = self.players[id].alive;
            crate::ui::chip_box(chip);
            let tint = if alive {
                WHITE
            } else {
                Color::new(0.5, 0.5, 0.5, 0.6)
            };
            art::sprite_at(
                HEADS[id % HEADS.len()],
                vec2(chip.x + 22.0, chip.y + chip.h / 2.0),
                34.0,
                crate::art::Draw {
                    tint,
                    ..Default::default()
                },
            );
            text_center(
                &s,
                chip.x + 44.0 + (chip.w - 44.0) / 2.0 - 4.0,
                chip.y + chip.h / 2.0 + 2.0,
                24.0,
                crate::ui::INK,
            );
        }
        self.screen_fx.draw();
        match self.state {
            State::Lost => {
                let s = format!("You had {:.1}%   Best {:.1}%", pct, self.best * 100.0);
                overlay(view, "Oops!", &[&s, "Tap to try again"]);
            }
            State::Won => overlay(
                view,
                "You win!",
                &["Half the map is yours!", "Tap to play again"],
            ),
            State::Playing => {}
        }
    }
}

pub fn icon(r: Rect) {
    crate::ui::icon("paper", r);
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
        let have = art::names();
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

    #[test]
    fn every_bot_has_a_name() {
        for id in 1..5 {
            assert!(!name(id).is_empty());
            assert_ne!(name(id), name(0));
        }
    }
}
