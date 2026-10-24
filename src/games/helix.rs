use crate::engine::*;
use crate::fx::*;
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
        let hue = 0.55 + self.level as f32 * 0.09;
        sky(view, hsv(hue, 0.35, 1.0), hsv(hue + 0.08, 0.55, 0.85));
        let sh = self.shake.offset();
        let cx = view.w / 2.0 + sh.x;
        let to_screen = |y: f32| y - self.cam + view.h * 0.15 + sh.y;
        let (outer, inner) = self.ring();
        let pole_w = inner * 1.1;
        draw_rectangle(
            cx - pole_w / 2.0,
            0.0,
            pole_w,
            view.h,
            Color::from_rgba(250, 245, 240, 255),
        );
        draw_rectangle(
            cx - pole_w / 2.0,
            0.0,
            pole_w * 0.3,
            view.h,
            Color::new(1.0, 1.0, 1.0, 0.6),
        );
        draw_rectangle(
            cx + pole_w * 0.25,
            0.0,
            pole_w * 0.25,
            view.h,
            Color::new(0.0, 0.0, 0.0, 0.06),
        );
        for (i, f) in self.floors.iter().enumerate() {
            let y = to_screen(Self::floor_y(i));
            if y < -100.0 || y > view.h + 100.0 {
                continue;
            }
            let base = hsv(f.hue, 0.55, 0.98);
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
                let depth = mid.sin();
                let col = match seg {
                    Seg::Danger => Color::from_rgba(240, 60, 70, 255),
                    _ => base,
                };
                let lit = darker(col, 0.75 + 0.25 * depth.max(-0.2));
                let push = if f.broken > 0.0 {
                    vec2(mid.cos(), 0.0) * fall * 0.6
                } else {
                    Vec2::ZERO
                };
                let yy = y + fall;
                let steps = 4;
                for s in 0..steps {
                    let b0 = a0 + (a1 - a0) * s as f32 / steps as f32;
                    let b1 = a0 + (a1 - a0) * (s + 1) as f32 / steps as f32;
                    let p = |a: f32, r: f32, dy: f32| {
                        vec2(cx + a.cos() * r, yy + a.sin() * r * 0.32 + dy) + push
                    };
                    let thick = 18.0;
                    if b0.sin() + b1.sin() > -0.5 {
                        draw_triangle(
                            p(b0, outer, 0.0),
                            p(b1, outer, 0.0),
                            p(b0, outer, thick),
                            with_alpha(darker(col, 0.6), fade),
                        );
                        draw_triangle(
                            p(b1, outer, 0.0),
                            p(b1, outer, thick),
                            p(b0, outer, thick),
                            with_alpha(darker(col, 0.6), fade),
                        );
                    }
                    draw_triangle(
                        p(b0, inner, 0.0),
                        p(b0, outer, 0.0),
                        p(b1, outer, 0.0),
                        with_alpha(lit, fade),
                    );
                    draw_triangle(
                        p(b0, inner, 0.0),
                        p(b1, outer, 0.0),
                        p(b1, inner, 0.0),
                        with_alpha(lit, fade),
                    );
                }
            }
            for (fi, seg, at) in &self.splats {
                if *fi == i && f.broken == 0.0 {
                    let a = self.angle - at + (*seg + 0.5) / SEGMENTS as f32 * TAU;
                    let r = (outer + inner) / 2.0;
                    draw_ellipse(
                        cx + a.cos() * r,
                        y + a.sin() * r * 0.32,
                        16.0,
                        6.0,
                        0.0,
                        Color::from_rgba(255, 120, 60, 200),
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
                    draw_circle(
                        bx,
                        bz - k as f32 * 10.0,
                        BALL_R * (1.0 - k as f32 * 0.12),
                        Color::new(1.0, 0.5, 0.2, 0.25),
                    );
                }
            }
            draw_ellipse(
                bx,
                bz,
                BALL_R * (1.0 + sq - stretch * 0.5),
                BALL_R * (1.0 - sq + stretch),
                0.0,
                Color::from_rgba(255, 120, 60, 255),
            );
            draw_circle(
                bx - 6.0,
                bz - 6.0,
                BALL_R * 0.3,
                Color::new(1.0, 1.0, 1.0, 0.6),
            );
        } else {
            let by = to_screen(self.y);
            let bx = cx + ball_angle().cos() * (outer + inner) / 2.0;
            let bz = by + (outer + inner) / 2.0 * 0.32;
            draw_ellipse(
                bx,
                bz + 4.0,
                BALL_R * 1.8,
                BALL_R * 0.5,
                0.0,
                Color::from_rgba(255, 120, 60, 255),
            );
        }
        self.fx.draw();
        self.pops.draw();
        let progress = self.passed as f32 / self.floors.len() as f32;
        let bar = Rect::new(view.w / 2.0 - 180.0, 24.0, 360.0, 26.0);
        round_rect(bar, 13.0, Color::new(1.0, 1.0, 1.0, 0.5));
        round_rect(
            Rect::new(bar.x, bar.y, (bar.w * progress).max(26.0), bar.h),
            13.0,
            Color::from_rgba(255, 120, 60, 255),
        );
        for (k, x) in [
            (self.level, bar.x - 26.0),
            (self.level + 1, bar.x + bar.w + 26.0),
        ] {
            draw_circle(
                x,
                bar.y + 13.0,
                22.0,
                if x < view.w / 2.0 || progress >= 1.0 {
                    Color::from_rgba(255, 120, 60, 255)
                } else {
                    WHITE
                },
            );
            text_center(
                &k.to_string(),
                x,
                bar.y + 13.0,
                26.0,
                if x < view.w / 2.0 || progress >= 1.0 {
                    WHITE
                } else {
                    Color::from_rgba(255, 120, 60, 255)
                },
            );
        }
        score_text(
            &self.score.to_string(),
            view.w / 2.0,
            100.0,
            64.0 * self.bump.scale(0.3),
            1.0,
        );
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

pub fn icon(r: Rect) {
    let c = vec2(r.x + r.w / 2.0, r.y);
    draw_rectangle(
        c.x - r.w * 0.08,
        r.y,
        r.w * 0.16,
        r.h,
        Color::from_rgba(250, 245, 240, 255),
    );
    for (k, col) in [
        (0.25, hsv(0.55, 0.55, 0.98)),
        (0.55, hsv(0.85, 0.55, 0.98)),
        (0.85, hsv(0.15, 0.55, 0.98)),
    ] {
        let y = c.y + r.h * k;
        draw_ellipse(c.x, y, r.w * 0.45, r.h * 0.1, 0.0, col);
        draw_ellipse(
            c.x,
            y,
            r.w * 0.12,
            r.h * 0.03,
            0.0,
            Color::from_rgba(250, 245, 240, 255),
        );
    }
    draw_triangle(
        vec2(c.x + r.w * 0.05, r.y + r.h * 0.55),
        vec2(c.x + r.w * 0.45, r.y + r.h * 0.45),
        vec2(c.x + r.w * 0.45, r.y + r.h * 0.65),
        Color::from_rgba(150, 200, 255, 255),
    );
    draw_circle(
        c.x + r.w * 0.25,
        r.y + r.h * 0.38,
        r.w * 0.08,
        Color::from_rgba(255, 120, 60, 255),
    );
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
