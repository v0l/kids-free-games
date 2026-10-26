use super::Play;
use crate::draw::Draw;
use crate::paint::Pen;
use crate::ui::{plain_score, Results};
use glam::{vec2, Vec2};
use kg_core::engine::{Game, Input, View};
use kg_core::fx::{ease_out_back, ease_out_cubic, Side};
use kg_core::games::stack::*;
use kg_core::geom::{with_alpha, Color, Rect, WHITE};

pub const KEY: &str = kg_core::games::stack::KEY;
const SHEET: &str = "stack";
const PLUM: Color = Color::from_rgba(70, 30, 100, 255);

pub struct StackPlay {
    pub g: Stack,
    results_age: f32,
}

impl StackPlay {
    pub fn new() -> Self {
        Self {
            g: Stack::new(),
            results_age: 0.0,
        }
    }
}

impl Default for StackPlay {
    fn default() -> Self {
        Self::new()
    }
}

impl Play for StackPlay {
    fn update(&mut self, input: &Input, view: &View, dt: f32) {
        let showing = self.g.shows_results();
        self.g.update(input, view, dt);
        if self.g.shows_results() {
            if !showing {
                self.results_age = 0.0;
                kg_core::net::request_board(KEY);
            }
            self.results_age += dt;
        }
    }

    fn draw(&self, pen: &mut Pen, view: &View) {
        draw(&self.g, pen, view, self.results_age);
    }

    fn over(&self) -> bool {
        self.g.state == State::Over
    }

    fn score(&self) -> u32 {
        let Stack { points, .. } = &self.g;
        *points
    }

