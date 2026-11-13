mod board;
mod body;
mod mesh;

use super::Play;
use crate::draw::Draw;
use crate::paint::Pen;
use crate::ui::{chip_rect, plain_score, Results};
use glam::{vec2, Vec2};
use kg_core::engine::{Game, Input, View};
use kg_core::fx::Side;
use kg_core::games::snake::*;
use kg_core::geom::{Color, Rect};

pub const KEY: &str = GAME;
const SHEET: &str = "snake";

pub struct SnakePlay {
    pub g: Snake,
    was_showing: bool,
}

impl SnakePlay {
    pub fn new() -> Self {
        Self {
            g: Snake::new(),
            was_showing: false,
        }
    }
}

impl Default for SnakePlay {
    fn default() -> Self {
        Self::new()
    }
}

impl Play for SnakePlay {
    fn update(&mut self, input: &Input, view: &View, dt: f32) {
        self.g.update(input, view, dt);
        let showing = self.g.shows_results();
        if showing && !self.was_showing {
            kg_core::net::request_board(GAME);
        }
        self.was_showing = showing;
    }

    fn draw(&self, pen: &mut Pen, view: &View) {
        draw(&self.g, pen, view);
    }

    fn score(&self) -> u32 {
        self.g.score
    }

    fn over(&self) -> bool {
        self.g.state == kg_core::games::snake::State::Dead
    }

