use crate::engine::draw_circle;
use crate::engine::*;
use crate::fx::*;
use macroquad::prelude::*;

pub const ROWS: usize = 6;
pub const COLS: usize = 10;
pub const BALL_R: f32 = 11.0;
pub const PADDLE_W: f32 = 170.0;
pub const PADDLE_H: f32 = 22.0;
pub const LIVES: u32 = 3;
const TRAIL_LEN: usize = 14;
const COMBO_WINDOW: f32 = 1.2;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Event {
    BrickBroken {
        at: Vec2,
        row: usize,
        points: u32,
        combo: u32,
    },
    PaddleHit(Vec2),
    WallHit(Vec2),
    Lost(Vec2),
    LevelCleared,
}

pub fn combo_points(combo: u32) -> u32 {
    10 * combo.clamp(1, 8)
}

pub fn squash_scale(t: f32) -> Vec2 {
    let k = (1.0 - ease_out_cubic(t / 0.3)).max(0.0);
    vec2(1.0 + 0.25 * k, 1.0 - 0.45 * k)
}

pub fn brick_color(row: usize) -> Color {
    hsv(row as f32 / ROWS as f32 * 0.8, 0.65, 0.95)
}

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
    pub combo: u32,
    pub events: Vec<Event>,
    size: Vec2,
    since_break: f32,
    paddle_age: f32,
    clock: f32,
    trail: Vec<Vec2>,
    pops: Vec<f32>,
    particles: Particles,
    popups: Popups,
    shake: Shake,
    bump: Bump,
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
            combo: 0,
            events: vec![],
            size: Vec2::ZERO,
            since_break: 10.0,
            paddle_age: 10.0,
            clock: 0.0,
            trail: vec![],
            pops: vec![],
            particles: Particles::default(),
            popups: Popups::default(),
            shake: Shake::default(),
            bump: Bump::default(),
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
        self.pops = (0..ROWS * COLS)
            .map(|i| -((i % COLS + i / COLS) as f32) * 0.04)
            .collect();
        self.serve();
    }

    fn serve(&mut self) {
        self.state = State::Serve;
        self.combo = 0;
        self.trail.clear();
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
            self.events.push(Event::WallHit(self.ball));
        }
        if self.ball.x > self.size.x - BALL_R {
            self.ball.x = self.size.x - BALL_R;
            self.vel.x = -self.vel.x.abs();
            self.events.push(Event::WallHit(self.ball));
        }
        if self.ball.y < 70.0 + BALL_R {
            self.ball.y = 70.0 + BALL_R;
            self.vel.y = self.vel.y.abs();
            self.events.push(Event::WallHit(self.ball));
        }
        let p = self.paddle_rect();
        if self.vel.y > 0.0 && circle_hits_rect(self.ball, BALL_R, p) {
            let k = ((self.ball.x - self.paddle_x) / (PADDLE_W / 2.0)).clamp(-1.0, 1.0);
            let angle = k * 1.05;
            self.vel = vec2(angle.sin(), -angle.cos()) * self.speed();
            self.ball.y = p.y - BALL_R;
            self.combo = 0;
            self.events.push(Event::PaddleHit(self.ball));
        }
        for (i, b) in self.bricks.iter_mut().enumerate() {
            let Some(r) = *b else { continue };
            if !circle_hits_rect(self.ball, BALL_R, r) {
                continue;
            }
            *b = None;
            self.combo += 1;
            let points = combo_points(self.combo);
            self.score += points;
            let c = r.center();
            self.events.push(Event::BrickBroken {
                at: c,
                row: i / COLS,
                points,
                combo: self.combo,
            });
            let d = (self.ball - c) / vec2(r.w, r.h);
            if d.x.abs() > d.y.abs() {
                self.vel.x = self.vel.x.abs() * d.x.signum();
            } else {
                self.vel.y = self.vel.y.abs() * d.y.signum();
            }
            break;
        }
        if self.ball.y > self.size.y + BALL_R {
            self.events
                .push(Event::Lost(vec2(self.ball.x, self.size.y - 10.0)));
            self.lives -= 1;
            if self.lives == 0 {
                self.state = State::Lost;
            } else {
                self.serve();
            }
        } else if self.bricks.iter().all(|b| b.is_none()) {
            self.state = State::Cleared;
            self.events.push(Event::LevelCleared);
        }
    }

    fn react(&mut self, view: &View) {
        for e in std::mem::take(&mut self.events) {
            match e {
                Event::BrickBroken {
                    at,
                    row,
                    points,
                    combo,
                } => {
                    let col = brick_color(row);
                    self.particles.emit(
                        at,
                        &[col, darker(col, 0.8), WHITE],
                        Burst {
                            count: 14,
                            speed: (80.0, 320.0),
                            size: (3.0, 8.0),
                            shape: Shape::Square,
                            ..Default::default()
                        },
                    );
                    self.shake.add(0.12 + 0.03 * combo.min(6) as f32);
                    self.bump.kick();
                    self.since_break = 0.0;
                    if combo >= 2 {
                        self.popups.add(
                            at - vec2(0.0, 20.0),
                            format!("+{points}"),
                            hsv(0.12 - combo.min(8) as f32 * 0.015, 0.7, 1.0),
                            26.0 + combo.min(8) as f32 * 4.0,
                        );
                    }
                }
                Event::PaddleHit(at) => {
                    self.paddle_age = 0.0;
                    self.particles.puff(vec2(at.x, at.y + BALL_R), WHITE);
                }
                Event::WallHit(at) => {
                    self.particles.emit(
                        at,
                        &[Color::from_rgba(200, 220, 255, 255)],
                        Burst {
                            count: 4,
                            speed: (40.0, 120.0),
                            size: (2.0, 4.0),
                            ..Default::default()
                        },
                    );
                }
                Event::Lost(at) => {
                    self.shake.add(0.7);
                    self.particles.emit(
                        at,
                        &[WHITE, Color::from_rgba(255, 120, 120, 255)],
                        Burst {
                            count: 22,
                            angle: -std::f32::consts::FRAC_PI_2,
                            spread: 2.4,
                            ..Default::default()
                        },
                    );
                }
                Event::LevelCleared => {
                    self.shake.add(0.4);
                    self.particles.confetti(view);
                }
            }
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
        self.clock += dt;
        self.paddle_age += dt;
        self.since_break += dt;
        if self.since_break > COMBO_WINDOW && self.state == State::Playing {
            self.combo = 0;
        }
        for p in &mut self.pops {
            *p += dt;
        }
        self.particles.update(dt);
        self.popups.update(dt);
        self.shake.update(dt);
        self.bump.update(dt);
        if self.state != State::Playing {
            self.trail.clear();
        }
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
            State::Playing => {
                self.step(dt);
                self.trail.push(self.ball);
                if self.trail.len() > TRAIL_LEN {
                    self.trail.remove(0);
                }
            }
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
        self.react(view);
    }

    fn draw(&self, view: &View) {
        sky(
            view,
            Color::from_rgba(30, 30, 70, 255),
            Color::from_rgba(70, 40, 110, 255),
        );
        for k in 0..14 {
            let x = (k as f32 * 197.0 + 40.0) % view.w;
            let y = (k as f32 * 131.0 + 20.0) % view.h;
            let tw = 0.5 + 0.5 * (self.clock * 1.5 + k as f32).sin();
            star(
                vec2(x, y),
                3.0 + tw * 2.0,
                self.clock * 0.2,
                with_alpha(WHITE, 0.15 + 0.2 * tw),
            );
        }
        let off = self.shake.offset();
        set_view_camera(view, vec2(off.x, off.y));
        for (i, b) in self.bricks.iter().enumerate() {
            if let Some(r) = b {
                let age = self.pops[i];
                if age < 0.0 {
                    continue;
                }
                let k = ease_out_back(age / 0.35).max(0.01);
                let c = brick_color(i / COLS);
                let center = r.center();
                let r = Rect::new(
                    center.x - r.w * k / 2.0,
                    center.y - r.h * k / 2.0,
                    r.w * k,
                    r.h * k,
                );
                round_rect(Rect::new(r.x, r.y + 4.0, r.w, r.h), 7.0, darker(c, 0.55));
                round_rect(r, 7.0, c);
                round_rect(
                    Rect::new(r.x + 2.0, r.y + r.h * 0.55, r.w - 4.0, r.h * 0.45 - 2.0),
                    6.0,
                    darker(c, 0.88),
                );
                gloss(r, 7.0);
                shine_sweep(
                    r,
                    self.clock * 0.7 - (i % COLS) as f32 * 0.15 - (i / COLS) as f32 * 0.1,
                );
            }
        }
        for (i, p) in self.trail.iter().enumerate() {
            let k = (i + 1) as f32 / self.trail.len() as f32;
            draw_circle(
                p.x,
                p.y,
                BALL_R * (0.3 + 0.6 * k),
                Color::new(0.7, 0.85, 1.0, 0.25 * k),
            );
        }
        let p = self.paddle_rect();
        let sq = squash_scale(self.paddle_age);
        let pc = p.center();
        let p = Rect::new(
            pc.x - p.w * sq.x / 2.0,
            p.y + p.h - p.h * sq.y,
            p.w * sq.x,
            p.h * sq.y,
        );
        soft_shadow(vec2(pc.x, p.y + p.h + 10.0), p.w * 0.45, 7.0, 0.6);
        round_rect(
            Rect::new(p.x, p.y + 4.0, p.w, p.h),
            PADDLE_H / 2.0,
            Color::from_rgba(120, 130, 190, 255),
        );
        round_rect(p, PADDLE_H / 2.0, Color::from_rgba(240, 240, 255, 255));
        round_rect(
            Rect::new(p.x + 6.0, p.y + 4.0, p.w - 12.0, p.h * 0.3),
            3.0,
            Color::from_rgba(150, 200, 255, 255),
        );
        shine_sweep(p, self.clock);
        let glow = 0.12 + 0.05 * (self.clock * 6.0).sin();
        draw_circle(
            self.ball.x,
            self.ball.y,
            BALL_R * 2.0,
            Color::new(0.7, 0.85, 1.0, glow),
        );
        draw_circle(self.ball.x, self.ball.y, BALL_R, WHITE);
        draw_circle(
            self.ball.x - 3.0,
            self.ball.y - 3.0,
            BALL_R * 0.35,
            Color::from_rgba(200, 230, 255, 255),
        );
        self.particles.draw();
        self.popups.draw();
        set_view_camera(view, Vec2::ZERO);
        vignette(view, 0.6);
        let score = format!("Score {}", self.score);
        let sw = text_size(&score, 38.0).width;
        score_text(&score, HUD_LEFT + sw / 2.0, 40.0, 38.0, self.bump.t);
        text_outline(
            &format!("Level {}", self.level),
            view.w / 2.0,
            34.0,
            34.0,
            Color::from_rgba(200, 200, 255, 255),
            Color::from_rgba(40, 30, 60, 255),
        );
        if self.combo >= 2 && self.state == State::Playing {
            let pulse = 1.0 + 0.06 * (self.clock * 14.0).sin();
            text_outline(
                &format!("Combo x{}", self.combo),
                view.w / 2.0,
                70.0,
                28.0 * pulse,
                Color::from_rgba(255, 220, 90, 255),
                Color::from_rgba(40, 30, 60, 255),
            );
        }
        for i in 0..self.lives {
            let c = vec2(view.w - 30.0 - i as f32 * 34.0, 35.0);
            draw_circle(c.x, c.y + 2.0, 11.0, Color::from_rgba(40, 30, 60, 255));
            draw_circle(c.x, c.y, 11.0, WHITE);
            draw_circle(
                c.x - 3.0,
                c.y - 3.0,
                4.0,
                Color::from_rgba(200, 230, 255, 255),
            );
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

    #[test]
    fn consecutive_bricks_build_a_combo_until_the_paddle() {
        let mut b = playing();
        assert_eq!(combo_points(1), 10);
        assert_eq!(combo_points(3), 30);
        assert_eq!(combo_points(99), 80);
        let first = b.bricks[ROWS * COLS - 1].unwrap();
        b.ball = vec2(first.center().x, first.y + first.h + BALL_R + 5.0);
        b.vel = vec2(0.0, -500.0);
        b.step(0.05);
        assert_eq!(b.combo, 1);
        b.combo = 2;
        let second = b.bricks[ROWS * COLS - 2].unwrap();
        b.ball = vec2(second.center().x, second.y + second.h + BALL_R + 5.0);
        b.vel = vec2(0.0, -500.0);
        b.step(0.05);
        assert_eq!(b.combo, 3);
        assert_eq!(b.score, 10 + 30);
        b.ball = vec2(b.paddle_x, b.paddle_y() - BALL_R + 1.0);
        b.vel = vec2(0.0, 400.0);
        b.step(0.001);
        assert_eq!(b.combo, 0);
    }

    #[test]
    fn breaking_a_brick_emits_particles_and_a_combo_popup() {
        let mut b = playing();
        b.combo = 1;
        let brick = b.bricks[ROWS * COLS - 1].unwrap();
        b.ball = vec2(brick.center().x, brick.y + brick.h + BALL_R + 5.0);
        b.vel = vec2(0.0, -500.0);
        b.step(0.05);
        assert!(matches!(
            b.events.first(),
            Some(Event::BrickBroken {
                combo: 2,
                points: 20,
                ..
            })
        ));
        b.react(&View::fit(MIN_W, MIN_H));
        assert!(!b.particles.list.is_empty());
        assert_eq!(b.popups.list.len(), 1);
        assert!(b.shake.active());
        assert!(b.events.is_empty());
    }

    #[test]
    fn paddle_hit_squashes_then_recovers() {
        let s = squash_scale(0.0);
        assert!(s.x > 1.0 && s.y < 1.0);
        assert_eq!(squash_scale(0.5), vec2(1.0, 1.0));
        let mut b = playing();
        b.ball = vec2(b.paddle_x, b.paddle_y() - BALL_R + 1.0);
        b.vel = vec2(0.0, 400.0);
        b.step(0.001);
        assert!(matches!(b.events.last(), Some(Event::PaddleHit(_))));
        b.react(&View::fit(MIN_W, MIN_H));
        assert_eq!(b.paddle_age, 0.0);
    }

    #[test]
    fn clearing_the_last_brick_throws_confetti() {
        let mut b = playing();
        for slot in b.bricks.iter_mut().skip(1) {
            *slot = None;
        }
        let last = b.bricks[0].unwrap();
        b.ball = vec2(last.center().x, last.y + last.h + BALL_R + 5.0);
        b.vel = vec2(0.0, -500.0);
        b.step(0.05);
        assert_eq!(b.state, State::Cleared);
        assert!(b.events.contains(&Event::LevelCleared));
        b.react(&View::fit(MIN_W, MIN_H));
        assert!(b.particles.list.len() > 50);
    }

    #[test]
    fn trail_follows_the_ball_and_is_bounded() {
        let mut b = Breakout::new();
        b.state = State::Playing;
        b.vel = vec2(0.0, -1.0);
        let input = Input::default();
        let view = View::fit(MIN_W, MIN_H);
        for _ in 0..40 {
            b.update(&input, &view, 1.0 / 60.0);
        }
        assert_eq!(b.trail.len(), TRAIL_LEN);
        b.state = State::Lost;
        b.update(&input, &view, 1.0 / 60.0);
        assert!(b.trail.is_empty());
    }
}
