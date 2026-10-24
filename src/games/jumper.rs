use crate::engine::draw_circle;
use crate::engine::draw_ellipse;
use crate::engine::*;
use crate::fx::*;
use crate::sfx::{self, Sfx};
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

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Landing {
    pub pos: Vec2,
    pub kind: Kind,
}

#[derive(Clone, Copy, Debug)]
pub struct Crumb {
    pub pos: Vec2,
    pub vel: Vec2,
    pub angle: f32,
    pub spin: f32,
    pub size: Vec2,
    pub age: f32,
}

pub const CRUMB_LIFE: f32 = 1.2;

pub fn squash_stretch(squish: f32, vy: f32) -> (f32, f32) {
    let stretch = (vy.abs() / 1400.0).min(1.0) * 0.2;
    let s = squish.clamp(0.0, 1.0) * 0.3 - stretch;
    (1.0 + s, 1.0 - s)
}

pub fn parallax_y(base: f32, cam: f32, factor: f32, span: f32) -> f32 {
    (base - cam * factor).rem_euclid(span) - span * 0.15
}

pub fn spring_extension(age: f32) -> f32 {
    if age >= 0.7 {
        1.0
    } else {
        0.35 + 0.65 * ease_out_elastic(age / 0.7)
    }
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
    pub landings: Vec<Landing>,
    face: f32,
    squish: f32,
    boings: Vec<(Vec2, f32)>,
    crumbs: Vec<Crumb>,
    particles: Particles,
    popups: Popups,
    shake: Shake,
    score_bump: Bump,
    milestone: u32,
    prev_cam: f32,
    t: f32,
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
            landings: vec![],
            face: 1.0,
            squish: 0.0,
            boings: vec![],
            crumbs: vec![],
            particles: Particles::default(),
            popups: Popups::default(),
            shake: Shake::default(),
            score_bump: Bump::default(),
            milestone: 0,
            prev_cam: 0.0,
            t: 0.0,
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
        self.milestone = 0;
        self.prev_cam = 0.0;
        self.landings.clear();
        self.boings.clear();
        self.crumbs.clear();
        self.particles.list.clear();
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
                        self.landings.push(Landing {
                            pos: vec2(p.x + PLAT_W / 2.0, p.y),
                            kind: Kind::Crumble,
                        });
                        continue;
                    }
                    self.landings.push(Landing {
                        pos: vec2(self.pos.x, p.y),
                        kind: p.kind,
                    });
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

const PLAT_GREEN: Color = Color::new(0.43, 0.78, 0.31, 1.0);

impl Jumper {
    fn react(&mut self, view: &View) {
        for l in std::mem::take(&mut self.landings) {
            let at = vec2(l.pos.x, l.pos.y - self.cam);
            match l.kind {
                Kind::Crumble => {
                    sfx::play(Sfx::Crunch);
                    self.crumble(l.pos)
                }
                Kind::Spring => {
                    sfx::play(Sfx::LevelUp);
                    self.boings.push((l.pos, 0.0));
                    self.shake.add(0.2);
                    self.popups.add(
                        at - vec2(0.0, 60.0),
                        "Boing!",
                        Color::from_rgba(255, 230, 90, 255),
                        38.0,
                    );
                    self.particles
                        .sparkle(at - vec2(0.0, 24.0), Color::from_rgba(255, 90, 90, 255));
                    self.dust(at);
                }
                _ => {
                    sfx::play(Sfx::Jump);
                    self.dust(at)
                }
            }
        }
        let steps = self.score / 100;
        if steps > self.milestone {
            self.milestone = steps;
            sfx::play(Sfx::Score);
            self.score_bump.kick();
            self.popups.add(
                vec2(view.w / 2.0, 120.0),
                format!("{}!", steps * 100),
                Color::from_rgba(255, 220, 80, 255),
                54.0,
            );
            self.particles.sparkle(
                vec2(HUD_LEFT + 50.0, 50.0),
                Color::from_rgba(255, 220, 80, 255),
            );
        }
    }

    fn dust(&mut self, at: Vec2) {
        for dir in [-1.0f32, 1.0] {
            self.particles.emit(
                at + vec2(dir * 14.0, -2.0),
                &[Color::from_rgba(235, 235, 245, 255)],
                Burst {
                    count: 5,
                    speed: (60.0, 170.0),
                    size: (5.0, 10.0),
                    life: (0.25, 0.5),
                    gravity: -20.0,
                    drag: 4.0,
                    spread: 0.7,
                    angle: if dir < 0.0 { std::f32::consts::PI } else { 0.0 },
                    ..Default::default()
                },
            );
        }
    }

