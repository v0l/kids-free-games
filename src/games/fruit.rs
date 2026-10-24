use crate::engine::*;
use macroquad::prelude::*;

pub const GRAVITY: f32 = 900.0;
pub const LIVES: u32 = 3;
pub const BLADE_LIFE: f32 = 0.15;
pub const MIN_SLICE_SPEED: f32 = 4.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Kind {
    Fruit(usize),
    Bomb,
}

pub struct Thing {
    pub pos: Vec2,
    pub vel: Vec2,
    pub r: f32,
    pub kind: Kind,
    pub spin: f32,
}

struct Half {
    pos: Vec2,
    vel: Vec2,
    r: f32,
    fruit: usize,
    angle: f32,
    spin: f32,
}

struct Splat {
    pos: Vec2,
    vel: Vec2,
    color: Color,
    life: f32,
}

type Rgb = (u8, u8, u8);

const FRUITS: [(Rgb, Rgb); 5] = [
    ((220, 40, 50), (255, 230, 220)),
    ((250, 150, 30), (255, 210, 120)),
    ((60, 170, 60), (240, 70, 70)),
    ((250, 220, 50), (255, 250, 200)),
    ((130, 60, 170), (220, 190, 240)),
];

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum State {
    Ready,
    Playing,
    Over,
}

pub struct Fruit {
    pub things: Vec<Thing>,
    halves: Vec<Half>,
    splats: Vec<Splat>,
    pub blade: Vec<(Vec2, f32)>,
    pub score: u32,
    pub best: Best,
    pub lives: u32,
    pub state: State,
    wave_in: f32,
    time: f32,
    flash: f32,
}

fn rgb(c: Rgb) -> Color {
    Color::from_rgba(c.0, c.1, c.2, 255)
}

impl Fruit {
    pub fn new() -> Self {
        Self {
            things: vec![],
            halves: vec![],
            splats: vec![],
            blade: vec![],
            score: 0,
            best: Best::default(),
            lives: LIVES,
            state: State::Ready,
            wave_in: 0.8,
            time: 0.0,
            flash: 0.0,
        }
    }

    fn launch_wave(&mut self, view: &View) {
        let count = 1 + (self.time / 20.0) as usize + rand::gen_range(0, 3);
        let bomb_chance = (0.08 + self.time * 0.003).min(0.25);
        for _ in 0..count.min(6) {
            let x = rand::gen_range(view.w * 0.15, view.w * 0.85);
            let peak = rand::gen_range(view.h * 0.12, view.h * 0.45);
            let vy = -(2.0 * GRAVITY * (view.h + 40.0 - peak)).sqrt();
            let vx = (view.w / 2.0 - x) * rand::gen_range(0.2, 0.6);
            let kind = if rand::gen_range(0.0, 1.0) < bomb_chance {
                Kind::Bomb
            } else {
                Kind::Fruit(rand::gen_range(0, FRUITS.len()))
            };
            self.things.push(Thing {
                pos: vec2(x, view.h + 40.0),
                vel: vec2(vx, vy),
                r: if kind == Kind::Bomb {
                    34.0
                } else {
                    rand::gen_range(34.0, 46.0)
                },
                kind,
                spin: rand::gen_range(-3.0, 3.0),
            });
        }
    }

    pub fn slice(&mut self, a: Vec2, b: Vec2) {
        if a.distance(b) < MIN_SLICE_SPEED {
            return;
        }
        let dir = (b - a).normalize_or_zero();
        let mut i = 0;
        let mut combo = 0;
        while i < self.things.len() {
            let t = &self.things[i];
            if !segment_hits_circle(a, b, t.pos, t.r) {
                i += 1;
                continue;
            }
            let t = self.things.swap_remove(i);
            match t.kind {
                Kind::Bomb => {
                    self.lives = 0;
                    self.flash = 1.0;
                }
                Kind::Fruit(f) => {
                    combo += 1;
                    self.score += 1;
                    let side = vec2(-dir.y, dir.x) * 120.0;
                    let angle = dir.y.atan2(dir.x);
                    for s in [-1.0, 1.0] {
                        self.halves.push(Half {
                            pos: t.pos,
                            vel: t.vel * 0.4 + side * s,
                            r: t.r,
                            fruit: f,
                            angle,
                            spin: s * 4.0,
                        });
                    }
                    for _ in 0..10 {
                        let v =
                            vec2(rand::gen_range(-1.0, 1.0), rand::gen_range(-1.0, 1.0)) * 260.0;
                        self.splats.push(Splat {
                            pos: t.pos,
                            vel: v,
                            color: rgb(FRUITS[f].1),
                            life: 0.6,
                        });
                    }
                }
            }
        }
        if combo >= 3 {
            self.score += combo;
        }
    }
}

impl Default for Fruit {
    fn default() -> Self {
        Self::new()
    }
}

