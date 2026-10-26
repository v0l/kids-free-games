use super::Play;
use crate::draw::Draw;
use crate::paint::{Pen, Tex, Vert};
use crate::ui::{chip_rect, plain_score, Results};
use glam::{vec2, Vec2};
use kg_core::engine::{Game, Input, View};
use kg_core::fx::{ease_out_cubic, ease_out_elastic, Side};
use kg_core::games::snake::*;
use kg_core::geom::{darker, Color, Rect, WHITE};

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

fn draw_meadow(pen: &mut Pen, view: &View, board: Rect) {
    let f = view.full();
    let side = f.w.max(f.h);
    pen.sprite(
        SHEET,
        "bg",
        Rect::new(
            f.x + (f.w - side) / 2.0,
            f.y + (f.h - side) / 2.0,
            side,
            side,
        ),
        Draw::default(),
    );
    let pad = 14.0;
    for (grow, alpha) in [(26.0, 0.06), (20.0, 0.08), (14.0, 0.1)] {
        pen.round_rect(
            Rect::new(
                board.x - grow,
                board.y - grow + 6.0,
                board.w + grow * 2.0,
                board.h + grow * 2.0,
            ),
            22.0,
            Color::new(0.0, 0.12, 0.0, alpha),
        );
    }
    pen.round_rect(
        Rect::new(
            board.x - pad,
            board.y - pad,
            board.w + pad * 2.0,
            board.h + pad * 2.0,
        ),
        pad + 6.0,
        Color::from_rgba(52, 120, 34, 255),
    );
    pen.round_rect(
        Rect::new(
            board.x - pad + 3.0,
            board.y - pad + 3.0,
            board.w + pad * 2.0 - 6.0,
            board.h + pad * 2.0 - 8.0,
        ),
        pad + 3.0,
        Color::from_rgba(120, 200, 70, 255),
    );
    pen.round_rect(
        Rect::new(board.x - 4.0, board.y - 4.0, board.w + 8.0, board.h + 8.0),
        10.0,
        Color::from_rgba(150, 214, 76, 255),
    );
}

fn draw_checker(pen: &mut Pen, g: &Snake, o: Vec2, cell: f32) {
    for y in 0..g.size.y {
        for x in 0..g.size.x {
            let c = if (x + y) % 2 == 0 {
                Color::new(1.0, 1.0, 0.7, 0.07)
            } else {
                Color::new(0.0, 0.3, 0.0, 0.08)
            };
            pen.rect(
                Rect::new(o.x + x as f32 * cell, o.y + y as f32 * cell, cell, cell),
                c,
            );
        }
    }
}

fn draw_rim(pen: &mut Pen, board: Rect) {
    let edge = Color::new(0.0, 0.2, 0.0, 0.22);
    let clear = Color::new(0.0, 0.2, 0.0, 0.0);
    let d = 16.0;
    let (x0, y0, x1, y1) = (board.x, board.y, board.x + board.w, board.y + board.h);
    let mut quad = |a: Vec2, b: Vec2, c: Vec2, e: Vec2| {
        let v = |p: Vec2, color: Color| Vert {
            pos: p,
            uv: Vec2::splat(0.5),
            color,
        };
        pen.mesh(
            Tex::White,
            &[v(a, edge), v(b, edge), v(c, clear), v(e, clear)],
            &[0, 1, 2, 0, 2, 3],
        );
    };
    quad(
        vec2(x0, y0),
        vec2(x1, y0),
        vec2(x1, y0 + d),
        vec2(x0, y0 + d),
    );
    quad(
        vec2(x0, y1),
        vec2(x1, y1),
        vec2(x1, y1 - d),
        vec2(x0, y1 - d),
    );
    quad(
        vec2(x0, y0),
        vec2(x0, y1),
        vec2(x0 + d, y1),
        vec2(x0 + d, y0),
    );
    quad(
        vec2(x1, y0),
        vec2(x1, y1),
        vec2(x1 - d, y1),
        vec2(x1 - d, y0),
    );
}

fn draw_apple(pen: &mut Pen, g: &Snake, o: Vec2, cell: f32) {
    let f = o + g.food.as_vec2() * cell + vec2(cell, cell) / 2.0;
    let pop = ease_out_elastic(g.food_age / 0.7).max(0.01);
    let bob = (g.clock * 4.0).sin() * cell * 0.03;
    let drop = (1.0 - ease_out_cubic(g.food_age / 0.35)) * cell * 0.8;
    let squash = 1.0 + (g.clock * 4.0).sin() * 0.03;
    let h = cell * 1.7 * pop;
    pen.soft_shadow(f + vec2(0.0, cell * 0.38), h * 0.4, h * 0.13, 0.6);
    let c = f + vec2(0.0, bob - drop);
    let name = g.fruit();
    let s = crate::art::size(SHEET, name);
    let w = h * s.x / s.y.max(1.0);
    pen.sprite(
        SHEET,
        name,
        Rect::new(
            c.x - w * squash / 2.0,
            c.y - h / squash / 2.0,
            w * squash,
            h / squash,
        ),
        Draw::default(),
    );
}

