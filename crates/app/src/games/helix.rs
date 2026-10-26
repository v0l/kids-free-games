use super::Play;
use crate::draw::measure;
use crate::paint::Pen;
use crate::scene3::{Item, Scene3, Shape3, Skin};
use crate::ui::{plain_score, Results, Tone, BODY, INK};
use glam::{vec2, vec3, Quat, Vec2, Vec3};
use kg_core::engine::{Game, Input, View};
use kg_core::fx::{ease_out_back, ease_out_cubic, Side};
use kg_core::games::helix::*;
use kg_core::geom::{hsv, with_alpha, Color, Rect, WHITE};
use std::f32::consts::TAU;

pub const KEY: &str = GAME;
const SHEET: &str = "helix";
const SKY_DEPTH: f32 = 2.4;
const CLOUD_DEPTH: f32 = 1.6;
const TILE_CROP: [f32; 4] = [0.14, 0.14, 0.72, 0.72];
const FADE_STEPS: f32 = 10.0;

pub struct HelixPlay {
    pub g: Helix,
    results_age: f32,
}

impl HelixPlay {
    pub fn new() -> Self {
        Self {
            g: Helix::new(),
            results_age: 0.0,
        }
    }

    fn shows_results(&self) -> bool {
        self.g.state == State::Dead && self.g.end_t > 0.5
            || self.g.state == State::Won && self.g.end_t > WIN_DELAY
    }
}

impl Default for HelixPlay {
    fn default() -> Self {
        Self::new()
    }
}

impl Play for HelixPlay {
    fn update(&mut self, input: &Input, view: &View, dt: f32) {
        let showing = self.shows_results();
        self.g.update(input, view, dt);
        if self.shows_results() {
            if !showing {
                self.results_age = 0.0;
                kg_core::net::request_board(GAME);
            }
            self.results_age += dt;
        }
    }

    fn draw(&self, pen: &mut Pen, view: &View) {
        draw(&self.g, pen, view, self.results_age);
    }

    fn score(&self) -> u32 {
        self.g.score
    }

    fn over(&self) -> bool {
        self.g.state == State::Dead
    }