    fn as_any(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

fn draw_progress(pen: &mut Pen, g: &Snake, x0: f32, x1: f32) {
    let slot = 34.0;
    let w = slot * STAGE_APPLES as f32 + 22.0;
    let w = w.min(x1 - x0);
    if w < slot * 3.0 {
        return;
    }
    let r = Rect::new((x0 + x1) / 2.0 - w / 2.0, 12.0, w, 48.0);
    pen.chip_box(r);
    let have = g.score % STAGE_APPLES;
    let fit = ((w - 22.0) / STAGE_APPLES as f32).min(slot);
    let name = g.fruit();
    let s = crate::art::size(SHEET, name);
    for i in 0..STAGE_APPLES {
        let c = vec2(r.x + 11.0 + fit * (i as f32 + 0.5), r.y + r.h / 2.0);
        if i < have {
            let h = fit * 1.1;
            let fw = h * s.x / s.y.max(1.0);
            pen.sprite(
                SHEET,
                name,
                Rect::new(c.x - fw / 2.0, c.y - h / 2.0, fw, h),
                Draw::default(),
            );
        } else {
            pen.circle(c, fit * 0.28, Color::new(1.0, 1.0, 1.0, 0.28));
        }
    }
}

pub fn draw(g: &Snake, pen: &mut Pen, view: &View) {
    let (cell, o) = g.layout(view);
    let board = Rect::new(o.x, o.y, cell * g.size.x as f32, cell * g.size.y as f32);
    pen.offset = g.shake.offset();
    board::meadow(pen, view);
    board::frame(pen, board, cell);
    board::lawn(pen, g, o, cell, board);
    board::fruit(pen, g, o, cell);
    body::draw_snake(pen, g, o, cell, board);
    board::eat_ring(pen, g, o, cell);
    pen.particles(&g.particles);
    pen.popups(&g.popups);
    pen.offset = Vec2::ZERO;
    if !g.shows_results() {
        pen.chip(view, Side::Left, "APPLES", &g.score.to_string(), g.bump.t);
        let right = pen.chip(view, Side::Right, "BEST", &g.best.0.to_string(), 1.0);
        let left = chip_rect(view, Side::Left, "APPLES", &g.score.to_string());
        draw_progress(pen, g, left.x + left.w + 12.0, right.x - 12.0);
    }
    match g.state {
        State::Ready => {
            let best = format!("Best stage {}", g.best_stage);
            let mut lines = vec!["Eat fruit to grow!"];
            if g.best_stage > 1 {
                lines.push(&best);
            }
            lines.push("Swipe to start");
            pen.panel(view, "Snake", &lines, g.clock);
            let swing = if (g.clock / 1.4) as i32 % 2 == 0 {
                Vec2::X
            } else {
                Vec2::NEG_Y
            };
            board::swipe_hint(pen, view, g.clock, swing);
        }
        State::Playing if g.score == 0 && g.moves < 30 => {
            let want = g.food.as_vec2() - g.body[0].as_vec2();
            let want = if want.x.abs() > want.y.abs() {
                vec2(want.x.signum(), 0.0)
            } else {
                vec2(0.0, want.y.signum())
            };
            board::swipe_hint(pen, view, g.clock, want);
        }
        State::Dead if g.shows_results() => {
            let line = format!("Apples {}   Stage {}", g.score, g.stage());
            pen.results(
                view,
                &Results {
                    game: GAME,
                    title: "Ouch!",
                    line: &line,
                    rank: g.rank,
                    action: "Play again",
                    format: plain_score,
                },
                g.results_age(),
            );
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paint::Tex;

    fn fresh() -> SnakePlay {
        kg_core::save::reset_for_test();
        kg_core::profile::create("Mia", 0);
        SnakePlay::new()
    }

    fn drawn(p: &SnakePlay, v: &View) -> Pen {
        let mut pen = Pen::default();
        pen.begin(*v, 1.0);
        p.draw(&mut pen, v);
        pen
    }

    fn textures(pen: &Pen) -> Vec<Tex> {
        pen.used().iter().filter_map(|b| b.tex).collect()
    }

    #[test]
    fn the_ready_screen_draws_meadow_snake_text_and_panel() {
        let v = View::ipad_landscape();
        let p = fresh();
        let pen = drawn(&p, &v);
        let texs = textures(&pen);
        assert!(texs.iter().any(|t| matches!(t, Tex::Page("snake", _))));
        assert!(texs.contains(&Tex::Glyphs));
        assert!(texs.iter().any(|t| matches!(t, Tex::Page("ui", _))));
    }

    #[test]
    fn the_meadow_covers_the_whole_screen_past_the_safe_area() {
        let v = View::fit_safe(
            2400.0,
            1080.0,
            kg_core::safe::Insets {
                left: 120.0,
                bottom: 40.0,
                ..Default::default()
            },
        );
        let p = fresh();
        let pen = drawn(&p, &v);
        let f = v.full();
        let bg = &pen.used()[0];
        let (mut lo, mut hi) = (f32::MAX, f32::MIN);
        let (mut top, mut bot) = (f32::MAX, f32::MIN);
        for q in &bg.pos {
            lo = lo.min(q[0]);
            hi = hi.max(q[0]);
            top = top.min(-q[1]);
            bot = bot.max(-q[1]);
        }
        assert!(
            lo <= f.x + 0.01 && hi >= f.x + f.w,
            "bg {lo}..{hi} vs {f:?}"
        );
        assert!(top <= f.y + 0.5 && bot >= f.y + f.h - 0.5);
    }

    #[test]
    fn the_board_fits_inside_the_safe_area_in_both_orientations() {
        for v in [View::ipad_landscape(), View::ipad_portrait()] {
            let mut p = fresh();
            p.update(&Input::default(), &v, 0.01);
            let (cell, o) = p.g.layout(&v);
            assert!(o.x >= 0.0 && o.y >= 0.0);
            assert!(o.x + cell * p.g.size.x as f32 <= v.w);
            assert!(o.y + cell * p.g.size.y as f32 <= v.h);
        }
    }

    #[test]
    fn the_results_panel_appears_after_a_crash_and_the_board_is_requested_once() {
        let v = View::ipad_landscape();
        let mut p = fresh();
        p.g.wrap = false;
        p.g.state = State::Playing;
        let w = p.g.size.x;
        p.g.body = [glam::ivec2(w - 1, 3), glam::ivec2(w - 2, 3)]
            .into_iter()
            .collect();
        for _ in 0..120 {
            p.update(&Input::default(), &v, 1.0 / 60.0);
        }
        assert_eq!(p.g.state, State::Dead);
        assert!(p.g.shows_results());
        let before = drawn(&p, &v).vertex_count();
        assert!(before > 500);
        let ready = drawn(&fresh(), &v).vertex_count();
        assert!(before != ready);
    }

    #[test]
    fn playing_with_no_apples_shows_the_steer_hint() {
        let v = View::ipad_portrait();
        let mut p = fresh();
        p.update(&Input::press_at(vec2(300.0, 300.0)), &v, 0.01);
        assert_eq!(p.g.state, State::Playing);
        let hinted = drawn(&p, &v).vertex_count();
        p.g.score = 1;
        assert!(drawn(&p, &v).vertex_count() != hinted);
    }
}
