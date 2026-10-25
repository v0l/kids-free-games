use crate::engine::*;
use crate::feel::{self, Impact};
use crate::fx::*;
use crate::sfx::{self, Sfx};
use macroquad::prelude::*;
use std::f32::consts::TAU;

pub const SEGMENTS: usize = 12;
pub const FLOOR_GAP: f32 = 170.0;
pub const GRAVITY: f32 = 1700.0;
pub const BOUNCE: f32 = -640.0;
pub const BALL_R: f32 = 18.0;
pub const SMASH_FLOORS: u32 = 3;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Seg {
    Solid,
    Gap,
    Danger,
}

#[derive(Clone, Debug)]
pub struct Floor {
    pub segs: [Seg; SEGMENTS],
    pub broken: f32,
    pub hue: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum State {
    Ready,
    Playing,
    Dead,
    Won,
}

pub struct Helix {
    pub floors: Vec<Floor>,
    pub angle: f32,
    pub y: f32,
    pub vy: f32,
    pub passed: usize,
    pub streak: u32,
    pub score: u32,
    pub best: Best,
    pub level: u32,
    pub state: State,
    cam: f32,
    t: f32,
    squash: f32,
    splats: Vec<(usize, f32, f32)>,
    fx: Particles,
    shake: Shake,
    pops: Popups,
    bump: Bump,
    end_t: f32,
    last_x: Option<f32>,
}

pub fn ball_angle() -> f32 {
    std::f32::consts::FRAC_PI_2
}

pub fn segment_at(angle: f32, rotation: f32) -> usize {
    let local = (angle - rotation).rem_euclid(TAU);
    ((local / TAU * SEGMENTS as f32) as usize).min(SEGMENTS - 1)
}

fn make_floor(index: usize, level: u32) -> Floor {
    let mut segs = [Seg::Solid; SEGMENTS];
    let gap_at = rand::gen_range(0, SEGMENTS);
    let gap_len = rand::gen_range(2, 4);
    for k in 0..gap_len {
        segs[(gap_at + k) % SEGMENTS] = Seg::Gap;
    }
    if index > 0 {
        let dangers = (rand::gen_range(0, 2) + (level as usize + index) / 6).min(4);
        for _ in 0..dangers {
            let k = rand::gen_range(0, SEGMENTS);
            if segs[k] == Seg::Solid {
                segs[k] = Seg::Danger;
            }
        }
    }
    Floor {
        segs,
        broken: 0.0,
        hue: rand::gen_range(0.0, 1.0),
    }
}

impl Default for Helix {
    fn default() -> Self {
        Self::new()
    }
}

impl Helix {
    pub fn new() -> Self {
        Self::with_level(1)
    }

    pub fn with_level(level: u32) -> Self {
        let count = 10 + level as usize * 2;
        let mut floors: Vec<Floor> = (0..count).map(|i| make_floor(i, level)).collect();
        let first_gap = floors[0].segs.iter().position(|s| *s == Seg::Gap).unwrap();
        let rotation = ball_angle() - (first_gap as f32 + 0.5) / SEGMENTS as f32 * TAU + TAU * 0.5;
        floors[0].segs[segment_at(ball_angle(), rotation)] = Seg::Solid;
        Self {
            floors,
            angle: rotation,
            y: -60.0,
            vy: 0.0,
            passed: 0,
            streak: 0,
            score: 0,
            best: Best::default(),
            level,
            state: State::Ready,
            cam: 0.0,
            t: 0.0,
            squash: 0.0,
            splats: vec![],
            fx: Particles::default(),
            shake: Shake::default(),
            pops: Popups::default(),
            bump: Bump::default(),
            end_t: 0.0,
            last_x: None,
        }
    }

    pub fn floor_y(i: usize) -> f32 {
        i as f32 * FLOOR_GAP
    }

    pub fn seg_under_ball(&self, i: usize) -> Seg {
        self.floors[i].segs[segment_at(ball_angle(), self.angle)]
    }

