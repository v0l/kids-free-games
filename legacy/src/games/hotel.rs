pub mod sim;

use crate::engine::*;
use crate::engine::{draw_circle, draw_ellipse};
use crate::feel::{self, Impact};
use crate::fx::*;
use crate::sfx::{self, Sfx};
use macroquad::prelude::*;
use sim::*;

pub const GAME: &str = "hotel";
pub const INTRO: f32 = 3.5;
pub const END_GUARD: f32 = 0.9;
pub const EASE_AFTER: i64 = 2;
pub const STICK_R: f32 = 70.0;
const HUD_H: f32 = 74.0;
const GOLD: Color = Color::new(1.0, 0.82, 0.24, 1.0);
const INK: Color = Color::new(0.39, 0.14, 0.18, 1.0);

mod art {
    crate::sprites!("hotel");
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
struct FlyCoin {
    from: Vec2,
    to: Vec2,
    delay: f32,
    t: f32,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Stick {
    pub origin: Option<Vec2>,
    pub knob: Vec2,
}

impl Stick {
    pub fn track(&mut self, input: &Input) -> Option<Vec2> {
        if input.pressed {
            self.origin = Some(input.pos);
        }
        if !input.down {
            self.origin = None;
        }
        let o = self.origin?;
        let d = (input.pos - o).clamp_length_max(STICK_R);
        self.knob = o + d;
        let k = d.length() / STICK_R;
        (k > 0.15).then(|| d.normalize_or_zero() * ((k - 0.15) / 0.85).clamp(0.45, 1.0))
    }
}

pub fn key_dir(input: &Input) -> Option<Vec2> {
    let d = vec2(
        input.right as i32 as f32 - input.left as i32 as f32,
        input.down_key as i32 as f32 - input.up as i32 as f32,
    );
    (d != Vec2::ZERO).then(|| d.normalize())
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Cam {
    pub scale: f32,
    pub offset: Vec2,
}

impl Cam {
    pub fn fit(view: &View) -> Self {
        let area = Rect::new(0.0, HUD_H, view.w, view.h - HUD_H);
        let scale = (area.w / WORLD_W).min(area.h / WORLD_H);
        let offset = vec2(
            area.x + (area.w - WORLD_W * scale) / 2.0,
            area.y + (area.h - WORLD_H * scale) / 2.0,
        );
        Self { scale, offset }
    }

    pub fn follow(view: &View, focus: f32) -> Self {
        let area = Rect::new(0.0, HUD_H, view.w, view.h - HUD_H);
        let fit = Self::fit(view);
        let scale = (area.h / WORLD_H).min(area.w / (WORLD_W * 0.42));
        if scale <= fit.scale * 1.05 {
            return fit;
        }
        let seen = area.w / scale;
        let left = (focus - seen / 2.0).clamp(0.0, WORLD_W - seen);
        let used_h = WORLD_H * scale;
        Self {
            scale,
            offset: vec2(-left * scale, area.y + (area.h - used_h) / 2.0),
        }
    }

    pub fn to_screen(&self, p: Vec2) -> Vec2 {
        self.offset + p * self.scale
    }

    pub fn to_world(&self, p: Vec2) -> Vec2 {
        (p - self.offset) / self.scale
    }

    pub fn rect(&self, r: Rect) -> Rect {
        let p = self.to_screen(r.point());
        Rect::new(p.x, p.y, r.w * self.scale, r.h * self.scale)
    }
}

pub struct Hotel {
    pub sim: Sim,
    pub mode: Mode,
    pub age: f32,
    pub end_age: f32,
    pub rank: Option<usize>,
    pub score: i64,
    pub easing: bool,
    pub stick: Stick,
    pub tap_to: Option<Vec2>,
    pub focus: f32,
    t: f32,
    shown_wallet: f32,
    popups: Popups,
    particles: Particles,
    shake: Shake,
    wallet_bump: Bump,
    built_pop: Vec<(Build, f32)>,
    flying: Vec<FlyCoin>,
    steps: f32,
}

impl Default for Hotel {
    fn default() -> Self {
        Self::new()
    }
}

impl Hotel {
    pub fn new() -> Self {
        let first = crate::profile::unlocked(GAME) == 0;
        let mut h = Self::blank(Day::for_level(continue_day(), false));
        if first {
            h.begin(1);
        }
        h
    }

    fn blank(day: Day) -> Self {
        Self {
            sim: Sim::new(day),
            mode: Mode::Select,
            age: 0.0,
            end_age: 0.0,
            rank: None,
            score: 0,
            easing: false,
            stick: Stick::default(),
            tap_to: None,
            focus: 0.0,
            t: 0.0,
            shown_wallet: 0.0,
            popups: Popups::default(),
            particles: Particles::default(),
            shake: Shake::default(),
            wallet_bump: Bump::default(),
            built_pop: vec![],
            flying: vec![],
            steps: 0.0,
        }
    }

    pub fn begin(&mut self, number: u32) {
        let easing = eased_for(number);
        let t = self.t;
        *self = Self::blank(Day::for_level(number, easing));
        self.t = t;
        self.focus = self.sim.player.pos.x;
        self.easing = easing;
        if crate::profile::unlocked(GAME) == 0 {
            crate::profile::unlock(GAME, 1);
        }
        self.mode = Mode::Playing;
    }

    pub fn day(&self) -> &Day {
        &self.sim.day
    }

    pub fn final_stars(&self) -> u8 {
        let rushed = match (self.sim.clock, self.sim.day.limit) {
            (Some(c), Some(l)) => c / l < 0.1,
            _ => false,
        };
        self.sim.stars().min(if rushed { 2 } else { 3 })
    }

    fn save_result(&mut self) {
        let n = self.sim.day.number;
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

    pub fn control(&mut self, input: &Input, view: &View) {
        let cam = self.cam(view);
        let dir = self.stick.track(input).or_else(|| key_dir(input));
        if input.pressed {
            self.tap_to = Some(cam.to_world(input.pos));
        }
        if input.down && dir.is_some() {
            self.tap_to = None;
        }
        if input.released {
            if let Some(p) = self.tap_to.take() {
                self.sim.walk_to(p);
            }
        }
        self.sim.steer(dir);
    }

    pub fn tick(&mut self, dt: f32) {
        self.sim.step(dt);
        match self.sim.outcome {
            Outcome::Won if self.mode == Mode::Playing => {
                let stars = self.final_stars();
                self.mode = Mode::Won { stars };
                self.end_age = 0.0;
                self.score = day_score(&self.sim.day, stars, self.sim.clock.unwrap_or(0.0));
                self.save_result();
            }
            Outcome::Lost if self.mode == Mode::Playing => {
                self.mode = Mode::Lost;
                self.end_age = 0.0;
                self.save_result();
            }
            _ => {}
        }
    }

    fn react(&mut self, view: &View) {
        let cam = self.cam(view);
        for e in std::mem::take(&mut self.sim.events) {
            match e {
                Event::Arrived => sfx::play(Sfx::Tick),
                Event::CheckedIn(i) => {
                    sfx::play(Sfx::Card);
                    let at = cam.to_screen(SERVE + vec2(0.0, 40.0));
                    self.particles.sparkle(at, GOLD);
                    let _ = i;
                }
                Event::Paid(n) => {
                    sfx::play(Sfx::Coin);
                    let from = cam.to_screen(SERVE + vec2(0.0, 60.0));
                    let to = cam.to_screen(CASH);
                    for k in 0..4 {
                        self.flying.push(FlyCoin {
                            from,
                            to,
                            delay: k as f32 * 0.06,
                            t: 0.0,
                        });
                    }
                    self.popups.add(from, format!("+${n}"), GOLD, 34.0);
                }
                Event::Collected(n) => {
                    sfx::play_step(Sfx::Coin, (n as usize / 10).min(7));
                    self.wallet_bump.kick();
                    let from = cam.to_screen(CASH);
                    let to = wallet_pos(view);
                    for k in 0..coins_for(n) {
                        self.flying.push(FlyCoin {
                            from: from + vec2(rand::gen_range(-20.0, 20.0), 0.0),
                            to,
                            delay: k as f32 * 0.05,
                            t: 0.0,
                        });
                    }
                    self.particles.sparkle(from, GOLD);
                }
                Event::Cleaned(i) => {
                    sfx::play(Sfx::Pop);
                    let at = cam.to_screen(bed_pos(i));
                    self.particles
                        .sparkle(at, Color::from_rgba(150, 220, 255, 255));
                    self.popups
                        .add(at, "Clean!", Color::from_rgba(150, 230, 255, 255), 32.0);
                }
                Event::NeedTowels(i) => {
                    sfx::play(Sfx::Wrong);
                    self.popups.add(
                        cam.to_screen(bed_pos(i)),
                        "Get towels!",
                        Color::from_rgba(255, 140, 120, 255),
                        30.0,
                    );
                }
                Event::Towels => {
                    sfx::play(Sfx::Swish);
                    self.particles
                        .puff(cam.to_screen(CART), Color::from_rgba(240, 245, 255, 255));
                }
                Event::Paying(k) => {
                    sfx::play(Sfx::Tick);
                    let to = cam.to_screen(self.sim.spots[k].pos());
                    self.flying.push(FlyCoin {
                        from: cam.to_screen(self.sim.player.pos + vec2(0.0, -60.0)),
                        to,
                        delay: 0.0,
                        t: 0.0,
                    });
                }
                Event::Built(b) => {
                    sfx::play(Sfx::LevelUp);
                    feel::impact_tinted(Impact::Medium, GOLD);
                    self.shake.add(0.25);
                    let at = cam.to_screen(build_pos(b));
                    self.particles.sparkle(at, GOLD);
                    self.particles.sparkle(at + vec2(0.0, -30.0), WHITE);
                    self.built_pop.push((b, 0.0));
                    let label = match b {
                        Build::Room(_) => "New room!",
                        Build::Clerk => "Hired!",
                        Build::Maid => "Hired!",
                        Build::Sofa | Build::Palm => "Comfy! +$4",
                    };
                    self.popups.add(at, label, GOLD, 40.0);
                }
                Event::Angry(p) => {
                    sfx::play(Sfx::Wrong);
                    self.shake.add(0.2);
                    self.popups.add(
                        cam.to_screen(p),
                        "Too slow!",
                        Color::from_rgba(255, 110, 100, 255),
                        32.0,
                    );
                }
                Event::Halfway => {
                    sfx::play(Sfx::Score);
                    sfx::say(sfx::Say::Great);
                    self.popups.add(
                        vec2(view.w / 2.0, HUD_H + 50.0),
                        "Halfway there!",
                        GOLD,
                        44.0,
                    );
                }
                Event::Won => {
                    sfx::play(Sfx::Win);
                    sfx::say(sfx::Say::Congrats);
                    feel::impact_tinted(Impact::Large, GOLD);
                    self.particles.confetti(view);
                }
                Event::Lost => {
                    sfx::play(Sfx::Lose);
                    feel::impact(Impact::Medium);
                }
            }
        }
    }

    pub fn cam(&self, view: &View) -> Cam {
        Cam::follow(view, self.focus)
    }

    fn animate(&mut self, dt: f32) {
        self.focus = approach(self.focus, self.sim.player.pos.x, 4.0, dt);
        self.particles.update(dt);
        self.popups.update(dt);
        self.shake.update(dt);
        self.wallet_bump.update(dt);
        for c in &mut self.flying {
            if c.delay > 0.0 {
                c.delay -= dt;
            } else {
                c.t += dt / 0.6;
            }
        }
        self.flying.retain(|c| c.t < 1.0);
        for b in &mut self.built_pop {
            b.1 += dt;
        }
        self.built_pop.retain(|b| b.1 < 1.0);
        self.shown_wallet = approach(self.shown_wallet, self.sim.wallet as f32, 12.0, dt);
        if (self.shown_wallet - self.sim.wallet as f32).abs() < 0.5 {
            self.shown_wallet = self.sim.wallet as f32;
        }
        if self.sim.player.moving {
            self.steps += dt;
            if self.steps > 0.3 {
                self.steps = 0.0;
                sfx::play(Sfx::Tap);
            }
        }
    }

    pub fn days_button(view: &View) -> Rect {
        Rect::new(view.w - 112.0, 12.0, 100.0, 50.0)
    }

    pub fn hint_text(&self) -> Option<&'static str> {
        if self.mode != Mode::Playing || self.sim.day.number > 2 {
            return None;
        }
        if self.sim.day.number == 1 && self.sim.served == 0 && self.sim.t < 2.0 {
            return Some("Drag to walk");
        }
        Some(match self.sim.next_task()? {
            Task::Desk => "Stand at the desk",
            Task::Clean(_) => "Clean the room",
            Task::Towels => "Grab towels",
            Task::Cash => "Pick up the money",
            Task::Build(_) => "Stand on the spot to build",
        })
    }
}

pub fn coins_for(earned: u32) -> usize {
    (earned as usize / 5).clamp(2, 8)
}

pub fn coin_pos(from: Vec2, to: Vec2, k: f32) -> Vec2 {
    let k = k.clamp(0.0, 1.0);
    let e = k * k * (3.0 - 2.0 * k);
    from + (to - from) * e - vec2(0.0, (k * std::f32::consts::PI).sin() * 60.0)
}

fn wallet_pos(view: &View) -> Vec2 {
    let _ = view;
    vec2(HUD_LEFT + 40.0, 37.0)
}

pub fn build_pos(b: Build) -> Vec2 {
    match b {
        Build::Room(i) => bed_pos(i),
        Build::Clerk => CLERK_SPOT,
        Build::Maid => MAID_SPOT,
        Build::Sofa => SOFA_SPOT,
        Build::Palm => PALM_SPOT,
    }
}

pub fn walk_sway(walk: f32) -> f32 {
    if walk <= 0.0 {
        0.0
    } else {
        (walk * 14.0).sin() * 0.09
    }
}

fn walk_hop(walk: f32) -> f32 {
    if walk <= 0.0 {
        0.0
    } else {
        (walk * 14.0).sin().abs() * 6.0
    }
}

fn person(cam: &Cam, sprite: &str, pos: Vec2, h: f32, walk: f32, face: f32, tint: Color) {
    let feet = cam.to_screen(pos);
    let s = cam.scale;
    soft_shadow(feet, 26.0 * s * h / 120.0, 7.0 * s, 0.5);
    let hh = h * s;
    let sz = art::size(sprite);
    let w = hh * sz.x / sz.y.max(1.0);
    let lift = walk_hop(walk) * s;
    art::sprite_ex(
        sprite,
        Rect::new(feet.x - w / 2.0, feet.y - hh - lift, w, hh),
        crate::art::Draw {
            rotation: walk_sway(walk),
            flip_x: face < 0.0,
            tint,
            pivot: vec2(0.5, 1.0),
            ..Default::default()
        },
    );
}

fn guest_sprite(look: u8, happy: bool) -> String {
    let l = look as usize % LOOKS;
    if happy {
        format!("guest{l}_cheer")
    } else {
        format!("guest{l}_idle")
    }
}

fn sprite_world(cam: &Cam, name: &str, center: Vec2, h: f32, d: crate::art::Draw) {
    art::sprite_at(name, cam.to_screen(center), h * cam.scale, d);
}

fn bar(r: Rect, k: f32, col: Color) {
    let rad = r.h / 2.0;
    round_rect(
        Rect::new(r.x - 2.0, r.y - 2.0, r.w + 4.0, r.h + 4.0),
        rad + 2.0,
        Color::from_rgba(70, 40, 40, 255),
    );
    round_rect(r, rad, Color::from_rgba(120, 90, 90, 255));
    if k > 0.02 {
        let w = (r.w * k.min(1.0)).max(r.h);
        round_rect(Rect::new(r.x, r.y, w, r.h), rad, col);
        round_rect(
            Rect::new(
                r.x + rad * 0.4,
                r.y + r.h * 0.12,
                (w - rad * 0.8).max(1.0),
                r.h * 0.3,
            ),
            rad * 0.4,
            Color::new(1.0, 1.0, 1.0, 0.45),
        );
    }
}

fn ring(c: Vec2, r: f32, k: f32, col: Color) {
    let n = 40;
    let end = (n as f32 * k.clamp(0.0, 1.0)) as usize;
    for i in 0..n {
        let a0 = i as f32 / n as f32 * std::f32::consts::TAU - std::f32::consts::FRAC_PI_2;
        let a1 = (i + 1) as f32 / n as f32 * std::f32::consts::TAU - std::f32::consts::FRAC_PI_2;
        let color = if i < end {
            col
        } else {
            Color::new(1.0, 1.0, 1.0, 0.35)
        };
        draw_line(
            c.x + a0.cos() * r,
            c.y + a0.sin() * r,
            c.x + a1.cos() * r,
            c.y + a1.sin() * r,
            r * 0.22,
            color,
        );
    }
}

fn tile(name: &str, r: Rect, tile_size: f32, tint: Color) {
    let nx = (r.w / tile_size).ceil() as usize;
    let ny = (r.h / tile_size).ceil() as usize;
    for y in 0..ny {
        for x in 0..nx {
            let cell = Rect::new(
                r.x + x as f32 * tile_size,
                r.y + y as f32 * tile_size,
                tile_size + 0.5,
                tile_size + 0.5,
            );
            art::sprite_ex(
                name,
                cell,
                crate::art::Draw {
                    tint,
                    flip_x: x % 2 == 1,
                    flip_y: y % 2 == 1,
                    ..Default::default()
                },
            );
        }
    }
}

impl Hotel {
    fn draw_world(&self, view: &View) {
        let cam = self.cam(view);
        let s = cam.scale;
        let world = cam.rect(Rect::new(0.0, 0.0, WORLD_W, WORLD_H));
        let f = view.full();
        gradient_rect(
            f,
            Color::from_rgba(120, 70, 60, 255),
            Color::from_rgba(80, 45, 45, 255),
        );
        round_rect(
            Rect::new(
                world.x - 10.0,
                world.y - 10.0,
                world.w + 20.0,
                world.h + 20.0,
            ),
            18.0,
            Color::from_rgba(60, 30, 30, 255),
        );
        tile("parquet", world, 200.0 * s, WHITE);
        let carpet = cam.rect(Rect::new(
            EXIT_X - 40.0,
            CORRIDOR_Y - 30.0,
            WORLD_W - EXIT_X + 30.0,
            60.0,
        ));
        round_rect(carpet, 10.0 * s, Color::from_rgba(190, 50, 60, 255));
        round_rect(
            Rect::new(
                carpet.x + 4.0 * s,
                carpet.y + 4.0 * s,
                carpet.w - 8.0 * s,
                carpet.h - 8.0 * s,
            ),
            8.0 * s,
            Color::from_rgba(220, 70, 80, 255),
        );
        let lobby_rug = cam.rect(Rect::new(110.0, 370.0, 180.0, 380.0));
        art::sprite_ex("carpet", lobby_rug, crate::art::Draw::default());
        for i in 0..ROOMS {
            self.draw_room(&cam, i);
        }
        self.draw_spots(&cam);
        self.draw_props(&cam);
        self.draw_people(&cam);
        self.draw_arrow(&cam);
        self.draw_bars(&cam);
    }

    fn draw_room(&self, cam: &Cam, i: usize) {
        let r = cam.rect(room_rect(i));
        let s = cam.scale;
        let room = self.sim.rooms[i];
        let pop = self
            .built_pop
            .iter()
            .find(|b| b.0 == Build::Room(i))
            .map(|b| 1.0 + 0.12 * (1.0 - ease_out_cubic(b.1)))
            .unwrap_or(1.0);
        if room == Room::Locked {
            round_rect(r, 14.0 * s, Color::new(0.2, 0.1, 0.1, 0.25));
            let n = 8;
            for k in 0..n {
                let x = r.x + r.w * (k as f32 + 0.5) / n as f32;
                draw_line(
                    x,
                    r.y + 6.0,
                    x,
                    r.y + r.h - 6.0,
                    2.0,
                    Color::new(1.0, 1.0, 1.0, 0.12),
                );
            }
            return;
        }
        let r = Rect::new(
            r.center().x - r.w * pop / 2.0,
            r.center().y - r.h * pop / 2.0,
            r.w * pop,
            r.h * pop,
        );
        round_rect(
            Rect::new(r.x, r.y + 5.0 * s, r.w, r.h),
            14.0 * s,
            Color::new(0.0, 0.0, 0.0, 0.3),
        );
        round_rect(r, 14.0 * s, Color::from_rgba(250, 236, 214, 255));
        let inner = Rect::new(r.x + 8.0 * s, r.y + 8.0 * s, r.w - 16.0 * s, r.h - 16.0 * s);
        art::sprite_ex("roomfloor", inner, crate::art::Draw::default());
        let door = cam.to_screen(room_door(i));
        let dw = 60.0 * s;
        let dy = if i < 4 { r.y + r.h - 9.0 * s } else { r.y };
        draw_rectangle(
            door.x - dw / 2.0,
            dy,
            dw,
            9.0 * s,
            Color::from_rgba(200, 150, 100, 255),
        );
        let bed = if i.is_multiple_of(2) { "bed1" } else { "bed2" };
        let dirty = matches!(room, Room::Dirty { .. });
        sprite_world(
            cam,
            bed,
            bed_pos(i) + vec2(0.0, -10.0),
            130.0,
            crate::art::Draw {
                tint: if dirty {
                    Color::from_rgba(215, 200, 185, 255)
                } else {
                    WHITE
                },
                ..Default::default()
            },
        );
        sprite_world(
            cam,
            "lamp",
            bed_pos(i) + vec2(-68.0, -50.0),
            60.0,
            crate::art::Draw::default(),
        );
        if let Room::Dirty { done } = room {
            for (k, off) in [vec2(-45.0, 40.0), vec2(40.0, 25.0), vec2(5.0, 70.0)]
                .iter()
                .enumerate()
            {
                if (k as f32) / 3.0 >= done {
                    let wob = (self.t * 3.0 + k as f32).sin() * 0.08;
                    sprite_world(
                        cam,
                        "dirt",
                        bed_pos(i) + *off,
                        44.0,
                        crate::art::Draw {
                            rotation: wob,
                            ..Default::default()
                        },
                    );
                }
            }
            let c = cam.to_screen(bed_pos(i) + vec2(0.0, -95.0));
            let bob = (self.t * 4.0).sin() * 4.0;
            if done > 0.0 {
                ring(
                    c,
                    22.0 * s.max(0.7),
                    done,
                    Color::from_rgba(100, 200, 255, 255),
                );
            }
            sprite_world(
                cam,
                "broom",
                cam.to_world(c + vec2(0.0, bob)),
                52.0,
                crate::art::Draw::default(),
            );
        }
        if room == Room::Taken {
            let asleep = self
                .sim
                .guests
                .iter()
                .find(|g| matches!(g.stage, Stage::Asleep(r, _) if r == i));
            if let Some(g) = asleep {
                art::sprite_ex(
                    &guest_sprite(g.look, false),
                    {
                        let c = cam.to_screen(bed_pos(i) + vec2(0.0, -48.0));
                        let h = 46.0 * s;
                        let sz = art::size(&guest_sprite(g.look, false));
                        let w = h * sz.x / sz.y.max(1.0);
                        Rect::new(c.x - w / 2.0, c.y - h * 0.5, w, h * 0.5)
                    },
                    crate::art::Draw::default(),
                );
                if let Stage::Asleep(_, left) = g.stage {
                    let c = cam.to_screen(bed_pos(i) + vec2(0.0, -95.0));
                    ring(
                        c,
                        18.0 * s.max(0.7),
                        1.0 - left / self.sim.day.stay,
                        Color::from_rgba(180, 150, 255, 255),
                    );
                }
                for k in 0..3 {
                    let ph = (self.t * 0.8 + k as f32 / 3.0).fract();
                    let c = cam.to_screen(bed_pos(i) + vec2(40.0 + ph * 30.0, -70.0 - ph * 60.0));
                    text_outline(
                        "z",
                        c.x,
                        c.y,
                        (18.0 + 10.0 * ph) * s.max(0.7),
                        with_alpha(WHITE, 1.0 - ph),
                        with_alpha(INK, 1.0 - ph),
                    );
                }
            }
        }
        if room == Room::Clean {
            let c = cam.to_screen(bed_pos(i) + vec2(55.0, -70.0));
            let k = 1.0 + 0.08 * (self.t * 3.0 + i as f32).sin();
            art::sprite_at("heart", c, 26.0 * s * k, crate::art::Draw::default());
        }
    }

    fn draw_spots(&self, cam: &Cam) {
        let s = cam.scale;
        for k in 0..self.sim.spots.len() {
            if !self.sim.active_spot(k) {
                continue;
            }
            let spot = self.sim.spots[k];
            let c = cam.to_screen(spot.pos());
            let under = self.sim.spot_under_player() == Some(k);
            let pulse = 1.0 + 0.05 * (self.t * 4.0).sin() + if under { 0.08 } else { 0.0 };
            let r = SPOT_R * s * pulse;
            let afford = self.sim.wallet >= spot.left();
            let base = if afford {
                Color::from_rgba(90, 210, 90, 255)
            } else {
                Color::from_rgba(240, 240, 240, 255)
            };
            draw_ellipse(
                c.x,
                c.y + 6.0 * s,
                r,
                r * 0.62,
                0.0,
                Color::new(0.0, 0.0, 0.0, 0.25),
            );
            draw_ellipse(c.x, c.y, r, r * 0.62, 0.0, darker(base, 0.7));
            draw_ellipse(
                c.x,
                c.y - 3.0 * s,
                r * 0.92,
                r * 0.56,
                0.0,
                with_alpha(base, 0.85),
            );
            let paid = spot.paid as f32 / spot.price as f32;
            if paid > 0.0 {
                draw_ellipse(
                    c.x,
                    c.y - 3.0 * s,
                    r * 0.92 * paid.sqrt(),
                    r * 0.56 * paid.sqrt(),
                    0.0,
                    Color::from_rgba(255, 210, 60, 230),
                );
            }
            let icon = match spot.build {
                Build::Room(_) => "bed1",
                Build::Clerk => "clerk",
                Build::Maid => "maid",
                Build::Sofa => "sofa",
                Build::Palm => "palm",
            };
            art::sprite_at(
                icon,
                c + vec2(0.0, -r * 0.65),
                r * 0.95,
                crate::art::Draw {
                    tint: Color::new(1.0, 1.0, 1.0, 0.75),
                    ..Default::default()
                },
            );
            let label = format!("${}", spot.left());
            let coin = c + vec2(
                -text_size(&label, 30.0 * s.max(0.8)).width / 2.0 - 14.0 * s,
                4.0 * s,
            );
            crate::ui::sprite_at(
                "p_coin",
                coin,
                26.0 * s.max(0.8),
                crate::art::Draw::default(),
            );
            text_outline(
                &label,
                c.x + 10.0 * s,
                c.y + 4.0 * s,
                30.0 * s.max(0.8),
                WHITE,
                INK,
            );
        }
    }

    fn draw_props(&self, cam: &Cam) {
        let s = cam.scale;
        sprite_world(
            cam,
            "door",
            DOOR + vec2(0.0, -40.0),
            120.0,
            crate::art::Draw::default(),
        );
        if self.sim.day.towels {
            sprite_world(
                cam,
                "towels",
                CART + vec2(0.0, -30.0),
                85.0,
                crate::art::Draw::default(),
            );
            let c = cam.to_screen(CART + vec2(0.0, 30.0));
            text_outline(
                &format!("{}/{}", self.sim.towels, TOWEL_MAX),
                c.x,
                c.y,
                24.0 * s.max(0.8),
                WHITE,
                INK,
            );
        }
        for (b, at, h) in [
            (Build::Sofa, SOFA_SPOT, 90.0),
            (Build::Palm, PALM_SPOT, 150.0),
        ] {
            if self.sim.spots.iter().any(|p| p.build == b && p.done) {
                let pop = self
                    .built_pop
                    .iter()
                    .find(|x| x.0 == b)
                    .map(|x| ease_out_back(x.1.min(1.0)))
                    .unwrap_or(1.0);
                let name = if b == Build::Sofa { "sofa" } else { "palm" };
                sprite_world(
                    cam,
                    name,
                    at + vec2(0.0, -h / 2.0 + 20.0),
                    h * pop.max(0.05),
                    crate::art::Draw::default(),
                );
            }
        }
        if self.sim.pile > 0 {
            let k = (self.sim.pile as f32 / 40.0).clamp(0.5, 1.4);
            let bob = (self.t * 3.0).sin() * 3.0;
            sprite_world(
                cam,
                "cash",
                CASH + vec2(0.0, bob),
                50.0 * k,
                crate::art::Draw::default(),
            );
            let c = cam.to_screen(CASH + vec2(0.0, -45.0));
            text_outline(
                &format!("${}", self.sim.pile),
                c.x,
                c.y,
                28.0 * s.max(0.8),
                GOLD,
                INK,
            );
        }
    }

    fn draw_people(&self, cam: &Cam) {
        enum Who<'a> {
            Guest(&'a Guest),
            Player,
            Clerk,
            Maid(&'a Maid),
        }
        let mut list: Vec<(f32, Who)> = vec![];
        for g in &self.sim.guests {
            if matches!(g.stage, Stage::Asleep(..)) {
                continue;
            }
            list.push((g.pos.y, Who::Guest(g)));
        }
        list.push((self.sim.player.pos.y, Who::Player));
        if self.sim.clerk {
            list.push((CLERK_SPOT.y, Who::Clerk));
        }
        if let Some(m) = &self.sim.maid {
            list.push((m.pos.y, Who::Maid(m)));
        }
        list.sort_by(|a, b| a.0.total_cmp(&b.0));
        let desk_drawn_at = DESK.y;
        let mut desk_done = false;
        for (y, who) in &list {
            if !desk_done && *y > desk_drawn_at {
                self.draw_desk(cam);
                desk_done = true;
            }
            match who {
                Who::Guest(g) => {
                    let happy = matches!(g.stage, Stage::Leaving { happy: true });
                    let angry = matches!(g.stage, Stage::Leaving { happy: false });
                    let tint = if angry {
                        Color::from_rgba(255, 170, 170, 255)
                    } else {
                        WHITE
                    };
                    person(
                        cam,
                        &guest_sprite(g.look, happy),
                        g.pos,
                        115.0,
                        g.walk,
                        g.face,
                        tint,
                    );
                    if g.stage == Stage::Waiting && g.max.is_finite() {
                        let k = g.patience / g.max;
                        let c = cam.to_screen(g.pos + vec2(0.0, -130.0));
                        let col = if k > 0.5 {
                            Color::from_rgba(90, 210, 70, 255)
                        } else if k > 0.25 {
                            Color::from_rgba(255, 200, 40, 255)
                        } else {
                            Color::from_rgba(240, 70, 60, 255)
                        };
                        ring(c, 13.0 * cam.scale.max(0.7), k, col);
                    }
                }
                Who::Player => {
                    let p = &self.sim.player;
                    person(cam, "bellhop", p.pos, 125.0, p.walk, p.face, WHITE);
                    if self.sim.day.towels && self.sim.towels > 0 {
                        sprite_world(
                            cam,
                            "towels",
                            p.pos + vec2(0.0, -140.0),
                            34.0,
                            crate::art::Draw::default(),
                        );
                    }
                }
                Who::Clerk => person(cam, "clerk", CLERK_SPOT, 120.0, 0.0, 1.0, WHITE),
                Who::Maid(m) => person(cam, "maid", m.pos, 115.0, m.walk, m.face, WHITE),
            }
        }
        if !desk_done {
            self.draw_desk(cam);
        }
    }

    fn draw_desk(&self, cam: &Cam) {
        sprite_world(
            cam,
            "desk",
            DESK + vec2(0.0, -25.0),
            90.0,
            crate::art::Draw::default(),
        );
    }

    fn draw_bars(&self, cam: &Cam) {
        let s = cam.scale;
        if self.sim.serving() && self.sim.desk > 0.0 {
            let c = cam.to_screen(SERVE + vec2(0.0, -40.0));
            let r = Rect::new(c.x - 45.0 * s, c.y - 8.0 * s, 90.0 * s, 16.0 * s);
            bar(r, self.sim.desk, Color::from_rgba(90, 210, 70, 255));
        }
        let waiting = self.sim.front_ready().is_some();
        if waiting && self.sim.free_room().is_none() {
            let c = cam.to_screen(queue_slot(0) + vec2(70.0, -110.0));
            text_outline(
                "No room!",
                c.x,
                c.y,
                26.0 * s.max(0.8),
                Color::from_rgba(255, 150, 130, 255),
                INK,
            );
        }
    }

    fn draw_arrow(&self, cam: &Cam) {
        if self.mode != Mode::Playing || self.sim.day.number > 3 {
            return;
        }
        let Some(task) = self.sim.next_task() else {
            return;
        };
        let target = self.sim.task_pos(task);
        if target.distance(self.sim.player.pos) < REACH * 0.8 {
            return;
        }
        let bob = (self.t * 6.0).sin().abs() * 14.0;
        let lift = match task {
            Task::Build(_) => 110.0,
            Task::Clean(_) => 170.0,
            _ => 120.0,
        };
        sprite_world(
            cam,
            "arrow",
            target + vec2(0.0, -lift - bob),
            60.0,
            crate::art::Draw::default(),
        );
    }

    fn draw_hud(&self, view: &View) {
        let f = view.full();
        let top = Rect::new(f.x - 8.0, f.y - 8.0, f.w + 16.0, HUD_H - f.y + 8.0);
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
        let w = wallet_pos(view);
        let k = self.wallet_bump.scale(0.3);
        crate::ui::sprite_at("p_coin", w, 44.0 * k, crate::art::Draw::default());
        let money = format!("{}", self.shown_wallet.round() as u32);
        score_text(
            &money,
            w.x + 30.0 + text_size(&money, 40.0).width / 2.0,
            w.y + 3.0,
            40.0,
            self.wallet_bump.t,
        );
        text_outline(
            &format!("Day {}", self.sim.day.number),
            w.x + 200.0,
            37.0,
            34.0,
            WHITE,
            INK,
        );
        let gb = Rect::new(
            w.x + 280.0,
            23.0,
            (view.w - 290.0 - (w.x + 280.0)).max(120.0),
            28.0,
        );
        bar(gb, self.sim.progress(), Color::from_rgba(90, 210, 70, 255));
        text_outline(
            &format!(
                "${} / ${}",
                self.sim.earned.min(self.sim.day.target),
                self.sim.day.target
            ),
            gb.center().x,
            gb.center().y,
            22.0,
            WHITE,
            INK,
        );
        let stars = match self.mode {
            Mode::Won { stars } => stars,
            _ => self.sim.stars(),
        };
        for s in 0..3u8 {
            crate::ui::sprite_at(
                "p_star",
                vec2(view.w - 270.0 + s as f32 * 32.0, 37.0),
                32.0,
                crate::art::Draw {
                    tint: if s < stars {
                        WHITE
                    } else {
                        Color::from_rgba(120, 60, 70, 150)
                    },
                    ..Default::default()
                },
            );
        }
        if let Some(c) = self.sim.clock {
            let low = c < 15.0 && self.mode == Mode::Playing;
            let k = if low {
                1.0 + 0.08 * (self.t * 8.0).sin()
            } else {
                1.0
            };
            text_outline(
                &clock(c),
                view.w - 160.0,
                37.0,
                32.0 * k,
                if low {
                    Color::from_rgba(255, 190, 170, 255)
                } else {
                    WHITE
                },
                INK,
            );
        }
        crate::ui::painted_button(Self::days_button(view), "Days", "blue", 0.0, 1.0);
    }

    fn draw_stick(&self) {
        let Some(o) = self.stick.origin else { return };
        if self.stick.knob.distance(o) < 4.0 {
            return;
        }
        draw_circle(o.x, o.y, STICK_R, Color::new(1.0, 1.0, 1.0, 0.18));
        draw_circle(o.x, o.y, STICK_R - 6.0, Color::new(0.0, 0.0, 0.0, 0.12));
        let k = self.stick.knob;
        draw_circle(k.x, k.y + 3.0, 32.0, Color::new(0.0, 0.0, 0.0, 0.25));
        draw_circle(k.x, k.y, 32.0, Color::new(1.0, 1.0, 1.0, 0.85));
    }

    fn draw_guidance(&self, view: &View) {
        if self.mode != Mode::Playing {
            return;
        }
        let at = vec2(view.w / 2.0, view.h - 50.0);
        let pulse = 1.0 + 0.04 * (self.t * 5.0).sin();
        if self.age < INTRO {
            crate::ui::hint(view, &self.sim.day.goal_line(), at, 1.0);
        } else if let Some(h) = self.hint_text() {
            crate::ui::hint(view, h, at, pulse);
        }
    }

    fn draw_end(&self, view: &View) {
        let k = self.end_age;
        match self.mode {
            Mode::Won { stars } => {
                let title = format!("Day {} done!", self.sim.day.number);
                let action = if self.sim.day.number >= MAX_DAY {
                    "All days"
                } else {
                    "Next day"
                };
                let r = crate::ui::Results {
                    game: GAME,
                    title: &title,
                    line: "",
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
                    crate::ui::sprite_at(
                        "p_star",
                        vec2(c.x + (s as f32 - 1.0) * 54.0, y),
                        48.0 * pop.max(0.01),
                        crate::art::Draw {
                            tint: if s < stars {
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
                let s = format!("Earned ${} of ${}", self.sim.earned, self.sim.day.target);
                overlay(view, "Time's up!", &[&s, "Try again"]);
            }
            _ => {}
        }
    }

    fn finish_screen(&mut self) {
        let n = self.sim.day.number;
        match self.mode {
            Mode::Won { .. } if n >= MAX_DAY => self.mode = Mode::Select,
            Mode::Won { .. } => self.begin(n + 1),
            _ => self.begin(n),
        }
    }
}

impl Game for Hotel {
    fn update(&mut self, input: &Input, view: &View, dt: f32) {
        self.age += dt;
        self.t += dt;
        if self.mode == Mode::Select {
            self.select_input(input, view);
            return;
        }
        if input.pressed && Self::days_button(view).contains(input.pos) {
            sfx::play(Sfx::Click);
            self.mode = Mode::Select;
            self.age = 0.0;
            return;
        }
        match self.mode {
            Mode::Playing => {
                self.control(input, view);
                self.tick(dt);
            }
            Mode::Won { .. } | Mode::Lost => {
                self.end_age += dt;
                if self.end_age >= END_GUARD && (input.pressed || input.action) {
                    self.finish_screen();
                    return;
                }
            }
            Mode::Select => {}
        }
        self.react(view);
        self.animate(dt);
    }

    fn draw(&self, view: &View) {
        if self.mode == Mode::Select {
            self.draw_select(view);
            return;
        }
        clear_background(Color::from_rgba(80, 45, 45, 255));
        let sh = self.shake.offset();
        set_view_camera(view, sh);
        self.draw_world(view);
        set_view_camera(view, Vec2::ZERO);
        self.draw_hud(view);
        for c in &self.flying {
            if c.delay > 0.0 {
                continue;
            }
            let p = coin_pos(c.from, c.to, c.t);
            let r = 13.0 * (1.0 - 0.3 * c.t);
            let w = r * 2.0 * (c.t * 12.0).cos().abs().max(0.2);
            crate::ui::sprite_ex(
                "p_coin",
                Rect::new(p.x - w / 2.0, p.y - r, w, r * 2.0),
                crate::art::Draw::default(),
            );
        }
        if self.mode == Mode::Playing {
            self.draw_stick();
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
            vec2(view.w - 168.0, 90.0),
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
        crate::ui::painted_button(play, &format!("Play day {next}"), "green", 0.0, 1.0);
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
            "parquet",
            "roomfloor",
            "carpet",
            "bed1",
            "bed2",
            "lamp",
            "dirt",
            "heart",
            "broom",
            "bellhop",
            "maid",
            "clerk",
            "desk",
            "door",
            "cash",
            "sofa",
            "palm",
            "towels",
            "arrow",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();
        for look in 0..LOOKS as u8 {
            want.push(guest_sprite(look, false));
            want.push(guest_sprite(look, true));
        }
        for n in &want {
            assert!(have.iter().any(|h| h == n), "hotel sheet lacks {n}");
        }
    }

    fn fresh() {
        crate::save::reset_for_test();
        crate::profile::create("Mia", 0);
    }

    fn view() -> View {
        View::fit(1280.0, 720.0)
    }

    #[test]
    fn the_whole_hotel_fits_under_the_hud() {
        for (w, h) in [
            (1280.0, 720.0),
            (2400.0, 1080.0),
            (1024.0, 768.0),
            (720.0, 1280.0),
        ] {
            let v = View::fit(w, h);
            let cam = Cam::fit(&v);
            let tl = cam.to_screen(Vec2::ZERO);
            let br = cam.to_screen(vec2(WORLD_W, WORLD_H));
            assert!(tl.y >= HUD_H - 0.5 && br.y <= v.h + 0.5, "{w}x{h}");
            assert!(tl.x >= -0.5 && br.x <= v.w + 0.5, "{w}x{h}");
            let p = vec2(321.0, 456.0);
            assert!(cam.to_world(cam.to_screen(p)).distance(p) < 0.01);
        }
    }

    #[test]
    fn a_tall_phone_zooms_in_and_follows_the_bellhop() {
        let v = View::fit(1080.0, 2400.0);
        let fit = Cam::fit(&v);
        let near = Cam::follow(&v, 0.0);
        let far = Cam::follow(&v, WORLD_W);
        assert!(near.scale > fit.scale * 1.5);
        assert!(near.to_screen(Vec2::ZERO).x.abs() < 0.5);
        assert!((far.to_screen(vec2(WORLD_W, 0.0)).x - v.w).abs() < 0.5);
        let mid = Cam::follow(&v, 600.0);
        assert!((mid.to_screen(vec2(600.0, 0.0)).x - v.w / 2.0).abs() < 0.5);
        let wide = View::fit(1280.0, 720.0);
        assert_eq!(Cam::follow(&wide, 600.0), Cam::fit(&wide));
    }

    #[test]
    fn dragging_steers_the_bellhop() {
        fresh();
        let mut h = Hotel::blank(Day::for_level(1, false));
        h.begin(1);
        let v = view();
        let start = h.sim.player.pos;
        let o = vec2(600.0, 500.0);
        h.update(&Input::press_at(o), &v, 0.016);
        for _ in 0..30 {
            let i = Input {
                pos: o + vec2(80.0, 0.0),
                down: true,
                ..Default::default()
            };
            h.update(&i, &v, 0.033);
        }
        assert!(h.sim.player.pos.x > start.x + 150.0);
        let i = Input {
            pos: o + vec2(80.0, 0.0),
            released: true,
            ..Default::default()
        };
        h.update(&i, &v, 0.033);
        let after = h.sim.player.pos;
        h.update(&Input::default(), &v, 0.1);
        assert!(h.sim.player.pos.distance(after) < 1.0);
    }

    #[test]
    fn a_tap_walks_the_bellhop_to_that_spot() {
        fresh();
        let mut h = Hotel::blank(Day::for_level(1, false));
        h.begin(1);
        let v = view();
        let cam = h.cam(&v);
        let goal = CASH;
        let at = cam.to_screen(goal);
        h.update(&Input::press_at(at), &v, 0.016);
        let i = Input {
            pos: at,
            released: true,
            ..Default::default()
        };
        h.update(&i, &v, 0.016);
        for _ in 0..120 {
            h.update(&Input::default(), &v, 0.033);
        }
        assert!(h.sim.player.pos.distance(goal) < 2.0);
    }

    #[test]
    fn winning_a_day_awards_stars_unlocks_the_next_and_submits_a_score() {
        fresh();
        let mut h = Hotel::blank(Day::for_level(1, false));
        h.begin(1);
        h.sim.earned = h.sim.day.target;
        h.tick(0.1);
        let Mode::Won { stars } = h.mode else {
            panic!("not won")
        };
        assert_eq!(stars, 3);
        assert_eq!(crate::profile::stars(GAME, 1), 3);
        assert_eq!(crate::profile::unlocked(GAME), 2);
        assert_eq!(h.rank, Some(0));
        assert_eq!(crate::profile::board(GAME)[0].score, h.score);
    }

    #[test]
    fn losing_twice_eases_the_day() {
        fresh();
        for _ in 0..EASE_AFTER {
            let mut h = Hotel::blank(Day::for_level(5, false));
            h.begin(5);
            h.sim.clock = Some(0.01);
            h.tick(0.1);
            assert_eq!(h.mode, Mode::Lost);
        }
        let mut h = Hotel::blank(Day::for_level(5, false));
        h.begin(5);
        assert!(h.easing);
        assert!(h.sim.day.target < Day::for_level(5, false).target);
    }

    #[test]
    fn progress_resumes_per_player_and_new_players_start_at_day_one() {
        fresh();
        for n in 1..=4 {
            let mut h = Hotel::blank(Day::for_level(n, false));
            h.begin(n);
            h.sim.earned = h.sim.day.target;
            h.tick(0.1);
        }
        let back = Hotel::new();
        assert_eq!(back.mode, Mode::Select);
        assert_eq!(continue_day(), 5);
        crate::profile::create("Leo", 1);
        let kid = Hotel::new();
        assert_eq!(kid.mode, Mode::Playing);
        assert_eq!(kid.sim.day.number, 1);
    }

    #[test]
    fn first_day_hints_lead_through_the_loop() {
        fresh();
        let mut h = Hotel::blank(Day::for_level(1, false));
        h.begin(1);
        assert_eq!(h.hint_text(), Some("Drag to walk"));
        h.sim.t = 5.0;
        h.sim.player.pos = vec2(1000.0, 700.0);
        h.sim.pile = 10;
        assert_eq!(h.hint_text(), Some("Pick up the money"));
        let mut late = Hotel::blank(Day::for_level(6, false));
        late.begin(6);
        assert_eq!(late.hint_text(), None);
    }
}
