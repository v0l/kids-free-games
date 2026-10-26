use super::Play;
use crate::draw::Draw;
use crate::paint::Pen;
use crate::ui::{plain_score, Results};
use glam::{vec2, Vec2};
use kg_core::engine::{Game, Input, View};
use kg_core::fx::{Side, CHIP_Y};
use kg_core::games::flappy::tile_x;
use kg_core::games::jumper::*;
use kg_core::geom::{Color, Rect, WHITE};

pub const KEY: &str = kg_core::games::jumper::KEY;
const SHEET: &str = "jumper";

pub struct JumperPlay {
    pub g: Jumper,
    age: f32,
}

impl JumperPlay {
    pub fn new() -> Self {
        Self {
            g: Jumper::new(),
            age: 0.0,
        }
    }
}

impl Default for JumperPlay {
    fn default() -> Self {
        Self::new()
    }
}

impl Play for JumperPlay {
    fn update(&mut self, input: &Input, view: &View, dt: f32) {
        let before = self.g.state;
        self.g.update(input, view, dt);
        if self.g.state != before {
            self.age = 0.0;
            if self.g.state == State::Fallen {
                kg_core::net::request_board(KEY);
            }
        }
        self.age += dt;
    }

    fn draw(&self, pen: &mut Pen, view: &View) {
        draw(&self.g, pen, view, self.age);
    }

    fn score(&self) -> u32 {
        self.g.score
    }

    fn over(&self) -> bool {
        self.g.state == State::Fallen
    }

