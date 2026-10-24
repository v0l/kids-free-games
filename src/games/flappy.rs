use crate::engine::*;
use crate::fx::*;
use macroquad::prelude::*;

pub const GRAVITY: f32 = 1500.0;
pub const FLAP: f32 = -520.0;
pub const SPEED: f32 = 200.0;
pub const GAP: f32 = 230.0;
pub const PIPE_W: f32 = 96.0;
pub const SPACING: f32 = 330.0;
pub const BIRD_R: f32 = 22.0;
pub const GROUND: f32 = 90.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum State {
    Ready,
    Playing,
    Dead,
}

pub struct Pipe {
    pub x: f32,
    pub gap_y: f32,
    pub scored: bool,
}

pub fn layer_x(i: usize, spacing: f32, scroll: f32, factor: f32, span: f32, margin: f32) -> f32 {
    (i as f32 * spacing - scroll * factor).rem_euclid(span) - margin
}

pub fn bird_scale(vy: f32, kick: f32) -> Vec2 {
    let len = 1.0 + (vy.abs() / 1400.0).min(0.2) + kick.clamp(0.0, 1.0) * 0.12;
    vec2(len, 1.0 / len)
}

pub struct Flappy {
    pub state: State,
    pub y: f32,
    pub vy: f32,
    pub pipes: Vec<Pipe>,
    pub score: u32,
    pub best: Best,
    t: f32,
    dead_t: f32,
    scroll: f32,
    kick: f32,
    flash: f32,
    fx: Particles,
    shake: Shake,
    popups: Popups,
    bump: Bump,
}

impl Flappy {
    pub fn new() -> Self {
        Self {
            state: State::Ready,
            y: 300.0,
            vy: 0.0,
            pipes: vec![],
            score: 0,
            best: Best::default(),
            t: 0.0,
            dead_t: 0.0,
            scroll: 0.0,
            kick: 0.0,
            flash: 0.0,
            fx: Particles::default(),
            shake: Shake::default(),
            popups: Popups::default(),
            bump: Bump::default(),
        }
    }

    fn bird_x(view: &View) -> f32 {
        view.w * 0.3
    }

    fn reset(&mut self, view: &View) {
        let best = self.best;
        *self = Self::new();
        self.best = best;
        self.y = view.h * 0.42;
    }

    pub fn pipe_rects(p: &Pipe, view: &View) -> [Rect; 2] {
        let top = p.gap_y - GAP / 2.0;
        let bottom = p.gap_y + GAP / 2.0;
        [
            Rect::new(p.x, -200.0, PIPE_W, top + 200.0),
            Rect::new(p.x, bottom, PIPE_W, view.h - bottom),
        ]
    }

    fn spawn_pipe(&mut self, x: f32, view: &View) {
        let lo = 150.0;
        let hi = (view.h - GROUND - 150.0).max(lo + 1.0);
        self.pipes.push(Pipe {
            x,
            gap_y: rand::gen_range(lo, hi),
            scored: false,
        });
    }

    fn flap(&mut self, view: &View) {
        self.vy = FLAP;
        self.kick = 1.0;
        let at = vec2(Self::bird_x(view) - BIRD_R * 0.8, self.y + BIRD_R * 0.3);
        self.fx.emit(
            at,
            &[WHITE, Color::from_rgba(255, 245, 200, 255)],
            Burst {
                count: 5,
                speed: (40.0, 120.0),
                size: (3.0, 6.0),
                life: (0.3, 0.55),
                gravity: 160.0,
                drag: 2.5,
                spread: 1.2,
                angle: std::f32::consts::PI * 0.8,
                ..Default::default()
            },
        );
    }

    fn die(&mut self, view: &View) {
        self.state = State::Dead;
        self.dead_t = 0.0;
        self.best.offer(self.score);
        self.shake.add(0.9);
        self.flash = 1.0;
        self.fx.emit(
            vec2(Self::bird_x(view), self.y),
            &[
                Color::from_rgba(255, 210, 40, 255),
                WHITE,
                Color::from_rgba(255, 120, 40, 255),
            ],
            Burst {
                count: 22,
                speed: (120.0, 420.0),
                size: (4.0, 9.0),
                life: (0.5, 0.9),
                gravity: 700.0,
                shape: Shape::Star,
                ..Default::default()
            },
        );
    }
}

impl Default for Flappy {
    fn default() -> Self {
        Self::new()
    }
}

