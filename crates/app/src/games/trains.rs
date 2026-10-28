pub mod world;

use super::Play;
use crate::draw::Draw;
use crate::game3d::World3d;
use crate::paint::Pen;
use crate::ui::{Results, Tone};
use glam::{vec2, vec3, Vec2};
use kg_core::engine::{Game, Input, View, HUD_LEFT};
use kg_core::fx::ease_out_back;
use kg_core::games::trains::*;
use kg_core::geom::{with_alpha, Color, Rect, WHITE};
use kg_core::learn::ui_scale;

pub const KEY: &str = GAME;
pub const SHEET: &str = "trains";
const INK: Color = Color::from_rgba(40, 30, 60, 255);
const STAR_GOLD: Color = Color::from_rgba(255, 210, 60, 255);
const BLUE_INK: Color = Color::from_rgba(10, 70, 140, 255);
const TRAIN_RED: Color = Color::from_rgba(225, 70, 60, 255);

pub struct TrainsPlay {
    pub g: Trains,
}

impl TrainsPlay {
    pub fn new() -> Self {
        Self { g: Trains::new() }
    }
}

impl Default for TrainsPlay {
    fn default() -> Self {
        Self::new()
    }
}

fn cleared(g: &Trains) -> bool {
    matches!(g.mode, Mode::Report(_)) && matches!(g.report, Some(Report::Cleared { .. }))
}