impl Game for Fruit {
    fn update(&mut self, input: &Input, view: &View, dt: f32) {
        self.flash = (self.flash - dt * 2.0).max(0.0);
        for b in &mut self.blade {
            b.1 -= dt;
        }
        self.blade.retain(|b| b.1 > 0.0);
        if input.down {
            if input.pressed {
                self.blade.clear();
            }
            self.blade.push((input.pos, BLADE_LIFE));
        }
        for h in &mut self.halves {
            h.vel.y += GRAVITY * dt;
            h.pos += h.vel * dt;
            h.angle += h.spin * dt;
        }
        self.halves.retain(|h| h.pos.y < view.h + 100.0);
        for s in &mut self.splats {
            s.vel.y += GRAVITY * 0.5 * dt;
            s.pos += s.vel * dt;
            s.life -= dt;
        }
        self.splats.retain(|s| s.life > 0.0);
        match self.state {
            State::Ready => {
                if input.tap() {
                    self.state = State::Playing;
                }
            }
            State::Playing => {
                self.time += dt;
                if input.down && !input.pressed {
                    self.slice(input.prev_pos, input.pos);
                }
                for t in &mut self.things {
                    t.vel.y += GRAVITY * dt;
                    t.pos += t.vel * dt;
                }
                let mut missed = 0;
                self.things.retain(|t| {
                    let gone = t.vel.y > 0.0 && t.pos.y > view.h + t.r + 10.0;
                    if gone && matches!(t.kind, Kind::Fruit(_)) {
                        missed += 1;
                    }
                    !gone
                });
                self.lives = self.lives.saturating_sub(missed);
                self.wave_in -= dt;
                if self.wave_in <= 0.0 && self.things.is_empty() {
                    self.launch_wave(view);
                    self.wave_in = (1.2 - self.time * 0.005).max(0.5);
                }
                if self.lives == 0 {
                    self.state = State::Over;
                    self.best.offer(self.score);
                }
            }
            State::Over => {
                if input.tap() && self.flash == 0.0 {
                    let best = self.best;
                    *self = Self::new();
                    self.best = best;
                    self.state = State::Playing;
                }
            }
        }
    }

    fn draw(&self, view: &View) {
        sky(
            view,
            Color::from_rgba(110, 70, 40, 255),
            Color::from_rgba(70, 40, 25, 255),
        );
        for i in 0..((view.w / 120.0) as i32 + 1) {
            draw_line(
                i as f32 * 120.0,
                0.0,
                i as f32 * 120.0,
                view.h,
                4.0,
                Color::from_rgba(60, 35, 20, 255),
            );
        }
        for s in &self.splats {
            draw_circle(s.pos.x, s.pos.y, 6.0, with_alpha(s.color, s.life / 0.6));
        }
        for t in &self.things {
            match t.kind {
                Kind::Bomb => {
                    draw_circle(t.pos.x, t.pos.y, t.r, Color::from_rgba(40, 40, 50, 255));
                    draw_circle(
                        t.pos.x - t.r * 0.3,
                        t.pos.y - t.r * 0.3,
                        t.r * 0.25,
                        Color::from_rgba(90, 90, 100, 255),
                    );
                    draw_line(
                        t.pos.x,
                        t.pos.y - t.r,
                        t.pos.x + 10.0,
                        t.pos.y - t.r - 16.0,
                        4.0,
                        Color::from_rgba(160, 120, 80, 255),
                    );
                    let spark = (self.time * 30.0).sin() * 3.0 + 7.0;
                    draw_circle(t.pos.x + 10.0, t.pos.y - t.r - 16.0, spark, ORANGE);
                    text_center("X", t.pos.x, t.pos.y + 2.0, t.r, RED);
                }
                Kind::Fruit(f) => {
                    let (skin, _) = FRUITS[f];
                    draw_circle(t.pos.x, t.pos.y, t.r, darker(rgb(skin), 0.75));
                    draw_circle(t.pos.x, t.pos.y, t.r - 4.0, rgb(skin));
                    draw_circle(
                        t.pos.x - t.r * 0.35,
                        t.pos.y - t.r * 0.35,
                        t.r * 0.22,
                        with_alpha(WHITE, 0.45),
                    );
                    draw_ellipse(
                        t.pos.x + t.r * 0.2,
                        t.pos.y - t.r,
                        t.r * 0.3,
                        t.r * 0.14,
                        t.spin * self.time * 20.0,
                        Color::from_rgba(60, 160, 50, 255),
                    );
                }
            }
        }
        for h in &self.halves {
            let (skin, flesh) = FRUITS[h.fruit];
            let n = vec2(h.angle.cos(), h.angle.sin());
            let off = vec2(-n.y, n.x) * if h.spin > 0.0 { 1.0 } else { -1.0 };
            let base = h.pos;
            let steps = 12;
            let start = off.y.atan2(off.x) - std::f32::consts::FRAC_PI_2;
            for i in 0..steps {
                let a0 = start + std::f32::consts::PI * i as f32 / steps as f32;
                let a1 = start + std::f32::consts::PI * (i + 1) as f32 / steps as f32;
                draw_triangle(
                    base,
                    base + vec2(a0.cos(), a0.sin()) * h.r,
                    base + vec2(a1.cos(), a1.sin()) * h.r,
                    rgb(skin),
                );
                draw_triangle(
                    base,
                    base + vec2(a0.cos(), a0.sin()) * (h.r - 6.0),
                    base + vec2(a1.cos(), a1.sin()) * (h.r - 6.0),
                    rgb(flesh),
                );
            }
        }
        for w in self.blade.windows(2) {
            let k = w[1].1 / BLADE_LIFE;
            draw_line(
                w[0].0.x,
                w[0].0.y,
                w[1].0.x,
                w[1].0.y,
                3.0 + 9.0 * k,
                with_alpha(Color::from_rgba(220, 240, 255, 255), k),
            );
        }
        text_shadow(
            &self.score.to_string(),
            HUD_LEFT + 50.0,
            50.0,
            60.0,
            Color::from_rgba(255, 220, 60, 255),
        );
        for i in 0..LIVES {
            let x = view.w - 50.0 - i as f32 * 50.0;
            let c = if i < self.lives {
                Color::from_rgba(240, 60, 70, 255)
            } else {
                Color::from_rgba(90, 60, 50, 255)
            };
            draw_circle(x - 8.0, 40.0, 11.0, c);
            draw_circle(x + 8.0, 40.0, 11.0, c);
            draw_triangle(vec2(x - 19.0, 44.0), vec2(x + 19.0, 44.0), vec2(x, 64.0), c);
        }
        if self.flash > 0.0 {
            draw_rectangle(0.0, 0.0, view.w, view.h, with_alpha(WHITE, self.flash));
        }
        match self.state {
            State::Ready => overlay(
                view,
                "Fruit Slice",
                &[
                    "Swipe across fruit to slice it",
                    "Don't touch the bombs!",
                    "Tap to start",
                ],
            ),
            State::Over => {
                let s = format!("Score {}   Best {}", self.score, self.best.0);
                overlay(view, "Game over", &[&s, "Tap to play again"]);
            }
            State::Playing => {}
        }
    }
}