impl Game for Flappy {
    fn update(&mut self, input: &Input, view: &View, dt: f32) {
        self.t += dt;
        self.kick = approach(self.kick, 0.0, 9.0, dt);
        self.flash = (self.flash - dt * 2.5).max(0.0);
        self.fx.update(dt);
        self.shake.update(dt);
        self.popups.update(dt);
        self.bump.update(dt);
        let ground_y = view.h - GROUND;
        match self.state {
            State::Ready => {
                self.y = view.h * 0.42 + (self.t * 3.0).sin() * 10.0;
                self.scroll += SPEED * dt;
                if input.tap() {
                    self.state = State::Playing;
                    self.flap(view);
                }
            }
            State::Playing => {
                if input.tap() {
                    self.flap(view);
                }
                self.vy += GRAVITY * dt;
                self.y += self.vy * dt;
                if self.y < BIRD_R {
                    self.y = BIRD_R;
                    self.vy = 0.0;
                }
                self.scroll += SPEED * dt;
                for p in &mut self.pipes {
                    p.x -= SPEED * dt;
                }
                self.pipes.retain(|p| p.x > -PIPE_W - 10.0);
                let last_x = self.pipes.last().map(|p| p.x);
                match last_x {
                    None => self.spawn_pipe(view.w + 60.0, view),
                    Some(x) if x < view.w - SPACING => self.spawn_pipe(x + SPACING, view),
                    _ => {}
                }
                let bird = vec2(Self::bird_x(view), self.y);
                let mut hit = self.y + BIRD_R >= ground_y;
                let mut scored_at = None;
                for p in &mut self.pipes {
                    if Self::pipe_rects(p, view)
                        .iter()
                        .any(|r| circle_hits_rect(bird, BIRD_R - 4.0, *r))
                    {
                        hit = true;
                    }
                    if !p.scored && p.x + PIPE_W < bird.x {
                        p.scored = true;
                        self.score += 1;
                        self.bump.kick();
                        scored_at = Some(vec2(p.x + PIPE_W / 2.0, p.gap_y));
                    }
                }
                if let Some(at) = scored_at {
                    self.fx.sparkle(at, Color::from_rgba(255, 210, 40, 255));
                    self.popups.add(
                        vec2(bird.x + 50.0, self.y - 40.0),
                        "+1",
                        Color::from_rgba(255, 230, 90, 255),
                        44.0,
                    );
                }
                if hit {
                    self.die(view);
                }
            }
            State::Dead => {
                self.dead_t += dt;
                if self.y + BIRD_R < ground_y {
                    self.vy += GRAVITY * dt;
                    self.y = (self.y + self.vy * dt).min(ground_y - BIRD_R);
                }
                if self.dead_t > 0.6 && input.tap() {
                    self.reset(view);
                }
            }
        }
    }

    fn draw(&self, view: &View) {
        set_view_camera(view, self.shake.offset());
        self.draw_world(view);
        self.fx.draw();
        self.popups.draw();
        set_view_camera(view, Vec2::ZERO);
        if self.flash > 0.0 {
            draw_rectangle(
                0.0,
                0.0,
                view.w,
                view.h,
                with_alpha(WHITE, self.flash * 0.8),
            );
        }
        score_text(
            &self.score.to_string(),
            view.w / 2.0,
            80.0,
            84.0,
            self.bump.t,
        );
        match self.state {
            State::Ready => {
                let bob = (self.t * 4.0).sin() * 4.0;
                text_shadow(
                    "Tap to flap!",
                    view.w / 2.0,
                    view.h * 0.62 + bob,
                    56.0,
                    WHITE,
                );
            }
            State::Dead if self.dead_t > 0.4 => {
                let s = format!("Score {}   Best {}", self.score, self.best.0);
                overlay(view, "Bonk!", &[&s, "Tap to fly again"]);
            }
            _ => {}
        }
    }
}

