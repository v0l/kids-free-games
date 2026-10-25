use crate::engine::draw_circle;
use crate::engine::*;
use crate::feel::{self, Impact};
use crate::fx::*;
use crate::sfx::{self, Sfx};
use macroquad::prelude::*;

pub const ROOMS: usize = 12;
pub const COLS: usize = 4;
pub const START_COINS: u32 = 0;
pub const TIP: u32 = 3;
pub const QUEUE_MAX: usize = 5;
pub const PATIENCE: f32 = 18.0;
pub const STAY: f32 = 9.0;
pub const CLEAN_TIME: f32 = 1.2;
pub const REQUEST_WAIT: f32 = 6.0;
pub const MAX_LEVEL: u8 = 3;
pub const WALK: f32 = 0.9;
pub const FLIGHT: f32 = 0.7;
pub const DAY: f32 = 50.0;
pub const LOOKS: usize = 5;
pub const MAX_DAY: u32 = 20;
pub const GAME: &str = "hotel";
pub const INTRO: f32 = 3.0;
pub const END_GUARD: f32 = 0.9;
pub const EASE_AFTER: i64 = 2;

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
    Halfway,
    Won,
    Lost,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Day {
    pub number: u32,
    pub target: u32,
    pub limit: Option<f32>,
    pub rooms: usize,
    pub room_level: u8,
    pub patience: Option<f32>,
    pub need_rate: f32,
    pub need_wait: f32,
    pub pace: f32,
    pub queue: usize,
    pub shop: bool,
}

impl Day {
    pub fn for_level(number: u32, eased: bool) -> Self {
        let n = number.clamp(1, MAX_DAY);
        let k = (n - 1) as f32;
        let mut d = Self {
            number: n,
            target: (36.0 + 22.0 * k + 0.9 * k * k) as u32,
            limit: (n > 2).then_some(80.0 + 3.0 * k),
            rooms: (2 + (n as usize - 1) / 3).min(6),
            room_level: 1 + (n >= 9) as u8 + (n >= 15) as u8,
            patience: match n {
                1 => None,
                2 => Some(40.0),
                _ => Some((32.0 - k * 0.8).max(16.0)),
            },
            need_rate: match n {
                1 => 0.0,
                2 => 0.06,
                _ => 0.12,
            },
            need_wait: (10.0 - k * 0.5).max(REQUEST_WAIT),
            pace: (1.3 - 0.06 * k).max(0.85),
            queue: (1 + n as usize).min(QUEUE_MAX),
            shop: n >= 3,
        };
        if eased {
            d.target = d.target * 4 / 5;
            d.limit = d.limit.map(|l| l * 1.2);
            d.patience = d.patience.map(|p| p * 1.5);
            d.pace *= 1.15;
        }
        d
    }

    pub fn goal_line(&self) -> String {
        match self.limit {
            Some(l) => format!("Earn ${} in {}", self.target, clock(l)),
            None => format!("Earn ${}", self.target),
        }
    }
}

pub fn gap_for(open: usize, pace: f32) -> f32 {
    ((7.5 - open as f32 * 0.35) * pace).max(1.8)
}

pub fn clock(secs: f32) -> String {
    let s = secs.ceil().max(0.0) as u32;
    format!("{}:{:02}", s / 60, s % 60)
}

pub fn stars_for_leavers(left: u32) -> u8 {
    match left {
        0 => 3,
        1..=2 => 2,
        _ => 1,
    }
}