    fn crumble(&mut self, world: Vec2) {
        for k in 0..5 {
            let f = k as f32 / 4.0 - 0.5;
            self.crumbs.push(Crumb {
                pos: vec2(world.x + f * PLAT_W * 0.8, world.y + PLAT_H / 2.0),
                vel: vec2(
                    f * 160.0 + rand::gen_range(-40.0, 40.0),
                    rand::gen_range(-260.0, -80.0),
                ),
                angle: 0.0,
                spin: rand::gen_range(-6.0, 6.0),
                size: vec2(PLAT_W / 5.0 + 2.0, PLAT_H * rand::gen_range(0.7, 1.1)),
                age: 0.0,
            });
        }
        let at = vec2(world.x, world.y - self.cam);
        self.particles.emit(
            at,
            &[
                Color::from_rgba(170, 120, 70, 255),
                Color::from_rgba(120, 85, 50, 255),
            ],
            Burst {
                count: 10,
                speed: (60.0, 220.0),
                size: (3.0, 6.0),
                life: (0.4, 0.8),
                shape: Shape::Square,
                ..Default::default()
            },
        );
    }

    fn animate(&mut self, dt: f32) {
        for b in &mut self.boings {
            b.1 += dt;
        }
        self.boings.retain(|b| b.1 < 0.8);
        for c in &mut self.crumbs {
            c.vel.y += 1400.0 * dt;
            c.pos += c.vel * dt;
            c.angle += c.spin * dt;
            c.age += dt;
        }
        self.crumbs.retain(|c| c.age < CRUMB_LIFE);
        let shift = self.prev_cam - self.cam;
        for p in &mut self.particles.list {
            p.pos.y += shift;
        }
        self.prev_cam = self.cam;
        self.particles.update(dt);
        self.popups.update(dt);
        self.shake.update(dt);
        self.score_bump.update(dt);
    }

    fn boing_age(&self, p: &Plat) -> Option<f32> {
        self.boings
            .iter()
            .find(|(w, _)| (w.y - p.y).abs() < 1.0 && (w.x - (p.x + PLAT_W / 2.0)).abs() < PLAT_W)
            .map(|b| b.1)
    }

    fn background(&self, view: &View) {
        for k in 0..7 {
            let f = k as f32;
            let span = view.h + 240.0;
            let x = (f * 211.0) % (view.w + 120.0) - 60.0;
            let y = parallax_y(f * 157.0, self.cam, 0.12, span);
            let s = 0.8 + (f * 0.37) % 0.9;
            cloud(
                x + (self.t * (4.0 + f)) % 40.0,
                y,
                s,
                Color::new(0.82, 0.9, 1.0, 0.6),
            );
        }
        let ink = Color::from_rgba(150, 160, 210, 120);
        for k in 0..9 {
            let f = k as f32;
            let span = view.h + 160.0;
            let x = (f * 263.0 + 40.0) % (view.w - 80.0) + 40.0;
            let y = parallax_y(f * 131.0, self.cam, 0.3, span);
            match k % 3 {
                0 => star(vec2(x, y), 16.0, self.t * 0.3 + f, ink),
                1 => {
                    draw_circle_lines(x, y, 14.0, 3.0, ink);
                    draw_circle_lines(x, y, 6.0, 2.0, ink);
                }
                _ => {
                    for z in 0..4 {
                        let a = vec2(x + z as f32 * 14.0, y + if z % 2 == 0 { 0.0 } else { 12.0 });
                        let b = vec2(a.x + 14.0, y + if z % 2 == 0 { 12.0 } else { 0.0 });
                        draw_line(a.x, a.y, b.x, b.y, 3.0, ink);
                    }
                }
            }
        }
    }

