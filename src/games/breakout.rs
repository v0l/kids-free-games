use crate::engine::*;
use macroquad::prelude::*;

pub const ROWS: usize = 6;
pub const COLS: usize = 10;
pub const BALL_R: f32 = 11.0;
pub const PADDLE_W: f32 = 170.0;
pub const PADDLE_H: f32 = 22.0;
pub const LIVES: u32 = 3;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum State {
    Serve,
    Playing,
    Lost,
    Cleared,
}

pub struct Breakout {
    pub bricks: Vec<Option<Rect>>,
    pub ball: Vec2,
    pub vel: Vec2,
    pub paddle_x: f32,
    pub lives: u32,
    pub score: u32,
    pub level: u32,
    pub state: State,
    size: Vec2,
}

impl Breakout {
    pub fn new() -> Self {
        let mut b = Self {
            bricks: vec![],
            ball: Vec2::ZERO,
            vel: Vec2::ZERO,
            paddle_x: 0.0,
            lives: LIVES,
            score: 0,
            level: 1,
            state: State::Serve,
            size: Vec2::ZERO,
        };
        b.build(View::fit(MIN_W, MIN_H));
        b
    }

    fn paddle_y(&self) -> f32 {
        self.size.y - 70.0
    }

    pub fn paddle_rect(&self) -> Rect {
        Rect::new(
            self.paddle_x - PADDLE_W / 2.0,
            self.paddle_y(),
            PADDLE_W,
            PADDLE_H,
        )
    }

    fn build(&mut self, view: View) {
        self.size = vec2(view.w, view.h);
        self.paddle_x = view.w / 2.0;
        let margin = 30.0;
        let bw = (view.w - margin * 2.0) / COLS as f32;
        let bh = 34.0;
        self.bricks = (0..ROWS * COLS)
            .map(|i| {
                let (r, c) = (i / COLS, i % COLS);
                Some(Rect::new(
                    margin + c as f32 * bw + 3.0,
                    90.0 + r as f32 * bh + 3.0,
                    bw - 6.0,
                    bh - 6.0,
                ))
            })
            .collect();
        self.serve();
    }

    fn serve(&mut self) {
        self.state = State::Serve;
        self.vel = Vec2::ZERO;
        self.ball = vec2(self.paddle_x, self.paddle_y() - BALL_R - 1.0);
    }

    pub fn speed(&self) -> f32 {
        430.0 + self.level as f32 * 40.0
    }

    pub fn step(&mut self, dt: f32) {
        let steps = ((self.vel.length() * dt) / (BALL_R * 0.5)).ceil().max(1.0) as usize;
        let h = dt / steps as f32;
        for _ in 0..steps {
            self.substep(h);
            if self.state != State::Playing {
                return;
            }
        }
    }

    fn substep(&mut self, dt: f32) {
        self.ball += self.vel * dt;
        if self.ball.x < BALL_R {
            self.ball.x = BALL_R;
            self.vel.x = self.vel.x.abs();
        }
        if self.ball.x > self.size.x - BALL_R {
            self.ball.x = self.size.x - BALL_R;
            self.vel.x = -self.vel.x.abs();
        }
        if self.ball.y < 70.0 + BALL_R {
            self.ball.y = 70.0 + BALL_R;
            self.vel.y = self.vel.y.abs();
        }
        let p = self.paddle_rect();
        if self.vel.y > 0.0 && circle_hits_rect(self.ball, BALL_R, p) {
            let k = ((self.ball.x - self.paddle_x) / (PADDLE_W / 2.0)).clamp(-1.0, 1.0);
            let angle = k * 1.05;
            self.vel = vec2(angle.sin(), -angle.cos()) * self.speed();
            self.ball.y = p.y - BALL_R;
        }
        for b in self.bricks.iter_mut() {
            let Some(r) = *b else { continue };
            if !circle_hits_rect(self.ball, BALL_R, r) {
                continue;
            }
            *b = None;
            self.score += 10;
            let c = r.center();
            let d = (self.ball - c) / vec2(r.w, r.h);
            if d.x.abs() > d.y.abs() {
                self.vel.x = self.vel.x.abs() * d.x.signum();
            } else {
                self.vel.y = self.vel.y.abs() * d.y.signum();
            }
            break;
        }
        if self.ball.y > self.size.y + BALL_R {
            self.lives -= 1;
            if self.lives == 0 {
                self.state = State::Lost;
            } else {
                self.serve();
            }
        } else if self.bricks.iter().all(|b| b.is_none()) {
            self.state = State::Cleared;
        }
    }
}

impl Default for Breakout {
    fn default() -> Self {
        Self::new()
    }
}

impl Game for Breakout {
    fn update(&mut self, input: &Input, view: &View, dt: f32) {
        if vec2(view.w, view.h) != self.size && self.state == State::Serve && self.score == 0 {
            self.build(*view);
        }
        self.size = vec2(view.w, view.h);
        if input.down {
            self.paddle_x = input.pos.x;
        }
        if input.left {
            self.paddle_x -= 700.0 * dt;
        }
        if input.right {
            self.paddle_x += 700.0 * dt;
        }
        self.paddle_x = self.paddle_x.clamp(PADDLE_W / 2.0, view.w - PADDLE_W / 2.0);
        match self.state {
            State::Serve => {
                self.ball = vec2(self.paddle_x, self.paddle_y() - BALL_R - 1.0);
                if input.released || input.action {
                    self.state = State::Playing;
                    self.vel = vec2(0.35, -1.0).normalize() * self.speed();
                }
            }
            State::Playing => self.step(dt),
            State::Lost => {
                if input.tap() {
                    *self = Self::new();
                    self.build(*view);
                }
            }
            State::Cleared => {
                if input.tap() {
                    self.level += 1;
                    self.build(*view);
                }
            }
        }
    }

