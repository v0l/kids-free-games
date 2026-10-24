use crate::engine::*;
use macroquad::prelude::*;

pub const GRAVITY: f32 = 1300.0;
pub const JUMP: f32 = -820.0;
pub const SPRING: f32 = -1350.0;
pub const PLAT_W: f32 = 120.0;
pub const PLAT_H: f32 = 22.0;
pub const HERO_R: f32 = 26.0;
pub const MOVE: f32 = 520.0;

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

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum State {
    Ready,
    Playing,
    Fallen,
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
    face: f32,
    squish: f32,
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
            face: 1.0,
            squish: 0.0,
        };
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
        self.fill(view);
    }

    fn gap(&self) -> f32 {
        (90.0 + self.score as f32 * 0.02).min(190.0)
    }

    fn fill(&mut self, view: &View) {
        while self.top > self.cam - 200.0 {
            let y = self.top - rand::gen_range(self.gap() * 0.7, self.gap());
            let x = rand::gen_range(10.0, view.w - PLAT_W - 10.0);
            let hard = (self.score as f32 / 5000.0).min(1.0);
            let roll = rand::gen_range(0.0, 1.0);
            let kind = if roll < 0.06 {
                Kind::Spring
            } else if roll < 0.06 + 0.25 * hard {
                Kind::Moving(if rand::gen_range(0, 2) == 0 {
                    120.0
                } else {
                    -120.0
                })
            } else if roll < 0.06 + 0.4 * hard {
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
                if over && old_feet <= p.y + 2.0 && feet >= p.y {
                    if p.kind == Kind::Crumble {
                        p.broken = true;
                        continue;
                    }
                    self.pos.y = p.y - HERO_R;
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

impl Game for Jumper {
    fn update(&mut self, input: &Input, view: &View, dt: f32) {
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
                }
            }
            State::Playing => {
                self.physics(steer, view, dt);
                self.fill(view);
                if self.pos.y - HERO_R > self.cam + view.h {
                    self.state = State::Fallen;
                    self.best.offer(self.score);
                }
            }
            State::Fallen => {
                if input.tap() {
                    self.reset(view);
                    self.state = State::Playing;
                }
            }
        }
    }

    fn draw(&self, view: &View) {
        clear_background(Color::from_rgba(250, 247, 235, 255));
        let grid = 36.0;
        let off = (-self.cam).rem_euclid(grid);
        for i in 0..((view.h / grid) as i32 + 2) {
            let y = i as f32 * grid + off - grid;
            draw_line(0.0, y, view.w, y, 1.0, Color::from_rgba(220, 225, 240, 255));
        }
        for i in 0..((view.w / grid) as i32 + 1) {
            draw_line(
                i as f32 * grid,
                0.0,
                i as f32 * grid,
                view.h,
                1.0,
                Color::from_rgba(230, 232, 245, 255),
            );
        }
        for p in &self.plats {
            let y = p.y - self.cam;
            let c = match p.kind {
                Kind::Solid => Color::from_rgba(110, 200, 80, 255),
                Kind::Moving(_) => Color::from_rgba(90, 160, 240, 255),
                Kind::Crumble => Color::from_rgba(170, 120, 70, 255),
                Kind::Spring => Color::from_rgba(110, 200, 80, 255),
            };
            round_rect(
                Rect::new(p.x, y + 3.0, PLAT_W, PLAT_H),
                10.0,
                darker(c, 0.7),
            );
            round_rect(Rect::new(p.x, y, PLAT_W, PLAT_H), 10.0, c);
            if p.kind == Kind::Crumble {
                draw_line(p.x + 40.0, y, p.x + 50.0, y + PLAT_H, 3.0, darker(c, 0.6));
                draw_line(p.x + 80.0, y, p.x + 72.0, y + PLAT_H, 3.0, darker(c, 0.6));
            }
            if p.kind == Kind::Spring {
                let sx = p.x + PLAT_W / 2.0 - 14.0;
                for k in 0..3 {
                    draw_line(
                        sx,
                        y - 6.0 - k as f32 * 8.0,
                        sx + 28.0,
                        y - 2.0 - k as f32 * 8.0,
                        4.0,
                        GRAY,
                    );
                }
                draw_rectangle(
                    sx - 4.0,
                    y - 30.0,
                    36.0,
                    6.0,
                    Color::from_rgba(240, 80, 80, 255),
                );
            }
        }
        let c = vec2(self.pos.x, self.pos.y - self.cam);
        let sq = self.squish * 0.25;
        for dx in [0.0, -view.w, view.w] {
            let cx = c.x + dx;
            if cx < -HERO_R * 2.0 || cx > view.w + HERO_R * 2.0 {
                continue;
            }
            let body = Color::from_rgba(250, 200, 50, 255);
            draw_ellipse(cx, c.y, HERO_R * (1.0 + sq), HERO_R * (1.0 - sq), 0.0, body);
            draw_rectangle(
                cx - HERO_R * 0.8,
                c.y + HERO_R * 0.4,
                HERO_R * 1.6,
                HERO_R * 0.35,
                Color::from_rgba(90, 160, 80, 255),
            );
            for s in [-1.0f32, 1.0] {
                let e = vec2(cx + self.face * 8.0 + s * 9.0, c.y - 6.0);
                draw_circle(e.x, e.y, 7.0, WHITE);
                draw_circle(
                    e.x + self.face * 2.5,
                    e.y + if self.vel.y > 0.0 { 2.0 } else { -2.0 },
                    3.5,
                    BLACK,
                );
            }
            draw_rectangle(
                cx + self.face * HERO_R * 0.7 - 6.0,
                c.y + 2.0,
                12.0 + self.face * 6.0,
                8.0,
                Color::from_rgba(240, 150, 40, 255),
            );
        }
        text_shadow(
            &self.score.to_string(),
            HUD_LEFT + 50.0,
            50.0,
            50.0,
            Color::from_rgba(80, 80, 120, 255),
        );
        match self.state {
            State::Ready => overlay(
                view,
                "Sky Jumper",
                &[
                    "Touch left or right side to steer",
                    "Jump as high as you can",
                    "Tap to start",
                ],
            ),
            State::Fallen => {
                let s = format!("Height {}   Best {}", self.score, self.best.0);
                overlay(view, "Whoops!", &[&s, "Tap to jump again"]);
            }
            State::Playing => {}
        }
    }
}

pub fn icon(r: Rect) {
    draw_rectangle(r.x, r.y, r.w, r.h, Color::from_rgba(250, 247, 235, 255));
    for (i, (px, py)) in [(0.1, 0.8), (0.5, 0.55), (0.2, 0.3)].iter().enumerate() {
        draw_rectangle(
            r.x + r.w * px,
            r.y + r.h * py,
            r.w * 0.35,
            r.h * 0.07,
            if i == 1 {
                Color::from_rgba(90, 160, 240, 255)
            } else {
                Color::from_rgba(110, 200, 80, 255)
            },
        );
    }
    draw_circle(
        r.x + r.w * 0.65,
        r.y + r.h * 0.35,
        r.w * 0.14,
        Color::from_rgba(250, 200, 50, 255),
    );
    draw_circle(r.x + r.w * 0.69, r.y + r.h * 0.32, r.w * 0.04, WHITE);
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
        j.pos = vec2(460.0, 500.0 - HERO_R - 5.0);
        j.vel = vec2(0.0, 400.0);
        (j, v)
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
}
