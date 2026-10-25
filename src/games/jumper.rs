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
pub const KEY: &str = "jumper";
pub const MILESTONE: u32 = 100;
pub const STAGES: [(u32, &str); 3] = [
    (200, "Moving platforms!"),
    (500, "Crumbly platforms!"),
    (800, "Sky high!"),
];

pub fn gap_for(score: u32, assist: u32) -> f32 {
    ((80.0 + score as f32 * 0.12).min(185.0) - assist as f32 * 8.0).max(68.0)
}

pub fn reach_for(score: u32) -> f32 {
    220.0 + score as f32 * 1.5
}

pub fn moving_chance(score: u32) -> f32 {
    if score < STAGES[0].0 {
        0.0
    } else {
        ((score - STAGES[0].0) as f32 / 500.0).min(1.0) * 0.25
    }
}

pub fn crumble_chance(score: u32) -> f32 {
    if score < STAGES[1].0 {
        0.0
    } else {
        ((score - STAGES[1].0) as f32 / 500.0).min(1.0) * 0.18
    }
}

pub fn moving_speed(score: u32) -> f32 {
    (90.0 + score as f32 * 0.1).min(190.0)
}

pub fn stage_for(score: u32) -> usize {
    STAGES.iter().filter(|s| s.0 <= score).count()
}

pub fn next_goal(score: u32) -> u32 {
    (score / MILESTONE + 1) * MILESTONE
}

pub fn goal_progress(score: u32) -> f32 {
    (score % MILESTONE) as f32 / MILESTONE as f32
}

pub fn rescues_for(runs: u32, assist: u32) -> u32 {
    if runs < 2 {
        2
    } else if assist >= 2 {
        1
    } else {
        0
    }
}

pub fn assist_after(assist: u32, score: u32) -> u32 {
    if score < MILESTONE {
        (assist + 1).min(3)
    } else if score >= MILESTONE * 2 {
        assist.saturating_sub(1)
    } else {
        assist
    }
}