pub fn icon(r: Rect) {
    draw_rectangle(r.x, r.y, r.w, r.h, Color::from_rgba(110, 70, 40, 255));
    let c = vec2(r.x + r.w * 0.5, r.y + r.h * 0.55);
    draw_circle(c.x, c.y, r.w * 0.3, Color::from_rgba(60, 170, 60, 255));
    draw_circle(c.x, c.y, r.w * 0.25, Color::from_rgba(240, 70, 70, 255));
    for i in 0..5 {
        let a = i as f32 * 1.25;
        draw_circle(
            c.x + a.cos() * r.w * 0.13,
            c.y + a.sin() * r.w * 0.13,
            3.0,
            BLACK,
        );
    }
    draw_line(
        r.x + r.w * 0.1,
        r.y + r.h * 0.85,
        r.x + r.w * 0.9,
        r.y + r.h * 0.15,
        5.0,
        WHITE,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    fn with(kind: Kind, at: Vec2) -> Fruit {
        let mut g = Fruit::new();
        g.state = State::Playing;
        g.things.push(Thing {
            pos: at,
            vel: Vec2::ZERO,
            r: 40.0,
            kind,
            spin: 0.0,
        });
        g
    }

    #[test]
    fn swipe_through_fruit_scores() {
        let mut g = with(Kind::Fruit(0), vec2(300.0, 300.0));
        g.slice(vec2(200.0, 290.0), vec2(400.0, 310.0));
        assert_eq!(g.score, 1);
        assert!(g.things.is_empty());
    }

    #[test]
    fn swipe_that_misses_does_nothing() {
        let mut g = with(Kind::Fruit(0), vec2(300.0, 300.0));
        g.slice(vec2(200.0, 400.0), vec2(400.0, 400.0));
        assert_eq!(g.score, 0);
        assert_eq!(g.things.len(), 1);
    }

    #[test]
    fn resting_finger_does_not_slice() {
        let mut g = with(Kind::Fruit(0), vec2(300.0, 300.0));
        g.slice(vec2(300.0, 300.0), vec2(301.0, 300.0));
        assert_eq!(g.score, 0);
    }

    #[test]
    fn slicing_a_bomb_ends_the_game() {
        let mut g = with(Kind::Bomb, vec2(300.0, 300.0));
        g.slice(vec2(200.0, 300.0), vec2(400.0, 300.0));
        g.update(&Input::default(), &View::ipad_landscape(), 0.01);
        assert_eq!(g.state, State::Over);
    }

    #[test]
    fn dropped_fruit_costs_a_life() {
        let v = View::ipad_landscape();
        let mut g = with(Kind::Fruit(1), vec2(300.0, v.h + 100.0));
        g.things[0].vel = vec2(0.0, 10.0);
        g.update(&Input::default(), &v, 0.01);
        assert_eq!(g.lives, LIVES - 1);
    }
}
