use super::Play;
use crate::draw::Draw;
use crate::paint::Pen;
use crate::ui::Results;
use glam::{vec2, Vec2};
use kg_core::engine::{Game, Input, View};
use kg_core::fx::{ease_out_back, Bump};
use kg_core::games::pattern::*;
use kg_core::geom::{darker, with_alpha, Color, Rect, WHITE};
use kg_core::learn::{hud_bottom, Ladder, UP_STREAK};

pub const KEY: &str = GAME;
const SHEET: &str = "pattern";
const CLOUDS: [&str; 8] = [
    "cloud1", "cloud2", "cloud3", "cloud4", "cloud5", "cloud6", "cloud7", "cloud8",
];
const INK: Color = Color::from_rgba(40, 30, 60, 255);
const STAR_GOLD: Color = Color::from_rgba(255, 210, 60, 255);
const BLUE_INK: Color = Color::from_rgba(10, 70, 140, 255);

pub struct PatternPlay {
    pub g: Pattern,
}

impl PatternPlay {
    pub fn new() -> Self {
        Self { g: Pattern::new() }
    }
}

impl Default for PatternPlay {
    fn default() -> Self {
        Self::new()
    }
}

fn cleared(g: &Pattern) -> bool {
    matches!(g.mode, Mode::Report(_)) && matches!(g.report, Some(Report::Cleared { .. }))
}

