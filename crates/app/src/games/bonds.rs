use super::Play;
use crate::draw::Draw;
use crate::paint::Pen;
use crate::ui::{Results, Tone};
use glam::{vec2, Vec2};
use kg_core::engine::{Game, Input, View};
use kg_core::fx::ease_out_back;
use kg_core::games::bonds::*;
use kg_core::geom::{with_alpha, Color, Rect, WHITE};

pub const KEY: &str = GAME;
pub const SHEET: &str = "bonds";
const INK: Color = Color::from_rgba(40, 30, 60, 255);
const STAR_GOLD: Color = Color::from_rgba(255, 210, 60, 255);
const BLUE_INK: Color = Color::from_rgba(10, 70, 140, 255);
const ACCENT: Color = Color::from_rgba(70, 150, 230, 255);
pub const SPRITES: [&str; 9] = [
    "bg", "stand", "beam", "pan", "w_red", "w_blue", "w_green", "w_orange", "ball",
];

pub struct BondsPlay {
    pub g: Bonds,
}

impl BondsPlay {
    pub fn new() -> Self {
        Self { g: Bonds::new() }
    }
}

impl Default for BondsPlay {
    fn default() -> Self {
        Self::new()
    }
}

fn cleared(g: &Bonds) -> bool {
    matches!(g.mode, Mode::Report(_)) && matches!(g.report, Some(Report::Cleared { .. }))
}

impl Play for BondsPlay {
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

fn draw_backdrop(pen: &mut Pen, view: &View) {
    let f = view.full();
    let side = f.w.max(f.h);
    pen.sprite(
        SHEET,
        "bg",
        Rect::new(f.x + (f.w - side) / 2.0, f.y + f.h - side, side, side),
        Draw::default(),
    );
}

fn equation(g: &Bonds) -> String {
    let q = &g.question;
    if g.attempt.solved {
        format!("{} + {} = {}", q.a, q.answer(), q.target)
    } else {
        format!("{} + ? = {}", q.a, q.target)
    }
}

fn draw_prompt(g: &Bonds, pen: &mut Pen, view: &View) {
    let size = prompt_size(view);
    let pop = ease_out_back(g.prompt_bump.t).max(0.3);
    let text = equation(g);
    pen.text_outline(&text, view.w / 2.0, prompt_y(view), size * pop, WHITE, INK);
}

fn weight_sprite(value: i32) -> &'static str {
    SPRITES[4 + (value.rem_euclid(4)) as usize]
}

fn draw_weight(pen: &mut Pen, name: &str, c: Vec2, size: f32, label: &str) {
    pen.sprite_at(SHEET, name, c, size, Draw::default());
    pen.text_outline(label, c.x, c.y + size * 0.1, size * 0.4, WHITE, INK);
}

fn draw_pan(pen: &mut Pen, end: Vec2, pan_w: f32) {
    pen.sprite_w(
        SHEET,
        "pan",
        end + vec2(0.0, pan_w * 0.72),
        pan_w,
        Draw::default(),
    );
}

fn draw_scale(g: &Bonds, pen: &mut Pen, view: &View) {
    let geo = scale_geo(view, g.tilt);
    let q = &g.question;
    let stand_h = (geo.bottom() - geo.pivot.y) * 1.1 + geo.arm * 0.5;
    pen.sprite_at(
        SHEET,
        "stand",
        vec2(geo.pivot.x, geo.pivot.y + stand_h * 0.4),
        stand_h,
        Draw::default(),
    );
    pen.sprite_w(SHEET, "beam", geo.pivot, geo.arm * 2.1, Draw::rot(-g.tilt));
    for end in [geo.left_end, geo.right_end] {
        draw_pan(pen, end, geo.pan_w);
    }
    let left = geo.left_dish();
    let right = geo.right_dish();
    let r = geo.pan_w * 0.085;
    if beads(q.level) {
        for p in bead_spots(q.a as usize, left, geo.pan_w) {
            pen.sprite_at(SHEET, "ball", p, r * 2.2, Draw::default());
        }
        if let Some(v) = g.on_pan {
            for p in bead_spots(v as usize, right, geo.pan_w) {
                pen.sprite_at(SHEET, "ball", p, r * 2.2, Draw::default());
            }
        }
    } else {
        let s = geo.pan_w * 0.5;
        draw_weight(
            pen,
            weight_sprite(q.a),
            left - vec2(0.0, s * 0.45),
            s,
            &q.a.to_string(),
        );
        if let Some(v) = g.on_pan {
            draw_weight(
                pen,
                weight_sprite(v),
                right - vec2(0.0, s * 0.45),
                s,
                &v.to_string(),
            );
        }
    }
}