pub fn day_score(day: &Day, stars: u8, seconds_left: f32) -> i64 {
    day.target as i64 + stars as i64 * 50 + seconds_left.max(0.0) as i64 * 2
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Mode {
    Select,
    Playing,
    Won { stars: u8 },
    Lost,
}

pub fn eased_for(day: u32) -> bool {
    crate::profile::get(GAME, "fails_day") == day as i64
        && crate::profile::get(GAME, "fails") >= EASE_AFTER
}

pub fn continue_day() -> u32 {
    crate::profile::unlocked(GAME).clamp(1, MAX_DAY)
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
    pub max: f32,
}

pub struct Hotel {
    pub rooms: [Room; ROOMS],
    pub levels: [u8; ROOMS],
    pub queue: Vec<Guest>,
    pub coins: u32,
    pub left: u32,
    pub served: u32,
    pub day: Day,
    pub mode: Mode,
    pub earned: u32,
    pub built: u32,
    pub tips: u32,
    pub age: f32,
    pub end_age: f32,
    pub rank: Option<usize>,
    pub score: i64,
    pub easing: bool,
    pub clock: Option<f32>,
    half_said: bool,
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
        let first = crate::profile::unlocked(GAME) == 0;
        let mut h = Self::blank(Day::for_level(continue_day(), false));
        if first {
            h.begin(1);
        } else {
            h.mode = Mode::Select;
        }
        h
    }

    fn blank(day: Day) -> Self {
        Self {
            rooms: [Room::Locked; ROOMS],
            levels: [1; ROOMS],
            queue: vec![],
            coins: START_COINS,
            left: 0,
            served: 0,
            day,
            mode: Mode::Select,
            earned: 0,
            built: 0,
            tips: 0,
            age: 0.0,
            end_age: 0.0,
            rank: None,
            score: 0,
            easing: false,
            clock: None,
            half_said: false,
            popups: Popups::default(),
            events: vec![],
            spawn_in: 0.6,
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

    pub fn begin(&mut self, number: u32) {
        let easing = eased_for(number);
        let day = Day::for_level(number, easing);
        let t = self.t;
        *self = Self::blank(day);
        self.t = t;
        self.easing = easing;
        self.clock = day.limit;
        for i in 0..ROOMS {
            self.levels[i] = day.room_level;
            if i < day.rooms {
                self.rooms[i] = Room::Free;
            }
        }
        if crate::profile::unlocked(GAME) == 0 {
            crate::profile::unlock(GAME, 1);
        }
        self.mode = Mode::Playing;
    }

    fn gain(&mut self, n: u32) {
        self.coins += n;
        self.earned += n;
    }

    pub fn progress(&self) -> f32 {
        (self.earned as f32 / self.day.target as f32).min(1.0)
    }

    pub fn arrival_gap(&self) -> f32 {
        let open = self.rooms.iter().filter(|r| **r != Room::Locked).count();
        gap_for(open, self.day.pace)
    }

    pub fn live_stars(&self) -> u8 {
        stars_for_leavers(self.left)
    }

    pub fn final_stars(&self) -> u8 {
        let rushed = match (self.clock, self.day.limit) {
            (Some(c), Some(l)) => c / l < 0.1,
            _ => false,
        };
        self.live_stars().min(if rushed { 2 } else { 3 })
    }

    pub fn tap_room(&mut self, i: usize) {
        match self.rooms[i] {
            Room::Locked if !self.day.shop => {}
            Room::Locked => {
                let next_locked = self.rooms.iter().position(|r| *r == Room::Locked);
                let price = room_price(i);
                if next_locked == Some(i) && self.coins >= price {
                    self.coins -= price;
                    self.rooms[i] = Room::Free;
                    self.built += 1;
                    self.events.push(Event::Built(i));
                } else if next_locked == Some(i) {
                    self.events.push(Event::Denied(i, price));
                }
            }
            Room::Free => {
                if !self.queue.is_empty() {
                    let g = self.queue.remove(0);
                    let mood = if g.patience >= g.max * 0.5 { 1 } else { 0 };
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
                self.gain(TIP);
                self.tips += 1;
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
        if !self.day.shop
            || self.rooms[i] == Room::Locked
            || lvl >= MAX_LEVEL
            || self.coins < upgrade_price(lvl)
        {
            return false;
        }
        self.coins -= upgrade_price(lvl);
        self.levels[i] += 1;
        self.events.push(Event::Upgraded(i));
        true
    }

    pub fn step(&mut self, dt: f32) -> Vec<(usize, i32)> {
        if self.mode != Mode::Playing {
            return vec![];
        }
        let mut events = vec![];
        self.spawn_in -= dt;
        if self.spawn_in <= 0.0 {
            self.spawn_in = self.arrival_gap() * rand::gen_range(0.7, 1.3);
            if self.queue.len() < self.day.queue {
                let patience = self.day.patience.unwrap_or(PATIENCE);
                self.queue.push(Guest {
                    look: rand::gen_range(0, LOOKS as u8),
                    patience,
                    max: patience,
                });
            }
        }
        let before = self.queue.len();
        if self.day.patience.is_some() {
            for g in &mut self.queue {
                g.patience -= dt;
            }
        }
        self.queue.retain(|g| g.patience > 0.0);
        let left_angry = before - self.queue.len();
        if left_angry > 0 {
            self.left += left_angry as u32;
            self.events.push(Event::Left(left_angry));
        }
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
                        self.gain(earned);
                        self.served += 1;
                        events.push((i, earned as i32));
                        Room::Dirty { cleaning: None }
                    } else {
                        let (need, mood) = match need {
                            Some((_, w)) if w - dt <= 0.0 => (None, mood - 1),
                            Some((n, w)) => (Some((n, w - dt)), mood),
                            None if left > 2.0
                                && rand::gen_range(0.0, 1.0) < dt * self.day.need_rate =>
                            {
                                let n =
                                    [Need::Towel, Need::Food, Need::Music][rand::gen_range(0, 3)];
                                (Some((n, self.day.need_wait)), mood)
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
        self.judge(dt);
        events
    }

    fn judge(&mut self, dt: f32) {
        if !self.half_said && self.earned * 2 >= self.day.target {
            self.half_said = true;
            self.events.push(Event::Halfway);
        }
        if self.earned >= self.day.target {
            let stars = self.final_stars();
            self.mode = Mode::Won { stars };
            self.end_age = 0.0;
            self.score = day_score(&self.day, stars, self.clock.unwrap_or(0.0));
            self.events.push(Event::Won);
            self.save_result();
            return;
        }
        if let Some(c) = &mut self.clock {
            *c -= dt;
            if *c <= 0.0 {
                *c = 0.0;
                self.mode = Mode::Lost;
                self.end_age = 0.0;
                self.events.push(Event::Lost);
                self.save_result();
            }
        }
    }

    fn save_result(&mut self) {
        let n = self.day.number;
        match self.mode {
            Mode::Won { stars } => {
                crate::profile::award_stars(GAME, n, stars);
                let next = (n + 1).min(MAX_DAY);
                crate::profile::unlock(GAME, next);
                if crate::profile::level(GAME) < next {
                    crate::profile::set_level(GAME, next);
                }
                crate::profile::set(GAME, "fails", 0);
                self.rank =
                    crate::profile::submit(GAME, self.score, crate::profile::Better::Higher);
            }
            Mode::Lost => {
                let same = crate::profile::get(GAME, "fails_day") == n as i64;
                let fails = if same {
                    crate::profile::get(GAME, "fails") + 1
                } else {
                    1
                };
                crate::profile::set(GAME, "fails_day", n as i64);
                crate::profile::set(GAME, "fails", fails);
            }
            _ => {}
        }
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
        vec2(view.w * 0.3, 35.0)
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
                Event::Halfway => {
                    sfx::play(Sfx::Score);
                    sfx::say(sfx::Say::Great);
                    self.popups
                        .add(vec2(view.w * 0.55, 110.0), "Halfway there!", gold, 44.0);
                }
                Event::Won => {
                    sfx::play(Sfx::Win);
                    sfx::say(sfx::Say::Congrats);
                    feel::impact_tinted(Impact::Large, gold);
                    self.particles.confetti(view);
                }
                Event::Lost => {
                    sfx::play(Sfx::Lose);
                    feel::impact(Impact::Medium);
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
    let l = look as usize % LOOKS;
    match mood {
        Pose::Cheer => format!("guest{l}_cheer"),
        _ => format!("guest{l}_idle"),
    }
}

pub fn guest_height(s: f32) -> f32 {
    100.0 * s
}

pub fn walk_sway(t: f32) -> f32 {
    (t / WALK * 5.0 * std::f32::consts::PI).sin() * 0.1
}

fn draw_guest(x: f32, y: f32, s: f32, look: u8, mood: Pose) {
    let feet = y + 42.0 * s;
    soft_shadow(vec2(x, feet), 22.0 * s, 5.0 * s, 0.6);
    let h = guest_height(s);
    let rotation = match mood {
        Pose::Walk(t) => walk_sway(t),
        _ => 0.0,
    };
    let tint = match mood {
        Pose::Think => Color::from_rgba(255, 205, 205, 255),
        _ => WHITE,
    };
    art::sprite_at(
        &guest_sprite(look, mood),
        vec2(x, feet - h / 2.0),
        h,
        crate::art::Draw {
            rotation,
            tint,
            pivot: vec2(0.5, 0.95),
            ..Default::default()
        },
    );
}

fn draw_coin(c: Vec2, r: f32, spin: f32) {
    let w = r * 2.0 * spin.cos().abs().max(0.2);
    crate::ui::sprite_ex(
        "p_coin",
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

const WOOD: Color = Color::new(0.62, 0.38, 0.24, 1.0);
const WOOD_LIGHT: Color = Color::new(0.82, 0.58, 0.38, 1.0);

fn frame(r: Rect, radius: f32, tint: Color) {
    round_rect(
        Rect::new(r.x, r.y + 3.0, r.w, r.h),
        radius,
        Color::new(0.2, 0.08, 0.05, 0.35),
    );
    round_rect(r, radius, darker(tint, 0.62));
    round_rect(
        Rect::new(r.x + 2.0, r.y + 2.0, r.w - 4.0, r.h - 5.0),
        radius,
        tint,
    );
    draw_rectangle(
        r.x + radius,
        r.y + 3.0,
        r.w - radius * 2.0,
        2.0,
        Color::new(1.0, 1.0, 1.0, 0.35),
    );
}

fn window_size(h: f32) -> Vec2 {
    let s = art::size("curtains");
    vec2(h * s.x / s.y, h)
}

pub fn glass_rect(w: Rect) -> Rect {
    Rect::new(w.x + w.w * 0.26, w.y + w.h * 0.1, w.w * 0.48, w.h * 0.78)
}

fn round_rect_lines(r: Rect, radius: f32, thickness: f32, color: Color) {
    let rad = radius.min(r.w / 2.0).min(r.h / 2.0);
    let corners = [
        (
            vec2(r.x + r.w - rad, r.y + rad),
            -std::f32::consts::FRAC_PI_2,
        ),
        (vec2(r.x + r.w - rad, r.y + r.h - rad), 0.0),
        (
            vec2(r.x + rad, r.y + r.h - rad),
            std::f32::consts::FRAC_PI_2,
        ),
        (vec2(r.x + rad, r.y + rad), std::f32::consts::PI),
    ];
    let mut pts = vec![];
    for (c, start) in corners {
        for k in 0..=6 {
            let a = start + k as f32 / 6.0 * std::f32::consts::FRAC_PI_2;
            pts.push(c + vec2(a.cos(), a.sin()) * rad);
        }
    }
    for k in 0..pts.len() {
        let (a, b) = (pts[k], pts[(k + 1) % pts.len()]);
        draw_line(a.x, a.y, b.x, b.y, thickness, color);
    }
}

fn moon(c: Vec2, r: f32, sky: Color, a: f32) {
    draw_circle(
        c.x,
        c.y,
        r * 1.5,
        with_alpha(Color::new(1.0, 0.95, 0.6, 1.0), 0.18 * a),
    );
    draw_circle(c.x, c.y, r, with_alpha(Color::new(1.0, 0.93, 0.55, 1.0), a));
    draw_circle(c.x + r * 0.45, c.y - r * 0.2, r * 0.85, with_alpha(sky, a));
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
    let fw = 4.0;
    round_rect(
        Rect::new(g.x - fw, g.y - fw, g.w + fw * 2.0, g.h + fw * 2.0),
        5.0,
        Color::from_rgba(250, 246, 238, 255),
    );
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
        moon(
            vec2(g.x + g.w * 0.5, g.y + g.h * 0.3),
            g.w * 0.2,
            mix(day_top, dark_top),
            a,
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
    let bar = Color::from_rgba(250, 246, 238, 255);
    draw_rectangle(g.x + g.w / 2.0 - 1.5, g.y, 3.0, g.h, bar);
    draw_rectangle(g.x, g.y + g.h * 0.45, g.w, 3.0, bar);
    draw_rectangle(g.x, g.y, g.w, g.h * 0.12, Color::new(1.0, 1.0, 1.0, 0.18));
    art::sprite("curtains", r);
}

fn bar(r: Rect, k: f32, fill: &str) {
    let rad = r.h / 2.0;
    round_rect(
        Rect::new(r.x - 1.5, r.y - 1.5, r.w + 3.0, r.h + 3.0),
        rad + 1.5,
        Color::from_rgba(70, 40, 40, 255),
    );
    round_rect(r, rad, Color::from_rgba(120, 90, 90, 255));
    if k > 0.02 {
        let w = (r.w * k).max(r.h);
        let col = match fill {
            "bar_green" => Color::from_rgba(90, 210, 70, 255),
            "bar_yellow" => Color::from_rgba(255, 200, 40, 255),
            "bar_red" => Color::from_rgba(240, 70, 60, 255),
            _ => Color::from_rgba(70, 170, 245, 255),
        };
        let fr = Rect::new(r.x, r.y, w, r.h);
        round_rect(fr, rad, col);
        round_rect(
            Rect::new(
                fr.x + rad * 0.4,
                fr.y + r.h * 0.12,
                (w - rad * 0.8).max(1.0),
                r.h * 0.34,
            ),
            rad * 0.4,
            Color::new(1.0, 1.0, 1.0, 0.45),
        );
    }
}

impl Hotel {
    fn draw_hud(&self, view: &View) {
        let f = view.full();
        let ink = Color::from_rgba(100, 35, 45, 255);
        let top = Rect::new(f.x - 8.0, f.y - 8.0, f.w + 16.0, 62.0 - f.y + 8.0);
        gradient_rect(
            top,
            Color::from_rgba(235, 105, 110, 255),
            Color::from_rgba(185, 55, 70, 255),
        );
        draw_rectangle(
            top.x,
            top.y + top.h - 3.0,
            top.w,
            3.0,
            Color::from_rgba(255, 215, 120, 255),
        );
        draw_rectangle(
            top.x,
            top.y + top.h,
            top.w,
            4.0,
            Color::new(0.3, 0.05, 0.1, 0.35),
        );
        let n = (f.w / 28.0) as usize + 2;
        for k in 0..n {
            let c = vec2(f.x + k as f32 * 28.0 + 14.0, 66.0);
            let lit = k % 2 == 0;
            let base = if lit {
                Color::from_rgba(255, 240, 170, 255)
            } else {
                Color::from_rgba(255, 90, 90, 255)
            };
            draw_circle(c.x, c.y, 11.0, with_alpha(base, 0.25));
            draw_circle(c.x, c.y, 7.0, base);
            draw_circle(c.x - 2.0, c.y - 2.5, 2.4, with_alpha(WHITE, 0.9));
        }
        text_outline(
            &format!("Day {}", self.day.number),
            HUD_LEFT + 60.0,
            38.0,
            38.0,
            WHITE,
            ink,
        );
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
        self.draw_goal(view, ink);
    }

    fn draw_goal(&self, view: &View, ink: Color) {
        let bar_rect = Self::goal_bar(view);
        bar(bar_rect, self.progress(), "bar_green");
        let label = format!(
            "${} / ${}",
            self.earned.min(self.day.target),
            self.day.target
        );
        text_outline(
            &label,
            bar_rect.x + bar_rect.w / 2.0,
            bar_rect.y + bar_rect.h / 2.0,
            22.0,
            WHITE,
            ink,
        );
        let done = match self.mode {
            Mode::Won { stars } => stars,
            _ => self.live_stars(),
        };
        for s in 0..3u8 {
            let on = s < done;
            let wob = if on {
                1.0 + 0.06 * (self.t * 3.0 + s as f32).sin()
            } else {
                1.0
            };
            crate::ui::sprite_at(
                "p_star",
                vec2(view.w - 270.0 + s as f32 * 32.0, 35.0),
                32.0 * wob,
                crate::art::Draw {
                    tint: if on {
                        WHITE
                    } else {
                        Color::from_rgba(120, 60, 70, 150)
                    },
                    ..Default::default()
                },
            );
        }
        if let Some(c) = self.clock {
            let low = c < 15.0 && self.mode == Mode::Playing;
            let k = if low {
                1.0 + 0.08 * (self.t * 8.0).sin()
            } else {
                1.0
            };
            text_outline(
                &clock(c),
                view.w - 160.0,
                35.0,
                32.0 * k,
                if low {
                    Color::from_rgba(255, 190, 170, 255)
                } else {
                    WHITE
                },
                ink,
            );
        }
        crate::ui::painted_button(Self::days_button(view), "Days", "blue", 0.0, 1.0);
    }

    pub fn goal_bar(view: &View) -> Rect {
        Rect::new(view.w * 0.43, 21.0, view.w - 290.0 - view.w * 0.43, 28.0)
    }

    pub fn days_button(view: &View) -> Rect {
        Rect::new(view.w - 112.0, 10.0, 100.0, 50.0)
    }

    fn draw_lobby(&self, lobby: Rect) {
        frame(lobby, 18.0, WOOD);
        let carpet = Rect::new(
            lobby.x + 8.0,
            lobby.y + 92.0,
            lobby.w - 16.0,
            lobby.h - 104.0,
        );
        frame(carpet, 12.0, Color::from_rgba(215, 70, 90, 255));
        let sign = Rect::new(lobby.x + 10.0, lobby.y + 10.0, lobby.w - 20.0, 70.0);
        frame(sign, 14.0, Color::from_rgba(150, 92, 58, 255));
        art::sprite_at(
            "plant",
            vec2(sign.x + 24.0, sign.y + sign.h * 0.5),
            sign.h - 6.0,
            crate::art::Draw::default(),
        );
        art::sprite_at(
            "lamp",
            vec2(sign.x + sign.w - 20.0, sign.y + sign.h * 0.5),
            sign.h - 6.0,
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
                frame(card, 12.0, Color::from_rgba(255, 244, 214, 255));
                round_rect_lines(
                    card,
                    12.0,
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
                let c = vec2(cx + 34.0 * s, y + slot * 0.42 - 44.0 * s - bob);
                draw_circle(c.x, c.y + 2.0, 14.0, Color::new(0.0, 0.0, 0.0, 0.25));
                draw_circle(c.x, c.y, 14.0, Color::from_rgba(240, 70, 60, 255));
                draw_circle(c.x - 4.0, c.y - 5.0, 4.0, with_alpha(WHITE, 0.5));
                text_center("!", c.x, c.y, 24.0, WHITE);
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
        frame(r, 14.0, Color::from_rgba(205, 175, 140, 255));
        let inner = Rect::new(r.x + 8.0, r.y + 8.0, r.w - 16.0, r.h - 18.0);
        round_rect(inner, 8.0, Color::from_rgba(150, 120, 96, 255));
        round_rect(
            Rect::new(inner.x, inner.y + inner.h * 0.5, inner.w, inner.h * 0.5),
            8.0,
            Color::from_rgba(130, 100, 80, 255),
        );
        let c = r.center();
        if next_locked != Some(i) {
            crate::ui::sprite_at(
                "icon_lock",
                c,
                44.0,
                crate::art::Draw {
                    tint: Color::from_rgba(70, 50, 40, 255),
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
        let color = if can { "green" } else { "blue" };
        let b = Rect::new(r.x + 14.0, c.y - 6.0, r.w - 28.0, 60.0);
        crate::ui::painted_button(
            b,
            &format!("${}", room_price(i)),
            color,
            press * 0.4,
            if can { 1.0 } else { 0.7 },
        );
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
        frame(r, 14.0, WOOD_LIGHT);
        let inner = Rect::new(r.x + 6.0, r.y + 6.0, r.w - 12.0, r.h - 14.0);
        art::sprite(&format!("wall{lvl}"), inner);
        let floor_h = (inner.h * 0.14).clamp(16.0, 30.0);
        let floor = Rect::new(inner.x, inner.y + inner.h - floor_h, inner.w, floor_h);
        art::sprite("floor", floor);
        draw_rectangle(
            floor.x,
            floor.y,
            floor.w,
            3.0,
            Color::new(0.2, 0.1, 0.05, 0.25),
        );
        let floor_y = floor.y + floor.h * 0.45;
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
        frame(badge, 8.0, Color::from_rgba(150, 100, 70, 255));
        text_center(
            &format!("{}", i + 1),
            badge.x + badge.w / 2.0,
            badge.y + badge.h / 2.0 - 1.0,
            24.0,
            WHITE,
        );
        for s in 0..lvl {
            crate::ui::sprite_at(
                "p_star",
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
                round_rect_lines(
                    Rect::new(r.x + 2.0, r.y + 2.0, r.w - 4.0, r.h - 4.0),
                    14.0,
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
                        art::sprite_at("heart", hp, 26.0, draw);
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
                    let fy = r.y + r.h * 0.6 + (k * 53 % 50) as f32 / 50.0 * r.h * 0.25;
                    art::sprite_at(
                        "dirt",
                        vec2(fx, fy),
                        20.0 + (k % 3) as f32 * 7.0,
                        crate::art::Draw {
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
                            let a = 1.0 - life;
                            draw_circle(
                                bx,
                                by,
                                br * 1.2,
                                with_alpha(Color::from_rgba(190, 230, 255, 255), 0.55 * a),
                            );
                            draw_circle(
                                bx - br * 0.35,
                                by - br * 0.4,
                                br * 0.3,
                                with_alpha(WHITE, 0.9 * a),
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
        if self.day.shop
            && matches!(self.rooms[i], Room::Free | Room::Dirty { .. })
            && lvl < MAX_LEVEL
        {
            let up = Self::upgrade_rect(r);
            let can = self.coins >= upgrade_price(lvl);
            let bob = if can {
                (self.t * 5.0 + i as f32).sin() * 2.0
            } else {
                0.0
            };
            let c = up.center() + vec2(0.0, bob);
            crate::ui::sprite_at(
                "p_round",
                c,
                up.h,
                crate::art::Draw {
                    tint: if can {
                        WHITE
                    } else {
                        Color::from_rgba(190, 190, 200, 255)
                    },
                    ..draw
                },
            );
            let arm = up.h * 0.2;
            let th = up.h * 0.1;
            let ink = if can {
                WHITE
            } else {
                Color::from_rgba(110, 100, 120, 255)
            };
            draw_rectangle(c.x - arm, c.y - th - 1.0, arm * 2.0, th * 2.0, ink);
            draw_rectangle(c.x - th, c.y - arm - 1.0, th * 2.0, arm * 2.0, ink);
        }
    }
}

impl Game for Hotel {
    fn update(&mut self, input: &Input, view: &View, dt: f32) {
        self.age += dt;
        self.t += dt;
        let (_, rects) = Self::layout(view);
        if self.mode == Mode::Select {
            self.select_input(input, view);
            self.particles.update(dt);
            self.popups.update(dt);
            return;
        }
        if input.pressed && Self::days_button(view).contains(input.pos) {
            sfx::play(Sfx::Click);
            self.mode = Mode::Select;
            return;
        }
        match self.mode {
            Mode::Playing => {
                if input.pressed {
                    for (i, r) in rects.iter().enumerate() {
                        let up = Self::upgrade_rect(*r);
                        if self.day.shop
                            && matches!(self.rooms[i], Room::Free | Room::Dirty { .. })
                            && up.contains(input.pos)
                        {
                            self.try_upgrade(i);
                        } else if r.contains(input.pos) {
                            self.tap_room(i);
                        }
                    }
                }
            }
            Mode::Won { .. } | Mode::Lost => {
                self.end_age += dt;
                if self.end_age >= END_GUARD && (input.pressed || input.action) {
                    self.finish_screen();
                }
            }
            Mode::Select => {}
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
        if self.mode == Mode::Select {
            self.draw_select(view);
            return;
        }
        clear_background(Color::from_rgba(250, 236, 214, 255));
        let night = night_amount(self.t);
        let sh = self.shake.offset();
        set_view_camera(view, vec2(sh.x, sh.y));
        let (lobby, rects) = Self::layout(view);
        self.draw_lobby(lobby);
        let next_locked = self
            .rooms
            .iter()
            .position(|r| *r == Room::Locked)
            .filter(|_| self.day.shop);
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
        self.draw_guidance(view);
        self.particles.draw();
        self.popups.draw();
        self.draw_end(view);
    }
}

pub fn select_layout(view: &View) -> (Vec<Rect>, Rect) {
    let cols = if view.w >= view.h { 5 } else { 4 };
    let rows = (MAX_DAY as usize).div_ceil(cols);
    let top = 150.0;
    let bottom = 110.0;
    let size = ((view.w - 60.0) / cols as f32).min((view.h - top - bottom) / rows as f32);
    let gw = size * cols as f32;
    let gh = size * rows as f32;
    let ox = (view.w - gw) / 2.0;
    let oy = top + (view.h - top - bottom - gh) / 2.0;
    let tiles = (0..MAX_DAY as usize)
        .map(|i| {
            Rect::new(
                ox + (i % cols) as f32 * size + 5.0,
                oy + (i / cols) as f32 * size + 5.0,
                size - 10.0,
                size - 10.0,
            )
        })
        .collect();
    let play = Rect::new(view.w / 2.0 - 160.0, view.h - bottom + 12.0, 320.0, 80.0);
    (tiles, play)
}

impl Hotel {
    fn select_input(&mut self, input: &Input, view: &View) {
        if !input.pressed {
            return;
        }
        let (tiles, play) = select_layout(view);
        let open = crate::profile::unlocked(GAME).max(1);
        for (i, t) in tiles.iter().enumerate() {
            if t.contains(input.pos) && (i as u32) < open {
                sfx::play(Sfx::Tap);
                self.begin(i as u32 + 1);
                return;
            }
        }
        if play.contains(input.pos) {
            sfx::play(Sfx::Tap);
            self.begin(continue_day());
        }
    }

    fn finish_screen(&mut self) {
        let n = self.day.number;
        match self.mode {
            Mode::Won { .. } if n >= MAX_DAY => {
                self.mode = Mode::Select;
            }
            Mode::Won { .. } => self.begin(n + 1),
            _ => self.begin(n),
        }
    }

    pub fn hint_text(&self) -> Option<&'static str> {
        if self.mode != Mode::Playing || self.day.number != 1 || crate::profile::stars(GAME, 1) > 0
        {
            return None;
        }
        let dirty = self
            .rooms
            .iter()
            .any(|r| matches!(r, Room::Dirty { cleaning: None }));
        let asks = self
            .rooms
            .iter()
            .any(|r| matches!(r, Room::Busy { need: Some(_), .. }));
        let free = self.rooms.contains(&Room::Free);
        if asks {
            Some("Tap the bubble to help")
        } else if dirty {
            Some("Tap the mess to clean")
        } else if !self.queue.is_empty() && free {
            Some("Tap a room to check in")
        } else if self.served == 0 {
            Some("A guest is coming")
        } else {
            None
        }
    }

    fn draw_guidance(&self, view: &View) {
        if self.mode != Mode::Playing {
            return;
        }
        let (_, rects) = Self::layout(view);
        let at = vec2(
            (rects[0].x + rects[3].x + rects[3].w) / 2.0,
            rects[0].y - 36.0,
        );
        let pulse = 1.0 + 0.04 * (self.t * 5.0).sin();
        if let Some(h) = self.hint_text() {
            crate::ui::hint(view, h, at, pulse);
        } else if self.age < INTRO {
            crate::ui::hint(view, &self.day.goal_line(), at, 1.0);
        }
    }

    fn draw_end(&self, view: &View) {
        let k = self.end_age;
        match self.mode {
            Mode::Won { stars } => {
                let title = format!("Day {} done!", self.day.number);
                let line = "";
                let action = if self.day.number >= MAX_DAY {
                    "All days"
                } else {
                    "Next day"
                };
                let r = crate::ui::Results {
                    game: GAME,
                    title: &title,
                    line,
                    rank: self.rank,
                    action,
                    format: crate::ui::plain_score,
                };
                results(view, &r);
                let board = crate::profile::board(GAME);
                let rows = crate::ui::results_rows(board.len(), self.rank).len();
                let ph = 230.0 + rows as f32 * 46.0 + if rows == 0 { 0.0 } else { 20.0 };
                let c = view.center() + vec2(0.0, 24.0);
                let y = c.y - ph / 2.0 + 90.0;
                for s in 0..3u8 {
                    let pop = ease_out_back(((k - 0.35 - s as f32 * 0.18) / 0.3).clamp(0.0, 1.0));
                    let on = s < stars;
                    crate::ui::sprite_at(
                        "p_star",
                        vec2(c.x + (s as f32 - 1.0) * 54.0, y),
                        48.0 * pop.max(0.01),
                        crate::art::Draw {
                            tint: if on {
                                WHITE
                            } else {
                                Color::from_rgba(120, 100, 130, 130)
                            },
                            ..Default::default()
                        },
                    );
                }
            }
            Mode::Lost => {
                let s = format!("Earned ${} of ${}", self.earned, self.day.target);
                overlay(view, "Time's up!", &[&s, "Try again"]);
            }
            _ => {}
        }
    }

    fn draw_select(&self, view: &View) {
        sky(
            view,
            Color::from_rgba(255, 226, 170, 255),
            Color::from_rgba(255, 170, 160, 255),
        );
        set_view_camera(view, Vec2::ZERO);
        let (tiles, play) = select_layout(view);
        let open = crate::profile::unlocked(GAME).max(1);
        let next = continue_day();
        crate::ui::ribbon(view, "Pick a day", vec2(view.w / 2.0, 90.0), self.age);
        let stars = crate::profile::total_stars(GAME);
        text_outline(
            &format!("{} / {}", stars, MAX_DAY * 3),
            view.w - 90.0,
            90.0,
            30.0,
            WHITE,
            Color::from_rgba(140, 40, 50, 255),
        );
        crate::ui::sprite_at(
            "p_star",
            vec2(view.w - 160.0 - 8.0, 90.0),
            42.0,
            crate::art::Draw::default(),
        );
        for (i, t) in tiles.iter().enumerate() {
            let n = i as u32 + 1;
            let locked = n > open;
            let current = n == next;
            let pulse = if current {
                1.0 + 0.04 * (self.age * 5.0).sin()
            } else {
                1.0
            };
            let c = t.center();
            let r = Rect::new(
                c.x - t.w * pulse / 2.0,
                c.y - t.h * pulse / 2.0,
                t.w * pulse,
                t.h * pulse,
            );
            let tint = if locked {
                Color::from_rgba(185, 160, 150, 255)
            } else if current {
                Color::from_rgba(255, 214, 90, 255)
            } else {
                Color::from_rgba(255, 244, 226, 255)
            };
            frame(r, 16.0, if locked { darker(tint, 0.9) } else { tint });
            if locked {
                crate::ui::sprite_at(
                    "icon_lock",
                    c,
                    r.h * 0.4,
                    crate::art::Draw {
                        tint: Color::from_rgba(90, 70, 70, 255),
                        ..Default::default()
                    },
                );
                continue;
            }
            text_outline(
                &n.to_string(),
                c.x,
                r.y + r.h * 0.4,
                r.h * 0.42,
                WHITE,
                Color::from_rgba(140, 40, 50, 255),
            );
            let got = crate::profile::stars(GAME, n);
            for s in 0..3u8 {
                crate::ui::sprite_at(
                    "p_star",
                    vec2(c.x + (s as f32 - 1.0) * r.h * 0.26, r.y + r.h * 0.78),
                    r.h * 0.26,
                    crate::art::Draw {
                        tint: if s < got {
                            WHITE
                        } else {
                            Color::from_rgba(130, 90, 90, 120)
                        },
                        ..Default::default()
                    },
                );
            }
        }
        let label = if crate::profile::stars(GAME, 1) == 0 && next == 1 {
            "Play day 1".to_string()
        } else {
            format!("Play day {next}")
        };
        crate::ui::painted_button(play, &label, "green", 0.0, 1.0);
    }
}

pub fn icon(r: Rect) {
    crate::ui::icon("hotel", r);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hotel_sprites_all_exist() {
        let have = art::names();
        let mut want: Vec<String> = [
            "wall1",
            "wall2",
            "wall3",
            "floor",
            "curtains",
            "bed1",
            "bed2",
            "lamp",
            "plant",
            "need_towel",
            "need_food",
            "need_music",
            "dirt",
            "heart",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();
        for look in 0..LOOKS as u8 {
            for pose in [Pose::Idle, Pose::Think, Pose::Cheer, Pose::Walk(0.3)] {
                want.push(guest_sprite(look, pose));
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
    fn every_spawned_look_has_art() {
        let have = art::names();
        let mut h = day_hotel(1);
        for _ in 0..200 {
            h.step(1.0);
            for g in &h.queue {
                assert!((g.look as usize) < LOOKS);
                assert!(have.iter().any(|n| *n == guest_sprite(g.look, Pose::Idle)));
            }
            h.queue.clear();
        }
    }

    #[test]
    fn walking_guests_sway_and_idle_ones_do_not() {
        let sway: Vec<f32> = (0..20).map(|k| walk_sway(k as f32 / 20.0 * WALK)).collect();
        assert!(sway.iter().any(|s| *s > 0.05) && sway.iter().any(|s| *s < -0.05));
        assert!(sway.iter().all(|s| s.abs() <= 0.1 + 1e-6));
    }

    #[test]
    fn glass_sits_inside_the_window_frame() {
        let w = Rect::new(0.0, 0.0, 48.0, 112.0);
        let g = glass_rect(w);
        assert!(w.contains(g.point()) && w.contains(g.point() + g.size()));
        assert!(g.w > 0.0 && g.h > 0.0);
    }

    fn fresh() {
        crate::save::reset_for_test();
        crate::profile::create("Mia", 0);
    }

    fn day_hotel(n: u32) -> Hotel {
        let mut h = Hotel::blank(Day::for_level(n, false));
        h.begin(n);
        h
    }

    fn with_guest() -> Hotel {
        let mut h = day_hotel(1);
        h.queue.push(Guest {
            look: 0,
            patience: PATIENCE,
            max: PATIENCE,
        });
        h.spawn_in = 999.0;
        h
    }

    fn play_bot(h: &mut Hotel, secs: f32, tap_every: f32) -> f32 {
        let mut cooldown = 0.0;
        let steps = (secs / 0.1) as usize;
        for _ in 0..steps {
            cooldown -= 0.1;
            if cooldown <= 0.0 && h.mode == Mode::Playing {
                let pick = (0..ROOMS)
                    .find(|i| matches!(h.rooms[*i], Room::Busy { need: Some(_), .. }))
                    .or_else(|| (0..ROOMS).find(|i| h.rooms[*i] == Room::Dirty { cleaning: None }))
                    .or_else(|| {
                        (0..ROOMS).find(|i| h.rooms[*i] == Room::Free && !h.queue.is_empty())
                    });
                if let Some(i) = pick {
                    h.tap_room(i);
                    cooldown = tap_every;
                }
            }
            h.step(0.1);
            h.events.clear();
            if h.mode != Mode::Playing {
                break;
            }
        }
        cooldown
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
        let mut h = day_hotel(3);
        h.coins = 1000;
        h.tap_room(5);
        assert_eq!(h.rooms[5], Room::Locked);
        h.tap_room(2);
        assert_eq!(h.rooms[2], Room::Free);
        assert_eq!(h.coins, 1000 - room_price(2));
    }

    #[test]
    fn cannot_build_without_coins() {
        let mut h = day_hotel(3);
        h.coins = room_price(2) - 1;
        h.tap_room(2);
        assert_eq!(h.rooms[2], Room::Locked);
    }

    #[test]
    fn impatient_guest_leaves_and_costs_a_star() {
        let mut h = day_hotel(3);
        h.queue.push(Guest {
            look: 0,
            patience: PATIENCE,
            max: PATIENCE,
        });
        h.spawn_in = 999.0;
        h.coins = 0;
        let stars = h.live_stars();
        for _ in 0..(PATIENCE * 10.0) as usize + 2 {
            h.step(0.1);
        }
        assert!(h.queue.is_empty());
        assert_eq!(h.left, 1);
        assert!(h.live_stars() < stars);
    }

    #[test]
    fn building_and_upgrading_emit_events() {
        let mut h = day_hotel(3);
        h.coins = 1000;
        h.tap_room(2);
        assert_eq!(h.events, vec![Event::Built(2)]);
        h.events.clear();
        assert!(h.try_upgrade(0));
        assert_eq!(h.events, vec![Event::Upgraded(0)]);
    }

    #[test]
    fn broke_build_attempt_is_denied_with_price() {
        let mut h = day_hotel(3);
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
        let mut h = day_hotel(1);
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
        let mut h = day_hotel(3);
        h.coins = upgrade_price(1);
        assert!(h.try_upgrade(0));
        assert_eq!(h.levels[0], 2);
        assert_eq!(h.coins, 0);
        assert!(pay(2, 0) > pay(1, 0));
        assert!(!h.try_upgrade(0));
    }

    #[test]
    fn a_slow_player_wins_day_one_with_three_stars_and_no_timer() {
        fresh();
        let mut h = Hotel::new();
        assert_eq!(h.mode, Mode::Playing);
        assert_eq!(h.clock, None);
        play_bot(&mut h, 600.0, 3.0);
        assert_eq!(h.mode, Mode::Won { stars: 3 });
        assert_eq!(h.left, 0);
    }

    #[test]
    fn day_one_guests_never_lose_patience() {
        fresh();
        let mut h = Hotel::new();
        h.queue.push(Guest {
            look: 0,
            patience: 40.0,
            max: 40.0,
        });
        h.spawn_in = 999.0;
        for _ in 0..3000 {
            h.step(0.1);
        }
        assert_eq!(h.queue.len(), 1);
        assert_eq!(h.left, 0);
    }

    #[test]
    fn day_one_starts_with_a_hint_until_the_first_star() {
        fresh();
        let mut h = Hotel::new();
        assert!(h.hint_text().is_some());
        h.queue.push(Guest {
            look: 0,
            patience: 40.0,
            max: 40.0,
        });
        assert_eq!(h.hint_text(), Some("Tap a room to check in"));
        crate::profile::award_stars(GAME, 1, 1);
        assert_eq!(h.hint_text(), None);
    }

    #[test]
    fn days_get_harder_as_the_number_rises() {
        let days: Vec<Day> = (1..=MAX_DAY).map(|n| Day::for_level(n, false)).collect();
        for w in days.windows(2) {
            assert!(w[1].target > w[0].target);
            assert!(w[1].rooms >= w[0].rooms);
            assert!(w[1].room_level >= w[0].room_level);
            assert!(w[1].pace <= w[0].pace);
            assert!(w[1].need_rate >= w[0].need_rate);
        }
        assert!(days[0].limit.is_none() && days[1].limit.is_none());
        assert!(days[2].limit.is_some());
        assert!(days[19].patience.unwrap() < days[2].patience.unwrap());
        assert!(days[19].rooms > days[0].rooms);
        assert!(!days[0].shop && !days[1].shop && days[2].shop);
    }

    #[test]
    fn an_eased_day_is_gentler_in_every_way() {
        let a = Day::for_level(8, false);
        let b = Day::for_level(8, true);
        assert!(b.target < a.target);
        assert!(b.limit.unwrap() > a.limit.unwrap());
        assert!(b.patience.unwrap() > a.patience.unwrap());
        assert!(b.pace > a.pace);
    }

    #[test]
    fn every_day_goal_is_reachable_by_a_quick_tapper() {
        for n in 1..=MAX_DAY {
            for seed in 0..4 {
                fresh();
                rand::srand(11 + n as u64 * 7 + seed);
                let mut h = day_hotel(n);
                let secs = h.day.limit.unwrap_or(400.0) + 1.0;
                play_bot(&mut h, secs, 1.0);
                assert!(matches!(h.mode, Mode::Won { .. }), "day {n}: {:?}", h.mode);
            }
        }
    }

    #[test]
    fn a_player_who_never_taps_loses_a_timed_day_not_a_free_one() {
        fresh();
        let mut free = day_hotel(1);
        for _ in 0..3000 {
            free.step(0.1);
        }
        assert_eq!(free.mode, Mode::Playing);
        let mut timed = day_hotel(3);
        for _ in 0..1000 {
            timed.step(0.1);
        }
        assert_eq!(timed.mode, Mode::Lost);
    }

    #[test]
    fn winning_a_day_awards_stars_unlocks_the_next_and_submits_a_score() {
        fresh();
        let mut h = day_hotel(1);
        h.earned = h.day.target;
        h.step(0.1);
        let Mode::Won { stars } = h.mode else {
            panic!("not won")
        };
        assert_eq!(stars, 3);
        assert_eq!(crate::profile::stars(GAME, 1), 3);
        assert_eq!(crate::profile::unlocked(GAME), 2);
        assert_eq!(crate::profile::level(GAME), 2);
        assert_eq!(h.rank, Some(0));
        assert_eq!(crate::profile::board(GAME)[0].score, h.score);
        assert!(h.events.contains(&Event::Won));
    }

    #[test]
    fn progress_resumes_per_player_and_new_players_start_at_day_one() {
        fresh();
        for n in 1..=4 {
            let mut h = day_hotel(n);
            h.earned = h.day.target;
            h.step(0.1);
        }
        let back = Hotel::new();
        assert_eq!(back.mode, Mode::Select);
        assert_eq!(back.day.number, 5);
        assert_eq!(continue_day(), 5);
        crate::profile::create("Leo", 1);
        let kid = Hotel::new();
        assert_eq!(kid.mode, Mode::Playing);
        assert_eq!(kid.day.number, 1);
        assert_eq!(crate::profile::unlocked(GAME), 1);
        let first = crate::profile::list()[0].id.clone();
        crate::profile::select(&first);
        assert_eq!(continue_day(), 5);
        assert_eq!(crate::profile::total_stars(GAME), 12);
    }

    #[test]
    fn losing_does_not_cost_progress_and_two_losses_ease_the_day() {
        fresh();
        crate::profile::unlock(GAME, 6);
        crate::profile::set_level(GAME, 6);
        assert!(!eased_for(6));
        for k in 1..=2 {
            let mut h = day_hotel(6);
            h.clock = Some(0.05);
            h.step(0.1);
            assert_eq!(h.mode, Mode::Lost);
            assert_eq!(crate::profile::get(GAME, "fails"), k);
        }
        assert_eq!(continue_day(), 6);
        assert_eq!(crate::profile::unlocked(GAME), 6);
        assert!(eased_for(6));
        assert!(!eased_for(5));
        let mut h = Hotel::blank(Day::for_level(6, false));
        h.begin(6);
        assert!(h.easing);
        assert!(h.day.target < Day::for_level(6, false).target);
        h.earned = h.day.target;
        h.step(0.1);
        assert_eq!(crate::profile::get(GAME, "fails"), 0);
        assert!(!eased_for(6));
    }

    #[test]
    fn a_replay_never_lowers_earned_stars() {
        fresh();
        let mut h = day_hotel(1);
        h.earned = h.day.target;
        h.step(0.1);
        let mut h = day_hotel(1);
        h.left = 5;
        h.earned = h.day.target;
        h.step(0.1);
        assert_eq!(h.mode, Mode::Won { stars: 1 });
        assert_eq!(crate::profile::stars(GAME, 1), 3);
    }

    #[test]
    fn leavers_cost_stars() {
        assert_eq!(stars_for_leavers(0), 3);
        assert_eq!(stars_for_leavers(2), 2);
        assert_eq!(stars_for_leavers(3), 1);
    }

    #[test]
    fn finishing_with_seconds_to_spare_beats_squeaking_by() {
        let d = Day::for_level(5, false);
        assert!(day_score(&d, 3, 30.0) > day_score(&d, 3, 2.0));
        assert!(day_score(&d, 3, 10.0) > day_score(&d, 2, 10.0));
    }

    #[test]
    fn a_squeaker_finish_caps_at_two_stars() {
        fresh();
        let mut h = day_hotel(4);
        h.clock = Some(1.0);
        h.earned = h.day.target;
        h.step(0.1);
        assert_eq!(h.mode, Mode::Won { stars: 2 });
    }

    #[test]
    fn halfway_cheer_fires_once() {
        fresh();
        let mut h = day_hotel(1);
        h.earned = h.day.target / 2;
        h.step(0.1);
        h.step(0.1);
        assert_eq!(h.events.iter().filter(|e| **e == Event::Halfway).count(), 1);
    }

    #[test]
    fn the_day_picker_only_starts_unlocked_days() {
        fresh();
        crate::profile::unlock(GAME, 3);
        let v = View::ipad_landscape();
        let (tiles, play) = select_layout(&v);
        let mut h = Hotel::new();
        assert_eq!(h.mode, Mode::Select);
        h.update(&Input::press_at(tiles[6].center()), &v, 0.02);
        assert_eq!(h.mode, Mode::Select);
        h.update(&Input::press_at(tiles[1].center()), &v, 0.02);
        assert_eq!(h.mode, Mode::Playing);
        assert_eq!(h.day.number, 2);
        h.update(&Input::press_at(Hotel::days_button(&v).center()), &v, 0.02);
        assert_eq!(h.mode, Mode::Select);
        h.update(&Input::press_at(play.center()), &v, 0.02);
        assert_eq!(h.day.number, 3);
    }

    #[test]
    fn tapping_through_the_results_goes_to_the_next_day() {
        fresh();
        let v = View::ipad_landscape();
        let mut h = day_hotel(1);
        h.earned = h.day.target;
        h.update(&Input::default(), &v, 0.1);
        assert!(matches!(h.mode, Mode::Won { .. }));
        h.update(&Input::press_at(v.center()), &v, 0.1);
        assert!(matches!(h.mode, Mode::Won { .. }));
        for _ in 0..20 {
            h.update(&Input::default(), &v, 0.1);
        }
        h.update(&Input::press_at(v.center()), &v, 0.1);
        assert_eq!(h.mode, Mode::Playing);
        assert_eq!(h.day.number, 2);
    }

    #[test]
    fn rooms_and_upgrades_wait_for_day_three() {
        fresh();
        let mut h = day_hotel(2);
        h.coins = 1000;
        h.tap_room(2);
        assert_eq!(h.rooms[2], Room::Locked);
        assert!(!h.try_upgrade(0));
        let mut h = day_hotel(3);
        h.coins = 1000;
        assert!(h.try_upgrade(0));
    }

    #[test]
    fn day_labels_read_as_a_goal() {
        assert_eq!(Day::for_level(1, false).goal_line(), "Earn $36");
        assert!(Day::for_level(3, false).goal_line().contains("in 1:"));
        assert_eq!(clock(61.0), "1:01");
    }
}