    fn as_any(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

fn tinted(c: Color) -> Draw {
    Draw::tint(c)
}

fn ellipse_outline(pen: &mut Pen, c: Vec2, rx: f32, ry: f32, width: f32, col: Color) {
    let n = 40;
    let at = |i: usize| {
        let a = std::f32::consts::TAU * i as f32 / n as f32;
        c + vec2(a.cos() * rx, a.sin() * ry)
    };
    for i in 0..n {
        pen.line(at(i), at(i + 1), width, col);
    }
}

fn draw_backdrop(g: &Stack, pen: &mut Pen, view: &View) {
    let sky_score = g.sky_score();
    let k = altitude(sky_score);
    let dusk = dusk_tint(sky_score);
    for (fx, fy, size) in star_field(48) {
        let twinkle = 0.6 + 0.4 * (g.time * 2.5 + fx * 40.0).sin();
        let y = (fy * view.h + g.cam * 0.05).rem_euclid(view.h);
        let a = (k * 1.6 - 0.5).clamp(0.0, 1.0) * twinkle;
        if a > 0.02 {
            if size > 3.0 {
                pen.star(
                    vec2(fx * view.w, y),
                    size * 1.8,
                    g.time * 0.3,
                    with_alpha(WHITE, a),
                );
            } else {
                pen.circle(vec2(fx * view.w, y), size, with_alpha(WHITE, a));
            }
        }
    }
    let moon_a = (k * 2.0 - 0.6).clamp(0.0, 1.0);
    if moon_a > 0.0 {
        pen.sprite_at(
            SHEET,
            "moon",
            vec2(view.w * 0.82, 150.0 + (1.0 - moon_a) * 60.0),
            90.0,
            tinted(with_alpha(WHITE, moon_a)),
        );
    }
    let cloud_alpha = 0.85 * (1.0 - k * 0.8);
    for i in 0..7 {
        let depth = 0.12 + (i % 3) as f32 * 0.08;
        let span = view.w + 500.0;
        let x = (i as f32 * 270.0 + g.time * (6.0 + depth * 40.0)).rem_euclid(span) - 250.0;
        let y = (60.0 + (i * 97 % 520) as f32 + g.cam * depth).rem_euclid(view.h + 160.0) - 80.0;
        pen.sprite_w(
            SHEET,
            ["cloud1", "cloud2", "cloud3"][i % 3],
            vec2(x, y),
            (0.8 + depth * 2.5) * 150.0,
            tinted(with_alpha(dusk_cloud(sky_score), cloud_alpha)),
        );
    }
    let ground = view.h - 80.0;
    let f = view.full();
    let peaks = crate::art::size(SHEET, "peaks");
    let peak_h = 190.0;
    let peak_w = peak_h * peaks.x / peaks.y;
    let peak_bottom = ground + 24.0 + g.cam * 0.55;
    let haze = Color::new(0.76 * dusk.r, 0.89 * dusk.g, 0.97 * dusk.b, 1.0);
    pen.rect(Rect::new(f.x, peak_bottom - 2.0, f.w, f.h + 400.0), haze);
    let shift = -g.center * 0.12;
    for (x, flip) in tile_spans(f.x - shift, f.x + f.w - shift, peak_w) {
        pen.sprite(
            SHEET,
            "peaks",
            Rect::new(x + shift, peak_bottom - peak_h, peak_w + 1.0, peak_h),
            Draw {
                tint: Color::new(dusk.r, dusk.g, dusk.b, 1.0),
                flip_x: flip,
                ..Default::default()
            },
        );
    }
}

fn draw_scenery(g: &Stack, pen: &mut Pen, view: &View) {
    let dusk = dusk_tint(g.sky_score());
    let reach = 1500.0;
    let hill_h = 230.0;
    let hs = crate::art::size(SHEET, "hills");
    let hill_w = hill_h * hs.x / hs.y;
    for (x, flip) in tile_spans(-reach, reach, hill_w) {
        pen.sprite(
            SHEET,
            "hills",
            Rect::new(x - 120.0, -hill_h + 30.0, hill_w + 1.0, hill_h),
            Draw {
                tint: Color::new(dusk.r, dusk.g, dusk.b, 1.0),
                flip_x: flip,
                ..Default::default()
            },
        );
    }
    for (x, name, h) in SCENERY {
        if x < -view.w || x > view.w {
            continue;
        }
        let k = if name == "castle" || name == "house" {
            0.95
        } else {
            1.0
        };
        pen.sprite_at(
            SHEET,
            name,
            vec2(x, 6.0 - h / 2.0),
            h,
            tinted(Color::new(dusk.r * k, dusk.g * k, dusk.b * k, 1.0)),
        );
    }
}

fn block_sprite(pen: &mut Pen, r: Rect, tint: Color) {
    let src_h = crate::art::size(SHEET, "blk").y;
    pen.nine(SHEET, "blk", r, src_h * 0.3, BLOCK_H / src_h, tint);
}

fn draw_goal_line(g: &Stack, pen: &mut Pen) {
    let y = block_top(g.goal as usize);
    let seg = 28.0;
    let n = (840.0 / seg) as i32;
    for i in 0..n {
        let c = if i % 2 == 0 {
            Color::from_rgba(255, 210, 60, 255)
        } else {
            WHITE
        };
        pen.rect(Rect::new(-420.0 + i as f32 * seg, y - 5.0, seg, 10.0), c);
    }
    pen.text_outline(
        "GOAL",
        300.0,
        y - 26.0,
        30.0,
        Color::from_rgba(255, 220, 80, 255),
        PLUM,
    );
}

fn draw_tower(g: &Stack, pen: &mut Pen, view: &View, ground: f32) {
    let screen_y = |wy: f32| ground + g.cam + wy;
    let dusk = dusk_tint(g.sky_score());
    draw_scenery(g, pen, view);
    pen.rect(
        Rect::new(-3000.0, 0.0, 6000.0, 700.0),
        Color::new(0.45 * dusk.r, 0.3 * dusk.g, 0.26 * dusk.b, 1.0),
    );
    let gs = crate::art::size(SHEET, "ground");
    let gh = 150.0;
    let gw = gh * gs.x / gs.y;
    for (x, flip) in tile_spans(-1800.0, 1800.0, gw) {
        pen.sprite(
            SHEET,
            "ground",
            Rect::new(x, -gh * 0.12, gw + 1.0, gh),
            Draw {
                tint: Color::new(dusk.r, dusk.g, dusk.b, 1.0),
                flip_x: flip,
                ..Default::default()
            },
        );
    }
    let base = g.tower[0];
    pen.soft_shadow(vec2(base.x + base.w / 2.0, 10.0), base.w * 0.6, 10.0, 0.8);
    for (i, b) in g.tower.iter().enumerate() {
        let y = block_top(i);
        let sy = screen_y(y);
        if sy > view.h + BLOCK_H || sy < -BLOCK_H {
            continue;
        }
        block_sprite(pen, Rect::new(b.x, y, b.w, BLOCK_H), block_tint(b.hue));
    }
    draw_goal_line(g, pen);
    let my = block_top(g.tower.len());
    if g.state == State::Playing {
        let b = g.moving;
        let c = block_tint(b.hue);
        block_sprite(
            pen,
            Rect::new(b.x - 5.0, my - 5.0, b.w + 10.0, BLOCK_H + 10.0),
            with_alpha(c, 0.25),
        );
        block_sprite(pen, Rect::new(b.x, my, b.w, BLOCK_H), c);
        pen.shine_sweep(Rect::new(b.x, my, b.w, BLOCK_H), g.time);
    }
    for c in &g.chips {
        block_sprite(
            pen,
            Rect::new(c.rect.x, my + c.rect.y, c.rect.w, c.rect.h),
            c.color,
        );
    }
}

fn draw_progress(g: &Stack, pen: &mut Pen, view: &View) {
    let w = 320.0f32.min(view.w - 200.0);
    let r = Rect::new(view.w / 2.0 - w / 2.0, 150.0, w, 26.0);
    pen.round_rect(
        Rect::new(r.x, r.y + 3.0, r.w, r.h),
        r.h / 2.0,
        Color::new(0.05, 0.02, 0.15, 0.3),
    );
    pen.round_rect(r, r.h / 2.0, Color::new(0.2, 0.08, 0.35, 0.72));
    let k = (g.score as f32 / g.goal.max(1) as f32).clamp(0.0, 1.0);
    if k > 0.0 {
        let fill = Rect::new(r.x + 3.0, r.y + 3.0, (r.w - 6.0) * k, r.h - 6.0);
        pen.round_rect(
            Rect::new(fill.x, fill.y, fill.w.max(fill.h), fill.h),
            fill.h / 2.0,
            Color::from_rgba(255, 200, 40, 255),
        );
    }
    pen.text_outline(
        &format!("{} / {}", g.score, g.goal),
        r.center().x,
        r.center().y,
        22.0,
        WHITE,
        Color::from_rgba(50, 20, 70, 255),
    );
}

fn draw_won_stars(g: &Stack, pen: &mut Pen, view: &View, age: f32) {
    let c = view.center() + vec2(0.0, 20.0);
    let size = 84.0;
    for k in 0..3u8 {
        let t = ((age - 0.3 - k as f32 * 0.22) / 0.3).clamp(0.0, 1.0);
        let earned = k < g.stars;
        let pop = if earned { ease_out_back(t) } else { t };
        let tint = if earned {
            WHITE
        } else {
            Color::new(0.45, 0.4, 0.55, 0.35 * t)
        };
        let x = c.x + (k as f32 - 1.0) * (size + 16.0);
        let lift = if k == 1 { -10.0 } else { 0.0 };
        pen.ui(
            "p_star",
            Rect::new(
                x - size * pop / 2.0,
                c.y - 40.0 + lift - size * pop / 2.0,
                size * pop,
                size * pop,
            ),
            tinted(tint),
        );
    }
}

fn draw_picker(g: &Stack, pen: &mut Pen, view: &View) {
    let open = kg_core::profile::unlocked(KEY).max(1);
    pen.ribbon(view, "Pick a level", vec2(view.w / 2.0, 72.0), g.time);
    let total = kg_core::profile::total_stars(KEY);
    pen.chip(
        view,
        Side::Left,
        "STARS",
        &format!("{}/{}", total, MAX_LEVEL * 3),
        1.0,
    );
    pen.painted_round(close_button(view), "icon_cross", 0.0);
    for (i, r) in picker_layout(view).iter().enumerate() {
        let level = i as u32 + 1;
        let locked = level > open;
        let tint = if locked {
            Color::new(0.62, 0.6, 0.7, 1.0)
        } else if level == g.level {
            Color::from_rgba(255, 236, 150, 255)
        } else {
            WHITE
        };
        pen.painted_panel(*r, tint);
        if locked {
            pen.ui(
                "icon_lock",
                Rect::new(
                    r.center().x - r.w * 0.17,
                    r.center().y - r.h * 0.17,
                    r.w * 0.34,
                    r.w * 0.34,
                ),
                tinted(Color::new(0.35, 0.3, 0.45, 0.9)),
            );
            continue;
        }
        pen.text_outline(
            &level.to_string(),
            r.center().x,
            r.y + r.h * 0.38,
            r.h * 0.34,
            WHITE,
            PLUM,
        );
        let got = kg_core::profile::stars(KEY, level);
        let ss = r.w * 0.22;
        for k in 0..3u8 {
            let x = r.center().x + (k as f32 - 1.0) * ss * 1.05;
            let tint = if k < got {
                WHITE
            } else {
                Color::new(0.45, 0.4, 0.55, 0.35)
            };
            pen.ui(
                "p_star",
                Rect::new(x - ss / 2.0, r.y + r.h * 0.62, ss, ss),
                tinted(tint),
            );
        }
    }
}

pub fn draw(g: &Stack, pen: &mut Pen, view: &View, results_age: f32) {
    let hue = g.tower.last().unwrap().hue;
    let (top, bottom) = sky_palette(hue, g.sky_score());
    pen.sky(view, top, bottom);
    draw_backdrop(g, pen, view);
    if g.picking {
        draw_picker(g, pen, view);
        return;
    }
    let ground = view.h - 80.0;
    let ox = view.w / 2.0 - g.center * 0.5;
    pen.offset = vec2(ox, ground + g.cam) + g.shake.offset();
    draw_tower(g, pen, view, ground);
    for r in &g.rings {
        let k = ease_out_cubic(r.age / RING_LIFE);
        let a = 1.0 - k;
        ellipse_outline(
            pen,
            r.at,
            r.w * (0.5 + 0.35 * k),
            BLOCK_H * (0.6 + 1.2 * k),
            5.0 * a + 1.0,
            with_alpha(WHITE, a),
        );
    }
    pen.particles(&g.fx);
    pen.popups(&g.popups);
    pen.offset = Vec2::ZERO;
    pen.big_score(view, &g.score.to_string(), 90.0, g.bump.t);
    draw_progress(g, pen, view);
    pen.chip(view, Side::Left, "LEVEL", &g.level.to_string(), 1.0);
    pen.chip(view, Side::Right, "SCORE", &g.points.to_string(), 1.0);
    if !g.taught && g.state == State::Playing {
        let bob = (g.time * 4.0).sin() * 4.0;
        pen.hint(
            "Tap to drop the block",
            vec2(view.w / 2.0, 240.0 + bob),
            1.0 + (g.time * 5.0).sin() * 0.03,
        );
    }
    pen.particles(&g.celebrate);
    match g.state {
        State::Over if g.shows_results() => {
            let line = format!("Reached {} of {} blocks", g.score, g.goal);
            pen.results(
                view,
                &Results {
                    game: KEY,
                    title: "Tower fell!",
                    line: &line,
                    rank: g.rank,
                    action: "Try again",
                    format: plain_score,
                },
                results_age,
            );
        }
        State::Won => {
            let title = format!("Level {} done!", g.level);
            let action = if g.level >= MAX_LEVEL {
                "Play again".to_string()
            } else {
                format!("Level {}", g.level + 1)
            };
            pen.panel(view, &title, &[" ", &action], g.age);
            draw_won_stars(g, pen, view, g.age);
        }
        _ => {}
    }
    if g.shows_levels_button() {
        pen.painted_button(
            levels_button(view),
            "Levels",
            crate::ui::Tone::Blue,
            0.0,
            1.0,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paint::Tex;

    const DT: f32 = 1.0 / 60.0;

    fn fresh() -> StackPlay {
        kg_core::save::reset_for_test();
        kg_core::profile::create("Mia", 0);
        StackPlay::new()
    }

    fn drawn(p: &StackPlay, v: &View) -> Pen {
        let mut pen = Pen::default();
        pen.begin(*v, 1.0);
        p.draw(&mut pen, v);
        pen
    }

    fn texs(pen: &Pen) -> Vec<Tex> {
        pen.used().iter().filter_map(|b| b.tex).collect()
    }

    #[test]
    fn the_play_screen_draws_the_stack_sheet_ui_and_text() {
        let p = fresh();
        let pen = drawn(&p, &View::ipad_landscape());
        let t = texs(&pen);
        assert!(t.iter().any(|t| matches!(t, Tex::Page("stack", _))));
        assert!(t.iter().any(|t| matches!(t, Tex::Page("ui", _))));
        assert!(t.contains(&Tex::Glyphs));
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
        assert!(
            lo <= f.x + 0.01 && hi >= f.x + f.w,
            "sky {lo}..{hi} vs {f:?}"
        );
        assert!(top <= f.y + 0.5 && bot >= f.y + f.h - 0.5);
    }

    #[test]
    fn the_world_offset_is_cleared_before_the_hud() {
        let mut p = fresh();
        let v = View::ipad_portrait();
        p.update(&Input::default(), &v, DT);
        let mut pen = drawn(&p, &v);
        assert_eq!(pen.offset, Vec2::ZERO);
        pen.begin(v, 1.0);
    }

    #[test]
    fn the_picker_draws_a_tile_per_level() {
        let mut p = fresh();
        p.g.picking = true;
        let v = View::ipad_landscape();
        let pen = drawn(&p, &v);
        assert!(texs(&pen).iter().any(|t| matches!(t, Tex::Page("ui", _))));
        assert!(pen.vertex_count() > 1000);
    }

    #[test]
    fn the_results_panel_appears_after_a_fall_and_its_age_restarts() {
        kg_core::save::reset_for_test();
        kg_core::profile::create("Mia", 0);
        kg_core::profile::set(KEY, "taught", 1);
        let v = View::ipad_landscape();
        let mut p = StackPlay::new();
        p.g = Stack::for_level(3);
        p.g.moving.x = 400.0;
        p.g.drop_block();
        let before = drawn(&p, &v).vertex_count();
        for _ in 0..200 {
            p.update(&Input::default(), &v, DT);
        }
        assert!(p.g.shows_results());
        assert!(p.results_age > 0.5);
        assert!(drawn(&p, &v).vertex_count() > before);
        p.update(&Input::press_at(vec2(500.0, 500.0)), &v, DT);
        assert_eq!(p.g.state, State::Playing);
    }

    #[test]
    fn the_win_panel_shows_stars_and_the_next_level() {
        let mut p = fresh();
        let v = View::ipad_portrait();
        for _ in 0..p.g.goal {
            p.g.moving.x = p.g.tower.last().unwrap().x;
            p.g.drop_block();
        }
        assert_eq!(p.g.state, State::Won);
        for _ in 0..90 {
            p.update(&Input::default(), &v, DT);
        }
        let pen = drawn(&p, &v);
        assert!(pen.vertex_count() > 500);
    }
}
