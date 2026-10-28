pub mod land;
pub mod world;

use super::Play;
use crate::draw::measure;
use crate::game3d::World3d;
use crate::paint::Pen;
use crate::ui::{Tone, BODY, GOLD_TEXT, INK};
use glam::{vec2, vec3, Vec2};
use kg_core::engine::{Game, Input, View, HUD_LEFT};
use kg_core::fx::{ease_out_back, ease_out_cubic, popup_scale, Particle};
use kg_core::games::paper::arena::{clock_label, score, score_label};
use kg_core::games::paper::camera;
use kg_core::games::paper::*;
use kg_core::geom::{Color, Rect, WHITE};

pub const KEY: &str = GAME;
pub const SLAB: f32 = kg_core::games::paper::camera::SLAB;
const MAP_PX: f32 = 176.0;

pub struct PaperPlay {
    pub g: Paper,
}

impl PaperPlay {
    pub fn new() -> Self {
        Self { g: Paper::new() }
    }
}

impl Default for PaperPlay {
    fn default() -> Self {
        Self::new()
    }
}

impl Play for PaperPlay {
    fn update(&mut self, input: &Input, view: &View, dt: f32) {
        let before = self.g.state;
        self.g.update(input, view, dt);
        if matches!(self.g.state, State::Dead | State::Won)
            && !matches!(before, State::Dead | State::Won)
        {
            kg_core::net::request_board(GAME);
        }
    }

    fn draw(&self, pen: &mut Pen, view: &View) {
        draw(&self.g, pen, view);
    }

    fn score(&self) -> u32 {
        (self.g.run.peak.max(self.g.share(0)) * 100.0) as u32
    }

    fn over(&self) -> bool {
        matches!(self.g.state, State::Dead | State::Won)
    }

    fn world(&self) -> World3d {
        World3d::Paper
    }