    pub fn physics(&mut self, dt: f32) -> Option<Seg> {
        let old = self.y;
        self.vy += GRAVITY * dt;
        self.y += self.vy * dt;
        if self.vy <= 0.0 {
            return None;
        }
        let i = self.passed;
        if i >= self.floors.len() {
            return None;
        }
        let fy = Self::floor_y(i) - BALL_R;
        if old <= fy && self.y >= fy {
            let seg = self.seg_under_ball(i);
            match seg {
                Seg::Gap => {
                    self.pass_floor();
                }
                _ if self.streak >= SMASH_FLOORS => {
                    sfx::play(Sfx::Boom);
                    feel::impact(Impact::Medium);
                    self.floors[i].broken = 0.001;
                    self.pass_floor();
                    self.streak = 0;
                    self.y = fy;
                    self.vy = BOUNCE * 0.6;
                    self.shake.add(0.5);
                    return Some(Seg::Gap);
                }
                Seg::Danger => {
                    self.y = fy;
                    return Some(Seg::Danger);
                }
                Seg::Solid => {
                    sfx::play(Sfx::Bounce);
                    self.y = fy;
                    self.vy = BOUNCE;
                    self.streak = 0;
                }
            }
            return Some(seg);
        }
        None
    }

    fn pass_floor(&mut self) {
        let i = self.passed;
        if self.floors[i].broken == 0.0 {
            self.floors[i].broken = 0.001;
        }
        self.passed += 1;
        self.streak += 1;
        let gained = self.streak * self.level;
        self.score += gained;
        self.bump.kick();
    }

    fn ring(&self) -> (f32, f32) {
        (150.0, 60.0)
    }
}

impl Game for Helix {
    fn update(&mut self, input: &Input, view: &View, dt: f32) {
        self.t += dt;
        self.fx.update(dt);
        self.shake.update(dt);
        self.pops.update(dt);
        self.bump.update(dt);
        self.squash = (self.squash - dt * 5.0).max(0.0);
        for f in &mut self.floors {
            if f.broken > 0.0 {
                f.broken += dt;
            }
        }
        if input.down {
            if let Some(lx) = self.last_x {
                self.angle += (input.pos.x - lx) * 0.012;
            }
            self.last_x = Some(input.pos.x);
        } else {
            self.last_x = None;
        }
        if input.left {
            self.angle -= 3.5 * dt;
        }
        if input.right {
            self.angle += 3.5 * dt;
        }
        let target = self.y - view.h * 0.35;
        if target > self.cam {
            self.cam = approach(self.cam, target, 10.0, dt);
        }
        match self.state {
            State::Ready => {
                self.y = -60.0 + (self.t * 4.0).sin().abs() * -40.0;
                if input.pressed || input.action {
                    self.state = State::Playing;
                    sfx::say(sfx::Say::Go);
                    self.vy = 0.0;
                }
            }
            State::Playing => {
                let before = self.passed;
                let ball_screen = vec2(view.w / 2.0, self.y - self.cam + view.h * 0.15);
                match self.physics(dt) {
                    Some(Seg::Solid) => {
                        self.squash = 1.0;
                        self.splats.push((
                            self.passed,
                            segment_at(ball_angle(), self.angle) as f32,
                            self.angle,
                        ));
                        self.fx.emit(
                            ball_screen + vec2(0.0, BALL_R),
                            &[Color::from_rgba(255, 120, 60, 255)],
                            Burst {
                                count: 8,
                                speed: (60.0, 180.0),
                                size: (3.0, 6.0),
                                spread: 2.0,
                                angle: -std::f32::consts::FRAC_PI_2,
                                ..Default::default()
                            },
                        );
                    }
                    Some(Seg::Danger) => {
                        sfx::play(Sfx::Hit);
                        sfx::play(Sfx::Lose);
                        if self.score > self.best.0 && self.best.0 > 0 {
                            sfx::say(sfx::Say::Best);
                        }
                        feel::impact(Impact::Large);
                        self.state = State::Dead;
                        self.end_t = 0.0;
                        self.best.offer(self.score);
                        self.shake.add(0.7);
                        self.fx.emit(
                            ball_screen,
                            &[Color::from_rgba(255, 120, 60, 255), WHITE],
                            Burst {
                                count: 30,
                                ..Default::default()
                            },
                        );
                    }
                    _ => {}
                }
                if self.passed > before {
                    sfx::play(Sfx::Swish);
                    let col = hsv(self.floors[before].hue, 0.6, 1.0);
                    self.fx.emit(
                        ball_screen,
                        &[col, darker(col, 0.8)],
                        Burst {
                            count: 18,
                            shape: Shape::Square,
                            speed: (200.0, 420.0),
                            gravity: 900.0,
                            ..Default::default()
                        },
                    );
                    if self.streak >= 2 {
                        self.pops.add(
                            ball_screen + vec2(70.0, -30.0),
                            format!("+{}", self.streak * self.level),
                            Color::from_rgba(255, 220, 60, 255),
                            40.0,
                        );
                    }
                }
                if self.passed >= self.floors.len() {
                    sfx::play(Sfx::Win);
                    sfx::say(sfx::Say::Win);
                    self.state = State::Won;
                    self.end_t = 0.0;
                    self.best.offer(self.score);
                    self.fx.confetti(view);
                }
            }
            State::Dead | State::Won => {
                self.end_t += dt;
                if self.state == State::Won {
                    self.physics(dt);
                    self.y = self.y.min(Self::floor_y(self.floors.len()) + 60.0);
                }
                if self.end_t > 0.7 && input.pressed {
                    let best = self.best;
                    let score = self.score;
                    let level = if self.state == State::Won {
                        self.level + 1
                    } else {
                        1
                    };
                    *self = Self::with_level(level);
                    self.best = best;
                    if level > 1 {
                        self.score = score;
                    }
                }
            }
        }
    }

