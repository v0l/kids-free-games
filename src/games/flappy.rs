use crate::engine::*;
use crate::feel::{self, Impact};
use crate::fx::*;
use crate::sfx::{self, Sfx};
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

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Spike {
    pub cx: f32,
    pub base_y: f32,
    pub tip_y: f32,
    pub half_w: f32,
}

impl Spike {
    pub fn new(cx: f32, base_y: f32, tip_y: f32) -> Self {
        let h = (tip_y - base_y).abs();
        Self {
            cx,
            base_y,
            tip_y,
            half_w: (h * 0.24).clamp(PIPE_W * 0.5, PIPE_W * 0.95),
        }
    }

    pub fn corners(&self) -> [Vec2; 3] {
        [
            vec2(self.cx, self.tip_y),
            vec2(self.cx - self.half_w, self.base_y),
            vec2(self.cx + self.half_w, self.base_y),
        ]
    }

    pub fn hits(&self, c: Vec2, r: f32) -> bool {
        let [a, b, d] = self.corners();
        point_in_triangle(c, a, b, d)
            || [(a, b), (b, d), (d, a)]
                .iter()
                .any(|(p, q)| segment_distance(c, *p, *q) < r)
    }
}

fn segment_distance(c: Vec2, a: Vec2, b: Vec2) -> f32 {
    let ab = b - a;
    let t = ((c - a).dot(ab) / ab.length_squared().max(1e-6)).clamp(0.0, 1.0);
    (a + ab * t).distance(c)
}

fn point_in_triangle(p: Vec2, a: Vec2, b: Vec2, c: Vec2) -> bool {
    let s = |p: Vec2, q: Vec2, r: Vec2| (p.x - r.x) * (q.y - r.y) - (q.x - r.x) * (p.y - r.y);
    let (d1, d2, d3) = (s(p, a, b), s(p, b, c), s(p, c, a));
    let neg = d1 < 0.0 || d2 < 0.0 || d3 < 0.0;
    let pos = d1 > 0.0 || d2 > 0.0 || d3 > 0.0;
    !(neg && pos)
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

    pub fn spikes(p: &Pipe, view: &View) -> [Spike; 2] {
        let cx = p.x + PIPE_W / 2.0;
        let top = p.gap_y - GAP / 2.0;
        let bottom = p.gap_y + GAP / 2.0;
        let roof = -view.inset.top - 20.0;
        let floor = view.h - GROUND + 30.0;
        [Spike::new(cx, roof, top), Spike::new(cx, floor, bottom)]
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
        sfx::play(Sfx::Flap);
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
        sfx::play(Sfx::Hit);
        sfx::play(Sfx::Lose);
        if self.score > self.best.0 && self.best.0 > 0 {
            sfx::say(sfx::Say::Best);
        }
        feel::impact(Impact::Large);
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
                    sfx::say(sfx::Say::Go);
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
                    if Self::spikes(p, view)
                        .iter()
                        .any(|sp| sp.hits(bird, BIRD_R - 6.0))
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
                    sfx::play_step(Sfx::Score, self.score as usize % 8);
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
            fill_screen(view, with_alpha(WHITE, self.flash * 0.7));
        }
        if self.state != State::Ready {
            big_score(view, &self.score.to_string(), 80.0, self.bump.t);
        }
        chip(view, Side::Right, "BEST", &self.best.0.to_string(), 1.0);
        match self.state {
            State::Ready => {
                let bob = (self.t * 4.0).sin() * 5.0;
                let w = (view.w * 0.5).min(420.0);
                art::sprite_w(
                    "get_ready",
                    vec2(view.w / 2.0, view.h * 0.2 + bob),
                    w,
                    Default::default(),
                );
                let bx = Self::bird_x(view);
                let tap = vec2(bx, view.h * 0.42 + 110.0);
                let press = ((self.t * 2.0).fract() < 0.15) as i32 as f32;
                art::sprite_at(
                    "tap",
                    tap + vec2(0.0, press * 6.0),
                    90.0,
                    Default::default(),
                );
                art::sprite_at(
                    "tap_left",
                    vec2(bx + 110.0, view.h * 0.42),
                    46.0,
                    Default::default(),
                );
                art::sprite_at(
                    "tap_right",
                    vec2(bx - 110.0, view.h * 0.42),
                    46.0,
                    Default::default(),
                );
            }
            State::Dead if self.dead_t > 0.4 => {
                let age = self.dead_t - 0.4;
                let k = crate::fx::ease_out_back((age / 0.35).min(1.0));
                let w = (view.w * 0.55).min(440.0) * k.max(0.01);
                art::sprite_w(
                    "game_over",
                    vec2(view.w / 2.0, view.h * 0.18),
                    w,
                    Default::default(),
                );
                let s = format!("Score {}   Best {}", self.score, self.best.0);
                overlay(view, "Bonk!", &[&s, "Tap to fly again"]);
                if let Some(m) = crate::ui::medal_for(self.score, self.best.0) {
                    let mk = crate::fx::ease_out_back(((age - 0.35) / 0.3).clamp(0.0, 1.0));
                    if mk > 0.0 {
                        crate::ui::sprite_at(
                            m,
                            view.center() + vec2(-220.0, -40.0),
                            110.0 * mk,
                            crate::art::Draw {
                                rotation: (1.0 - mk) * 2.0 - 0.2,
                                ..Default::default()
                            },
                        );
                    }
                }
            }
            _ => {}
        }
    }
}