impl Play for PatternPlay {
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
        self.g.ladder.stars as u32
    }

    fn over(&self) -> bool {
        cleared(&self.g)
    }

    fn as_any(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

fn stone_sprite(c: u8) -> String {
    format!("stone_{}", COLOURS[c as usize])
}

fn shape_sprite(s: u8) -> String {
    format!("shape_{}", SHAPES[s as usize])
}

pub fn sprite_names() -> Vec<String> {
    let mut v = vec!["bg".to_string(), "frog".to_string()];
    v.extend((0..COLOURS.len() as u8).map(stone_sprite));
    v.extend((0..SHAPES.len() as u8).map(shape_sprite));
    v
}

fn draw_token(pen: &mut Pen, token: Token, c: Vec2, size: f32) {
    let h = size * 0.92;
    let plain = |pen: &mut Pen| {
        pen.ui(
            "p_round",
            Rect::new(c.x - size / 2.0, c.y - size / 2.0, size, size),
            Draw::default(),
        )
    };
    match token {
        Token::Colour(k) => pen.sprite_at(SHEET, &stone_sprite(k), c, h, Draw::default()),
        Token::Shape(s) => {
            plain(pen);
            pen.sprite_at(SHEET, &shape_sprite(s), c, h * 0.62, Draw::default());
        }
        Token::Pair(k, s) => {
            pen.sprite_at(SHEET, &stone_sprite(k), c, h, Draw::default());
            pen.sprite_at(SHEET, &shape_sprite(s), c, h * 0.5, Draw::default());
        }
        Token::Number(n) => {
            plain(pen);
            pen.text_outline(&n.to_string(), c.x, c.y + size * 0.04, size * 0.46, WHITE, BLUE_INK);
        }
    }
}

fn draw_backdrop(pen: &mut Pen, view: &View, t: f32) {
    let f = view.full();
    let side = f.w.max(f.h);
    pen.sprite(
        SHEET,
        "bg",
        Rect::new(f.x + (f.w - side) / 2.0, f.y + f.h - side, side, side),
        Draw::default(),
    );
    for i in 0..5usize {
        let span = view.w + 500.0;
        let x = (i as f32 * 0.23 * span + t * (7.0 + i as f32 * 3.0)) % span - 250.0;
        let y = hud_bottom(view) + 10.0 + (i as f32 * 0.37 % 1.0) * view.h * 0.12;
        pen.sprite_w(
            "ui",
            CLOUDS[(i * 2 + 1) % CLOUDS.len()],
            vec2(x, y),
            150.0 + (i % 3) as f32 * 50.0,
            Draw::tint(with_alpha(WHITE, 0.9)),
        );
    }
}

pub fn draw_hud(
    pen: &mut Pen,
    view: &View,
    h: &kg_core::learn::Hud,
    ladder: &Ladder,
    label: &str,
    accent: Color,
    bump: &Bump,
) {
    pen.hud_title(view, h, label, darker(accent, 0.6));
    let r = h.star_r;
    for (i, c) in h.stars.iter().enumerate().take(ladder.levels) {
        pen.star(
            *c + vec2(0.0, 2.0),
            r * 1.1,
            0.0,
            Color::new(0.1, 0.05, 0.25, 0.35),
        );
        let fill = if i < ladder.stars {
            STAR_GOLD
        } else {
            with_alpha(WHITE, 0.55)
        };
        pen.star(*c, r, 0.0, fill);
    }
    let Some(bar) = h.progress else { return };
    let gap = (bar.h * 0.45).max(4.0);
    let seg = (bar.w - gap * (UP_STREAK as f32 - 1.0)) / UP_STREAK as f32;
    for j in 0..UP_STREAK {
        let r = Rect::new(bar.x + j as f32 * (seg + gap), bar.y, seg, bar.h);
        let filled = j < ladder.streak;
        let last = filled && j + 1 == ladder.streak;
        let sc = if last { bump.scale(0.35) } else { 1.0 };
        let rr = Rect::new(
            r.x + r.w * (1.0 - sc) / 2.0,
            r.y + r.h * (1.0 - sc) / 2.0,
            r.w * sc,
            r.h * sc,
        );
        pen.round_rect(
            Rect::new(rr.x, rr.y + 3.0, rr.w, rr.h),
            rr.h / 2.0,
            Color::new(0.0, 0.0, 0.0, 0.15),
        );
        let col = if filled {
            STAR_GOLD
        } else {
            with_alpha(WHITE, 0.5)
        };
        pen.round_rect(rr, rr.h / 2.0, col);
        if filled {
            pen.gloss(rr, rr.h / 2.0);
        }
    }
}

fn draw_levels(g: &Pattern, pen: &mut Pen, view: &View) {
    let unlocked = g.unlocked_level();
    pen.ribbon(view, "Pick a level", vec2(view.w / 2.0, 90.0), g.t);
    for (i, r) in level_tiles(view).iter().enumerate() {
        let open = i <= unlocked;
        if i == g.ladder.level {
            let glow = 8.0 + 3.0 * (g.t * 5.0).sin();
            pen.round_rect(
                Rect::new(r.x - glow, r.y - glow, r.w + glow * 2.0, r.h + glow * 2.0),
                34.0,
                Color::new(1.0, 0.95, 0.4, 0.8),
            );
        }
        let tint = if open { WHITE } else { with_alpha(WHITE, 0.55) };
        pen.painted_panel(*r, tint);
        let label = level_name(i);
        if open {
            pen.text_outline(
                &label,
                r.center().x,
                r.y + r.h * 0.34,
                r.h * 0.17,
                WHITE,
                BLUE_INK,
            );
            let earned = kg_core::profile::stars(GAME, i as u32);
            for j in 0..3u8 {
                let c = vec2(
                    r.center().x + (j as f32 - 1.0) * r.h * 0.26,
                    r.y + r.h * 0.72,
                );
                let fill = if j < earned {
                    STAR_GOLD
                } else {
                    Color::from_rgba(110, 90, 140, 150)
                };
                pen.star(c, r.h * 0.11, 0.0, fill);
            }
        } else {
            pen.sprite_at(
                "ui",
                "icon_lock",
                r.center() - vec2(0.0, r.h * 0.05),
                r.h * 0.34,
                Draw::tint(INK),
            );
            pen.text_center(
                &label,
                r.center().x,
                r.y + r.h * 0.8,
                r.h * 0.12,
                with_alpha(INK, 0.6),
            );
        }
    }
    pen.painted_button(play_button(view), "Play", crate::ui::Tone::Green, 0.0, 1.0);
}

fn draw_report(g: &Pattern, pen: &mut Pen, view: &View, age: f32) {
    match g.report {
        Some(Report::Level { level, stars }) => {
            let title = format!("{} done!", level_name(level));
            let next = if level + 1 < LEVELS {
                format!("Next: {}", level_name(level + 1))
            } else {
                "Pattern champion!".to_string()
            };
            let sheet = pen.panel(view, &title, &["", &next, &g.summary], age);
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
                    title: "You are a pattern master!",
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

fn draw_prompt(g: &Pattern, pen: &mut Pen, view: &View) {
    let pop = ease_out_back(g.prompt_bump.t).max(0.2);
    pen.text_outline(
        "What comes next?",
        view.w / 2.0,
        prompt_y(view),
        prompt_size(view) * pop,
        WHITE,
        INK,
    );
}

fn draw_path(g: &Pattern, pen: &mut Pen, view: &View) {
    let q = &g.question;
    let cells = stone_cells(view, q.seq.len());
    let area = path_area(view);
    pen.painted_panel(
        Rect::new(area.x - 16.0, area.y - 10.0, area.w + 32.0, area.h + 20.0),
        with_alpha(WHITE, 0.3),
    );
    let last = q.seq.len() - 1;
    let frog = g.frog_stone();
    for (i, cell) in cells.iter().enumerate() {
        let intro = ease_out_back((g.t * 2.5 - i as f32 * 0.12).clamp(0.0, 1.0));
        let c = cell.center();
        let size = cell.w * intro;
        pen.soft_shadow(c + vec2(0.0, cell.w * 0.35), cell.w * 0.45, cell.w * 0.1, 0.5);
        if i == last && g.celebrate.is_none() {
            let wob = 1.0 + (g.t * 4.0).sin() * 0.05;
            pen.round_rect(
                Rect::new(
                    c.x - size * 0.4 * wob,
                    c.y - size * 0.4 * wob,
                    size * 0.8 * wob,
                    size * 0.8 * wob,
                ),
                size * 0.4,
                with_alpha(WHITE, 0.55),
            );
            pen.text_outline("?", c.x, c.y, size * 0.55, WHITE, BLUE_INK);
        } else {
            draw_token(pen, q.seq[i], c, size);
        }
    }
    if g.celebrate.is_some() {
        let c = cells[frog].center();
        let hop = ((g.t * 14.0).sin().abs()) * cells[frog].w * 0.15;
        pen.sprite_at(
            SHEET,
            "frog",
            c - vec2(0.0, cells[frog].w * 0.55 + hop),
            cells[frog].w * 0.7,
            Draw::default(),
        );
    }
}

fn draw_buttons(g: &Pattern, pen: &mut Pen, view: &View) {
    use crate::ui::Tone;
    let rects = button_rects(view, g.question.options.len());
    for (i, (r, token)) in rects.iter().zip(&g.question.options).enumerate() {
        let tried = g.attempt.tried[i];
        let right = *token == g.question.answer();
        let hint = g.attempt.revealed() && right;
        let pop = g.press[i].scale(0.12);
        let wob = (g.wobble[i] * 36.0).sin() * 12.0 * g.wobble[i];
        let c = r.center() + vec2(wob, 0.0);
        let rr = Rect::new(
            c.x - r.w * pop / 2.0,
            c.y - r.h * pop / 2.0,
            r.w * pop,
            r.h * pop,
        );
        if hint {
            let glow = 10.0 + 4.0 * (g.t * 6.0).sin();
            pen.round_rect(
                Rect::new(
                    rr.x - glow,
                    rr.y - glow,
                    rr.w + glow * 2.0,
                    rr.h + glow * 2.0,
                ),
                24.0,
                Color::new(1.0, 0.95, 0.4, 0.7),
            );
        }
        let tone = if g.attempt.solved && right {
            Tone::Green
        } else {
            Tone::Blue
        };
        let alpha = if tried { 0.45 } else { 1.0 };
        pen.painted_button(rr, "", tone, 0.0, alpha);
        draw_token(pen, *token, rr.center(), rr.h.min(rr.w) * 0.8);
    }
}

pub fn draw(g: &Pattern, pen: &mut Pen, view: &View) {
    draw_backdrop(pen, view, g.t);
    if g.mode == Mode::Levels {
        draw_levels(g, pen, view);
        return;
    }
    let h = hud(view);
    draw_hud(
        pen,
        view,
        &h,
        &g.ladder,
        &level_name(g.ladder.level),
        Color::from_rgba(60, 170, 230, 255),
        &g.streak_bump,
    );
    pen.offset = g.shake.offset();
    pen.hud_goal(view, &h, &g.goal_text(), g.first_time, WHITE, INK);
    if let (false, Some(b)) = (g.first_time, h.levels) {
        pen.painted_button(b, "Levels", crate::ui::Tone::Blue, 0.0, 1.0);
    }
    draw_prompt(g, pen, view);
    draw_path(g, pen, view);
    draw_buttons(g, pen, view);
    if g.first_time && g.mode == Mode::Play {
        let pulse = 1.0 + (g.t * 4.0).sin() * 0.03;
        pen.hint(
            "Find the pattern, then pick the next stone",
            hint_pos(view, g.question.options.len()),
            pulse,
        );
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
    use crate::paint::Tex;

    fn fresh() -> PatternPlay {
        kg_core::save::reset_for_test();
        kg_core::profile::create("Mia", 0);
        PatternPlay::new()
    }

    fn drawn(p: &PatternPlay, v: &View) -> Pen {
        let mut pen = Pen::default();
        pen.begin(*v, 1.0);
        p.draw(&mut pen, v);
        pen
    }

    #[test]
    fn pattern_sprites_all_exist() {
        let have = crate::art::names(SHEET);
        for n in sprite_names() {
            assert!(have.iter().any(|h| *h == n), "missing {n}");
        }
        let ui = crate::art::names("ui");
        for n in ["icon_lock", "p_round", "cloud1", "cloud2", "cloud6", "cloud8"] {
            assert!(ui.iter().any(|h| h == n), "missing {n}");
        }
        assert!(crate::art::frame("icons", "icon_pattern").is_some());
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
                let pen = drawn(&p, &v);
                let t: Vec<Tex> = pen.used().iter().filter_map(|b| b.tex).collect();
                assert!(t.iter().any(|t| matches!(t, Tex::Page("pattern", _))));
                assert!(t.contains(&Tex::Glyphs));
            }
        }
    }

    #[test]
    fn the_level_picker_draws_in_both_orientations() {
        for v in [View::ipad_landscape(), View::ipad_portrait()] {
            let mut p = fresh();
            p.g.mode = Mode::Levels;
            assert!(drawn(&p, &v).vertex_count() > 500);
        }
    }

    #[test]
    fn both_reports_draw() {
        let v = View::ipad_landscape();
        let mut p = fresh();
        p.g.report = Some(Report::Level { level: 0, stars: 2 });
        p.g.mode = Mode::Report(1.0);
        assert!(drawn(&p, &v).vertex_count() > 800);
        p.g.report = Some(Report::Cleared { rank: Some(0) });
        assert!(drawn(&p, &v).vertex_count() > 800);
    }

    #[test]
    fn clearing_the_top_level_opens_the_board_report() {
        let v = View::ipad_landscape();
        let mut p = fresh();
        let last = LEVELS - 1;
        kg_core::profile::unlock(GAME, last as u32);
        p.g.start_level(last);
        for _ in 0..UP_STREAK {
            let slot = p.g.question.slot_of_answer();
            let tap = p.g.pick(slot);
            p.g.react(&v, slot, tap);
            for _ in 0..400 {
                p.update(&Input::default(), &v, 1.0 / 60.0);
            }
        }
        assert!(cleared(&p.g));
        assert!(p.over());
        assert!(drawn(&p, &v).vertex_count() > 800);
    }
}
