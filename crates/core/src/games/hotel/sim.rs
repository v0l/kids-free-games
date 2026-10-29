use super::layout::*;
use super::nav::Nav;
use crate::geom::*;
use crate::brain::{Effort, Net};
use glam::{vec2, Vec2};
use std::sync::{Arc, OnceLock};

mod crew;
pub use crew::{Chooser, CREW_FEATURES, THINK_FASTEST};
use crew::Pick;

pub const LOOKS: usize = 10;
pub const PLAYER_R: f32 = 0.32;
pub const AGENT_R: f32 = 0.36;
pub const SPEEDS: [f32; 3] = [3.6, 4.5, 5.4];
pub const STACKS: [u8; 3] = [2, 4, 6];
pub const GUEST_SPEED: f32 = 2.5;
pub const STAFF_SPEED: f32 = 2.1;
pub const CHECKIN_TIME: f32 = 0.7;
pub const CLERK_TIME: f32 = 2.2;
pub const SPOT_TIME: f32 = 0.55;
pub const MAID_SPOT_TIME: f32 = 1.6;
pub const SLEEP_TIME: f32 = 8.0;
pub const TOILET_TIME: f32 = 2.6;
pub const EAT_TIME: f32 = 3.4;
pub const SWIM_TIME: f32 = 6.0;
pub const PAPER_MAX: u8 = 2;
pub const GRAB_TIME: f32 = 0.2;
pub const PAD_TIME: f32 = 1.3;
pub const PAD_ARM: f32 = 0.6;
pub const PICK_R: f32 = 1.0;
pub const SPOT_R: f32 = 0.75;
pub const PAD_R: f32 = 0.9;
pub const MAX_QUEUE: usize = 5;
pub const QUEUE_DRAIN: f32 = 0.35;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Item {
    Paper,
    Snack,
    Towel,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Want {
    Toilet,
    Snack,
    Pool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pile {
    Desk,
    Toilet,
    Cafe,
    Pool,
}

pub const PILES: [Pile; 4] = [Pile::Desk, Pile::Toilet, Pile::Cafe, Pile::Pool];

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Stage {
    Arrive,
    Queue,
    ToRoom(usize),
    Sleep(usize, f32),
    WaitStall,
    ToStall(usize),
    Toilet(usize, f32),
    ToSeat(usize),
    WaitSnack(usize),
    Eat(usize, f32),
    ToBed(usize),
    WaitTowel(usize),
    Swim(usize, f32),
    Leave,
}

#[derive(Clone, Debug)]
pub struct Guest {
    pub id: u32,
    pub look: u8,
    pub vip: bool,
    pub pos: Vec2,
    pub face: f32,
    pub walk: f32,
    pub path: Vec<Vec2>,
    pub stage: Stage,
    pub patience: f32,
    pub max: f32,
    pub wants: Vec<Want>,
    pub happy: bool,
    pub angry: bool,
    pub gone: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Job {
    Idle,
    Clean(usize),
    Fetch,
    Deliver(usize),
}

#[derive(Clone, Debug)]
pub struct Staff {
    pub role: Build,
    pub pos: Vec2,
    pub face: f32,
    pub walk: f32,
    pub path: Vec<Vec2>,
    pub job: Job,
    pub target: Option<u32>,
    pub carry: Option<Item>,
    pub work: f32,
    pub think: f32,
    pub rested: bool,
    pub pick: Option<usize>,
}

#[derive(Clone, Debug)]
pub struct Player {
    pub pos: Vec2,
    pub face: f32,
    pub walk: f32,
    pub moving: bool,
    pub carry: Option<Item>,
    pub count: u8,
    pub grab: f32,
    pub work: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Room {
    pub guest: Option<u32>,
    pub dirty: Vec<bool>,
    pub spot_work: Vec<f32>,
    pub tip: u32,
}

impl Room {
    pub fn is_dirty(&self) -> bool {
        self.dirty.iter().any(|d| *d)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Stall {
    pub paper: u8,
    pub guest: Option<u32>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Task {
    Desk,
    Clean(usize, usize),
    Tip(usize),
    Fetch(Item),
    Serve(Vec2),
    Cash(Pile),
    Build(usize),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Event {
    Cash { at: Vec2, amount: u32 },
    Paid { at: Vec2 },
    CheckIn { at: Vec2 },
    SpotClean { at: Vec2 },
    RoomClean { at: Vec2 },
    Served { at: Vec2 },
    Angry { at: Vec2 },
    Grab { at: Vec2 },
    Built { build: Build, at: Vec2 },
    LevelUp { level: u32 },
    Xp { at: Vec2 },
    Splash { at: Vec2 },
    Spend,
}

pub struct Sim {
    pub lay: Layout,
    pub built: Vec<bool>,
    pub paid: Vec<u32>,
    pub rooms: Vec<Room>,
    pub stalls: Vec<Stall>,
    pub seats: Vec<Option<u32>>,
    pub beds: Vec<Option<u32>>,
    pub guests: Vec<Guest>,
    pub staff: Vec<Staff>,
    pub player: Player,
    pub piles: [u32; 4],
    pub wallet: u32,
    pub earned: u32,
    pub xp: u32,
    pub level: u32,
    pub served: u32,
    pub angry: u32,
    pub desk: f32,
    pub clerk_work: f32,
    pub t: f32,
    pub idle: f32,
    pub events: Vec<Event>,
    pub net: Option<Arc<Net>>,
    pub rules: bool,
    pub effort: Option<Effort>,
    spawn_in: f32,
    next_id: u32,
    looks: [Option<u8>; 2],
    rng: u64,
    pad_acc: f32,
    pub pad_arm: f32,
    walls: Vec<Rect>,
    locked: Vec<Rect>,
    nav: Nav,
    player_path: Vec<Vec2>,
}

fn pile_index(p: Pile) -> usize {
    PILES.iter().position(|q| *q == p).unwrap_or(0)
}

pub fn walk_along(pos: &mut Vec2, path: &mut Vec<Vec2>, speed: f32, dt: f32) -> Option<Vec2> {
    let mut left = speed * dt;
    let mut dir = None;
    while left > 0.0 {
        let Some(&next) = path.first() else {
            break;
        };
        let d = next - *pos;
        let len = d.length();
        if len > 1e-4 {
            dir = Some(d / len);
        }
        if len <= left {
            *pos = next;
            left -= len;
            path.remove(0);
        } else {
            *pos += d / len * left;
            left = 0.0;
        }
    }
    dir
}

pub fn heading(dir: Vec2) -> f32 {
    dir.x.atan2(dir.y)
}

pub fn turn_to(face: f32, want: f32, dt: f32) -> f32 {
    let mut d = (want - face) % std::f32::consts::TAU;
    if d > std::f32::consts::PI {
        d -= std::f32::consts::TAU;
    }
    if d < -std::f32::consts::PI {
        d += std::f32::consts::TAU;
    }
    face + d * (1.0 - (-14.0 * dt).exp())
}

fn blocked(q: Vec2, r: f32, walls: &[Rect]) -> bool {
    walls.iter().any(|w| {
        let cx = q.x.clamp(w.x, w.x + w.w);
        let cy = q.y.clamp(w.y, w.y + w.h);
        (vec2(cx, cy) - q).length_squared() < r * r
    })
}

pub fn slide(pos: Vec2, delta: Vec2, r: f32, walls: &[Rect]) -> Vec2 {
    let p = slide_axes(pos, delta, r, walls);
    let want = delta.length();
    if want < 1e-5 || (p - pos).length() > want * 0.3 {
        return p;
    }
    for ang in [0.6f32, -0.6, 1.1, -1.1, 1.57, -1.57] {
        let (s, c) = ang.sin_cos();
        let d = vec2(delta.x * c - delta.y * s, delta.x * s + delta.y * c);
        let q = pos + d;
        if !blocked(q, r, walls) {
            return q;
        }
    }
    let away = walls
        .iter()
        .map(|w| {
            let c = vec2(pos.x.clamp(w.x, w.x + w.w), pos.y.clamp(w.y, w.y + w.h));
            pos - c
        })
        .filter(|v| v.length() < r + 0.05)
        .fold(Vec2::ZERO, |a, v| a + v.normalize_or_zero());
    if away != Vec2::ZERO {
        let q = pos + (away.normalize() * 0.6 + delta.normalize_or_zero() * 0.8).normalize() * want;
        if !blocked(q, r * 0.98, walls) {
            return q;
        }
    }
    p
}

fn slide_axes(pos: Vec2, delta: Vec2, r: f32, walls: &[Rect]) -> Vec2 {
    let mut p = pos;
    for axis in 0..2 {
        let mut q = p;
        if axis == 0 {
            q.x += delta.x;
        } else {
            q.y += delta.y;
        }
        if !blocked(q, r, walls) {
            p = q;
        }
    }
    p
}

pub fn xp_for(level: u32, scale: f32) -> u32 {
    ((6.0 + level as f32 * 6.0) * scale).round() as u32
}

impl Sim {
    pub fn new(number: u32, built: &[bool], wallet: u32, seed: u64) -> Self {
        let lay = Layout::new(number);
        let n = lay.pads.len();
        let mut b = vec![false; n];
        for (i, v) in built.iter().enumerate().take(n) {
            b[i] = *v;
        }
        let stalls = vec![
            Stall {
                paper: PAPER_MAX,
                guest: None
            };
            lay.stalls.len()
        ];
        let seats = vec![None; lay.tables.len() * 2];
        let beds = vec![None; lay.sunbeds.len()];
        let level = lay
            .pads
            .iter()
            .zip(&b)
            .filter(|(_, d)| **d)
            .map(|(p, _)| p.level)
            .max()
            .unwrap_or(1);
        let mut s = Self {
            built: b,
            paid: vec![0; n],
            rooms: vec![],
            stalls,
            seats,
            beds,
            guests: vec![],
            staff: vec![],
            player: Player {
                pos: vec2(0.0, 1.2),
                face: std::f32::consts::PI,
                walk: 0.0,
                moving: false,
                carry: None,
                count: 0,
                grab: 0.0,
                work: 0.0,
            },
            piles: [0; 4],
            wallet,
            earned: 0,
            xp: 0,
            level,
            served: 0,
            angry: 0,
            desk: 0.0,
            clerk_work: 0.0,
            t: 0.0,
            idle: 0.0,
            events: vec![],
            net: crew::shipped(),
            rules: false,
            effort: None,
            spawn_in: 1.0,
            next_id: 0,
            looks: [None; 2],
            rng: seed | 1,
            pad_acc: 0.0,
            pad_arm: 0.0,
            walls: vec![],
            locked: vec![],
            nav: Nav::new(Rect::new(0.0, 0.0, 1.0, 1.0), &[], AGENT_R),
            player_path: vec![],
            lay,
        };
        s.rooms = (0..s.lay.rooms.len())
            .map(|r| {
                let n = s.spots(r).len();
                Room {
                    guest: None,
                    dirty: vec![false; n],
                    spot_work: vec![0.0; n],
                    tip: 0,
                }
            })
            .collect();
        for i in 0..n {
            if s.built[i] {
                s.on_built(s.lay.pads[i].build, false);
            }
        }
        s.refresh_walls();
        s
    }

    pub fn number(&self) -> u32 {
        self.lay.number
    }

    pub fn walls(&self) -> &[Rect] {
        &self.walls
    }

    pub fn locked(&self) -> &[Rect] {
        &self.locked
    }

    fn refresh_walls(&mut self) {
        let built = self.built_list();
        let solids = self.lay.solids(&|b| built.contains(&b));
        self.walls = solids.iter().map(|(r, _)| *r).collect();
        self.locked = solids
            .iter()
            .filter(|(_, k)| *k == Solid::Locked)
            .map(|(r, _)| *r)
            .collect();
        let b = self.lay.bounds();
        self.nav = Nav::new(Rect::new(b.x, b.y, b.w, b.h + 2.0), &self.walls, AGENT_R);
        self.player_path.clear();
    }

    pub fn route(&self, from: Vec2, to: Vec2) -> Vec<Vec2> {
        self.nav.path(from, to)
    }

    pub fn nav_line_free(&self, a: Vec2, b: Vec2) -> bool {
        self.nav.line_free(a, b)
    }

    fn built_list(&self) -> Vec<Build> {
        self.lay
            .pads
            .iter()
            .zip(&self.built)
            .filter(|(_, b)| **b)
            .map(|(p, _)| p.build)
            .collect()
    }

    pub fn has(&self, b: Build) -> bool {
        match b {
            Build::Room(0) => true,
            _ => self
                .lay
                .pads
                .iter()
                .zip(&self.built)
                .any(|(p, done)| p.build == b && *done),
        }
    }

    pub fn room_open(&self, k: usize) -> bool {
        self.has(Build::Room(k as u8))
    }

    pub fn open_rooms(&self) -> usize {
        (0..self.rooms.len()).filter(|k| self.room_open(*k)).count()
    }

    pub fn decor(&self, r: usize) -> u8 {
        (2..=3)
            .rev()
            .find(|&l| self.has(Build::Decor(r as u8, l)))
            .unwrap_or(1)
    }

    pub fn spots(&self, r: usize) -> Vec<Spot> {
        room_spots(r, self.decor(r), self.lay.extra_spots)
    }

    pub fn complete(&self) -> bool {
        self.has(Build::Next) || self.built.iter().all(|b| *b)
    }

    pub fn rand(&mut self) -> f32 {
        self.rng ^= self.rng << 13;
        self.rng ^= self.rng >> 7;
        self.rng ^= self.rng << 17;
        (self.rng >> 40) as f32 / (1u64 << 24) as f32
    }

    pub fn shoes(&self) -> usize {
        (1..=2)
            .rev()
            .find(|&l| self.has(Build::Shoes(l as u8)))
            .unwrap_or(0)
    }

    pub fn stack_level(&self) -> usize {
        (1..=2)
            .rev()
            .find(|&l| self.has(Build::Stack(l as u8)))
            .unwrap_or(0)
    }

    pub fn maid_speed(&self) -> f32 {
        if self.has(Build::Mop) {
            2.0
        } else {
            1.0
        }
    }

    pub fn speed(&self) -> f32 {
        SPEEDS[self.shoes()]
    }

    pub fn capacity(&self) -> u8 {
        STACKS[self.stack_level()]
    }

    pub fn room_price(&self, r: usize, vip: bool) -> u32 {
        let k = [1.0, 1.0, 1.6, 2.3][self.decor(r) as usize];
        (self.lay.room_price as f32 * k * if vip { 2.0 } else { 1.0 }).round() as u32
    }

    pub fn extra_price(&self, w: Want) -> u32 {
        let k = self.lay.number;
        match w {
            Want::Toilet => 2 + k,
            Want::Snack => 3 + k * 2,
            Want::Pool => 5 + k * 2,
        }
    }

    pub fn xp_needed(&self) -> u32 {
        xp_for(self.level, self.lay.xp_scale)
    }

    pub fn gain_xp(&mut self, n: u32, at: Vec2) {
        if self.level >= self.lay.max_level {
            return;
        }
        self.xp += n;
        self.events.push(Event::Xp { at });
        while self.level < self.lay.max_level && self.xp >= self.xp_needed() {
            self.xp -= self.xp_needed();
            self.level += 1;
            self.events.push(Event::LevelUp { level: self.level });
        }
        if self.level >= self.lay.max_level {
            self.xp = 0;
        }
    }

    pub fn visible_pads(&self) -> Vec<usize> {
        let mut out = vec![];
        for (i, p) in self.lay.pads.iter().enumerate() {
            if self.built[i] || p.level > self.level || !self.pad_ready(p.build) {
                continue;
            }
            out.push(i);
            if out.len() >= 3 {
                break;
            }
        }
        out
    }

    pub fn next_locked_level(&self) -> Option<u32> {
        self.lay
            .pads
            .iter()
            .zip(&self.built)
            .filter(|(p, b)| !**b && p.level > self.level)
            .map(|(p, _)| p.level)
            .min()
    }

    fn pad_ready(&self, b: Build) -> bool {
        match b {
            Build::Room(k) => {
                let w = k / 2;
                w == 0 || self.has(Build::Gate(w))
            }
            Build::Gate(w) => w == 1 || self.has(Build::Gate(w - 1)),
            Build::Decor(k, l) => {
                self.room_open(k as usize) && (l == 2 || self.has(Build::Decor(k, l - 1)))
            }
            Build::Shoes(l) | Build::Stack(l) => {
                l == 1
                    || self.has(if matches!(b, Build::Shoes(_)) {
                        Build::Shoes(l - 1)
                    } else {
                        Build::Stack(l - 1)
                    })
            }
            Build::Janitor => self.has(Build::Toilets),
            Build::Waiter => self.has(Build::Cafe),
            Build::Lifeguard => self.has(Build::Pool),
            Build::Mop => self.has(Build::Maid),
            Build::Pool => (1..self.lay.wings).all(|w| self.has(Build::Gate(w))),
            Build::Next => self.built.iter().filter(|b| !**b).count() == 1,
            _ => true,
        }
    }

    pub fn pick_look(&mut self) -> u8 {
        loop {
            let l = (self.rand() * LOOKS as f32) as u8 % LOOKS as u8;
            if !self.looks.contains(&Some(l)) {
                self.looks = [Some(l), self.looks[0]];
                return l;
            }
        }
    }

    pub fn waiting_line(&self) -> Vec<usize> {
        let mut v: Vec<usize> = (0..self.guests.len())
            .filter(|&i| matches!(self.guests[i].stage, Stage::Queue | Stage::Arrive))
            .collect();
        v.sort_by_key(|&i| self.guests[i].id);
        v
    }

    pub fn front_ready(&self) -> Option<usize> {
        let line = self.waiting_line();
        let &g = line.first()?;
        let guest = &self.guests[g];
        (guest.stage == Stage::Queue && (guest.pos - self.lay.desk_front).length() < 0.15)
            .then_some(g)
    }

    pub fn free_room(&self) -> Option<usize> {
        (0..self.rooms.len()).find(|&k| {
            self.room_open(k) && !self.rooms[k].is_dirty() && self.rooms[k].guest.is_none()
        })
    }

    pub fn at_desk(&self) -> bool {
        self.lay.desk_zone.contains(self.player.pos)
    }

    pub fn steer(&mut self, dir: Option<Vec2>, dt: f32) {
        let speed = self.speed();
        let b = self.lay.bounds();
        let p = &mut self.player;
        match dir {
            Some(d) if d.length() > 0.05 => {
                let d = d.clamp_length_max(1.0);
                let to = slide(p.pos, d * speed * dt, PLAYER_R, &self.walls);
                p.pos = vec2(
                    to.x.clamp(b.x + PLAYER_R, b.x + b.w - PLAYER_R),
                    to.y.clamp(b.y + PLAYER_R, b.y + b.h - PLAYER_R),
                );
                p.face = turn_to(p.face, heading(d), dt);
                p.walk += dt * 9.0 * d.length() * speed / SPEEDS[0];
                p.moving = true;
                self.idle = 0.0;
            }
            _ => {
                p.moving = false;
            }
        }
    }

    pub fn arrive_interval(&self) -> f32 {
        let rooms = self.open_rooms().max(1) as f32;
        let turnover = SLEEP_TIME + 10.0 + self.spots(0).len() as f32 * 2.5;
        (turnover / rooms).max(self.lay.arrive_every * 0.6) * self.lay.arrive_every / 6.0
    }

    fn spawn(&mut self) {
        let look = self.pick_look();
        let vip = self.lay.vip_share > 0.0 && self.rand() < self.lay.vip_share;
        let id = self.next_id;
        self.next_id += 1;
        let k = self.waiting_line().len();
        let path = self.route(SPAWN, queue_slot(k));
        let max = self.lay.patience * if vip { 0.75 } else { 1.0 };
        self.guests.push(Guest {
            id,
            look,
            vip,
            pos: SPAWN,
            face: std::f32::consts::PI,
            walk: 0.0,
            path,
            stage: Stage::Arrive,
            patience: max,
            max,
            wants: vec![],
            happy: false,
            angry: false,
            gone: false,
        });
    }

    fn plan_wants(&mut self) -> Vec<Want> {
        let mut w = vec![];
        let options = [
            (Build::Toilets, Want::Toilet),
            (Build::Cafe, Want::Snack),
            (Build::Pool, Want::Pool),
        ];
        for (b, want) in options {
            if self.has(b) && self.rand() < 0.6 {
                w.push(want);
            }
        }
        w
    }

    fn check_in(&mut self, g: usize, room: usize) {
        let price = self.room_price(room, self.guests[g].vip);
        self.piles[pile_index(Pile::Desk)] += price;
        self.rooms[room].guest = Some(self.guests[g].id);
        let to = room_pt(room, 2.0, 0.6);
        let path = self.route(self.guests[g].pos, to);
        let guest = &mut self.guests[g];
        guest.stage = Stage::ToRoom(room);
        guest.path = path;
        guest.happy = guest.patience > guest.max * 0.5;
        self.events.push(Event::CheckIn {
            at: self.lay.desk_front,
        });
        self.events.push(Event::Paid {
            at: self.lay.desk_pile,
        });
        self.gain_xp(2, self.lay.desk_front);
        self.relineup();
    }

    fn relineup(&mut self) {
        let line = self.waiting_line();
        for (k, &g) in line.iter().enumerate() {
            let slot = queue_slot(k);
            let guest = &mut self.guests[g];
            if guest.stage == Stage::Queue || guest.path.len() <= 1 || guest.pos.y < LOBBY_S {
                guest.path = vec![slot];
            } else if let Some(last) = guest.path.last_mut() {
                *last = slot;
            }
        }
    }

    fn leave(&mut self, g: usize, angry: bool) {
        let from = self.guests[g].pos;
        let path = self.route(from, SPAWN);
        let guest = &mut self.guests[g];
        guest.stage = Stage::Leave;
        guest.path = path;
        if angry {
            guest.angry = true;
            guest.happy = false;
            self.angry += 1;
            self.events.push(Event::Angry { at: from });
        } else {
            self.served += 1;
        }
    }

    fn next_want(&mut self, g: usize) {
        loop {
            let Some(want) = self.guests[g].wants.first().copied() else {
                self.leave(g, false);
                return;
            };
            self.guests[g].wants.remove(0);
            let from = self.guests[g].pos;
            let max = self.guests[g].max;
            match want {
                Want::Toilet => {
                    let to = self.lay.toilet_wait;
                    self.guests[g].path = self.route(from, to);
                    self.guests[g].stage = Stage::WaitStall;
                    self.guests[g].patience = max;
                    return;
                }
                Want::Snack => {
                    if let Some(s) = self.seats.iter().position(|s| s.is_none()) {
                        self.seats[s] = Some(self.guests[g].id);
                        let spot = seat_spots(self.lay.tables[s / 2])[s % 2];
                        self.guests[g].path = self.route(from, spot);
                        self.guests[g].stage = Stage::ToSeat(s);
                        self.guests[g].patience = max;
                        return;
                    }
                }
                Want::Pool => {
                    if let Some(b) = self.beds.iter().position(|s| s.is_none()) {
                        self.beds[b] = Some(self.guests[g].id);
                        let spot = self.lay.sunbeds[b] + vec2(0.0, 1.0);
                        self.guests[g].path = self.route(from, spot);
                        self.guests[g].stage = Stage::ToBed(b);
                        self.guests[g].patience = max;
                        return;
                    }
                }
            }
        }
    }

    pub fn step(&mut self, dt: f32) {
        let net = if self.rules { None } else { self.net.clone() };
        match net {
            Some(net) => {
                crate::rng::srand(self.rng ^ self.t.to_bits() as u64);
                self.step_with(dt, &mut |effort, options| {
                    let scores: Vec<f32> = options.iter().map(|o| net.score(o)).collect();
                    Some(crate::brain::pick_soft(&scores, effort))
                });
            }
            None => self.step_with(dt, &mut |_, _| None),
        }
    }

    pub fn step_with(&mut self, dt: f32, choose: Chooser) {
        self.t += dt;
        self.idle += dt;
        self.spawn_in -= dt;
        if self.spawn_in <= 0.0 {
            self.spawn_in = self.arrive_interval();
            if self.waiting_line().len() < MAX_QUEUE {
                self.spawn();
            }
        }
        self.step_guests(dt);
        self.step_desk(dt);
        self.step_player_actions(dt);
        self.step_staff(dt, choose);
        self.guests.retain(|g| !g.gone);
    }

    fn step_guests(&mut self, dt: f32) {
        let front = self.waiting_line().first().copied();
        for g in 0..self.guests.len() {
            let guest = &mut self.guests[g];
            let moving = !guest.path.is_empty();
            if let Some(d) = walk_along(&mut guest.pos, &mut guest.path, GUEST_SPEED, dt) {
                guest.face = turn_to(guest.face, heading(d), dt);
                guest.walk += dt * 8.0;
            } else if !moving {
                guest.walk = 0.0;
            }
            let arrived = guest.path.is_empty();
            match guest.stage {
                Stage::Arrive if arrived => guest.stage = Stage::Queue,
                Stage::Arrive | Stage::Queue => {
                    let rate = if front == Some(g) { 1.0 } else { QUEUE_DRAIN };
                    if guest.stage == Stage::Queue || guest.pos.y < LOBBY_S {
                        guest.patience -= dt * rate;
                    }
                    if arrived {
                        guest.face = turn_to(guest.face, std::f32::consts::PI, dt);
                    }
                    if guest.patience <= 0.0 {
                        self.leave(g, true);
                        self.relineup();
                    }
                }
                Stage::ToRoom(k) if arrived => {
                    guest.stage = Stage::Sleep(k, SLEEP_TIME);
                }
                Stage::Sleep(k, t) => {
                    let t = t - dt;
                    if t <= 0.0 {
                        self.check_out(g, k);
                    } else {
                        guest.stage = Stage::Sleep(k, t);
                    }
                }
                Stage::WaitStall => {
                    if arrived {
                        guest.patience -= dt;
                    }
                    let id = guest.id;
                    let free = (0..self.stalls.len())
                        .find(|&s| self.stalls[s].guest.is_none() && self.stalls[s].paper > 0);
                    if let (true, Some(s)) = (arrived, free) {
                        self.stalls[s].guest = Some(id);
                        let p = self.lay.stalls[s];
                        let front = self.lay.stall_front[s];
                        let guest = &mut self.guests[g];
                        guest.path = vec![front, p];
                        guest.stage = Stage::ToStall(s);
                    } else if self.guests[g].patience <= 0.0 {
                        self.leave(g, true);
                    }
                }
                Stage::ToStall(s) if arrived => {
                    guest.stage = Stage::Toilet(s, TOILET_TIME);
                }
                Stage::Toilet(s, t) => {
                    let t = t - dt;
                    if t <= 0.0 {
                        self.stalls[s].guest = None;
                        self.stalls[s].paper = self.stalls[s].paper.saturating_sub(1);
                        self.pay(Pile::Toilet, Want::Toilet);
                        let front = self.lay.stall_front[s];
                        self.guests[g].pos = front;
                        self.next_want(g);
                    } else {
                        guest.stage = Stage::Toilet(s, t);
                    }
                }
                Stage::ToSeat(s) if arrived => {
                    guest.stage = Stage::WaitSnack(s);
                }
                Stage::WaitSnack(_) | Stage::WaitTowel(_) => {
                    guest.patience -= dt;
                    if guest.patience <= 0.0 {
                        self.free_spot(g);
                        self.leave(g, true);
                    }
                }
                Stage::Eat(s, t) => {
                    let t = t - dt;
                    if t <= 0.0 {
                        self.seats[s] = None;
                        self.pay(Pile::Cafe, Want::Snack);
                        self.next_want(g);
                    } else {
                        guest.stage = Stage::Eat(s, t);
                    }
                }
                Stage::ToBed(b) if arrived => {
                    guest.stage = Stage::WaitTowel(b);
                }
                Stage::Swim(b, t) => {
                    let t = t - dt;
                    if t <= 0.0 {
                        self.beds[b] = None;
                        self.pay(Pile::Pool, Want::Pool);
                        self.guests[g].pos = self.lay.sunbeds[b] + vec2(0.0, 1.0);
                        self.events.push(Event::Splash {
                            at: self.guests[g].pos,
                        });
                        self.next_want(g);
                    } else {
                        let id = guest.id;
                        guest.stage = Stage::Swim(b, t);
                        if (t / 1.5).floor() != ((t + dt) / 1.5).floor() {
                            let at = swim_pos(&self.lay.pool, t, id);
                            self.events.push(Event::Splash { at });
                        }
                    }
                }
                Stage::Leave if arrived => guest.gone = true,
                _ => {}
            }
        }
    }

    fn check_out(&mut self, g: usize, k: usize) {
        let n = self.rooms[k].dirty.len();
        for i in 0..n {
            self.rooms[k].dirty[i] = true;
            self.rooms[k].spot_work[i] = 0.0;
        }
        self.rooms[k].guest = None;
        let happy = self.guests[g].happy;
        let base = self.room_price(k, self.guests[g].vip) / 2;
        self.rooms[k].tip = if happy { base.max(2) } else { 1 };
        let wants = self.plan_wants();
        self.guests[g].wants = wants;
        self.guests[g].pos = room_pt(k, 1.6, 1.2);
        self.next_want(g);
    }

    fn free_spot(&mut self, g: usize) {
        let id = self.guests[g].id;
        for s in self.seats.iter_mut().chain(self.beds.iter_mut()) {
            if *s == Some(id) {
                *s = None;
            }
        }
    }

    fn pay(&mut self, pile: Pile, w: Want) {
        let amount = self.extra_price(w);
        self.piles[pile_index(pile)] += amount;
        let at = self.pile_pos(pile);
        self.events.push(Event::Paid { at });
    }

    pub fn pile_pos(&self, p: Pile) -> Vec2 {
        match p {
            Pile::Desk => self.lay.desk_pile,
            Pile::Toilet => self.lay.toilet_pile,
            Pile::Cafe => self.lay.cafe_pile,
            Pile::Pool => self.lay.pool_pile,
        }
    }

    pub fn pile(&self, p: Pile) -> u32 {
        self.piles[pile_index(p)]
    }

    fn step_desk(&mut self, dt: f32) {
        let ready = self.front_ready().zip(self.free_room());
        if ready.is_some() && self.at_desk() {
            self.desk += dt / CHECKIN_TIME;
            self.idle = 0.0;
        } else {
            self.desk = 0.0;
        }
        let clerk_on = self.has(Build::Clerk) && !self.at_desk() && self.clerk_home();
        if ready.is_some() && clerk_on {
            self.clerk_work += dt / CLERK_TIME;
        } else {
            self.clerk_work = 0.0;
        }
        if let Some((g, k)) = ready {
            if self.desk >= 1.0 || self.clerk_work >= 1.0 {
                self.desk = 0.0;
                self.clerk_work = 0.0;
                self.check_in(g, k);
            }
        }
    }

    fn clerk_home(&self) -> bool {
        self.staff
            .iter()
            .any(|s| s.role == Build::Clerk && (s.pos - self.lay.clerk_spot).length() < 0.2)
    }

    fn step_player_actions(&mut self, dt: f32) {
        let pos = self.player.pos;
        let mut working = false;
        for k in 0..self.rooms.len() {
            if self.rooms[k].tip > 0 {
                let at = room_pt(k, 1.6, 1.2);
                if (at - pos).length() < PICK_R {
                    let amount = self.rooms[k].tip;
                    self.rooms[k].tip = 0;
                    self.collect(amount, at);
                }
            }
            if !self.rooms[k].is_dirty() {
                continue;
            }
            let spots = self.spots(k);
            for (i, sp) in spots.iter().enumerate() {
                if self.rooms[k].dirty[i] && (sp.stand - pos).length() < SPOT_R {
                    working = true;
                    self.rooms[k].spot_work[i] += dt / SPOT_TIME;
                    self.idle = 0.0;
                    if self.rooms[k].spot_work[i] >= 1.0 {
                        self.clean_spot(k, i);
                    }
                    break;
                }
            }
        }
        self.player.work = if working { self.player.work + dt } else { 0.0 };
        for p in PILES {
            let i = pile_index(p);
            if self.piles[i] > 0 && (self.pile_pos(p) - pos).length() < PICK_R {
                let amount = self.piles[i];
                self.piles[i] = 0;
                self.collect(amount, self.pile_pos(p));
            }
        }
        self.step_carry(dt);
        self.step_deliver();
        self.step_pads(dt);
    }

    fn collect(&mut self, amount: u32, at: Vec2) {
        self.wallet += amount;
        self.earned += amount;
        self.events.push(Event::Cash { at, amount });
        self.idle = 0.0;
    }

    fn clean_spot(&mut self, k: usize, i: usize) {
        self.rooms[k].dirty[i] = false;
        self.rooms[k].spot_work[i] = 0.0;
        let at = self.spots(k)[i].obj;
        self.events.push(Event::SpotClean { at });
        self.gain_xp(1, at);
        if !self.rooms[k].is_dirty() {
            self.events.push(Event::RoomClean { at: room_center(k) });
        }
    }

    pub fn source(&self, item: Item) -> Option<Vec2> {
        match item {
            Item::Paper if self.has(Build::Toilets) => Some(self.lay.paper_shelf),
            Item::Snack if self.has(Build::Cafe) => Some(self.lay.counter_spot),
            Item::Towel if self.has(Build::Pool) => Some(self.lay.towel_shelf),
            _ => None,
        }
    }

    fn step_carry(&mut self, dt: f32) {
        let cap = self.capacity();
        for item in [Item::Paper, Item::Snack, Item::Towel] {
            let Some(src) = self.source(item) else {
                continue;
            };
            if (src - self.player.pos).length() > PICK_R {
                continue;
            }
            let p = &mut self.player;
            if p.carry.is_some() && p.carry != Some(item) {
                p.carry = None;
                p.count = 0;
            }
            if p.count >= cap {
                continue;
            }
            p.grab += dt;
            if p.grab >= GRAB_TIME {
                p.grab = 0.0;
                p.carry = Some(item);
                p.count += 1;
                self.idle = 0.0;
                self.events.push(Event::Grab { at: src });
            }
            return;
        }
        self.player.grab = 0.0;
    }

    fn step_deliver(&mut self) {
        let Some(item) = self.player.carry else {
            return;
        };
        let pos = self.player.pos;
        match item {
            Item::Paper => {
                for s in 0..self.stalls.len() {
                    if self.stalls[s].paper < PAPER_MAX
                        && (self.lay.stall_front[s] - pos).length() < PICK_R + 0.2
                    {
                        self.stalls[s].paper = PAPER_MAX;
                        self.use_item();
                        let at = self.lay.stalls[s];
                        self.events.push(Event::Served { at });
                        self.gain_xp(1, at);
                        return;
                    }
                }
            }
            Item::Snack | Item::Towel => {
                for g in 0..self.guests.len() {
                    let want = match (item, self.guests[g].stage) {
                        (Item::Snack, Stage::WaitSnack(s)) => Some(Stage::Eat(s, EAT_TIME)),
                        (Item::Towel, Stage::WaitTowel(b)) => Some(Stage::Swim(b, SWIM_TIME)),
                        _ => None,
                    };
                    if let Some(next) = want {
                        if (self.guests[g].pos - pos).length() < PICK_R + 0.3 {
                            self.serve(g, next);
                            self.use_item();
                            let at = self.guests[g].pos;
                            self.gain_xp(1, at);
                            return;
                        }
                    }
                }
            }
        }
    }

    fn serve(&mut self, g: usize, next: Stage) {
        let guest = &mut self.guests[g];
        guest.stage = next;
        guest.happy = guest.patience > guest.max * 0.4;
        self.events.push(Event::Served { at: guest.pos });
        if let Stage::Swim(..) = next {
            let at = self.guests[g].pos;
            self.events.push(Event::Splash { at });
        }
        self.idle = 0.0;
    }

    fn use_item(&mut self) {
        let p = &mut self.player;
        p.count = p.count.saturating_sub(1);
        if p.count == 0 {
            p.carry = None;
        }
    }

    pub fn pad_under_player(&self) -> Option<usize> {
        self.visible_pads()
            .into_iter()
            .find(|&i| (self.lay.pads[i].pos - self.player.pos).length() < PAD_R)
    }

    fn step_pads(&mut self, dt: f32) {
        let Some(i) = self.pad_under_player() else {
            self.pad_acc = 0.0;
            self.pad_arm = 0.0;
            return;
        };
        let cost = self.lay.pads[i].cost;
        if self.wallet == 0 || self.paid[i] >= cost {
            return;
        }
        if self.player.moving {
            self.pad_arm = 0.0;
            self.pad_acc = 0.0;
            return;
        }
        self.pad_arm += dt;
        if self.pad_arm < PAD_ARM {
            return;
        }
        self.idle = 0.0;
        self.pad_acc += cost as f32 * dt / PAD_TIME;
        let n = (self.pad_acc.floor() as u32)
            .min(self.wallet)
            .min(cost - self.paid[i]);
        if n > 0 {
            self.pad_acc -= n as f32;
            self.wallet -= n;
            self.paid[i] += n;
            self.events.push(Event::Spend);
        }
        if self.paid[i] >= cost {
            self.built[i] = true;
            let b = self.lay.pads[i].build;
            self.on_built(b, true);
            self.refresh_walls();
            self.unstick_player();
        }
    }

    fn unstick_player(&mut self) {
        let p = self.player.pos;
        let hit = |q: Vec2, walls: &[Rect]| {
            walls.iter().any(|w| {
                let c = vec2(q.x.clamp(w.x, w.x + w.w), q.y.clamp(w.y, w.y + w.h));
                (c - q).length() < PLAYER_R
            })
        };
        if !hit(p, &self.walls) {
            return;
        }
        for r in 1..30 {
            for a in 0..12 {
                let ang = a as f32 / 12.0 * std::f32::consts::TAU;
                let q = p + vec2(ang.cos(), ang.sin()) * r as f32 * 0.15;
                if !hit(q, &self.walls) {
                    self.player.pos = q;
                    return;
                }
            }
        }
    }

    fn on_built(&mut self, b: Build, fresh: bool) {
        if b.is_staff() {
            let at = self.lay.idle_spot(b);
            self.staff.push(Staff {
                role: b,
                pos: if fresh {
                    self.lay.pad(b).map(|p| p.pos).unwrap_or(at)
                } else {
                    at
                },
                face: std::f32::consts::PI,
                walk: 0.0,
                path: if fresh { vec![at] } else { vec![] },
                job: Job::Idle,
                target: None,
                carry: None,
                work: 0.0,
                think: 0.0,
                rested: false,
                pick: None,
            });
        }
        if let Build::Decor(r, _) = b {
            let r = r as usize;
            if r < self.rooms.len() {
                let n = self.spots(r).len();
                let dirty = self.rooms[r].is_dirty();
                self.rooms[r].dirty.resize(n, dirty);
                self.rooms[r].spot_work.resize(n, 0.0);
            }
        }
        if fresh {
            let at = self.lay.pad(b).map(|p| p.pos).unwrap_or_default();
            self.events.push(Event::Built { build: b, at });
            self.gain_xp(3, at);
        }
    }

    fn step_staff(&mut self, dt: f32, choose: Chooser) {
        for s in 0..self.staff.len() {
            let st = &mut self.staff[s];
            st.think -= dt;
            let moving = !st.path.is_empty();
            if let Some(d) = walk_along(&mut st.pos, &mut st.path, STAFF_SPEED, dt) {
                st.face = turn_to(st.face, heading(d), dt);
                st.walk += dt * 8.0;
            } else if !moving {
                st.walk = 0.0;
            }
            let strolling_home =
                self.staff[s].job == Job::Idle && self.staff[s].role != Build::Clerk;
            if !self.staff[s].path.is_empty() && !strolling_home {
                continue;
            }
            match self.staff[s].role {
                Build::Clerk => self.staff_clerk(s),
                Build::Maid => self.staff_maid(s, dt, choose),
                Build::Janitor => self.staff_janitor(s, choose),
                Build::Waiter => self.staff_server(s, Item::Snack, choose),
                Build::Lifeguard => self.staff_server(s, Item::Towel, choose),
                _ => {}
            }
        }
    }

    fn go(&mut self, s: usize, to: Vec2) {
        let from = self.staff[s].pos;
        self.staff[s].path = self.route(from, to);
    }

    fn go_home(&mut self, s: usize) {
        let home = self.lay.idle_spot(self.staff[s].role);
        if self.staff[s].path.is_empty() && (self.staff[s].pos - home).length() > 0.1 {
            self.go(s, home);
        }
        self.staff[s].job = Job::Idle;
        self.staff[s].target = None;
    }

    fn staff_clerk(&mut self, s: usize) {
        let home = self.lay.clerk_spot;
        if (self.staff[s].pos - home).length() > 0.1 {
            self.go(s, home);
        } else {
            self.staff[s].face = std::f32::consts::FRAC_PI_2 * 0.0;
        }
    }

    fn claimed_room(&self, k: usize) -> bool {
        self.staff
            .iter()
            .any(|st| st.role == Build::Maid && st.job == Job::Clean(k))
    }

    fn staff_maid(&mut self, s: usize, dt: f32, choose: Chooser) {
        match self.staff[s].job {
            Job::Clean(k) => {
                if self.rooms[k].tip > 0 {
                    let amount = self.rooms[k].tip;
                    self.rooms[k].tip = 0;
                    self.piles[pile_index(Pile::Desk)] += amount;
                }
                let spots = self.spots(k);
                let Some(i) = (0..spots.len()).find(|&i| self.rooms[k].dirty[i]) else {
                    self.go_home(s);
                    return;
                };
                let stand = spots[i].stand;
                if (self.staff[s].pos - stand).length() > 0.15 {
                    self.go(s, stand);
                    return;
                }
                self.staff[s].work += dt;
                self.rooms[k].spot_work[i] += dt / MAID_SPOT_TIME * self.maid_speed();
                if self.rooms[k].spot_work[i] >= 1.0 {
                    self.clean_spot(k, i);
                }
            }
            _ => {
                self.staff[s].work = 0.0;
                match self.crew_pick(s, choose) {
                    Some(Pick::Room(k)) => self.staff[s].job = Job::Clean(k),
                    _ => self.go_home(s),
                }
            }
        }
    }

    fn staff_janitor(&mut self, s: usize, choose: Chooser) {
        match self.staff[s].job {
            Job::Fetch => {
                self.staff[s].carry = Some(Item::Paper);
                let wanted = self.staff[s].pick.filter(|&k| self.stalls[k].paper < PAPER_MAX);
                let need = wanted
                    .or_else(|| (0..self.stalls.len()).find(|&k| self.stalls[k].paper < PAPER_MAX));
                match need {
                    Some(k) => {
                        self.staff[s].job = Job::Deliver(k);
                        let to = self.lay.stall_front[k];
                        self.go(s, to);
                    }
                    None => {
                        self.staff[s].carry = None;
                        self.go_home(s);
                    }
                }
            }
            Job::Deliver(k) => {
                if self.stalls[k].paper < PAPER_MAX {
                    self.stalls[k].paper = PAPER_MAX;
                    self.events.push(Event::Served {
                        at: self.lay.stalls[k],
                    });
                }
                self.staff[s].carry = None;
                self.go_home(s);
            }
            _ => {
                match self.crew_pick(s, choose) {
                    Some(Pick::Stall(k)) => {
                        self.staff[s].pick = Some(k);
                        self.staff[s].job = Job::Fetch;
                        let to = self.lay.paper_shelf + vec2(0.0, 0.8);
                        self.go(s, to);
                    }
                    _ => self.go_home(s),
                }
            }
        }
    }

    fn claimed_guest(&self, id: u32) -> bool {
        self.staff.iter().any(|st| st.target == Some(id))
    }

    fn staff_server(&mut self, s: usize, item: Item, choose: Chooser) {
        let waiting = |st: Stage| match item {
            Item::Snack => matches!(st, Stage::WaitSnack(_)),
            _ => matches!(st, Stage::WaitTowel(_)),
        };
        let target = self.staff[s].target;
        let find = |sim: &Sim| {
            target.and_then(|id| {
                sim.guests
                    .iter()
                    .position(|g| g.id == id && waiting(g.stage))
            })
        };
        match self.staff[s].job {
            Job::Fetch => {
                self.staff[s].carry = Some(item);
                match find(self) {
                    Some(g) => {
                        self.staff[s].job = Job::Deliver(g);
                        let to = self.guests[g].pos + vec2(0.0, 0.7);
                        self.go(s, to);
                    }
                    None => {
                        self.staff[s].carry = None;
                        self.go_home(s);
                    }
                }
            }
            Job::Deliver(_) => {
                if let Some(g) = find(self) {
                    let next = match self.guests[g].stage {
                        Stage::WaitSnack(k) => Stage::Eat(k, EAT_TIME),
                        Stage::WaitTowel(k) => Stage::Swim(k, SWIM_TIME),
                        st => st,
                    };
                    self.serve(g, next);
                }
                self.staff[s].carry = None;
                self.go_home(s);
            }
            _ => {
                let g = match self.crew_pick(s, choose) {
                    Some(Pick::Guest(id)) => Some(id),
                    _ => None,
                };
                match (g, self.source(item)) {
                    (Some(id), Some(src)) => {
                        self.staff[s].target = Some(id);
                        self.staff[s].job = Job::Fetch;
                        let to = src
                            + if item == Item::Snack {
                                vec2(-0.2, 0.6)
                            } else {
                                vec2(-0.7, 0.0)
                            };
                        self.go(s, to);
                    }
                    _ => self.go_home(s),
                }
            }
        }
    }

    fn cash_unlocks_a_build(&self) -> Option<Pile> {
        let banked: u32 = self.piles.iter().sum();
        let need = self
            .visible_pads()
            .into_iter()
            .map(|i| self.lay.pads[i].cost - self.paid[i])
            .min()?;
        (self.wallet < need && self.wallet + banked >= need)
            .then(|| PILES.into_iter().max_by_key(|p| self.pile(*p)))
            .flatten()
    }

    pub fn affordable_pad(&self) -> Option<usize> {
        self.visible_pads()
            .into_iter()
            .find(|&i| self.wallet + self.paid[i] >= self.lay.pads[i].cost)
    }

    fn dirty_spot(&self) -> Option<(usize, usize)> {
        let p = self.player.pos;
        let mut best: Option<((usize, usize), f32)> = None;
        for k in 0..self.rooms.len() {
            if !self.rooms[k].is_dirty() {
                continue;
            }
            if self.claimed_room(k) {
                continue;
            }
            for (i, sp) in self.spots(k).iter().enumerate() {
                if self.rooms[k].dirty[i] {
                    let d = (sp.stand - p).length();
                    if best.is_none_or(|(_, b)| d < b) {
                        best = Some(((k, i), d));
                    }
                }
            }
        }
        best.map(|b| b.0)
    }

    pub fn next_task(&self) -> Option<Task> {
        if let Some(p) = self.cash_unlocks_a_build() {
            return Some(Task::Cash(p));
        }
        if self.player.carry.is_none() {
            if let Some(i) = self.affordable_pad() {
                return Some(Task::Build(i));
            }
        }
        let clerk = self.has(Build::Clerk) && self.clerk_home();
        if self.front_ready().is_some() && self.free_room().is_some() && !clerk {
            return Some(Task::Desk);
        }
        if let Some(spot) = self.player.carry.and_then(|item| self.deliver_spot(item)) {
            return Some(Task::Serve(spot));
        }
        if let Some(k) = (0..self.rooms.len()).find(|&k| self.rooms[k].tip > 0) {
            return Some(Task::Tip(k));
        }
        if let Some((k, i)) = self.dirty_spot() {
            return Some(Task::Clean(k, i));
        }
        let free_hands = |item: Item| self.player.carry.is_none_or(|c| c == item);
        if !self.has(Build::Janitor)
            && self.has(Build::Toilets)
            && self.stalls_need_paper()
            && free_hands(Item::Paper)
        {
            return Some(Task::Fetch(Item::Paper));
        }
        if !self.has(Build::Waiter) && self.guest_waiting(Item::Snack) && free_hands(Item::Snack) {
            return Some(Task::Fetch(Item::Snack));
        }
        if !self.has(Build::Lifeguard) && self.guest_waiting(Item::Towel) && free_hands(Item::Towel)
        {
            return Some(Task::Fetch(Item::Towel));
        }
        let best = PILES
            .into_iter()
            .filter(|p| self.pile(*p) > 0)
            .max_by_key(|p| self.pile(*p));
        if let Some(p) = best {
            return Some(Task::Cash(p));
        }
        if self.front_ready().is_some() && self.free_room().is_some() {
            return Some(Task::Desk);
        }
        None
    }

    fn stalls_need_paper(&self) -> bool {
        self.stalls.iter().any(|s| s.paper == 0)
            || (self.stalls.iter().any(|s| s.paper < PAPER_MAX)
                && self
                    .guests
                    .iter()
                    .any(|g| matches!(g.stage, Stage::WaitStall)))
    }

    fn guest_waiting(&self, item: Item) -> bool {
        self.guests.iter().any(|g| match item {
            Item::Snack => matches!(g.stage, Stage::WaitSnack(_)),
            Item::Towel => matches!(g.stage, Stage::WaitTowel(_)),
            Item::Paper => false,
        })
    }

    fn deliver_spot(&self, item: Item) -> Option<Vec2> {
        match item {
            Item::Paper => (0..self.stalls.len())
                .filter(|&s| self.stalls[s].paper < PAPER_MAX)
                .min_by_key(|&s| self.stalls[s].paper)
                .map(|s| self.lay.stall_front[s]),
            _ => self
                .guests
                .iter()
                .find(|g| match item {
                    Item::Snack => matches!(g.stage, Stage::WaitSnack(_)),
                    _ => matches!(g.stage, Stage::WaitTowel(_)),
                })
                .map(|g| g.pos),
        }
    }

    pub fn task_pos(&self, t: Task) -> Vec2 {
        match t {
            Task::Desk => vec2(0.0, 1.2),
            Task::Clean(k, i) => self.spots(k)[i].stand,
            Task::Tip(k) => room_pt(k, 1.6, 1.2),
            Task::Fetch(item) => self.source(item).unwrap_or_default(),
            Task::Serve(p) => p,
            Task::Cash(p) => self.pile_pos(p),
            Task::Build(i) => self.lay.pads[i].pos,
        }
    }

    pub fn goal_for(&self, t: Task) -> Vec2 {
        match t {
            Task::Fetch(item) => {
                let full = self.player.count >= self.capacity() && self.player.carry == Some(item);
                match (full, self.deliver_spot(item)) {
                    (true, Some(spot)) => spot,
                    _ => self.task_pos(t),
                }
            }
            _ => self.task_pos(t),
        }
    }

    pub fn task_live(&self, t: Task) -> bool {
        match t {
            Task::Desk => self.front_ready().is_some() && self.free_room().is_some(),
            Task::Clean(k, i) => self.rooms[k].dirty.get(i).copied().unwrap_or(false),
            Task::Tip(k) => self.rooms[k].tip > 0,
            Task::Fetch(item) => match item {
                Item::Paper => self.stalls.iter().any(|s| s.paper < PAPER_MAX),
                _ => self.guest_waiting(item),
            },
            Task::Serve(_) => self.player.carry.is_some(),
            Task::Cash(p) => self.pile(p) > 0,
            Task::Build(i) => !self.built[i] && self.wallet > 0,
        }
    }

    pub fn walk_toward(&mut self, goal: Vec2, dt: f32, pace: f32) {
        let here = self.player.pos;
        let stale = self
            .player_path
            .last()
            .is_none_or(|l| (*l - goal).length() > 0.05)
            || self
                .player_path
                .first()
                .is_some_and(|f| !self.nav.line_free(here, *f));
        if stale {
            self.player_path = self.route(here, goal);
        }
        while self
            .player_path
            .first()
            .is_some_and(|p| (*p - here).length() < 0.12 && self.player_path.len() > 1)
        {
            self.player_path.remove(0);
        }
        let next = self.player_path.first().copied();
        if (goal - here).length() < 0.25 {
            self.steer(None, dt);
            return;
        }
        match next {
            Some(n) => {
                let d = (n - self.player.pos).normalize_or_zero();
                let close = (goal - self.player.pos).length();
                let k = if close < 0.3 { close / 0.3 } else { 1.0 };
                self.steer(Some(d * k.max(0.2) * pace), dt);
            }
            None => self.steer(None, dt),
        }
    }

    pub fn add_swimmers(&mut self, n: usize) {
        for b in 0..n.min(self.beds.len()) {
            self.spawn();
            let g = self.guests.len() - 1;
            let id = self.guests[g].id;
            self.beds[b] = Some(id);
            self.guests[g].path.clear();
            self.guests[g].stage = Stage::Swim(b, SWIM_TIME * (0.3 + b as f32 * 0.17));
        }
    }

    #[cfg(test)]
    pub fn check_out_for_test(&mut self, k: usize) {
        self.spawn();
        let g = self.guests.len() - 1;
        self.guests[g].happy = true;
        self.rooms[k].guest = Some(self.guests[g].id);
        self.check_out(g, k);
    }

    #[cfg(test)]
    pub fn check_out_for_test_unhappy(&mut self, k: usize) {
        self.spawn();
        let g = self.guests.len() - 1;
        self.guests[g].happy = false;
        self.rooms[k].guest = Some(self.guests[g].id);
        self.check_out(g, k);
    }

    pub fn autopilot(&mut self, dt: f32) {
        if let Some(Task::Build(i)) = self.next_task() {
            if self.pad_under_player() == Some(i) {
                self.steer(None, dt);
                return;
            }
        }
        match self.next_task() {
            Some(t) => {
                let goal = self.goal_for(t);
                self.walk_toward(goal, dt, 1.0);
            }
            None => self.steer(None, dt),
        }
    }
}

pub fn swim_pos(pool: &Rect, t: f32, id: u32) -> Vec2 {
    let lane = (id % 3) as f32;
    let z = pool.y + 0.6 + lane * (pool.h - 1.2) / 2.0;
    let k = (t / SWIM_TIME * 2.0).fract();
    let x = if k < 0.5 {
        pool.x + 0.7 + (pool.w - 1.4) * k * 2.0
    } else {
        pool.x + pool.w - 0.7 - (pool.w - 1.4) * (k - 0.5) * 2.0
    };
    vec2(x, z)
}

pub fn swim_dir(t: f32) -> f32 {
    let k = (t / SWIM_TIME * 2.0).fract();
    if k < 0.5 {
        -std::f32::consts::FRAC_PI_2
    } else {
        std::f32::consts::FRAC_PI_2
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    pub fn run(sim: &mut Sim, secs: f32, mut brain: impl FnMut(&mut Sim, f32)) -> f32 {
        let dt = 1.0 / 30.0;
        let mut t = 0.0;
        while t < secs && !sim.complete() {
            brain(sim, dt);
            sim.step(dt);
            sim.events.clear();
            t += dt;
        }
        t
    }

    fn strong(sim: &mut Sim, dt: f32) {
        sim.autopilot(dt);
    }

    fn weak() -> impl FnMut(&mut Sim, f32) {
        let mut task: Option<Task> = None;
        let mut held = 0.0;
        move |sim: &mut Sim, dt: f32| {
            let beat = (sim.t / 5.0) as u32;
            if beat % 3 == 2 {
                sim.steer(None, dt);
                return;
            }
            held += dt;
            let keep = task.is_some_and(|t| sim.task_live(t) && held < 12.0);
            if !keep {
                task = sim.next_task();
                held = 0.0;
            }
            match task {
                Some(Task::Build(i)) if sim.pad_under_player() == Some(i) => sim.steer(None, dt),
                Some(t) => {
                    let goal = sim.goal_for(t);
                    sim.walk_toward(goal, dt, 0.6);
                }
                None => sim.steer(None, dt),
            }
        }
    }

    fn built(sim: &Sim) -> usize {
        sim.built.iter().filter(|b| **b).count()
    }

    #[test]
    fn a_strong_player_finishes_every_hotel_and_each_takes_longer() {
        let mut prev = 0.0;
        for n in 1..=MAX_HOTEL {
            let mut sim = Sim::new(n, &[], 0, 3);
            let t = run(&mut sim, 2400.0, strong);
            assert!(
                sim.complete(),
                "hotel {n} stuck at {}/{}",
                built(&sim),
                sim.built.len()
            );
            assert!(t > 600.0, "hotel {n} done in {t:.0}s, too easy");
            assert!(
                t > prev,
                "hotel {n} ({t:.0}s) quicker than the one before ({prev:.0}s)"
            );
            prev = t;
        }
    }

    #[test]
    fn no_purchase_waits_more_than_a_few_minutes_for_a_strong_player() {
        for n in [1, 4] {
            let mut sim = Sim::new(n, &[], 0, 3);
            let (mut last, mut count, mut t) = (0.0f32, built(&sim), 0.0f32);
            let dt = 1.0 / 30.0;
            while t < 2400.0 && !sim.complete() {
                sim.autopilot(dt);
                sim.step(dt);
                sim.events.clear();
                t += dt;
                if built(&sim) != count {
                    count = built(&sim);
                    assert!(
                        t - last < 240.0,
                        "hotel {n}: {:.0}s between builds",
                        t - last
                    );
                    last = t;
                }
            }
            assert!(sim.complete());
        }
    }

    #[test]
    fn a_weak_player_keeps_progressing() {
        for n in [1, 3] {
            let mut sim = Sim::new(n, &[], 0, 11);
            run(&mut sim, 900.0, weak());
            assert!(built(&sim) >= 6, "hotel {n}: only {} built", built(&sim));
            assert!(sim.level >= 3, "hotel {n}: level {}", sim.level);
        }
        let mut sim = Sim::new(1, &[], 0, 5);
        run(&mut sim, 3000.0, weak());
        assert!(sim.complete(), "weak player never finished hotel 1");
    }

    #[test]
    fn a_weak_player_loses_far_more_guests_than_a_strong_one() {
        let mut a = Sim::new(3, &[], 0, 7);
        run(&mut a, 900.0, strong);
        let mut b = Sim::new(3, &[], 0, 7);
        run(&mut b, 900.0, weak());
        let rate = |s: &Sim| s.angry as f32 / (s.served + s.angry).max(1) as f32;
        assert!(
            rate(&b) > rate(&a) * 1.5,
            "strong {:.2} weak {:.2}",
            rate(&a),
            rate(&b)
        );
        assert!(built(&a) > built(&b));
    }

    #[test]
    fn standing_still_earns_nothing_and_the_queue_gives_up() {
        let mut sim = Sim::new(1, &[], 0, 5);
        sim.player.pos = vec2(-4.0, 6.0);
        run(&mut sim, 300.0, |s, dt| s.steer(None, dt));
        assert_eq!(sim.wallet, 0);
        assert_eq!(sim.served, 0);
        assert!(sim.angry >= 3);
        assert!(sim.waiting_line().len() <= MAX_QUEUE);
    }

    #[test]
    fn waiting_in_line_wears_everyone_down_but_the_front_fastest() {
        let mut sim = Sim::new(1, &[], 0, 5);
        sim.player.pos = vec2(-4.0, 6.0);
        run(&mut sim, 45.0, |s, dt| s.steer(None, dt));
        let line = sim.waiting_line();
        assert!(line.len() >= 2, "line {}", line.len());
        let lost = |g: usize| sim.guests[g].max - sim.guests[g].patience;
        assert!(lost(line[0]) > 0.0);
        for &g in &line[1..] {
            let queued = sim.guests[g].stage == Stage::Queue;
            if queued {
                assert!(lost(g) > 0.0, "guest {g} in line lost no patience");
                assert!(lost(g) < lost(line[0]));
            }
        }
    }

    #[test]
    fn rooms_are_cleaned_object_by_object() {
        let mut sim = Sim::new(1, &[], 0, 1);
        sim.check_out_for_test(0);
        let spots = sim.spots(0);
        assert!(spots.len() >= 2);
        assert!(sim.rooms[0].dirty.iter().all(|d| *d));
        sim.player.pos = spots[0].stand;
        run(&mut sim, SPOT_TIME + 0.2, |s, dt| s.steer(None, dt));
        assert!(!sim.rooms[0].dirty[0]);
        assert!(sim.rooms[0].dirty[1]);
        assert!(sim.free_room() != Some(0));
    }

    #[test]
    fn a_happy_guest_leaves_a_bigger_tip() {
        let mut a = Sim::new(2, &[], 0, 1);
        a.check_out_for_test(0);
        let mut b = Sim::new(2, &[], 0, 1);
        b.check_out_for_test_unhappy(0);
        assert!(a.rooms[0].tip > b.rooms[0].tip);
        a.player.pos = room_pt(0, 1.6, 1.2);
        let before = a.wallet;
        run(&mut a, 0.2, |s, dt| s.steer(None, dt));
        assert!(a.wallet > before);
    }

    #[test]
    fn every_upgrade_changes_something_you_can_measure() {
        for n in 1..=MAX_HOTEL {
            let lay = Layout::new(n);
            for (i, pad) in lay.pads.iter().enumerate() {
                let mut built: Vec<bool> = vec![false; lay.pads.len()];
                for (j, p) in lay.pads.iter().enumerate() {
                    built[j] = j < i && p.build != Build::Next;
                }
                let before = Sim::new(n, &built, 0, 1);
                built[i] = true;
                let after = Sim::new(n, &built, 0, 1);
                let changed = before.speed() != after.speed()
                    || before.maid_speed() != after.maid_speed()
                    || before.has(Build::Pool) != after.has(Build::Pool)
                    || before.locked().len() != after.locked().len()
                    || before.capacity() != after.capacity()
                    || before.open_rooms() != after.open_rooms()
                    || before.staff.len() != after.staff.len()
                    || before.walls().len() != after.walls().len()
                    || (0..before.rooms.len()).any(|r| {
                        before.room_price(r, false) != after.room_price(r, false)
                            || before.spots(r).len() != after.spots(r).len()
                    })
                    || after.complete();
                assert!(changed, "hotel {n}: {:?} does nothing", pad.build);
            }
        }
    }

    #[test]
    fn levels_gate_what_you_can_build() {
        let mut sim = Sim::new(1, &[], 9999, 1);
        assert_eq!(sim.level, 1);
        assert!(sim
            .visible_pads()
            .iter()
            .all(|&i| sim.lay.pads[i].level <= 1));
        assert!(sim.next_locked_level().is_some());
        let need = sim.xp_needed();
        sim.gain_xp(need, Vec2::ZERO);
        assert_eq!(sim.level, 2);
        assert!(sim
            .visible_pads()
            .iter()
            .any(|&i| sim.lay.pads[i].level == 2));
    }

    #[test]
    fn xp_needed_rises_with_each_level() {
        for n in 1..=MAX_HOTEL {
            let s = Layout::new(n).xp_scale;
            for l in 1..9 {
                assert!(xp_for(l + 1, s) > xp_for(l, s));
            }
        }
    }

    #[test]
    fn shoes_and_stacks_have_levels() {
        let lay = Layout::new(2);
        let idx = |b: Build| lay.pads.iter().position(|p| p.build == b).unwrap();
        let mut built = vec![false; lay.pads.len()];
        let base = Sim::new(2, &built, 0, 1);
        built[idx(Build::Shoes(1))] = true;
        built[idx(Build::Stack(1))] = true;
        let one = Sim::new(2, &built, 0, 1);
        built[idx(Build::Shoes(2))] = true;
        built[idx(Build::Stack(2))] = true;
        let two = Sim::new(2, &built, 0, 1);
        assert!(base.speed() < one.speed() && one.speed() < two.speed());
        assert!(base.capacity() < one.capacity() && one.capacity() < two.capacity());
    }

    #[test]
    fn guests_never_look_like_the_last_two() {
        let mut sim = Sim::new(4, &[], 0, 9);
        let looks: Vec<u8> = (0..300).map(|_| sim.pick_look()).collect();
        for w in looks.windows(3) {
            assert!(w[0] != w[1] && w[0] != w[2] && w[1] != w[2], "{w:?}");
        }
        for l in 0..LOOKS as u8 {
            assert!(looks.contains(&l));
        }
    }

    #[test]
    fn every_spot_is_reachable_with_nothing_and_everything_built() {
        for n in 1..=MAX_HOTEL {
            for all in [false, true] {
                let pads = Layout::new(n).pads.len();
                let sim = Sim::new(n, &vec![all; pads], 0, 1);
                let mut goals: Vec<Vec2> = sim
                    .visible_pads()
                    .iter()
                    .map(|i| sim.lay.pads[*i].pos)
                    .collect();
                goals.extend(PILES.iter().map(|p| sim.pile_pos(*p)));
                goals.push(sim.lay.desk_front);
                goals.push(SPAWN);
                if all {
                    for r in 0..sim.rooms.len() {
                        goals.extend(sim.spots(r).iter().map(|s| s.stand));
                    }
                    goals.extend(sim.lay.stall_front.iter().copied());
                    goals.push(sim.lay.paper_shelf);
                    goals.push(sim.lay.toilet_wait);
                    if sim.lay.has_cafe {
                        goals.push(sim.lay.counter_spot);
                        for t in &sim.lay.tables {
                            goals.extend(seat_spots(*t));
                        }
                    }
                    if sim.lay.has_pool {
                        goals.push(sim.lay.towel_shelf);
                    }
                    goals.extend(sim.lay.pads.iter().map(|p| p.pos));
                }
                for g in goals {
                    let path = sim.route(vec2(0.0, 1.2), g);
                    let mut at = vec2(0.0, 1.2);
                    for q in &path[..path.len() - 1] {
                        assert!(
                            sim.nav_line_free(at, *q),
                            "hotel {n} all={all}: {at} -> {q} on the way to {g} hits a wall"
                        );
                        at = *q;
                    }
                    assert!(
                        (*path.last().unwrap() - g).length() < 0.01,
                        "hotel {n}: no path to {g}"
                    );
                }
            }
        }
    }

    #[test]
    fn walking_a_route_never_snags_on_a_corner() {
        let mut sim = Sim::new(4, &[true; 40], 0, 1);
        sim.player.pos = vec2(1.1262901, 0.16499412);
        let goal = vec2(0.8, -20.0);
        for _ in 0..600 {
            sim.walk_toward(goal, 1.0 / 30.0, 1.0);
        }
        assert!(
            (sim.player.pos - goal).length() < 0.5,
            "stuck at {:?}",
            sim.player.pos
        );
    }

    #[test]
    fn walking_across_a_build_spot_spends_nothing() {
        let mut sim = Sim::new(1, &[], 500, 1);
        let pad = sim.lay.pads[0].pos;
        sim.player.pos = pad + vec2(-1.5, 0.0);
        for _ in 0..90 {
            sim.steer(Some(vec2(1.0, 0.0)), 1.0 / 30.0);
            sim.step(1.0 / 30.0);
        }
        assert!(sim.player.pos.x > pad.x + 1.0, "never crossed the spot");
        assert_eq!(sim.wallet, 500);
        assert_eq!(sim.paid[0], 0);
    }

    #[test]
    fn stopping_on_a_build_spot_pays_after_a_short_wait() {
        let mut sim = Sim::new(1, &[], 500, 1);
        sim.player.pos = sim.lay.pads[0].pos;
        run(&mut sim, PAD_ARM * 0.8, |s, dt| s.steer(None, dt));
        assert_eq!(sim.wallet, 500);
        run(&mut sim, PAD_ARM * 0.2 + PAD_TIME + 0.3, |s, dt| {
            s.steer(None, dt)
        });
        assert!(sim.built[0]);
    }

    #[test]
    fn standing_on_a_spot_without_cash_keeps_what_is_paid() {
        let mut sim = Sim::new(1, &[], 5, 1);
        sim.player.pos = sim.lay.pads[0].pos;
        run(&mut sim, PAD_ARM + 3.0, |s, dt| s.steer(None, dt));
        assert_eq!(sim.paid[0], 5);
        assert_eq!(sim.wallet, 0);
        assert!(!sim.built[0]);
    }

    #[test]
    fn hired_staff_run_the_hotel_without_the_player() {
        let pads = Layout::new(3).pads.len();
        let mut all = vec![true; pads];
        all[pads - 1] = false;
        let mut sim = Sim::new(3, &all, 0, 4);
        sim.player.pos = vec2(-4.0, 6.0);
        run(&mut sim, 400.0, |s, dt| s.steer(None, dt));
        assert_eq!(sim.maid_speed(), 2.0);
        assert!(sim.served > 15, "served {}", sim.served);
        assert!(
            sim.served > sim.angry * 2,
            "served {} angry {}",
            sim.served,
            sim.angry
        );
        assert!(sim.piles.iter().sum::<u32>() > 150, "piles {:?}", sim.piles);
    }

    #[test]
    fn swimmers_do_lengths_inside_the_pool() {
        let pool = Layout::new(3).pool;
        let mut xs = vec![];
        for k in 0..60 {
            let t = SWIM_TIME * k as f32 / 60.0;
            let p = swim_pos(&pool, t, 4);
            assert!(pool.contains(p), "{p} outside the pool");
            xs.push(p.x);
        }
        let span = xs.iter().cloned().fold(f32::MIN, f32::max)
            - xs.iter().cloned().fold(f32::MAX, f32::min);
        assert!(span > pool.w * 0.7, "swimmer only covers {span}");
    }
}