    fn draw(&self, view: &View) {
        draw_backdrop(view, self.level);
        let sh = self.shake.offset();
        let cx = view.w / 2.0 + sh.x;
        let to_screen = |y: f32| y - self.cam + view.h * 0.15 + sh.y;
        draw_clouds(view, self.cam, self.t);
        let (outer, inner) = self.ring();
        let pole_w = inner * 1.1;
        draw_pole(cx, view, pole_w);
        for (i, f) in self.floors.iter().enumerate() {
            let y = to_screen(Self::floor_y(i));
            if y < -100.0 || y > view.h + 100.0 {
                continue;
            }
            let fall = f.broken * f.broken * 900.0;
            let fade = (1.0 - f.broken * 1.5).max(0.0);
            if f.broken > 0.0 && fade <= 0.0 {
                continue;
            }
            for k in 0..SEGMENTS {
                let seg = f.segs[k];
                if seg == Seg::Gap {
                    continue;
                }
                let a0 = self.angle + k as f32 / SEGMENTS as f32 * TAU;
                let a1 = a0 + TAU / SEGMENTS as f32;
                let mid = (a0 + a1) / 2.0;
                let push = if f.broken > 0.0 {
                    vec2(mid.cos() * fall * 0.6, fall)
                } else {
                    Vec2::ZERO
                };
                let name = if seg == Seg::Danger {
                    "tile_red"
                } else {
                    floor_tile(f.hue)
                };
                draw_ring_segment(&Ring {
                    centre: vec2(cx, y) + push,
                    a0,
                    a1,
                    outer,
                    inner,
                    thick: 18.0,
                    flat: 0.32,
                    tile: name,
                    light: 0.75 + 0.25 * mid.sin().max(-0.2),
                    alpha: fade,
                });
            }
            for (fi, seg, at) in &self.splats {
                if *fi == i && f.broken == 0.0 {
                    let a = self.angle - at + (*seg + 0.5) / SEGMENTS as f32 * TAU;
                    let r = (outer + inner) / 2.0;
                    art::sprite_at(
                        "splat",
                        vec2(cx + a.cos() * r, y + a.sin() * r * 0.32),
                        24.0,
                        crate::art::Draw {
                            tint: Color::from_rgba(255, 255, 255, 235),
                            ..Default::default()
                        },
                    );
                }
            }
        }
        if self.state != State::Dead {
            let by = to_screen(self.y);
            let bx = cx + ball_angle().cos() * (outer + inner) / 2.0;
            let bz = by + ball_angle().sin() * (outer + inner) / 2.0 * 0.32;
            let sq = self.squash * 0.35;
            let stretch = (self.vy.abs() / 1200.0).min(0.25) * (1.0 - self.squash);
            if self.streak >= SMASH_FLOORS && self.state == State::Playing {
                for k in 0..6 {
                    art::sprite_at(
                        "ball",
                        vec2(bx, bz - k as f32 * 10.0),
                        BALL_R * 2.0 * (1.0 - k as f32 * 0.12),
                        crate::art::Draw {
                            tint: Color::new(1.0, 0.6, 0.2, 0.3),
                            ..Default::default()
                        },
                    );
                }
            }
            let w = BALL_R * 2.2 * (1.0 + sq - stretch * 0.5);
            let h = BALL_R * 2.2 * (1.0 - sq + stretch);
            art::sprite_ex(
                "ball",
                Rect::new(bx - w / 2.0, bz - h / 2.0, w, h),
                crate::art::Draw {
                    ..Default::default()
                },
            );
        } else {
            let by = to_screen(self.y);
            let bx = cx + ball_angle().cos() * (outer + inner) / 2.0;
            let bz = by + (outer + inner) / 2.0 * 0.32;
            art::sprite(
                "splat",
                Rect::new(
                    bx - BALL_R * 1.8,
                    bz + 4.0 - BALL_R * 0.9,
                    BALL_R * 3.6,
                    BALL_R * 1.8,
                ),
            );
        }
        self.fx.draw();
        self.pops.draw();
        let progress = self.passed as f32 / self.floors.len() as f32;
        let bar = Rect::new(view.w / 2.0 - 180.0, 24.0, 360.0, 26.0);
        crate::ui::nine("panel", bar, 14.0, bar.h / 64.0, WHITE);
        crate::ui::nine(
            "btn_yellow",
            Rect::new(bar.x, bar.y, (bar.w * progress).max(bar.h), bar.h),
            18.0,
            bar.h / 64.0,
            WHITE,
        );
        for (k, x) in [
            (self.level, bar.x - 26.0),
            (self.level + 1, bar.x + bar.w + 26.0),
        ] {
            let done = x < view.w / 2.0 || progress >= 1.0;
            crate::ui::sprite_at(
                if done { "round_yellow" } else { "round_white" },
                vec2(x, bar.y + 13.0),
                46.0,
                Default::default(),
            );
            text_outline(
                &k.to_string(),
                x,
                bar.y + 11.0,
                26.0,
                WHITE,
                if done {
                    Color::from_rgba(150, 90, 0, 255)
                } else {
                    crate::ui::INK
                },
            );
        }
        big_score(view, &self.score.to_string(), 100.0, self.bump.t);
        chip(view, Side::Right, "BEST", &self.best.0.to_string(), 1.0);
        match self.state {
            State::Ready => text_shadow(
                "Drag sideways to spin the tower",
                view.w / 2.0,
                view.h * 0.85,
                36.0,
                WHITE,
            ),
            State::Dead if self.end_t > 0.5 => {
                let s = format!("Score {}   Best {}", self.score, self.best.0);
                overlay(
                    view,
                    "Splat!",
                    &["Red parts are sticky!", &s, "Tap to try again"],
                );
            }
            State::Won if self.end_t > 0.8 => {
                let s = format!("Level {} done!", self.level);
                overlay(view, "Hooray!", &[&s, "Tap for the next level"]);
            }
            _ => {}
        }
    }
}

mod art {
    crate::sprites!("helix");

