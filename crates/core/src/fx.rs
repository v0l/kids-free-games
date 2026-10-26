use crate::engine::{View, HUD_LEFT};
use crate::geom::{Color, Rect, WHITE};
use glam::{vec2, Vec2};
use quad_rand as rand;

pub fn ease_out_back(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    let c1 = 1.70158;
    let c3 = c1 + 1.0;
    1.0 + c3 * (t - 1.0).powi(3) + c1 * (t - 1.0).powi(2)
}

pub fn ease_out_cubic(t: f32) -> f32 {
    1.0 - (1.0 - t.clamp(0.0, 1.0)).powi(3)
}

pub fn ease_out_elastic(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    if t == 0.0 || t == 1.0 {
        return t;
    }
    let c4 = std::f32::consts::TAU / 3.0;
    2f32.powf(-10.0 * t) * ((t * 10.0 - 0.75) * c4).sin() + 1.0
}

pub fn approach(current: f32, target: f32, rate: f32, dt: f32) -> f32 {
    target + (current - target) * (-rate * dt).exp()
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Shape {
    Circle,
    Square,
    Star,
    Confetti,
}

#[derive(Clone, Copy, Debug)]
pub struct Particle {
    pub pos: Vec2,
    pub vel: Vec2,
    pub life: f32,
    pub max_life: f32,
    pub size: f32,
    pub color: Color,
    pub gravity: f32,
    pub drag: f32,
    pub spin: f32,
    pub angle: f32,
    pub shape: Shape,
}

#[derive(Default)]
pub struct Particles {
    pub list: Vec<Particle>,
}

pub struct Burst {
    pub count: usize,
    pub speed: (f32, f32),
    pub size: (f32, f32),
    pub life: (f32, f32),
    pub gravity: f32,
    pub drag: f32,
    pub shape: Shape,
    pub spread: f32,
    pub angle: f32,
}

impl Default for Burst {
    fn default() -> Self {
        Self {
            count: 16,
            speed: (120.0, 360.0),
            size: (4.0, 9.0),
            life: (0.4, 0.8),
            gravity: 600.0,
            drag: 1.5,
            shape: Shape::Circle,
            spread: std::f32::consts::TAU,
            angle: 0.0,
        }
    }
}

const MAX_PARTICLES: usize = 1500;

impl Particles {
    pub fn emit(&mut self, at: Vec2, colors: &[Color], b: Burst) {
        for i in 0..b.count {
            if self.list.len() >= MAX_PARTICLES {
                self.list.remove(0);
            }
            let a = b.angle + rand::gen_range(-0.5, 0.5) * b.spread;
            let speed = rand::gen_range(b.speed.0, b.speed.1);
            let life = rand::gen_range(b.life.0, b.life.1);
            self.list.push(Particle {
                pos: at,
                vel: vec2(a.cos(), a.sin()) * speed,
                life,
                max_life: life,
                size: rand::gen_range(b.size.0, b.size.1),
                color: colors[i % colors.len().max(1)],
                gravity: b.gravity,
                drag: b.drag,
                spin: rand::gen_range(-8.0, 8.0),
                angle: rand::gen_range(0.0, std::f32::consts::TAU),
                shape: b.shape,
            });
        }
    }

    pub fn sparkle(&mut self, at: Vec2, color: Color) {
        self.emit(
            at,
            &[color, WHITE, Color::from_rgba(255, 230, 120, 255)],
            Burst {
                count: 14,
                shape: Shape::Star,
                gravity: 200.0,
                size: (5.0, 10.0),
                ..Default::default()
            },
        );
    }

    pub fn puff(&mut self, at: Vec2, color: Color) {
        self.emit(
            at,
            &[color],
            Burst {
                count: 10,
                speed: (40.0, 140.0),
                size: (8.0, 16.0),
                life: (0.3, 0.6),
                gravity: -60.0,
                drag: 3.0,
                ..Default::default()
            },
        );
    }

    pub fn confetti(&mut self, view: &View) {
        let colors = [
            Color::from_rgba(255, 90, 90, 255),
            Color::from_rgba(255, 200, 60, 255),
            Color::from_rgba(90, 200, 120, 255),
            Color::from_rgba(80, 160, 250, 255),
            Color::from_rgba(200, 110, 240, 255),
        ];
        for k in 0..6 {
            let x = view.w * (k as f32 + 0.5) / 6.0;
            self.emit(
                vec2(x, -10.0),
                &colors,
                Burst {
                    count: 18,
                    speed: (100.0, 380.0),
                    size: (6.0, 11.0),
                    life: (1.6, 2.6),
                    gravity: 420.0,
                    drag: 1.2,
                    shape: Shape::Confetti,
                    spread: 1.6,
                    angle: std::f32::consts::FRAC_PI_2,
                },
            );
        }
    }

    pub fn update(&mut self, dt: f32) {
        for p in &mut self.list {
            p.vel.y += p.gravity * dt;
            p.vel *= (-p.drag * dt).exp();
            p.pos += p.vel * dt;
            p.angle += p.spin * dt;
            p.life -= dt;
        }
        self.list.retain(|p| p.life > 0.0);
    }
}

#[derive(Default)]
pub struct Shake {
    trauma: f32,
    t: f32,
}

impl Shake {
    pub fn add(&mut self, amount: f32) {
        self.trauma = (self.trauma + amount).min(1.0);
    }

    pub fn update(&mut self, dt: f32) {
        self.t += dt;
        self.trauma = (self.trauma - dt * 1.8).max(0.0);
    }

    pub fn offset(&self) -> Vec2 {
        let k = self.trauma * self.trauma * 18.0;
        vec2(
            (self.t * 53.0).sin() + (self.t * 97.0).sin() * 0.5,
            (self.t * 61.0).cos() + (self.t * 89.0).cos() * 0.5,
        ) * k
    }

    pub fn active(&self) -> bool {
        self.trauma > 0.0
    }
}

pub struct Popup {
    pub pos: Vec2,
    pub text: String,
    pub color: Color,
    pub age: f32,
    pub life: f32,
    pub size: f32,
}

#[derive(Default)]
pub struct Popups {
    pub list: Vec<Popup>,
}

impl Popups {
    pub fn add(&mut self, pos: Vec2, text: impl Into<String>, color: Color, size: f32) {
        self.list.push(Popup {
            pos,
            text: text.into(),
            color,
            age: 0.0,
            life: 1.0,
            size,
        });
    }

    pub fn update(&mut self, dt: f32) {
        for p in &mut self.list {
            p.age += dt;
            p.pos.y -= 70.0 * dt * (1.0 - p.age / p.life).max(0.0);
        }
        self.list.retain(|p| p.age < p.life);
    }
}

pub fn popup_scale(p: &Popup) -> (f32, f32) {
    let pop = ease_out_back((p.age / 0.25).min(1.0));
    let fade = 1.0 - ((p.age - p.life * 0.6) / (p.life * 0.4)).clamp(0.0, 1.0);
    (pop.max(0.1), fade)
}

pub fn particle_look(p: &Particle) -> (f32, f32) {
    let k = (p.life / p.max_life).clamp(0.0, 1.0);
    (
        p.color.a * k.min(1.0) * (0.3 + 0.7 * k.sqrt()),
        p.size * (0.4 + 0.6 * k),
    )
}

pub fn bump_scale(bump: f32, amount: f32) -> f32 {
    1.0 + amount * (1.0 - ease_out_cubic(bump))
}

pub struct Bump {
    pub t: f32,
}

impl Default for Bump {
    fn default() -> Self {
        Self { t: 1.0 }
    }
}

impl Bump {
    pub fn kick(&mut self) {
        self.t = 0.0;
    }

    pub fn update(&mut self, dt: f32) {
        self.t = (self.t + dt * 4.0).min(1.0);
    }

    pub fn scale(&self, amount: f32) -> f32 {
        1.0 + amount * (1.0 - ease_out_cubic(self.t))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    Left,
    Right,
}

pub const CHIP_Y: f32 = 12.0;
pub const CHIP_H: f32 = 48.0;
const CHIP_LABEL: f32 = 20.0;
const CHIP_VALUE: f32 = 34.0;
const CHIP_PAD: f32 = 16.0;
pub const INK: Color = Color::new(0.16, 0.12, 0.24, 1.0);

pub fn chip_rect(view: &View, side: Side, label: &str, value: &str) -> Rect {
    let w = chip_width(label, value);
    let x = match side {
        Side::Left => HUD_LEFT,
        Side::Right => view.w - 14.0 - w,
    };
    Rect::new(x, CHIP_Y, w, CHIP_H)
}

pub fn chip_width(label: &str, value: &str) -> f32 {
    let lw = if label.is_empty() {
        0.0
    } else {
        crate::font::measure(label, CHIP_LABEL) + 10.0
    };
    CHIP_PAD * 2.0 + lw + crate::font::measure(value, CHIP_VALUE)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn easings_hit_their_endpoints() {
        for f in [ease_out_back, ease_out_cubic, ease_out_elastic] {
            assert!(f(0.0).abs() < 1e-4);
            assert!((f(1.0) - 1.0).abs() < 1e-4);
        }
        assert!(ease_out_back(0.6) > 1.0);
    }

    #[test]
    fn particles_expire() {
        let mut p = Particles::default();
        p.emit(
            Vec2::ZERO,
            &[WHITE],
            Burst {
                count: 20,
                life: (0.1, 0.2),
                ..Default::default()
            },
        );
        assert_eq!(p.list.len(), 20);
        for _ in 0..30 {
            p.update(1.0 / 60.0);
        }
        assert!(p.list.is_empty());
    }

    #[test]
    fn particle_pool_is_capped() {
        let mut p = Particles::default();
        for _ in 0..200 {
            p.emit(
                Vec2::ZERO,
                &[WHITE],
                Burst {
                    count: 20,
                    ..Default::default()
                },
            );
        }
        assert!(p.list.len() <= MAX_PARTICLES);
    }

    #[test]
    fn shake_settles() {
        let mut s = Shake::default();
        s.add(0.8);
        assert!(s.active());
        for _ in 0..120 {
            s.update(1.0 / 60.0);
        }
        assert!(!s.active());
        assert_eq!(s.offset(), Vec2::ZERO);
    }

    #[test]
    fn approach_converges_without_overshoot() {
        let mut v = 0.0;
        for _ in 0..200 {
            let next = approach(v, 10.0, 8.0, 1.0 / 60.0);
            assert!(next >= v && next <= 10.0);
            v = next;
        }
        assert!((v - 10.0).abs() < 0.01);
    }
}
