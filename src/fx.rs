use crate::engine::*;
use macroquad::models::{draw_mesh, Mesh, Vertex};
use macroquad::prelude::*;

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

    pub fn draw(&self) {
        for p in &self.list {
            let k = (p.life / p.max_life).clamp(0.0, 1.0);
            let c = with_alpha(p.color, p.color.a * k.min(1.0) * (0.3 + 0.7 * k.sqrt()));
            let s = p.size * (0.4 + 0.6 * k);
            match p.shape {
                Shape::Circle => draw_circle(p.pos.x, p.pos.y, s, c),
                Shape::Square => draw_poly(p.pos.x, p.pos.y, 4, s, p.angle.to_degrees(), c),
                Shape::Star => star(p.pos, s * 1.3, p.angle, c),
                Shape::Confetti => {
                    let w = s * (p.angle * 2.0).cos().abs().max(0.15);
                    draw_rectangle_ex(
                        p.pos.x,
                        p.pos.y,
                        w * 1.6,
                        s,
                        DrawRectangleParams {
                            rotation: p.angle * 0.3,
                            offset: vec2(0.5, 0.5),
                            color: c,
                        },
                    );
                }
            }
        }
    }
}

pub fn star(c: Vec2, r: f32, angle: f32, color: Color) {
    for k in 0..5 {
        let a = angle + k as f32 * std::f32::consts::TAU / 5.0 - std::f32::consts::FRAC_PI_2;
        let b0 = a - std::f32::consts::PI / 5.0;
        let b1 = a + std::f32::consts::PI / 5.0;
        draw_triangle(
            c + vec2(a.cos(), a.sin()) * r,
            c + vec2(b0.cos(), b0.sin()) * r * 0.42,
            c + vec2(b1.cos(), b1.sin()) * r * 0.42,
            color,
        );
    }
    draw_poly(
        c.x,
        c.y,
        5,
        r * 0.45,
        angle.to_degrees() - 90.0 + 36.0,
        color,
    );
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

    pub fn draw(&self) {
        for p in &self.list {
            let pop = ease_out_back((p.age / 0.25).min(1.0));
            let fade = 1.0 - ((p.age - p.life * 0.6) / (p.life * 0.4)).clamp(0.0, 1.0);
            text_outline(
                &p.text,
                p.pos.x,
                p.pos.y,
                p.size * pop.max(0.1),
                with_alpha(p.color, fade),
                with_alpha(Color::from_rgba(40, 30, 60, 255), fade),
            );
        }
    }
}

pub fn text_outline(s: &str, cx: f32, cy: f32, size: f32, fill: Color, outline: Color) {
    let o = (size * 0.07).max(1.5);
    for (dx, dy) in [
        (-o, 0.0),
        (o, 0.0),
        (0.0, -o),
        (0.0, o),
        (-o * 0.7, -o * 0.7),
        (o * 0.7, -o * 0.7),
        (-o * 0.7, o * 0.7),
        (o * 0.7, o * 0.7),
        (0.0, o * 1.6),
    ] {
        text_center(s, cx + dx, cy + dy, size, outline);
    }
    text_center(s, cx, cy, size, fill);
}