    pub fn bytes() -> &'static [u8] {
        SHEET_BYTES
    }
}

thread_local! {
    static TILES: std::cell::RefCell<Option<(Texture2D, crate::art::Index)>> =
        const { std::cell::RefCell::new(None) };
}

pub fn floor_tile(hue: f32) -> &'static str {
    ["tile_green", "tile_blue", "tile_pink", "tile_grey"][(hue.rem_euclid(1.0) * 4.0) as usize % 4]
}

pub struct Ring {
    pub centre: Vec2,
    pub a0: f32,
    pub a1: f32,
    pub outer: f32,
    pub inner: f32,
    pub thick: f32,
    pub flat: f32,
    pub tile: &'static str,
    pub light: f32,
    pub alpha: f32,
}

fn with_tiles(f: impl FnOnce(&Texture2D, &crate::art::Index)) {
    TILES.with(|t| {
        let mut t = t.borrow_mut();
        let (tex, index) = t.get_or_insert_with(|| {
            let index = crate::art::parse(art::bytes()).expect("bad helix sheet");
            let (at, len) = index.pages[0];
            let tex = Texture2D::from_file_with_format(
                &art::bytes()[at..at + len],
                Some(ImageFormat::Png),
            );
            tex.set_filter(FilterMode::Linear);
            (tex, index)
        });
        f(tex, index);
    });
}