impl Play for TrainsPlay {
    fn update(&mut self, input: &Input, view: &View, dt: f32) {
        let showing = cleared(&self.g);
        self.g.update(input, view, dt);
        for (at, c) in std::mem::take(&mut self.g.bursts) {
            if let Some(p) = self.g.cam_of(view).project(view, at + vec3(0.0, 0.5, 0.0)) {
                self.g.particles.sparkle(p, c);
            }
        }
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

    fn world(&self) -> World3d {
        World3d::Trains
    }

    fn as_any(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

fn draw_goal(g: &Trains, pen: &mut Pen, view: &View) {
    let k = ui_scale(view);
    let y = goal_row_y(view);
    let button = levels_button(view);
    let room = if g.first_time {
        view.w - 2.0 * HUD_LEFT
    } else {
        button.x - 20.0 - HUD_LEFT
    };
    let text = g.goal_text();
    let size = (26.0 * k).min(room / text.len() as f32 * 2.0);
    let left = if g.first_time {
        view.w / 2.0
    } else {
        HUD_LEFT + room / 2.0
    };
    pen.text_outline(&text, left, y, size, WHITE, INK);
    if !g.first_time {
        pen.painted_button(button, "Levels", Tone::Blue, 0.0, 1.0);
    }
}

fn equation(g: &Trains) -> String {
    let q = &g.question;
    if g.attempt.solved {
        q.solved_text()
    } else if missing_form(q.level) {
        q.sum_text()
    } else {
        format!("{} = ?", q.sum_text())
    }
}

fn draw_prompt(g: &Trains, pen: &mut Pen, view: &View) {
    let r = prompt_rect(view);
    let pop = ease_out_back(g.prompt_bump.t).max(0.3);
    let c = r.center();
    let rr = Rect::new(
        c.x - r.w * pop / 2.0,
        c.y - r.h * pop / 2.0,
        r.w * pop,
        r.h * pop,
    );
    pen.painted_panel(rr, with_alpha(WHITE, 0.94));
    let text = equation(g);
    let size = (rr.h * 0.5).min(rr.w * 0.9 / text.len().max(4) as f32 * 1.8);
    pen.text_outline(&text, c.x, c.y - rr.h * 0.02, size, WHITE, BLUE_INK);
    let ear = Vec2::new(rr.x + rr.w - rr.h * 0.42, c.y);
    pen.painted_round(
        Rect::new(ear.x - rr.h * 0.3, ear.y - rr.h * 0.3, rr.h * 0.6, rr.h * 0.6),
        "icon_audio",
        0.0,
    );
}

fn draw_left(g: &Trains, pen: &mut Pen, view: &View) {
    if !pictures(g.ladder.level) || !matches!(g.phase, Phase::Remove | Phase::Choose) {
        return;
    }
    let cam = g.cam_of(view);
    let top = kg_core::games::trains::seat(0, g.riders.len());
    let anchor = vec3(g.train_center(), top.y + 2.4, 0.0);
    let Some(p) = cam.project(view, anchor) else {
        return;
    };
    let left = g.aboard();
    let k = ui_scale(view);
    let r = 30.0 * k;
    let pulse = g.streak_bump.scale(0.25);
    pen.ui(
        "p_round",
        Rect::new(
            p.x - r * 1.2 * pulse,
            p.y - r * 1.2 * pulse,
            r * 2.4 * pulse,
            r * 2.4 * pulse,
        ),
        Draw::default(),
    );
    let shown = if g.hide_k() > 0.5 {
        "?".to_string()
    } else {
        left.to_string()
    };
    pen.text_outline(&shown, p.x, p.y - r * 0.05, r * 1.35, WHITE, BLUE_INK);
}

fn draw_hint(g: &Trains, pen: &mut Pen, view: &View) {
    let Some(i) = g.hint_rider() else {
        return;
    };
    let cam = g.cam_of(view);
    let at = g.rider_world(i) + vec3(0.0, 1.9, 0.0);
    let Some(p) = cam.project(view, at) else {
        return;
    };
    let bob = (g.t * 6.0).sin() * 10.0;
    let k = ui_scale(view);
    let s = 24.0 * k;
    let tip = p + vec2(0.0, bob);
    pen.tri(
        tip + vec2(-s * 0.2, 4.0),
        tip + vec2(s * 0.2, 4.0),
        tip + vec2(0.0, s * 0.8 + 4.0),
        Color::new(0.1, 0.05, 0.25, 0.35),
    );
    pen.tri(
        tip + vec2(-s * 0.8, -s * 0.9),
        tip + vec2(s * 0.8, -s * 0.9),
        tip + vec2(0.0, s * 0.3),
        STAR_GOLD,
    );
    if g.first_time {
        let pulse = 1.0 + (g.t * 4.0).sin() * 0.03;
        let y = button_rects(view)[0].y - 40.0;
        pen.hint(
            "Tap the riders to hop them off",
            vec2(view.w / 2.0, y),
            pulse,
        );
    }
}

fn draw_buttons(g: &Trains, pen: &mut Pen, view: &View) {
    if !matches!(g.phase, Phase::Choose | Phase::Resolve) {
        return;
    }
    let slide = match g.phase {
        Phase::Choose => ease_out_back((g.phase_t / 0.4).min(1.0)).clamp(0.0, 1.1),
        _ => 1.0,
    };
    let rects = button_rects(view);
    let glow_slot = g.glow_slot();
    for (i, (r, v)) in rects.iter().zip(&g.question.options).enumerate() {
        let tried = g.attempt.tried[i];
        let pop = g.press[i].scale(0.12);
        let wob = (g.wobble[i] * 36.0).sin() * 12.0 * g.wobble[i];
        let drop = (1.0 - slide) * 160.0;
        let c = r.center() + vec2(wob, drop);
        let rr = Rect::new(
            c.x - r.w * pop / 2.0,
            c.y - r.h * pop / 2.0,
            r.w * pop,
            r.h * pop,
        );
        if glow_slot == Some(i) {
            let glow = 10.0 + 4.0 * (g.t * 6.0).sin();
            pen.round_rect(
                Rect::new(rr.x - glow, rr.y - glow, rr.w + glow * 2.0, rr.h + glow * 2.0),
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

fn draw_levels(g: &Trains, pen: &mut Pen, view: &View) {
    let f = view.full();
    pen.sky(
        view,
        Color::from_rgba(120, 200, 250, 255),
        Color::from_rgba(190, 235, 170, 255),
    );
    let side = f.w.max(f.h);
    pen.sprite(
        SHEET,
        "bg",
        Rect::new(f.x + (f.w - side) / 2.0, f.y + f.h - side, side, side),
        Draw::default(),
    );
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
                TRAIN_RED,
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

fn draw_report(g: &Trains, pen: &mut Pen, view: &View, age: f32) {
    match g.report {
        Some(Report::Level { level, stars }) => {
            let title = format!("Level {} done!", level + 1);
            let next = if level + 1 < LEVELS {
                format!("Next: {}", level_name(level + 1))
            } else {
                "Subtraction star!".to_string()
            };
            pen.panel(view, &title, &["", &next, &g.summary], age);
            let c = view.center() + vec2(0.0, 20.0);
            let ph = 150.0 + 3.0 * 64.0;
            let y = c.y - ph / 2.0 + 118.0;
            for j in 0..3u8 {
                let k = ease_out_back(((age - 0.3 - j as f32 * 0.15) / 0.3).clamp(0.0, 1.0));
                let fill = if j < stars {
                    STAR_GOLD
                } else {
                    Color::from_rgba(150, 140, 170, 140)
                };
                pen.star(vec2(c.x + (j as f32 - 1.0) * 90.0, y), 38.0 * k, 0.0, fill);
            }
        }
        Some(Report::Cleared { rank }) => {
            let total = kg_core::profile::total_stars(GAME);
            let line = format!("{total} stars earned");
            pen.results(
                view,
                &Results {
                    game: GAME,
                    title: "Subtraction star!",
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

pub fn draw(g: &Trains, pen: &mut Pen, view: &View) {
    if g.mode == Mode::Levels {
        draw_levels(g, pen, view);
        return;
    }
    crate::games::count::draw_hud(
        pen,
        view,
        &g.ladder,
        &format!("Level {}", g.ladder.level + 1),
        TRAIN_RED,
        &g.streak_bump,
    );
    pen.offset = g.shake.offset() * 0.5;
    draw_goal(g, pen, view);
    draw_prompt(g, pen, view);
    draw_left(g, pen, view);
    draw_hint(g, pen, view);
    draw_buttons(g, pen, view);
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

    fn fresh() -> TrainsPlay {
        kg_core::save::reset_for_test();
        kg_core::profile::create("Mia", 0);
        TrainsPlay::new()
    }

    fn drawn(p: &TrainsPlay, v: &View) -> Pen {
        let mut pen = Pen::default();
        pen.begin(*v, 1.0);
        p.draw(&mut pen, v);
        pen
    }

    #[test]
    fn trains_sprites_all_exist() {
        let have = crate::art::names(SHEET);
        for n in SPRITES.iter().chain(["gravel"].iter()) {
            assert!(have.iter().any(|h| h == n), "missing {n}");
        }
        let ui = crate::art::names("ui");
        for n in ["icon_lock", "icon_audio", "p_round"] {
            assert!(ui.iter().any(|h| h == n), "missing {n}");
        }
        assert!(crate::art::frame("icons", "icon_trains").is_some());
    }

    #[test]
    fn the_hud_draws_over_the_3d_world_in_both_orientations() {
        for v in [View::ipad_landscape(), View::ipad_portrait()] {
            let mut p = fresh();
            for _ in 0..200 {
                p.update(&Input::default(), &v, 1.0 / 60.0);
            }
            let pen = drawn(&p, &v);
            assert!(pen.vertex_count() > 300);
            assert!(pen.used().iter().any(|b| b.tex == Some(Tex::Glyphs)));
            assert_eq!(p.world(), World3d::Trains);
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

    #[test]
    fn hopping_riders_throw_sparks_where_they_leave() {
        let v = View::ipad_landscape();
        let mut p = fresh();
        for _ in 0..200 {
            p.update(&Input::default(), &v, 1.0 / 60.0);
        }
        let i = p.g.riders.len() - 1;
        p.g.tap_rider(i);
        p.update(&Input::default(), &v, 1.0 / 60.0);
        assert!(!p.g.particles.list.is_empty());
        assert!(p.g.bursts.is_empty());
    }

    #[test]
    fn tapping_a_rider_on_screen_sends_it_off() {
        for v in [View::ipad_landscape(), View::ipad_portrait()] {
            let mut p = fresh();
            for _ in 0..200 {
                p.update(&Input::default(), &v, 1.0 / 60.0);
            }
            assert_eq!(p.g.phase, Phase::Remove);
            let i = p.g.riders.len() - 1;
            let at = p
                .g
                .cam_of(&v)
                .project(&v, p.g.rider_world(i) + vec3(0.0, 0.35, 0.0))
                .unwrap();
            p.update(&Input::press_at(at), &v, 1.0 / 60.0);
            assert_eq!(p.g.removed(), 1, "{v:?}");
        }
    }

    #[test]
    fn every_rider_is_a_big_enough_touch_target() {
        for v in [View::ipad_landscape(), View::ipad_portrait()] {
            let mut p = fresh();
            p.g.start_level(1);
            p.g.question = Question::from_pair(1, 10, 3);
            for _ in 0..300 {
                p.update(&Input::default(), &v, 1.0 / 60.0);
            }
            let c = p.g.cam_of(&v);
            let pts: Vec<_> = (0..p.g.riders.len())
                .map(|i| c.project(&v, p.g.rider_world(i)).unwrap())
                .collect();
            let f = v.full();
            for (i, a) in pts.iter().enumerate() {
                assert!(a.x > f.x && a.x < f.x + f.w, "rider {i} off screen {a:?}");
                for b in &pts[i + 1..] {
                    assert!(a.distance(*b) > 14.0, "{a:?} {b:?}");
                }
            }
        }
    }
}