fn draw_snake(pen: &mut Pen, g: &Snake, o: Vec2, cell: f32) {
    let chain = g.chain(g.frac());
    let n = chain.len();
    let size = g.size.as_vec2();
    let dead = g.state == State::Dead;
    let bulge_pos: Vec<f32> = g.bulges.iter().map(|b| *b as f32 + g.frac()).collect();
    let radius = |i: usize| {
        let k = 1.0 - i as f32 / n as f32 * 0.3;
        let swell = bulge_pos
            .iter()
            .map(|p| bulge_scale(i, *p))
            .fold(1.0, f32::max);
        cell * 0.56 * k * swell
    };
    let visible = |p: Vec2| p.x > -1.0 && p.y > -1.0 && p.x < size.x + 1.0 && p.y < size.y + 1.0;
    let shifts = [-1.0f32, 0.0, 1.0];
    for sx in shifts {
        for sy in shifts {
            let shift = vec2(sx * size.x, sy * size.y);
            for i in (1..n).rev() {
                let p = chain[i] + shift;
                if !visible(p) {
                    continue;
                }
                let to = o + (p + vec2(0.5, 0.5)) * cell;
                let from = o + (chain[i - 1] + shift + vec2(0.5, 0.5)) * cell;
                let r = radius(i);
                pen.soft_shadow(to + vec2(r * 0.18, r * 0.62), r * 0.9, r * 0.32, 0.5);
                let shade = if i % 2 == 0 { 1.0 } else { 0.93 };
                let tint = if dead {
                    Color::from_rgba(170, 180, 205, 255)
                } else {
                    darker(WHITE, shade)
                };
                let steps = 2;
                for k in 0..steps {
                    let t = k as f32 / steps as f32;
                    let c = to + (from - to) * t * 0.5;
                    pen.sprite_at(
                        SHEET,
                        "ball",
                        c,
                        r * 2.0 * (1.0 - 0.04 * t),
                        Draw::tint(tint),
                    );
                }
            }
        }
    }
    let head = o + (chain[0] + vec2(0.5, 0.5)) * cell;
    let fwd = g.dir.delta().as_vec2();
    let r = radius(0);
    if !dead {
        let len = tongue_len(g.clock) * cell * 0.75;
        if len > 1.0 {
            let side = vec2(-fwd.y, fwd.x);
            let red = Color::from_rgba(230, 50, 80, 255);
            let base = head + fwd * r * 0.8;
            let tip = base + fwd * len;
            pen.line(base, tip, cell * 0.07, red);
            for s in [-1.0, 1.0] {
                let fork = tip + (fwd + side * s * 0.8) * cell * 0.14;
                pen.line(tip, fork, cell * 0.06, red);
            }
        }
    }
    let wob = if dead {
        (g.dead_age * 20.0).sin() * 0.12
    } else {
        0.0
    };
    let lean = fwd.x * 0.16 + wob;
    let at = head + vec2(fwd.x * r * 0.25, -r * 0.18 + fwd.y * r * 0.2);
    pen.soft_shadow(head + vec2(r * 0.15, r * 0.7), r * 1.2, r * 0.4, 0.55);
    pen.sprite_at(
        SHEET,
        "head",
        at,
        r * 3.4,
        Draw {
            rotation: lean,
            tint: if dead {
                Color::from_rgba(190, 200, 235, 255)
            } else {
                WHITE
            },
            ..Default::default()
        },
    );
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
    draw_meadow(pen, view, board);
    draw_checker(pen, g, o, cell);
    draw_rim(pen, board);
    draw_apple(pen, g, o, cell);
    draw_snake(pen, g, o, cell);
    pen.particles(&g.particles);
    pen.popups(&g.popups);
    pen.offset = Vec2::ZERO;
    pen.chip(view, Side::Left, "APPLES", &g.score.to_string(), g.bump.t);
    let right = pen.chip(view, Side::Right, "BEST", &g.best.0.to_string(), 1.0);
    let left = chip_rect(view, Side::Left, "APPLES", &g.score.to_string());
    draw_progress(pen, g, left.x + left.w + 12.0, right.x - 12.0);
    match g.state {
        State::Ready => {
            let best = format!("Best stage {}", g.best_stage);
            let mut lines = vec!["Eat fruit to grow!"];
            if g.best_stage > 1 {
                lines.push(&best);
            }
            lines.push("Swipe to start");
            pen.panel(view, "Snake", &lines, g.clock);
        }
        State::Playing if g.score == 0 => {
            let pulse = 1.0 + (g.clock * 5.0).sin() * 0.05;
            pen.hint("Swipe to steer", vec2(view.w / 2.0, view.h - 56.0), pulse);
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
