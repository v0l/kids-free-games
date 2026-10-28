use super::arena::{self, Tuning, NAMES, PERSONAS};
use crate::engine::Dir;
use crate::rng as rand;
use glam::{ivec2, vec2, IVec2, Vec2};
use std::collections::VecDeque;

pub const GW: i32 = 110;
pub const GH: i32 = 110;
pub const START_R: f32 = 3.2;
pub const SUB: f32 = 1.0 / 120.0;
pub const HEAD_HIT: f32 = 0.6;
pub const MAX_PLAYERS: usize = 12;
pub const QUEUE: usize = 2;
pub const EARLY_TURN: f32 = 0.22;
const SWEEP_STEP: f32 = 0.022;
const SWEEP_MAX: f32 = 0.7;

pub fn idx(p: IVec2) -> usize {
    (p.y * GW + p.x) as usize
}

pub fn cell_of(i: usize) -> IVec2 {
    ivec2(i as i32 % GW, i as i32 / GW)
}

pub fn in_bounds(p: IVec2) -> bool {
    p.x >= 0 && p.y >= 0 && p.x < GW && p.y < GH
}

pub fn center(c: IVec2) -> Vec2 {
    c.as_vec2() + 0.5
}

pub fn bezier(a: Vec2, b: Vec2, c: Vec2, t: f32) -> Vec2 {
    let u = 1.0 - t;
    a * u * u + b * 2.0 * u * t + c * t * t
}

pub fn turn_left(d: Dir) -> Dir {
    match d {
        Dir::Up => Dir::Left,
        Dir::Left => Dir::Down,
        Dir::Down => Dir::Right,
        Dir::Right => Dir::Up,
    }
}

pub fn turn_right(d: Dir) -> Dir {
    turn_left(d).opposite()
}

fn dvec(d: Dir) -> Vec2 {
    d.delta().as_vec2()
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
    Leave(Dir),
    Out { left: i32, turns: u8, right: bool },
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
    pub cell: IVec2,
    pub dir: Dir,
    pub in_dir: Dir,
    pub progress: f32,
    pub queue: VecDeque<Dir>,
    pub trail: Vec<IVec2>,
    pub trail_from: IVec2,
    pub alive: bool,
    pub respawn: f32,
    pub kills: u32,
    pub age: f32,
    pub peak: f32,
    pub turned_at: f32,
    plan: Plan,
}

impl Player {
    fn new(
        name: &'static str,
        bot: bool,
        persona: usize,
        tune: Tuning,
        at: IVec2,
        dir: Dir,
    ) -> Self {
        Self {
            name,
            bot,
            persona,
            tune,
            speed: tune.speed,
            cell: at,
            dir,
            in_dir: dir,
            progress: 0.0,
            queue: VecDeque::new(),
            trail: vec![],
            trail_from: at,
            alive: true,
            respawn: 0.0,
            kills: 0,
            age: 0.0,
            peak: 0.0,
            turned_at: -10.0,
            plan: Plan::Home,
        }
    }

    pub fn pos(&self) -> Vec2 {
        center(self.cell) + dvec(self.dir) * self.progress
    }

    pub fn look(&self) -> Vec2 {
        let c = center(self.cell);
        bezier(
            c - dvec(self.in_dir) * 0.5,
            c,
            c + dvec(self.dir) * 0.5,
            self.progress,
        )
    }

    pub fn heading(&self) -> Vec2 {
        let t = self.progress;
        let a = dvec(self.in_dir);
        let b = dvec(self.dir);
        (a * (1.0 - t) + b * t).normalize_or(b)
    }

    pub fn trail_points(&self) -> Vec<Vec2> {
        if self.trail.is_empty() {
            return vec![];
        }
        let mut cells = Vec::with_capacity(self.trail.len() + 1);
        cells.push(center(self.trail_from));
        cells.extend(self.trail.iter().map(|c| center(*c)));
        let mid = |k: usize| (cells[k] + cells[k + 1]) * 0.5;
        let mut out = vec![mid(0)];
        for (k, &b) in cells.iter().enumerate().take(cells.len() - 1).skip(1) {
            let (a, c) = (mid(k - 1), mid(k));
            if (b - a).perp_dot(c - b).abs() < 1e-4 {
                out.push(c);
            } else {
                for s in 1..=5 {
                    out.push(bezier(a, b, c, s as f32 / 5.0));
                }
            }
        }
        let n = cells.len() - 1;
        let a = if n >= 1 { mid(n - 1) } else { cells[n] };
        let b = cells[n];
        let c = b + dvec(self.dir) * 0.5;
        for s in 1..=4 {
            out.push(bezier(a, b, c, self.progress * s as f32 / 4.0));
        }
        out
    }