pub const RESCUE_LIMIT: u32 = 600;

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
    stage: usize,
    prev_cam: f32,
    t: f32,
    assist: u32,
    runs: u32,
    pub rescues: u32,
    last_x: f32,
    pub rank: Option<usize>,
    steered: bool,
    run_t: f32,
    fallen_t: f32,
    confetti: Particles,
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
            stage: 0,
            prev_cam: 0.0,
            t: 0.0,
            assist: 0,
            runs: 0,
            rescues: 0,
            last_x: 0.0,
            rank: None,
            steered: false,
            run_t: 0.0,
            fallen_t: 0.0,
            confetti: Particles::default(),
        };
        j.best = Best::load(KEY);
        j.assist = crate::profile::get(KEY, "assist").max(0) as u32;
        j.runs = crate::profile::get(KEY, "runs").max(0) as u32;
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
        self.stage = 0;
        self.last_x = view.w / 2.0 - PLAT_W / 2.0;
        self.rescues = rescues_for(self.runs, self.assist);
        self.rank = None;
        self.steered = false;
        self.run_t = 0.0;
        self.fallen_t = 0.0;
        self.confetti.list.clear();
        self.prev_cam = 0.0;
        self.landings.clear();
        self.boings.clear();
        self.crumbs.clear();
        self.particles.list.clear();
        self.fill(view);
    }

    fn gap(&self) -> f32 {
        gap_for(self.score, self.assist)
    }

    fn fill(&mut self, view: &View) {
        while self.top > self.cam - 200.0 {
            let y = self.top - rand::gen_range(self.gap() * 0.7, self.gap());
            let span = view.w - PLAT_W - 20.0;
            let reach = reach_for(self.score);
            let x = if reach >= span {
                rand::gen_range(10.0, view.w - PLAT_W - 10.0)
            } else {
                rand::gen_range(
                    (self.last_x - reach).max(10.0),
                    (self.last_x + reach).min(10.0 + span),
                )
            };
            let roll = rand::gen_range(0.0, 1.0);
            let moving = moving_chance(self.score);
            let crumble = crumble_chance(self.score);
            let kind = if roll < 0.07 {
                Kind::Spring
            } else if roll < 0.07 + moving {
                let v = moving_speed(self.score);
                Kind::Moving(if rand::gen_range(0, 2) == 0 { v } else { -v })
            } else if roll < 0.07 + moving + crumble {
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
                self.last_x = sx;
            } else {
                self.last_x = x;
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
        let steps = self.score / MILESTONE;
        if steps > self.milestone {
            self.milestone = steps;
            self.score_bump.kick();
            self.particles.sparkle(
                vec2(HUD_LEFT + 50.0, 50.0),
                Color::from_rgba(255, 220, 80, 255),
            );
            let stage = stage_for(self.score);
            if stage > self.stage {
                self.stage = stage;
                sfx::play(Sfx::LevelUp);
                sfx::say(sfx::Say::LevelUp);
                feel::impact_tinted(Impact::Large, Color::from_rgba(255, 220, 80, 255));
                self.confetti.confetti(view);
                self.popups.add(
                    vec2(view.w / 2.0, 190.0),
                    STAGES[stage - 1].1,
                    Color::from_rgba(255, 255, 255, 255),
                    50.0,
                );
            } else {
                sfx::play(Sfx::Score);
                if steps.is_multiple_of(5) {
                    sfx::say(sfx::Say::Great);
                }
            }
            self.popups.add(
                vec2(view.w / 2.0, 120.0),
                format!("{}!", steps * MILESTONE),
                Color::from_rgba(255, 220, 80, 255),
                54.0,
            );
        }
    }

    fn rescue(&mut self, view: &View) {
        self.rescues -= 1;
        let x = (self.pos.x - PLAT_W / 2.0).clamp(0.0, view.w - PLAT_W);
        let y = self.cam + view.h - 90.0;
        self.pos = vec2(x + PLAT_W / 2.0, y - HERO_R - 2.0);
        self.vel = vec2(0.0, SPRING);
        self.squish = 1.0;
        sfx::play(Sfx::Bounce);
        sfx::say(sfx::Say::Go);
        self.popups.add(
            vec2(self.pos.x, y - self.cam - 90.0),
            "Whoa! Try again!",
            Color::from_rgba(255, 255, 255, 255),
            44.0,
        );
        self.particles.sparkle(
            vec2(self.pos.x, y - self.cam),
            Color::from_rgba(120, 220, 255, 255),
        );
    }

    fn finish(&mut self) {
        self.state = State::Fallen;
        self.fallen_t = 0.0;
        let before = self.best.0;
        self.rank = self.best.submit(KEY, self.score);
        if self.rank == Some(0) && before > 0 {
            sfx::say(sfx::Say::Best);
        }
        self.runs += 1;
        self.assist = assist_after(self.assist, self.score);
        crate::profile::set(KEY, "runs", self.runs as i64);
        crate::profile::set(KEY, "assist", self.assist as i64);
        let furthest = crate::profile::get(KEY, "milestone").max(0) as u32;
        crate::profile::set(KEY, "milestone", furthest.max(self.milestone) as i64);
        sfx::play(Sfx::Whoosh);
        sfx::play(Sfx::Lose);
        feel::impact(Impact::Medium);
        self.shake.add(0.6);
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
        let climb = (-self.cam / 15000.0).clamp(0.0, 1.0);
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
                self.run_t += dt;
                if steer != 0.0 && self.run_t > 0.5 {
                    self.steered = true;
                }
                self.physics(steer, view, dt);
                self.fill(view);
                if self.pos.y - HERO_R > self.cam + view.h {
                    if self.rescues > 0 && self.score < RESCUE_LIMIT {
                        self.rescue(view);
                    } else {
                        self.finish();
                    }
                }
            }
            State::Fallen => {
                self.fallen_t += dt;
                if self.fallen_t > 0.7 && input.tap() {
                    self.reset(view);
                    self.state = State::Playing;
                    sfx::say(sfx::Say::Go);
                }
            }
        }
        self.react(view);
        self.animate(dt);
        self.confetti.update(dt);
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
        self.draw_hud(view);
        self.confetti.draw();
        let best = self.best.0.to_string();
        match self.state {
            State::Ready => {
                let goal = format!("Climb to {} for a cheer", MILESTONE);
                let mut lines = vec!["Touch left or right to steer".to_string(), goal];
                if self.best.0 > 0 {
                    lines[1] = format!("Best {}   Climb higher!", best);
                }
                lines.push("Tap to start".to_string());
                let refs: Vec<&str> = lines.iter().map(|l| l.as_str()).collect();
                overlay(view, "Sky Jumper", &refs);
            }
            State::Fallen => {
                let line = format!("Height {}   Best {}", self.score, best);
                let title = if self.rank == Some(0) && self.score > 0 {
                    "Great jump!"
                } else {
                    "Whoops!"
                };
                results(
                    view,
                    &crate::ui::Results {
                        game: KEY,
                        title,
                        line: &line,
                        rank: self.rank,
                        action: "Jump again",
                        format: crate::ui::plain_score,
                    },
                );
            }
            State::Playing => {
                if !self.steered && self.runs < 3 {
                    let pulse = 1.0 + (self.t * 4.0).sin() * 0.04;
                    crate::ui::hint(
                        view,
                        "Touch left or right to steer",
                        vec2(view.w / 2.0, 150.0),
                        pulse,
                    );
                }
            }
        }
    }
}

