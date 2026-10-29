pub mod world;

use super::Play;
use crate::game3d::World3d;
use crate::paint::Pen;
use crate::ui::{plain_score, Results, Tone, BODY, INK};
use glam::{vec2, vec3};
use kg_core::engine::{Game, Input, View};
use kg_core::fx::{ease_out_back, ease_out_cubic, Side};
use kg_core::games::ballsort::*;
use kg_core::geom::{with_alpha, Color, Rect, WHITE};

pub const KEY: &str = GAME;
pub const SHEET: &str = "ballsort";

pub struct BallSortPlay {
    pub g: BallSort,
    results_age: f32,
}

impl BallSortPlay {
    pub fn new() -> Self {
        Self {
            g: BallSort::new(),
            results_age: 0.0,
        }
    }

    fn shows_results(&self) -> bool {
        self.g.state == State::Won && self.g.end_t > WIN_DELAY
    }
}

impl Default for BallSortPlay {
    fn default() -> Self {
        Self::new()
    }
}

impl Play for BallSortPlay {
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
        self.g.state == State::Won && self.g.end_t > WIN_DELAY + 0.5
    }

    fn world(&self) -> World3d {
        World3d::BallSort
    }

    fn as_any(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

fn draw_select(g: &BallSort, pen: &mut Pen, view: &View) {
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
        pen.nine(
            "ui",
            "panel",
            rr,
            14.0,
            (rr.w.min(rr.h) / 64.0).clamp(0.6, 2.5),
            tint,
        );
        let ink = if locked {
            Color::new(0.4, 0.38, 0.5, 0.7)
        } else {
            INK
        };
        pen.text_center(
            &n.to_string(),
            r.center().x,
            r.y + r.h * 0.38,
            r.h * 0.4,
            ink,
        );
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
                    if k < earned {
                        "star_yellow"
                    } else {
                        "star_white"
                    },
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

fn star_row(pen: &mut Pen, at: glam::Vec2, size: f32, earned: u8, shown: f32) {
    for k in 0..3usize {
        let on = k < earned as usize;
        let pop = if on {
            ease_out_back(((shown - STAR_START - STAR_GAP * k as f32) / 0.3).clamp(0.0, 1.0))
        } else {
            (shown / 0.3).clamp(0.0, 1.0)
        };
        let tint = if on {
            WHITE
        } else {
            Color::new(0.4, 0.4, 0.46, 0.85)
        };
        let lift = if k == 1 { -size * 0.14 } else { 0.0 };
        pen.sprite_at(
            "ui",
            "p_star",
            vec2(at.x + (k as f32 - 1.0) * size * 1.2, at.y + lift),
            size * pop,
            crate::draw::Draw::tint(tint),
        );
    }
}

fn draw_win(g: &BallSort, pen: &mut Pen, view: &View, age: f32) {
    let line = format!("Level {} done, score {}", g.level, g.score);
    pen.results(
        view,
        &Results {
            game: GAME,
            title: "Sorted!",
            line: &line,
            rank: g.rank,
            action: if g.level >= MAX_LEVEL {
                "Play again"
            } else {
                "Next level"
            },
            format: plain_score,
        },
        age,
    );
    let rows = kg_core::profile::board(GAME).len().min(5);
    let ph = 230.0 + rows as f32 * 46.0 + if rows == 0 { 0.0 } else { 20.0 };
    let top = view.h / 2.0 + 24.0 - ph / 2.0;
    let s = ease_out_back(age / 0.32).max(0.01);
    star_row(
        pen,
        vec2(view.w / 2.0, (top - 62.0 * s).max(48.0)),
        84.0 * s,
        g.stars,
        age,
    );
    let w = win_layout(view, g.can_skip());
    let a = ((age - 0.5) / 0.25).clamp(0.0, 1.0);
    if g.can_skip() {
        pen.painted_button(w.skip, "Skip ahead", Tone::Orange, 0.0, a);
    }
    pen.painted_button(w.levels, "Levels", Tone::Blue, 0.0, a);
}

fn hidden_marks(g: &BallSort, pen: &mut Pen, view: &View) {
    for b in g.balls() {
        if b.ball.revealed {
            continue;
        }
        let c = g.cam.project(view, b.pos);
        let e = g.cam.project(view, b.pos + vec3(BALL_D * 0.5, 0.0, 0.0));
        let r = (e.x - c.x).abs();
        pen.text_outline(
            "?",
            c.x,
            c.y,
            r * 1.5,
            WHITE,
            Color::from_rgba(60, 40, 90, 255),
        );
    }
}

fn draw_hud(g: &BallSort, pen: &mut Pen, view: &View) {
    pen.chip(view, Side::Left, "LEVEL", &g.level.to_string(), 1.0);
    pen.chip(view, Side::Right, "MOVES", &g.moves.to_string(), g.bump.t);
    let sorted = format!("{} of {} sorted", g.sorted_tubes(), g.total_colors());
    pen.text_shadow(&sorted, view.w / 2.0, 36.0, 28.0, WHITE);
    star_row(pen, vec2(view.w / 2.0, 78.0), 30.0, g.stars_now(), 10.0);
    let b = buttons(view);
    let can_undo = !g.history.is_empty() && g.state == State::Playing;
    pen.painted_button(
        b.undo,
        "Undo",
        Tone::Blue,
        0.0,
        if can_undo { 1.0 } else { 0.45 },
    );
    pen.painted_button(b.restart, "Restart", Tone::Orange, 0.0, 1.0);
    pen.painted_button(b.hint, "Hint", Tone::Green, 0.0, 1.0);
    pen.painted_button(b.levels, "Levels", Tone::Blue, 0.0, 1.0);
    let pulse = 1.0 + (g.t * 3.0).sin() * 0.03;
    let at = vec2(view.w / 2.0, b.undo.y - 36.0);
    if g.stuck && g.flights.is_empty() {
        pen.hint("No more moves. Tap Undo!", at, pulse);
    } else if g.hint_dead_end {
        pen.hint("That path is stuck. Tap Undo!", at, pulse);
    } else if !g.taught && g.moves == 0 && g.selected.is_none() {
        pen.hint("Tap a tube, then tap where the ball goes", at, pulse);
    } else if !g.taught && g.selected.is_some() {
        pen.hint("Now tap another tube", at, pulse);
    }
}

pub fn draw(g: &BallSort, pen: &mut Pen, view: &View, results_age: f32) {
    if g.state == State::Select {
        draw_select(g, pen, view);
        return;
    }
    if g.state == State::Playing || g.end_t < WIN_DELAY {
        hidden_marks(g, pen, view);
    }
    pen.particles(&g.fx);
    pen.popups(&g.pops);
    if g.state == State::Playing {
        draw_hud(g, pen, view);
    } else if g.end_t > WIN_DELAY {
        draw_win(g, pen, view, results_age);
    } else {
        pen.chip(view, Side::Left, "LEVEL", &g.level.to_string(), 1.0);
        let fade = ease_out_cubic(g.end_t / 0.4);
        pen.text_outline(
            "Sorted!",
            view.w / 2.0,
            view.h * 0.12,
            64.0 * (0.8 + 0.2 * fade),
            with_alpha(WHITE, fade),
            with_alpha(BODY, fade),
        );
    }
}