    fn draw(&self, view: &View) {
        sky(
            view,
            Color::from_rgba(30, 30, 70, 255),
            Color::from_rgba(70, 40, 110, 255),
        );
        for (i, b) in self.bricks.iter().enumerate() {
            if let Some(r) = b {
                let c = hsv((i / COLS) as f32 / ROWS as f32 * 0.8, 0.65, 0.95);
                round_rect(*r, 6.0, c);
                draw_rectangle(
                    r.x + 4.0,
                    r.y + 3.0,
                    r.w - 8.0,
                    5.0,
                    with_alpha(WHITE, 0.35),
                );
            }
        }
        let p = self.paddle_rect();
        round_rect(p, PADDLE_H / 2.0, Color::from_rgba(240, 240, 255, 255));
        round_rect(
            Rect::new(p.x + 6.0, p.y + 4.0, p.w - 12.0, 6.0),
            3.0,
            Color::from_rgba(150, 200, 255, 255),
        );
        draw_circle(self.ball.x, self.ball.y, BALL_R, WHITE);
        draw_circle(
            self.ball.x - 3.0,
            self.ball.y - 3.0,
            BALL_R * 0.35,
            Color::from_rgba(200, 230, 255, 255),
        );
        text(
            &format!("Score {}", self.score),
            HUD_LEFT,
            48.0,
            38.0,
            WHITE,
        );
        text_center(
            &format!("Level {}", self.level),
            view.w / 2.0,
            34.0,
            34.0,
            Color::from_rgba(200, 200, 255, 255),
        );
        for i in 0..self.lives {
            draw_circle(view.w - 30.0 - i as f32 * 34.0, 35.0, 11.0, WHITE);
        }
        match self.state {
            State::Serve => text_shadow(
                "Drag to move, let go to launch",
                view.w / 2.0,
                view.h * 0.62,
                38.0,
                WHITE,
            ),
            State::Lost => {
                let s = format!("Score {}", self.score);
                overlay(view, "Out of balls", &[&s, "Tap to play again"]);
            }
            State::Cleared => overlay(view, "Level clear!", &["Tap for the next level"]),
            State::Playing => {}
        }
    }
}

pub fn icon(r: Rect) {
    draw_rectangle(r.x, r.y, r.w, r.h, Color::from_rgba(40, 35, 85, 255));
    for row in 0..3 {
        for col in 0..4 {
            draw_rectangle(
                r.x + 4.0 + col as f32 * r.w * 0.24,
                r.y + 8.0 + row as f32 * r.h * 0.12,
                r.w * 0.22,
                r.h * 0.1,
                hsv(row as f32 * 0.2, 0.65, 0.95),
            );
        }
    }
    draw_circle(r.x + r.w * 0.55, r.y + r.h * 0.65, r.w * 0.06, WHITE);
    draw_rectangle(
        r.x + r.w * 0.3,
        r.y + r.h * 0.85,
        r.w * 0.4,
        r.h * 0.06,
        WHITE,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    fn playing() -> Breakout {
        let mut b = Breakout::new();
        b.state = State::Playing;
        b
    }

    #[test]
    fn ball_breaks_brick_and_bounces() {
        let mut b = playing();
        let brick = b.bricks[ROWS * COLS - 1].unwrap();
        b.ball = vec2(brick.center().x, brick.y + brick.h + BALL_R + 5.0);
        b.vel = vec2(0.0, -500.0);
        b.step(0.05);
        assert!(b.bricks[ROWS * COLS - 1].is_none());
        assert_eq!(b.score, 10);
        assert!(b.vel.y > 0.0);
    }

    #[test]
    fn fast_ball_does_not_tunnel_through_paddle() {
        let mut b = playing();
        b.ball = vec2(b.paddle_x, b.paddle_y() - 60.0);
        b.vel = vec2(0.0, 3000.0);
        b.step(1.0 / 30.0);
        assert!(b.vel.y < 0.0);
        assert_eq!(b.lives, LIVES);
    }

    #[test]
    fn edge_of_paddle_sends_ball_sideways() {
        let mut b = playing();
        b.ball = vec2(
            b.paddle_x + PADDLE_W / 2.0 - 2.0,
            b.paddle_y() - BALL_R + 1.0,
        );
        b.vel = vec2(0.0, 400.0);
        b.step(0.001);
        assert!(b.vel.x > 0.0 && b.vel.y < 0.0);
    }

    #[test]
    fn missing_the_ball_costs_a_life() {
        let mut b = playing();
        b.ball = vec2(10.0 + BALL_R, b.size.y - 5.0);
        b.paddle_x = b.size.x - PADDLE_W;
        b.vel = vec2(0.0, 600.0);
        b.step(0.1);
        assert_eq!(b.lives, LIVES - 1);
        assert_eq!(b.state, State::Serve);
    }
}