    fn draw_plat(&self, p: &Plat, y: f32) {
        let c = match p.kind {
            Kind::Solid | Kind::Spring => PLAT_GREEN,
            Kind::Moving(_) => Color::from_rgba(90, 160, 240, 255),
            Kind::Crumble => Color::from_rgba(170, 120, 70, 255),
        };
        let r = Rect::new(p.x, y, PLAT_W, PLAT_H);
        soft_shadow(
            vec2(p.x + PLAT_W / 2.0, y + PLAT_H + 8.0),
            PLAT_W * 0.45,
            6.0,
            0.5,
        );
        round_rect(
            Rect::new(p.x, y + 4.0, PLAT_W, PLAT_H),
            10.0,
            darker(c, 0.7),
        );
        round_rect(r, 10.0, c);
        gloss(r, 10.0);
        match p.kind {
            Kind::Solid | Kind::Spring => {
                for k in 0..4 {
                    let gx = p.x + 18.0 + k as f32 * 28.0;
                    draw_triangle(
                        vec2(gx - 4.0, y + 2.0),
                        vec2(gx + 4.0, y + 2.0),
                        vec2(gx, y - 6.0),
                        darker(c, 0.85),
                    );
                }
            }
            Kind::Moving(v) => {
                for k in 0..3 {
                    let ax = p.x + 30.0 + k as f32 * 30.0;
                    let d = v.signum() * 6.0;
                    draw_triangle(
                        vec2(ax - d, y + 5.0),
                        vec2(ax - d, y + PLAT_H - 5.0),
                        vec2(ax + d, y + PLAT_H / 2.0),
                        with_alpha(WHITE, 0.55),
                    );
                }
            }
            Kind::Crumble => {
                draw_line(p.x + 40.0, y, p.x + 50.0, y + PLAT_H, 3.0, darker(c, 0.6));
                draw_line(p.x + 80.0, y, p.x + 72.0, y + PLAT_H, 3.0, darker(c, 0.6));
                draw_line(
                    p.x + 50.0,
                    y + 8.0,
                    p.x + 60.0,
                    y + 4.0,
                    2.0,
                    darker(c, 0.6),
                );
            }
        }
        if p.kind == Kind::Spring {
            let ext = self.boing_age(p).map_or(1.0, spring_extension);
            let sx = p.x + PLAT_W / 2.0 - 14.0;
            let height = 30.0 * ext;
            for k in 0..3 {
                let yy = y - 6.0 - (k as f32 / 2.0) * (height - 12.0);
                draw_line(sx, yy, sx + 28.0, yy + 4.0, 4.0, GRAY);
                draw_line(
                    sx + 28.0,
                    yy + 4.0,
                    sx,
                    yy - 4.0 - height * 0.1,
                    3.0,
                    LIGHTGRAY,
                );
            }
            let cap = Rect::new(sx - 4.0, y - 6.0 - height, 36.0, 8.0);
            round_rect(cap, 4.0, Color::from_rgba(240, 80, 80, 255));
            gloss(cap, 4.0);
        }
    }

    fn draw_hero(&self, view: &View, c: Vec2) {
        let (sx, sy) = squash_stretch(self.squish, self.vel.y);
        let anchor = c.y + HERO_R;
        for dx in [0.0, -view.w, view.w] {
            let cx = c.x + dx;
            if cx < -HERO_R * 2.0 || cx > view.w + HERO_R * 2.0 {
                continue;
            }
            let cy = anchor - HERO_R * sy;
            let body = Color::from_rgba(250, 200, 50, 255);
            draw_ellipse(cx, cy, HERO_R * sx, HERO_R * sy, 0.0, darker(body, 0.85));
            draw_ellipse(
                cx,
                cy - 1.5,
                HERO_R * sx * 0.96,
                HERO_R * sy * 0.96,
                0.0,
                body,
            );
            draw_ellipse(
                cx - HERO_R * sx * 0.3,
                cy - HERO_R * sy * 0.45,
                HERO_R * sx * 0.35,
                HERO_R * sy * 0.2,
                -0.5,
                with_alpha(WHITE, 0.35),
            );
            draw_rectangle(
                cx - HERO_R * 0.8 * sx,
                cy + HERO_R * 0.4 * sy,
                HERO_R * 1.6 * sx,
                HERO_R * 0.35 * sy,
                Color::from_rgba(90, 160, 80, 255),
            );
            for s in [-1.0f32, 1.0] {
                let e = vec2(cx + self.face * 8.0 * sx + s * 9.0 * sx, cy - 6.0 * sy);
                draw_circle(e.x, e.y, 7.0, WHITE);
                draw_circle(
                    e.x + self.face * 2.5,
                    e.y + if self.vel.y > 0.0 { 2.0 } else { -2.0 },
                    3.5,
                    BLACK,
                );
            }
            draw_rectangle(
                cx + self.face * HERO_R * 0.7 * sx - 6.0,
                cy + 2.0 * sy,
                12.0 + self.face * 6.0,
                8.0,
                Color::from_rgba(240, 150, 40, 255),
            );
        }
    }
}

