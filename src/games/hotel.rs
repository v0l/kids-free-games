use crate::engine::draw_circle;
use crate::engine::*;
use crate::fx::*;
use crate::sfx::{self, Sfx};
use macroquad::prelude::*;

pub const ROOMS: usize = 12;
pub const COLS: usize = 4;
pub const START_COINS: u32 = 0;
pub const QUEUE_MAX: usize = 5;
pub const PATIENCE: f32 = 18.0;
pub const STAY: f32 = 9.0;
pub const CLEAN_TIME: f32 = 1.2;
pub const REQUEST_WAIT: f32 = 6.0;
pub const MAX_LEVEL: u8 = 3;
pub const WALK: f32 = 0.9;
pub const FLIGHT: f32 = 0.7;
pub const DAY: f32 = 50.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Event {
    Built(usize),
    Upgraded(usize),
    Seated { room: usize, look: u8 },
    Tip(usize),
    CleanStarted(usize),
    Cleaned(usize),
    Denied(usize, u32),
    Left(usize),
}

#[derive(Clone, Copy, Debug)]
struct Walker {
    room: usize,
    look: u8,
    t: f32,
}

#[derive(Clone, Copy, Debug)]
struct FlyCoin {
    from: Vec2,
    delay: f32,
    t: f32,
}

pub fn walk_pos(from: Vec2, to: Vec2, k: f32) -> Vec2 {
    let k = k.clamp(0.0, 1.0);
    let e = k * k * (3.0 - 2.0 * k);
    let hop = (k * 5.0 * std::f32::consts::PI).sin().abs() * 12.0 * (k * (1.0 - k) * 4.0).min(1.0);
    from + (to - from) * e - vec2(0.0, hop)
}

pub fn coin_pos(from: Vec2, to: Vec2, k: f32) -> Vec2 {
    let k = k.clamp(0.0, 1.0);
    let e = k * k * (3.0 - 2.0 * k);
    from + (to - from) * e - vec2(0.0, (k * std::f32::consts::PI).sin() * 60.0)
}

pub fn night_amount(t: f32) -> f32 {
    let s = 0.5 - 0.5 * (t / DAY * std::f32::consts::TAU).cos();
    let k = ((s - 0.25) / 0.5).clamp(0.0, 1.0);
    k * k * (3.0 - 2.0 * k)
}

