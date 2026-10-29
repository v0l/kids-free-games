pub mod world;

use super::Play;
use crate::draw::{measure, Draw};
use crate::game3d::World3d;
use crate::paint::Pen;
use crate::ui::{Results, Tone};
use glam::{vec2, Vec2};
use kg_core::engine::{Game, Input, View};
use kg_core::fx::{ease_out_back, Side};
use kg_core::games::garden::*;
use kg_core::geom::{darker, with_alpha, Color, Rect, WHITE};
use kg_core::learn::{hud_bottom, ui_scale};

pub const KEY: &str = GAME;
pub const SHEET: &str = "garden";
const INK: Color = Color::from_rgba(40, 30, 60, 255);
const LEAF: Color = Color::from_rgba(20, 90, 40, 255);
const STAR_GOLD: Color = Color::from_rgba(255, 210, 60, 255);
const SEED_INK: Color = Color::from_rgba(110, 60, 20, 255);

pub struct GardenPlay {
    pub g: Garden,
}

impl GardenPlay {
    pub fn new() -> Self {
        Self { g: Garden::new() }
    }
}

impl Default for GardenPlay {
    fn default() -> Self {
        Self::new()
    }
}

impl Play for GardenPlay {
    fn update(&mut self, input: &Input, view: &View, dt: f32) {
        let showing = self.g.cleared();
        self.g.update(input, view, dt);
        if self.g.cleared() && !showing {
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
        self.g.cleared()
    }

    fn world(&self) -> World3d {
        World3d::Garden
    }

    fn as_any(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

fn draw_pips(g: &Garden, pen: &mut Pen, view: &View) {
    let k = ui_scale(view);
    let n = if g.stage == 2 {
        MASTER_RUN
    } else {
        STAGE_UP_RUN
    } as usize;
    let have = if g.stage == 2 { g.run } else { g.stage_run } as usize;
    let r = 11.0 * k;
    let gap = r * 2.7;
    let y = hud_bottom(view) + 92.0 * k;
    let x0 = view.w / 2.0 - gap * (n as f32 - 1.0) / 2.0;
    for i in 0..n {
        let c = vec2(x0 + i as f32 * gap, y);
        let on = i < have;
        let pop = if on && i + 1 == have {
            g.run_bump.scale(0.5)
        } else {
            1.0
        };
        pen.circle(c + vec2(0.0, 2.5), r * pop, Color::new(0.0, 0.0, 0.0, 0.2));
        pen.circle(
            c,
            r * pop,
            if on {
                STAR_GOLD
            } else {
                with_alpha(WHITE, 0.6)
            },
        );
        if on {
            pen.circle(
                c - vec2(r * 0.25, r * 0.3),
                r * 0.35,
                with_alpha(WHITE, 0.7),
            );
        }
    }
    pen.text_shadow(
        stage_name(g.stage),
        view.w / 2.0,
        y + 30.0 * k,
        22.0 * k,
        WHITE,
    );
}

fn draw_prompt(g: &Garden, pen: &mut Pen, view: &View) {
    let at = prompt_pos(view);
    let pop = ease_out_back(g.prompt_bump.t).max(0.2);
    let text = g.q.prompt();
    pen.text_outline(&text, at.x, at.y, prompt_size(view) * pop, WHITE, INK);
    let w = measure(&text, prompt_size(view)) / 2.0 + 56.0;
    pen.sprite_at(
        "ui",
        "icon_audio",
        vec2(at.x + w, at.y),
        44.0 * ui_scale(view),
        Draw::tint(with_alpha(WHITE, 0.95)),
    );
}

fn draw_seeds(g: &Garden, pen: &mut Pen, view: &View) {
    pen.offset = g.shake.offset();
    for (i, r) in seed_rects(view).iter().enumerate() {
        let v = g.q.options[i];
        let tried = g.tried[i];
        let right_now = g.solved.is_some() && v == g.q.answer;
        let hint = g.wrong >= 2 && v == g.q.answer && g.solved.is_none();
        let pop = g.press[i].scale(0.14);
        let wob = (g.wilt[i] * 34.0).sin() * 10.0 * g.wilt[i];
        let droop = g.wilt[i] * 0.16 * (g.t * 30.0).sin();
        let lift = if right_now {
            -(g.solved.unwrap_or(0.0) * 6.0).min(1.0) * 18.0
        } else {
            0.0
        };
        let c = r.center() + vec2(wob, lift);
        if hint {
            let glow = 14.0 + 5.0 * (g.t * 6.0).sin();
            pen.round_rect(
                Rect::new(r.x - glow, r.y - glow, r.w + glow * 2.0, r.h + glow * 2.0),
                40.0,
                Color::new(1.0, 0.95, 0.4, 0.75),
            );
        }
        pen.soft_shadow(vec2(c.x, r.bottom() + 6.0), r.w * 0.42, r.h * 0.05, 0.5);
        let tint = if tried {
            Color::new(0.55, 0.5, 0.5, 0.7)
        } else {
            WHITE
        };
        pen.sprite_at(
            SHEET,
            "seed",
            c,
            r.h * pop,
            Draw {
                tint,
                rotation: droop,
                ..Default::default()
            },
        );
        let label = v.to_string();
        let size = r.h * 0.3 * pop;
        let ink = if right_now { LEAF } else { SEED_INK };
        pen.text_outline(&label, c.x, c.y + r.h * 0.2, size, WHITE, ink);
    }
    pen.offset = Vec2::ZERO;
}

fn draw_beds(g: &Garden, pen: &mut Pen, view: &View) {
    pen.ribbon(view, "Pick a flower bed", vec2(view.w / 2.0, 90.0), g.t);
    let open = unlocked_level();
    for (i, r) in bed_tiles(view).iter().enumerate() {
        let can = i <= open;
        if i == g.level {
            let glow = 8.0 + 3.0 * (g.t * 5.0).sin();
            pen.round_rect(
                Rect::new(r.x - glow, r.y - glow, r.w + glow * 2.0, r.h + glow * 2.0),
                30.0,
                Color::new(1.0, 0.95, 0.4, 0.7),
            );
        }
        pen.round_rect(
            *r,
            26.0,
            Color::new(0.1, 0.25, 0.1, if can { 0.28 } else { 0.5 }),
        );
        let label = level_name(i);
        let size = (r.h * 0.17).min(r.w * 0.8 / measure(&label, 10.0) * 10.0);
        let ly = r.y + r.h * 0.66;
        if can {
            pen.text_outline(&label, r.center().x, ly, size, WHITE, LEAF);
            let earned = kg_core::profile::stars(GAME, i as u32);
            for j in 0..3u8 {
                let c = vec2(
                    r.center().x + (j as f32 - 1.0) * r.h * 0.17,
                    r.y + r.h * 0.86,
                );
                let fill = if j < earned {
                    STAR_GOLD
                } else {
                    Color::from_rgba(110, 90, 140, 150)
                };
                pen.star(c, r.h * 0.07, 0.0, fill);
            }
        } else {
            pen.sprite_at(
                "ui",
                "icon_lock",
                vec2(r.center().x, r.y + r.h * 0.36),
                r.h * 0.26,
                Draw::tint(with_alpha(WHITE, 0.9)),
            );
            pen.text_outline(
                &label,
                r.center().x,
                ly + 8.0,
                size * 0.9,
                with_alpha(WHITE, 0.75),
                darker(LEAF, 0.6),
            );
        }
    }
    pen.painted_button(play_button(view), "Play", Tone::Green, 0.0, 1.0);
}

fn draw_report(g: &Garden, pen: &mut Pen, view: &View, age: f32) {
    let stars_at = |pen: &mut Pen, stars: u8, y: f32| {
        for j in 0..3u8 {
            let k = ease_out_back(((age - 0.3 - j as f32 * 0.15) / 0.3).clamp(0.0, 1.0));
            let fill = if j < stars {
                STAR_GOLD
            } else {
                Color::from_rgba(150, 140, 170, 140)
            };
            pen.star(
                vec2(view.w / 2.0 + (j as f32 - 1.0) * 90.0, y),
                38.0 * k,
                0.0,
                fill,
            );
        }
    };
    let c = view.center() + vec2(0.0, 20.0);
    match g.report {
        Some(Report::Round { stars }) => {
            pen.panel(
                view,
                "Lovely planting!",
                &["", &g.summary, "Keep growing"],
                age,
            );
            let ph = 150.0 + 3.0 * 64.0;
            stars_at(pen, stars, c.y - ph / 2.0 + 118.0);
        }
        Some(Report::Bloom { level, stars }) => {
            let next = if level + 1 < LEVELS {
                format!("Next: {}", level_name(level + 1))
            } else {
                String::new()
            };
            let title = format!("{} bed blooms!", level_name(level));
            pen.panel(view, &title, &["", &next, "Keep growing"], age);
            let ph = 150.0 + 3.0 * 64.0;
            stars_at(pen, stars, c.y - ph / 2.0 + 118.0);
        }
        Some(Report::Cleared { rank }) => {
            let total = kg_core::profile::total_stars(GAME);
            let line = format!("{total} stars earned");
            pen.results(
                view,
                &Results {
                    game: GAME,
                    title: "Every bed is in bloom!",
                    line: &line,
                    rank,
                    action: "Keep growing",
                    format: stars_score,
                },
                age,
            );
        }
        None => {}
    }
}

pub fn draw(g: &Garden, pen: &mut Pen, view: &View) {
    if g.mode == Mode::Beds {
        draw_beds(g, pen, view);
        return;
    }
    let k = ui_scale(view);
    let beds = format!("{}/{}", beds_bloomed(), LEVELS);
    pen.chip(view, Side::Right, "BEDS", &beds, 1.0);
    let name = level_name(g.level);
    pen.chip(view, Side::Left, "", &name, 1.0);
    if g.mode == Mode::Play || matches!(g.report, Some(Report::Round { .. })) {
        draw_prompt(g, pen, view);
        draw_pips(g, pen, view);
        draw_seeds(g, pen, view);
        if !g.first_time {
            let b = beds_button(view);
            pen.painted_button(b, "Beds", Tone::Blue, 0.0, 1.0);
        }
        if g.first_time && g.mode == Mode::Play && g.solved.is_none() {
            let pulse = 1.0 + (g.t * 4.0).sin() * 0.03;
            let y = seed_rects(view)[0].y - 40.0 * k;
            pen.hint(
                "Count the flowers, then tap the right seed",
                vec2(view.w / 2.0, y),
                pulse,
            );
        }
    }
    pen.particles(&g.fx);
    pen.popups(&g.popups);
    if let Mode::Report(age) = g.mode {
        draw_report(g, pen, view, age);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paint::Tex;

    fn fresh() -> GardenPlay {
        kg_core::save::reset_for_test();
        kg_core::profile::create("Mia", 0);
        GardenPlay::new()
    }

    fn drawn(p: &GardenPlay, v: &View) -> Pen {
        let mut pen = Pen::default();
        pen.begin(*v, 1.0);
        p.draw(&mut pen, v);
        pen
    }

    #[test]
    fn garden_sprites_all_exist() {
        let have = crate::art::names(SHEET);
        for n in [
            "soil",
            "red",
            "yellow",
            "pink",
            "blue",
            "purple",
            "orange",
            "sprout",
            "butterfly",
            "bee",
            "can",
            "seed",
        ] {
            assert!(have.iter().any(|h| h == n), "missing {n}");
        }
        let ui = crate::art::names("ui");
        for n in ["icon_lock", "icon_audio"] {
            assert!(ui.iter().any(|h| h == n), "missing {n}");
        }
        assert!(crate::art::names("icons")
            .iter()
            .any(|h| h == "icon_garden"));
    }

    #[test]
    fn the_question_screen_draws_seeds_and_text_in_both_orientations() {
        for v in [View::ipad_landscape(), View::ipad_portrait()] {
            let pen = drawn(&fresh(), &v);
            let t: Vec<Tex> = pen.used().iter().filter_map(|b| b.tex).collect();
            assert!(t.iter().any(|t| matches!(t, Tex::Page("garden", _))));
            assert!(t.contains(&Tex::Glyphs));
        }
    }

    #[test]
    fn the_bed_picker_and_every_report_draw() {
        for v in [View::ipad_landscape(), View::ipad_portrait()] {
            let mut p = fresh();
            p.g.mode = Mode::Beds;
            assert!(drawn(&p, &v).vertex_count() > 400);
            p.g.mode = Mode::Report(1.0);
            for r in [
                Report::Round { stars: 2 },
                Report::Bloom { level: 0, stars: 3 },
                Report::Cleared { rank: Some(0) },
            ] {
                p.g.report = Some(r);
                assert!(drawn(&p, &v).vertex_count() > 400);
            }
        }
    }

    #[test]
    fn the_world_is_garden_and_the_game_ends_when_every_bed_blooms() {
        let mut p = fresh();
        assert_eq!(p.world(), World3d::Garden);
        assert!(!p.over());
        p.g.report = Some(Report::Cleared { rank: None });
        assert!(p.over());
    }

    #[test]
    fn a_tap_on_a_seed_answers_and_a_tap_on_the_prompt_repeats_it() {
        let v = View::ipad_landscape();
        let mut p = fresh();
        kg_core::sfx::take_voice();
        p.update(&Input::press_at(prompt_pos(&v)), &v, 1.0 / 60.0);
        assert!(kg_core::sfx::take_voice().is_some());
        let slot = p.g.q.slot_of_answer();
        p.update(
            &Input::press_at(seed_rects(&v)[slot].center()),
            &v,
            1.0 / 60.0,
        );
        assert!(p.g.solved.is_some());
    }
}