impl Game for Jumper {
    fn update(&mut self, input: &Input, view: &View, dt: f32) {
        self.t += dt;
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
                    sfx::play(Sfx::Whoosh);
                    self.shake.add(0.6);
                }
            }
            State::Fallen => {
                if input.tap() {
                    self.reset(view);
                    self.state = State::Playing;
                }
            }
        }
        self.react(view);
        self.animate(dt);
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
        self.background(view);
        let sh = self.shake.offset();
        set_view_camera(view, vec2(sh.x, sh.y));
        for p in &self.plats {
            if !p.broken {
                self.draw_plat(p, p.y - self.cam);
            }
        }
        for c in &self.crumbs {
            let fade = 1.0 - (c.age / CRUMB_LIFE).powi(2);
            let at = vec2(c.pos.x, c.pos.y - self.cam);
            draw_rectangle_ex(
                at.x,
                at.y,
                c.size.x,
                c.size.y,
                DrawRectangleParams {
                    rotation: c.angle,
                    offset: vec2(0.5, 0.5),
                    color: Color::new(0.67, 0.47, 0.27, fade),
                },
            );
        }
        let c = vec2(self.pos.x, self.pos.y - self.cam);
        if self.state != State::Fallen {
            let ground = self
                .plats
                .iter()
                .filter(|p| {
                    !p.broken && p.y - self.cam > c.y && p.x - 4.0 < c.x && c.x < p.x + PLAT_W + 4.0
                })
                .map(|p| p.y - self.cam)
                .fold(f32::MAX, f32::min);
            if ground < f32::MAX {
                let near = 1.0 - ((ground - c.y) / 260.0).clamp(0.0, 1.0);
                soft_shadow(
                    vec2(c.x, ground + 2.0),
                    HERO_R * (0.5 + 0.5 * near),
                    5.0,
                    0.5 * near,
                );
            }
        }
        self.draw_hero(view, c);
        self.particles.draw();
        self.popups.draw();
        set_view_camera(view, Vec2::ZERO);
        score_text(
            &self.score.to_string(),
            HUD_LEFT + 50.0,
            50.0,
            50.0,
            self.score_bump.t,
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
    fn landing_is_reported_with_its_platform_kind() {
        let (mut j, v) = on_platform(Kind::Spring);
        j.physics(0.0, &v, 1.0 / 30.0);
        assert_eq!(j.landings.len(), 1);
        assert_eq!(j.landings[0].kind, Kind::Spring);
        assert_eq!(j.landings[0].pos.y, 500.0);
    }

    #[test]
    fn crumbling_drops_pieces_and_dust() {
        let (mut j, v) = on_platform(Kind::Crumble);
        j.state = State::Playing;
        j.physics(0.0, &v, 1.0 / 30.0);
        assert_eq!(j.landings[0].kind, Kind::Crumble);
        j.react(&v);
        assert_eq!(j.crumbs.len(), 5);
        assert!(!j.particles.list.is_empty());
        for _ in 0..80 {
            j.animate(1.0 / 60.0);
        }
        assert!(j.crumbs.is_empty());
    }

    #[test]
    fn spring_landing_boings_with_a_popup() {
        let (mut j, v) = on_platform(Kind::Spring);
        j.physics(0.0, &v, 1.0 / 30.0);
        j.react(&v);
        assert_eq!(j.boings.len(), 1);
        assert_eq!(j.popups.list.len(), 1);
        assert!(j.shake.active());
        assert!(j.boing_age(&j.plats[0]).is_some());
    }

    #[test]
    fn solid_landing_puffs_dust_each_side() {
        let (mut j, v) = on_platform(Kind::Solid);
        j.physics(0.0, &v, 1.0 / 30.0);
        j.react(&v);
        assert_eq!(j.particles.list.len(), 10);
        assert!(j.boings.is_empty());
    }

    #[test]
    fn squash_on_landing_and_stretch_in_flight() {
        let (sx, sy) = squash_stretch(1.0, 0.0);
        assert!(sx > 1.0 && sy < 1.0);
        let (sx, sy) = squash_stretch(0.0, JUMP);
        assert!(sx < 1.0 && sy > 1.0);
        assert_eq!(squash_stretch(0.0, 0.0), (1.0, 1.0));
    }

    #[test]
    fn spring_compresses_then_overshoots() {
        assert!((spring_extension(0.0) - 0.35).abs() < 1e-4);
        assert!((0..70).any(|k| spring_extension(k as f32 / 100.0) > 1.02));
        assert_eq!(spring_extension(2.0), 1.0);
    }

    #[test]
    fn parallax_layers_scroll_slower_and_wrap_on_screen() {
        for cam in [0.0, -123.0, -9000.0] {
            let y = parallax_y(40.0, cam, 0.3, 900.0);
            assert!((-135.0..765.0).contains(&y));
        }
        let near = parallax_y(0.0, -100.0, 0.3, 5000.0) - parallax_y(0.0, 0.0, 0.3, 5000.0);
        let far = parallax_y(0.0, -100.0, 0.1, 5000.0) - parallax_y(0.0, 0.0, 0.1, 5000.0);
        assert!(near > far && far > 0.0);
    }

    #[test]
    fn particles_follow_the_camera() {
        let mut j = Jumper::new();
        j.particles.emit(
            vec2(100.0, 100.0),
            &[WHITE],
            Burst {
                count: 1,
                speed: (0.0, 0.0),
                gravity: 0.0,
                life: (5.0, 5.0),
                ..Default::default()
            },
        );
        j.cam = -50.0;
        j.animate(0.0);
        assert!((j.particles.list[0].pos.y - 150.0).abs() < 0.01);
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