impl Jumper {
    fn draw_hud(&self, view: &View) {
        chip(
            view,
            Side::Left,
            "HEIGHT",
            &self.score.to_string(),
            self.score_bump.t,
        );
        chip(view, Side::Right, "BEST", &self.best.0.to_string(), 1.0);
        if self.state == State::Fallen {
            return;
        }
        let w = 240.0;
        let r = Rect::new(view.w / 2.0 - w / 2.0, CHIP_Y + 8.0, w, 30.0);
        round_rect(r, r.h / 2.0, Color::new(0.1, 0.1, 0.3, 0.45));
        let fill = goal_progress(self.score).max(0.04);
        let inner = Rect::new(r.x + 3.0, r.y + 3.0, (r.w - 6.0) * fill, r.h - 6.0);
        round_rect(inner, inner.h / 2.0, Color::from_rgba(255, 210, 60, 255));
        text_center(
            &next_goal(self.score).to_string(),
            r.x + r.w / 2.0,
            r.y + r.h / 2.0 + 1.0,
            22.0,
            WHITE,
        );
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

    fn fresh_player() {
        crate::save::reset_for_test();
        crate::profile::create("Mia", 0);
    }

    fn autopilot(j: &mut Jumper, v: &View, seconds: f32, tap_only: bool) {
        let dt = 1.0 / 60.0;
        let mut t = 0.0;
        while t < seconds && j.state == State::Playing {
            let falling = j.vel.y > 0.0;
            let target = j
                .plats
                .iter()
                .filter(|p| !p.broken && p.kind != Kind::Crumble)
                .filter(|p| {
                    if falling {
                        p.top() > j.pos.y + HERO_R && p.top() < j.pos.y + 200.0
                    } else {
                        p.top() < j.pos.y - 10.0
                    }
                })
                .min_by(|a, b| {
                    (a.top() - j.pos.y)
                        .abs()
                        .partial_cmp(&(b.top() - j.pos.y).abs())
                        .unwrap()
                })
                .map(|p| p.x + PLAT_W / 2.0);
            let steer = match target {
                Some(x) if !tap_only && (x - j.pos.x).abs() > 20.0 => (x - j.pos.x).signum(),
                _ => 0.0,
            };
            let input = Input {
                down: steer != 0.0,
                pos: vec2(if steer < 0.0 { 10.0 } else { v.w - 10.0 }, 100.0),
                ..Default::default()
            };
            j.update(&input, v, dt);
            t += dt;
        }
    }

    fn start(j: &mut Jumper, v: &View) {
        j.update(&Input::press_at(vec2(100.0, 100.0)), v, 1.0 / 60.0);
        assert_eq!(j.state, State::Playing);
    }

    #[test]
    fn first_stretch_has_only_plain_platforms_and_small_gaps() {
        fresh_player();
        let v = View::ipad_landscape();
        let mut j = Jumper::new();
        j.cam = -(MILESTONE as f32) * 10.0 * 1.9;
        j.fill(&v);
        assert!(j
            .plats
            .iter()
            .all(|p| matches!(p.kind, Kind::Solid | Kind::Spring)));
        assert!(gap_for(0, 0) <= 80.0);
        assert_eq!(moving_chance(0), 0.0);
        assert_eq!(crumble_chance(0), 0.0);
    }

    #[test]
    fn early_platforms_stay_within_a_short_sideways_hop() {
        fresh_player();
        let v = View::ipad_landscape();
        let mut j = Jumper::new();
        j.cam = -3000.0;
        j.fill(&v);
        let mut ps: Vec<&Plat> = j.plats.iter().collect();
        ps.sort_by(|a, b| b.y.partial_cmp(&a.y).unwrap());
        for w in ps.windows(2).skip(1) {
            assert!((w[0].x - w[1].x).abs() <= reach_for(0) + 1.0);
        }
    }

    #[test]
    fn a_child_who_only_taps_survives_the_first_ten_seconds() {
        fresh_player();
        let v = View::ipad_landscape();
        let mut j = Jumper::new();
        start(&mut j, &v);
        autopilot(&mut j, &v, 10.0, true);
        assert_eq!(j.state, State::Playing);
    }

    #[test]
    fn a_first_fall_is_caught_and_the_second_ends_the_run() {
        fresh_player();
        let v = View::ipad_landscape();
        let mut j = Jumper::new();
        start(&mut j, &v);
        assert_eq!(j.rescues, 2);
        for expected in [1, 0] {
            j.plats.clear();
            j.pos.y = j.cam + v.h + 200.0;
            j.update(&Input::default(), &v, 1.0 / 60.0);
            assert_eq!(j.state, State::Playing);
            assert_eq!(j.rescues, expected);
            assert!(j.pos.y < j.cam + v.h);
        }
        j.plats.clear();
        j.pos.y = j.cam + v.h + 200.0;
        j.update(&Input::default(), &v, 1.0 / 60.0);
        assert_eq!(j.state, State::Fallen);
    }

    #[test]
    fn difficulty_rises_with_height() {
        assert!(gap_for(0, 0) < gap_for(400, 0));
        assert!(gap_for(400, 0) < gap_for(2000, 0));
        assert!(reach_for(0) < reach_for(300));
        assert_eq!(moving_chance(STAGES[0].0), 0.0);
        assert!(moving_chance(600) > 0.0);
        assert!(crumble_chance(STAGES[1].0 - 1) == 0.0 && crumble_chance(900) > 0.0);
        assert!(moving_speed(0) < moving_speed(900));
        assert_eq!(stage_for(0), 0);
        assert_eq!(stage_for(STAGES[0].0), 1);
        assert_eq!(stage_for(10_000), STAGES.len());
    }

    #[test]
    fn new_platform_kinds_appear_after_their_milestone() {
        fresh_player();
        let v = View::ipad_landscape();
        let mut j = Jumper::new();
        j.score = 1500;
        j.cam = -15000.0;
        j.top = j.cam + 100.0;
        for _ in 0..40 {
            j.cam -= 400.0;
            j.fill(&v);
        }
        assert!(j.plats.iter().any(|p| matches!(p.kind, Kind::Moving(_))) || j.plats.len() > 5);
        let mut kinds = std::collections::HashSet::new();
        for _ in 0..60 {
            j.top = j.cam + 100.0;
            j.plats.clear();
            j.cam -= 300.0;
            j.fill(&v);
            for p in &j.plats {
                kinds.insert(std::mem::discriminant(&p.kind));
            }
        }
        assert!(kinds.len() >= 3);
    }

    #[test]
    fn gap_never_exceeds_a_jump_even_when_assisted_or_high() {
        let max_jump = JUMP * JUMP / (2.0 * GRAVITY);
        for score in [0, 100, 1000, 100_000] {
            for assist in 0..=3 {
                assert!(gap_for(score, assist) < max_jump);
            }
        }
    }

    #[test]
    fn crossing_a_milestone_cheers_and_shows_the_next_goal() {
        fresh_player();
        let v = View::ipad_landscape();
        let mut j = Jumper::new();
        start(&mut j, &v);
        j.score = MILESTONE;
        j.react(&v);
        assert_eq!(j.milestone, 1);
        assert!(!j.popups.list.is_empty());
        assert_eq!(next_goal(MILESTONE), 2 * MILESTONE);
        assert_eq!(goal_progress(MILESTONE + 50), 0.5);
    }

    #[test]
    fn reaching_a_new_stage_celebrates_with_confetti() {
        fresh_player();
        let v = View::ipad_landscape();
        let mut j = Jumper::new();
        start(&mut j, &v);
        j.score = STAGES[0].0;
        j.react(&v);
        assert_eq!(j.stage, 1);
        assert!(!j.confetti.list.is_empty());
    }

    #[test]
    fn the_run_score_goes_on_the_board_and_saves_the_best_per_player() {
        fresh_player();
        let v = View::ipad_landscape();
        let mut j = Jumper::new();
        start(&mut j, &v);
        j.rescues = 0;
        j.score = 340;
        j.milestone = 3;
        j.plats.clear();
        j.pos.y = j.cam + v.h + 200.0;
        j.update(&Input::default(), &v, 1.0 / 60.0);
        assert_eq!(j.state, State::Fallen);
        assert_eq!(j.rank, Some(0));
        assert_eq!(crate::profile::personal_best(KEY), Some(340));
        assert_eq!(crate::profile::get(KEY, "milestone"), 3);
        assert_eq!(Jumper::new().best.0, 340);
        crate::profile::create("Leo", 1);
        assert_eq!(Jumper::new().best.0, 0);
    }

    #[test]
    fn a_short_run_makes_the_next_one_gentler_and_a_long_one_eases_back() {
        assert_eq!(assist_after(0, 40), 1);
        assert_eq!(assist_after(3, 40), 3);
        assert_eq!(assist_after(2, 150), 2);
        assert_eq!(assist_after(2, 400), 1);
        assert_eq!(assist_after(0, 400), 0);
        assert!(gap_for(0, 3) < gap_for(0, 0));
        assert_eq!(rescues_for(5, 0), 0);
        assert_eq!(rescues_for(5, 2), 1);
        assert_eq!(rescues_for(0, 0), 2);
    }

    #[test]
    fn runs_and_assist_resume_per_player() {
        fresh_player();
        let v = View::ipad_landscape();
        let mut j = Jumper::new();
        start(&mut j, &v);
        j.rescues = 0;
        j.score = 30;
        j.plats.clear();
        j.pos.y = j.cam + v.h + 200.0;
        j.update(&Input::default(), &v, 1.0 / 60.0);
        let again = Jumper::new();
        assert_eq!(again.runs, 1);
        assert_eq!(again.assist, 1);
        crate::profile::create("Leo", 1);
        assert_eq!(Jumper::new().runs, 0);
    }

    #[test]
    fn game_over_ignores_a_tap_until_the_panel_has_settled() {
        fresh_player();
        let v = View::ipad_landscape();
        let mut j = Jumper::new();
        start(&mut j, &v);
        j.rescues = 0;
        j.plats.clear();
        j.pos.y = j.cam + v.h + 200.0;
        j.update(&Input::default(), &v, 1.0 / 60.0);
        assert_eq!(j.state, State::Fallen);
        j.update(&Input::press_at(vec2(100.0, 100.0)), &v, 1.0 / 60.0);
        assert_eq!(j.state, State::Fallen);
        for _ in 0..60 {
            j.update(&Input::default(), &v, 1.0 / 60.0);
        }
        j.update(&Input::press_at(vec2(100.0, 100.0)), &v, 1.0 / 60.0);
        assert_eq!(j.state, State::Playing);
        assert_eq!(j.score, 0);
    }

    #[test]
    fn a_steering_player_climbs_past_the_first_milestone() {
        fresh_player();
        crate::profile::set(KEY, "runs", 5);
        let v = View::ipad_landscape();
        let mut j = Jumper::new();
        start(&mut j, &v);
        autopilot(&mut j, &v, 40.0, false);
        assert!(j.score >= MILESTONE, "score {}", j.score);
    }
}
