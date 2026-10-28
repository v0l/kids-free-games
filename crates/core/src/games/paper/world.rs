use super::arena::{self, Tuning, NAMES, PERSONAS};
use crate::brain::{Brain, Effort, Net};
use crate::rng as rand;
use glam::{ivec2, vec2, IVec2, Vec2};
use std::collections::VecDeque;
use std::f32::consts::{FRAC_PI_2, PI, TAU};
use std::sync::OnceLock;

mod sense;
pub use sense::FEATURES;

static SHIPPED: &[u8] = include_bytes!("../../../../../assets/brains/paper.kgb");

pub fn shipped_net() -> Option<&'static Net> {
    static NET: OnceLock<Option<Net>> = OnceLock::new();
    NET.get_or_init(|| Net::from_bytes(SHIPPED).filter(|n| n.inputs() == FEATURES))
        .as_ref()
}

pub type Lesson = (Vec<Vec<f32>>, usize);

pub type Policy<'a> = &'a mut dyn FnMut(usize, &[Vec<f32>]) -> Option<usize>;

pub const SIZE: f32 = 110.0;
pub const RES: i32 = 4;
pub const N: i32 = 440;
pub const UNITS: i32 = N / RES;
pub const CELL: f32 = 1.0 / RES as f32;
pub const START_R: f32 = 3.2;
pub const SUB: f32 = 1.0 / 120.0;
pub const HEAD_HIT: f32 = 0.6;
pub const MAX_PLAYERS: usize = 12;
pub const PLAYER_TURN: f32 = 6.5;
pub const BOT_TURN: f32 = 5.0;
pub const SELF_GAP: f32 = 0.9;
pub const LOOK: f32 = 1.8;
pub const WALL: f32 = 0.35;
const TRAIL_STEP: f32 = 0.2;
const STAMP: i32 = 1;
const SWEEP_STEP: f32 = 0.0055;
const SWEEP_MAX: f32 = 0.7;
const THINK: f32 = 0.16;
const PROBE_STEP: f32 = 0.2;

pub fn block_of(i: usize) -> usize {
    let (x, y) = (i % N as usize, i / N as usize);
    (y / BLOCK).min(BLOCKS - 1) * BLOCKS + (x / BLOCK).min(BLOCKS - 1)
}

pub fn idx(c: IVec2) -> usize {
    (c.y * N + c.x) as usize
}

pub fn cell_of(i: usize) -> IVec2 {
    ivec2(i as i32 % N, i as i32 / N)
}

pub fn in_grid(c: IVec2) -> bool {
    c.x >= 0 && c.y >= 0 && c.x < N && c.y < N
}

pub fn cell_at(p: Vec2) -> IVec2 {
    (p * RES as f32)
        .floor()
        .as_ivec2()
        .clamp(IVec2::ZERO, IVec2::splat(N - 1))
}

pub fn cell_center(c: IVec2) -> Vec2 {
    (c.as_vec2() + 0.5) * CELL
}

pub fn wrap_angle(a: f32) -> f32 {
    (a + PI).rem_euclid(TAU) - PI
}

pub fn dir_of(a: f32) -> Vec2 {
    vec2(a.cos(), a.sin())
}

pub fn angle_of(v: Vec2) -> f32 {
    v.y.atan2(v.x)
}