    pub fn wants(&mut self, d: Dir) {
        let last = self.queue.back().copied().unwrap_or(self.dir);
        if d == last || d == last.opposite() {
            return;
        }
        if self.queue.len() >= QUEUE {
            self.queue.pop_back();
        }
        self.queue.push_back(d);
    }
}

pub struct World {
    pub owner: Vec<Option<u8>>,
    pub trail: Vec<Option<u8>>,
    pub prev: Vec<Option<u8>>,
    pub changed: Vec<f32>,
    pub counts: Vec<u32>,
    pub players: Vec<Player>,
    pub deaths: Vec<Death>,
    pub claims: Vec<Claim>,
    pub clock: f32,
    pub skill: f32,
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
        let n = (GW * GH) as usize;
        Self {
            owner: vec![None; n],
            trail: vec![None; n],
            prev: vec![None; n],
            changed: vec![-10.0; n],
            counts: vec![0; MAX_PLAYERS],
            players: vec![],
            deaths: vec![],
            claims: vec![],
            clock: 0.0,
            skill,
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

    fn set_owner(&mut self, i: usize, to: Option<u8>, delay: f32) {
        let was = self.owner[i];
        if was == to {
            return;
        }
        self.prev[i] = self.shown(i);
        self.changed[i] = self.clock + delay;
        if let Some(w) = was {
            self.counts[w as usize] -= 1;
        }
        if let Some(t) = to {
            self.counts[t as usize] += 1;
        }
        self.owner[i] = to;
    }

    pub fn add_player(&mut self, at: IVec2, dir: Dir, bot: bool) -> usize {
        let id = self.players.len();
        let persona = id % PERSONAS.len();
        let name = if bot { self.fresh_name() } else { "You" };
        let tune = arena::tuning(PERSONAS[persona], self.skill);
        let mut p = Player::new(name, bot, persona, tune, at, dir);
        if !bot {
            p.speed = arena::PLAYER_SPEED;
        }
        self.players.push(p);
        self.stake(id, at);
        id
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

    pub fn stake(&mut self, id: usize, at: IVec2) {
        let r = START_R.ceil() as i32;
        let c = center(at);
        for dy in -r..=r {
            for dx in -r..=r {
                let p = at + ivec2(dx, dy);
                if in_bounds(p) && center(p).distance(c) <= START_R {
                    if let Some(t) = self.trail[idx(p)] {
                        self.kill(t as usize, None, Cause::Trail);
                    }
                    self.set_owner(idx(p), Some(id as u8), dx.abs().max(dy.abs()) as f32 * 0.05);
                }
            }
        }
    }

    pub fn free_spot(&self, tries: usize) -> Option<IVec2> {
        let r = START_R.ceil() as i32 + 2;
        for _ in 0..tries {
            let at = ivec2(rand::gen_range(r, GW - r), rand::gen_range(r, GH - r));
            let clear = (-r..=r).all(|dy| {
                (-r..=r).all(|dx| {
                    let i = idx(at + ivec2(dx, dy));
                    self.owner[i].is_none() && self.trail[i].is_none()
                })
            });
            let far = self
                .players
                .iter()
                .filter(|p| p.alive)
                .all(|p| p.pos().distance(center(at)) > 10.0);
            if clear && far {
                return Some(at);
            }
        }
        None
    }

    pub fn respawn(&mut self, id: usize, at: IVec2) {
        let dir = Dir::ALL[rand::gen_range(0, 4)];
        let p = &self.players[id];
        let (bot, persona) = (p.bot, p.persona);
        let name = if bot { self.fresh_name() } else { "You" };
        let persona = if bot {
            (persona + 1 + rand::gen_range(0, PERSONAS.len() - 1)) % PERSONAS.len()
        } else {
            persona
        };
        let tune = arena::tuning(PERSONAS[persona], self.skill);
        let mut p = Player::new(name, bot, persona, tune, at, dir);
        if !bot {
            p.speed = arena::PLAYER_SPEED;
        }
        self.players[id] = p;
        self.stake(id, at);
    }

    pub fn kill(&mut self, victim: usize, killer: Option<usize>, cause: Cause) {
        if !self.players[victim].alive {
            return;
        }
        let at = self.players[victim].look();
        let name = self.players[victim].name;
        let p = &mut self.players[victim];
        p.alive = false;
        p.respawn = rand::gen_range(2.5, 4.0);
        p.queue.clear();
        for t in std::mem::take(&mut p.trail) {
            if self.trail[idx(t)] == Some(victim as u8) {
                self.trail[idx(t)] = None;
            }
        }
        for i in 0..self.owner.len() {
            if self.owner[i] == Some(victim as u8) {
                let d = center(cell_of(i)).distance(at);
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
        let trail = std::mem::take(&mut self.players[id].trail);
        for t in &trail {
            self.trail[idx(*t)] = None;
            self.set_owner(idx(*t), me, 0.0);
        }
        let (mut lo, mut hi) = (ivec2(GW, GH), ivec2(-1, -1));
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
        let mine = |c: IVec2| in_bounds(c) && self.owner[idx(c)] == me;
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
            for d in Dir::ALL {
                let n = c + d.delta();
                if n.x < lo.x || n.y < lo.y || n.x > hi.x || n.y > hi.y {
                    continue;
                }
                if !outside[local(n)] && !mine(n) {
                    outside[local(n)] = true;
                    q.push_back(n);
                }
            }
        }
        let mut inside = vec![u16::MAX; w * h];
        let mut cells = vec![];
        for y in lo.y..=hi.y {
            for x in lo.x..=hi.x {
                let c = ivec2(x, y);
                if in_bounds(c) && !outside[local(c)] && !mine(c) {
                    inside[local(c)] = u16::MAX - 1;
                    cells.push(c);
                }
            }
        }
        let mut q: VecDeque<(IVec2, u16)> = trail.iter().map(|t| (*t, 0)).collect();
        while let Some((c, dist)) = q.pop_front() {
            for d in Dir::ALL {
                let n = c + d.delta();
                if n.x < lo.x || n.y < lo.y || n.x > hi.x || n.y > hi.y {
                    continue;
                }
                if inside[local(n)] == u16::MAX - 1 {
                    inside[local(n)] = dist + 1;
                    q.push_back((n, dist + 1));
                }
            }
        }
        let mut taken = 0;
        for c in &cells {
            let i = idx(*c);
            if self.owner[i].is_some() {
                taken += 1;
            }
            let dist = inside[local(*c)];
            let delay = if dist >= u16::MAX - 1 {
                SWEEP_MAX * 0.5
            } else {
                (dist as f32 * SWEEP_STEP).min(SWEEP_MAX)
            };
            self.set_owner(i, me, delay);
        }
        self.claims.push(Claim {
            id,
            cells: cells.len() + trail.len(),
            at: self.players[id].look(),
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

    pub fn substep(&mut self) {
        self.clock += SUB;
        for id in 0..self.players.len() {
            if !self.players[id].alive {
                continue;
            }
            let p = &mut self.players[id];
            p.age += SUB;
            if p.progress == 0.0 && !in_bounds(p.cell + p.dir.delta()) {
                self.steer(id);
                let p = &self.players[id];
                if !in_bounds(p.cell + p.dir.delta()) {
                    continue;
                }
            }
            let p = &mut self.players[id];
            if !p.bot && p.progress < EARLY_TURN && p.dir == p.in_dir {
                if let Some(&d) = p.queue.front() {
                    p.queue.pop_front();
                    if d != p.dir && d != p.dir.opposite() {
                        p.dir = d;
                        p.turned_at = self.clock;
                    }
                }
            }
            p.progress = (p.progress + p.speed * SUB).min(1.0);
        }
        self.head_ons();
        for id in 0..self.players.len() {
            if self.players[id].alive && self.players[id].progress >= 1.0 {
                self.arrive(id);
            }
        }
        for id in 0..self.players.len() {
            let p = &mut self.players[id];
            if p.alive {
                p.peak = p.peak.max(self.counts[id] as f32 / self.owner.len() as f32);
            }
        }
    }

    fn head_ons(&mut self) {
        let n = self.players.len();
        for a in 0..n {
            for b in (a + 1)..n {
                let (pa, pb) = (&self.players[a], &self.players[b]);
                if !pa.alive || !pb.alive || pa.pos().distance(pb.pos()) >= HEAD_HIT {
                    continue;
                }
                let (la, lb) = (pa.trail.len(), pb.trail.len());
                if la <= lb {
                    self.kill(a, Some(b), Cause::HeadOn);
                }
                if lb <= la {
                    self.kill(b, Some(a), Cause::HeadOn);
                }
            }
        }
    }

    fn arrive(&mut self, id: usize) {
        let me = id as u8;
        let p = &mut self.players[id];
        let from = p.cell;
        p.progress = 0.0;
        p.cell += p.dir.delta();
        p.in_dir = p.dir;
        let c = p.cell;
        let i = idx(c);
        if let Some(o) = self.trail[i] {
            if o == me {
                self.kill(id, None, Cause::OwnTrail);
                return;
            }
            self.kill(o as usize, Some(id), Cause::Trail);
        }
        if self.owner[i] == Some(me) {
            if !self.players[id].trail.is_empty() {
                self.close_loop(id);
            }
        } else {
            let p = &mut self.players[id];
            if p.trail.is_empty() {
                p.trail_from = from;
            }
            p.trail.push(c);
            self.trail[i] = Some(me);
        }
        if self.players[id].alive {
            self.steer(id);
        }
    }

    fn steer(&mut self, id: usize) {
        if self.players[id].bot {
            self.bot_think(id);
            return;
        }
        let clock = self.clock;
        let p = &mut self.players[id];
        while let Some(d) = p.queue.pop_front() {
            if d != p.dir && d != p.dir.opposite() {
                p.dir = d;
                p.turned_at = clock;
                break;
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
    fn path(&self, id: usize, goal: impl Fn(usize) -> bool, budget: usize) -> Option<(Dir, usize)> {
        let me = Some(id as u8);
        let p = &self.players[id];
        let start = p.cell;
        let mut seen = std::collections::HashMap::with_capacity(budget.min(4096));
        let mut q = VecDeque::new();
        seen.insert(start, None::<Dir>);
        q.push_back((start, 0usize));
        let mut visited = 0;
        while let Some((c, dist)) = q.pop_front() {
            if c != start && goal(idx(c)) {
                return seen[&c].map(|d| (d, dist));
            }
            visited += 1;
            if visited > budget {
                return None;
            }
            for d in Dir::ALL {
                if c == start && d == p.dir.opposite() {
                    continue;
                }
                let n = c + d.delta();
                if !in_bounds(n) || seen.contains_key(&n) {
                    continue;
                }
                if self.trail[idx(n)] == me {
                    continue;
                }
                let first = seen[&c].or(Some(d));
                seen.insert(n, first);
                q.push_back((n, dist + 1));
            }
        }
        None
    }

    pub fn way_home(&self, id: usize) -> Option<(Dir, usize)> {
        let me = Some(id as u8);
        self.path(id, |i| self.owner[i] == me, 6000)
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
            .map(|(_, p)| (p.pos() - at).abs().element_sum())
            .fold(f32::MAX, f32::min)
    }

    fn bot_safe(&self, id: usize, d: Dir, hunting: bool) -> bool {
        let p = &self.players[id];
        let me = Some(id as u8);
        let n = p.cell + d.delta();
        if !in_bounds(n) || d == p.dir.opposite() || self.trail[idx(n)] == me {
            return false;
        }
        let nc = center(n);
        for (o, q) in self.players.iter().enumerate() {
            if o == id || !q.alive {
                continue;
            }
            let near = q.pos().distance(nc) < 1.8;
            if near && (self.gentle_to(id, o) || q.trail.len() >= p.trail.len()) {
                return false;
            }
        }
        if let Some(t) = self.trail[idx(n)] {
            if self.gentle_to(id, t as usize) {
                return false;
            }
            if !hunting && rand::gen_range(0.0, 1.0) < p.tune.caution {
                return false;
            }
        }
        true
    }

    pub fn open_ahead(&self, id: usize, d: Dir) -> i32 {
        let p = &self.players[id];
        let me = Some(id as u8);
        let mut c = p.cell;
        let mut score = 0;
        for k in 0..14 {
            c += d.delta();
            if !in_bounds(c) {
                return score - 20 + k;
            }
            let i = idx(c);
            if self.trail[i].is_some() {
                return score - 5;
            }
            if self.owner[i] != me {
                score += 1;
            }
        }
        score
    }

    fn leg(&self, id: usize) -> i32 {
        let reach = self.players[id].tune.reach as i32;
        rand::gen_range((reach / 4).max(2), (reach / 3).max(3) + 1)
    }

    fn prey(&self, id: usize) -> Option<(Dir, usize)> {
        let me = &self.players[id];
        let eligible = |o: u8| {
            let o = o as usize;
            o != id
                && self.players[o].alive
                && (o != 0
                    || self.players[0].bot
                    || arena::hunts_player(self.skill, self.players[0].age))
        };
        let close = self.players.iter().enumerate().any(|(o, q)| {
            eligible(o as u8)
                && q.trail
                    .iter()
                    .any(|t| (*t - me.cell).abs().element_sum() <= 10)
        });
        if !close {
            return None;
        }
        self.path(id, |i| self.trail[i].is_some_and(eligible), 500)
            .filter(|(_, d)| *d <= 12)
    }

    fn bot_think(&mut self, id: usize) {
        let me = Some(id as u8);
        let p = &self.players[id];
        let out = !p.trail.is_empty();
        let dir = p.dir;
        let tune = p.tune;
        let mut plan = p.plan;
        let mut want = dir;
        if !out {
            let home = self.owner[idx(p.cell)] == me;
            if !matches!(plan, Plan::Leave(_)) || rand::gen_range(0, 10) == 0 || !home {
                let pick = [dir, turn_left(dir), turn_right(dir)]
                    .into_iter()
                    .max_by_key(|d| self.open_ahead(id, *d) * 4 + rand::gen_range(0, 12))
                    .unwrap_or(dir);
                plan = Plan::Leave(pick);
            }
            if let Plan::Leave(d) = plan {
                want = d;
            }
        } else {
            if matches!(plan, Plan::Leave(_)) {
                plan = Plan::Out {
                    left: self.leg(id),
                    turns: 0,
                    right: rand::gen_range(0, 2) == 0,
                };
            }
            let len = p.trail.len();
            let home = self.way_home(id);
            let home_dist = home.map_or(99, |(_, d)| d) as f32;
            let threat = self.nearest_head(id, p.pos());
            if plan != Plan::Home {
                let flee = threat <= home_dist + 4.0 && rand::gen_range(0.0, 1.0) < tune.caution;
                if len >= tune.reach || home.is_none() || flee {
                    plan = Plan::Home;
                } else if plan != Plan::Hunt
                    && len * 2 < tune.reach
                    && rand::gen_range(0.0, 1.0) < tune.hunt * 0.3
                    && self.prey(id).is_some()
                {
                    plan = Plan::Hunt;
                }
            }
            match plan {
                Plan::Out { left, turns, right } => {
                    if left <= 1 {
                        if turns >= 2 {
                            plan = Plan::Home;
                            want = home.map_or(dir, |(d, _)| d);
                        } else {
                            want = if right {
                                turn_right(dir)
                            } else {
                                turn_left(dir)
                            };
                            plan = Plan::Out {
                                left: self.leg(id) + if turns == 0 { 0 } else { 2 },
                                turns: turns + 1,
                                right,
                            };
                        }
                    } else {
                        plan = Plan::Out {
                            left: left - 1,
                            turns,
                            right,
                        };
                    }
                }
                Plan::Hunt => match self.prey(id) {
                    Some((d, _)) if len < tune.reach => want = d,
                    _ => {
                        plan = Plan::Home;
                        want = home.map_or(dir, |(d, _)| d);
                    }
                },
                _ => want = home.map_or(dir, |(d, _)| d),
            }
        }
        let hunting = plan == Plan::Hunt;
        if !self.bot_safe(id, want, hunting) {
            let options = [dir, turn_left(dir), turn_right(dir)];
            let mut best = None;
            let mut best_score = i32::MIN;
            for d in options {
                if !self.bot_safe(id, d, hunting) && in_bounds(self.players[id].cell + d.delta()) {
                    continue;
                }
                let n = self.players[id].cell + d.delta();
                if !in_bounds(n) || self.trail[idx(n)] == me {
                    continue;
                }
                let mut s = self.open_ahead(id, d) + rand::gen_range(0, 3);
                if out {
                    let saved = self.players[id].dir;
                    self.players[id].dir = d;
                    self.players[id].cell = n;
                    let back = self.owner[idx(n)] == me || self.way_home(id).is_some();
                    self.players[id].cell = n - d.delta();
                    self.players[id].dir = saved;
                    if back {
                        s += 100;
                    }
                }
                if s > best_score {
                    best_score = s;
                    best = Some(d);
                }
            }
            want = best.unwrap_or_else(|| {
                options
                    .into_iter()
                    .find(|d| {
                        let n = self.players[id].cell + d.delta();
                        in_bounds(n) && self.trail[idx(n)] != me
                    })
                    .unwrap_or(dir)
            });
        }
        let clock = self.clock;
        let p = &mut self.players[id];
        p.plan = plan;
        if want != p.dir {
            p.dir = want;
            p.turned_at = clock;
        }
    }
}

pub fn spread_spots(n: usize) -> Vec<IVec2> {
    let mut spots = vec![];
    let ring = |k: usize, total: usize, r: f32| {
        let a = k as f32 / total as f32 * std::f32::consts::TAU + 0.4;
        let c = vec2(GW as f32, GH as f32) * 0.5;
        (c + vec2(a.cos(), a.sin()) * r).as_ivec2()
    };
    let inner = n.min(4);
    for k in 0..inner {
        spots.push(ring(k, inner, GW as f32 * 0.22));
    }
    for k in 0..n - inner {
        spots.push(ring(k, n - inner, GW as f32 * 0.4));
    }
    spots
}
