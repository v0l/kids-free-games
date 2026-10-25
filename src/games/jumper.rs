use crate::engine::*;
use crate::feel::{self, Impact};
use crate::fx::*;
use crate::sfx::{self, Sfx};
use macroquad::prelude::*;

pub const GRAVITY: f32 = 1300.0;
pub const JUMP: f32 = -820.0;
pub const SPRING: f32 = -1350.0;
pub const PLAT_W: f32 = 120.0;
pub const PLAT_H: f32 = 22.0;
pub const HERO_R: f32 = 26.0;
pub const MOVE: f32 = 520.0;
pub const SPRING_H: f32 = 30.0;
const HERO_H: f32 = 104.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Kind {
    Solid,
    Moving(f32),
    Crumble,
    Spring,
}

#[derive(Clone, Copy, Debug)]
pub struct Plat {
    pub x: f32,
    pub y: f32,
    pub kind: Kind,
    pub broken: bool,
}

impl Plat {
    pub fn top(&self) -> f32 {
        if self.kind == Kind::Spring {
            self.y - SPRING_H
        } else {
            self.y
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum State {
    Ready,
    Playing,
    Fallen,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Landing {
    pub pos: Vec2,
    pub kind: Kind,
}

#[derive(Clone, Copy, Debug)]
pub struct Crumb {
    pub pos: Vec2,
    pub vel: Vec2,
    pub angle: f32,
    pub spin: f32,
    pub size: Vec2,
    pub age: f32,
}

pub const CRUMB_LIFE: f32 = 1.2;

pub fn hero_height(frame: &str) -> f32 {
    match frame {
        "bunny_jump" => 0.95,
        "bunny_ready" => 0.82,
        "bunny_hurt" => 0.78,
        _ => 1.0,
    }
}

pub fn squash_stretch(squish: f32, vy: f32) -> (f32, f32) {
    let stretch = (vy.abs() / 1400.0).min(1.0) * 0.2;
    let s = squish.clamp(0.0, 1.0) * 0.3 - stretch;
    (1.0 + s, 1.0 - s)
}

pub fn parallax_y(base: f32, cam: f32, factor: f32, span: f32) -> f32 {
    (base - cam * factor).rem_euclid(span) - span * 0.15
}

pub fn spring_extension(age: f32) -> f32 {
    if age >= 0.7 {
        1.0
    } else {
        0.35 + 0.65 * ease_out_elastic(age / 0.7)
    }
}

pub struct Jumper {
    pub pos: Vec2,
    pub vel: Vec2,
    pub plats: Vec<Plat>,
    pub top: f32,
    pub cam: f32,
    pub state: State,
    pub score: u32,
    pub best: Best,
    width: f32,
    pub landings: Vec<Landing>,
    face: f32,
    squish: f32,
    boings: Vec<(Vec2, f32)>,
    crumbs: Vec<Crumb>,
    particles: Particles,
    popups: Popups,
    shake: Shake,
    score_bump: Bump,
    milestone: u32,
    prev_cam: f32,
    t: f32,
}

impl Jumper {
    pub fn new() -> Self {
        let mut j = Self {
            pos: vec2(480.0, 600.0),
            vel: Vec2::ZERO,
            plats: vec![],
            top: 0.0,
            cam: 0.0,
            state: State::Ready,
            score: 0,
            best: Best::default(),
            width: MIN_W,
            landings: vec![],
            face: 1.0,
            squish: 0.0,
            boings: vec![],
            crumbs: vec![],
            particles: Particles::default(),
            popups: Popups::default(),
            shake: Shake::default(),
            score_bump: Bump::default(),
            milestone: 0,
            prev_cam: 0.0,
            t: 0.0,
        };
        j.reset(&View::fit(MIN_W, MIN_H));
        j
    }

    fn reset(&mut self, view: &View) {
        self.width = view.w;
        self.cam = 0.0;
        self.pos = vec2(view.w / 2.0, view.h - 140.0);
        self.vel = Vec2::ZERO;
        self.plats = vec![Plat {
            x: view.w / 2.0 - PLAT_W / 2.0,
            y: view.h - 100.0,
            kind: Kind::Solid,
            broken: false,
        }];
        self.top = view.h - 100.0;
        self.score = 0;
        self.milestone = 0;
        self.prev_cam = 0.0;
        self.landings.clear();
        self.boings.clear();
        self.crumbs.clear();
        self.particles.list.clear();
        self.fill(view);
    }

    fn gap(&self) -> f32 {
        (90.0 + self.score as f32 * 0.02).min(190.0)
    }

    fn fill(&mut self, view: &View) {
        while self.top > self.cam - 200.0 {
            let y = self.top - rand::gen_range(self.gap() * 0.7, self.gap());
            let x = rand::gen_range(10.0, view.w - PLAT_W - 10.0);
            let hard = (self.score as f32 / 5000.0).min(1.0);
            let roll = rand::gen_range(0.0, 1.0);
            let kind = if roll < 0.06 {
                Kind::Spring
            } else if roll < 0.06 + 0.25 * hard {
                Kind::Moving(if rand::gen_range(0, 2) == 0 {
                    120.0
                } else {
                    -120.0
                })
            } else if roll < 0.06 + 0.4 * hard {
                Kind::Crumble
            } else {
                Kind::Solid
            };
            self.plats.push(Plat {
                x,
                y,
                kind,
                broken: false,
            });
            if kind == Kind::Crumble {
                let sx = (x + view.w / 2.0).rem_euclid(view.w - PLAT_W);
                self.plats.push(Plat {
                    x: sx,
                    y: y - 20.0,
                    kind: Kind::Solid,
                    broken: false,
                });
            }
            self.top = y;
        }
        let bottom = self.cam + view.h + 50.0;
        self.plats.retain(|p| p.y < bottom);
    }

    pub fn physics(&mut self, steer: f32, view: &View, dt: f32) {
        self.vel.x = steer * MOVE;
        if steer != 0.0 {
            self.face = steer.signum();
        }
        self.vel.y += GRAVITY * dt;
        let old_feet = self.pos.y + HERO_R;
        self.pos += self.vel * dt;
        self.pos.x = self.pos.x.rem_euclid(view.w);
        self.squish = (self.squish - dt * 4.0).max(0.0);
        for p in &mut self.plats {
            if let Kind::Moving(v) = p.kind {
                p.x += v * dt;
                if p.x < 0.0 || p.x > view.w - PLAT_W {
                    p.kind = Kind::Moving(-v);
                    p.x = p.x.clamp(0.0, view.w - PLAT_W);
                }
            }
        }
        if self.vel.y > 0.0 {
            let feet = self.pos.y + HERO_R;
            for p in &mut self.plats {
                if p.broken {
                    continue;
                }
                let over =
                    self.pos.x + HERO_R * 0.6 > p.x && self.pos.x - HERO_R * 0.6 < p.x + PLAT_W;
                let top = p.top();
                if over && old_feet <= top + 2.0 && feet >= top {
                    if p.kind == Kind::Crumble {
                        p.broken = true;
                        self.landings.push(Landing {
                            pos: vec2(p.x + PLAT_W / 2.0, top),
                            kind: Kind::Crumble,
                        });
                        continue;
                    }
                    self.landings.push(Landing {
                        pos: vec2(self.pos.x, top),
                        kind: p.kind,
                    });
                    self.pos.y = top - HERO_R;
                    self.vel.y = if p.kind == Kind::Spring { SPRING } else { JUMP };
                    self.squish = 1.0;
                    break;
                }
            }
        }
        for p in &mut self.plats {
            if p.broken {
                p.y += 600.0 * dt;
            }
        }
        let line = self.cam + view.h * 0.4;
        if self.pos.y < line {
            self.cam -= line - self.pos.y;
        }
        self.score = self.score.max((-self.cam / 10.0) as u32);
    }
}

impl Default for Jumper {
    fn default() -> Self {
        Self::new()
    }
}

mod art {
    crate::sprites!("jumper");
}

impl Jumper {
    fn react(&mut self, view: &View) {
        for l in std::mem::take(&mut self.landings) {
            let at = vec2(l.pos.x, l.pos.y - self.cam);
            match l.kind {
                Kind::Crumble => {
                    sfx::play(Sfx::Crunch);
                    self.crumble(l.pos)
                }
                Kind::Spring => {
                    sfx::play(Sfx::LevelUp);
                    self.boings.push((l.pos, 0.0));
                    self.shake.add(0.2);
                    self.popups.add(
                        at - vec2(0.0, 60.0),
                        "Boing!",
                        Color::from_rgba(255, 230, 90, 255),
                        38.0,
                    );
                    self.particles
                        .sparkle(at - vec2(0.0, 24.0), Color::from_rgba(255, 90, 90, 255));
                    self.dust(at);
                }
                _ => {
                    sfx::play(Sfx::Jump);
                    self.dust(at)
                }
            }
        }
        let steps = self.score / 100;
        if steps > self.milestone {
            self.milestone = steps;
            sfx::play(Sfx::Score);
            self.score_bump.kick();
            self.popups.add(
                vec2(view.w / 2.0, 120.0),
                format!("{}!", steps * 100),
                Color::from_rgba(255, 220, 80, 255),
                54.0,
            );
            self.particles.sparkle(
                vec2(HUD_LEFT + 50.0, 50.0),
                Color::from_rgba(255, 220, 80, 255),
            );
        }
    }

    fn dust(&mut self, at: Vec2) {
        for dir in [-1.0f32, 1.0] {
            self.particles.emit(
                at + vec2(dir * 14.0, -2.0),
                &[Color::from_rgba(235, 235, 245, 255)],
                Burst {
                    count: 5,
                    speed: (60.0, 170.0),
                    size: (5.0, 10.0),
                    life: (0.25, 0.5),
                    gravity: -20.0,
                    drag: 4.0,
                    spread: 0.7,
                    angle: if dir < 0.0 { std::f32::consts::PI } else { 0.0 },
                    ..Default::default()
                },
            );
        }
    }

    fn crumble(&mut self, world: Vec2) {
        for k in 0..5 {
            let f = k as f32 / 4.0 - 0.5;
            self.crumbs.push(Crumb {
                pos: vec2(world.x + f * PLAT_W * 0.8, world.y + PLAT_H / 2.0),
                vel: vec2(
                    f * 160.0 + rand::gen_range(-40.0, 40.0),
                    rand::gen_range(-260.0, -80.0),
                ),
                angle: 0.0,
                spin: rand::gen_range(-6.0, 6.0),
                size: vec2(PLAT_W / 5.0 + 2.0, PLAT_H * rand::gen_range(0.7, 1.1)),
                age: 0.0,
            });
        }
        let at = vec2(world.x, world.y - self.cam);
        self.particles.emit(
            at,
            &[
                Color::from_rgba(170, 120, 70, 255),
                Color::from_rgba(120, 85, 50, 255),
            ],
            Burst {
                count: 10,
                speed: (60.0, 220.0),
                size: (3.0, 6.0),
                life: (0.4, 0.8),
                shape: Shape::Square,
                ..Default::default()
            },
        );
    }

    fn animate(&mut self, dt: f32) {
        for b in &mut self.boings {
            b.1 += dt;
        }
        self.boings.retain(|b| b.1 < 0.8);
        for c in &mut self.crumbs {
            c.vel.y += 1400.0 * dt;
            c.pos += c.vel * dt;
            c.angle += c.spin * dt;
            c.age += dt;
        }
        self.crumbs.retain(|c| c.age < CRUMB_LIFE);
        let shift = self.prev_cam - self.cam;
        for p in &mut self.particles.list {
            p.pos.y += shift;
        }
        self.prev_cam = self.cam;
        self.particles.update(dt);
        self.popups.update(dt);
        self.shake.update(dt);
        self.score_bump.update(dt);
    }

    fn boing_age(&self, p: &Plat) -> Option<f32> {
        self.boings
            .iter()
            .find(|(w, _)| {
                (w.y - p.top()).abs() < 1.0 && (w.x - (p.x + PLAT_W / 2.0)).abs() < PLAT_W
            })
            .map(|b| b.1)
    }

    fn background(&self, view: &View) {
        let f = view.full();
        let climb = (-self.cam / 24000.0).clamp(0.0, 1.0);
        let night = 1.0 - climb * 0.6;
        let dusk = Color::new(night, night * (1.0 - climb * 0.1), 1.0 - climb * 0.3, 1.0);
        let sky_s = art::size("sky");
        let sky_h = f.h.max(f.w * sky_s.y / sky_s.x);
        let sky_w = sky_h * sky_s.x / sky_s.y;
        art::sprite_ex(
            "sky",
            Rect::new(f.x + (f.w - sky_w) / 2.0, f.y + f.h - sky_h, sky_w, sky_h),
            crate::art::Draw {
                tint: dusk,
                ..Default::default()
            },
        );
        for (name, factor, height) in [("hills_far", 0.08, 0.5), ("hills", 0.2, 0.42)] {
            let h = view.h * height;
            let s = art::size(name);
            let w = h * s.x / s.y;
            let bottom = view.h + view.inset.bottom - self.cam * factor;
            if bottom - h > f.y + f.h {
                continue;
            }
            let shade = if name == "hills" { 1.0 } else { 0.95 };
            let tint = Color::new(night * shade, night * shade, night.max(0.55) * shade, 1.0);
            for x in crate::games::flappy::tile_x(f.x, f.w, w, self.t * 3.0 * factor) {
                art::sprite_ex(
                    name,
                    Rect::new(x, bottom - h, w + 1.0, h),
                    crate::art::Draw {
                        tint,
                        ..Default::default()
                    },
                );
            }
        }
        for k in 0..7 {
            let f = k as f32;
            let span = view.h + 260.0;
            let x = (f * 211.0) % (view.w + 120.0) - 60.0;
            let y = parallax_y(f * 157.0, self.cam, 0.14 + (f % 3.0) * 0.06, span);
            let w = 110.0 + (f * 53.0) % 90.0;
            let name = ["cloud1", "cloud2", "cloud3"][k % 3];
            art::sprite_w(
                name,
                vec2(x + (self.t * (4.0 + f)) % 40.0, y),
                w,
                crate::art::Draw {
                    tint: Color::new(1.0, 1.0, 1.0, 0.8 - climb * 0.4),
                    ..Default::default()
                },
            );
        }
    }

    fn draw_plat(&self, p: &Plat, y: f32) {
        let name = match p.kind {
            Kind::Solid | Kind::Spring => "grass",
            Kind::Moving(_) => "stone",
            Kind::Crumble => "wood_cracked",
        };
        let (w, lift) = match name {
            "grass" => (PLAT_W + 36.0, 0.17),
            "stone" => (PLAT_W + 30.0, 0.1),
            _ => (PLAT_W + 24.0, 0.12),
        };
        let h = w * art::size(name).y / art::size(name).x;
        art::sprite(
            name,
            Rect::new(p.x + PLAT_W / 2.0 - w / 2.0, y - h * lift, w, h),
        );
        if p.kind == Kind::Spring {
            let ext = self.boing_age(p).map_or(1.0, spring_extension);
            let frame = if ext < 0.7 {
                "spring_in"
            } else if ext > 1.04 {
                "spring_out"
            } else {
                "spring"
            };
            let sh = 64.0;
            let sw = sh * art::size(frame).x / art::size(frame).y;
            art::sprite(
                frame,
                Rect::new(p.x + PLAT_W / 2.0 - sw / 2.0, y + 4.0 - sh, sw, sh),
            );
        }
    }

    fn hero_frame(&self) -> &'static str {
        if self.state == State::Fallen {
            "bunny_hurt"
        } else if self.squish > 0.6 {
            "bunny_ready"
        } else if self.vel.y < 0.0 {
            "bunny_jump"
        } else {
            "bunny_stand"
        }
    }

    fn draw_hero(&self, view: &View, c: Vec2) {
        let (sx, sy) = squash_stretch(self.squish, self.vel.y);
        let frame = self.hero_frame();
        let fh = HERO_H * hero_height(frame);
        let w = fh * art::size(frame).x / art::size(frame).y * sx;
        let h = fh * sy;
        let anchor = c.y + HERO_R + 2.0;
        for dx in [0.0, -view.w, view.w] {
            let cx = c.x + dx;
            if cx < -w || cx > view.w + w {
                continue;
            }
            art::sprite_ex(
                frame,
                Rect::new(cx - w / 2.0, anchor - h, w, h),
                crate::art::Draw {
                    flip_x: self.face < 0.0,
                    ..Default::default()
                },
            );
        }
    }
}

impl Game for Jumper {
    fn update(&mut self, input: &Input, view: &View, dt: f32) {
        self.t += dt;
        if (view.w - self.width).abs() > 1.0 && self.state == State::Ready {
            self.reset(view);
        }
        let steer = if input.left {
            -1.0
        } else if input.right {
            1.0
        } else if input.down {
            if input.pos.x < view.w / 2.0 {
                -1.0
            } else {
                1.0
            }
        } else {
            0.0
        };
        match self.state {
            State::Ready => {
                if input.tap() {
                    self.state = State::Playing;
                    sfx::say(sfx::Say::Go);
                }
            }
            State::Playing => {
                self.physics(steer, view, dt);
                self.fill(view);
                if self.pos.y - HERO_R > self.cam + view.h {
                    self.state = State::Fallen;
                    if self.score > self.best.0 && self.best.0 > 0 {
                        sfx::say(sfx::Say::Best);
                    }
                    self.best.offer(self.score);
                    sfx::play(Sfx::Whoosh);
                    sfx::play(Sfx::Lose);
                    feel::impact(Impact::Medium);
                    self.shake.add(0.6);
                }
            }
            State::Fallen => {
                if input.tap() {
                    self.reset(view);
                    self.state = State::Playing;
                    sfx::say(sfx::Say::Go);
                }
            }
        }
        self.react(view);
        self.animate(dt);
    }

    fn draw(&self, view: &View) {
        self.background(view);
        let sh = self.shake.offset();
        set_view_camera(view, vec2(sh.x, sh.y));
        for p in &self.plats {
            if !p.broken {
                self.draw_plat(p, p.y - self.cam);
            }
        }
        for c in &self.crumbs {
            let fade = 1.0 - (c.age / CRUMB_LIFE).powi(2);
            let at = vec2(c.pos.x, c.pos.y - self.cam);
            art::sprite_ex(
                "crumb",
                Rect::new(
                    at.x - c.size.x * 0.7,
                    at.y - c.size.y * 0.7,
                    c.size.x * 1.4,
                    c.size.y * 1.4,
                ),
                crate::art::Draw {
                    rotation: c.angle,
                    tint: Color::new(1.0, 1.0, 1.0, fade),
                    ..Default::default()
                },
            );
        }
        let c = vec2(self.pos.x, self.pos.y - self.cam);
        if self.state != State::Fallen {
            let ground = self
                .plats
                .iter()
                .filter(|p| {
                    !p.broken
                        && p.top() - self.cam > c.y
                        && p.x - 4.0 < c.x
                        && c.x < p.x + PLAT_W + 4.0
                })
                .map(|p| p.top() - self.cam)
                .fold(f32::MAX, f32::min);
            if ground < f32::MAX {
                let near = 1.0 - ((ground - c.y) / 260.0).clamp(0.0, 1.0);
                soft_shadow(
                    vec2(c.x, ground + 2.0),
                    HERO_R * (0.5 + 0.5 * near),
                    5.0,
                    0.5 * near,
                );
            }
        }
        self.draw_hero(view, c);
        self.particles.draw();
        self.popups.draw();
        set_view_camera(view, Vec2::ZERO);
        chip(
            view,
            Side::Left,
            "HEIGHT",
            &self.score.to_string(),
            self.score_bump.t,
        );
        chip(view, Side::Right, "BEST", &self.best.0.to_string(), 1.0);
        match self.state {
            State::Ready => overlay(
                view,
                "Sky Jumper",
                &[
                    "Touch left or right side to steer",
                    "Jump as high as you can",
                    "Tap to start",
                ],
            ),
            State::Fallen => {
                let s = format!("Height {}   Best {}", self.score, self.best.0);
                overlay(view, "Whoops!", &[&s, "Tap to jump again"]);
            }
            State::Playing => {}
        }
    }
}

pub fn icon(r: Rect) {
    crate::ui::icon("jumper", r);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn on_platform(kind: Kind) -> (Jumper, View) {
        let v = View::ipad_landscape();
        let mut j = Jumper::new();
        j.plats = vec![Plat {
            x: 400.0,
            y: 500.0,
            kind,
            broken: false,
        }];
        j.pos = vec2(460.0, j.plats[0].top() - HERO_R - 5.0);
        j.vel = vec2(0.0, 400.0);
        (j, v)
    }

    #[test]
    fn jumper_sprites_all_exist() {
        let have = art::names();
        for n in [
            "sky",
            "hills_far",
            "hills",
            "cloud1",
            "cloud2",
            "cloud3",
            "grass",
            "stone",
            "wood_cracked",
            "spring",
            "spring_in",
            "spring_out",
            "bunny_stand",
            "bunny_jump",
            "bunny_ready",
            "bunny_hurt",
            "crumb",
        ] {
            assert!(have.iter().any(|h| h == n), "missing {n}");
        }
    }

    #[test]
    fn landing_on_a_spring_stops_at_its_top_not_the_platform() {
        let v = View::ipad_landscape();
        let mut j = Jumper::new();
        j.plats = vec![Plat {
            x: 400.0,
            y: 500.0,
            kind: Kind::Spring,
            broken: false,
        }];
        j.pos = vec2(460.0, 500.0 - SPRING_H - HERO_R - 4.0);
        j.vel = vec2(0.0, 400.0);
        j.physics(0.0, &v, 1.0 / 30.0);
        assert_eq!(j.vel.y, SPRING);
        assert_eq!(j.pos.y, 500.0 - SPRING_H - HERO_R);
        assert_eq!(j.landings[0].pos.y, 500.0 - SPRING_H);
    }

    #[test]
    fn hero_frame_follows_motion() {
        let mut j = Jumper::new();
        j.vel.y = -300.0;
        assert_eq!(j.hero_frame(), "bunny_jump");
        j.vel.y = 300.0;
        assert_eq!(j.hero_frame(), "bunny_stand");
        j.state = State::Fallen;
        assert_eq!(j.hero_frame(), "bunny_hurt");
    }

    #[test]
    fn landing_on_a_platform_bounces() {
        let (mut j, v) = on_platform(Kind::Solid);
        j.physics(0.0, &v, 1.0 / 30.0);
        assert_eq!(j.vel.y, JUMP);
    }

    #[test]
    fn spring_launches_higher() {
        let (mut j, v) = on_platform(Kind::Spring);
        j.physics(0.0, &v, 1.0 / 30.0);
        assert_eq!(j.vel.y, SPRING);
    }

    #[test]
    fn crumbly_platform_breaks_and_drops_you() {
        let (mut j, v) = on_platform(Kind::Crumble);
        j.physics(0.0, &v, 1.0 / 30.0);
        assert!(j.vel.y > 0.0);
        assert!(j.plats[0].broken);
    }

    #[test]
    fn jumping_up_through_a_platform_passes() {
        let (mut j, v) = on_platform(Kind::Solid);
        j.pos.y = 500.0 + 10.0;
        j.vel.y = -600.0;
        j.physics(0.0, &v, 1.0 / 30.0);
        assert!(j.vel.y < 0.0 && j.vel.y > JUMP);
    }

    #[test]
    fn landing_is_reported_with_its_platform_kind() {
        let (mut j, v) = on_platform(Kind::Spring);
        j.physics(0.0, &v, 1.0 / 30.0);
        assert_eq!(j.landings.len(), 1);
        assert_eq!(j.landings[0].kind, Kind::Spring);
        assert_eq!(j.landings[0].pos.y, 500.0 - SPRING_H);
    }

    #[test]
    fn crumbling_drops_pieces_and_dust() {
        let (mut j, v) = on_platform(Kind::Crumble);
        j.state = State::Playing;
        j.physics(0.0, &v, 1.0 / 30.0);
        assert_eq!(j.landings[0].kind, Kind::Crumble);
        j.react(&v);
        assert_eq!(j.crumbs.len(), 5);
        assert!(!j.particles.list.is_empty());
        for _ in 0..80 {
            j.animate(1.0 / 60.0);
        }
        assert!(j.crumbs.is_empty());
    }

    #[test]
    fn spring_landing_boings_with_a_popup() {
        let (mut j, v) = on_platform(Kind::Spring);
        j.physics(0.0, &v, 1.0 / 30.0);
        j.react(&v);
        assert_eq!(j.boings.len(), 1);
        assert_eq!(j.popups.list.len(), 1);
        assert!(j.shake.active());
        assert!(j.boing_age(&j.plats[0]).is_some());
    }

    #[test]
    fn solid_landing_puffs_dust_each_side() {
        let (mut j, v) = on_platform(Kind::Solid);
        j.physics(0.0, &v, 1.0 / 30.0);
        j.react(&v);
        assert_eq!(j.particles.list.len(), 10);
        assert!(j.boings.is_empty());
    }

    #[test]
    fn squash_on_landing_and_stretch_in_flight() {
        let (sx, sy) = squash_stretch(1.0, 0.0);
        assert!(sx > 1.0 && sy < 1.0);
        let (sx, sy) = squash_stretch(0.0, JUMP);
        assert!(sx < 1.0 && sy > 1.0);
        assert_eq!(squash_stretch(0.0, 0.0), (1.0, 1.0));
    }

    #[test]
    fn spring_compresses_then_overshoots() {
        assert!((spring_extension(0.0) - 0.35).abs() < 1e-4);
        assert!((0..70).any(|k| spring_extension(k as f32 / 100.0) > 1.02));
        assert_eq!(spring_extension(2.0), 1.0);
    }

    #[test]
    fn parallax_layers_scroll_slower_and_wrap_on_screen() {
        for cam in [0.0, -123.0, -9000.0] {
            let y = parallax_y(40.0, cam, 0.3, 900.0);
            assert!((-135.0..765.0).contains(&y));
        }
        let near = parallax_y(0.0, -100.0, 0.3, 5000.0) - parallax_y(0.0, 0.0, 0.3, 5000.0);
        let far = parallax_y(0.0, -100.0, 0.1, 5000.0) - parallax_y(0.0, 0.0, 0.1, 5000.0);
        assert!(near > far && far > 0.0);
    }

    #[test]
    fn particles_follow_the_camera() {
        let mut j = Jumper::new();
        j.particles.emit(
            vec2(100.0, 100.0),
            &[WHITE],
            Burst {
                count: 1,
                speed: (0.0, 0.0),
                gravity: 0.0,
                life: (5.0, 5.0),
                ..Default::default()
            },
        );
        j.cam = -50.0;
        j.animate(0.0);
        assert!((j.particles.list[0].pos.y - 150.0).abs() < 0.01);
    }

    #[test]
    fn edges_wrap_around() {
        let (mut j, v) = on_platform(Kind::Solid);
        j.plats.clear();
        j.pos.x = v.w - 2.0;
        j.physics(1.0, &v, 0.05);
        assert!(j.pos.x < 50.0);
    }

    #[test]
    fn world_keeps_generating_reachable_platforms() {
        let v = View::ipad_landscape();
        let mut j = Jumper::new();
        j.cam = -50000.0;
        j.score = 5000;
        j.fill(&v);
        let mut ys: Vec<f32> = j
            .plats
            .iter()
            .filter(|p| p.kind != Kind::Crumble)
            .map(|p| p.y)
            .collect();
        ys.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let max_jump = JUMP * JUMP / (2.0 * GRAVITY);
        assert!(
            ys.windows(2).all(|w| w[1] - w[0] < max_jump),
            "gap too big for a jump of {max_jump}"
        );
    }
}