    fn as_any(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

fn rgba(c: Color) -> [f32; 4] {
    [c.r, c.g, c.b, c.a]
}

fn skin(name: &'static str) -> Skin {
    Skin {
        sheet: SHEET,
        name,
        crop: [0.0, 0.0, 1.0, 1.0],
    }
}

fn yaw(angle: f32) -> [f32; 4] {
    Quat::from_rotation_y(-angle).to_array()
}

struct Frame {
    eye: Vec3,
    fwd: Vec3,
    right: Vec3,
    up: Vec3,
    dist: f32,
    view: View,
}

impl Frame {
    fn new(g: &Helix, view: &View) -> Self {
        let rig = Rig::new(view, g.cam);
        let fwd = (rig.target - rig.eye).normalize();
        let right = fwd.cross(Vec3::Y).normalize();
        let up = right.cross(fwd);
        let sh = g.shake.offset();
        let nudge = right * -sh.x + up * -sh.y;
        Self {
            eye: rig.eye + nudge,
            fwd,
            right,
            up,
            dist: rig.distance(),
            view: *view,
        }
    }

    fn facing(&self) -> [f32; 4] {
        let m = glam::Mat3::from_cols(self.right, self.up, -self.fwd);
        Quat::from_mat3(&m).to_array()
    }

    fn at_depth(&self, k: f32, px: Vec2) -> (Vec3, f32) {
        let f = self.view.full();
        let per_px = self.dist * k / self.dist;
        let c = vec2(f.x + f.w / 2.0, f.y + f.h / 2.0);
        let p = self.eye
            + self.fwd * self.dist * k
            + self.right * (px.x - c.x) * per_px
            - self.up * (px.y - c.y) * per_px;
        (p, per_px)
    }

    fn billboard(&self, name: &'static str, k: f32, px: Vec2, w: f32, tint: Color) -> Item {
        let (pos, s) = self.at_depth(k, px);
        let a = crate::art::size(SHEET, name);
        let mut it = Item::new(Shape3::Quad);
        it.skin = Some(skin(name));
        it.color = rgba(tint);
        it.pos = pos.to_array();
        it.rot = self.facing();
        it.scale = [w * s, w * s * a.y / a.x.max(1.0), 1.0];
        it.unlit = true;
        it.blend = true;
        it.cast = false;
        it
    }
}

fn backdrop(g: &Helix, fr: &Frame, items: &mut Vec<Item>) {
    let f = fr.view.full();
    let s = crate::art::size(SHEET, "bg");
    let k = (f.w / s.x).max(f.h / s.y) * 1.02;
    let hue = (g.level as f32 * 0.07).rem_euclid(1.0);
    let mut it = fr.billboard("bg", SKY_DEPTH, f.center(), s.x * k, hsv(hue, 0.12, 1.0));
    it.blend = false;
    items.push(it);
    for i in 0..6 {
        let depth = 0.08 + (i % 3) as f32 * 0.05;
        let span = fr.view.w + 400.0;
        let x = (i as f32 * 233.0 + g.t * (5.0 + depth * 40.0)).rem_euclid(span) - 200.0;
        let y = ((i * 131 % 700) as f32 - g.cam * depth).rem_euclid(fr.view.h + 200.0) - 100.0;
        items.push(fr.billboard(
            if i % 2 == 0 { "cloud1" } else { "cloud2" },
            CLOUD_DEPTH,
            vec2(x, y),
            150.0 + depth * 900.0,
            Color::new(1.0, 1.0, 1.0, 0.85),
        ));
    }
}

fn floor_items(g: &Helix, items: &mut Vec<Item>) {
    let near = g.cam - 500.0;
    let far = g.cam + 1100.0;
    let step = TAU / SEGMENTS as f32;
    for (i, f) in g.floors.iter().enumerate() {
        let fy = Helix::floor_y(i);
        if fy < near || fy > far {
            continue;
        }
        let fall = f.broken * f.broken * 900.0;
        let fade = (1.0 - f.broken * 1.5).max(0.0);
        if f.broken > 0.0 && fade <= 0.0 {
            continue;
        }
        let alpha = (fade * FADE_STEPS).ceil() / FADE_STEPS;
        for k in 0..SEGMENTS {
            let seg = f.segs[k];
            if seg == Seg::Gap {
                continue;
            }
            let a0 = g.angle + k as f32 * step;
            let mid = a0 + step / 2.0;
            let name = if seg == Seg::Danger {
                "tile_red"
            } else {
                floor_tile(f.hue)
            };
            let mut it = Item::new(Shape3::Sector);
            it.skin = Some(Skin {
                sheet: SHEET,
                name,
                crop: TILE_CROP,
            });
            it.color = [1.0, 1.0, 1.0, alpha];
            it.blend = f.broken > 0.0;
            it.cast = f.broken == 0.0;
            it.rot = yaw(a0);
            it.pos = [
                mid.cos() * fall * 0.6,
                -fy - fall,
                mid.sin() * fall * 0.6,
            ];
            items.push(it);
        }
        if f.broken == 0.0 {
            for (fi, seg, _) in &g.splats {
                if *fi != i {
                    continue;
                }
                let a = g.angle + (*seg + 0.5) * step;
                let r = (RING_OUTER + RING_INNER) / 2.0;
                let mut it = Item::new(Shape3::Quad);
                it.skin = Some(skin("splat"));
                it.color = [1.0, 1.0, 1.0, 0.92];
                it.pos = [a.cos() * r, -fy + 0.4, a.sin() * r];
                it.rot = Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2).to_array();
                it.scale = [BALL_R * 2.6, BALL_R * 2.6 * 1.05, 1.0];
                it.blend = true;
                it.cast = false;
                items.push(it);
            }
        }
    }
}

fn ball_items(g: &Helix, items: &mut Vec<Item>) {
    let base = g.ball_world();
    if g.state == State::Dead {
        let floor = Helix::floor_y(g.passed.min(g.floors.len() - 1));
        let mut it = Item::new(Shape3::Quad);
        it.skin = Some(skin("splat"));
        it.pos = [base.x, -floor + 0.6, base.z];
        it.rot = Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2).to_array();
        it.scale = [BALL_R * 3.6, BALL_R * 3.8, 1.0];
        it.blend = true;
        it.cast = false;
        items.push(it);
        return;
    }
    if g.streak >= SMASH_FLOORS && g.state == State::Playing {
        for k in 1..=6 {
            let mut t = Item::new(Shape3::Sphere);
            t.color = [1.0, 0.6, 0.2, 0.3];
            t.unlit = true;
            t.blend = true;
            t.cast = false;
            t.pos = [base.x, base.y + k as f32 * 12.0, base.z];
            let r = BALL_R * (1.0 - k as f32 * 0.12);
            t.scale = [r, r, r];
            items.push(t);
        }
    }
    let sq = g.squash * 0.35;
    let stretch = (g.vy.abs() / 1200.0).min(0.25) * (1.0 - g.squash);
    let sx = 1.0 + sq - stretch * 0.5;
    let sy = 1.0 - sq + stretch;
    let mut it = Item::new(Shape3::Sphere);
    it.color = [1.0, 0.52, 0.12, 1.0];
    it.pos = [base.x, base.y - BALL_R * (1.0 - sy), base.z];
    it.scale = [BALL_R * sx, BALL_R * sy, BALL_R * sx];
    items.push(it);
}

