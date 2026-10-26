use super::Play;
use crate::draw::Draw;
use crate::paint::Pen;
use crate::ui::{plain_score, Results, Tone};
use glam::{vec2, Vec2};
use kg_core::engine::{Dir, Game, Input, View};
use kg_core::fx::{ease_out_back, Side};
use kg_core::games::crossy::*;
use kg_core::geom::{Color, Rect, RED, WHITE};

pub const KEY: &str = GAME;
const SHEET: &str = "crossy";

pub struct CrossyPlay {
    pub g: Crossy,
    results_age: f32,
}

impl CrossyPlay {
    pub fn new() -> Self {
        Self {
            g: Crossy::new(),
            results_age: 0.0,
        }
    }
}

impl Default for CrossyPlay {
    fn default() -> Self {
        Self::new()
    }
}

impl Play for CrossyPlay {
    fn update(&mut self, input: &Input, view: &View, dt: f32) {
        let showing = self.g.shows_results();
        self.g.update(input, view, dt);
        if self.g.shows_results() {
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
        self.g.state == kg_core::games::crossy::State::Dead
    }

    fn as_any(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

fn tiled(pen: &mut Pen, name: &str, f: Rect, top: f32, h: f32, tint: Color) {
    tiled_offset(pen, name, f, top, h, tint, 0);
}

fn tiled_offset(pen: &mut Pen, name: &str, f: Rect, top: f32, h: f32, tint: Color, shift: usize) {
    let s = crate::art::size(SHEET, name);
    let w = h.max(ROW_PX) * s.x / s.y;
    let mut x = f.x - (shift as f32 * w * 0.37).rem_euclid(w);
    while x < f.x + f.w {
        pen.sprite(SHEET, name, Rect::new(x, top, w + 1.0, h), Draw::tint(tint));
        x += w;
    }
}

fn tiled_strip(pen: &mut Pen, name: &str, f: Rect, top: f32, h: f32, flip_y: bool) {
    let s = crate::art::size(SHEET, name);
    let w = h * s.x / s.y;
    let mut x = f.x;
    while x < f.x + f.w {
        pen.sprite(
            SHEET,
            name,
            Rect::new(x, top, w + 1.0, h),
            Draw {
                flip_y,
                ..Default::default()
            },
        );
        x += w;
    }
}

fn draw_lane(g: &Crossy, pen: &mut Pen, i: usize, f: Rect, top: f32, ox: f32) {
    let r = &g.rows[i];
    let above_road = g
        .rows
        .get(i + 1)
        .is_some_and(|n| matches!(n.lane, Lane::Road { .. }));
    match r.lane {
        Lane::Grass => {
            let k = if i.is_multiple_of(2) { 1.0 } else { 0.9 };
            tiled_offset(pen, "grass", f, top, ROW_PX, Color::new(k, k, k, 1.0), i);
        }
        Lane::Road { .. } => {
            tiled_offset(pen, "road", f, top, ROW_PX, WHITE, i);
            if !above_road {
                tiled_strip(pen, "kerb", f, top, KERB_H, false);
            } else {
                for k in 0..((f.w / 60.0) as i32 + 2) {
                    pen.rect(
                        Rect::new(f.x + k as f32 * 60.0 + 10.0, top - 3.0, 32.0, 6.0),
                        Color::from_rgba(240, 240, 240, 210),
                    );
                }
            }
            let below_is_road = i > 0 && matches!(g.rows[i - 1].lane, Lane::Road { .. });
            if !below_is_road {
                tiled_strip(pen, "kerb", f, top + ROW_PX - KERB_H, KERB_H, true);
            }
        }
        Lane::River { .. } => {
            tiled_offset(pen, "water", f, top, ROW_PX, WHITE, i);
            for k in 0..((f.w / 50.0) as i32 + 2) {
                let wx = (k as f32 * 50.0 + g.t * 20.0 + i as f32 * 13.0).rem_euclid(f.w + 50.0)
                    - 25.0
                    + f.x;
                pen.rect(
                    Rect::new(wx, top + 20.0 + (k % 3) as f32 * 14.0, 18.0, 3.0),
                    Color::new(1.0, 1.0, 1.0, 0.4),
                );
            }
        }
        Lane::Rail { .. } => {
            tiled(pen, "rail", f, top, ROW_PX, WHITE);
            let lit = r.warning() && (g.t * 8.0).sin() > 0.0;
            let tint = if lit {
                RED
            } else {
                Color::from_rgba(120, 60, 60, 255)
            };
            pen.sprite_at(
                "ui",
                "round_white",
                vec2(ox - 30.0, top + 6.0),
                30.0,
                Draw::tint(tint),
            );
        }
    }
}

fn draw_trees(g: &Crossy, pen: &mut Pen, i: usize, y: f32, cell: f32, ox: f32) {
    for c in &g.rows[i].trees {
        let tx = ox + (*c as f32 + 0.5) * cell;
        pen.soft_shadow(vec2(tx, y + 20.0), cell * 0.38, 9.0, 0.45);
        let (name, h) = match (*c as usize + i) % 3 {
            0 => ("tree", ROW_PX * 1.7),
            1 => ("bush", ROW_PX * 0.95),
            _ => ("rock", ROW_PX * 0.8),
        };
        pen.sprite_at(
            SHEET,
            name,
            vec2(tx, y + 24.0 - h / 2.0),
            h,
            Draw::default(),
        );
    }
}

fn draw_things(g: &Crossy, pen: &mut Pen, i: usize, y: f32, cell: f32, ox: f32) {
    let r = &g.rows[i];
    match r.lane {
        Lane::Road { speed, .. } => {
            for (k, t) in r.things.iter().enumerate() {
                let (a, b) = car_span(*t, r.len);
                let cx = ox + (a + b) / 2.0 * cell;
                let w = (b - a) * cell;
                let name = vehicle(r, k);
                let s = crate::art::size(SHEET, name);
                let h = w * s.y / s.x;
                pen.soft_shadow(vec2(cx, y + 24.0), w * 0.42, 8.0, 0.4);
                pen.sprite(
                    SHEET,
                    name,
                    Rect::new(cx - w / 2.0, y + 28.0 - h, w, h),
                    Draw {
                        flip_x: speed < 0.0,
                        ..Default::default()
                    },
                );
            }
        }
        Lane::River { .. } => {
            for t in &r.things {
                let x = ox + t * cell;
                let w = r.len * cell;
                pen.soft_shadow(vec2(x + w / 2.0, y + 22.0), w * 0.45, 8.0, 0.3);
                pen.nine(
                    SHEET,
                    "log",
                    Rect::new(x + 3.0, y - 28.0, w - 6.0, 56.0),
                    46.0,
                    0.5,
                    WHITE,
                );
            }
        }
        Lane::Rail { .. } => {
            for t in &r.things {
                for k in 0..6 {
                    let slot = 2.0 * cell;
                    let name = if k == 5 { "engine" } else { "wagon" };
                    let x = ox + (t + k as f32 * 2.0) * cell;
                    let h = if k == 5 { 118.0 } else { 92.0 };
                    let s = crate::art::size(SHEET, name);
                    let w = (h * s.x / s.y).min(slot + 8.0);
                    pen.sprite(
                        SHEET,
                        name,
                        Rect::new(x + slot - w, y + 30.0 - h, w, h),
                        Draw::default(),
                    );
                }
            }
        }
        Lane::Grass => {}
    }
}

fn draw_chicken(g: &Crossy, pen: &mut Pen, view: &View, cell: f32, ox: f32, shake: Vec2) {
    let p = g.player_pos();
    let arc = (g.hop * std::f32::consts::PI).sin() * 22.0;
    let c = vec2(
        ox + (p.x + 0.5) * cell,
        g.screen_y(p.y, view) + shake.y - arc,
    );
    let flat = g.state == State::Dead && matches!(g.death, Some(Death::Car) | Some(Death::Train));
    if g.state == State::Dead && g.death == Some(Death::Water) {
        return;
    }
    pen.soft_shadow(
        vec2(c.x, c.y + 22.0 + arc),
        20.0 * (1.0 - arc / 60.0),
        7.0,
        0.5,
    );
    let (sx, sy) = if flat {
        (1.6, 0.25)
    } else {
        let s = 1.0 - (g.hop * std::f32::consts::PI).sin() * 0.12;
        (s, 2.0 - s)
    };
    let frame = if arc > 6.0 && !flat {
        "hopper_air"
    } else {
        "hopper"
    };
    let h = HOPPER_H * sy;
    let sz = crate::art::size(SHEET, frame);
    let w = HOPPER_H * sz.x / sz.y * sx;
    let bottom = c.y + 26.0;
    pen.sprite(
        SHEET,
        frame,
        Rect::new(c.x - w / 2.0, bottom - h, w, h),
        Draw {
            flip_x: g.facing == Dir::Left,
            ..Default::default()
        },
    );
}

fn draw_world(g: &Crossy, pen: &mut Pen, view: &View) {
    let shake = g.shake.offset();
    let (cell, ox) = g.col_x(view);
    let ox = ox + shake.x;
    let first = (g.cam - 4.0).floor().max(0.0) as usize;
    let last = (g.cam + view.h / ROW_PX + 2.0) as usize;
    let f = view.full();
    let row0 = g.screen_y(0.0, view) + shake.y + ROW_PX / 2.0;
    tiled(pen, "grass", f, row0, f.y + f.h + ROW_PX - row0, WHITE);
    for i in (first..last.min(g.rows.len())).rev() {
        let y = g.screen_y(i as f32, view) + shake.y;
        draw_lane(g, pen, i, f, y - ROW_PX / 2.0, ox);
        draw_trees(g, pen, i, y, cell, ox);
        draw_things(g, pen, i, y, cell, ox);
        if i as i32 == g.row {
            draw_chicken(g, pen, view, cell, ox, shake);
        }
    }
}

fn draw_goal(g: &Crossy, pen: &mut Pen, view: &View) {
    let w = (view.w * 0.6).min(360.0);
    let bar = Rect::new(view.w / 2.0 - w / 2.0, 122.0, w, 18.0);
    pen.round_rect(bar, 9.0, Color::new(0.0, 0.0, 0.0, 0.35));
    let fill = g.goal_progress().clamp(0.0, 1.0);
    if fill > 0.0 {
        let inner = Rect::new(bar.x + 2.0, bar.y + 2.0, (bar.w - 4.0) * fill, bar.h - 4.0);
        pen.round_rect(inner, 7.0, Color::from_rgba(255, 200, 40, 255));
    }
    let flag = vec2(bar.x + bar.w, bar.y + bar.h / 2.0);
    pen.ui(
        "p_star",
        Rect::new(flag.x - 20.0, flag.y - 20.0, 40.0, 40.0),
        Draw::default(),
    );
    pen.text_shadow(
        &format!("Reach checkpoint {}", g.goal_stage()),
        view.w / 2.0,
        165.0,
        26.0,
        WHITE,
    );
}

fn draw_banner(g: &Crossy, pen: &mut Pen, view: &View) {
    let Some((stage, age)) = g.banner else { return };
    let fade = (1.0 - (age - (BANNER_TIME - 0.5)) / 0.5).clamp(0.0, 1.0);
    let pop = ease_out_back((age / 0.35).min(1.0));
    pen.ribbon(
        view,
        &format!("Checkpoint {stage}"),
        vec2(view.w / 2.0, view.h * 0.3),
        g.t,
    );
    pen.text_shadow(
        &format!("Next: {}", zone_name(stage)),
        view.w / 2.0,
        view.h * 0.3 + 70.0,
        36.0 * pop.max(0.01),
        WHITE.with_alpha(fade),
    );
}

fn draw_hurry(g: &Crossy, pen: &mut Pen, view: &View) {
    let Some(limit) = g.eagle_limit() else { return };
    if g.idle > limit - 2.5 && g.state == State::Playing {
        let k = (g.idle - (limit - 2.5)) / 2.5;
        pen.text_shadow(
            "Hurry up!",
            view.w / 2.0,
            210.0,
            40.0 * (1.0 + (g.t * 10.0).sin() * 0.05 * k),
            Color::from_rgba(255, 90, 90, 255),
        );
    }
}

pub fn draw(g: &Crossy, pen: &mut Pen, view: &View, results_age: f32) {
    pen.offset = Vec2::ZERO;
    draw_world(g, pen, view);
    pen.particles(&g.fx);
    pen.big_score(view, &g.score.to_string(), 60.0, g.bump.t);
    pen.chip(view, Side::Right, "BEST", &g.best.0.to_string(), 1.0);
    draw_goal(g, pen, view);
    draw_hurry(g, pen, view);
    draw_banner(g, pen, view);
    match g.state {
        State::Ready => {
            let pulse = 1.0 + (g.t * 5.0).sin() * 0.05;
            pen.hint(
                "Tap to hop, swipe to turn",
                vec2(view.w / 2.0, view.h * 0.3),
                pulse,
            );
            if let Some(n) = g.checkpoint_offer() {
                pen.painted_button(
                    g.checkpoint_button(view),
                    &format!("Start at checkpoint {n}"),
                    Tone::Blue,
                    0.0,
                    1.0,
                );
            }
        }
        State::Playing if g.oops > 0.0 => {
            pen.text_shadow(
                "Oops! Try again",
                view.w / 2.0,
                view.h * 0.3,
                46.0,
                Color::from_rgba(255, 220, 90, 255),
            );
        }
        State::Playing if g.banner.is_none() => {
            if let Some(text) = g.coach() {
                let pulse = 1.0 + (g.t * 5.0).sin() * 0.05;
                pen.hint(text, vec2(view.w / 2.0, view.h * 0.3), pulse);
            }
        }
        State::Dead if g.shows_results() => {
            let line = g.death_line();
            pen.results(
                view,
                &Results {
                    game: GAME,
                    title: "Oh no!",
                    line: &line,
                    rank: g.rank,
                    action: "Hop again",
                    format: plain_score,
                },
                results_age,
            );
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paint::Tex;

    fn fresh() -> CrossyPlay {
        kg_core::save::reset_for_test();
        kg_core::profile::create("Mia", 0);
        CrossyPlay::new()
    }

    fn drawn(p: &CrossyPlay, v: &View) -> Pen {
        let mut pen = Pen::default();
        pen.begin(*v, 1.0);
        p.draw(&mut pen, v);
        pen
    }

    #[test]
    fn the_ready_screen_draws_the_lanes_the_chicken_and_text() {
        let v = View::ipad_landscape();
        let pen = drawn(&fresh(), &v);
        let texs: Vec<_> = pen.used().iter().filter_map(|b| b.tex).collect();
        assert!(texs.iter().any(|t| matches!(t, Tex::Page("crossy", _))));
        assert!(texs.contains(&Tex::Glyphs));
        assert!(texs.iter().any(|t| matches!(t, Tex::Page("ui", _))));
    }

    #[test]
    fn the_grass_covers_the_whole_screen_past_the_safe_area() {
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
        let (mut lo, mut hi, mut bot) = (f32::MAX, f32::MIN, f32::MIN);
        for b in pen.used() {
            if matches!(b.tex, Some(Tex::Page("crossy", _))) {
                for q in &b.pos {
                    lo = lo.min(q[0]);
                    hi = hi.max(q[0]);
                    bot = bot.max(-q[1]);
                }
            }
        }
        assert!(lo <= f.x + 0.01 && hi >= f.x + f.w, "{lo}..{hi} vs {f:?}");
        assert!(bot >= f.y + f.h - 0.5);
    }

    #[test]
    fn the_results_panel_appears_after_a_crash_and_its_age_restarts() {
        let v = View::ipad_landscape();
        let mut p = fresh();
        p.update(&Input::press_at(vec2(500.0, 500.0)), &v, 1.0 / 60.0);
        p.g.gentle = false;
        p.g.die(Death::Car);
        for _ in 0..90 {
            p.update(&Input::default(), &v, 1.0 / 60.0);
        }
        assert!(p.g.shows_results());
        assert!(p.results_age > 0.5);
        assert!(drawn(&p, &v).vertex_count() > 500);
        for _ in 0..30 {
            p.update(&Input::press_at(vec2(500.0, 500.0)), &v, 1.0 / 60.0);
        }
        assert_eq!(p.g.state, State::Ready);
    }

    #[test]
    fn every_state_draws_in_portrait_too() {
        let v = View::ipad_portrait();
        let mut p = fresh();
        drawn(&p, &v);
        p.update(&Input::press_at(vec2(300.0, 500.0)), &v, 1.0 / 60.0);
        drawn(&p, &v);
        p.g.die(Death::Water);
        drawn(&p, &v);
    }
}
