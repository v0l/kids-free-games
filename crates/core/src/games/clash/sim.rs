use super::bots::Brain;
use super::powers::{Boosts, Kind, Power, MAGNET_REACH, SPEED_GAIN};
use crate::rng as rand;
use glam::{vec2, Vec2};
use std::f32::consts::{PI, TAU};

pub const NAMES: [&str; 48] = [
    "Mochi", "Zippy", "Bubbles", "Pixel", "Nugget", "Waffle", "Pickle", "Sunny", "Biscuit",
    "Peanut", "Comet", "Tango", "Doodle", "Ziggy", "Muffin", "Jelly", "Pogo", "Taco", "Bean",
    "Pip", "Fizz", "Cocoa", "Noodle", "Sprout", "Boing", "Turbo", "Gizmo", "Lemon", "Wiggles",
    "Honey", "Rocket", "Tofu", "Pancake", "Cricket", "Maple", "Poppy", "Sparky", "Twist", "Dash",
    "Moxie", "Olive", "Pebble", "Ripple", "Skip", "Toffee", "Yoyo", "Zest", "Dino",
];

pub fn xp_for_level(level: u32) -> u32 {
    let l = level.max(1);
    (l - 1) * (l + 2)
}

pub fn level_of(xp: u32) -> u32 {
    let mut l = ((((9.0 + 4.0 * xp as f64).sqrt() - 1.0) / 2.0) as u32).max(1);
    while l > 1 && xp_for_level(l) > xp {
        l -= 1;
    }
    while xp_for_level(l + 1) <= xp {
        l += 1;
    }
    l
}

pub fn level_progress(xp: u32) -> f32 {
    let l = level_of(xp);
    let lo = xp_for_level(l);
    (xp - lo) as f32 / (xp_for_level(l + 1) - lo) as f32
}

pub fn radius_for(level: u32) -> f32 {
    12.0 + (level.min(40) as f32).sqrt() * 3.2
}

pub fn speed_for(level: u32) -> f32 {
    175.0 + level.min(30) as f32 * 2.2
}

pub fn turn_for(level: u32) -> f32 {
    4.4 - level.min(30) as f32 * 0.05
}

pub fn segs_for(xp: u32) -> usize {
    (8.0 + (xp as f32).powf(0.78) * 0.8).min(110.0) as usize
}

pub fn xp_for_length(len: usize) -> u32 {
    let (mut lo, mut hi) = (0u32, 1u32 << 16);
    while lo < hi {
        let mid = (lo + hi).div_ceil(2);
        if segs_for(mid) <= len {
            lo = mid;
        } else {
            hi = mid - 1;
        }
    }
    lo
}

pub const NECK: usize = 8;
pub const WALL_PUSH: f32 = 30.0;
pub const WALL_STUN: f32 = 0.35;

pub fn wrap_angle(a: f32) -> f32 {
    (a + PI).rem_euclid(TAU) - PI
}

pub fn taper(i: usize, n: usize) -> f32 {
    1.0 - 0.42 * (i as f32 / n.max(2) as f32)
}

#[derive(Clone, Debug)]
pub struct Snake {
    pub id: u32,
    pub name: String,
    pub skin: usize,
    pub segs: Vec<Vec2>,
    pub angle: f32,
    pub want: f32,
    pub xp: u32,
    pub scale: f32,
    pub player: bool,
    pub boss: bool,
    pub alive: bool,
    pub shield: f32,
    pub stun: f32,
    pub kills: u32,
    pub pop: f32,
    pub phase: f32,
    pub age: f32,
    pub brain: Brain,
    pub boosts: Boosts,
    pub rival: Option<std::sync::Arc<crate::brain::conv::Policy>>,
    pub seat: Option<usize>,
    pub effort: crate::brain::Effort,
    pub blunder: (usize, f32),
}

impl Snake {
    pub fn head(&self) -> Vec2 {
        self.segs[0]
    }

    pub fn level(&self) -> u32 {
        level_of(self.xp)
    }

    pub fn radius(&self) -> f32 {
        radius_for(self.level()) * self.scale
    }

    pub fn dir(&self) -> Vec2 {
        vec2(self.angle.cos(), self.angle.sin())
    }

    pub fn speed(&self) -> f32 {
        let boost = if self.boosts.speed > 0.0 {
            SPEED_GAIN
        } else {
            1.0
        };
        speed_for(self.level()) * if self.boss { 0.92 } else { 1.0 } * boost
    }

    pub fn spacing(&self) -> f32 {
        self.radius() * 0.72
    }