impl Flappy {
    fn draw_world(&self, view: &View) {
        let gy = view.h - GROUND;
        sky(
            view,
            Color::from_rgba(90, 190, 240, 255),
            Color::from_rgba(200, 240, 255, 255),
        );
        let sun = vec2(view.w * 0.82, 120.0);
        draw_circle(sun.x, sun.y, 70.0, Color::new(1.0, 0.95, 0.7, 0.25));
        draw_circle(sun.x, sun.y, 48.0, Color::new(1.0, 0.95, 0.7, 0.5));
        draw_circle(sun.x, sun.y, 32.0, Color::from_rgba(255, 248, 200, 255));
        let cw = view.w + 400.0;
        for i in 0..5 {
            let x = layer_x(i, 300.0, self.scroll, 0.25, cw, 200.0);
            cloud(
                x,
                90.0 + (i * 53 % 160) as f32,
                1.0 + (i % 3) as f32 * 0.3,
                with_alpha(WHITE, 0.85),
            );
        }
        let far = Color::from_rgba(150, 215, 190, 255);
        for i in 0..6 {
            let x = layer_x(i, 330.0, self.scroll, 0.12, view.w + 660.0, 330.0);
            let h = 120.0 + (i * 37 % 80) as f32;
            draw_ellipse(x, gy + 20.0, 230.0, h, 0.0, far);
        }
        let hills = Color::from_rgba(120, 200, 120, 255);
        for i in 0..8 {
            let x = layer_x(i, 220.0, self.scroll, 0.4, view.w + 440.0, 220.0);
            draw_circle(x, gy + 40.0, 140.0, hills);
        }
        let bush = Color::from_rgba(70, 170, 80, 255);
        for i in 0..10 {
            let x = layer_x(i, 170.0, self.scroll, 0.7, view.w + 340.0, 170.0);
            let s = 0.8 + (i * 31 % 5) as f32 * 0.12;
            draw_circle(x, gy + 6.0, 34.0 * s, bush);
            draw_circle(x + 30.0 * s, gy - 4.0, 26.0 * s, darker(bush, 1.1));
            draw_circle(x - 28.0 * s, gy, 24.0 * s, darker(bush, 0.9));
            draw_circle(x - 8.0 * s, gy - 16.0 * s, 8.0 * s, with_alpha(WHITE, 0.18));
        }
        for p in &self.pipes {
            Self::draw_pipe(p, view);
        }
        draw_rectangle(
            0.0,
            gy,
            view.w,
            GROUND,
            Color::from_rgba(222, 200, 130, 255),
        );
        draw_rectangle(
            0.0,
            gy + 34.0,
            view.w,
            GROUND,
            Color::from_rgba(205, 180, 110, 255),
        );
        draw_rectangle(0.0, gy, view.w, 18.0, Color::from_rgba(110, 200, 70, 255));
        draw_rectangle(0.0, gy, view.w, 4.0, Color::from_rgba(150, 225, 100, 255));
        for i in 0..((view.w / 40.0) as i32 + 2) {
            let x = i as f32 * 40.0 - self.scroll.rem_euclid(40.0);
            draw_triangle(
                vec2(x, gy + 18.0),
                vec2(x + 20.0, gy + 18.0),
                vec2(x, gy + 30.0),
                Color::from_rgba(90, 170, 60, 255),
            );
        }
        self.draw_bird(view);
    }

    fn draw_pipe(p: &Pipe, view: &View) {
        let body = Color::from_rgba(80, 180, 60, 255);
        let dark = Color::from_rgba(50, 135, 45, 255);
        let light = Color::from_rgba(140, 220, 100, 255);
        let outline = Color::from_rgba(40, 100, 40, 255);
        let [top, bottom] = Self::pipe_rects(p, view);
        for (r, is_top) in [(top, true), (bottom, false)] {
            draw_rectangle(r.x - 2.0, r.y, r.w + 4.0, r.h, outline);
            draw_rectangle(r.x, r.y, r.w, r.h, body);
            draw_rectangle(r.x + 6.0, r.y, 14.0, r.h, light);
            draw_rectangle(r.x + 24.0, r.y, 6.0, r.h, with_alpha(WHITE, 0.18));
            draw_rectangle(r.x + r.w * 0.72, r.y, r.w * 0.28, r.h, dark);
            let lip_y = if is_top { r.y + r.h - 34.0 } else { r.y };
            let lip = Rect::new(r.x - 8.0, lip_y, PIPE_W + 16.0, 34.0);
            round_rect(
                Rect::new(lip.x - 2.0, lip.y - 2.0, lip.w + 4.0, lip.h + 4.0),
                8.0,
                outline,
            );
            round_rect(lip, 7.0, body);
            draw_rectangle(lip.x + 6.0, lip.y + 3.0, 14.0, lip.h - 6.0, light);
            draw_rectangle(
                lip.x + lip.w * 0.74,
                lip.y + 3.0,
                lip.w * 0.22,
                lip.h - 6.0,
                dark,
            );
            gloss(lip, 8.0);
        }
    }