mod art {
    crate::sprites!("flappy");
}

impl Flappy {
    fn draw_world(&self, view: &View) {
        let f = view.full();
        let gy = view.h - GROUND;
        let bg = art::size("bg");
        let bh = f.h;
        let bw = bh * bg.x / bg.y;
        let off = (self.scroll * 0.25).rem_euclid(bw);
        let mut x = f.x - off;
        while x < f.x + f.w {
            art::sprite("bg", Rect::new(x, f.y, bw + 1.0, bh));
            x += bw;
        }
        for p in &self.pipes {
            Self::draw_pipe(p, view);
        }
        let g = art::size("ground");
        let gh = GROUND + 30.0 + view.inset.bottom;
        let gw = g.x * (GROUND + 30.0) / g.y;
        let off = self.scroll.rem_euclid(gw);
        let mut x = f.x - off;
        while x < f.x + f.w {
            art::sprite("ground", Rect::new(x, gy - 30.0, gw + 1.0, gh));
            x += gw;
        }
        self.draw_bird(view);
    }

    fn draw_pipe(p: &Pipe, view: &View) {
        for (sp, name) in Self::spikes(p, view).iter().zip(["rock_down", "rock"]) {
            let top = sp.base_y.min(sp.tip_y);
            let h = (sp.tip_y - sp.base_y).abs();
            let w = sp.half_w * 2.0 / 0.94;
            art::sprite(name, Rect::new(sp.cx - w * 0.6, top, w, h));
        }
    }

    fn draw_bird(&self, view: &View) {
        let bx = Self::bird_x(view);
        let angle = (self.vy / 900.0).clamp(-0.5, 1.2);
        let sc = bird_scale(self.vy, self.kick);
        let ground_y = view.h - GROUND;
        let height = (ground_y - self.y).max(0.0);
        soft_shadow(
            vec2(bx, ground_y + 6.0),
            BIRD_R * (1.4 - (height / 600.0).min(0.7)),
            5.0,
            0.6,
        );
        let frame = if self.state == State::Dead {
            "plane1"
        } else {
            ["plane1", "plane2", "plane3", "plane2"][(self.t * 14.0) as usize % 4]
        };
        let h = BIRD_R * 2.6;
        let w = h * art::size(frame).x / art::size(frame).y;
        art::sprite_ex(
            frame,
            Rect::new(
                bx - w * sc.x / 2.0,
                self.y - h * sc.y / 2.0,
                w * sc.x,
                h * sc.y,
            ),
            crate::art::Draw {
                rotation: angle,
                ..Default::default()
            },
        );
    }
}

pub fn icon(r: Rect) {
    art::sprite(
        "bg",
        Rect::new(r.x - r.w * 0.3, r.y, r.h * 800.0 / 480.0, r.h),
    );
    art::sprite(
        "rock",
        Rect::new(r.x + r.w * 0.62, r.y + r.h * 0.45, r.w * 0.3, r.h * 0.6),
    );
    art::sprite(
        "rock_down",
        Rect::new(r.x + r.w * 0.62, r.y - r.h * 0.05, r.w * 0.3, r.h * 0.35),
    );
    art::sprite_at(
        "plane1",
        vec2(r.x + r.w * 0.36, r.y + r.h * 0.5),
        r.h * 0.34,
        Default::default(),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flappy_sprites_all_exist() {
        let have = art::names();
        for n in [
            "bg",
            "ground",
            "rock",
            "rock_down",
            "plane1",
            "plane2",
            "plane3",
            "tap",
            "get_ready",
            "game_over",
        ] {
            assert!(have.iter().any(|h| h == n), "missing {n}");
        }
    }

    #[test]
    fn flapping_and_crashing_make_sounds() {
        let v = View::ipad_landscape();
        let mut g = Flappy::new();
        crate::sfx::take_pending();
        g.update(&Input::press_at(vec2(500.0, 500.0)), &v, 1.0 / 60.0);
        assert!(crate::sfx::take_pending().contains(&Sfx::Flap));
        for _ in 0..240 {
            g.update(&Input::default(), &v, 1.0 / 60.0);
        }
        assert!(crate::sfx::take_pending().contains(&Sfx::Hit));
    }

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
            x: Flappy::bird_x(&v) - PIPE_W / 2.0,
            gap_y: 450.0,
            scored: false,
        });
        g.update(&Input::press_at(Vec2::ZERO), &v, 1.0 / 60.0);
        assert_eq!(g.state, State::Dead);
    }

    #[test]
    fn spike_tips_are_forgiving_but_their_bodies_are_not() {
        let s = Spike::new(500.0, 700.0, 400.0);
        assert!(s.hits(vec2(500.0, 600.0), 10.0));
        assert!(s.hits(vec2(500.0, 395.0), 10.0));
        assert!(!s.hits(vec2(500.0, 380.0), 10.0));
        assert!(!s.hits(vec2(500.0 + s.half_w * 0.5, 420.0), 10.0));
        assert!(s.hits(vec2(500.0 + s.half_w * 0.5, 650.0), 10.0));
        let down = Spike::new(500.0, 0.0, 300.0);
        assert!(down.hits(vec2(500.0, 100.0), 10.0));
        assert!(!down.hits(vec2(500.0, 320.0), 10.0));
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
            x: Flappy::bird_x(&v) - PIPE_W / 2.0,
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