    pub fn seg_radius(&self, i: usize) -> f32 {
        self.radius() * taper(i, self.segs.len())
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Egg {
    pub pos: Vec2,
    pub vel: Vec2,
    pub value: u32,
    pub kind: u8,
    pub age: f32,
}

#[derive(Clone, Copy, Debug)]
pub struct Rock {
    pub pos: Vec2,
    pub r: f32,
}

#[derive(Clone, Debug)]
pub enum Event {
    Egg {
        by: u32,
        at: Vec2,
        value: u32,
    },
    Level {
        id: u32,
        level: u32,
        at: Vec2,
    },
    Eaten {
        victim: u32,
        by: Option<u32>,
        at: Vec2,
        level: u32,
        skin: usize,
        player: bool,
        by_player: bool,
        boss: bool,
    },
    Bump {
        at: Vec2,
        player: bool,
    },
    Cut {
        victim: u32,
        by: u32,
        at: Vec2,
        lost: u32,
        player: bool,
        by_player: bool,
    },
    Power {
        id: u32,
        kind: Kind,
        at: Vec2,
        player: bool,
    },
}

#[derive(Clone, Debug)]
pub struct Spec {
    pub size: f32,
    pub bots: usize,
    pub eggs: usize,
    pub rocks: usize,
    pub aggression: f32,
    pub skill: f32,
    pub big_share: f32,
    pub lo: u32,
    pub hi: u32,
    pub adaptive: bool,
    pub start: u32,
    pub powerups: bool,
    pub rival: Option<std::sync::Arc<crate::brain::conv::Policy>>,
    pub seat: Option<usize>,
    pub rival_share: f32,
    pub respawn: bool,
    pub effort: crate::brain::Effort,
}

impl Spec {
    pub fn survival(player_level: u32, training: bool) -> Self {
        Self {
            respawn: false,
            ..Self::arena(player_level, training)
        }
    }

    pub fn arena(player_level: u32, training: bool) -> Self {
        let band = player_level / 5;
        let b = band as f32;
        Self {
            size: 3000.0,
            bots: (11 + band as usize).min(15),
            eggs: 300,
            rocks: 0,
            aggression: if training {
                0.0
            } else {
                (0.25 + 0.1 * b).min(0.9)
            },
            skill: if training {
                0.2
            } else {
                (0.4 + 0.1 * b).min(0.95)
            },
            big_share: if training {
                0.0
            } else {
                (0.15 + 0.07 * b).min(0.5)
            },
            lo: 1,
            hi: 4,
            adaptive: true,
            start: 1,
            powerups: true,
            rival: None,
            seat: None,
            rival_share: 0.0,
            respawn: true,
            effort: crate::brain::Effort::BEST,
        }
    }
}

pub struct World {
    pub size: f32,
    pub snakes: Vec<Snake>,
    pub eggs: Vec<Egg>,
    pub rocks: Vec<Rock>,
    pub events: Vec<Event>,
    pub spec: Spec,
    pub t: f32,
    pub player_eats: u32,
    pub boss_down: bool,
    pub powers: Vec<Power>,
    pub(super) power_clock: f32,
    respawns: Vec<f32>,
    next_id: u32,
    name_at: usize,
    egg_budget: f32,
    bump_cd: Vec<(u32, u32, f32)>,
    #[cfg(test)]
    pub hits: Vec<Hit>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    Bounce,
    Cut,
    Eat,
}

#[derive(Clone, Copy, Debug)]
pub struct Hit {
    pub biter: u32,
    pub bitten: u32,
    pub shielded: bool,
    pub seg: usize,
    pub outcome: Outcome,
}

impl World {
    pub fn new(spec: Spec, player_name: &str, skin: usize) -> Self {
        let mut w = Self {
            size: spec.size,
            snakes: vec![],
            eggs: vec![],
            rocks: vec![],
            events: vec![],
            t: 0.0,
            player_eats: 0,
            boss_down: false,
            powers: vec![],
            power_clock: 2.0,
            respawns: vec![],
            next_id: 1,
            name_at: rand::gen_range(0usize, NAMES.len()),
            egg_budget: 0.0,
            bump_cd: vec![],
            #[cfg(test)]
            hits: vec![],
            spec,
        };
        w.scatter_rocks();
        let mid = vec2(w.size / 2.0, w.size / 2.0);
        let start = w.spec.start;
        let p = w.add_snake(mid, start, player_name.to_string(), skin);
        w.snakes[p].player = true;
        w.snakes[p].shield = 5.0;
        w.snakes[p].angle = -PI / 2.0;
        w.snakes[p].want = -PI / 2.0;
        w.snakes[p].brain.skill = 1.0;
        w.snakes[p].brain.hunting = true;
        w.layout_tail(p);
        for _ in 0..w.spec.eggs {
            w.add_egg_random();
        }
        for _ in 0..w.spec.bots {
            w.spawn_bot();
        }
        w
    }

    fn scatter_rocks(&mut self) {
        for _ in 0..self.spec.rocks {
            for _ in 0..30 {
                let pos = self.random_point(260.0);
                let r = rand::gen_range(50.0, 90.0);
                let centre = vec2(self.size / 2.0, self.size / 2.0);
                let clear = self
                    .rocks
                    .iter()
                    .all(|o| (o.pos - pos).length() > o.r + r + 160.0);
                if clear && (pos - centre).length() > r + 260.0 {
                    self.rocks.push(Rock { pos, r });
                    break;
                }
            }
        }
    }

    pub fn player(&self) -> &Snake {
        &self.snakes[0]
    }

    pub fn near_wall(&self, p: Vec2) -> bool {
        let m = WALL_PUSH + radius_for(40) * 1.5;
        p.x < m || p.y < m || p.x > self.size - m || p.y > self.size - m
    }

    pub fn player_alive(&self) -> bool {
        self.snakes[0].alive
    }

    pub fn by_id(&self, id: u32) -> Option<&Snake> {
        self.snakes.iter().find(|s| s.id == id)
    }

    pub fn bots(&self) -> impl Iterator<Item = &Snake> {
        self.snakes.iter().filter(|s| !s.player && s.alive)
    }

    pub(super) fn random_point(&self, margin: f32) -> Vec2 {
        vec2(
            rand::gen_range(margin, self.size - margin),
            rand::gen_range(margin, self.size - margin),
        )
    }

    pub(super) fn near_rock(&self, p: Vec2, pad: f32) -> bool {
        self.rocks.iter().any(|r| (r.pos - p).length() < r.r + pad)
    }

    pub fn add_egg_random(&mut self) {
        for _ in 0..12 {
            let pos = self.random_point(60.0);
            if !self.near_rock(pos, 20.0) {
                let big = rand::gen_range(0.0, 1.0) < 0.07;
                self.eggs.push(Egg {
                    pos,
                    vel: Vec2::ZERO,
                    value: if big { 3 } else { 1 },
                    kind: rand::gen_range(0u8, 4),
                    age: rand::gen_range(0.0, 6.0),
                });
                return;
            }
        }
    }

    fn next_name(&mut self) -> String {
        for _ in 0..NAMES.len() {
            self.name_at = (self.name_at + 1) % NAMES.len();
            let n = NAMES[self.name_at];
            if !self.snakes.iter().any(|s| s.alive && s.name == n) {
                return n.to_string();
            }
        }
        NAMES[self.name_at].to_string()
    }

    pub fn add_snake(&mut self, pos: Vec2, level: u32, name: String, skin: usize) -> usize {
        let id = self.next_id;
        self.next_id += 1;
        let angle = rand::gen_range(-PI, PI);
        let lo = xp_for_level(level);
        let hi = xp_for_level(level + 1);
        let xp = lo + rand::gen_range(0, ((hi - lo) / 2).max(1));
        self.snakes.push(Snake {
            id,
            name,
            skin,
            segs: vec![pos],
            angle,
            want: angle,
            xp,
            scale: 1.0,
            player: false,
            boss: false,
            alive: true,
            shield: 0.0,
            stun: 0.0,
            kills: 0,
            pop: 0.0,
            phase: rand::gen_range(0.0, TAU),
            age: 0.0,
            brain: Brain::new(self.spec.skill, self.spec.aggression),
            boosts: Boosts::default(),
            rival: None,
            seat: None,
            effort: self.spec.effort,
            blunder: (0, 0.0),
        });
        let i = self.snakes.len() - 1;
        self.layout_tail(i);
        i
    }

    pub fn layout_tail(&mut self, i: usize) {
        let size = self.size;
        let s = &mut self.snakes[i];
        let n = segs_for(s.xp);
        let gap = s.spacing();
        let margin = s.radius() + gap;
        let mut at = s.segs[0];
        let mut back = wrap_angle(s.angle + PI);
        let mut segs = Vec::with_capacity(n);
        segs.push(at);
        let turn = gap / (margin * 2.0).max(1.0);
        let mut bend = if s.phase.sin() < 0.0 { -0.3 } else { 0.3 };
        for k in 1..n {
            if k % 9 == 0 {
                bend = -bend * 0.8 + (s.phase * k as f32).cos() * 0.12;
            }
            back = wrap_angle(back + bend * gap / 40.0);
            let mut next = at + vec2(back.cos(), back.sin()) * gap;
            let mut tries = 0;
            while (next.x < margin
                || next.y < margin
                || next.x > size - margin
                || next.y > size - margin)
                && tries < 64
            {
                back = wrap_angle(back + turn.max(0.15));
                next = at + vec2(back.cos(), back.sin()) * gap;
                tries += 1;
            }
            next.x = next.x.clamp(0.0, size);
            next.y = next.y.clamp(0.0, size);
            segs.push(next);
            at = next;
        }
        s.segs = segs;
    }

    pub fn level_for_new_bot(&self) -> u32 {
        let p = self.snakes[0].level();
        let big = rand::gen_range(0.0, 1.0) < self.spec.big_share;
        if self.spec.adaptive {
            let band = (p / 5) as i32;
            if big {
                (p as i32 + rand::gen_range(1, 4 + band)) as u32
            } else {
                (p as i32 + rand::gen_range(-4, 2)).max(1) as u32
            }
        } else if big {
            rand::gen_range((self.spec.lo + self.spec.hi) / 2, self.spec.hi + 1)
        } else {
            rand::gen_range(self.spec.lo, (self.spec.lo + self.spec.hi) / 2 + 1)
        }
    }

    pub fn free_spot(&self, away: f32) -> Vec2 {
        let me = self.snakes[0].head();
        let mut best = self.random_point(220.0);
        let mut best_d = 0.0;
        for _ in 0..40 {
            let p = self.random_point(220.0);
            if self.near_rock(p, 100.0) {
                continue;
            }
            let near_body = self
                .snakes
                .iter()
                .filter(|s| s.alive)
                .any(|s| s.segs.iter().step_by(3).any(|q| (*q - p).length() < 140.0));
            if near_body {
                continue;
            }
            let d = (p - me).length();
            if d >= away {
                return p;
            }
            if d > best_d {
                best_d = d;
                best = p;
            }
        }
        best
    }

    pub fn spawn_bot(&mut self) {
        let level = self.level_for_new_bot();
        let pos = self.free_spot(700.0);
        let name = self.next_name();
        let skin = rand::gen_range(0usize, super::skins::SKINS.len());
        let i = self.add_snake(pos, level, name, skin);
        self.snakes[i].shield = 1.2;
        let rivals = self.spec.rival.is_some() || self.spec.seat.is_some();
        if rivals && rand::gen_range(0.0, 1.0) < self.spec.rival_share {
            self.snakes[i].rival = self.spec.rival.clone();
            self.snakes[i].seat = self.spec.seat;
        }
    }

    pub fn spawn_boss(&mut self, level: u32, skin: usize) {
        let pos = self.free_spot(900.0);
        let i = self.add_snake(pos, level, "BOSS".to_string(), skin);
        let s = &mut self.snakes[i];
        s.boss = true;
        s.scale = 1.5;
        s.xp = xp_for_level(level);
        s.brain.skill = 0.9;
        s.brain.aggression = 1.0;
        self.layout_tail(i);
    }

    pub fn boss(&self) -> Option<&Snake> {
        self.snakes.iter().find(|s| s.boss && s.alive)
    }

    pub fn step(&mut self, dt: f32, steer: Option<f32>) {
        self.t += dt;
        for i in 0..self.snakes.len() {
            if !self.snakes[i].alive {
                continue;
            }
            if self.snakes[i].player {
                if let Some(a) = steer {
                    self.snakes[i].want = a;
                }
            } else if self.snakes[i].seat.is_some() {
            } else if let Some(net) = self.snakes[i].rival.clone() {
                let mut brain = std::mem::take(&mut self.snakes[i].brain);
                brain.timer -= dt;
                if brain.timer <= 0.0 {
                    let effort = self.snakes[i].effort;
                    brain.timer = effort.think_every(super::rl::DECIDE);
                    let k = self.policy_turn(i, &net, effort);
                    self.snakes[i].want = wrap_angle(self.snakes[i].angle + super::rl::TURNS[k]);
                }
                self.snakes[i].brain = brain;
            } else {
                let mut brain = std::mem::take(&mut self.snakes[i].brain);
                brain.timer -= dt;
                if brain.timer <= 0.0 {
                    brain.timer = 0.1 + (1.0 - brain.skill) * 0.25;
                    self.snakes[i].want = self.think(i, &mut brain);
                }
                self.snakes[i].brain = brain;
            }
            self.advance(i, dt);
        }
        self.collect_eggs(dt);
        self.tick_powers(dt);
        self.collisions(dt);
        self.refill(dt);
        self.respawn(dt);
        self.snakes.retain(|s| s.alive || s.player);
    }

    fn advance(&mut self, i: usize, dt: f32) {
        let size = self.size;
        let mut wall_hit = None;
        let rocks = self.rocks.clone();
        let s = &mut self.snakes[i];
        s.age += dt;
        s.shield = (s.shield - dt).max(0.0);
        s.pop = (s.pop - dt * 3.0).max(0.0);
        let level = s.level();
        let turn = turn_for(level) * dt;
        if s.stun > 0.0 {
            s.stun -= dt;
        } else {
            let d = wrap_angle(s.want - s.angle);
            s.angle = wrap_angle(s.angle + d.clamp(-turn, turn));
        }
        let sp = s.speed() * if s.stun > 0.0 { 0.5 } else { 1.0 };
        let r = s.radius();
        let mut head = s.segs[0] + s.dir() * sp * dt;
        let mut dir = s.dir();
        let mut hit = false;
        for (p, d) in [(&mut head.x, &mut dir.x), (&mut head.y, &mut dir.y)] {
            if *p < r {
                *p = r + WALL_PUSH;
                *d = d.abs();
                hit = true;
            } else if *p > size - r {
                *p = size - r - WALL_PUSH;
                *d = -d.abs();
                hit = true;
            }
        }
        if hit {
            s.angle = dir.y.atan2(dir.x);
            s.want = s.angle;
            s.stun = s.stun.max(WALL_STUN);
            wall_hit = Some((head, s.player));
        }
        for k in &rocks {
            let d = head - k.pos;
            let min = k.r + r * 0.8;
            if d.length() < min {
                head = k.pos + d.normalize_or_zero() * min;
            }
        }
        s.segs[0] = head;
        let gap = s.spacing();
        let dir = s.dir();
        for k in 1..s.segs.len() {
            let prev = s.segs[k - 1];
            let d = s.segs[k] - prev;
            let len = d.length();
            if len > gap {
                s.segs[k] = prev + d / len * gap;
            } else if len < 1e-3 {
                s.segs[k] = prev - dir * gap;
            }
            let p = &mut s.segs[k];
            p.x = p.x.clamp(0.0, size);
            p.y = p.y.clamp(0.0, size);
        }
        let want = segs_for(s.xp);
        while s.segs.len() < want {
            let tail = *s.segs.last().unwrap();
            s.segs.push(tail);
        }
        if let Some((at, player)) = wall_hit {
            self.events.push(Event::Bump { at, player });
        }
    }

    pub(super) fn gain(&mut self, i: usize, amount: u32) {
        let s = &mut self.snakes[i];
        if s.boss {
            return;
        }
        let before = s.level();
        s.xp += amount;
        s.pop = 1.0;
        let after = s.level();
        if after > before {
            self.events.push(Event::Level {
                id: s.id,
                level: after,
                at: s.head(),
            });
        }
    }

    fn collect_eggs(&mut self, dt: f32) {
        for e in &mut self.eggs {
            e.age += dt;
            if e.vel.length_squared() > 1.0 {
                e.pos += e.vel * dt;
                e.vel *= (1.0 - 4.0 * dt).max(0.0);
                e.pos.x = e.pos.x.clamp(20.0, self.size - 20.0);
                e.pos.y = e.pos.y.clamp(20.0, self.size - 20.0);
            }
        }
        for i in 0..self.snakes.len() {
            if !self.snakes[i].alive {
                continue;
            }
            let head = self.snakes[i].head();
            let r = self.snakes[i].radius();
            let reach = r + 8.0;
            let magnet = (55.0 + r * 1.6)
                * if self.snakes[i].boosts.magnet > 0.0 {
                    MAGNET_REACH
                } else {
                    1.0
                };
            let pull_speed = self.snakes[i].speed() * 1.8;
            let mut got = 0;
            let mut k = 0;
            let far = magnet.max(reach);
            while k < self.eggs.len() {
                let d = self.eggs[k].pos - head;
                if d.x.abs() >= far || d.y.abs() >= far {
                    k += 1;
                    continue;
                }
                let len = d.length();
                if len < reach {
                    got += self.eggs[k].value;
                    let e = self.eggs.swap_remove(k);
                    self.events.push(Event::Egg {
                        by: self.snakes[i].id,
                        at: e.pos,
                        value: e.value,
                    });
                    continue;
                }
                if len < magnet {
                    let pull = pull_speed + 420.0 * (1.0 - len / magnet);
                    self.eggs[k].pos -= d / len * (pull * dt).min(len);
                }
                k += 1;
            }
            if got > 0 {
                self.gain(i, got);
            }
        }
    }

    fn contact(&self, a: usize, b: usize) -> Option<(usize, Vec2)> {
        let sa = &self.snakes[a];
        let sb = &self.snakes[b];
        let ha = sa.head();
        let ra = sa.radius() * 0.8;
        let rb = sb.radius() * 0.85;
        let n = sb.segs.len();
        let body = sb.spacing() * n as f32 + rb;
        if (ha - sb.head()).length_squared() > (body + ra) * (body + ra) {
            return None;
        }
        let step = sb.spacing().max(1e-3);
        let most = ra + rb;
        let mut j = 0;
        while j < n {
            let p = sb.segs[j];
            let d2 = (ha - p).length_squared();
            let reach = ra + rb * taper(j, n);
            if d2 < reach * reach {
                return Some((j, p));
            }
            let skip = (d2.sqrt() - most) / step;
            j += if skip > 1.0 { skip as usize } else { 1 };
        }
        None
    }

    fn collisions(&mut self, dt: f32) {
        for c in &mut self.bump_cd {
            c.2 -= dt;
        }
        self.bump_cd.retain(|c| c.2 > 0.0);
        let n = self.snakes.len();
        let frame: Vec<(u32, bool)> = self
            .snakes
            .iter()
            .map(|s| (s.level(), s.shield > 0.0))
            .collect();
        for a in 0..n {
            for b in 0..n {
                if a == b || !self.snakes[a].alive || !self.snakes[b].alive {
                    continue;
                }
                let Some((seg, at)) = self.contact(a, b) else {
                    continue;
                };
                let ((la, sa), (lb, sb)) = (frame[a], frame[b]);
                let shielded = sa || sb;
                let outcome = if la == lb || shielded || (la < lb && seg >= NECK) {
                    Outcome::Bounce
                } else if la > lb && seg >= NECK {
                    Outcome::Cut
                } else {
                    Outcome::Eat
                };
                #[cfg(test)]
                self.hits.push(Hit {
                    biter: if la < lb && outcome == Outcome::Eat {
                        lb
                    } else {
                        la
                    },
                    bitten: if la < lb && outcome == Outcome::Eat {
                        la
                    } else {
                        lb
                    },
                    shielded,
                    seg,
                    outcome,
                });
                match outcome {
                    Outcome::Bounce => self.bump(a, b, at, seg),
                    Outcome::Cut => self.cut(b, a, seg),
                    Outcome::Eat if la > lb => self.kill(b, Some(a)),
                    Outcome::Eat => self.kill(a, Some(b)),
                }
            }
        }
    }

    fn bump(&mut self, a: usize, b: usize, at: Vec2, seg: usize) {
        let key = (
            self.snakes[a].id.min(self.snakes[b].id),
            self.snakes[a].id.max(self.snakes[b].id),
        );
        let fresh = !self.bump_cd.iter().any(|c| (c.0, c.1) == key);
        if fresh {
            self.bump_cd.push((key.0, key.1, 0.5));
        }
        let ha = self.snakes[a].head();
        let hb = self.snakes[b].head();
        let body = &self.snakes[b].segs;
        let along = if body.len() > 1 {
            let lo = seg.saturating_sub(1);
            let hi = (seg + 1).min(body.len() - 1);
            (body[lo] - body[hi]).normalize_or_zero()
        } else {
            Vec2::ZERO
        };
        let side = vec2(-along.y, along.x);
        let off = ha - at;
        let away = if seg >= NECK && side != Vec2::ZERO {
            if side.dot(off) >= 0.0 {
                side
            } else {
                -side
            }
        } else {
            off.normalize_or_zero()
        };
        let away = if away == Vec2::ZERO {
            (ha - hb).normalize_or_zero()
        } else {
            away
        };
        let reach = self.snakes[a].radius() * 0.8 + self.snakes[b].radius() * 0.85;
        let push = (reach - off.dot(away)).max(0.0) + 6.0;
        let size = self.size;
        let s = &mut self.snakes[a];
        let mut h = s.segs[0] + away * push;
        h.x = h.x.clamp(5.0, size - 5.0);
        h.y = h.y.clamp(5.0, size - 5.0);
        s.segs[0] = h;
        if !fresh {
            return;
        }
        s.angle = away.y.atan2(away.x);
        s.want = s.angle;
        s.stun = 0.35;
        let player = s.player || self.snakes[b].player;
        self.events.push(Event::Bump { at, player });
    }

    fn cut(&mut self, victim: usize, by: usize, seg: usize) {
        let s = &self.snakes[victim];
        let seg = seg.clamp(NECK, s.segs.len());
        let lost = s.xp - xp_for_length(seg).min(s.xp);
        if lost == 0 {
            return;
        }
        let at = s.segs[seg];
        let tail: Vec<Vec2> = s.segs[seg..].to_vec();
        let by_player = self.snakes[by].player;
        let by_id = self.snakes[by].id;
        let s = &mut self.snakes[victim];
        s.xp -= lost;
        s.segs.truncate(seg);
        s.pop = 1.0;
        self.events.push(Event::Cut {
            victim: s.id,
            by: by_id,
            at,
            lost,
            player: s.player,
            by_player,
        });
        self.drop_trail(&tail, lost / 2 + 1);
    }

    fn drop_trail(&mut self, segs: &[Vec2], total: u32) {
        let drops = (segs.len() / 2).clamp(3, 40) as u32;
        let each = (total / drops).max(1);
        let mut left = total;
        for k in 0..drops {
            if left == 0 {
                break;
            }
            let value = each.min(left);
            left -= value;
            let at = segs[(k as usize * segs.len() / drops as usize).min(segs.len() - 1)];
            let a = rand::gen_range(0.0, TAU);
            self.eggs.push(Egg {
                pos: at,
                vel: vec2(a.cos(), a.sin()) * rand::gen_range(60.0, 200.0),
                value,
                kind: rand::gen_range(0u8, 4),
                age: 0.0,
            });
        }
    }

    pub fn kill(&mut self, victim: usize, by: Option<usize>) {
        let v = self.snakes[victim].clone();
        self.snakes[victim].alive = false;
        let by_player = by.is_some_and(|i| self.snakes[i].player);
        self.events.push(Event::Eaten {
            victim: v.id,
            by: by.map(|i| self.snakes[i].id),
            at: v.head(),
            level: v.level(),
            skin: v.skin,
            player: v.player,
            by_player,
            boss: v.boss,
        });
        if let Some(i) = by {
            self.snakes[i].kills += 1;
            self.gain(i, v.level() * 2 + 2);
            if by_player {
                self.player_eats += 1;
                if v.boss {
                    self.boss_down = true;
                }
            }
        }
        self.drop_trail(&v.segs, v.xp / 2 + 2);
        if !v.player && !v.boss {
            self.respawns.push(rand::gen_range(2.0, 4.0));
        }
    }

    fn refill(&mut self, dt: f32) {
        if self.eggs.len() >= self.spec.eggs {
            self.egg_budget = 0.0;
            return;
        }
        self.egg_budget += dt * 25.0;
        while self.egg_budget >= 1.0 {
            self.egg_budget -= 1.0;
            self.add_egg_random();
        }
    }

    fn policy_turn(
        &mut self,
        i: usize,
        net: &crate::brain::conv::Policy,
        effort: crate::brain::Effort,
    ) -> usize {
        let (held, left) = self.snakes[i].blunder;
        if left > 0.0 {
            self.snakes[i].blunder.1 -= effort.think_every(super::rl::DECIDE);
            return held;
        }
        if rand::gen_range(0.0, 1.0) < effort.blunder() {
            let k = rand::gen_range(0, super::rl::ACTIONS);
            self.snakes[i].blunder = (k, rand::gen_range(0.4, 1.2));
            return k;
        }
        net.choose(&super::rl::observe(self, i), effort)
    }

    pub fn snakes_left(&self) -> usize {
        self.snakes.iter().filter(|s| s.alive).count()
    }

    pub fn won(&self) -> bool {
        !self.spec.respawn && self.player_alive() && self.snakes_left() == 1
    }

    fn respawn(&mut self, dt: f32) {
        if !self.spec.respawn {
            self.respawns.clear();
            return;
        }
        for t in &mut self.respawns {
            *t -= dt;
        }
        let ready = self.respawns.iter().filter(|t| **t <= 0.0).count();
        self.respawns.retain(|t| *t > 0.0);
        for _ in 0..ready {
            let alive = self.bots().filter(|s| !s.boss).count();
            if alive < self.spec.bots {
                self.spawn_bot();
            }
        }
    }

    pub fn autopilot(&mut self) -> f32 {
        let mut brain = std::mem::take(&mut self.snakes[0].brain);
        let a = self.think(0, &mut brain);
        self.snakes[0].brain = brain;
        a
    }

    pub fn nearest_threat(&self, from: Vec2, level: u32) -> Option<(f32, usize)> {
        self.snakes
            .iter()
            .enumerate()
            .filter(|(_, s)| s.alive && s.level() > level)
            .map(|(i, s)| ((s.head() - from).length(), i))
            .min_by(|a, b| a.0.total_cmp(&b.0))
    }

    pub fn leaderboard(&self) -> Vec<(u32, String, u32, bool)> {
        self.leaderboard_for(0)
    }

    pub fn leaderboard_for(&self, me: usize) -> Vec<(u32, String, u32, bool)> {
        let mut v: Vec<_> = self
            .snakes
            .iter()
            .enumerate()
            .filter(|(_, s)| s.alive)
            .map(|(k, s)| (s.id, s.name.clone(), s.xp, k == me))
            .collect();
        v.sort_by_key(|e| std::cmp::Reverse(e.2));
        v
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn quiet() -> Spec {
        Spec {
            bots: 0,
            eggs: 0,
            ..Spec::arena(1, true)
        }
    }

    fn straight(w: &mut World, i: usize) {
        let s = &mut w.snakes[i];
        let n = segs_for(s.xp);
        let back = -s.dir() * s.spacing();
        let head = s.segs[0];
        s.segs = (0..n).map(|k| head + back * k as f32).collect();
    }

    fn duel(a_level: u32, b_level: u32) -> World {
        let mut w = World::new(quiet(), "Me", 0);
        w.snakes[0].shield = 0.0;
        w.snakes[0].xp = xp_for_level(a_level);
        w.snakes[0].segs[0] = vec2(1000.0, 1000.0);
        w.snakes[0].angle = 0.0;
        w.snakes[0].want = 0.0;
        straight(&mut w, 0);
        let i = w.add_snake(vec2(1022.0, 1000.0), b_level, "Rival".into(), 1);
        w.snakes[i].xp = xp_for_level(b_level);
        w.snakes[i].angle = PI / 2.0;
        w.snakes[i].want = PI / 2.0;
        straight(&mut w, i);
        w
    }

    #[test]
    fn level_of_matches_counting_up_for_every_xp() {
        let mut want = 1;
        for xp in 0..200_000u32 {
            while xp_for_level(want + 1) <= xp {
                want += 1;
            }
            assert_eq!(level_of(xp), want, "xp {xp}");
        }
    }

    #[test]
    fn a_long_snake_spawned_by_a_wall_keeps_its_whole_body_on_the_map() {
        let mut w = World::new(quiet(), "Me", 0);
        let i = w.add_snake(vec2(230.0, 1500.0), 30, "Long".into(), 0);
        w.snakes[i].angle = 0.0;
        w.snakes[i].want = 0.0;
        w.layout_tail(i);
        let s = &w.snakes[i];
        assert!(s.segs.len() > 40);
        let size = w.size;
        let off = s
            .segs
            .iter()
            .filter(|p| p.x < 0.0 || p.y < 0.0 || p.x > size || p.y > size)
            .count();
        assert_eq!(off, 0, "{off} of {} segments off the map", s.segs.len());
        let gap = s.spacing();
        for k in 1..s.segs.len() {
            let d = (s.segs[k] - s.segs[k - 1]).length();
            assert!(
                (d - gap).abs() < 1.0,
                "segment {k} is {d} from the last, spacing {gap}"
            );
        }
    }

    #[test]
    fn a_new_snake_spawns_curled_not_as_a_straight_line() {
        let mut w = World::new(quiet(), "Me", 0);
        let i = w.add_snake(vec2(1500.0, 1500.0), 20, "Wiggle".into(), 0);
        let s = &w.snakes[i];
        assert!(s.segs.len() > 20);
        let first = (s.segs[1] - s.segs[0]).normalize();
        let last = (s.segs[s.segs.len() - 1] - s.segs[s.segs.len() - 2]).normalize();
        assert!(first.dot(last) < 0.99, "body is a straight line");
    }

    #[test]
    fn no_snake_ever_has_segments_off_the_map() {
        for seed in 0..2u64 {
            crate::rng::srand(seed);
            let (spec, _) = super::super::rl::training_spec(seed * 5 + 4);
            let mut w = World::new(spec, "Me", 0);
            for _ in 0..3000 {
                let a = w.autopilot();
                w.step(1.0 / 30.0, Some(a));
                let size = w.size;
                for s in w.snakes.iter().filter(|s| s.alive) {
                    assert!(
                        s.segs
                            .iter()
                            .all(|p| p.x >= 0.0 && p.y >= 0.0 && p.x <= size && p.y <= size),
                        "seed {seed}: {} has segments off the map",
                        s.name
                    );
                }
            }
        }
    }

    #[test]
    fn in_survival_nobody_respawns_and_the_last_snake_wins() {
        crate::rng::srand(9);
        let mut w = World::new(Spec::survival(5, false), "Me", 0);
        w.snakes[0].shield = 1e9;
        let start = w.snakes_left();
        assert!(start > 5 && !w.won());
        let ids: Vec<u32> = w.snakes.iter().map(|s| s.id).collect();
        for i in 1..w.snakes.len() {
            w.kill(i, None);
        }
        for _ in 0..30 * 10 {
            w.step(1.0 / 30.0, None);
        }
        assert!(
            w.snakes.iter().all(|s| ids.contains(&s.id)),
            "nobody new joined"
        );
        assert_eq!(w.snakes_left(), 1);
        assert!(w.won());
    }

    #[test]
    fn arena_mode_keeps_refilling_and_never_wins() {
        crate::rng::srand(9);
        let mut w = World::new(Spec::arena(5, false), "Me", 0);
        w.snakes[0].shield = 1e9;
        for i in 1..w.snakes.len() {
            w.kill(i, None);
        }
        for _ in 0..30 * 10 {
            w.step(1.0 / 30.0, None);
        }
        assert!(w.snakes_left() > 5);
        assert!(!w.won());
    }

    #[test]
    fn a_low_effort_policy_snake_blunders_and_thinks_slowly() {
        let lazy = crate::brain::Effort::new(0.0);
        let keen = crate::brain::Effort::BEST;
        assert_eq!(keen.blunder(), 0.0);
        assert!(lazy.blunder() > 0.1);
        assert!(
            lazy.think_every(super::super::rl::DECIDE)
                >= 3.0 * keen.think_every(super::super::rl::DECIDE)
        );
    }

    #[test]
    fn the_bigger_snake_eats_the_smaller_on_head_contact() {
        let mut w = duel(5, 2);
        w.step(1.0 / 60.0, None);
        assert!(w.player_alive());
        assert!(w.by_id(2).is_none());
        assert_eq!(w.player_eats, 1);
        assert!(w.events.iter().any(|e| matches!(
            e,
            Event::Eaten {
                by_player: true,
                ..
            }
        )));
        assert!(w.eggs.len() >= 3);
    }

    fn tail_duel(a_level: u32, b_level: u32, seg: usize) -> World {
        let mut w = duel(a_level, b_level);
        let i = w.snakes.len() - 1;
        let target = w.snakes[i].segs[seg];
        let sp = w.snakes[0].spacing();
        w.snakes[0].segs[0] = target - vec2(w.snakes[0].radius() * 0.5, 0.0);
        let head = w.snakes[0].segs[0];
        for k in 1..w.snakes[0].segs.len() {
            w.snakes[0].segs[k] = head - vec2(sp * k as f32, 0.0);
        }
        w
    }

    #[test]
    fn biting_a_tail_cuts_it_off_and_nobody_dies() {
        let mut w = tail_duel(8, 3, 10);
        let (xp, len) = (w.snakes[1].xp, w.snakes[1].segs.len());
        assert!(len > 10);
        w.step(1.0 / 60.0, None);
        let v = &w.snakes[1];
        assert!(v.alive && w.player_alive());
        assert!(v.xp < xp && v.xp > 0, "lost some xp: {} -> {}", xp, v.xp);
        assert!(v.segs.len() <= 10 && v.segs.len() < len);
        assert_eq!(v.shield, 0.0, "a cut snake gets no shield");
        assert!(w.events.iter().any(|e| matches!(
            e,
            Event::Cut {
                by_player: true,
                ..
            }
        )));
        assert!(!w.events.iter().any(|e| matches!(e, Event::Eaten { .. })));
    }

    #[test]
    fn a_bite_splits_the_snake_at_the_bitten_segment() {
        for bite in [10, 20, 30] {
            let mut w = tail_duel(12, 9, bite);
            let len = w.snakes[1].segs.len();
            assert!(len > 30, "victim long enough: {len}");
            w.step(1.0 / 60.0, None);
            w.snakes[0].alive = false;
            w.eggs.clear();
            for _ in 0..10 {
                w.step(1.0 / 60.0, None);
            }
            let v = &w.snakes[1];
            assert!(
                v.segs.len().abs_diff(bite) <= 1,
                "bite at {bite} of {len} left {} segments",
                v.segs.len()
            );
            assert!(segs_for(v.xp) <= bite + 1);
        }
    }

    #[test]
    fn a_shielded_snake_bounces_off_a_body_instead_of_crossing_it() {
        let mut w = tail_duel(3, 8, 20);
        w.snakes[0].shield = 3.0;
        w.step(1.0 / 60.0, None);
        assert!(w
            .events
            .iter()
            .any(|e| matches!(e, Event::Bump { player: true, .. })));
        let head = w.snakes[0].head();
        let r = w.snakes[0].radius() * 0.5 + w.snakes[1].radius() * 0.5;
        let inside = w.snakes[1].segs.iter().any(|p| (*p - head).length() < r);
        assert!(!inside, "head ended up inside the other body");
    }

    #[test]
    fn a_cut_snake_can_be_bitten_again_right_away() {
        let mut w = tail_duel(12, 9, 30);
        w.step(1.0 / 60.0, None);
        assert!(w.events.iter().any(|e| matches!(e, Event::Cut { .. })));
        let len = w.snakes[1].segs.len();
        w.snakes[0].segs[0] = w.snakes[1].segs[15];
        w.events.clear();
        w.step(1.0 / 60.0, None);
        assert!(w.events.iter().any(|e| matches!(e, Event::Cut { .. })));
        assert!(w.snakes[1].segs.len() < len);
    }

    #[test]
    fn a_cut_tail_drops_a_string_of_eggs_like_a_kill_does() {
        let mut w = tail_duel(12, 9, 10);
        w.eggs.clear();
        let (xp, tail) = (w.snakes[1].xp, w.snakes[1].segs[10..].to_vec());
        w.step(1.0 / 60.0, None);
        let lost = xp - w.snakes[1].xp;
        let eaten: u32 = w
            .events
            .iter()
            .map(|e| match e {
                Event::Egg { value, .. } => *value,
                _ => 0,
            })
            .sum();
        let laid: u32 = w.eggs.iter().map(|e| e.value).sum::<u32>() + eaten;
        let drops = (tail.len() / 2).clamp(3, 40) as u32;
        let want = lost / 2 + 1;
        assert!(laid <= want && laid + drops > want, "laid {laid} of {want}");
        assert!(w.eggs.len() as u32 >= drops - 2);
        let start = tail[0];
        let end = *tail.last().unwrap();
        let spread = w
            .eggs
            .iter()
            .map(|e| (e.pos - start).length())
            .fold(0.0, f32::max);
        assert!(
            spread > (end - start).length() * 0.5,
            "eggs string along the cut tail"
        );
    }

    #[test]
    fn a_bigger_snake_biting_the_players_tail_does_not_end_the_run() {
        let mut w = tail_duel(8, 3, 10);
        w.snakes[0].player = false;
        w.snakes[1].player = true;
        w.snakes[1].shield = 0.0;
        w.step(1.0 / 60.0, None);
        assert!(w.snakes[1].alive);
        assert!(w
            .events
            .iter()
            .any(|e| matches!(e, Event::Cut { player: true, .. })));
    }

    #[test]
    fn hitting_the_neck_still_eats_the_whole_snake() {
        let mut w = tail_duel(8, 3, 2);
        w.step(1.0 / 60.0, None);
        assert!(w.by_id(2).is_none());
    }

    #[test]
    fn a_snake_driving_into_a_wall_bounces_back_into_the_arena() {
        let mut w = World::new(quiet(), "Me", 0);
        w.snakes.truncate(1);
        w.snakes[0].shield = 0.0;
        w.snakes[0].segs[0] = vec2(w.size - 40.0, 1000.0);
        w.snakes[0].angle = 0.0;
        w.layout_tail(0);
        for _ in 0..30 {
            w.step(1.0 / 30.0, Some(0.0));
        }
        let s = &w.snakes[0];
        assert!(
            s.head().x < w.size - s.radius() - 20.0,
            "pushed off the wall: {}",
            s.head().x
        );
        assert!(w
            .events
            .iter()
            .any(|e| matches!(e, Event::Bump { player: true, .. })));
    }

    #[test]
    fn a_snake_cannot_park_in_a_corner() {
        let mut w = World::new(quiet(), "Me", 0);
        w.snakes.truncate(1);
        w.snakes[0].segs[0] = vec2(60.0, 60.0);
        w.snakes[0].angle = -2.4;
        w.layout_tail(0);
        let mut near = 0;
        for _ in 0..300 {
            w.step(1.0 / 30.0, Some(-2.4));
            let h = w.snakes[0].head();
            if h.x < 80.0 && h.y < 80.0 {
                near += 1;
            }
        }
        assert!(near < 150, "frames hugging the corner: {near}/300");
    }

    #[test]
    fn a_bigger_snake_never_bounces_off_a_smaller_open_one() {
        for seg in [0, 2, 5, 10, 20] {
            let mut w = tail_duel(12, 6, seg);
            w.snakes[1].shield = 0.0;
            w.step(1.0 / 60.0, None);
            assert!(
                !w.events.iter().any(|e| matches!(e, Event::Bump { .. })),
                "hit at segment {seg} bounced"
            );
            assert!(
                w.events
                    .iter()
                    .any(|e| matches!(e, Event::Cut { .. } | Event::Eaten { .. })),
                "hit at segment {seg} did nothing"
            );
        }
    }

    #[test]
    fn a_shielded_snake_is_never_cut_or_eaten() {
        for seg in [0, 2, 5, 10, 20] {
            let mut w = tail_duel(12, 6, seg);
            w.snakes[1].shield = 2.0;
            let xp = w.snakes[1].xp;
            for _ in 0..20 {
                w.step(1.0 / 60.0, None);
                assert!(
                    !w.events
                        .iter()
                        .any(|e| matches!(e, Event::Cut { .. } | Event::Eaten { .. })),
                    "segment {seg}: shielded snake hurt"
                );
            }
            assert!(w.snakes[1].alive && w.snakes[1].xp >= xp);
        }
    }

    #[test]
    fn a_shielded_small_snake_ramming_a_big_head_survives() {
        let mut w = duel(3, 12);
        w.snakes[0].shield = 2.0;
        for _ in 0..30 {
            w.step(1.0 / 60.0, None);
        }
        assert!(w.player_alive());
        assert!(!w
            .events
            .iter()
            .any(|e| matches!(e, Event::Eaten { .. } | Event::Cut { .. })));
    }

    #[test]
    fn collisions_follow_the_rules_in_real_games() {
        let mut checked = 0;
        for seed in 0..4u64 {
            crate::rng::srand(seed);
            let mut w = World::new(Spec::arena(10, false), "Me", 0);
            w.snakes[0].shield = 1e9;
            for _ in 0..60 * 60 {
                w.step(1.0 / 60.0, None);
                w.events.clear();
                for h in std::mem::take(&mut w.hits) {
                    checked += 1;
                    match h.outcome {
                        Outcome::Cut | Outcome::Eat => {
                            assert!(!h.shielded, "seed {seed}: shielded snake hurt: {h:?}");
                            assert!(h.biter > h.bitten, "seed {seed}: {h:?}");
                        }
                        Outcome::Bounce => {
                            assert!(
                                h.shielded || h.biter <= h.bitten,
                                "seed {seed}: a bigger snake bounced off an open smaller one: {h:?}"
                            );
                        }
                    }
                }
            }
        }
        assert!(checked > 50, "only {checked} hits");
    }

    #[test]
    fn running_into_a_bigger_snakes_tail_bounces_off() {
        let mut w = tail_duel(3, 12, 20);
        let xp = w.snakes[0].xp;
        w.step(1.0 / 60.0, None);
        assert!(w.player_alive());
        assert_eq!(w.snakes[0].xp, xp);
        assert!(w
            .events
            .iter()
            .any(|e| matches!(e, Event::Bump { player: true, .. })));
        assert!(!w
            .events
            .iter()
            .any(|e| matches!(e, Event::Eaten { .. } | Event::Cut { .. })));
    }

    #[test]
    fn running_into_a_bigger_snake_ends_the_run() {
        let mut w = duel(2, 5);
        w.step(1.0 / 60.0, None);
        assert!(!w.player_alive());
        assert!(w
            .events
            .iter()
            .any(|e| matches!(e, Event::Eaten { player: true, .. })));
    }

    #[test]
    fn equal_levels_bump_apart_and_nobody_dies() {
        let mut w = duel(3, 3);
        let before = w.snakes[0].head();
        w.step(1.0 / 60.0, None);
        assert!(w.player_alive() && w.snakes[1].alive);
        assert!(w.events.iter().any(|e| matches!(e, Event::Bump { .. })));
        assert!((w.snakes[0].head() - before).length() > 5.0);
        assert!(w.snakes[0].stun > 0.0);
    }

    #[test]
    fn a_spawn_shield_protects_a_small_snake_for_a_moment() {
        let mut w = duel(2, 5);
        w.snakes[0].shield = 2.0;
        w.step(1.0 / 60.0, None);
        assert!(w.player_alive());
    }

    #[test]
    fn eating_eggs_grows_levels_and_length() {
        let mut w = World::new(quiet(), "Me", 0);
        let head = w.snakes[0].head();
        let len = w.snakes[0].segs.len();
        for k in 0..30 {
            w.eggs.push(Egg {
                pos: head + vec2(30.0 + k as f32 * 8.0, 0.0),
                vel: Vec2::ZERO,
                value: 1,
                kind: 0,
                age: 0.0,
            });
        }
        w.snakes[0].angle = 0.0;
        w.snakes[0].want = 0.0;
        for _ in 0..120 {
            w.step(1.0 / 60.0, Some(0.0));
        }
        assert!(w.snakes[0].level() >= 4);
        assert!(w.snakes[0].segs.len() > len);
        assert!(w.events.iter().any(|e| matches!(e, Event::Level { .. })));
    }

    #[test]
    fn an_egg_beside_the_head_is_pulled_in_faster_than_the_snake_moves() {
        for level in [1, 10, 30] {
            let mut w = World::new(quiet(), "Me", 0);
            w.snakes[0].xp = xp_for_level(level);
            w.snakes[0].segs[0] = vec2(1000.0, 1000.0);
            w.snakes[0].angle = 0.0;
            w.snakes[0].want = 0.0;
            w.layout_tail(0);
            let r = w.snakes[0].radius();
            for side in [-1.0, 1.0] {
                w.eggs.push(Egg {
                    pos: vec2(1000.0 - r, 1000.0 + side * (r + 14.0)),
                    vel: Vec2::ZERO,
                    value: 1,
                    kind: 0,
                    age: 0.0,
                });
            }
            for _ in 0..30 {
                w.step(1.0 / 60.0, Some(0.0));
            }
            assert!(
                w.eggs.is_empty(),
                "level {level}: {} eggs trailing the head",
                w.eggs.len()
            );
        }
    }

    #[test]
    fn levels_come_slower_and_bigger_snakes_are_faster_and_wider() {
        for l in 1..30 {
            assert!(
                xp_for_level(l + 2) - xp_for_level(l + 1) > xp_for_level(l + 1) - xp_for_level(l)
            );
            assert!(speed_for(l + 1) > speed_for(l));
            assert!(radius_for(l + 1) > radius_for(l));
            assert_eq!(level_of(xp_for_level(l)), l);
            assert_eq!(level_of(xp_for_level(l + 1) - 1), l);
        }
    }

    #[test]
    fn bots_far_from_the_player_travel_instead_of_circling() {
        for training in [true, false] {
            let mut loops = 0;
            let mut straightness = vec![];
            for seed in 1..6 {
                crate::rng::srand(seed);
                let mut w = World::new(Spec::arena(1, training), "Me", 0);
                let mut hist: std::collections::HashMap<u32, Vec<(Option<f32>, Vec2)>> =
                    Default::default();
                for f in 0..30 * 90 {
                    let a = w.autopilot();
                    w.step(1.0 / 30.0, Some(a));
                    w.events.clear();
                    let p = w.snakes[0].head();
                    for s in w.snakes.iter().filter(|s| !s.player && s.alive) {
                        let h = hist.entry(s.id).or_default();
                        h.push(((s.stun <= 0.0).then_some(s.angle), s.head()));
                        if h.len() < 90 || f % 30 != 0 || (s.head() - p).length() < 700.0 {
                            continue;
                        }
                        let span = &h[h.len() - 90..];
                        let turned: f32 = span
                            .windows(2)
                            .filter_map(|x| Some(wrap_angle(x[1].0? - x[0].0?)))
                            .sum();
                        let path: f32 = span.windows(2).map(|x| (x[1].1 - x[0].1).length()).sum();
                        if turned.abs() > TAU {
                            loops += 1;
                        }
                        straightness.push((span[89].1 - span[0].1).length() / path.max(1.0));
                    }
                }
            }
            straightness.sort_by(f32::total_cmp);
            let median = straightness[straightness.len() / 2];
            let rate = loops as f32 / straightness.len() as f32;
            assert!(
                rate < 0.005,
                "training {training}: {loops}/{} windows looped",
                straightness.len()
            );
            assert!(
                median > 0.8,
                "training {training}: median straightness {median:.2}"
            );
        }
    }

    #[test]
    fn bots_wander_far_and_never_sit_stuck_for_two_minutes() {
        crate::rng::srand(7);
        let mut w = World::new(Spec::arena(1, false), "Me", 0);
        let mut last: Vec<(u32, Vec2)> = vec![];
        let mut still = std::collections::HashMap::<u32, f32>::new();
        let mut worst = 0.0f32;
        for f in 0..30 * 120 {
            let a = w.snakes[0].angle;
            w.step(1.0 / 30.0, Some(a));
            w.events.clear();
            if f % 30 == 0 {
                for s in w.snakes.iter().filter(|s| s.alive && !s.player) {
                    let moved = last
                        .iter()
                        .find(|(id, _)| *id == s.id)
                        .map(|(_, p)| (*p - s.head()).length())
                        .unwrap_or(1000.0);
                    let e = still.entry(s.id).or_insert(0.0);
                    *e = if moved < 40.0 { *e + 1.0 } else { 0.0 };
                    worst = worst.max(*e);
                }
                last = w.snakes.iter().map(|s| (s.id, s.head())).collect();
            }
        }
        assert!(worst < 4.0, "a bot sat still for {worst} seconds");
        assert!(w.bots().count() >= Spec::arena(1, false).bots - 2);
    }

    #[test]
    fn bots_stay_inside_the_arena_and_the_egg_count_is_refilled() {
        crate::rng::srand(3);
        let spec = Spec::arena(1, false);
        let mut w = World::new(spec.clone(), "Me", 0);
        for _ in 0..30 * 90 {
            let a = w.snakes[0].angle;
            w.step(1.0 / 30.0, Some(a));
            for s in &w.snakes {
                let h = s.head();
                assert!(h.x >= 0.0 && h.y >= 0.0 && h.x <= w.size && h.y <= w.size);
            }
        }
        assert!(w.eggs.len() as f32 > spec.eggs as f32 * 0.8);
    }

    #[test]
    fn bots_flee_from_a_bigger_snake_and_chase_a_smaller_one() {
        crate::rng::srand(5);
        let mut w = duel(8, 3);
        w.snakes[0].shield = 0.0;
        w.snakes[1].brain = Brain::new(1.0, 1.0);
        w.snakes[1].segs[0] = vec2(1500.0, 1500.0);
        w.snakes[0].segs[0] = vec2(1400.0, 1500.0);
        w.layout_tail(0);
        w.layout_tail(1);
        let mut brain = Brain::new(1.0, 1.0);
        let flee = w.think(1, &mut brain);
        let away = (w.snakes[1].head() - w.snakes[0].head()).normalize();
        assert!(vec2(flee.cos(), flee.sin()).dot(away) > 0.5);
        w.snakes[1].xp = xp_for_level(12);
        w.snakes[0].xp = xp_for_level(3);
        let mut brain = Brain::new(1.0, 1.0);
        brain.hunting = true;
        let chase = w.think(1, &mut brain);
        let to = (w.snakes[0].head() - w.snakes[1].head()).normalize();
        assert!(vec2(chase.cos(), chase.sin()).dot(to) > 0.3);
    }

    #[test]
    fn spawned_bots_follow_the_players_level_band() {
        crate::rng::srand(11);
        let w = World::new(Spec::arena(1, false), "Me", 0);
        let mut w2 = w;
        w2.snakes[0].xp = xp_for_level(15);
        let levels: Vec<u32> = (0..200).map(|_| w2.level_for_new_bot()).collect();
        let min = *levels.iter().min().unwrap();
        let max = *levels.iter().max().unwrap();
        assert!(min >= 11 && max <= 21, "{min}..{max}");
        assert!(levels.iter().any(|l| *l > 15) && levels.iter().any(|l| *l < 15));
    }

    #[test]
    fn dead_bots_are_replaced_so_the_arena_stays_busy() {
        crate::rng::srand(2);
        let mut w = World::new(Spec::arena(1, false), "Me", 0);
        let want = w.bots().count();
        let first: Vec<u32> = w.snakes.iter().map(|s| s.id).collect();
        let victim = w.snakes.iter().position(|s| !s.player).unwrap();
        w.kill(victim, Some(0));
        for _ in 0..30 * 6 {
            w.step(1.0 / 30.0, Some(w.snakes[0].angle));
        }
        assert!(w.bots().any(|s| !first.contains(&s.id)));
        assert!(w.bots().count() + w.respawns.len() >= want);
    }
}