fn inside(p: Vec2, pad: f32) -> bool {
    p.x >= pad && p.y >= pad && p.x <= SIZE - pad && p.y <= SIZE - pad
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Cause {
    Trail,
    OwnTrail,
    HeadOn,
    Land,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Death {
    pub victim: usize,
    pub killer: Option<usize>,
    pub at: Vec2,
    pub cause: Cause,
    pub name: &'static str,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Claim {
    pub id: usize,
    pub cells: usize,
    pub at: Vec2,
    pub taken: usize,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Plan {
    Leave(f32),
    Out {
        to: Vec2,
        dir: f32,
        turns: u8,
        side: f32,
    },
    Home,
    Hunt,
}

#[derive(Clone, Debug)]
pub struct Player {
    pub name: &'static str,
    pub bot: bool,
    pub persona: usize,
    pub tune: Tuning,
    pub speed: f32,
    pub turn: f32,
    pub pos: Vec2,
    pub angle: f32,
    pub want: f32,
    pub spin: f32,
    pub trail: Vec<Vec2>,
    pub cells: Vec<usize>,
    pub odo: f32,
    pub alive: bool,
    pub respawn: f32,
    pub kills: u32,
    pub age: f32,
    pub peak: f32,
    think: f32,
    plan: Plan,
    pub brain: Option<Brain>,
    pub rl: Option<Effort>,
    pub manual: bool,
}

impl Player {
    fn new(
        name: &'static str,
        bot: bool,
        persona: usize,
        tune: Tuning,
        at: Vec2,
        angle: f32,
    ) -> Self {
        Self {
            name,
            bot,
            persona,
            tune,
            speed: if bot { tune.speed } else { arena::PLAYER_SPEED },
            turn: if bot { BOT_TURN } else { PLAYER_TURN },
            pos: at,
            angle,
            want: angle,
            spin: 0.0,
            trail: vec![],
            cells: vec![],
            odo: 0.0,
            alive: true,
            respawn: 0.0,
            kills: 0,
            age: 0.0,
            peak: 0.0,
            think: rand::gen_range(0.0, THINK),
            plan: Plan::Home,
            brain: None,
            rl: None,
            manual: false,
        }
    }

    pub fn look(&self) -> Vec2 {
        self.pos
    }

    pub fn heading(&self) -> Vec2 {
        dir_of(self.angle)
    }

    pub fn out(&self) -> bool {
        !self.cells.is_empty()
    }

    pub fn trail_points(&self) -> Vec<Vec2> {
        if self.trail.is_empty() {
            return vec![];
        }
        let mut pts = self.trail.clone();
        pts.push(self.pos);
        pts
    }
}

pub const BLOCKS: usize = 16;
pub const BLOCK: usize = N as usize / BLOCKS;

pub struct World {
    pub owner: Vec<Option<u8>>,
    pub trail: Vec<Option<u8>>,
    pub block_owner: Vec<[u16; MAX_PLAYERS]>,
    pub block_trail: Vec<u16>,
    pub laid: Vec<f32>,
    pub prev: Vec<Option<u8>>,
    pub changed: Vec<f32>,
    pub counts: Vec<u32>,
    pub players: Vec<Player>,
    pub deaths: Vec<Death>,
    pub claims: Vec<Claim>,
    pub clock: f32,
    pub skill: f32,
    pub brains: bool,
    pub effort: Option<f32>,
    pub lessons: Option<Vec<Lesson>>,
    acc: f32,
    name_turn: usize,
}

impl Default for World {
    fn default() -> Self {
        Self::new(0.0)
    }
}

impl World {
    pub fn new(skill: f32) -> Self {
        let n = (N * N) as usize;
        Self {
            owner: vec![None; n],
            trail: vec![None; n],
            block_owner: vec![[0; MAX_PLAYERS]; BLOCKS * BLOCKS],
            block_trail: vec![0; BLOCKS * BLOCKS],
            laid: vec![0.0; n],
            prev: vec![None; n],
            changed: vec![-10.0; n],
            counts: vec![0; MAX_PLAYERS],
            players: vec![],
            deaths: vec![],
            claims: vec![],
            clock: 0.0,
            skill,
            brains: true,
            effort: None,
            lessons: None,
            acc: 0.0,
            name_turn: rand::gen_range(0, NAMES.len()),
        }
    }

    pub fn total(&self) -> usize {
        self.owner.len()
    }

    pub fn share(&self, id: usize) -> f32 {
        self.counts.get(id).copied().unwrap_or(0) as f32 / self.total() as f32
    }

    pub fn shown(&self, i: usize) -> Option<u8> {
        if self.clock < self.changed[i] {
            self.prev[i]
        } else {
            self.owner[i]
        }
    }

    pub fn age(&self, i: usize) -> f32 {
        self.clock - self.changed[i]
    }

    pub fn owner_at(&self, p: Vec2) -> Option<u8> {
        self.owner[idx(cell_at(p))]
    }

    fn set_owner(&mut self, i: usize, to: Option<u8>, delay: f32) {
        let was = self.owner[i];
        if was == to {
            return;
        }
        self.prev[i] = self.shown(i);
        self.changed[i] = self.clock + delay;
        let b = block_of(i);
        if let Some(w) = was {
            self.counts[w as usize] -= 1;
            self.block_owner[b][w as usize] -= 1;
        }
        if let Some(t) = to {
            self.counts[t as usize] += 1;
            self.block_owner[b][t as usize] += 1;
        }
        self.owner[i] = to;
    }

    pub fn nearest_own(&self, id: usize) -> Option<Vec2> {
        let p = &self.players[id];
        let me = Some(id as u8);
        let span = SIZE / BLOCKS as f32;
        let here = (p.pos / span).floor().as_ivec2();
        let mut best: Option<(f32, Vec2)> = None;
        for by in 0..BLOCKS as i32 {
            for bx in 0..BLOCKS as i32 {
                if self.block_owner[by as usize * BLOCKS + bx as usize][id] == 0 {
                    continue;
                }
                let lo = (ivec2(bx, by) - here).abs().max(IVec2::ZERO).as_vec2() * span;
                if best.is_some_and(|b| (lo.length() - span * 1.5) > b.0) {
                    continue;
                }
                let x0 = bx as usize * BLOCK;
                let y0 = by as usize * BLOCK;
                for y in (y0..y0 + BLOCK).step_by(2) {
                    for x in (x0..x0 + BLOCK).step_by(2) {
                        if self.owner[y * N as usize + x] != me {
                            continue;
                        }
                        let c = cell_center(ivec2(x as i32, y as i32));
                        let d = c.distance(p.pos);
                        if best.is_none_or(|b| d < b.0) {
                            best = Some((d, c));
                        }
                    }
                }
            }
        }
        best.map(|b| b.1)
    }

    pub fn claim(&mut self, i: usize, to: Option<u8>) {
        self.set_owner(i, to, 0.0);
    }

    fn set_trail(&mut self, i: usize, to: Option<u8>) {
        let was = self.trail[i];
        if was.is_some() == to.is_some() {
            self.trail[i] = to;
            return;
        }
        let b = block_of(i);
        if was.is_some() {
            self.block_trail[b] -= 1;
        } else {
            self.block_trail[b] += 1;
        }
        self.trail[i] = to;
    }

    pub fn add_player(&mut self, at: Vec2, angle: f32, bot: bool) -> usize {
        let id = self.players.len();
        let persona = id % PERSONAS.len();
        let name = if bot { self.fresh_name() } else { "You" };
        let tune = arena::tuning(PERSONAS[persona], self.skill);
        let mut p = Player::new(name, bot, persona, tune, at, angle);
        p.brain = self.brain_for(bot, tune);
        self.players.push(p);
        self.stake(id, at);
        id
    }

    fn brain_for(&self, bot: bool, tune: Tuning) -> Option<Brain> {
        if !bot || !self.brains {
            return None;
        }
        shipped_net()
            .map(|n| Brain::new(n.clone(), Effort::new(self.effort.unwrap_or(tune.effort))))
    }

    pub fn fresh_name(&mut self) -> &'static str {
        for _ in 0..NAMES.len() {
            self.name_turn = (self.name_turn + 7) % NAMES.len();
            let n = NAMES[self.name_turn];
            if !self.players.iter().any(|p| p.alive && p.name == n) {
                return n;
            }
        }
        NAMES[self.name_turn]
    }

    fn disc(at: Vec2, r: f32) -> impl Iterator<Item = IVec2> {
        let lo = cell_at(at - Vec2::splat(r));
        let hi = cell_at(at + Vec2::splat(r));
        (lo.y..=hi.y)
            .flat_map(move |y| (lo.x..=hi.x).map(move |x| ivec2(x, y)))
            .filter(move |c| cell_center(*c).distance(at) <= r)
    }

    pub fn stake(&mut self, id: usize, at: Vec2) {
        for c in Self::disc(at, START_R).collect::<Vec<_>>() {
            let i = idx(c);
            if let Some(t) = self.trail[i] {
                self.kill(t as usize, None, Cause::Trail);
            }
            let d = cell_center(c).distance(at);
            self.set_owner(i, Some(id as u8), d * 0.05);
        }
    }

    pub fn free_spot(&self, tries: usize) -> Option<Vec2> {
        let r = START_R + 2.0;
        for _ in 0..tries {
            let at = vec2(rand::gen_range(r, SIZE - r), rand::gen_range(r, SIZE - r));
            let clear = (0..=8).all(|k| {
                let ring = r * k as f32 / 8.0;
                (0..12).all(|s| {
                    let p = at + dir_of(s as f32 / 12.0 * TAU) * ring;
                    let i = idx(cell_at(p));
                    self.owner[i].is_none() && self.trail[i].is_none()
                })
            });
            let far = self
                .players
                .iter()
                .filter(|p| p.alive)
                .all(|p| p.pos.distance(at) > 10.0);
            if clear && far {
                return Some(at);
            }
        }
        None
    }

    pub fn respawn(&mut self, id: usize, at: Vec2) {
        let angle = rand::gen_range(-PI, PI);
        let p = &self.players[id];
        let (bot, persona) = (p.bot, p.persona);
        let name = if bot { self.fresh_name() } else { "You" };
        let persona = if bot {
            (persona + 1 + rand::gen_range(0, PERSONAS.len() - 1)) % PERSONAS.len()
        } else {
            persona
        };
        let tune = arena::tuning(PERSONAS[persona], self.skill);
        let mut p = Player::new(name, bot, persona, tune, at, angle);
        p.brain = self.brain_for(bot, tune);
        self.players[id] = p;
        self.stake(id, at);
    }

    fn clear_trail(&mut self, id: usize) {
        let me = Some(id as u8);
        let p = &mut self.players[id];
        p.trail.clear();
        p.odo = 0.0;
        for i in std::mem::take(&mut p.cells) {
            if self.trail[i] == me {
                self.set_trail(i, None);
            }
        }
    }

    pub fn kill(&mut self, victim: usize, killer: Option<usize>, cause: Cause) {
        if !self.players[victim].alive {
            return;
        }
        let at = self.players[victim].pos;
        let name = self.players[victim].name;
        let p = &mut self.players[victim];
        p.alive = false;
        p.respawn = rand::gen_range(2.5, 4.0);
        self.clear_trail(victim);
        for i in 0..self.owner.len() {
            if self.owner[i] == Some(victim as u8) {
                let d = cell_center(cell_of(i)).distance(at);
                self.set_owner(i, None, (d * 0.012).min(0.6));
            }
        }
        let killer = killer.filter(|k| *k != victim);
        if let Some(k) = killer {
            self.players[k].kills += 1;
        }
        self.deaths.push(Death {
            victim,
            killer,
            at,
            cause,
            name,
        });
    }

    pub fn close_loop(&mut self, id: usize) {
        let me = Some(id as u8);
        let cells = std::mem::take(&mut self.players[id].cells);
        self.players[id].trail.clear();
        self.players[id].odo = 0.0;
        let mut ring = vec![];
        for &i in &cells {
            if self.trail[i] == me {
                self.set_trail(i, None);
                ring.push(i);
            }
            self.set_owner(i, me, 0.0);
        }
        let (mut lo, mut hi) = (ivec2(N, N), ivec2(-1, -1));
        for (i, o) in self.owner.iter().enumerate() {
            if *o == me {
                let c = cell_of(i);
                lo = lo.min(c);
                hi = hi.max(c);
            }
        }
        let lo = lo - 1;
        let hi = hi + 1;
        let w = (hi.x - lo.x + 1) as usize;
        let h = (hi.y - lo.y + 1) as usize;
        let local = |c: IVec2| ((c.y - lo.y) as usize) * w + (c.x - lo.x) as usize;
        let within = |c: IVec2| c.x >= lo.x && c.y >= lo.y && c.x <= hi.x && c.y <= hi.y;
        let mine = |c: IVec2| in_grid(c) && self.owner[idx(c)] == me;
        let steps = [ivec2(1, 0), ivec2(-1, 0), ivec2(0, 1), ivec2(0, -1)];
        let mut outside = vec![false; w * h];
        let mut q = VecDeque::new();
        for y in lo.y..=hi.y {
            for x in lo.x..=hi.x {
                let c = ivec2(x, y);
                let edge = x == lo.x || y == lo.y || x == hi.x || y == hi.y;
                if edge && !mine(c) {
                    outside[local(c)] = true;
                    q.push_back(c);
                }
            }
        }
        while let Some(c) = q.pop_front() {
            for s in steps {
                let n = c + s;
                if within(n) && !outside[local(n)] && !mine(n) {
                    outside[local(n)] = true;
                    q.push_back(n);
                }
            }
        }
        const UNSET: u16 = u16::MAX - 1;
        let mut inside = vec![u16::MAX; w * h];
        let mut filled = vec![];
        for y in lo.y..=hi.y {
            for x in lo.x..=hi.x {
                let c = ivec2(x, y);
                if in_grid(c) && !outside[local(c)] && !mine(c) {
                    inside[local(c)] = UNSET;
                    filled.push(c);
                }
            }
        }
        let mut q: VecDeque<(IVec2, u16)> = ring.iter().map(|i| (cell_of(*i), 0)).collect();
        while let Some((c, dist)) = q.pop_front() {
            for s in steps {
                let n = c + s;
                if within(n) && inside[local(n)] == UNSET {
                    inside[local(n)] = dist + 1;
                    q.push_back((n, dist + 1));
                }
            }
        }
        let mut taken = 0;
        for c in &filled {
            let i = idx(*c);
            if self.owner[i].is_some() {
                taken += 1;
            }
            if let Some(t) = self.trail[i] {
                let t = t as usize;
                if t != id && self.players[t].alive {
                    self.kill(t, Some(id), Cause::Trail);
                }
            }
            let dist = inside[local(*c)];
            let delay = if dist >= UNSET {
                SWEEP_MAX * 0.5
            } else {
                (dist as f32 * SWEEP_STEP).min(SWEEP_MAX)
            };
            self.set_owner(i, me, delay);
        }
        self.claims.push(Claim {
            id,
            cells: filled.len() + cells.len(),
            at: self.players[id].pos,
            taken,
        });
        for o in 0..self.players.len() {
            if o != id && self.players[o].alive && self.counts[o] == 0 {
                self.kill(o, Some(id), Cause::Land);
            }
        }
    }

    pub fn advance(&mut self, dt: f32) {
        self.acc += dt;
        while self.acc >= SUB {
            self.acc -= SUB;
            self.substep();
        }
    }

    pub fn advance_with(&mut self, dt: f32, policy: Policy) {
        self.acc += dt;
        while self.acc >= SUB {
            self.acc -= SUB;
            self.step_with(policy);
        }
    }

    pub fn substep(&mut self) {
        self.step_with(&mut |_, _| None);
    }

    pub fn step_with(&mut self, policy: Policy) {
        self.clock += SUB;
        for id in 0..self.players.len() {
            if !self.players[id].alive {
                continue;
            }
            if let Some(effort) = self.players[id].rl.filter(|_| !self.players[id].manual) {
                self.players[id].think -= SUB;
                if self.players[id].think <= 0.0 {
                    self.players[id].think += effort.think_every(super::rl::DECIDE);
                    self.rl_think(id, effort);
                }
            } else if self.players[id].bot && !self.players[id].manual {
                self.players[id].think -= SUB;
                if self.players[id].think <= 0.0 {
                    let every = self.players[id]
                        .brain
                        .as_ref()
                        .map_or(THINK, |b| b.effort.think_every(THINK));
                    self.players[id].think += every;
                    self.bot_think(id, policy);
                }
            }
            self.move_player(id);
        }
        self.head_ons();
        for id in 0..self.players.len() {
            if self.players[id].alive {
                self.touch(id);
            }
        }
        for id in 0..self.players.len() {
            let p = &mut self.players[id];
            if p.alive {
                p.peak = p.peak.max(self.counts[id] as f32 / self.owner.len() as f32);
            }
        }
    }

    fn move_player(&mut self, id: usize) {
        let p = &mut self.players[id];
        p.age += SUB;
        let limit = p.turn * SUB;
        let turn = wrap_angle(p.want - p.angle).clamp(-limit, limit);
        p.angle = wrap_angle(p.angle + turn);
        p.spin = turn / SUB;
        let step = p.speed * SUB;
        if step <= 0.0 {
            return;
        }
        let mut d = p.heading();
        let next = p.pos + d * step;
        let (lo, hi) = (WALL, SIZE - WALL);
        let off_x = next.x < lo || next.x > hi;
        let off_y = next.y < lo || next.y > hi;
        if off_x || off_y {
            let mut slide = vec2(if off_x { 0.0 } else { d.x }, if off_y { 0.0 } else { d.y });
            if slide.length_squared() < 0.01 {
                let to_mid = Vec2::splat(SIZE / 2.0) - p.pos;
                slide = if off_x {
                    vec2(0.0, to_mid.y.signum())
                } else {
                    vec2(to_mid.x.signum(), 0.0)
                };
            }
            d = slide.normalize();
            p.angle = angle_of(d);
            if p.bot {
                p.want = p.angle;
            }
        }
        p.pos = (p.pos + d * step).clamp(Vec2::splat(lo), Vec2::splat(hi));
        if p.out() {
            p.odo += step;
            if p.trail
                .last()
                .is_none_or(|l| l.distance(p.pos) >= TRAIL_STEP)
            {
                p.trail.push(p.pos);
            }
        }
    }

    fn head_ons(&mut self) {
        let n = self.players.len();
        for a in 0..n {
            for b in (a + 1)..n {
                let (pa, pb) = (&self.players[a], &self.players[b]);
                if !pa.alive || !pb.alive || pa.pos.distance(pb.pos) >= HEAD_HIT {
                    continue;
                }
                let (la, lb) = (pa.odo, pb.odo);
                if la <= lb {
                    self.kill(a, Some(b), Cause::HeadOn);
                }
                if lb <= la {
                    self.kill(b, Some(a), Cause::HeadOn);
                }
            }
        }
    }

    fn touch(&mut self, id: usize) {
        let me = id as u8;
        let pos = self.players[id].pos;
        let c = cell_at(pos);
        for dy in -STAMP..=STAMP {
            for dx in -STAMP..=STAMP {
                let q = c + ivec2(dx, dy);
                if !in_grid(q) {
                    continue;
                }
                if let Some(o) = self.trail[idx(q)] {
                    if o != me {
                        self.kill(o as usize, Some(id), Cause::Trail);
                    }
                }
            }
        }
        let i = idx(c);
        let odo = self.players[id].odo;
        if self.trail[i] == Some(me) && self.laid[i] < odo - SELF_GAP {
            self.kill(id, None, Cause::OwnTrail);
            return;
        }
        if self.owner[i] == Some(me) {
            if self.players[id].out() {
                self.close_loop(id);
            }
            return;
        }
        if !self.players[id].out() {
            let p = &mut self.players[id];
            let back = pos - p.heading() * p.speed * SUB;
            p.trail = vec![back];
            p.odo = 0.0;
        }
        for dy in -STAMP..=STAMP {
            for dx in -STAMP..=STAMP {
                let q = c + ivec2(dx, dy);
                if !in_grid(q) || dx * dx + dy * dy > STAMP * STAMP {
                    continue;
                }
                let j = idx(q);
                if self.owner[j] == Some(me) || self.trail[j] == Some(me) {
                    continue;
                }
                self.set_trail(j, Some(me));
                self.laid[j] = odo;
                self.players[id].cells.push(j);
            }
        }
    }

    pub fn respawn_bots(&mut self, dt: f32) {
        for id in 0..self.players.len() {
            let p = &mut self.players[id];
            if !p.bot || p.alive {
                continue;
            }
            p.respawn -= dt;
            if p.respawn > 0.0 {
                continue;
            }
            match self.free_spot(30) {
                Some(at) => self.respawn(id, at),
                None => self.players[id].respawn = 0.5,
            }
        }
    }

    pub fn ranking(&self) -> Vec<usize> {
        let mut ids: Vec<usize> = (0..self.players.len())
            .filter(|i| self.players[*i].alive)
            .collect();
        ids.sort_by(|a, b| self.counts[*b].cmp(&self.counts[*a]).then(a.cmp(b)));
        ids
    }
}

impl World {
    fn unit_blocked(&self, id: usize, u: IVec2) -> bool {
        let me = Some(id as u8);
        let fresh = self.players[id].odo - 1.0;
        (0..RES).any(|y| {
            (0..RES).any(|x| {
                let i = idx(u * RES + ivec2(x, y));
                self.trail[i] == me && self.laid[i] < fresh
            })
        })
    }

    fn route(&self, id: usize, goal: impl Fn(IVec2) -> bool, budget: usize) -> Option<(f32, f32)> {
        let p = &self.players[id];
        let unit = |v: Vec2| {
            v.floor()
                .as_ivec2()
                .clamp(IVec2::ZERO, IVec2::splat(UNITS - 1))
        };
        let start = unit(p.pos);
        thread_local! {
            static SCRATCH: std::cell::RefCell<(Vec<i32>, Vec<u16>, Vec<usize>)> =
                const { std::cell::RefCell::new((Vec::new(), Vec::new(), Vec::new())) };
        }
        SCRATCH.with(|scratch| {
            let mut scratch = scratch.borrow_mut();
            let (parent, depth, touched) = &mut *scratch;
            if parent.len() != (UNITS * UNITS) as usize {
                *parent = vec![-1; (UNITS * UNITS) as usize];
                *depth = vec![0; (UNITS * UNITS) as usize];
            }
            let out = self.route_in(id, &goal, budget, start, parent, depth, touched);
            for i in touched.drain(..) {
                parent[i] = -1;
                depth[i] = 0;
            }
            out
        })
    }

    #[allow(clippy::too_many_arguments)]
    fn route_in(
        &self,
        id: usize,
        goal: &impl Fn(IVec2) -> bool,
        budget: usize,
        start: IVec2,
        parent: &mut [i32],
        depth: &mut [u16],
        touched: &mut Vec<usize>,
    ) -> Option<(f32, f32)> {
        let p = &self.players[id];
        let at = |u: IVec2| (u.y * UNITS + u.x) as usize;
        parent[at(start)] = at(start) as i32;
        touched.push(at(start));
        let mut q = VecDeque::from([start]);
        let heading = p.heading();
        let steps = [ivec2(1, 0), ivec2(-1, 0), ivec2(0, 1), ivec2(0, -1)];
        let mut visited = 0;
        while let Some(c) = q.pop_front() {
            if c != start && goal(c) {
                let mut node = c;
                while depth[at(node)] > 2 {
                    let up = parent[at(node)] as usize;
                    node = ivec2(up as i32 % UNITS, up as i32 / UNITS);
                }
                let target = node.as_vec2() + 0.5;
                return Some((angle_of(target - p.pos), depth[at(c)] as f32));
            }
            visited += 1;
            if visited > budget {
                return None;
            }
            for s in steps {
                if c == start && s.as_vec2().dot(heading) < -0.5 {
                    continue;
                }
                let n = c + s;
                if n.x < 0 || n.y < 0 || n.x >= UNITS || n.y >= UNITS || parent[at(n)] >= 0 {
                    continue;
                }
                if self.unit_blocked(id, n) {
                    continue;
                }
                parent[at(n)] = at(c) as i32;
                depth[at(n)] = depth[at(c)] + 1;
                touched.push(at(n));
                q.push_back(n);
            }
        }
        None
    }

    pub fn way_home(&self, id: usize) -> Option<(f32, f32)> {
        let me = Some(id as u8);
        self.route(
            id,
            |u| self.owner[idx(u * RES + IVec2::splat(RES / 2))] == me,
            6000,
        )
    }

    fn gentle_to(&self, id: usize, other: usize) -> bool {
        other == 0
            && !self.players[0].bot
            && !arena::hunts_player(self.skill, self.players[0].age)
            && id != 0
    }

    pub fn nearest_head(&self, id: usize, at: Vec2) -> f32 {
        self.players
            .iter()
            .enumerate()
            .filter(|(o, p)| *o != id && p.alive)
            .map(|(_, p)| p.pos.distance(at))
            .fold(f32::MAX, f32::min)
    }

    pub fn probe(&self, id: usize, aim: f32, avoid_trails: bool) -> f32 {
        let p = &self.players[id];
        let me = Some(id as u8);
        let mut a = p.angle;
        let mut pos = p.pos;
        let mut odo = p.odo;
        let dt = PROBE_STEP / p.speed.max(0.5);
        let steps = (LOOK / PROBE_STEP) as usize;
        for k in 1..=steps {
            let limit = p.turn * dt;
            a += wrap_angle(aim - a).clamp(-limit, limit);
            pos += dir_of(a) * PROBE_STEP;
            odo += PROBE_STEP;
            let gone = (k - 1) as f32 * PROBE_STEP;
            if !inside(pos, WALL + 0.3) {
                return gone;
            }
            let i = idx(cell_at(pos));
            if p.out() && self.owner[i] == me {
                return LOOK;
            }
            match self.trail[i] {
                Some(o) if Some(o) == me => {
                    if self.laid[i] < odo - SELF_GAP {
                        return gone;
                    }
                }
                Some(o) if avoid_trails || self.gentle_to(id, o as usize) => return gone,
                _ => {}
            }
            if k <= 6 {
                for (o, q) in self.players.iter().enumerate() {
                    if o == id || !q.alive || q.pos.distance(pos) > 1.4 {
                        continue;
                    }
                    if self.gentle_to(id, o) || q.odo >= p.odo {
                        return gone;
                    }
                }
            }
        }
        LOOK
    }

    pub fn safe_aim(&self, id: usize, want: f32, avoid_trails: bool) -> f32 {
        let mut best = (want, -1.0);
        for k in 0..13 {
            let off = ((k + 1) / 2) as f32 * if k % 2 == 0 { 1.0 } else { -1.0 } * 0.4;
            let a = wrap_angle(want + off);
            let s = self.probe(id, a, avoid_trails);
            if s >= LOOK {
                return a;
            }
            if s > best.1 {
                best = (a, s);
            }
        }
        best.0
    }

    pub fn open_ahead(&self, id: usize, angle: f32) -> i32 {
        let p = &self.players[id];
        let me = Some(id as u8);
        let d = dir_of(angle);
        let mut score = 0;
        for k in 1..=14 {
            let at = p.pos + d * (k as f32 * 0.5);
            if !inside(at, 1.0) {
                return score - 20 + k;
            }
            let i = idx(cell_at(at));
            if self.trail[i].is_some() {
                return score - 5;
            }
            if self.owner[i] != me {
                score += 1;
            }
        }
        score
    }

    fn leg(&self, id: usize) -> f32 {
        let reach = self.players[id].tune.reach as f32;
        rand::gen_range((reach / 4.0).max(2.0), (reach / 3.0).max(3.0) + 1.0)
    }

    fn prey(&self, id: usize) -> Option<(f32, f32)> {
        let me = &self.players[id];
        let mut best: Option<(f32, f32)> = None;
        for (o, q) in self.players.iter().enumerate() {
            let eligible = o != id
                && q.alive
                && (o != 0 || self.players[0].bot || arena::hunts_player(self.skill, q.age));
            if !eligible {
                continue;
            }
            for t in &q.trail {
                let d = t.distance(me.pos);
                if d <= 10.0 && best.is_none_or(|b| d < b.1) {
                    best = Some((angle_of(*t - me.pos), d));
                }
            }
        }
        best
    }

    fn pick_exit(&self, id: usize) -> f32 {
        let a = self.players[id].angle;
        (-3..=3)
            .map(|k| wrap_angle(a + k as f32 * 0.45))
            .max_by_key(|d| self.open_ahead(id, *d) * 4 + rand::gen_range(0, 12))
            .unwrap_or(a)
    }

    fn rl_think(&mut self, id: usize, effort: Effort) {
        let Some(net) = super::rl::shipped() else {
            return;
        };
        let coarse = super::rl::Coarse::of(self);
        let a = net.choose(&super::rl::observe(self, id, &coarse), effort);
        let p = &mut self.players[id];
        p.want = wrap_angle(p.angle + super::rl::TURNS[a]);
    }

    fn bot_think(&mut self, id: usize, policy: Policy) {
        let (aims, feats) = self.options(id);
        let picked = match policy(id, &feats) {
            Some(k) => Some(k),
            None => self.players[id].brain.as_mut().map(|b| b.choose(&feats)),
        };
        let Some(k) = picked else {
            self.plan_think(id);
            if self.lessons.is_some() {
                let want = self.players[id].want;
                let k = (0..aims.len())
                    .min_by(|a, b| {
                        let d = |k: usize| wrap_angle(aims[k] - want).abs();
                        d(*a).total_cmp(&d(*b))
                    })
                    .unwrap_or(0);
                if let Some(l) = self.lessons.as_mut() {
                    l.push((feats, k));
                }
            }
            return;
        };
        let mut aim = aims[k.min(aims.len() - 1)];
        if self.hits_gentle(id, aim) {
            aim = self.safe_aim(id, aim, false);
        }
        self.players[id].want = aim;
    }

    fn plan_think(&mut self, id: usize) {
        let me = Some(id as u8);
        let p = &self.players[id];
        let out = p.out();
        let tune = p.tune;
        let pos = p.pos;
        let mut plan = p.plan;
        let mut want = p.want;
        if !out {
            let home = self.owner_at(pos) == me;
            if !matches!(plan, Plan::Leave(_)) || rand::gen_range(0, 20) == 0 || !home {
                plan = Plan::Leave(self.pick_exit(id));
            }
            if let Plan::Leave(d) = plan {
                want = d;
            }
        } else {
            if matches!(plan, Plan::Leave(_)) {
                let dir = p.angle;
                let to = (pos + dir_of(dir) * self.leg(id))
                    .clamp(Vec2::splat(2.0), Vec2::splat(SIZE - 2.0));
                plan = Plan::Out {
                    to,
                    dir,
                    turns: 0,
                    side: if rand::gen_range(0, 2) == 0 {
                        1.0
                    } else {
                        -1.0
                    },
                };
            }
            let len = p.odo;
            let home = self.way_home(id);
            let home_dist = home.map_or(99.0, |(_, d)| d);
            let threat = self.nearest_head(id, pos);
            if plan != Plan::Home {
                let flee = threat <= home_dist + 4.0 && rand::gen_range(0.0, 1.0) < tune.effort;
                if len >= tune.reach as f32 || home.is_none() || flee {
                    plan = Plan::Home;
                } else if plan != Plan::Hunt
                    && len * 2.0 < tune.reach as f32
                    && rand::gen_range(0.0, 1.0) < tune.effort * 0.15
                    && self.prey(id).is_some()
                {
                    plan = Plan::Hunt;
                }
            }
            let go_home = |w: &World| home.map_or(w.players[id].angle, |(d, _)| d);
            match plan {
                Plan::Out {
                    to,
                    dir,
                    turns,
                    side,
                } => {
                    if pos.distance(to) < 0.9 || (to - pos).dot(dir_of(dir)) < 0.0 {
                        if turns >= 2 {
                            plan = Plan::Home;
                            want = go_home(self);
                        } else {
                            let bend = rand::gen_range(0.75, 1.3) * FRAC_PI_2;
                            let dir = wrap_angle(dir + side * bend);
                            let leg = self.leg(id) + if turns == 0 { 0.0 } else { 2.0 };
                            let to = (to + dir_of(dir) * leg)
                                .clamp(Vec2::splat(2.0), Vec2::splat(SIZE - 2.0));
                            plan = Plan::Out {
                                to,
                                dir,
                                turns: turns + 1,
                                side,
                            };
                            want = angle_of(to - pos);
                        }
                    } else {
                        want = angle_of(to - pos);
                    }
                }
                Plan::Hunt => match self.prey(id) {
                    Some((d, _)) if len < tune.reach as f32 => want = d,
                    _ => {
                        plan = Plan::Home;
                        want = go_home(self);
                    }
                },
                _ => want = go_home(self),
            }
        }
        let hunting = plan == Plan::Hunt;
        let avoid = !hunting && rand::gen_range(0.0, 1.0) < tune.effort;
        let want = self.safe_aim(id, want, avoid);
        let p = &mut self.players[id];
        p.plan = plan;
        p.want = want;
    }
}

pub fn spread_spots(n: usize) -> Vec<Vec2> {
    let mut spots = vec![];
    let ring = |k: usize, total: usize, r: f32| {
        let a = k as f32 / total as f32 * TAU + 0.4;
        Vec2::splat(SIZE / 2.0) + dir_of(a) * r
    };
    let inner = n.min(4);
    for k in 0..inner {
        spots.push(ring(k, inner, SIZE * 0.22));
    }
    for k in 0..n - inner {
        spots.push(ring(k, n - inner, SIZE * 0.4));
    }
    spots
}