pub fn coins_for(earned: u32) -> usize {
    (earned as usize / 5).clamp(2, 8)
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Need {
    Towel,
    Food,
    Music,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Room {
    Locked,
    Free,
    Busy {
        guest: u8,
        left: f32,
        need: Option<(Need, f32)>,
        mood: i32,
    },
    Dirty {
        cleaning: Option<f32>,
    },
}

#[derive(Clone, Copy, Debug)]
pub struct Guest {
    pub look: u8,
    pub patience: f32,
}

pub struct Hotel {
    pub rooms: [Room; ROOMS],
    pub levels: [u8; ROOMS],
    pub queue: Vec<Guest>,
    pub coins: u32,
    pub stars: f32,
    pub served: u32,
    pub popups: Popups,
    pub events: Vec<Event>,
    spawn_in: f32,
    t: f32,
    walkers: Vec<Walker>,
    flying: Vec<FlyCoin>,
    particles: Particles,
    shake: Shake,
    coin_bump: Bump,
    guest_bump: Bump,
    pop: Vec<Bump>,
}

pub fn room_price(index: usize) -> u32 {
    if index < 2 {
        0
    } else {
        30 + (index as u32 - 2) * 25
    }
}

pub fn upgrade_price(level: u8) -> u32 {
    40 * level as u32
}

pub fn pay(level: u8, mood: i32) -> u32 {
    (10 * level as i32 + mood * 3).max(2) as u32
}

impl Hotel {
    pub fn new() -> Self {
        let mut rooms = [Room::Locked; ROOMS];
        let levels = [1; ROOMS];
        for (i, r) in rooms.iter_mut().enumerate() {
            if room_price(i) == 0 {
                *r = Room::Free;
            }
        }
        Self {
            rooms,
            levels,
            queue: vec![],
            coins: START_COINS,
            stars: 3.0,
            served: 0,
            popups: Popups::default(),
            events: vec![],
            spawn_in: 0.5,
            t: 0.0,
            walkers: vec![],
            flying: vec![],
            particles: Particles::default(),
            shake: Shake::default(),
            coin_bump: Bump::default(),
            guest_bump: Bump::default(),
            pop: (0..ROOMS).map(|_| Bump::default()).collect(),
        }
    }

    pub fn arrival_gap(&self) -> f32 {
        let open = self.rooms.iter().filter(|r| **r != Room::Locked).count() as f32;
        (7.5 - open * 0.35 - self.stars * 0.3).max(1.8)
    }

    pub fn tap_room(&mut self, i: usize) {
        match self.rooms[i] {
            Room::Locked => {
                let next_locked = self.rooms.iter().position(|r| *r == Room::Locked);
                let price = room_price(i);
                if next_locked == Some(i) && self.coins >= price {
                    self.coins -= price;
                    self.rooms[i] = Room::Free;
                    self.events.push(Event::Built(i));
                } else if next_locked == Some(i) {
                    self.events.push(Event::Denied(i, price));
                }
            }
            Room::Free => {
                if !self.queue.is_empty() {
                    let g = self.queue.remove(0);
                    let mood = if g.patience > PATIENCE * 0.5 { 1 } else { 0 };
                    self.events.push(Event::Seated {
                        room: i,
                        look: g.look,
                    });
                    self.rooms[i] = Room::Busy {
                        guest: g.look,
                        left: STAY,
                        need: None,
                        mood,
                    };
                }
            }
            Room::Busy {
                guest,
                left,
                need: Some(_),
                mood,
            } => {
                self.rooms[i] = Room::Busy {
                    guest,
                    left,
                    need: None,
                    mood: mood + 1,
                };
                self.coins += 3;
                self.events.push(Event::Tip(i));
            }
            Room::Dirty { cleaning: None } => {
                self.rooms[i] = Room::Dirty {
                    cleaning: Some(CLEAN_TIME),
                };
                self.events.push(Event::CleanStarted(i));
            }
            _ => {}
        }
    }

    pub fn try_upgrade(&mut self, i: usize) -> bool {
        let lvl = self.levels[i];
        if self.rooms[i] == Room::Locked || lvl >= MAX_LEVEL || self.coins < upgrade_price(lvl) {
            return false;
        }
        self.coins -= upgrade_price(lvl);
        self.levels[i] += 1;
        self.events.push(Event::Upgraded(i));
        true
    }

    pub fn step(&mut self, dt: f32) -> Vec<(usize, i32)> {
        self.t += dt;
        let mut events = vec![];
        self.spawn_in -= dt;
        if self.spawn_in <= 0.0 {
            self.spawn_in = self.arrival_gap() * rand::gen_range(0.7, 1.3);
            if self.queue.len() < QUEUE_MAX {
                self.queue.push(Guest {
                    look: rand::gen_range(0, 6),
                    patience: PATIENCE,
                });
            }
        }
        let before = self.queue.len();
        for g in &mut self.queue {
            g.patience -= dt;
        }
        self.queue.retain(|g| g.patience > 0.0);
        let left_angry = before - self.queue.len();
        if left_angry > 0 {
            self.events.push(Event::Left(left_angry));
        }
        self.stars = (self.stars - 0.25 * left_angry as f32).max(0.0);
        for i in 0..ROOMS {
            self.rooms[i] = match self.rooms[i] {
                Room::Busy {
                    guest,
                    left,
                    need,
                    mood,
                } => {
                    let left = left - dt;
                    if left <= 0.0 {
                        let earned = pay(self.levels[i], mood);
                        self.coins += earned;
                        self.served += 1;
                        self.stars =
                            (self.stars + if mood > 0 { 0.1 } else { -0.05 }).clamp(0.0, 5.0);
                        events.push((i, earned as i32));
                        Room::Dirty { cleaning: None }
                    } else {
                        let (need, mood) = match need {
                            Some((_, w)) if w - dt <= 0.0 => (None, mood - 1),
                            Some((n, w)) => (Some((n, w - dt)), mood),
                            None if left > 2.0 && rand::gen_range(0.0, 1.0) < dt * 0.12 => {
                                let n =
                                    [Need::Towel, Need::Food, Need::Music][rand::gen_range(0, 3)];
                                (Some((n, REQUEST_WAIT)), mood)
                            }
                            None => (None, mood),
                        };
                        Room::Busy {
                            guest,
                            left,
                            need,
                            mood,
                        }
                    }
                }
                Room::Dirty { cleaning: Some(c) } if c - dt <= 0.0 => {
                    self.events.push(Event::Cleaned(i));
                    Room::Free
                }
                Room::Dirty { cleaning: Some(c) } => Room::Dirty {
                    cleaning: Some(c - dt),
                },
                r => r,
            };
        }
        events
    }

    fn layout(view: &View) -> (Rect, Vec<Rect>) {
        let top = 80.0;
        let lobby_w = (view.w * 0.26).clamp(220.0, 320.0);
        let lobby = Rect::new(16.0, top, lobby_w, view.h - top - 16.0);
        let gx = lobby.x + lobby.w + 16.0;
        let gw = view.w - gx - 16.0;
        let rows = ROOMS.div_ceil(COLS);
        let cw = gw / COLS as f32;
        let ch = (view.h - top - 16.0) / rows as f32;
        let rects = (0..ROOMS)
            .map(|i| {
                let (c, r) = (i % COLS, rows - 1 - i / COLS);
                Rect::new(
                    gx + c as f32 * cw + 6.0,
                    top + r as f32 * ch + 6.0,
                    cw - 12.0,
                    ch - 12.0,
                )
            })
            .collect();
        (lobby, rects)
    }

    fn upgrade_rect(room: Rect) -> Rect {
        Rect::new(room.x + room.w - 46.0, room.y + 6.0, 40.0, 40.0)
    }

    fn queue_slot(lobby: Rect) -> f32 {
        ((lobby.h - 110.0) / QUEUE_MAX as f32).min(110.0)
    }

    fn lobby_spot(lobby: Rect, i: usize) -> Vec2 {
        let slot = Self::queue_slot(lobby);
        vec2(
            lobby.x + 60.0,
            lobby.y + 100.0 + i as f32 * slot + slot * 0.45,
        )
    }

    fn room_spot(room: Rect) -> Vec2 {
        vec2(room.x + room.w * 0.78, room.y + room.h - 60.0)
    }

    fn coin_target(view: &View) -> Vec2 {
        vec2(view.w * 0.42, 35.0)
    }

    fn fly_coins(&mut self, from: Vec2, count: usize) {
        for k in 0..count {
            self.flying.push(FlyCoin {
                from,
                delay: k as f32 * 0.07,
                t: 0.0,
            });
        }
    }

    fn react(&mut self, view: &View) {
        let (lobby, rects) = Self::layout(view);
        let gold = Color::from_rgba(255, 210, 60, 255);
        for e in std::mem::take(&mut self.events) {
            match e {
                Event::Built(i) => {
                    sfx::play(Sfx::LevelUp);
                    self.pop[i].kick();
                    self.particles.sparkle(rects[i].center(), gold);
                    self.particles
                        .sparkle(rects[i].center() + vec2(0.0, -30.0), WHITE);
                    self.shake.add(0.2);
                    self.popups.add(rects[i].center(), "Built!", gold, 40.0);
                }
                Event::Upgraded(i) => {
                    sfx::play(Sfx::LevelUp);
                    self.pop[i].kick();
                    let up = Self::upgrade_rect(rects[i]).center();
                    self.particles.sparkle(up, gold);
                    self.particles.sparkle(rects[i].center(), WHITE);
                    self.popups.add(up, "Upgrade!", gold, 34.0);
                }
                Event::Seated { room, look } => {
                    sfx::play(Sfx::Tap);
                    self.walkers.push(Walker { room, look, t: 0.0 });
                }
                Event::Tip(i) => {
                    sfx::play(Sfx::Right);
                    let at = vec2(
                        rects[i].x + rects[i].w * 0.62,
                        rects[i].y + rects[i].h * 0.38,
                    );
                    self.popups.add(at, "+3", gold, 30.0);
                    self.particles.sparkle(at, gold);
                    self.fly_coins(at, 1);
                }
                Event::CleanStarted(i) => {
                    sfx::play(Sfx::Swish);
                    self.particles.emit(
                        rects[i].center(),
                        &[
                            Color::from_rgba(190, 230, 255, 255),
                            Color::from_rgba(255, 255, 255, 255),
                        ],
                        Burst {
                            count: 10,
                            speed: (40.0, 140.0),
                            size: (4.0, 9.0),
                            gravity: -120.0,
                            ..Default::default()
                        },
                    );
                }
                Event::Cleaned(i) => {
                    sfx::play(Sfx::Pop);
                    self.pop[i].kick();
                    self.particles
                        .sparkle(rects[i].center(), Color::from_rgba(150, 220, 255, 255));
                }
                Event::Denied(i, price) => {
                    sfx::play(Sfx::Wrong);
                    self.shake.add(0.15);
                    self.popups.add(
                        rects[i].center(),
                        format!("Need ${price}"),
                        Color::from_rgba(255, 110, 100, 255),
                        32.0,
                    );
                }
                Event::Left(n) => {
                    sfx::play(Sfx::Wrong);
                    self.shake.add(0.25);
                    self.popups.add(
                        vec2(lobby.x + lobby.w / 2.0, lobby.y + 90.0),
                        if n > 1 {
                            format!("{n} left!")
                        } else {
                            "Left!".into()
                        },
                        Color::from_rgba(255, 110, 100, 255),
                        34.0,
                    );
                }
            }
        }
    }

    fn animate(&mut self, view: &View, dt: f32) {
        let (_, rects) = Self::layout(view);
        for w in &mut self.walkers {
            w.t += dt;
        }
        let arrived: Vec<Walker> = self
            .walkers
            .iter()
            .copied()
            .filter(|w| w.t >= WALK)
            .collect();
        self.walkers.retain(|w| w.t < WALK);
        for w in arrived {
            let at = Self::room_spot(rects[w.room]) + vec2(0.0, 40.0);
            self.particles
                .puff(at, Color::from_rgba(240, 230, 215, 255));
            self.pop[w.room].kick();
        }
        let target = Self::coin_target(view);
        let mut landed = 0;
        for c in &mut self.flying {
            if c.delay > 0.0 {
                c.delay -= dt;
            } else {
                c.t += dt / FLIGHT;
                if c.t >= 1.0 {
                    landed += 1;
                }
            }
        }
        self.flying.retain(|c| c.t < 1.0);
        if landed > 0 {
            sfx::play(Sfx::Coin);
            self.coin_bump.kick();
            self.particles
                .sparkle(target, Color::from_rgba(255, 220, 90, 255));
        }
        for b in &mut self.pop {
            b.update(dt);
        }
        self.coin_bump.update(dt);
        self.guest_bump.update(dt);
        self.particles.update(dt);
        self.popups.update(dt);
        self.shake.update(dt);
    }
}

impl Default for Hotel {
    fn default() -> Self {
        Self::new()
    }
}

mod art {
    crate::sprites!("hotel");
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Pose {
    Idle,
    Think,
    Cheer,
    Walk(f32),
}

fn guest_sprite(look: u8, mood: Pose) -> String {
    let l = look as usize % 6;
    match mood {
        Pose::Idle => format!("g{l}_idle"),
        Pose::Think => format!("g{l}_think"),
        Pose::Cheer => format!("g{l}_cheer"),
        Pose::Walk(t) => format!("g{l}_walk{}", (t * 11.0) as usize % 8),
    }
}

pub fn guest_height(s: f32) -> f32 {
    95.0 * s
}

fn draw_guest(x: f32, y: f32, s: f32, look: u8, mood: Pose) {
    let feet = y + 42.0 * s;
    soft_shadow(vec2(x, feet), 22.0 * s, 5.0 * s, 0.6);
    let h = guest_height(s);
    art::sprite_at(
        &guest_sprite(look, mood),
        vec2(x, feet - h / 2.0),
        h,
        crate::art::Draw::default(),
    );
}

fn draw_coin(c: Vec2, r: f32, spin: f32) {
    let w = r * 2.0 * spin.cos().abs().max(0.2);
    crate::ui::sprite_ex(
        "coin",
        Rect::new(c.x - w / 2.0, c.y - r, w, r * 2.0),
        crate::art::Draw::default(),
    );
}

fn need_sprite(need: Need) -> &'static str {
    match need {
        Need::Towel => "need_towel",
        Need::Food => "need_food",
        Need::Music => "need_music",
    }
}

fn draw_need(c: Vec2, need: Need) {
    art::sprite_at(need_sprite(need), c, 34.0, crate::art::Draw::default());
}

const WALLS: [Color; 3] = [
    Color::new(1.0, 0.98, 0.94, 1.0),
    Color::new(0.86, 0.94, 1.0, 1.0),
    Color::new(1.0, 0.9, 0.98, 1.0),
];

const WOOD: Color = Color::new(0.78, 0.56, 0.42, 1.0);

fn frame(r: Rect, scale: f32, tint: Color) {
    crate::ui::nine("panel", r, 14.0, scale, tint);
}

fn wallpaper(r: Rect, lvl: u8) {
    let ink = darker(WALLS[(lvl - 1) as usize], 0.93);
    match lvl {
        1 => {
            let n = (r.w / 18.0) as usize;
            for k in (0..n).step_by(2) {
                draw_rectangle(r.x + k as f32 * 18.0, r.y, 9.0, r.h, ink);
            }
        }
        2 => {
            let (nx, ny) = ((r.w / 22.0) as usize, (r.h / 22.0) as usize);
            for gy in 0..ny {
                for gx in 0..nx {
                    let off = if gy % 2 == 0 { 0.0 } else { 11.0 };
                    draw_circle(
                        r.x + 11.0 + gx as f32 * 22.0 + off,
                        r.y + 11.0 + gy as f32 * 22.0,
                        3.0,
                        ink,
                    );
                }
            }
        }
        _ => {
            let (nx, ny) = ((r.w / 26.0) as usize + 1, (r.h / 26.0) as usize + 1);
            for gy in 0..ny {
                for gx in 0..nx {
                    let c = vec2(r.x + gx as f32 * 26.0, r.y + gy as f32 * 26.0);
                    if r.contains(c) {
                        draw_poly(c.x, c.y, 4, 7.0, 0.0, ink);
                    }
                }
            }
        }
    }
}

fn window_size(h: f32) -> Vec2 {
    let s = art::size("window");
    vec2(h * s.x / s.y, h)
}

pub fn glass_rect(w: Rect) -> Rect {
    Rect::new(
        w.x + w.w * 8.0 / 48.0,
        w.y + w.h * 14.0 / 112.0,
        w.w * 22.0 / 48.0,
        w.h * 82.0 / 112.0,
    )
}

fn window(c: Vec2, h: f32, night: f32, t: f32, seed: f32) {
    let sz = window_size(h);
    let r = Rect::new(c.x - sz.x / 2.0, c.y - sz.y / 2.0, sz.x, sz.y);
    let g = glass_rect(r);
    let day_top = Color::from_rgba(120, 200, 255, 255);
    let day_bottom = Color::from_rgba(205, 238, 255, 255);
    let dark_top = Color::from_rgba(20, 26, 80, 255);
    let dark_bottom = Color::from_rgba(70, 70, 140, 255);
    let mix = |a: Color, b: Color| {
        Color::new(
            a.r + (b.r - a.r) * night,
            a.g + (b.g - a.g) * night,
            a.b + (b.b - a.b) * night,
            1.0,
        )
    };
    gradient_rect(g, mix(day_top, dark_top), mix(day_bottom, dark_bottom));
    if night > 0.4 {
        let a = (night - 0.4) / 0.6;
        for k in 0..4 {
            let f = k as f32 + seed;
            let sx = g.x + 4.0 + (f * 37.0) % (g.w - 8.0);
            let sy = g.y + 4.0 + (f * 23.0) % (g.h * 0.5);
            let tw = 0.5 + 0.5 * (t * 3.0 + f * 2.0).sin();
            draw_circle(sx, sy, 1.0 + tw, with_alpha(WHITE, a * (0.5 + 0.5 * tw)));
        }
        art::sprite_at(
            "moon",
            vec2(g.x + g.w * 0.55, g.y + g.h * 0.28),
            g.w * 0.6,
            crate::art::Draw {
                tint: with_alpha(WHITE, a),
                ..Default::default()
            },
        );
    } else {
        let a = 1.0 - night / 0.4;
        let drift = (t * 3.0 + seed * 9.0) % (g.w * 0.5);
        crate::ui::cloud(
            seed as usize,
            vec2(g.x + g.w * 0.3 + drift, g.y + g.h * 0.28),
            g.w * 0.55,
            with_alpha(WHITE, a * 0.95),
        );
    }
    art::sprite("window", r);
}

fn bar(r: Rect, k: f32, fill: &str) {
    let scale = r.h / 16.0;
    art::nine("bar_back", r, 8.0, scale, WHITE);
    if k > 0.02 {
        let w = (r.w * k).max(r.h);
        art::nine(fill, Rect::new(r.x, r.y, w, r.h), 8.0, scale, WHITE);
    }
}

impl Hotel {
    fn draw_hud(&self, view: &View) {
        let f = view.full();
        let ink = Color::from_rgba(100, 35, 45, 255);
        let top = Rect::new(f.x - 8.0, f.y - 8.0, f.w + 16.0, 62.0 - f.y + 8.0);
        frame(top, 0.9, Color::from_rgba(215, 95, 105, 255));
        let n = (f.w / 28.0) as usize + 2;
        for k in 0..n {
            let name = if k % 2 == 0 { "bulb_white" } else { "bulb_red" };
            art::sprite_at(
                name,
                vec2(f.x + k as f32 * 28.0 + 14.0, 68.0),
                30.0,
                crate::art::Draw::default(),
            );
        }
        text_outline("Hotel Hop", HUD_LEFT + 80.0, 38.0, 40.0, WHITE, ink);
        let target = Self::coin_target(view);
        let k = self.coin_bump.scale(0.3);
        draw_coin(target, 21.0 * k, 0.0);
        score_text(
            &self.coins.to_string(),
            target.x + 32.0 + text_size(&self.coins.to_string(), 38.0).width / 2.0,
            target.y + 3.0,
            38.0,
            self.coin_bump.t,
        );
        for s in 0..5 {
            let x = view.w * 0.62 + s as f32 * 38.0;
            let filled = self.stars - s as f32;
            let on = filled >= 0.5;
            let wob = if on {
                1.0 + 0.06 * (self.t * 3.0 + s as f32).sin()
            } else {
                1.0
            };
            crate::ui::sprite_at(
                if on { "star" } else { "star_empty" },
                vec2(x, 35.0),
                34.0 * wob,
                crate::art::Draw::default(),
            );
        }
        text_outline(
            &format!("Guests {}", self.served),
            view.w - 100.0,
            35.0,
            30.0,
            WHITE,
            ink,
        );
    }

    fn draw_lobby(&self, lobby: Rect) {
        frame(lobby, 0.9, WOOD);
        let carpet = Rect::new(
            lobby.x + 8.0,
            lobby.y + 92.0,
            lobby.w - 16.0,
            lobby.h - 104.0,
        );
        frame(carpet, 0.45, Color::from_rgba(235, 110, 125, 255));
        let sign = Rect::new(lobby.x + 10.0, lobby.y + 10.0, lobby.w - 20.0, 70.0);
        frame(sign, 0.8, Color::from_rgba(190, 120, 80, 255));
        art::sprite_at(
            "plant",
            vec2(sign.x + 22.0, sign.y + sign.h - 28.0),
            52.0,
            crate::art::Draw::default(),
        );
        art::sprite_at(
            "lamp",
            vec2(sign.x + sign.w - 18.0, sign.y + sign.h * 0.5),
            sign.h - 8.0,
            crate::art::Draw::default(),
        );
        let glow = 0.8 + 0.2 * (self.t * 2.0).sin();
        text_outline(
            "Lobby",
            lobby.x + lobby.w / 2.0,
            lobby.y + 45.0,
            34.0 * glow.max(0.95),
            Color::from_rgba(255, 235, 170, 255),
            Color::from_rgba(70, 35, 20, 255),
        );
        let slot = Self::queue_slot(lobby);
        for (i, g) in self.queue.iter().enumerate() {
            let y = lobby.y + 100.0 + i as f32 * slot;
            let cx = lobby.x + 60.0;
            if i == 0 {
                let pulse = 0.5 + 0.5 * (self.t * 4.0).sin();
                let card = Rect::new(lobby.x + 10.0, y, lobby.w - 20.0, slot - 8.0);
                frame(card, 0.5, Color::from_rgba(255, 244, 214, 255));
                draw_rectangle_lines(
                    card.x,
                    card.y,
                    card.w,
                    card.h,
                    3.0,
                    with_alpha(Color::from_rgba(255, 190, 60, 255), 0.3 + 0.5 * pulse),
                );
            }
            let urgent = g.patience < PATIENCE * 0.25;
            let shake = if urgent {
                (self.t * 40.0).sin() * 2.5
            } else {
                0.0
            };
            let bob = (self.t * 3.0 + i as f32 * 1.3).sin().abs() * 3.0;
            let waiting = g.patience > PATIENCE * 0.4;
            let s = (slot / 110.0).max(0.6);
            draw_guest(
                cx + shake,
                y + slot * 0.42 - bob,
                s,
                g.look,
                if waiting { Pose::Idle } else { Pose::Think },
            );
            if urgent {
                art::sprite_at(
                    "face_sad",
                    vec2(cx + 30.0 * s, y + slot * 0.42 - 40.0 * s - bob),
                    26.0,
                    crate::art::Draw::default(),
                );
            }
            let k = g.patience / PATIENCE;
            let fill = if k > 0.5 {
                "bar_green"
            } else if k > 0.25 {
                "bar_yellow"
            } else {
                "bar_red"
            };
            bar(
                Rect::new(lobby.x + 100.0, y + slot * 0.4, lobby.w - 120.0, 16.0),
                k,
                fill,
            );
        }
        if self.queue.is_empty() {
            text_center(
                "Waiting for guests",
                lobby.x + lobby.w / 2.0,
                lobby.y + 140.0,
                26.0,
                Color::from_rgba(255, 245, 225, 255),
            );
        }
    }

    fn draw_locked(&self, i: usize, r: Rect, next_locked: Option<usize>) {
        frame(r, 0.55, Color::from_rgba(240, 222, 196, 255));
        let inner = Rect::new(r.x + 6.0, r.y + 6.0, r.w - 12.0, r.h - 14.0);
        frame(inner, 0.3, Color::from_rgba(205, 185, 160, 255));
        let c = r.center();
        if next_locked != Some(i) {
            crate::ui::sprite_at(
                "icon_lock",
                c,
                44.0,
                crate::art::Draw {
                    tint: Color::from_rgba(120, 110, 100, 255),
                    ..Default::default()
                },
            );
            return;
        }
        let can = self.coins >= room_price(i);
        text_outline(
            "Build",
            c.x,
            r.y + r.h * 0.34,
            32.0,
            WHITE,
            Color::from_rgba(90, 70, 55, 255),
        );
        let press = if can {
            0.5 + 0.5 * (self.t * 6.0).sin()
        } else {
            0.0
        };
        let tone = if can {
            crate::ui::Tone::Green
        } else {
            crate::ui::Tone::Grey
        };
        let b = Rect::new(r.x + 14.0, c.y - 6.0, r.w - 28.0, 60.0);
        crate::ui::button(b, &format!("${}", room_price(i)), tone, press * 0.4);
        if can {
            shine_sweep(r, self.t + i as f32);
        }
    }

    fn draw_room(&self, i: usize, r: Rect, next_locked: Option<usize>, night: f32) {
        let lvl = self.levels[i];
        let k = self.pop[i].scale(0.12);
        let c = r.center();
        let r = Rect::new(c.x - r.w * k / 2.0, c.y - r.h * k / 2.0, r.w * k, r.h * k);
        if self.rooms[i] == Room::Locked {
            self.draw_locked(i, r, next_locked);
            return;
        }
        frame(r, 0.55, WOOD);
        let inner = Rect::new(r.x + 6.0, r.y + 6.0, r.w - 12.0, r.h - 14.0);
        art::sprite_ex(
            "wall",
            inner,
            crate::art::Draw {
                tint: WALLS[(lvl - 1) as usize],
                ..Default::default()
            },
        );
        let floor_h = 16.0;
        wallpaper(Rect::new(inner.x, inner.y, inner.w, inner.h - floor_h), lvl);
        let floor = Rect::new(inner.x, inner.y + inner.h - floor_h, inner.w, floor_h);
        art::sprite("floor", floor);
        let floor_y = floor.y + 4.0;
        let win_h = (inner.h * 0.46).min(inner.w * 0.62 / 0.43);
        window(
            vec2(
                inner.x + inner.w * 0.36,
                inner.y + inner.h * 0.12 + win_h / 2.0,
            ),
            win_h,
            night,
            self.t,
            i as f32,
        );
        let draw = crate::art::Draw::default();
        let lamp_at = vec2(inner.x + 14.0, floor_y);
        if lvl >= 2 {
            let lh = (inner.h * 0.5).min(110.0);
            art::sprite_at("lamp", lamp_at - vec2(0.0, lh / 2.0), lh, draw);
            if night > 0.4 {
                draw_circle(
                    lamp_at.x + 2.0,
                    lamp_at.y - lh + 6.0,
                    26.0,
                    Color::new(1.0, 0.9, 0.5, 0.2 * night),
                );
            }
        }
        let bed_w = inner.w * 0.78;
        let bed = if lvl >= 2 { "bed2" } else { "bed1" };
        let bs = art::size(bed);
        let bed_h = bed_w * bs.y / bs.x;
        art::sprite_ex(
            bed,
            Rect::new(inner.x + 8.0, floor_y - bed_h, bed_w, bed_h),
            draw,
        );
        if lvl >= 3 {
            let ph = (inner.h * 0.3).min(70.0);
            art::sprite_at(
                "plant",
                vec2(inner.x + inner.w - 14.0, floor_y - ph / 2.0),
                ph,
                draw,
            );
        }
        let badge = Rect::new(r.x + 6.0, r.y + 6.0, 34.0, 30.0);
        frame(badge, 0.45, Color::from_rgba(150, 100, 70, 255));
        text_center(
            &format!("{}", i + 1),
            badge.x + badge.w / 2.0,
            badge.y + badge.h / 2.0 - 1.0,
            24.0,
            WHITE,
        );
        for s in 0..lvl {
            crate::ui::sprite_at(
                "star",
                vec2(r.x + 56.0 + s as f32 * 19.0, r.y + 21.0),
                18.0 + (self.pop[i].t < 1.0) as u8 as f32 * 3.0,
                draw,
            );
        }
        match self.rooms[i] {
            Room::Free => {
                let pulse = if self.queue.is_empty() {
                    0.0
                } else {
                    (self.t * 5.0).sin() * 0.5 + 0.5
                };
                draw_rectangle_lines(
                    r.x + 2.0,
                    r.y + 2.0,
                    r.w - 4.0,
                    r.h - 4.0,
                    4.0,
                    with_alpha(GREEN, 0.3 + 0.6 * pulse),
                );
                text_outline(
                    "Free",
                    r.x + r.w * 0.62,
                    r.y + r.h * 0.6,
                    30.0,
                    Color::from_rgba(80, 190, 90, 255),
                    WHITE,
                );
            }
            Room::Busy {
                guest,
                left,
                need,
                mood,
            } => {
                let walking = self.walkers.iter().any(|w| w.room == i);
                if !walking {
                    let bob = (self.t * 2.5 + i as f32).sin() * 1.5;
                    let happy = need.is_none() && mood > 1;
                    let pose = if happy {
                        Pose::Cheer
                    } else if mood >= 0 {
                        Pose::Idle
                    } else {
                        Pose::Think
                    };
                    let spot = Self::room_spot(r);
                    draw_guest(spot.x, spot.y + bob, 0.95, guest, pose);
                    if happy {
                        let hp = vec2(
                            spot.x + 26.0,
                            spot.y - 38.0 - (self.t * 2.0 + i as f32).sin().abs() * 6.0,
                        );
                        art::sprite_at("heart", hp, 28.0, draw);
                    }
                }
                bar(
                    Rect::new(
                        inner.x + 8.0,
                        floor.y + floor.h * 0.5 - 4.0,
                        inner.w - 16.0,
                        8.0,
                    ),
                    (left / STAY).min(1.0),
                    "bar_blue",
                );
                if let Some((n, w)) = need {
                    let c = vec2(r.x + r.w * 0.66, r.y + r.h * 0.4);
                    let bob = (self.t * 6.0).sin() * 3.0;
                    let urgent = w < 2.0;
                    let pulse = if urgent {
                        1.0 + 0.1 * (self.t * 14.0).sin()
                    } else {
                        1.0
                    };
                    draw_triangle(
                        vec2(c.x - 8.0, c.y + 26.0 + bob),
                        vec2(c.x + 8.0, c.y + 26.0 + bob),
                        vec2(c.x + 4.0, c.y + 38.0 + bob),
                        WHITE,
                    );
                    let tint = if urgent {
                        Color::from_rgba(255, 130, 130, 255)
                    } else {
                        Color::from_rgba(255, 215, 150, 255)
                    };
                    let d = 62.0 * pulse;
                    crate::ui::sprite_ex(
                        "round_white",
                        Rect::new(c.x - d / 2.0, c.y + bob - d / 2.0, d, d),
                        crate::art::Draw {
                            tint,
                            ..Default::default()
                        },
                    );
                    draw_need(vec2(c.x, c.y + bob - 2.0), n);
                }
            }
            Room::Dirty { cleaning } => {
                for k in 0..5 {
                    let fx = r.x + 20.0 + (k * 37 % 70) as f32 / 70.0 * (r.w - 40.0);
                    let fy = r.y + 50.0 + (k * 53 % 50) as f32 / 50.0 * (r.h - 110.0).max(10.0);
                    art::sprite_at(
                        if k % 2 == 0 { "dirt1" } else { "dirt2" },
                        vec2(fx, fy),
                        18.0 + (k % 3) as f32 * 5.0,
                        crate::art::Draw {
                            tint: Color::from_rgba(120, 85, 55, 210),
                            rotation: k as f32,
                            ..Default::default()
                        },
                    );
                }
                match cleaning {
                    Some(c) => {
                        let k = 1.0 - c / CLEAN_TIME;
                        for b in 0..9 {
                            let f = b as f32;
                            let life = (self.t * 0.9 + f * 0.37).fract();
                            let bx = r.x
                                + 20.0
                                + (f * 29.0) % (r.w - 40.0)
                                + (life * 9.0 + f).sin() * 6.0;
                            let by = r.y + r.h - 40.0 - life * (r.h - 80.0);
                            let br = 4.0 + (f * 7.0) % 8.0;
                            crate::ui::sprite_at(
                                "bubble",
                                vec2(bx, by),
                                br * 2.4,
                                crate::art::Draw {
                                    tint: with_alpha(WHITE, 0.9 * (1.0 - life)),
                                    ..Default::default()
                                },
                            );
                        }
                        text_outline(
                            "Cleaning",
                            r.x + r.w / 2.0,
                            r.y + r.h * 0.45,
                            26.0,
                            Color::from_rgba(80, 140, 200, 255),
                            WHITE,
                        );
                        bar(
                            Rect::new(r.x + 14.0, r.y + r.h * 0.55, r.w - 28.0, 16.0),
                            k,
                            "bar_blue",
                        );
                    }
                    None => {
                        let pulse = 1.0 + 0.05 * (self.t * 6.0).sin();
                        text_outline(
                            "Tap to clean",
                            r.x + r.w / 2.0,
                            r.y + r.h * 0.45,
                            24.0 * pulse,
                            Color::from_rgba(160, 100, 60, 255),
                            WHITE,
                        );
                    }
                }
            }
            _ => {}
        }
        if matches!(self.rooms[i], Room::Free | Room::Dirty { .. }) && lvl < MAX_LEVEL {
            let up = Self::upgrade_rect(r);
            let can = self.coins >= upgrade_price(lvl);
            let bob = if can {
                (self.t * 5.0 + i as f32).sin() * 2.0
            } else {
                0.0
            };
            let c = up.center() + vec2(0.0, bob);
            let base = if can { "round_yellow" } else { "round_white" };
            crate::ui::sprite_at(base, c, up.h, draw);
            art::sprite_at(
                "plus",
                c - vec2(0.0, 2.0),
                up.h * 0.5,
                crate::art::Draw {
                    tint: if can { WHITE } else { crate::ui::BODY },
                    ..draw
                },
            );
        }
    }
}

impl Game for Hotel {
    fn update(&mut self, input: &Input, view: &View, dt: f32) {
        let (_, rects) = Self::layout(view);
        if input.pressed {
            for (i, r) in rects.iter().enumerate() {
                let up = Self::upgrade_rect(*r);
                if matches!(self.rooms[i], Room::Free | Room::Dirty { .. })
                    && up.contains(input.pos)
                {
                    self.try_upgrade(i);
                } else if r.contains(input.pos) {
                    self.tap_room(i);
                }
            }
        }
        for (i, earned) in self.step(dt) {
            self.popups.add(
                rects[i].center(),
                format!("+{earned}"),
                Color::from_rgba(255, 210, 60, 255),
                36.0,
            );
            self.fly_coins(rects[i].center(), coins_for(earned as u32));
            self.particles.puff(
                Self::room_spot(rects[i]),
                Color::from_rgba(240, 230, 215, 255),
            );
        }
        self.react(view);
        self.animate(view, dt);
    }

    fn draw(&self, view: &View) {
        clear_background(Color::from_rgba(250, 236, 214, 255));
        let night = night_amount(self.t);
        let sh = self.shake.offset();
        set_view_camera(view, vec2(sh.x, sh.y));
        let (lobby, rects) = Self::layout(view);
        self.draw_lobby(lobby);
        let next_locked = self.rooms.iter().position(|r| *r == Room::Locked);
        for (i, r) in rects.iter().enumerate() {
            self.draw_room(i, *r, next_locked, night);
        }
        for w in &self.walkers {
            let from = Self::lobby_spot(lobby, 0);
            let to = Self::room_spot(rects[w.room]);
            let k = w.t / WALK;
            let p = walk_pos(from, to, k);
            let scale = (Self::queue_slot(lobby) / 110.0).max(0.6) * (1.0 - k) + 0.95 * k;
            draw_guest(p.x, p.y, scale, w.look, Pose::Walk(w.t));
        }
        set_view_camera(view, Vec2::ZERO);
        self.draw_hud(view);
        let target = Self::coin_target(view);
        for c in &self.flying {
            if c.delay > 0.0 {
                continue;
            }
            let p = coin_pos(c.from, target, c.t);
            draw_coin(p, 13.0 * (1.0 - 0.3 * c.t), c.t * 12.0);
        }
        self.particles.draw();
        self.popups.draw();
    }
}

pub fn icon(r: Rect) {
    let draw = crate::art::Draw::default();
    art::sprite(
        "wall",
        Rect::new(r.x - r.w * 0.05, r.y - r.h * 0.05, r.w * 1.1, r.h * 1.1),
    );
    art::sprite("floor", Rect::new(r.x, r.y + r.h * 0.86, r.w, r.h * 0.16));
    let wh = r.h * 0.5;
    art::sprite_at("window", vec2(r.x + r.w * 0.27, r.y + r.h * 0.3), wh, draw);
    let bs = art::size("bed2");
    let bw = r.w * 0.62;
    art::sprite(
        "bed2",
        Rect::new(
            r.x + r.w * 0.02,
            r.y + r.h * 0.92 - bw * bs.y / bs.x,
            bw,
            bw * bs.y / bs.x,
        ),
    );
    art::sprite_at(
        "lamp",
        vec2(r.x + r.w * 0.52, r.y + r.h * 0.55),
        r.h * 0.5,
        draw,
    );
    art::sprite_at(
        "g3_cheer",
        vec2(r.x + r.w * 0.76, r.y + r.h * 0.6),
        r.h * 0.62,
        draw,
    );
    crate::ui::sprite_at(
        "coin",
        vec2(r.x + r.w * 0.88, r.y + r.h * 0.16),
        r.h * 0.2,
        draw,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hotel_sprites_all_exist() {
        let have = art::names();
        let mut want: Vec<String> = [
            "wall",
            "floor",
            "window",
            "bed1",
            "bed2",
            "lamp",
            "plant",
            "need_towel",
            "need_food",
            "need_music",
            "dirt1",
            "dirt2",
            "moon",
            "plus",
            "bulb_red",
            "bulb_white",
            "bar_back",
            "bar_green",
            "bar_yellow",
            "bar_red",
            "bar_blue",
            "face_sad",
            "heart",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();
        for look in 0..6 {
            for pose in [Pose::Idle, Pose::Think, Pose::Cheer] {
                want.push(guest_sprite(look, pose));
            }
            for k in 0..8 {
                want.push(guest_sprite(look, Pose::Walk(k as f32 / 11.0)));
            }
        }
        for n in &want {
            assert!(have.iter().any(|h| h == n), "hotel sheet lacks {n}");
        }
        for need in [Need::Towel, Need::Food, Need::Music] {
            assert!(have.iter().any(|h| h == need_sprite(need)));
        }
    }

    #[test]
    fn walk_cycle_uses_all_eight_frames() {
        let frames: std::collections::HashSet<String> = (0..80)
            .map(|k| guest_sprite(0, Pose::Walk(k as f32 / 80.0 * WALK)))
            .collect();
        assert_eq!(frames.len(), 8);
    }

    #[test]
    fn glass_sits_inside_the_window_frame() {
        let w = Rect::new(0.0, 0.0, 48.0, 112.0);
        let g = glass_rect(w);
        assert!(w.contains(g.point()) && w.contains(g.point() + g.size()));
        assert!(g.w > 0.0 && g.h > 0.0);
    }

    fn with_guest() -> Hotel {
        let mut h = Hotel::new();
        h.queue.push(Guest {
            look: 0,
            patience: PATIENCE,
        });
        h.spawn_in = 999.0;
        h
    }

    #[test]
    fn full_guest_cycle_pays_and_frees_room() {
        let mut h = with_guest();
        h.tap_room(0);
        assert!(matches!(h.rooms[0], Room::Busy { .. }));
        assert!(h.queue.is_empty());
        let mut paid = 0;
        for _ in 0..(STAY * 10.0) as usize + 2 {
            if let Room::Busy { need: Some(_), .. } = h.rooms[0] {
                h.tap_room(0);
            }
            paid += h.step(0.1).len();
        }
        assert_eq!(paid, 1);
        assert!(h.coins >= pay(1, 1));
        assert_eq!(h.rooms[0], Room::Dirty { cleaning: None });
        h.tap_room(0);
        for _ in 0..(CLEAN_TIME * 10.0) as usize + 2 {
            h.step(0.1);
        }
        assert_eq!(h.rooms[0], Room::Free);
    }

    #[test]
    fn rooms_unlock_in_order_and_cost_coins() {
        let mut h = Hotel::new();
        h.coins = 1000;
        h.tap_room(5);
        assert_eq!(h.rooms[5], Room::Locked);
        h.tap_room(2);
        assert_eq!(h.rooms[2], Room::Free);
        assert_eq!(h.coins, 1000 - room_price(2));
    }

    #[test]
    fn cannot_build_without_coins() {
        let mut h = Hotel::new();
        h.coins = room_price(2) - 1;
        h.tap_room(2);
        assert_eq!(h.rooms[2], Room::Locked);
    }

    #[test]
    fn impatient_guest_leaves_and_costs_rating() {
        let mut h = with_guest();
        let stars = h.stars;
        for _ in 0..(PATIENCE * 10.0) as usize + 2 {
            h.step(0.1);
        }
        assert!(h.queue.is_empty());
        assert!(h.stars < stars);
    }

    #[test]
    fn building_and_upgrading_emit_events() {
        let mut h = Hotel::new();
        h.coins = 1000;
        h.tap_room(2);
        assert_eq!(h.events, vec![Event::Built(2)]);
        h.events.clear();
        assert!(h.try_upgrade(0));
        assert_eq!(h.events, vec![Event::Upgraded(0)]);
    }

    #[test]
    fn broke_build_attempt_is_denied_with_price() {
        let mut h = Hotel::new();
        h.tap_room(2);
        assert_eq!(h.events, vec![Event::Denied(2, room_price(2))]);
        h.events.clear();
        h.tap_room(5);
        assert!(h.events.is_empty());
    }

    #[test]
    fn seating_starts_a_walk_and_tip_flies_a_coin() {
        let v = View::ipad_landscape();
        let mut h = with_guest();
        h.tap_room(0);
        h.update(&Input::default(), &v, 0.01);
        assert_eq!(h.walkers.len(), 1);
        for _ in 0..((WALK / 0.02) as usize + 2) {
            h.update(&Input::default(), &v, 0.02);
        }
        assert!(h.walkers.is_empty());
        assert!(!h.particles.list.is_empty());
        h.rooms[0] = Room::Busy {
            guest: 0,
            left: STAY,
            need: Some((Need::Food, REQUEST_WAIT)),
            mood: 1,
        };
        h.tap_room(0);
        h.update(&Input::default(), &v, 0.01);
        assert_eq!(h.flying.len(), 1);
        assert!(h.popups.list.iter().any(|p| p.text == "+3"));
    }

    #[test]
    fn payout_flies_coins_that_land_on_the_counter() {
        let v = View::ipad_landscape();
        let mut h = with_guest();
        h.tap_room(0);
        h.rooms[0] = Room::Busy {
            guest: 0,
            left: 0.01,
            need: None,
            mood: 1,
        };
        h.update(&Input::default(), &v, 0.05);
        assert_eq!(h.flying.len(), coins_for(pay(1, 1)));
        for _ in 0..60 {
            h.update(&Input::default(), &v, 0.05);
        }
        assert!(h.flying.is_empty());
        assert!(h.coin_bump.t > 0.0);
    }

    #[test]
    fn clean_events_fire_for_start_and_finish() {
        let mut h = Hotel::new();
        h.spawn_in = 999.0;
        h.rooms[0] = Room::Dirty { cleaning: None };
        h.tap_room(0);
        assert_eq!(h.events, vec![Event::CleanStarted(0)]);
        h.events.clear();
        h.step(CLEAN_TIME + 0.1);
        assert_eq!(h.events, vec![Event::Cleaned(0)]);
    }

    #[test]
    fn walk_and_coin_paths_hit_their_endpoints() {
        let (a, b) = (vec2(0.0, 0.0), vec2(100.0, 50.0));
        assert_eq!(walk_pos(a, b, 0.0), a);
        assert!(walk_pos(a, b, 1.0).distance(b) < 1e-3);
        assert!(walk_pos(a, b, 0.5).y < 25.0);
        assert_eq!(coin_pos(a, b, 0.0), a);
        assert!(coin_pos(a, b, 1.0).distance(b) < 1e-3);
        assert!(coin_pos(a, b, 0.5).y < 25.0 - 30.0);
    }

    #[test]
    fn day_cycle_reaches_night_and_returns() {
        assert_eq!(night_amount(0.0), 0.0);
        assert!(night_amount(DAY / 2.0) > 0.99);
        assert!(night_amount(DAY) < 0.01);
        assert!(night_amount(DAY * 0.3) > 0.0 && night_amount(DAY * 0.3) < 1.0);
    }

    #[test]
    fn bigger_payouts_fly_more_coins_up_to_a_cap() {
        assert_eq!(coins_for(1), 2);
        assert!(coins_for(30) > coins_for(10));
        assert_eq!(coins_for(10_000), 8);
    }

    #[test]
    fn upgrade_raises_pay() {
        let mut h = Hotel::new();
        h.coins = upgrade_price(1);
        assert!(h.try_upgrade(0));
        assert_eq!(h.levels[0], 2);
        assert_eq!(h.coins, 0);
        assert!(pay(2, 0) > pay(1, 0));
        assert!(!h.try_upgrade(0));
    }
}