fn textured_quad(tex: &Texture2D, pts: [Vec2; 4], uvs: [Vec2; 4], color: Color) {
    use macroquad::models::{draw_mesh, Mesh, Vertex};
    let vertices = (0..4)
        .map(|i| Vertex::new2(vec3(pts[i].x, pts[i].y, 0.0), uvs[i], color))
        .collect();
    draw_mesh(&Mesh {
        vertices,
        indices: vec![0, 1, 2, 0, 2, 3],
        texture: Some(tex.clone()),
    });
}

pub fn draw_ring_segment(r: &Ring) {
    with_tiles(|tex, index| {
        let Some(f) = index.frames.get(r.tile) else {
            return;
        };
        let (tw, th) = (tex.width(), tex.height());
        let uv = |u: f32, v: f32| {
            let (u, v) = (0.14 + u * 0.72, 0.14 + v * 0.72);
            vec2(
                (f.src.x + 0.5 + (f.src.w - 1.0) * u) / tw,
                (f.src.y + 0.5 + (f.src.h - 1.0) * v) / th,
            )
        };
        let steps = 4;
        let top = Color::new(r.light, r.light, r.light, r.alpha);
        let side = Color::new(r.light * 0.7, r.light * 0.7, r.light * 0.7, r.alpha);
        let p =
            |a: f32, rad: f32, dy: f32| r.centre + vec2(a.cos() * rad, a.sin() * rad * r.flat + dy);
        for s in 0..steps {
            let k0 = s as f32 / steps as f32;
            let k1 = (s + 1) as f32 / steps as f32;
            let b0 = r.a0 + (r.a1 - r.a0) * k0;
            let b1 = r.a0 + (r.a1 - r.a0) * k1;
            if b0.sin() + b1.sin() > -0.5 {
                textured_quad(
                    tex,
                    [
                        p(b0, r.outer, 0.0),
                        p(b1, r.outer, 0.0),
                        p(b1, r.outer, r.thick),
                        p(b0, r.outer, r.thick),
                    ],
                    [uv(k0, 0.8), uv(k1, 0.8), uv(k1, 1.0), uv(k0, 1.0)],
                    side,
                );
            }
            textured_quad(
                tex,
                [
                    p(b0, r.inner, 0.0),
                    p(b1, r.inner, 0.0),
                    p(b1, r.outer, 0.0),
                    p(b0, r.outer, 0.0),
                ],
                [uv(k0, 0.0), uv(k1, 0.0), uv(k1, 0.8), uv(k0, 0.8)],
                top,
            );
        }
    });
}

fn draw_pole(cx: f32, view: &View, width: f32) {
    let f = view.full();
    art::sprite("pole", Rect::new(cx - width / 2.0, f.y, width, f.h));
}

fn draw_backdrop(view: &View, level: u32) {
    let f = view.full();
    let s = art::size("bg");
    let k = (f.w / s.x).max(f.h / s.y);
    let (w, h) = (s.x * k, s.y * k);
    let hue = (level as f32 * 0.07).rem_euclid(1.0);
    art::sprite_ex(
        "bg",
        Rect::new(f.x + (f.w - w) / 2.0, f.y + (f.h - h) / 2.0, w, h),
        crate::art::Draw {
            tint: hsv(hue, 0.12, 1.0),
            ..Default::default()
        },
    );
}

