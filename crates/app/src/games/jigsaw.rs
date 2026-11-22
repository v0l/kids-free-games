use super::Play;
use crate::draw::Draw;
use crate::paint::{Pen, Tex};
use crate::ui::{Results, Tone};
use glam::{vec2, Vec2};
use kg_core::engine::{Game, Input, View};
use kg_core::fx::ease_out_back;
use kg_core::games::jigsaw::*;
use kg_core::geom::{with_alpha, Color, Rect, WHITE};

pub const KEY: &str = GAME;
const SHEET: &str = "jigsaw";
const INK: Color = Color::from_rgba(40, 30, 60, 255);
const BLUE_INK: Color = Color::from_rgba(120, 60, 10, 255);
const STAR_GOLD: Color = Color::from_rgba(255, 210, 60, 255);
const WARM_TOP: Color = Color::from_rgba(255, 226, 150, 255);
const WARM_BOTTOM: Color = Color::from_rgba(250, 170, 110, 255);

pub struct JigsawPlay {
    pub g: Jigsaw,
}

impl JigsawPlay {
    pub fn new() -> Self {
        Self { g: Jigsaw::new() }
    }
}

impl Default for JigsawPlay {
    fn default() -> Self {
        Self::new()
    }
}

fn cleared(g: &Jigsaw) -> bool {
    matches!(g.mode, Mode::Report(_)) && matches!(g.report, Some(Report::Cleared { .. }))
}

impl Play for JigsawPlay {
    fn update(&mut self, input: &Input, view: &View, dt: f32) {
        let showing = cleared(&self.g);
        self.g.update(input, view, dt);
        if cleared(&self.g) && !showing {
            kg_core::net::request_board(GAME);
        }
    }

    fn draw(&self, pen: &mut Pen, view: &View) {
        draw(&self.g, pen, view);
    }

    fn score(&self) -> u32 {
        kg_core::profile::total_stars(GAME)
    }

    fn over(&self) -> bool {
        cleared(&self.g)
    }