    fn as_any(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

fn background(g: &Jumper, pen: &mut Pen, view: &View) {
    let f = view.full();
    let climb = (-g.cam / 15000.0).clamp(0.0, 1.0);
    let night = 1.0 - climb * 0.6;
    let dusk = Color::new(night, night * (1.0 - climb * 0.1), 1.0 - climb * 0.3, 1.0);
    let sky_s = crate::art::size(SHEET, "sky");
    let sky_h = f.h.max(f.w * sky_s.y / sky_s.x);
    let sky_w = sky_h * sky_s.x / sky_s.y;
    pen.sprite(
        SHEET,
        "sky",
        Rect::new(f.x + (f.w - sky_w) / 2.0, f.y + f.h - sky_h, sky_w, sky_h),
        Draw::tint(dusk),
    );
    for (name, factor, height) in [("hills_far", 0.08, 0.5), ("hills", 0.2, 0.42)] {
        let h = view.h * height;
        let s = crate::art::size(SHEET, name);
        let w = h * s.x / s.y;
        let bottom = view.h + view.inset.bottom - g.cam * factor;
        if bottom - h > f.y + f.h {
            continue;
        }
        let shade = if name == "hills" { 1.0 } else { 0.95 };
        let tint = Color::new(night * shade, night * shade, night.max(0.55) * shade, 1.0);
        for x in tile_x(f.x, f.w, w, g.t * 3.0 * factor) {
            pen.sprite(SHEET, name, Rect::new(x, bottom - h, w + 1.0, h), Draw::tint(tint));
        }
    }
    for k in 0..7 {
        let kf = k as f32;
        let span = view.h + 260.0;
        let x = (kf * 211.0) % (view.w + 120.0) - 60.0;
        let y = parallax_y(kf * 157.0, g.cam, 0.14 + (kf % 3.0) * 0.06, span);
        let w = 110.0 + (kf * 53.0) % 90.0;
        let name = ["cloud1", "cloud2", "cloud3"][k % 3];
        pen.sprite_w(
            SHEET,
            name,
            vec2(x + (g.t * (4.0 + kf)) % 40.0, y),
            w,
            Draw::tint(Color::new(1.0, 1.0, 1.0, 0.8 - climb * 0.4)),
        );
    }
}

fn draw_plat(g: &Jumper, pen: &mut Pen, p: &Plat, y: f32) {
    let name = match p.kind {
        Kind::Solid | Kind::Spring => "grass",
        Kind::Moving(_) => "stone",
        Kind::Crumble => "wood_cracked",
    };
    let (w, lift) = match name {
        "grass" => (PLAT_W + 36.0, 0.17),
        "stone" => (PLAT_W + 30.0, 0.1),
        _ => (PLAT_W + 24.0, 0.12),
    };
    let s = crate::art::size(SHEET, name);
    let h = w * s.y / s.x;
    pen.sprite(
        SHEET,
        name,
        Rect::new(p.x + PLAT_W / 2.0 - w / 2.0, y - h * lift, w, h),
        Draw::default(),
    );
    if p.kind == Kind::Spring {
        let ext = g.boing_age(p).map_or(1.0, spring_extension);
        let frame = if ext < 0.7 {
            "spring_in"
        } else if ext > 1.04 {
            "spring_out"
        } else {
            "spring"
        };
        let sh = 64.0;
        let fs = crate::art::size(SHEET, frame);
        let sw = sh * fs.x / fs.y;
        pen.sprite(
            SHEET,
            frame,
            Rect::new(p.x + PLAT_W / 2.0 - sw / 2.0, y + 4.0 - sh, sw, sh),
            Draw::default(),
        );
    }
}

fn draw_hero(g: &Jumper, pen: &mut Pen, view: &View, c: Vec2) {
    let (sx, sy) = squash_stretch(g.squish, g.vel.y);
    let frame = g.hero_frame();
    let fh = HERO_H * hero_height(frame);
    let fs = crate::art::size(SHEET, frame);
    let w = fh * fs.x / fs.y * sx;
    let h = fh * sy;
    let anchor = c.y + HERO_R + 2.0;
    for dx in [0.0, -view.w, view.w] {
        let cx = c.x + dx;
        if cx < -w || cx > view.w + w {
            continue;
        }
        pen.sprite(
            SHEET,
            frame,
            Rect::new(cx - w / 2.0, anchor - h, w, h),
            Draw {
                flip_x: g.face < 0.0,
                ..Default::default()
            },
        );
    }
}

fn draw_world(g: &Jumper, pen: &mut Pen, view: &View) {
    for p in &g.plats {
        if !p.broken {
            draw_plat(g, pen, p, p.y - g.cam);
        }
    }
    for c in &g.crumbs {
        let fade = 1.0 - (c.age / CRUMB_LIFE).powi(2);
        let at = vec2(c.pos.x, c.pos.y - g.cam);
        pen.sprite(
            SHEET,
            "crumb",
            Rect::new(
                at.x - c.size.x * 0.7,
                at.y - c.size.y * 0.7,
                c.size.x * 1.4,
                c.size.y * 1.4,
            ),
            Draw {
                rotation: c.angle,
                tint: Color::new(1.0, 1.0, 1.0, fade),
                ..Default::default()
            },
        );
    }
    let c = vec2(g.pos.x, g.pos.y - g.cam);
    if let Some(ground) = g.shadow_ground() {
        let near = 1.0 - ((ground - c.y) / 260.0).clamp(0.0, 1.0);
        pen.soft_shadow(
            vec2(c.x, ground + 2.0),
            HERO_R * (0.5 + 0.5 * near),
            5.0,
            0.5 * near,
        );
    }
    draw_hero(g, pen, view, c);
}

fn draw_hud(g: &Jumper, pen: &mut Pen, view: &View) {
    pen.chip(view, Side::Left, "HEIGHT", &g.score.to_string(), g.score_bump.t);
    pen.chip(view, Side::Right, "BEST", &g.best.0.to_string(), 1.0);
    if g.state == State::Fallen {
        return;
    }
    let w = 240.0;
    let r = Rect::new(view.w / 2.0 - w / 2.0, CHIP_Y + 8.0, w, 30.0);
    pen.round_rect(r, r.h / 2.0, Color::new(0.1, 0.1, 0.3, 0.45));
    let fill = goal_progress(g.score).max(0.04);
    let inner = Rect::new(r.x + 3.0, r.y + 3.0, (r.w - 6.0) * fill, r.h - 6.0);
    pen.round_rect(inner, inner.h / 2.0, Color::from_rgba(255, 210, 60, 255));
    pen.text_center(
        &next_goal(g.score).to_string(),
        r.x + r.w / 2.0,
        r.y + r.h / 2.0 + 1.0,
        22.0,
        WHITE,
    );
}

pub fn draw(g: &Jumper, pen: &mut Pen, view: &View, age: f32) {
    background(g, pen, view);
    pen.offset = g.shake.offset();
    draw_world(g, pen, view);
    pen.particles(&g.particles);
    pen.popups(&g.popups);
    pen.offset = Vec2::ZERO;
    draw_hud(g, pen, view);
    pen.particles(&g.confetti);
    match g.state {
        State::Ready => {
            let lines = g.ready_lines();
            let refs: Vec<&str> = lines.iter().map(|l| l.as_str()).collect();
            pen.panel(view, "Sky Jumper", &refs, g.t);
        }
        State::Fallen => {
            let line = g.result_line();
            pen.results(
                view,
                &Results {
                    game: KEY,
                    title: g.result_title(),
                    line: &line,
                    rank: g.rank,
                    action: "Jump again",
                    format: plain_score,
                },
                age,
            );
        }
        State::Playing => {
            if g.shows_steer_hint() {
                let pulse = 1.0 + (g.t * 4.0).sin() * 0.04;
                pen.hint("Touch left or right to steer", vec2(view.w / 2.0, 150.0), pulse);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paint::Tex;

    fn fresh() -> JumperPlay {
        kg_core::save::reset_for_test();
        kg_core::profile::create("Mia", 0);
        JumperPlay::new()
    }

    fn drawn(p: &JumperPlay, v: &View) -> Pen {
        let mut pen = Pen::default();
        pen.begin(*v, 1.0);
        p.draw(&mut pen, v);
        pen
    }

    #[test]
    fn the_ready_screen_draws_world_text_and_panel() {
        let v = View::ipad_landscape();
        let pen = drawn(&fresh(), &v);
        let texs: Vec<_> = pen.used().iter().filter_map(|b| b.tex).collect();
        assert!(texs.iter().any(|t| matches!(t, Tex::Page("jumper", _))));
        assert!(texs.contains(&Tex::Glyphs));
        assert!(texs.iter().any(|t| matches!(t, Tex::Page("ui", _))));
    }

    #[test]
    fn the_sky_covers_the_whole_screen_past_the_safe_area() {
        let v = View::fit_safe(
            2400.0,
            1080.0,
            kg_core::safe::Insets {
                left: 120.0,
                bottom: 40.0,
                ..Default::default()
            },
        );
        let pen = drawn(&fresh(), &v);
        let f = v.full();
        let sky = &pen.used()[0];
        let (mut lo, mut hi) = (f32::MAX, f32::MIN);
        let (mut top, mut bot) = (f32::MAX, f32::MIN);
        for q in &sky.pos {
            lo = lo.min(q[0]);
            hi = hi.max(q[0]);
            top = top.min(-q[1]);
            bot = bot.max(-q[1]);
        }
        assert!(lo <= f.x + 0.01 && hi >= f.x + f.w, "sky {lo}..{hi} vs {f:?}");
        assert!(top <= f.y + 0.5 && bot >= f.y + f.h - 0.5);
    }

    #[test]
    fn playing_draws_hud_and_the_results_age_restarts_on_each_fall() {
        let v = View::ipad_landscape();
        let mut p = fresh();
        p.update(&Input::press_at(vec2(100.0, 100.0)), &v, 1.0 / 60.0);
        assert_eq!(p.g.state, State::Playing);
        assert!(p.age < 0.1);
        for _ in 0..30 {
            p.update(&Input::default(), &v, 1.0 / 60.0);
        }
        assert!(drawn(&p, &v).vertex_count() > 300);
        p.g.rescues = 0;
        p.g.plats.clear();
        p.g.pos.y = p.g.cam + v.h + 200.0;
        p.update(&Input::default(), &v, 1.0 / 60.0);
        assert_eq!(p.g.state, State::Fallen);
        assert!(p.age < 0.1);
        for _ in 0..60 {
            p.update(&Input::default(), &v, 1.0 / 60.0);
        }
        assert!(p.age > 0.9);
        assert!(drawn(&p, &v).vertex_count() > 500);
    }
}
