use super::Play;
use crate::draw::Draw;
use crate::paint::Pen;
use crate::ui::{plain_score, Results};
use glam::{vec2, Vec2};
use kg_core::engine::{Game, Input, View};
use kg_core::games::blocks::*;
use kg_core::geom::{Color, Rect, WHITE};

pub const KEY: &str = GAME;
const SHEET: &str = "blocks";

pub struct BlocksPlay {
    pub g: Blocks,
    results_age: f32,
    ready_age: f32,
}

impl BlocksPlay {
    pub fn new() -> Self {
        Self {
            g: Blocks::new(),
            results_age: 0.0,
            ready_age: 0.0,
        }
    }
}

impl Default for BlocksPlay {
    fn default() -> Self {
        Self::new()
    }
}

impl Play for BlocksPlay {
    fn update(&mut self, input: &Input, view: &View, dt: f32) {
        let showing = self.g.shows_results();
        self.g.update(input, view, dt);
        if self.g.state == State::Ready {
            self.ready_age += dt;
        } else {
            self.ready_age = 0.0;
        }
        if self.g.shows_results() {
            if !showing {
                self.results_age = 0.0;
                kg_core::net::request_board(GAME);
            }
            self.results_age += dt;
        }
    }

    fn draw(&self, pen: &mut Pen, view: &View) {
        draw(&self.g, pen, view, self.ready_age, self.results_age);
    }

    fn score(&self) -> u32 {
        self.g.score
    }

    fn over(&self) -> bool {
        self.g.state == State::Over
    }