pub fn scene(g: &Helix, view: &View) -> Scene3 {
    let fr = Frame::new(g, view);
    let mut items = vec![];
    backdrop(g, &fr, &mut items);
    if g.state != State::Select {
        let top = -(Helix::floor_y(0)) + 900.0;
        let bottom = -(Helix::floor_y(g.floors.len())) - 500.0;
        let mut pole = Item::new(Shape3::Cylinder);
        pole.color = [0.86, 0.8, 0.95, 1.0];
        pole.pos = [0.0, (top + bottom) / 2.0, 0.0];
        pole.scale = [RING_INNER * 1.03, top - bottom, RING_INNER * 1.03];
        items.push(pole);
        floor_items(g, &mut items);
        ball_items(g, &mut items);
    }
    Scene3 {
        eye: fr.eye.to_array(),
        target: (fr.eye + fr.fwd * fr.dist).to_array(),
        fov: FOV,
        near: fr.dist * 0.3,
        far: fr.dist * 4.0,
        light: [-0.45, -1.0, -0.55],
        illuminance: 9000.0,
        ambient: 420.0,
        sky: [0.55, 0.78, 0.98],
        items,
    }
}

fn levels_pill(pen: &mut Pen, view: &View) {
    pen.painted_button(levels_button(view), "Levels", Tone::Blue, 0.0, 1.0);
}

fn draw_select(g: &Helix, pen: &mut Pen, view: &View) {
    pen.ribbon(view, "Pick a level", vec2(view.w / 2.0, 84.0), g.t);
    pen.chip(
        view,
        Side::Right,
        "STARS",
        &kg_core::profile::total_stars(GAME).to_string(),
        1.0,
    );
    let l = select_layout(view);
    let open = reached();
    let now = start_level();
    for (i, r) in l.cells.iter().enumerate() {
        let n = i as u32 + 1;
        let locked = n > open;
        let tint = if locked {
            Color::new(0.55, 0.55, 0.62, 0.8)
        } else if n == now {
            Color::from_rgba(255, 224, 120, 255)
        } else {
            WHITE
        };
        let pulse = if n == now {
            1.0 + (g.t * 4.0).sin() * 0.03
        } else {
            1.0
        };
        let rr = Rect::new(
            r.center().x - r.w * pulse / 2.0,
            r.center().y - r.h * pulse / 2.0,
            r.w * pulse,
            r.h * pulse,
        );
        pen.nine("ui", "panel", rr, 14.0, (rr.w.min(rr.h) / 64.0).clamp(0.6, 2.5), tint);
        let ink = if locked {
            Color::new(0.4, 0.38, 0.5, 0.7)
        } else {
            INK
        };
        pen.text_center(&n.to_string(), r.center().x, r.y + r.h * 0.38, r.h * 0.4, ink);
        if !locked {
            let earned = kg_core::profile::stars(GAME, n);
            for k in 0..3u8 {
                let tint = if k < earned {
                    WHITE
                } else {
                    Color::new(0.6, 0.6, 0.7, 0.6)
                };
                pen.sprite_at(
                    "ui",
                    if k < earned { "star_yellow" } else { "star_white" },
                    vec2(
                        r.center().x + (k as f32 - 1.0) * r.w * 0.26,
                        r.y + r.h * 0.74,
                    ),
                    r.h * 0.24,
                    crate::draw::Draw::tint(tint),
                );
            }
        }
    }
    let label = format!("Play level {now}");
    pen.painted_button(l.play, &label, Tone::Green, 0.0, 1.0);
}

