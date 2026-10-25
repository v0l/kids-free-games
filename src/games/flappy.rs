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
        let roof = -view.inset.top - 40.0;
        [
            Rect::new(p.x, roof, PIPE_W, top - roof),
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
                    if Self::pipe_rects(p, view)
                        .iter()
                        .any(|r| circle_hits_rect(bird, BIRD_R - 5.0, *r))
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
        if self.state == State::Playing {
            big_score(view, &self.score.to_string(), 80.0, self.bump.t);
        }
        chip(view, Side::Right, "BEST", &self.best.0.to_string(), 1.0);
        match self.state {
            State::Ready => {
                let bob = (self.t * 4.0).sin() * 6.0;
                crate::ui::ribbon(
                    view,
                    "Get Ready!",
                    vec2(view.w / 2.0, view.h * 0.2 + bob),
                    self.t,
                );
                let pulse = 1.0 + (self.t * 5.0).sin() * 0.05;
                crate::ui::hint(
                    view,
                    "Tap to flap",
                    vec2(view.w / 2.0, view.h * 0.74),
                    pulse,
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

mod art {
    crate::sprites!("flappy");
}

pub fn tile_x(view_x: f32, view_w: f32, tile_w: f32, scroll: f32) -> Vec<f32> {
    let off = scroll.rem_euclid(tile_w);
    let mut out = vec![];
    let mut x = view_x - off;
    while x < view_x + view_w {
        out.push(x);
        x += tile_w;
    }
    out
}

fn band(name: &str, view: &View, bottom: f32, h: f32, scroll: f32) {
    let f = view.full();
    let s = art::size(name);
    let w = h * s.x / s.y;
    for x in tile_x(f.x, f.w, w, scroll) {
        art::sprite(name, Rect::new(x, bottom - h, w + 1.0, h));
    }
}

impl Flappy {
    fn draw_world(&self, view: &View) {
        let f = view.full();
        let gy = view.h - GROUND;
        band("sky", view, f.y + f.h, f.h, self.scroll * 0.08);
        band(
            "hills",
            view,
            gy + 20.0,
            (f.h * 0.42).max(260.0),
            self.scroll * 0.35,
        );
        for p in &self.pipes {
            Self::draw_pipe(p, view);
        }
        let gs = art::size("ground");
        let gh = GROUND + 34.0 + view.inset.bottom + 40.0;
        let top_frac = 116.0 / gs.y;
        let h = (GROUND + 34.0) / (1.0 - top_frac);
        let w = h * gs.x / gs.y;
        for x in tile_x(f.x, f.w, w, self.scroll) {
            art::sprite(
                "ground",
                Rect::new(x, gy - 34.0 - h * top_frac, w + 1.0, h.max(gh)),
            );
        }
        self.draw_bird(view);
    }

    fn draw_pipe(p: &Pipe, view: &View) {
        let [top, bottom] = Self::pipe_rects(p, view);
        let cap = art::size("pipe_cap");
        let body = art::size("pipe_body");
        let cw = PIPE_W * 1.24;
        let ch = cw * cap.y / cap.x;
        let bw = PIPE_W * 1.02;
        let _ = body;
        let seg = bw * body.y / body.x;
        let cx = p.x + PIPE_W / 2.0;
        let _ = seg;
        let column = |from: f32, to: f32| {
            if to > from {
                art::sprite("pipe_body", Rect::new(cx - bw / 2.0, from, bw, to - from));
            }
        };
        column(top.y, top.y + top.h - ch * 0.6);
        art::sprite_ex(
            "pipe_cap",
            Rect::new(cx - cw / 2.0, top.y + top.h - ch, cw, ch),
            crate::art::Draw {
                flip_y: true,
                ..Default::default()
            },
        );
        column(bottom.y + ch * 0.6, bottom.y + bottom.h);
        art::sprite("pipe_cap", Rect::new(cx - cw / 2.0, bottom.y, cw, ch));
    }

    fn bird_frame(&self) -> &'static str {
        if self.state == State::Dead {
            return "bird_hurt";
        }
        if self.kick > 0.55 {
            "bird_down"
        } else if self.kick > 0.2 || self.state == State::Ready && (self.t * 6.0).sin() > 0.0 {
            "bird_mid"
        } else {
            "bird_up"
        }
    }

    fn draw_bird(&self, view: &View) {
        let bx = Self::bird_x(view);
        let angle = (self.vy / 900.0).clamp(-0.45, 1.1);
        let sc = bird_scale(self.vy, self.kick);
        let ground_y = view.h - GROUND;
        let height = (ground_y - self.y).max(0.0);
        soft_shadow(
            vec2(bx, ground_y + 6.0),
            BIRD_R * (1.4 - (height / 600.0).min(0.7)),
            5.0,
            0.5,
        );
        let frame = self.bird_frame();
        let h = BIRD_R * 3.4;
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
    crate::ui::icon("flappy", r);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flappy_sprites_all_exist() {
        let have = art::names();
        for n in [
            "sky",
            "hills",
            "ground",
            "pipe_cap",
            "pipe_body",
            "bird_up",
            "bird_mid",
            "bird_down",
            "bird_hurt",
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
            x: Flappy::bird_x(&v) - 10.0,
            gap_y: 450.0,
            scored: false,
        });
        g.update(&Input::press_at(Vec2::ZERO), &v, 1.0 / 60.0);
        assert_eq!(g.state, State::Dead);
    }

    #[test]
    fn ground_and_sky_tiles_cover_the_screen_at_any_scroll() {
        for scroll in [0.0, 13.0, 777.7, 123456.0] {
            let xs = tile_x(-40.0, 1100.0, 300.0, scroll);
            assert!(xs[0] <= -40.0);
            assert!(xs.last().unwrap() + 300.0 >= 1060.0);
            assert!(xs.windows(2).all(|w| (w[1] - w[0] - 300.0).abs() < 1e-3));
        }
    }

    #[test]
    fn pipe_columns_reach_past_the_top_edge_and_down_to_the_ground() {
        let v = View::fit_safe(
            2400.0,
            1080.0,
            crate::safe::Insets {
                top: 0.0,
                left: 100.0,
                ..Default::default()
            },
        );
        let p = Pipe {
            x: 300.0,
            gap_y: 300.0,
            scored: false,
        };
        let [top, bottom] = Flappy::pipe_rects(&p, &v);
        assert!(top.y < -v.inset.top);
        assert_eq!(top.y + top.h, 300.0 - GAP / 2.0);
        assert_eq!(bottom.y, 300.0 + GAP / 2.0);
        assert!(bottom.y + bottom.h >= v.h - GROUND);
    }

    #[test]
    fn the_bird_flaps_its_wings_down_after_a_tap() {
        let v = View::ipad_landscape();
        let mut g = Flappy::new();
        g.reset(&v);
        g.state = State::Playing;
        g.kick = 0.0;
        g.vy = 200.0;
        assert_eq!(g.bird_frame(), "bird_up");
        g.flap(&v);
        assert_eq!(g.bird_frame(), "bird_down");
        g.state = State::Dead;
        assert_eq!(g.bird_frame(), "bird_hurt");
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