fn draw_buttons(g: &Bonds, pen: &mut Pen, view: &View) {
    let rects = button_rects(view);
    let glow_slot = g.hint_slot();
    for (i, (r, v)) in rects.iter().zip(&g.question.options).enumerate() {
        let tried = g.attempt.tried[i];
        let pop = g.press[i].scale(0.12);
        let wob = (g.wobble[i] * 36.0).sin() * 12.0 * g.wobble[i];
        let c = r.center() + vec2(wob, 0.0);
        let rr = Rect::new(
            c.x - r.w * pop / 2.0,
            c.y - r.h * pop / 2.0,
            r.w * pop,
            r.h * pop,
        );
        if glow_slot == Some(i) {
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
        let correct_now = g.attempt.solved && *v == g.question.answer();
        let tone = if correct_now {
            Tone::Green
        } else {
            [Tone::Orange, Tone::Blue, Tone::Green][i % 3]
        };
        let alpha = if tried { 0.45 } else { 1.0 };
        pen.painted_button(rr, &v.to_string(), tone, 0.0, alpha);
    }
}

fn draw_levels(g: &Bonds, pen: &mut Pen, view: &View) {
    draw_backdrop(pen, view);
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
        pen.painted_panel(*r, if open { WHITE } else { with_alpha(WHITE, 0.55) });
        if open {
            pen.text_outline(
                &(i + 1).to_string(),
                r.x + r.w * 0.17,
                r.y + r.h * 0.24,
                r.h * 0.2,
                WHITE,
                ACCENT,
            );
            let ex = level_example(i);
            let size = (r.h * 0.26).min(r.w * 0.86 / ex.len() as f32 * 1.7);
            pen.text_outline(ex, r.center().x, r.y + r.h * 0.46, size, WHITE, BLUE_INK);
            let earned = kg_core::profile::stars(GAME, i as u32);
            for j in 0..3u8 {
                let c = vec2(
                    r.center().x + (j as f32 - 1.0) * r.h * 0.24,
                    r.y + r.h * 0.76,
                );
                let fill = if j < earned {
                    STAR_GOLD
                } else {
                    Color::from_rgba(110, 90, 140, 150)
                };
                pen.star(c, r.h * 0.1, 0.0, fill);
            }
        } else {
            pen.sprite_at(
                "ui",
                "icon_lock",
                r.center() - vec2(0.0, r.h * 0.05),
                r.h * 0.34,
                Draw::tint(INK),
            );
        }
    }
    pen.painted_button(play_button(view), "Play", Tone::Green, 0.0, 1.0);
}

fn draw_report(g: &Bonds, pen: &mut Pen, view: &View, age: f32) {
    match g.report {
        Some(Report::Level { level, stars }) => {
            let title = format!("Level {} done!", level + 1);
            let next = if level + 1 < LEVELS {
                format!("Next: {}", level_name(level + 1))
            } else {
                "Bonds star!".to_string()
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
                    title: "Bonds star!",
                    line: &line,
                    rank,
                    action: "Keep going",
                    format: stars_score,
                },
                age,
            );
        }
        None => {}
    }
}

pub fn draw(g: &Bonds, pen: &mut Pen, view: &View) {
    if g.mode == Mode::Levels {
        draw_levels(g, pen, view);
        return;
    }
    draw_backdrop(pen, view);
    let h = hud(view);
    crate::games::count::draw_hud(
        pen,
        view,
        &h,
        &g.ladder,
        &format!("Level {}", g.ladder.level + 1),
        ACCENT,
        &g.streak_bump,
    );
    pen.offset = g.shake.offset() * 0.5;
    crate::games::count::draw_goal(pen, view, &h, &g.goal_text(), g.first_time);
    draw_prompt(g, pen, view);
    draw_scale(g, pen, view);
    draw_buttons(g, pen, view);
    if g.first_time && g.mode == Mode::Play && hint_room(view) {
        let pulse = 1.0 + (g.t * 4.0).sin() * 0.03;
        pen.hint(level_hint(g.ladder.level), hint_pos(view), pulse);
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

    fn fresh() -> BondsPlay {
        kg_core::save::reset_for_test();
        kg_core::profile::create("Mia", 0);
        BondsPlay::new()
    }

    fn drawn(p: &BondsPlay, v: &View) -> Pen {
        let mut pen = Pen::default();
        pen.begin(*v, 1.0);
        p.draw(&mut pen, v);
        pen
    }

    #[test]
    fn bonds_sprites_all_exist() {
        let have = crate::art::names(SHEET);
        for n in SPRITES {
            assert!(have.iter().any(|h| h == n), "missing {n}");
        }
        let ui = crate::art::names("ui");
        for n in ["icon_lock", "icon_audio"] {
            assert!(ui.iter().any(|h| h == n), "missing {n}");
        }
        assert!(crate::art::frame("icons", "icon_bonds").is_some());
    }

    #[test]
    fn the_play_screen_draws_in_every_shape_of_screen() {
        for v in [
            View::ipad_landscape(),
            View::ipad_portrait(),
            View::fit(390.0, 844.0),
            View::fit(844.0, 390.0),
        ] {
            let mut p = fresh();
            for lvl in [0, 2, 5] {
                p.g.start_level(lvl);
                let pen = drawn(&p, &v);
                assert!(pen.vertex_count() > 300);
                let t: Vec<Tex> = pen.used().iter().filter_map(|b| b.tex).collect();
                assert!(t.iter().any(|t| matches!(t, Tex::Page("bonds", _))));
                assert!(t.contains(&Tex::Glyphs));
            }
        }
    }

    #[test]
    fn the_level_picker_and_both_reports_draw() {
        for v in [View::ipad_landscape(), View::ipad_portrait()] {
            let mut p = fresh();
            p.g.mode = Mode::Levels;
            assert!(drawn(&p, &v).vertex_count() > 500);
            p.g.mode = Mode::Report(1.0);
            p.g.report = Some(Report::Level { level: 0, stars: 2 });
            assert!(drawn(&p, &v).vertex_count() > 800);
            p.g.report = Some(Report::Cleared { rank: Some(0) });
            assert!(drawn(&p, &v).vertex_count() > 800);
        }
    }
}