    fn as_any(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

pub fn sprite_names() -> Vec<String> {
    let mut v = vec!["icon_jigsaw".to_string()];
    v.extend((0..LEVELS).map(picture_name));
    v
}

fn picture_uv(name: &str, cols: usize, rows: usize) -> Option<(Tex, [f32; 4])> {
    let f = crate::art::frame(SHEET, name)?;
    let (pw, ph) = crate::art::page_size(SHEET, f.page);
    let aspect = cols as f32 / rows as f32;
    let (mut w, mut h) = (1.0f32, 1.0f32);
    if aspect > 1.0 {
        h = 1.0 / aspect;
    } else {
        w = aspect;
    }
    let x0 = (1.0 - w) / 2.0;
    let y0 = (1.0 - h) / 2.0;
    let u = |k: f32| (f.src.x + f.src.w * k) / pw;
    let v = |k: f32| (f.src.y + f.src.h * k) / ph;
    Some((
        Tex::Page(SHEET, f.page as u16),
        [u(x0), v(y0), u(x0 + w), v(y0 + h)],
    ))
}

fn cell_uv(uv: [f32; 4], cols: usize, rows: usize, col: usize, row: usize) -> [Vec2; 4] {
    let du = (uv[2] - uv[0]) / cols as f32;
    let dv = (uv[3] - uv[1]) / rows as f32;
    let (u0, v0) = (uv[0] + du * col as f32, uv[1] + dv * row as f32);
    [
        vec2(u0, v0),
        vec2(u0 + du, v0),
        vec2(u0 + du, v0 + dv),
        vec2(u0, v0 + dv),
    ]
}

fn quad_at(pen: &mut Pen, tex: Tex, r: Rect, uvs: [Vec2; 4], tint: Color) {
    pen.quad(
        tex,
        [
            vec2(r.x, r.y),
            vec2(r.right(), r.y),
            vec2(r.right(), r.bottom()),
            vec2(r.x, r.bottom()),
        ],
        uvs,
        tint,
    );
}

fn draw_backdrop(pen: &mut Pen, view: &View) {
    let f = view.full();
    pen.gradient(f, WARM_TOP, WARM_BOTTOM);
}

fn draw_hud(g: &Jigsaw, pen: &mut Pen, view: &View) {
    let h = hud(view);
    let title = format!("{} pieces", pieces_in(g.level));
    pen.hud_title(view, &h, &title, Color::from_rgba(150, 70, 0, 255));
    for (i, c) in h.stars.iter().enumerate() {
        let earned = kg_core::profile::stars(GAME, i as u32) > 0;
        pen.star(
            *c + vec2(0.0, 2.0),
            h.star_r * 1.1,
            0.0,
            Color::new(0.1, 0.05, 0.25, 0.35),
        );
        let fill = if earned {
            STAR_GOLD
        } else {
            with_alpha(WHITE, 0.55)
        };
        pen.star(*c, h.star_r, 0.0, fill);
    }
    let goal = format!("{} of {} in place", g.placed_count(), g.pieces.len());
    let text = if g.first_time { HUD_GOAL } else { &goal };
    pen.hud_goal(view, &h, text, g.first_time, WHITE, INK);
    if let (false, Some(b)) = (g.first_time, h.levels) {
        pen.painted_button(b, "Levels", Tone::Orange, 0.0, 1.0);
    }
}

fn draw_board(g: &Jigsaw, pen: &mut Pen, view: &View) {
    let lay = layout(view, g.level);
    let (cols, rows) = GRIDS[g.level];
    let b = lay.board;
    let frame = Rect::new(b.x - 10.0, b.y - 10.0, b.w + 20.0, b.h + 20.0);
    pen.round_rect(
        frame.offset(vec2(0.0, 6.0)),
        18.0,
        Color::new(0.0, 0.0, 0.0, 0.2),
    );
    pen.round_rect(frame, 18.0, Color::from_rgba(255, 248, 235, 255));
    pen.rect(b, Color::from_rgba(235, 215, 190, 255));
    let Some((tex, uv)) = picture_uv(&picture_name(g.level), cols, rows) else {
        return;
    };
    if ghost(g.level) {
        let full = [
            vec2(uv[0], uv[1]),
            vec2(uv[2], uv[1]),
            vec2(uv[2], uv[3]),
            vec2(uv[0], uv[3]),
        ];
        quad_at(pen, tex, b, full, with_alpha(WHITE, 0.3));
    }
    for i in 0..cols {
        for j in 0..rows {
            let r = Rect::new(
                b.x + i as f32 * lay.cell,
                b.y + j as f32 * lay.cell,
                lay.cell,
                lay.cell,
            );
            pen.rect(
                Rect::new(r.x, r.y, r.w, 1.5),
                with_alpha(Color::from_rgba(150, 110, 70, 255), 0.35),
            );
            pen.rect(
                Rect::new(r.x, r.y, 1.5, r.h),
                with_alpha(Color::from_rgba(150, 110, 70, 255), 0.35),
            );
        }
    }
}

fn draw_piece(g: &Jigsaw, pen: &mut Pen, view: &View, i: usize, lay_cell: f32, lift: f32) {
    let (cols, rows) = GRIDS[g.level];
    let Some((tex, uv)) = picture_uv(&picture_name(g.level), cols, rows) else {
        return;
    };
    let p = &g.pieces[i];
    let pop = if p.placed { p.pop.scale(0.12) } else { 1.0 };
    let wob = (p.wobble * 30.0).sin() * 8.0 * p.wobble;
    let size = lay_cell * pop * (1.0 + lift);
    let c = p.pos + vec2(wob, -lift * lay_cell * 0.3);
    let r = Rect::new(c.x - size / 2.0, c.y - size / 2.0, size, size);
    if !p.placed {
        pen.soft_shadow(
            c + vec2(0.0, size * (0.42 + lift)),
            size * 0.46,
            size * 0.1,
            0.5,
        );
        let rim = r.scaled_about_center(1.04);
        pen.round_rect(rim, size * 0.08, Color::from_rgba(255, 255, 255, 255));
    }
    quad_at(pen, tex, r, cell_uv(uv, cols, rows, p.col, p.row), WHITE);
    let _ = view;
}

fn draw_pieces(g: &Jigsaw, pen: &mut Pen, view: &View) {
    let lay = layout(view, g.level);
    for i in 0..g.pieces.len() {
        if g.pieces[i].placed {
            draw_piece(g, pen, view, i, lay.cell, 0.0);
        }
    }
    let held = g.held.map(|h| h.0);
    let hint = g.hint_piece();
    for &i in &g.order {
        if g.pieces[i].placed {
            continue;
        }
        let lift = if Some(i) == held { 0.12 } else { 0.0 };
        if Some(i) == hint {
            let p = &g.pieces[i];
            let glow = lay.cell * (0.62 + 0.05 * (g.t * 6.0).sin());
            pen.round_rect(
                Rect::new(p.pos.x - glow, p.pos.y - glow, glow * 2.0, glow * 2.0),
                lay.cell * 0.2,
                Color::new(1.0, 0.95, 0.4, 0.7),
            );
        }
        draw_piece(g, pen, view, i, lay.cell, lift);
    }
    if let Some(i) = hint {
        let p = &g.pieces[i];
        let to = lay.cell_center(p.col, p.row);
        let ring = lay.cell * (0.5 + 0.06 * (g.t * 6.0).sin());
        pen.ring(to, ring, 5.0, Color::new(1.0, 0.95, 0.4, 0.9));
    }
}

fn draw_levels(g: &Jigsaw, pen: &mut Pen, view: &View) {
    let open = unlocked_level();
    pen.ribbon(view, "Pick a puzzle", vec2(view.w / 2.0, 90.0), g.t);
    for (i, r) in level_tiles(view).iter().enumerate() {
        let is_open = i <= open;
        if i == g.level {
            let glow = 8.0 + 3.0 * (g.t * 5.0).sin();
            pen.round_rect(
                Rect::new(r.x - glow, r.y - glow, r.w + glow * 2.0, r.h + glow * 2.0),
                34.0,
                Color::new(1.0, 0.95, 0.4, 0.8),
            );
        }
        let tint = if is_open { WHITE } else { with_alpha(WHITE, 0.55) };
        pen.painted_panel(*r, tint);
        let side = r.h * 0.5;
        let thumb = Rect::new(r.center().x - side / 2.0, r.y + r.h * 0.12, side, side);
        if let Some((tex, uv)) = picture_uv(&picture_name(i), 1, 1) {
            let full = [
                vec2(uv[0], uv[1]),
                vec2(uv[2], uv[1]),
                vec2(uv[2], uv[3]),
                vec2(uv[0], uv[3]),
            ];
            let t = if is_open {
                WHITE
            } else {
                Color::new(0.4, 0.4, 0.45, 0.9)
            };
            quad_at(pen, tex, thumb, full, t);
        }
        if is_open {
            pen.text_outline(
                &level_name(i),
                r.center().x,
                r.y + r.h * 0.72,
                r.h * 0.13,
                WHITE,
                BLUE_INK,
            );
            let earned = kg_core::profile::stars(GAME, i as u32);
            for j in 0..3u8 {
                let c = vec2(
                    r.center().x + (j as f32 - 1.0) * r.h * 0.22,
                    r.y + r.h * 0.88,
                );
                let fill = if j < earned {
                    STAR_GOLD
                } else {
                    Color::from_rgba(110, 90, 140, 150)
                };
                pen.star(c, r.h * 0.08, 0.0, fill);
            }
        } else {
            pen.sprite_at(
                "ui",
                "icon_lock",
                thumb.center(),
                r.h * 0.3,
                Draw::tint(WHITE),
            );
            pen.text_center(
                &level_name(i),
                r.center().x,
                r.y + r.h * 0.76,
                r.h * 0.11,
                with_alpha(INK, 0.6),
            );
        }
    }
    pen.painted_button(play_button(view), "Play", Tone::Green, 0.0, 1.0);
}

fn draw_report(g: &Jigsaw, pen: &mut Pen, view: &View, age: f32) {
    match g.report {
        Some(Report::Level { level, stars }) => {
            let title = format!("{} done!", level_name(level));
            let next = if level + 1 < LEVELS {
                format!("Next: {}", level_name(level + 1))
            } else {
                "Keep puzzling".to_string()
            };
            let sheet = pen.panel(view, &title, &["", &next], age);
            pen.panel_star_row(
                &sheet,
                age,
                stars,
                STAR_GOLD,
                Color::from_rgba(150, 140, 170, 140),
            );
        }
        Some(Report::Cleared { rank }) => {
            let total = kg_core::profile::total_stars(GAME);
            let line = format!("{total} stars earned");
            pen.results(
                view,
                &Results {
                    game: GAME,
                    title: "Puzzle master!",
                    line: &line,
                    rank,
                    action: "Keep playing",
                    format: stars_score,
                },
                age,
            );
        }
        None => {}
    }
}

pub fn draw(g: &Jigsaw, pen: &mut Pen, view: &View) {
    draw_backdrop(pen, view);
    if g.mode == Mode::Levels {
        draw_levels(g, pen, view);
        return;
    }
    draw_hud(g, pen, view);
    pen.offset = g.shake.offset();
    draw_board(g, pen, view);
    draw_pieces(g, pen, view);
    if let Mode::Done(t) = g.mode {
        let lay = layout(view, g.level);
        let k = ease_out_back((t / 0.4).clamp(0.0, 1.0));
        pen.text_outline(
            "Perfect!",
            view.w / 2.0,
            lay.board.y - 4.0 + lay.board.h / 2.0,
            lay.cell.min(90.0) * k,
            WHITE,
            BLUE_INK,
        );
    }
    if g.first_time && g.mode == Mode::Play {
        let lay = layout(view, g.level);
        let pulse = 1.0 + (g.t * 4.0).sin() * 0.03;
        let at = vec2(view.w / 2.0, lay.area.bottom() - 36.0);
        pen.hint("Drag a piece onto the picture", at, pulse);
    }
    pen.offset = Vec2::ZERO;
    pen.particles(&g.particles);
    pen.popups(&g.popups);
    if let Mode::Report(age) = g.mode {
        draw_report(g, pen, view, age);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fresh() -> JigsawPlay {
        kg_core::save::reset_for_test();
        kg_core::profile::create("Mia", 0);
        JigsawPlay::new()
    }

    fn drawn(p: &JigsawPlay, v: &View) -> Pen {
        let mut pen = Pen::default();
        pen.begin(*v, 1.0);
        p.draw(&mut pen, v);
        pen
    }

    #[test]
    fn jigsaw_sprites_all_exist() {
        let have = crate::art::names(SHEET);
        for n in sprite_names() {
            assert!(have.iter().any(|h| *h == n), "missing {n}");
        }
        assert!(crate::art::frame(SHEET, "icon_jigsaw").is_some());
        let ui = crate::art::names("ui");
        assert!(ui.iter().any(|h| h == "icon_lock"));
    }

    #[test]
    fn every_level_draws_in_both_orientations_and_on_a_phone() {
        for v in [
            View::ipad_landscape(),
            View::ipad_portrait(),
            View::fit(390.0, 844.0),
        ] {
            for level in 0..LEVELS {
                let mut p = fresh();
                kg_core::profile::unlock(GAME, level as u32);
                p.g.start_level(level);
                p.update(&Input::default(), &v, 1.0 / 60.0);
                let pen = drawn(&p, &v);
                let t: Vec<Tex> = pen.used().iter().filter_map(|b| b.tex).collect();
                assert!(t.iter().any(|t| matches!(t, Tex::Page("jigsaw", _))));
                assert!(t.contains(&Tex::Glyphs));
            }
        }
    }

    #[test]
    fn picker_and_both_reports_draw() {
        let v = View::ipad_landscape();
        let mut p = fresh();
        p.g.mode = Mode::Levels;
        assert!(drawn(&p, &v).vertex_count() > 300);
        p.g.report = Some(Report::Level { level: 0, stars: 2 });
        p.g.mode = Mode::Report(1.0);
        assert!(drawn(&p, &v).vertex_count() > 500);
        p.g.report = Some(Report::Cleared { rank: Some(0) });
        assert!(drawn(&p, &v).vertex_count() > 500);
    }

    #[test]
    fn solving_the_top_level_opens_the_board_report() {
        let v = View::ipad_landscape();
        let mut p = fresh();
        kg_core::profile::unlock(GAME, LEVELS as u32 - 1);
        p.g.start_level(LEVELS - 1);
        p.update(&Input::default(), &v, 1.0 / 60.0);
        let lay = layout(&v, LEVELS - 1);
        for i in 0..p.g.pieces.len() {
            let t = lay.cell_center(p.g.pieces[i].col, p.g.pieces[i].row);
            let from = p.g.pieces[i].pos;
            p.update(&Input::press_at(from), &v, 1.0 / 60.0);
            let mut mv = Input::press_at(t);
            mv.pressed = false;
            p.update(&mv, &v, 1.0 / 60.0);
            let mut up = Input::default();
            up.pos = t;
            up.released = true;
            p.update(&up, &v, 1.0 / 60.0);
        }
        for _ in 0..200 {
            p.update(&Input::default(), &v, 1.0 / 60.0);
        }
        assert!(cleared(&p.g));
        assert!(p.over());
        assert!(drawn(&p, &v).vertex_count() > 500);
    }
}
