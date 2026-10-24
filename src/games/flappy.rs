use crate::engine::*;
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

    fn die(&mut self) {
        self.state = State::Dead;
        self.dead_t = 0.0;
        self.best.offer(self.score);
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
        let ground_y = view.h - GROUND;
        match self.state {
            State::Ready => {
                self.y = view.h * 0.42 + (self.t * 3.0).sin() * 10.0;
                self.scroll += SPEED * dt;
                if input.tap() {
                    self.state = State::Playing;
                    self.vy = FLAP;
                }
            }
            State::Playing => {
                if input.tap() {
                    self.vy = FLAP;
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
                    }
                }
                if hit {
                    self.die();
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
        sky(
            view,
            Color::from_rgba(90, 190, 240, 255),
            Color::from_rgba(200, 240, 255, 255),
        );
        let cw = view.w + 400.0;
        for i in 0..5 {
            let x = (i as f32 * 300.0 - self.scroll * 0.25).rem_euclid(cw) - 200.0;
            cloud(
                x,
                90.0 + (i * 53 % 160) as f32,
                1.0 + (i % 3) as f32 * 0.3,
                with_alpha(WHITE, 0.85),
            );
        }
        let hills = Color::from_rgba(120, 200, 120, 255);
        for i in 0..8 {
            let x = (i as f32 * 220.0 - self.scroll * 0.5).rem_euclid(view.w + 440.0) - 220.0;
            draw_circle(x, view.h - GROUND + 40.0, 140.0, hills);
        }
        let pipe = Color::from_rgba(80, 180, 60, 255);
        let lip = Color::from_rgba(60, 150, 45, 255);
        for p in &self.pipes {
            let [top, bottom] = Self::pipe_rects(p, view);
            draw_rectangle(top.x, top.y, top.w, top.h, pipe);
            draw_rectangle(bottom.x, bottom.y, bottom.w, bottom.h, pipe);
            draw_rectangle(top.x + 10.0, top.y, 12.0, top.h, with_alpha(WHITE, 0.25));
            draw_rectangle(
                bottom.x + 10.0,
                bottom.y,
                12.0,
                bottom.h,
                with_alpha(WHITE, 0.25),
            );
            draw_rectangle(top.x - 8.0, top.y + top.h - 34.0, PIPE_W + 16.0, 34.0, lip);
            draw_rectangle(bottom.x - 8.0, bottom.y, PIPE_W + 16.0, 34.0, lip);
        }
        let gy = view.h - GROUND;
        draw_rectangle(
            0.0,
            gy,
            view.w,
            GROUND,
            Color::from_rgba(222, 200, 130, 255),
        );
        draw_rectangle(0.0, gy, view.w, 18.0, Color::from_rgba(110, 200, 70, 255));
        for i in 0..((view.w / 40.0) as i32 + 2) {
            let x = i as f32 * 40.0 - self.scroll.rem_euclid(40.0);
            draw_triangle(
                vec2(x, gy + 18.0),
                vec2(x + 20.0, gy + 18.0),
                vec2(x, gy + 30.0),
                Color::from_rgba(90, 170, 60, 255),
            );
        }

        let bx = Self::bird_x(view);
        let angle = (self.vy / 900.0).clamp(-0.5, 1.2);
        let fwd = vec2(angle.cos(), angle.sin());
        let up = vec2(fwd.y, -fwd.x);
        let c = vec2(bx, self.y);
        draw_circle(c.x, c.y, BIRD_R, Color::from_rgba(255, 210, 40, 255));
        let flap = (self.t * 18.0).sin() * 6.0;
        draw_ellipse(
            c.x - fwd.x * 6.0,
            c.y - fwd.y * 6.0 + flap,
            13.0,
            8.0,
            angle.to_degrees(),
            Color::from_rgba(250, 240, 200, 255),
        );
        let eye = c + fwd * 9.0 + up * 8.0;
        draw_circle(eye.x, eye.y, 8.0, WHITE);
        draw_circle(eye.x + fwd.x * 3.0, eye.y + fwd.y * 3.0, 4.0, BLACK);
        let beak = c + fwd * (BIRD_R - 2.0);
        draw_triangle(
            beak + up * 6.0,
            beak - up * 6.0,
            beak + fwd * 16.0,
            Color::from_rgba(255, 120, 40, 255),
        );

        text_shadow(&self.score.to_string(), view.w / 2.0, 80.0, 84.0, WHITE);
        match self.state {
            State::Ready => {
                text_shadow("Tap to flap!", view.w / 2.0, view.h * 0.62, 56.0, WHITE);
            }
            State::Dead if self.dead_t > 0.4 => {
                let s = format!("Score {}   Best {}", self.score, self.best.0);
                overlay(view, "Bonk!", &[&s, "Tap to fly again"]);
            }
            _ => {}
        }
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
}
