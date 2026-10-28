use super::bots::Brain;
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
    let mut l = 1;
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
        speed_for(self.level()) * if self.boss { 0.92 } else { 1.0 }
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
}

impl Spec {
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
    respawns: Vec<f32>,
    next_id: u32,
    name_at: usize,
    egg_budget: f32,
    bump_cd: Vec<(u32, u32, f32)>,
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
            respawns: vec![],
            next_id: 1,
            name_at: rand::gen_range(0usize, NAMES.len()),
            egg_budget: 0.0,
            bump_cd: vec![],
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

    pub fn player_alive(&self) -> bool {
        self.snakes[0].alive
    }

    pub fn by_id(&self, id: u32) -> Option<&Snake> {
        self.snakes.iter().find(|s| s.id == id)
    }

    pub fn bots(&self) -> impl Iterator<Item = &Snake> {
        self.snakes.iter().filter(|s| !s.player && s.alive)
    }

    fn random_point(&self, margin: f32) -> Vec2 {
        vec2(
            rand::gen_range(margin, self.size - margin),
            rand::gen_range(margin, self.size - margin),
        )
    }

    fn near_rock(&self, p: Vec2, pad: f32) -> bool {
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
        });
        let i = self.snakes.len() - 1;
        self.layout_tail(i);
        i
    }

    fn layout_tail(&mut self, i: usize) {
        let s = &mut self.snakes[i];
        let n = segs_for(s.xp);
        let back = -s.dir() * s.spacing();
        let head = s.segs[0];
        s.segs = (0..n).map(|k| head + back * k as f32).collect();
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
        self.collisions(dt);
        self.refill(dt);
        self.respawn(dt);
        self.snakes.retain(|s| s.alive || s.player);
    }

    fn advance(&mut self, i: usize, dt: f32) {
        let size = self.size;
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
        head.x = head.x.clamp(r, size - r);
        head.y = head.y.clamp(r, size - r);
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
        }
        let want = segs_for(s.xp);
        while s.segs.len() < want {
            let tail = *s.segs.last().unwrap();
            s.segs.push(tail);
        }
    }

    fn gain(&mut self, i: usize, amount: u32) {
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
            let magnet = 55.0 + r * 1.6;
            let pull_speed = self.snakes[i].speed() * 1.8;
            let mut got = 0;
            let mut k = 0;
            while k < self.eggs.len() {
                let d = self.eggs[k].pos - head;
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

    fn contact(&self, a: usize, b: usize) -> Option<Vec2> {
        let sa = &self.snakes[a];
        let sb = &self.snakes[b];
        let ha = sa.head();
        let ra = sa.radius() * 0.8;
        sb.segs
            .iter()
            .enumerate()
            .find(|(j, p)| (ha - **p).length() < ra + sb.seg_radius(*j) * 0.85)
            .map(|(_, p)| *p)
    }

    fn collisions(&mut self, dt: f32) {
        for c in &mut self.bump_cd {
            c.2 -= dt;
        }
        self.bump_cd.retain(|c| c.2 > 0.0);
        let n = self.snakes.len();
        for a in 0..n {
            for b in 0..n {
                if a == b || !self.snakes[a].alive || !self.snakes[b].alive {
                    continue;
                }
                let Some(at) = self.contact(a, b) else {
                    continue;
                };
                let (la, lb) = (self.snakes[a].level(), self.snakes[b].level());
                if la == lb || (la < lb && self.snakes[a].shield > 0.0) {
                    self.bump(a, b, at);
                } else if la > lb {
                    if self.snakes[b].shield > 0.0 {
                        self.bump(a, b, at);
                    } else {
                        self.kill(b, Some(a));
                    }
                } else {
                    self.kill(a, Some(b));
                }
            }
        }
    }

    fn bump(&mut self, a: usize, b: usize, at: Vec2) {
        let key = (
            self.snakes[a].id.min(self.snakes[b].id),
            self.snakes[a].id.max(self.snakes[b].id),
        );
        if self.bump_cd.iter().any(|c| (c.0, c.1) == key) {
            return;
        }
        self.bump_cd.push((key.0, key.1, 0.5));
        let ha = self.snakes[a].head();
        let hb = self.snakes[b].head();
        let away = (ha - at).normalize_or_zero();
        let away = if away == Vec2::ZERO {
            (ha - hb).normalize_or_zero()
        } else {
            away
        };
        let push = self.snakes[a].radius() + 6.0;
        let size = self.size;
        let s = &mut self.snakes[a];
        let mut h = s.segs[0] + away * push;
        h.x = h.x.clamp(5.0, size - 5.0);
        h.y = h.y.clamp(5.0, size - 5.0);
        s.segs[0] = h;
        s.angle = away.y.atan2(away.x);
        s.want = s.angle;
        s.stun = 0.35;
        let player = s.player || self.snakes[b].player;
        self.events.push(Event::Bump { at, player });
    }

    fn kill(&mut self, victim: usize, by: Option<usize>) {
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
        let total = v.xp / 2 + 2;
        let drops = (v.segs.len() / 2).clamp(3, 40) as u32;
        let each = (total / drops).max(1);
        let mut left = total;
        for k in 0..drops {
            if left == 0 {
                break;
            }
            let value = each.min(left);
            left -= value;
            let at = v.segs[(k as usize * v.segs.len() / drops as usize).min(v.segs.len() - 1)];
            let a = rand::gen_range(0.0, TAU);
            self.eggs.push(Egg {
                pos: at,
                vel: vec2(a.cos(), a.sin()) * rand::gen_range(60.0, 200.0),
                value,
                kind: rand::gen_range(0u8, 4),
                age: 0.0,
            });
        }
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

    fn respawn(&mut self, dt: f32) {
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
        let mut v: Vec<_> = self
            .snakes
            .iter()
            .filter(|s| s.alive)
            .map(|s| (s.id, s.name.clone(), s.xp, s.player))
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

    fn duel(a_level: u32, b_level: u32) -> World {
        let mut w = World::new(quiet(), "Me", 0);
        w.snakes[0].shield = 0.0;
        w.snakes[0].xp = xp_for_level(a_level);
        w.snakes[0].segs[0] = vec2(1000.0, 1000.0);
        w.snakes[0].angle = 0.0;
        w.snakes[0].want = 0.0;
        w.layout_tail(0);
        let i = w.add_snake(vec2(1022.0, 1000.0), b_level, "Rival".into(), 1);
        w.snakes[i].xp = xp_for_level(b_level);
        w.snakes[i].angle = PI / 2.0;
        w.snakes[i].want = PI / 2.0;
        w.layout_tail(i);
        w
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