    fn draw_bird(&self, view: &View) {
        let bx = Self::bird_x(view);
        let angle = (self.vy / 900.0).clamp(-0.5, 1.2);
        let fwd = vec2(angle.cos(), angle.sin());
        let up = vec2(fwd.y, -fwd.x);
        let sc = bird_scale(self.vy, self.kick);
        let c = vec2(bx, self.y);
        let ground_y = view.h - GROUND;
        let height = (ground_y - self.y).max(0.0);
        soft_shadow(
            vec2(bx, ground_y + 14.0),
            BIRD_R * (1.2 - (height / 600.0).min(0.6)),
            5.0,
            0.8,
        );
        let deg = angle.to_degrees();
        let gold = Color::from_rgba(255, 210, 40, 255);
        let tail = c - fwd * BIRD_R * 0.9;
        for k in [-1.0, 0.0, 1.0] {
            let tip = tail - fwd * 14.0 + up * k * 9.0;
            draw_triangle(
                tail + up * (k * 4.0 + 4.0),
                tail + up * (k * 4.0 - 4.0),
                tip,
                Color::from_rgba(240, 160, 30, 255),
            );
        }
        draw_ellipse(
            c.x,
            c.y,
            (BIRD_R + 2.0) * sc.x,
            (BIRD_R + 2.0) * sc.y,
            deg,
            Color::from_rgba(200, 130, 20, 255),
        );
        draw_ellipse(c.x, c.y, BIRD_R * sc.x, BIRD_R * sc.y, deg, gold);
        let belly = c - up * BIRD_R * 0.35 * sc.y + fwd * 2.0;
        draw_ellipse(
            belly.x,
            belly.y,
            BIRD_R * 0.75 * sc.x,
            BIRD_R * 0.45 * sc.y,
            deg,
            Color::from_rgba(255, 238, 170, 255),
        );
        let flap = (self.t * 18.0).sin() * 6.0 * (0.4 + self.kick);
        let wing = c - fwd * 6.0 * sc.x + up * -flap * 0.4;
        draw_ellipse(
            wing.x,
            wing.y + flap,
            13.0 * sc.x,
            8.0 * sc.y,
            deg,
            Color::from_rgba(250, 240, 200, 255),
        );
        draw_ellipse(
            wing.x,
            wing.y + flap,
            13.0 * sc.x,
            8.0 * sc.y,
            deg,
            with_alpha(Color::from_rgba(240, 170, 40, 255), 0.35),
        );
        let eye = c + fwd * 9.0 * sc.x + up * 8.0 * sc.y;
        draw_circle(eye.x, eye.y, 8.0, WHITE);
        draw_circle(eye.x + fwd.x * 3.0, eye.y + fwd.y * 3.0, 4.0, BLACK);
        draw_circle(
            eye.x + fwd.x * 2.0 + 1.5,
            eye.y + fwd.y * 2.0 - 2.0,
            1.5,
            WHITE,
        );
        let cheek = c + fwd * 8.0 * sc.x - up * 3.0 * sc.y;
        draw_circle(cheek.x, cheek.y, 4.0, Color::new(1.0, 0.45, 0.4, 0.45));
        let beak = c + fwd * (BIRD_R * sc.x - 2.0);
        draw_triangle(
            beak + up * 6.0,
            beak - up * 6.0,
            beak + fwd * 16.0,
            Color::from_rgba(255, 120, 40, 255),
        );
        draw_triangle(
            beak + up * 6.0,
            beak - up * 0.5,
            beak + fwd * 16.0,
            Color::from_rgba(255, 160, 70, 255),
        );
    }
}