    fn as_any(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

fn ink() -> Color {
    Color::from_rgba(30, 60, 40, 255)
}

fn navy() -> Color {
    Color::from_rgba(20, 28, 72, 255)
}

fn at(g: &Paper, view: &View, p: glam::Vec2, h: f32) -> Vec2 {
    g.eye(view).project(view, vec3(p.x, camera::SLAB + h, p.y))
}

fn px_per_unit(g: &Paper, view: &View, p: glam::Vec2) -> f32 {
    let a = at(g, view, p, 0.0);
    let b = at(g, view, p + glam::Vec2::X, 0.0);
    a.distance(b)
}

fn draw_labels(g: &Paper, pen: &mut Pen, view: &View) {
    let mut order: Vec<usize> = (0..g.w.players.len())
        .filter(|i| g.w.players[*i].alive)
        .collect();
    order.sort_by(|a, b| {
        g.w.players[*a]
            .look()
            .y
            .total_cmp(&g.w.players[*b].look().y)
    });
    for id in order {
        let p = &g.w.players[id];
        let s = px_per_unit(g, view, p.look());
        let c = at(g, view, p.look(), 1.1);
        let (tag, size, fill) = if id == 0 {
            ("You", 26.0, GOLD_TEXT)
        } else {
            (p.name, 21.0, WHITE)
        };
        pen.text_outline(tag, c.x, c.y - s * 0.9, size, fill, color(id).darker(0.45));
    }
}

fn draw_world_fx(g: &Paper, pen: &mut Pen, view: &View) {
    for p in &g.particles.list {
        let k = px_per_unit(g, view, p.pos);
        let q = Particle {
            pos: at(g, view, p.pos, 0.6),
            size: p.size * k,
            ..*p
        };
        pen.particle(&q);
    }
    for p in &g.popups.list {
        let (pop, fade) = popup_scale(p);
        let rise = ease_out_cubic((p.age / p.life).min(1.0)) * 1.8;
        let k = px_per_unit(g, view, p.pos);
        let c = at(g, view, p.pos, 1.8 + rise);
        let size = (p.size * k * 0.8 * pop).clamp(24.0, 90.0);
        pen.text_outline(
            &p.text,
            c.x,
            c.y - k * 0.0,
            size,
            p.color.with_alpha(fade),
            INK.with_alpha(fade),
        );
    }
}

fn pill(pen: &mut Pen, r: Rect, col: Color) {
    pen.round_rect(
        r.offset(vec2(0.0, 4.0)),
        r.h / 2.0,
        Color::new(0.0, 0.0, 0.0, 0.18),
    );
    pen.round_rect(r, r.h / 2.0, col);
}

fn skull(pen: &mut Pen, c: Vec2, r: f32, col: Color) {
    pen.circle(c - vec2(0.0, r * 0.1), r * 0.82, col);
    pen.round_rect(
        Rect::new(c.x - r * 0.45, c.y + r * 0.2, r * 0.9, r * 0.7),
        r * 0.2,
        col,
    );
    for s in [-1.0, 1.0] {
        pen.circle(c + vec2(s * r * 0.34, -r * 0.1), r * 0.22, navy());
    }
}

fn draw_share(g: &Paper, pen: &mut Pen) {
    let pct = g.share(0) * 100.0;
    let k = g.bump.scale(0.12);
    let r = Rect::new(HUD_LEFT, 12.0, 250.0, 46.0);
    pill(pen, r, navy());
    let fill = (g.share(0) / 0.5).clamp(0.0, 1.0);
    let w = (r.w * fill).max(r.h * 0.9).min(r.w);
    pen.round_rect(
        Rect::new(r.x, r.y, w, r.h),
        r.h / 2.0,
        Color::from_rgba(255, 200, 30, 255),
    );
    pen.round_rect(
        Rect::new(r.x + 4.0, r.y + 4.0, w - 8.0, r.h * 0.34),
        r.h * 0.17,
        Color::new(1.0, 1.0, 1.0, 0.3),
    );
    let label = if pct < 10.0 {
        format!("{pct:.2} %")
    } else {
        format!("{pct:.1} %")
    };
    pen.text_outline(&label, r.x + r.w / 2.0, r.center().y, 30.0 * k, WHITE, INK);
    let kills = Rect::new(HUD_LEFT, r.y + r.h + 10.0, 118.0, 40.0);
    pill(pen, kills, Color::new(1.0, 1.0, 1.0, 0.92));
    skull(pen, vec2(kills.x + 24.0, kills.center().y), 14.0, navy());
    pen.text_left(
        &format!("x{}", g.me().kills),
        kills.x + 48.0,
        kills.center().y,
        28.0,
        navy(),
    );
    let left = Rect::new(HUD_LEFT, kills.y + kills.h + 10.0, 196.0, 36.0);
    pill(pen, left, Color::new(0.08, 0.1, 0.2, 0.55));
    let n = g.snakes_left();
    let label = if n == 1 {
        "1 snake left".to_string()
    } else {
        format!("{n} snakes left")
    };
    pen.text_outline(&label, left.center().x, left.center().y, 22.0, WHITE, INK);
    if let Some(m) = g.milestone.filter(|_| g.state == State::Playing) {
        let goal = format!("Next: {}%", m / 10);
        pen.text_outline(
            &goal,
            left.center().x,
            left.y + left.h + 22.0,
            20.0,
            WHITE.with_alpha(0.85),
            INK,
        );
    }
}

pub fn board_rows(g: &Paper) -> Vec<(usize, usize)> {
    let ranks = g.w.ranking();
    let mut rows: Vec<(usize, usize)> = ranks.iter().take(3).copied().enumerate().collect();
    if let Some(me) = ranks.iter().position(|i| *i == 0).filter(|r| *r >= 3) {
        rows.push((me, 0));
    }
    rows
}

fn draw_board(g: &Paper, pen: &mut Pen, view: &View) {
    let rows = board_rows(g);
    let w = 262.0;
    let row_h = 46.0;
    let x = view.w - w - 12.0;
    let y = 12.0;
    for (k, (rank, id)) in rows.iter().enumerate() {
        let r = Rect::new(x + 26.0, y + k as f32 * (row_h + 8.0), w - 26.0, row_h);
        let cy = r.center().y;
        let mine = *id == 0;
        let base = if mine {
            Color::from_rgba(76, 214, 120, 255)
        } else {
            color(*id).lerp(Color::from_rgba(120, 120, 130, 255), 0.35)
        };
        pill(pen, r, base.with_alpha(0.88));
        let p = &g.w.players[*id];
        let pct = g.share(*id) * 100.0;
        pen.text_outline(&format!("{}", rank + 1), x + 10.0, cy, 22.0, WHITE, INK);
        let chip = Rect::new(r.x + 6.0, cy - 15.0, 84.0, 30.0);
        pen.round_rect(chip, 15.0, Color::new(1.0, 1.0, 1.0, 0.35));
        pen.text_center(&format!("{pct:.2}%"), chip.center().x, cy, 19.0, navy());
        let name = if mine { "You" } else { p.name };
        let size = if measure(name, 22.0) > r.w - 108.0 {
            17.0
        } else {
            22.0
        };
        pen.text_left(name, r.x + 98.0, cy, size, navy());
    }
}

fn draw_feed(g: &Paper, pen: &mut Pen, view: &View) {
    for (k, f) in g.feed.iter().enumerate() {
        let a = (1.0 - ((f.age - (FEED_LIFE - 0.5)) / 0.5).clamp(0.0, 1.0))
            * ease_out_cubic((f.age / 0.15).min(1.0));
        let pop = ease_out_back((f.age / 0.3).min(1.0)).max(0.1);
        let size = if f.mine { 30.0 } else { 22.0 } * if k == 0 { pop } else { 1.0 };
        let w = measure(&f.text, size) + 48.0;
        let top = if view.w < view.h { 300.0 } else { 120.0 };
        let cy = top + k as f32 * 46.0;
        let r = Rect::new(view.w / 2.0 - w / 2.0, cy - size * 0.8, w, size * 1.6);
        pen.round_rect(r, r.h / 2.0, Color::new(0.08, 0.1, 0.2, 0.5 * a));
        pen.circle(vec2(r.x + 18.0, cy), 7.0, f.color.with_alpha(a));
        let fill = if f.mine { GOLD_TEXT } else { WHITE };
        pen.text_outline(
            &f.text,
            view.w / 2.0 + 8.0,
            cy,
            size,
            fill.with_alpha(a),
            INK.with_alpha(a),
        );
    }
}

pub fn minimap_rect(view: &View) -> Rect {
    Rect::new(16.0, view.h - MAP_PX - 16.0, MAP_PX, MAP_PX)
}

fn draw_minimap(g: &Paper, pen: &mut Pen, view: &View) {
    let disc = minimap_rect(view);
    let c = disc.center();
    pen.circle(
        c + vec2(0.0, 5.0),
        disc.w / 2.0 + 8.0,
        Color::new(0.0, 0.0, 0.0, 0.2),
    );
    pen.circle(c, disc.w / 2.0 + 8.0, Color::from_rgba(26, 158, 86, 255));
    pen.circle(c, disc.w / 2.0, Color::from_rgba(198, 198, 200, 255));
    let side = disc.w * 0.74;
    let r = Rect::new(c.x - side / 2.0, c.y - side / 2.0, side, side);
    pen.rect(r, Color::from_rgba(232, 252, 244, 255));
    let step = RES * 2;
    let k = r.w / N as f32;
    let at_cell = |x: i32, y: i32| g.w.owner[idx(glam::ivec2(x + step / 2, y + step / 2))];
    for y in (0..N).step_by(step as usize) {
        let mut x = 0;
        while x < N {
            let o = at_cell(x, y);
            let start = x;
            while x < N && at_cell(x, y) == o {
                x += step;
            }
            if let Some(o) = o {
                pen.rect(
                    Rect::new(
                        r.x + start as f32 * k,
                        r.y + y as f32 * k,
                        (x - start) as f32 * k,
                        step as f32 * k,
                    ),
                    color(o as usize),
                );
            }
        }
    }
    let k = r.w / SIZE;
    for (id, p) in g.w.players.iter().enumerate().rev() {
        if !p.alive {
            continue;
        }
        let q = vec2(r.x, r.y) + p.look() * k;
        if id == 0 {
            let pulse = 1.0 + 0.25 * (g.clock * 6.0).sin();
            pen.circle(q, 7.0 * pulse, WHITE);
            pen.circle(q, 5.0, color(0));
        } else {
            pen.circle(q, 4.5, color(id).darker(0.6));
            pen.circle(q, 3.2, color(id));
        }
    }
}

fn draw_arrows(g: &Paper, pen: &mut Pen, view: &View) {
    if !g.me().alive {
        return;
    }
    let f = view.full();
    let c = f.center();
    let half = vec2(f.w / 2.0 - 48.0, f.h / 2.0 - 48.0);
    for (id, dir, dist) in g.threats(view) {
        let t = (half.x / dir.x.abs().max(1e-3)).min(half.y / dir.y.abs().max(1e-3));
        let pos = c + dir * t;
        let near = (1.0 - dist / 26.0).clamp(0.0, 1.0);
        let pulse = 1.0 + 0.12 * (g.clock * 8.0).sin() * near;
        let size = (22.0 + 14.0 * near) * pulse;
        let col = color(id);
        let tip = pos + dir * size;
        let side = dir.perp() * size * 0.75;
        pen.circle(pos, size * 0.95, Color::new(1.0, 1.0, 1.0, 0.85));
        pen.tri(tip, pos + side, pos - side, col);
        pen.circle(pos, size * 0.55, col);
    }
}

fn draw_ready(g: &Paper, pen: &mut Pen, view: &View) {
    pen.fill_screen(view, Color::new(0.05, 0.12, 0.08, 0.3));
    let c = vec2(view.w / 2.0, view.h * 0.32);
    pen.ribbon(view, "Paper Land", c, g.clock);
    pen.text_outline(
        "Be the last snake standing!",
        c.x,
        c.y + 86.0,
        36.0,
        WHITE,
        ink(),
    );
    if g.best > 0 {
        let best = format!("Your best: {}", score_label(g.best));
        pen.text_outline(&best, c.x, c.y + 136.0, 30.0, GOLD_TEXT, ink());
    }
    let pulse = 1.0 + (g.clock * 4.0).sin() * 0.04;
    pen.hint("Touch to start", vec2(view.w / 2.0, view.h - 70.0), pulse);
}

pub fn cause_line(g: &Paper) -> String {
    match (g.run.cause, g.run.killer) {
        (Some(Cause::HeadOn), Some(k)) => format!("You bumped into {k}"),
        (Some(_), Some(k)) => format!("{k} got you"),
        _ => "You ran into your own trail".into(),
    }
}

fn stat(pen: &mut Pen, c: Vec2, s: f32, value: &str, label: &str, col: Color) {
    pen.text_outline(value, c.x, c.y, 46.0 * s, col, INK);
    pen.text_center(label, c.x, c.y + 40.0 * s, 22.0 * s, BODY);
}

fn draw_dead(g: &Paper, pen: &mut Pen, view: &View) {
    let age = g.since;
    pen.fill_screen(
        view,
        Color::new(0.06, 0.04, 0.16, 0.5 * ease_out_cubic(age / 0.25)),
    );
    let s = ease_out_back(((age - 0.1) / 0.35).clamp(0.0, 1.0)).max(0.01);
    let r = death_panel(view);
    let c = r.center();
    let pr = Rect::new(c.x - r.w * s / 2.0, c.y - r.h * s / 2.0, r.w * s, r.h * s);
    pen.painted_panel(pr, WHITE);
    let won = g.state == State::Won;
    let title = if won {
        "Last snake!"
    } else if g.new_best {
        "New best!"
    } else {
        "Oh no!"
    };
    pen.ribbon(view, title, vec2(c.x, pr.y + 8.0 * s), age);
    let line = if won {
        "Every other snake is gone".to_string()
    } else {
        cause_line(g)
    };
    pen.text_center(&line, c.x, pr.y + 96.0 * s, 28.0 * s, BODY);
    let row = pr.y + pr.h * 0.5;
    let gap = pr.w / 3.0;
    let stats = [
        (score_label(score(g.run.peak)), "land", color(0)),
        (
            g.run.kills.to_string(),
            "eliminated",
            Color::from_rgba(255, 120, 60, 255),
        ),
        (
            clock_label(g.run.time),
            "time",
            Color::from_rgba(120, 200, 90, 255),
        ),
    ];
    for (k, (value, label, col)) in stats.iter().enumerate() {
        let pop = ease_out_back(((age - 0.3 - k as f32 * 0.12) / 0.3).clamp(0.0, 1.0)).max(0.01);
        stat(
            pen,
            vec2(pr.x + gap * (k as f32 + 0.5), row),
            s * pop,
            value,
            label,
            *col,
        );
    }
    let best = if g.new_best {
        "That's your best ever!".to_string()
    } else {
        format!("Best: {}", score_label(g.best))
    };
    pen.text_outline(
        &best,
        c.x,
        pr.y + pr.h - 56.0 * s,
        30.0 * s,
        GOLD_TEXT,
        Color::from_rgba(130, 50, 0, 255),
    );
    if g.new_best {
        let k = ease_out_back(((age - 0.5) / 0.3).clamp(0.0, 1.0));
        pen.trophy(vec2(pr.x + 50.0 * s, pr.y + 60.0 * s), 90.0 * k.max(0.01));
    }
    let a = ((age - RESPAWN_WAIT + 0.2) / 0.2).clamp(0.0, 1.0);
    let pulse = 1.0 + (age * 4.5).sin() * 0.04 * a;
    let b = play_again_button(view);
    let bc = b.center();
    let b = Rect::new(
        bc.x - b.w * pulse / 2.0,
        bc.y - b.h * pulse / 2.0,
        b.w * pulse,
        b.h * pulse,
    );
    pen.painted_button(b, "Play again", Tone::Green, 0.0, a.max(0.35));
}

pub fn draw(g: &Paper, pen: &mut Pen, view: &View) {
    pen.offset = g.shake.offset();
    draw_labels(g, pen, view);
    draw_world_fx(g, pen, view);
    pen.offset = Vec2::ZERO;
    match g.state {
        State::Ready => {
            draw_board(g, pen, view);
            draw_ready(g, pen, view);
        }
        State::Playing => {
            draw_arrows(g, pen, view);
            draw_share(g, pen);
            draw_board(g, pen, view);
            draw_minimap(g, pen, view);
            draw_feed(g, pen, view);
        }
        State::Dead | State::Won => {
            draw_board(g, pen, view);
            draw_feed(g, pen, view);
            draw_dead(g, pen, view);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paint::Tex;
    use kg_core::games::paper::world::Cause;

    fn fresh() -> PaperPlay {
        kg_core::save::reset_for_test();
        kg_core::profile::create("Mia", 0);
        PaperPlay::new()
    }

    fn drawn(p: &PaperPlay, v: &View) -> Pen {
        let mut pen = Pen::default();
        pen.begin(*v, 1.0);
        p.draw(&mut pen, v);
        pen
    }

    #[test]
    fn the_start_screen_draws_the_title_and_text() {
        let v = View::ipad_landscape();
        let p = fresh();
        let pen = drawn(&p, &v);
        assert!(pen
            .used()
            .iter()
            .any(|b| b.tex == Some(Tex::Page("ui", 0)) || b.tex == Some(Tex::Glyphs)));
        assert!(pen.vertex_count() > 100);
    }

    #[test]
    fn every_state_draws_in_both_orientations() {
        for v in [View::ipad_landscape(), View::ipad_portrait()] {
            let mut p = fresh();
            assert!(drawn(&p, &v).vertex_count() > 100);
            p.update(&Input::key(kg_core::engine::Dir::Up), &v, 1.0 / 60.0);
            assert_eq!(p.g.state, State::Playing);
            for _ in 0..30 {
                p.update(&Input::default(), &v, 1.0 / 60.0);
            }
            assert!(drawn(&p, &v).vertex_count() > 100);
            p.g.w.kill(0, Some(1), Cause::Trail);
            for _ in 0..90 {
                p.update(&Input::default(), &v, 1.0 / 60.0);
            }
            assert_eq!(p.g.state, State::Dead);
            assert!(drawn(&p, &v).vertex_count() > 100);
        }
    }

    #[test]
    fn winning_draws_the_results_panel() {
        let v = View::ipad_landscape();
        let mut p = fresh();
        p.update(&Input::key(kg_core::engine::Dir::Up), &v, 1.0 / 60.0);
        for id in 1..p.g.w.players.len() {
            p.g.w.kill(id, Some(0), Cause::Trail);
        }
        for _ in 0..10 {
            p.update(&Input::default(), &v, 1.0 / 60.0);
        }
        assert_eq!(p.g.state, State::Won);
        assert!(p.over());
        assert!(drawn(&p, &v).vertex_count() > 100);
    }

    #[test]
    fn dying_asks_for_the_board_and_scores_the_peak() {
        let v = View::ipad_landscape();
        let mut p = fresh();
        p.update(&Input::key(kg_core::engine::Dir::Up), &v, 1.0 / 60.0);
        p.g.w.players[0].peak = 0.07;
        p.g.w.kill(0, None, Cause::OwnTrail);
        p.update(&Input::default(), &v, 1.0 / 60.0);
        assert!(p.over());
        assert_eq!(p.score(), 7);
        assert_eq!(cause_line(&p.g), "You ran into your own trail");
    }

    #[test]
    fn the_board_shows_the_top_three_and_your_place() {
        let p = fresh();
        let rows = board_rows(&p.g);
        assert!(rows.len() == 3 || rows.len() == 4);
        assert!(rows.iter().any(|(_, id)| *id == 0) || p.g.my_rank() <= 3);
    }

    #[test]
    fn the_hud_stays_inside_the_safe_area_on_a_phone() {
        let v = View::fit(2400.0, 1080.0);
        let m = minimap_rect(&v);
        assert!(m.x >= 0.0 && m.y >= 0.0 && m.x + m.w <= v.w && m.y + m.h <= v.h);
        let b = play_again_button(&v);
        assert!(b.y + b.h <= v.h && b.w >= 200.0 && b.h >= 80.0);
    }

    #[test]
    fn the_hud_screen_stays_under_the_vertex_budget() {
        let v = View::ipad_landscape();
        let mut p = fresh();
        p.update(&Input::key(kg_core::engine::Dir::Up), &v, 1.0 / 60.0);
        for _ in 0..60 * 20 {
            p.update(&Input::default(), &v, 1.0 / 60.0);
            if p.over() {
                break;
            }
        }
        let n = drawn(&p, &v).vertex_count();
        assert!(n < 20_000, "{n} vertices");
    }
}