fn draw_clouds(view: &View, cam: f32, t: f32) {
    for i in 0..6 {
        let depth = 0.08 + (i % 3) as f32 * 0.05;
        let span = view.w + 400.0;
        let x = (i as f32 * 233.0 + t * (5.0 + depth * 40.0)).rem_euclid(span) - 200.0;
        let y = ((i * 131 % 700) as f32 - cam * depth).rem_euclid(view.h + 200.0) - 100.0;
        art::sprite_w(
            if i % 2 == 0 { "cloud1" } else { "cloud2" },
            vec2(x, y),
            150.0 + depth * 900.0,
            crate::art::Draw {
                tint: Color::new(1.0, 1.0, 1.0, 0.85),
                ..Default::default()
            },
        );
    }
}

pub fn icon(r: Rect) {
    crate::ui::icon("helix", r);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn above(g: &mut Helix, floor: usize, seg: Seg) {
        let k = segment_at(ball_angle(), g.angle);
        g.floors[floor].segs[k] = seg;
        g.passed = floor;
        g.y = Helix::floor_y(floor) - BALL_R - 5.0;
        g.vy = 400.0;
        g.state = State::Playing;
    }

    #[test]
    fn helix_sprites_all_exist() {
        let have = art::names();
        for n in [
            "tile_green",
            "tile_pink",
            "tile_blue",
            "tile_grey",
            "tile_red",
            "ball",
            "pole",
            "splat",
            "cloud1",
            "cloud2",
            "bg",
        ] {
            assert!(have.iter().any(|h| h == n), "missing {n}");
        }
        for k in 0..8 {
            let name = floor_tile(k as f32 / 8.0);
            assert!(have.iter().any(|h| h == name), "missing {name}");
        }
    }

    #[test]
    fn hue_picks_a_safe_tile_that_is_never_red() {
        for k in 0..100 {
            assert_ne!(floor_tile(k as f32 / 100.0), "tile_red");
        }
        assert_eq!(floor_tile(1.0), floor_tile(0.0));
    }

    #[test]
    fn segment_lookup_wraps() {
        assert_eq!(segment_at(0.01, 0.0), 0);
        assert_eq!(segment_at(TAU - 0.01, 0.0), SEGMENTS - 1);
        assert_eq!(segment_at(0.01, TAU), 0);
        assert_eq!(segment_at(0.01, -TAU / SEGMENTS as f32), 1);
    }

    #[test]
    fn solid_floor_bounces() {
        let mut g = Helix::new();
        above(&mut g, 2, Seg::Solid);
        assert_eq!(g.physics(0.05), Some(Seg::Solid));
        assert_eq!(g.vy, BOUNCE);
        assert_eq!(g.passed, 2);
    }

    #[test]
    fn gap_lets_ball_through_and_scores() {
        let mut g = Helix::new();
        above(&mut g, 2, Seg::Gap);
        g.physics(0.05);
        assert_eq!(g.passed, 3);
        assert!(g.score > 0);
    }

    #[test]
    fn dying_plays_the_sad_jingle() {
        let mut g = Helix::new();
        above(&mut g, 2, Seg::Danger);
        crate::sfx::take_pending();
        g.update(&Input::default(), &View::ipad_landscape(), 0.05);
        assert_eq!(g.state, State::Dead);
        assert!(crate::sfx::take_pending().contains(&Sfx::Lose));
    }

    #[test]
    fn red_segment_is_deadly() {
        let mut g = Helix::new();
        above(&mut g, 2, Seg::Danger);
        assert_eq!(g.physics(0.05), Some(Seg::Danger));
    }

    #[test]
    fn falling_through_several_floors_smashes_red() {
        let mut g = Helix::new();
        above(&mut g, 3, Seg::Danger);
        g.streak = SMASH_FLOORS;
        assert_ne!(g.physics(0.05), Some(Seg::Danger));
        assert_eq!(g.passed, 4);
    }

    #[test]
    fn start_floor_is_safe_under_the_ball() {
        for _ in 0..30 {
            let g = Helix::new();
            assert_eq!(g.seg_under_ball(0), Seg::Solid);
            assert!(g.floors[0].segs.contains(&Seg::Gap));
            assert!(!g.floors[0].segs.contains(&Seg::Danger));
        }
    }

    #[test]
    fn every_floor_has_a_way_down() {
        for level in 1..8 {
            let g = Helix::with_level(level);
            for f in &g.floors {
                assert!(f.segs.contains(&Seg::Gap));
            }
        }
    }
}