pub fn icon(r: Rect) {
    draw_rectangle(
        r.x + r.w * 0.62,
        r.y,
        r.w * 0.2,
        r.h * 0.3,
        Color::from_rgba(80, 180, 60, 255),
    );
    draw_rectangle(
        r.x + r.w * 0.62,
        r.y + r.h * 0.72,
        r.w * 0.2,
        r.h * 0.28,
        Color::from_rgba(80, 180, 60, 255),
    );
    let c = vec2(r.x + r.w * 0.35, r.y + r.h * 0.5);
    draw_circle(c.x, c.y, r.w * 0.16, Color::from_rgba(255, 210, 40, 255));
    draw_circle(c.x + r.w * 0.06, c.y - r.w * 0.05, r.w * 0.05, WHITE);
    draw_triangle(
        vec2(c.x + r.w * 0.13, c.y - 6.0),
        vec2(c.x + r.w * 0.13, c.y + 6.0),
        vec2(c.x + r.w * 0.24, c.y),
        Color::from_rgba(255, 120, 40, 255),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    fn step(g: &mut Flappy, input: &Input, n: usize) {
        let v = View::ipad_landscape();
        for _ in 0..n {
            g.update(input, &v, 1.0 / 60.0);
        }
    }

    #[test]
    fn bird_without_flaps_hits_ground() {
        let mut g = Flappy::new();
        step(&mut g, &Input::press_at(vec2(500.0, 500.0)), 1);
        assert_eq!(g.state, State::Playing);
        step(&mut g, &Input::default(), 240);
        assert_eq!(g.state, State::Dead);
    }

    #[test]
    fn passing_a_pipe_scores() {
        let v = View::ipad_landscape();
        let mut g = Flappy::new();
        g.state = State::Playing;
        g.y = 300.0;
        g.pipes.push(Pipe {
            x: Flappy::bird_x(&v) - PIPE_W + 1.0,
            gap_y: 300.0,
            scored: false,
        });
        g.update(&Input::press_at(Vec2::ZERO), &v, 1.0 / 60.0);
        assert_eq!(g.score, 1);
        assert_eq!(g.state, State::Playing);
    }

    #[test]
    fn flying_into_a_pipe_kills() {
        let v = View::ipad_landscape();
        let mut g = Flappy::new();
        g.state = State::Playing;
        g.y = 100.0;
        g.pipes.push(Pipe {
            x: Flappy::bird_x(&v) - 10.0,
            gap_y: 450.0,
            scored: false,
        });
        g.update(&Input::press_at(Vec2::ZERO), &v, 1.0 / 60.0);
        assert_eq!(g.state, State::Dead);
    }

    #[test]
    fn flapping_puffs_feathers_and_squashes() {
        let mut g = Flappy::new();
        step(&mut g, &Input::press_at(vec2(500.0, 500.0)), 1);
        assert!(!g.fx.list.is_empty());
        assert!(g.kick > 0.5);
        step(&mut g, &Input::default(), 120);
        assert!(g.kick < 0.01);
    }

    #[test]
    fn scoring_bumps_and_sparkles() {
        let v = View::ipad_landscape();
        let mut g = Flappy::new();
        g.state = State::Playing;
        g.y = 300.0;
        g.pipes.push(Pipe {
            x: Flappy::bird_x(&v) - PIPE_W + 1.0,
            gap_y: 300.0,
            scored: false,
        });
        g.update(&Input::default(), &v, 1.0 / 60.0);
        assert_eq!(g.score, 1);
        assert!(g.bump.t < 1.0);
        assert!(g.fx.list.iter().any(|p| p.shape == Shape::Star));
        assert_eq!(g.popups.list.len(), 1);
    }

    #[test]
    fn hitting_a_pipe_shakes_and_flashes() {
        let v = View::ipad_landscape();
        let mut g = Flappy::new();
        g.state = State::Playing;
        g.y = 100.0;
        g.pipes.push(Pipe {
            x: Flappy::bird_x(&v) - 10.0,
            gap_y: 450.0,
            scored: false,
        });
        g.update(&Input::default(), &v, 1.0 / 60.0);
        assert_eq!(g.state, State::Dead);
        assert!(g.shake.active());
        assert!(g.flash > 0.9);
        step(&mut g, &Input::default(), 120);
        assert!(!g.shake.active());
        assert_eq!(g.flash, 0.0);
    }

    #[test]
    fn restart_clears_effects() {
        let mut g = Flappy::new();
        step(&mut g, &Input::press_at(vec2(500.0, 500.0)), 1);
        step(&mut g, &Input::default(), 240);
        assert_eq!(g.state, State::Dead);
        step(&mut g, &Input::press_at(vec2(500.0, 500.0)), 1);
        assert_eq!(g.state, State::Ready);
        assert!(g.fx.list.is_empty());
        assert!(!g.shake.active());
    }

    #[test]
    fn bird_scale_preserves_volume_and_stretches_when_fast() {
        let rest = bird_scale(0.0, 0.0);
        assert_eq!(rest, vec2(1.0, 1.0));
        let fast = bird_scale(-520.0, 1.0);
        assert!(fast.x > 1.1 && fast.y < 0.9);
        assert!((fast.x * fast.y - 1.0).abs() < 1e-4);
    }

    #[test]
    fn parallax_layers_wrap_inside_their_span() {
        for scroll in [0.0, 137.0, 99999.0] {
            for i in 0..8 {
                let x = layer_x(i, 220.0, scroll, 0.4, 1400.0, 220.0);
                assert!((-220.0..1180.0).contains(&x));
            }
        }
        let slow = layer_x(1, 220.0, 100.0, 0.1, 1400.0, 220.0);
        let fast = layer_x(1, 220.0, 100.0, 0.7, 1400.0, 220.0);
        assert!(slow > fast);
    }
}