fn draw_win(g: &Helix, pen: &mut Pen, view: &View) {
    let age = (g.end_t - WIN_DELAY).max(0.0);
    let fade = ease_out_cubic(age / 0.2);
    pen.fill_screen(view, Color::new(0.06, 0.04, 0.16, 0.5 * fade));
    let pw = 560.0f32.min(view.w - 40.0);
    let ph = 420.0;
    let s = ease_out_back(age / 0.32).max(0.01);
    let c = view.center() + vec2(0.0, 10.0 + (1.0 - ease_out_cubic(age / 0.32)) * 90.0);
    let r = Rect::new(c.x - pw * s / 2.0, c.y - ph * s / 2.0, pw * s, ph * s);
    pen.painted_panel(r, WHITE);
    pen.ribbon(view, "Hooray!", vec2(c.x, r.y + 8.0 * s), age);
    let a = |d: f32| ((age - d) / 0.2).clamp(0.0, 1.0);
    let line = format!("Level {} done!", g.level);
    pen.text_center(&line, c.x, r.y + 100.0 * s, 36.0 * s, with_alpha(BODY, a(0.12)));
    for k in 0..3usize {
        let earned = k < g.stars as usize;
        let pop = if earned {
            ease_out_back(((age - STAR_START - STAR_GAP * k as f32) / 0.3).clamp(0.0, 1.0))
        } else {
            a(STAR_START)
        };
        let tint = if earned {
            WHITE
        } else {
            Color::new(0.4, 0.4, 0.46, 0.85)
        };
        let lift = if k == 1 { -14.0 } else { 0.0 };
        pen.sprite_at(
            "ui",
            "p_star",
            vec2(c.x + (k as f32 - 1.0) * 120.0 * s, r.y + (205.0 + lift) * s),
            100.0 * s * pop,
            crate::draw::Draw::tint(tint),
        );
    }
    let score = format!("Score {}   Best {}", g.score, g.best.0);
    pen.text_center(&score, c.x, r.y + 292.0 * s, 30.0 * s, with_alpha(BODY, a(0.5)));
    let label = if g.level >= MAX_LEVEL {
        "Play again"
    } else {
        "Next level"
    };
    let by = r.y + r.h - 56.0 * s;
    let pulse = 1.0 + (age * 4.5).sin() * 0.03;
    let bw = (measure(label, 30.0) + 110.0) * s * pulse;
    pen.painted_button(
        Rect::new(c.x - bw / 2.0, by - 10.0 * s, bw, 78.0 * s * pulse),
        label,
        Tone::Green,
        0.0,
        a(0.6),
    );
}

fn draw_progress(g: &Helix, pen: &mut Pen, view: &View) {
    let progress = g.passed as f32 / g.floors.len() as f32;
    let bar = Rect::new(view.w / 2.0 - 180.0, 24.0, 360.0, 26.0);
    pen.nine("ui", "panel", bar, 14.0, bar.h / 64.0, WHITE);
    pen.nine(
        "ui",
        "btn_yellow",
        Rect::new(bar.x, bar.y, (bar.w * progress).max(bar.h), bar.h),
        18.0,
        bar.h / 64.0,
        WHITE,
    );
    for (k, x) in [(g.level, bar.x - 26.0), (g.level + 1, bar.x + bar.w + 26.0)] {
        let done = x < view.w / 2.0 || progress >= 1.0;
        pen.sprite_at(
            "ui",
            if done { "round_yellow" } else { "round_white" },
            vec2(x, bar.y + 13.0),
            46.0,
            Default::default(),
        );
        pen.text_outline(
            &k.to_string(),
            x,
            bar.y + 11.0,
            26.0,
            WHITE,
            if done {
                Color::from_rgba(150, 90, 0, 255)
            } else {
                INK
            },
        );
    }
    let left = g.floors.len() - g.passed;
    let goal = match left {
        0 => "You made it!".to_string(),
        1 => "1 floor to go".to_string(),
        n => format!("{n} floors to go"),
    };
    pen.text_shadow(&goal, view.w / 2.0, 72.0, 24.0, WHITE);
}

pub fn draw(g: &Helix, pen: &mut Pen, view: &View, results_age: f32) {
    pen.scene = Some(scene(g, view));
    if g.state == State::Select {
        draw_select(g, pen, view);
        return;
    }
    pen.particles(&g.fx);
    pen.popups(&g.pops);
    draw_progress(g, pen, view);
    pen.big_score(view, &g.score.to_string(), 128.0, g.bump.t);
    pen.chip(view, Side::Left, "LEVEL", &g.level.to_string(), 1.0);
    pen.chip(view, Side::Right, "BEST", &g.best.0.to_string(), 1.0);
    let pulse = 1.0 + (g.t * 3.0).sin() * 0.03;
    let at = vec2(view.w / 2.0, view.h * 0.8);
    match g.state {
        State::Ready => {
            pen.hint("Tap, then drag sideways to spin the tower", at, pulse);
            levels_pill(pen, view);
        }
        State::Playing if !g.taught => pen.hint("Line up the hole to drop through!", at, pulse),
        State::Dead if g.end_t > 0.5 => {
            let line = format!("Level {}, score {}", g.level, g.score);
            pen.results(
                view,
                &Results {
                    game: GAME,
                    title: "Splat!",
                    line: &line,
                    rank: g.rank,
                    action: "Try again",
                    format: plain_score,
                },
                results_age,
            );
            levels_pill(pen, view);
        }
        State::Won if g.end_t > WIN_DELAY => {
            draw_win(g, pen, view);
            levels_pill(pen, view);
        }
        _ => {}
    }
    let _ = vec3(0.0, 0.0, 0.0);
}