pub fn score_text(s: &str, cx: f32, cy: f32, size: f32, bump: f32) {
    let k = 1.0 + 0.35 * (1.0 - ease_out_cubic(bump)).max(0.0) * if bump < 1.0 { 1.0 } else { 0.0 };
    text_outline(
        s,
        cx,
        cy,
        size * k,
        WHITE,
        Color::from_rgba(40, 30, 60, 255),
    );
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

pub fn panel(view: &View, title: &str, lines: &[&str], age: f32, accent: Color) {
    let fade = ease_out_cubic(age / 0.3);
    draw_rectangle(
        0.0,
        0.0,
        view.w,
        view.h,
        Color::new(0.08, 0.05, 0.15, 0.5 * fade),
    );
    let pw = 600.0f32.min(view.w - 40.0);
    let ph = 190.0 + lines.len() as f32 * 50.0;
    let s = ease_out_back(age / 0.45).max(0.01);
    let c = view.center() + vec2(0.0, (1.0 - ease_out_cubic(age / 0.45)) * 80.0);
    let r = Rect::new(c.x - pw * s / 2.0, c.y - ph * s / 2.0, pw * s, ph * s);
    round_rect(
        Rect::new(r.x, r.y + 10.0 * s, r.w, r.h),
        34.0 * s,
        Color::new(0.0, 0.0, 0.0, 0.25),
    );
    round_rect(r, 34.0 * s, darker(accent, 0.75));
    round_rect(Rect::new(r.x, r.y, r.w, r.h - 8.0 * s), 34.0 * s, WHITE);
    let band = Rect::new(r.x, r.y, r.w, 96.0 * s);
    round_rect(band, 34.0 * s, accent);
    draw_rectangle(band.x, band.y + band.h - 34.0 * s, band.w, 34.0 * s, accent);
    for i in 0..6 {
        let x = band.x + band.w * (i as f32 + 0.5) / 6.0;
        draw_circle(x, band.y + band.h * 0.3, 6.0 * s, with_alpha(WHITE, 0.15));
    }
    let wob = (age * 3.0).sin() * 0.03;
    text_outline(
        title,
        c.x,
        band.y + band.h / 2.0,
        58.0 * s * (1.0 + wob),
        WHITE,
        darker(accent, 0.55),
    );
    for (i, l) in lines.iter().enumerate() {
        let la = ((age - 0.2 - i as f32 * 0.08) / 0.25).clamp(0.0, 1.0);
        let last = i + 1 == lines.len();
        let y = band.y + band.h + 50.0 * s + i as f32 * 50.0 * s + (1.0 - la) * 16.0;
        if last && lines.len() > 1 {
            let pulse = 1.0 + (age * 5.0).sin() * 0.04;
            let bw = (text_size(l, 32.0).width + 60.0) * s * pulse;
            let br = Rect::new(c.x - bw / 2.0, y - 26.0 * s, bw, 52.0 * s);
            round_rect(
                Rect::new(br.x, br.y + 5.0, br.w, br.h),
                26.0 * s,
                with_alpha(darker(accent, 0.6), la),
            );
            round_rect(br, 26.0 * s, with_alpha(accent, la));
            text_center(l, c.x, y, 30.0 * s * pulse, with_alpha(WHITE, la));
        } else {
            text_center(
                l,
                c.x,
                y,
                36.0 * s,
                with_alpha(Color::from_rgba(70, 60, 100, 255), la),
            );
        }
    }
}

pub fn vignette(view: &View, strength: f32) {
    let inset = view.w.min(view.h) * 0.22;
    let edge = Color::new(0.0, 0.0, 0.05, strength * 0.45);
    let clear = Color::new(0.0, 0.0, 0.05, 0.0);
    let outer = [
        vec2(0.0, 0.0),
        vec2(view.w, 0.0),
        vec2(view.w, view.h),
        vec2(0.0, view.h),
    ];
    let inner = [
        vec2(inset, inset),
        vec2(view.w - inset, inset),
        vec2(view.w - inset, view.h - inset),
        vec2(inset, view.h - inset),
    ];
    let mut vertices = vec![];
    for k in 0..4 {
        vertices.push(Vertex::new2(outer[k].extend(0.0), Vec2::ZERO, edge));
        vertices.push(Vertex::new2(inner[k].extend(0.0), Vec2::ZERO, clear));
    }
    let mut indices = vec![];
    for k in 0..4u16 {
        let (o0, i0, o1, i1) = (k * 2, k * 2 + 1, ((k + 1) % 4) * 2, ((k + 1) % 4) * 2 + 1);
        indices.extend_from_slice(&[o0, o1, i1, o0, i1, i0]);
    }
    draw_mesh(&Mesh {
        vertices,
        indices,
        texture: None,
    });
}

pub fn soft_shadow(c: Vec2, rx: f32, ry: f32, alpha: f32) {
    for i in 0..4 {
        let k = 1.0 - i as f32 * 0.18;
        draw_ellipse(
            c.x,
            c.y,
            rx * k,
            ry * k,
            0.0,
            Color::new(0.0, 0.0, 0.0, alpha * 0.3),
        );
    }
}

pub fn gloss(r: Rect, radius: f32) {
    round_rect(
        Rect::new(r.x + r.w * 0.08, r.y + r.h * 0.06, r.w * 0.84, r.h * 0.32),
        radius * 0.6,
        Color::new(1.0, 1.0, 1.0, 0.22),
    );
}

pub fn shine_sweep(r: Rect, t: f32) {
    let x = r.x - r.w * 0.5 + (t.rem_euclid(3.0) / 3.0) * r.w * 2.0;
    if x > r.x + r.w || x + r.w * 0.2 < r.x {
        return;
    }
    let a = x.max(r.x);
    let b = (x + r.w * 0.18).min(r.x + r.w);
    if b > a {
        draw_rectangle(a, r.y, b - a, r.h, Color::new(1.0, 1.0, 1.0, 0.18));
    }
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