    fn as_any(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

fn block(pen: &mut Pen, x: f32, y: f32, s: f32, kind: u8, wash: f32) {
    pen.sprite(
        SHEET,
        sprite_for(kind),
        Rect::new(x + 0.5, y + 0.5, s - 1.0, s - 1.0),
        Draw::default(),
    );
    if wash > 0.0 {
        pen.round_rect(
            Rect::new(x + 1.0, y + 1.0, s - 2.0, s - 2.0),
            s * 0.2,
            Color::new(1.0, 1.0, 1.0, wash * 0.9),
        );
    }
}

fn backdrop(pen: &mut Pen, view: &View) {
    let f = view.full();
    let name = if view.w > view.h { "bg" } else { "bg_p" };
    let s = crate::art::size(SHEET, name);
    let k = (f.w / s.x).max(f.h / s.y);
    let (w, h) = (s.x * k, s.y * k);
    pen.sprite(
        SHEET,
        name,
        Rect::new(f.x + (f.w - w) / 2.0, f.y + (f.h - h) / 2.0, w, h),
        Draw::default(),
    );
}

fn draw_progress(g: &Blocks, pen: &mut Pen, bar: Rect) {
    if g.state == State::Ready {
        return;
    }
    let done = g.progress();
    pen.round_rect(bar, bar.h / 2.0, Color::new(0.0, 0.0, 0.0, 0.35));
    if done > 0.0 {
        pen.round_rect(
            Rect::new(
                bar.x + 2.0,
                bar.y + 2.0,
                ((bar.w - 4.0) * done).max(bar.h - 4.0),
                bar.h - 4.0,
            ),
            (bar.h - 4.0) / 2.0,
            Color::from_rgba(255, 210, 80, 255),
        );
    }
    let label = format!("{} to Level {}", g.lines_to_next(), g.level() + 1);
    let c = bar.center();
    pen.text_center(&label, c.x, c.y, bar.h * 0.7, WHITE);
}

fn draw_board(g: &Blocks, pen: &mut Pen, o: Vec2, cell: f32) {
    let bw = cell * W as f32;
    let bh = cell * H as f32;
    pen.round_rect(
        Rect::new(o.x - 4.0, o.y - 2.0, bw + 8.0, bh + 12.0),
        26.0,
        Color::from_rgba(255, 215, 120, 255),
    );
    pen.round_rect(
        Rect::new(o.x - 4.0, o.y - 4.0, bw + 8.0, bh + 8.0),
        24.0,
        Color::from_rgba(186, 120, 235, 255),
    );
    pen.round_rect(
        Rect::new(o.x - 1.0, o.y - 1.0, bw + 2.0, bh + 2.0),
        20.0,
        Color::from_rgba(24, 14, 62, 255),
    );
    pen.round_rect(Rect::new(o.x, o.y, bw, bh), 19.0, Color::from_rgba(40, 26, 92, 245));
    for y in 0..H {
        for x in 0..W {
            if (x + y) % 2 == 0 {
                pen.rect(
                    Rect::new(o.x + x as f32 * cell, o.y + y as f32 * cell, cell, cell),
                    Color::new(1.0, 1.0, 1.0, 0.035),
                );
            }
        }
    }
    let flash = g.clearing.as_ref().map(|(rows, t)| (rows, *t / CLEAR_TIME));
    for y in 0..H {
        for x in 0..W {
            let Some(k) = g.cells[y][x] else { continue };
            let mut wash = 0.0;
            let mut s = cell;
            if let Some((rows, p)) = &flash {
                if rows.contains(&y) {
                    let d = (x as f32 - 4.5).abs() / 5.0;
                    let k2 = ((p - d * 0.4) / 0.6).clamp(0.0, 1.0);
                    wash = k2;
                    s = cell * (1.0 - k2 * 0.9);
                }
            }
            let off = (cell - s) / 2.0;
            block(pen, o.x + x as f32 * cell + off, o.y + y as f32 * cell + off, s, k, wash);
        }
    }
    if g.state == State::Playing && g.clearing.is_none() {
        let ghost = g.ghost();
        for (x, y) in ghost.cells() {
            if y >= 0 {
                pen.sprite(
                    SHEET,
                    sprite_for(g.piece.kind),
                    Rect::new(o.x + x as f32 * cell, o.y + y as f32 * cell, cell, cell),
                    Draw::tint(Color::new(1.0, 1.0, 1.0, 0.3)),
                );
            }
        }
        for (x, y) in g.piece.cells() {
            if y >= 0 {
                block(pen, o.x + x as f32 * cell, o.y + y as f32 * cell, cell, g.piece.kind, 0.0);
            }
        }
    }
}

fn draw_side_panel(g: &Blocks, pen: &mut Pen, px: f32, py: f32) {
    pen.painted_panel(Rect::new(px, py, 170.0, 150.0), Color::new(1.0, 1.0, 1.0, 0.95));
    pen.text_outline("NEXT", px + 85.0, py + 28.0, 26.0, WHITE, Color::from_rgba(110, 50, 170, 255));
    let pc = Piece {
        kind: g.next,
        rot: 0,
        x: 0,
        y: 0,
    };
    let s = 28.0;
    let bob = (g.t * 3.0).sin() * 3.0;
    let shift = s * g.next_offset();
    for (x, y) in pc.cells() {
        block(
            pen,
            px + 85.0 - 2.0 * s + x as f32 * s + shift,
            py + 60.0 + y as f32 * s + bob,
            s,
            g.next,
            0.0,
        );
    }
    let stats = [("SCORE", g.score), ("LINES", g.lines), ("LEVEL", g.level()), ("BEST", g.best.0)];
    for (k, (label, v)) in stats.iter().enumerate() {
        let y = py + 180.0 + k as f32 * 80.0;
        pen.text_left(label, px, y - 7.0, 22.0, Color::from_rgba(255, 225, 150, 255));
        let sc = if k == 0 { g.bump.scale(0.25) } else { 1.0 };
        let digits = v.to_string();
        let tw = crate::draw::measure(&digits, 40.0 * sc);
        pen.text_outline(
            &digits,
            px + tw / 2.0,
            y + 24.0,
            40.0 * sc,
            WHITE,
            Color::from_rgba(70, 30, 120, 255),
        );
    }
}

pub fn draw(g: &Blocks, pen: &mut Pen, view: &View, ready_age: f32, results_age: f32) {
    backdrop(pen, view);
    let (cell, o, side) = Blocks::layout(view);
    let o = o + g.shake.offset() + vec2(0.0, g.land * 3.0);
    let bw = cell * W as f32;
    let bh = cell * H as f32;
    draw_board(g, pen, o, cell);
    let panel_x = if side { o.x + bw + 30.0 } else { o.x };
    let panel_y = if side { o.y } else { 20.0 };
    if side {
        draw_side_panel(g, pen, panel_x, panel_y);
    } else {
        pen.text_shadow(
            &g.score.to_string(),
            view.w / 2.0,
            50.0,
            52.0 * g.bump.scale(0.25),
            WHITE,
        );
        pen.text_center(&format!("Lv {}", g.level()), view.w - 120.0, 40.0, 30.0, WHITE);
    }
    let bar = if side {
        Rect::new(panel_x, panel_y + 486.0, 190.0, 28.0)
    } else {
        Rect::new(o.x, 34.0, bw * 0.32, 26.0)
    };
    draw_progress(g, pen, bar);
    pen.particles(&g.fx);
    pen.popups(&g.pops);
    if g.state == State::Playing && g.clearing.is_none() {
        if let Some(h) = g.hint_text() {
            let pulse = 1.0 + (g.t * 4.0).sin() * 0.04;
            pen.hint(
                h,
                vec2(o.x + bw / 2.0, if side { o.y - 40.0 } else { o.y + bh * 0.3 }),
                pulse,
            );
        }
    }
    match g.state {
        State::Ready => {
            let top = kg_core::profile::unlocked(GAME);
            let best = format!("Your best: Level {top}");
            let first = if top > 1 { best.as_str() } else { "Fill rows to clear them" };
            pen.panel(view, "Block Drop", &[first, "Tap to start"], ready_age);
        }
        State::Over if g.shows_results() => {
            let line = format!("Score {}   Lines {}   Level {}", g.score, g.lines, g.level());
            pen.results(
                view,
                &Results {
                    game: GAME,
                    title: "Stacked up!",
                    line: &line,
                    rank: g.rank,
                    action: "Play again",
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

    fn fresh() -> BlocksPlay {
        kg_core::save::reset_for_test();
        kg_core::profile::create("Mia", 0);
        BlocksPlay::new()
    }

    fn drawn(p: &BlocksPlay, v: &View) -> Pen {
        let mut pen = Pen::default();
        pen.begin(*v, 1.0);
        p.draw(&mut pen, v);
        pen
    }

    fn tap() -> Input {
        Input::press_at(vec2(500.0, 500.0))
    }

    #[test]
    fn the_ready_and_playing_screens_draw_blocks_text_and_ui() {
        for v in [View::ipad_landscape(), View::ipad_portrait()] {
            let mut p = fresh();
            let pen = drawn(&p, &v);
            let texs: Vec<_> = pen.used().iter().filter_map(|b| b.tex).collect();
            assert!(texs.iter().any(|t| matches!(t, Tex::Page("blocks", _))));
            assert!(texs.contains(&Tex::Glyphs));
            assert!(texs.iter().any(|t| matches!(t, Tex::Page("ui", _))));
            p.update(&tap(), &v, 1.0 / 60.0);
            p.update(&Input::default(), &v, 1.0 / 60.0);
            assert_eq!(p.g.state, State::Playing);
            assert!(drawn(&p, &v).vertex_count() > 500);
        }
    }

    #[test]
    fn the_backdrop_covers_the_whole_screen_past_the_safe_area() {
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
        assert!(lo <= f.x + 0.01 && hi >= f.x + f.w, "bg {lo}..{hi} vs {f:?}");
        assert!(top <= f.y + 0.5 && bot >= f.y + f.h - 0.5);
    }

    #[test]
    fn the_results_panel_appears_when_the_stack_tops_out_and_restarts_each_run() {
        let v = View::ipad_landscape();
        let mut p = fresh();
        kg_core::profile::set(GAME, "cleared", 1);
        p.update(&tap(), &v, 1.0 / 60.0);
        p.g.cells = [[Some(2); W]; H];
        for _ in 0..400 {
            p.update(&Input::default(), &v, 1.0 / 60.0);
        }
        assert!(p.g.shows_results());
        assert!(p.results_age > 1.0);
        assert!(drawn(&p, &v).vertex_count() > 500);
        for _ in 0..60 {
            p.update(&Input::default(), &v, 1.0 / 60.0);
        }
        p.update(&tap(), &v, 1.0 / 60.0);
        assert_eq!(p.g.state, State::Playing);
        assert!(!p.g.shows_results());
    }
}
