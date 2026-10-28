use super::Play;
use crate::draw::{measure, Draw};
use crate::paint::{Pen, Tex, Vert};
use crate::ui::{Tone, BODY, GOLD_TEXT, INK};
use glam::{vec2, Vec2};
use kg_core::engine::{Game, Input, View, HUD_LEFT};
use kg_core::fx::{ease_out_back, ease_out_cubic, popup_scale, Particle};
use kg_core::games::paper::arena::{clock_label, score, score_label};
use kg_core::games::paper::*;
use kg_core::geom::{Color, Rect, WHITE};
use std::f32::consts::{FRAC_PI_2, PI};

pub const KEY: &str = GAME;
const SHEET: &str = "paper";
const LAWN_CELLS: f32 = 10.0;
const LIP: f32 = 0.2;
const ROUND: f32 = 0.42;
const TRAIL_W: f32 = 0.58;
const HEAD: f32 = 1.55;
const MAP_PX: f32 = 168.0;

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
        if self.g.state == State::Dead && before != State::Dead {
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
        self.g.state == State::Dead
    }

    fn as_any(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

fn ink() -> Color {
    Color::from_rgba(30, 60, 40, 255)
}

fn lighter(c: Color, k: f32) -> Color {
    c.lerp(WHITE, k)
}

fn outside_ground() -> Color {
    Color::from_rgba(96, 160, 84, 255)
}

struct Cam {
    cell: f32,
    origin: Vec2,
    x0: i32,
    y0: i32,
    x1: i32,
    y1: i32,
}

impl Cam {
    fn new(g: &Paper, view: &View) -> Self {
        let cell = g.cell_px(view);
        let origin = g.origin(view);
        let f = view.full();
        Self {
            cell,
            origin,
            x0: (((f.x - origin.x) / cell).floor() as i32 - 1).max(0),
            y0: (((f.y - origin.y) / cell).floor() as i32 - 1).max(0),
            x1: (((f.x + f.w - origin.x) / cell).ceil() as i32 + 1).min(GW),
            y1: (((f.y + f.h - origin.y) / cell).ceil() as i32 + 2).min(GH),
        }
    }

    fn at(&self, w: Vec2) -> Vec2 {
        self.origin + w * self.cell
    }
}

fn draw_ground(pen: &mut Pen, view: &View, cam: &Cam) {
    pen.rect(view.full(), outside_ground());
    let board = vec2(GW as f32, GH as f32) * cam.cell;
    pen.rect(
        Rect::new(cam.origin.x + 6.0, cam.origin.y + 10.0, board.x, board.y),
        Color::new(0.05, 0.2, 0.05, 0.3),
    );
    let tile = LAWN_CELLS * cam.cell;
    let tx0 = (cam.x0 as f32 / LAWN_CELLS).floor() as i32;
    let ty0 = (cam.y0 as f32 / LAWN_CELLS).floor() as i32;
    let tx1 = (cam.x1 as f32 / LAWN_CELLS).ceil() as i32;
    let ty1 = (cam.y1 as f32 / LAWN_CELLS).ceil() as i32;
    for ty in ty0..ty1 {
        for tx in tx0..tx1 {
            let at = cam.at(vec2(tx as f32, ty as f32) * LAWN_CELLS);
            let w = tile.min(board.x - (at.x - cam.origin.x));
            let h = tile.min(board.y - (at.y - cam.origin.y));
            if w <= 0.0 || h <= 0.0 {
                continue;
            }
            pen.sprite(
                SHEET,
                "grass",
                Rect::new(at.x, at.y, w + 0.5, h + 0.5),
                Draw {
                    flip_x: tx % 2 == 1,
                    flip_y: ty % 2 == 1,
                    tint: Color::new(1.0, 1.0, 1.0, 1.0),
                    ..Default::default()
                },
            );
        }
    }
}

fn draw_fence(pen: &mut Pen, view: &View, cam: &Cam) {
    let board = vec2(GW as f32, GH as f32) * cam.cell;
    let ph = cam.cell * 2.0;
    let fs = crate::art::size(SHEET, "fence");
    let pw = fs.x * ph / fs.y.max(1.0);
    let f = view.full();
    let seen = Rect::new(f.x - pw, f.y - pw, f.w + pw * 2.0, f.h + pw * 2.0);
    let o = cam.origin;
    let n = (board.x / pw).ceil() as usize;
    for k in 0..n {
        let x = o.x + k as f32 * pw + pw / 2.0;
        for y in [o.y - ph * 0.35, o.y + board.y + ph * 0.05] {
            if seen.contains(vec2(x, y)) {
                pen.sprite(
                    SHEET,
                    "fence",
                    Rect::new(x - pw / 2.0, y - ph / 2.0, pw + 1.0, ph),
                    Draw::default(),
                );
            }
        }
    }
    let m = (board.y / pw).ceil() as usize;
    for k in 0..m {
        let y = o.y + k as f32 * pw + pw / 2.0;
        for x in [o.x - ph * 0.35, o.x + board.x + ph * 0.35] {
            if seen.contains(vec2(x, y)) {
                pen.sprite(
                    SHEET,
                    "fence",
                    Rect::new(x - pw / 2.0, y - ph / 2.0, pw + 1.0, ph),
                    Draw::rot(FRAC_PI_2),
                );
            }
        }
    }
}

fn shown_at(g: &Paper, x: i32, y: i32) -> Option<u8> {
    if x < 0 || y < 0 || x >= GW || y >= GH {
        None
    } else {
        g.w.shown(idx(glam::ivec2(x, y)))
    }
}

fn cell_shape(pen: &mut Pen, r: Rect, convex: [bool; 4], rad: f32, col: Color) {
    let h = Vec2::splat(0.5);
    let corners = [
        (vec2(r.x + r.w, r.y + r.h), vec2(-1.0, -1.0), 0.0),
        (vec2(r.x, r.y + r.h), vec2(1.0, -1.0), FRAC_PI_2),
        (vec2(r.x, r.y), vec2(1.0, 1.0), PI),
        (vec2(r.x + r.w, r.y), vec2(-1.0, 1.0), PI * 1.5),
    ];
    let mut verts = vec![Vert {
        pos: r.center(),
        uv: h,
        color: col,
    }];
    for (k, (p, inward, start)) in corners.into_iter().enumerate() {
        if convex[k] {
            let c = p + inward * rad;
            for s in 0..=5 {
                let a = start + FRAC_PI_2 * s as f32 / 5.0;
                verts.push(Vert {
                    pos: c + vec2(a.cos(), a.sin()) * rad,
                    uv: h,
                    color: col,
                });
            }
        } else {
            verts.push(Vert {
                pos: p,
                uv: h,
                color: col,
            });
        }
    }
    let n = verts.len() as u32 - 1;
    let idx: Vec<u32> = (0..n).flat_map(|i| [0, i + 1, (i + 1) % n + 1]).collect();
    pen.mesh(Tex::White, &verts, &idx);
}

fn corners_of(g: &Paper, x: i32, y: i32, o: u8) -> [bool; 4] {
    let same = |dx: i32, dy: i32| shown_at(g, x + dx, y + dy) == Some(o);
    let (l, r, u, d) = (same(-1, 0), same(1, 0), same(0, -1), same(0, 1));
    [!r && !d, !l && !d, !l && !u, !r && !u]
}

fn draw_land(g: &Paper, pen: &mut Pen, cam: &Cam) {
    let c = cam.cell;
    let rad = c * ROUND;
    for pass in 0..2 {
        for y in cam.y0..cam.y1 {
            let mut x = cam.x0;
            while x < cam.x1 {
                let Some(o) = shown_at(g, x, y) else {
                    x += 1;
                    continue;
                };
                let base = color(o as usize);
                let i = idx(glam::ivec2(x, y));
                let age = g.w.age(i);
                let fresh = (0.0..0.4).contains(&age);
                let corners = corners_of(g, x, y, o);
                let lip_here = shown_at(g, x, y + 1) != Some(o);
                if pass == 0 {
                    if lip_here {
                        let at = cam.at(vec2(x as f32, y as f32 + LIP));
                        cell_shape(
                            pen,
                            Rect::new(at.x, at.y, c + 0.5, c),
                            [corners[0], corners[1], false, false],
                            rad,
                            base.darker(0.68),
                        );
                    }
                    x += 1;
                    continue;
                }
                if fresh || corners.iter().any(|k| *k) {
                    let k = if fresh {
                        0.55 + 0.45 * ease_out_back((age / 0.3).min(1.0))
                    } else {
                        1.0
                    };
                    let at = cam.at(vec2(x as f32 + 0.5, y as f32 + 0.5));
                    let s = (c + 0.5) * k;
                    let col = if fresh {
                        lighter(base, 0.45 * (1.0 - age / 0.4))
                    } else {
                        base
                    };
                    cell_shape(
                        pen,
                        Rect::new(at.x - s / 2.0, at.y - s / 2.0, s, s),
                        corners,
                        rad * k,
                        col,
                    );
                    x += 1;
                    continue;
                }
                let start = x;
                while x < cam.x1 {
                    let Some(n) = shown_at(g, x, y) else { break };
                    let j = idx(glam::ivec2(x, y));
                    if n != o
                        || (0.0..0.4).contains(&g.w.age(j))
                        || corners_of(g, x, y, o).iter().any(|k| *k)
                    {
                        break;
                    }
                    x += 1;
                }
                let at = cam.at(vec2(start as f32, y as f32));
                pen.rect(
                    Rect::new(at.x, at.y, (x - start) as f32 * c + 0.5, c + 0.5),
                    base,
                );
            }
        }
    }
    for y in cam.y0..cam.y1 {
        for x in cam.x0..cam.x1 {
            let Some(o) = shown_at(g, x, y) else { continue };
            if shown_at(g, x, y - 1) != Some(o) {
                let at = cam.at(vec2(x as f32, y as f32));
                let corners = corners_of(g, x, y, o);
                let inset = if corners[2] { c * ROUND } else { 0.0 };
                let out = if corners[3] { c * ROUND } else { 0.0 };
                pen.rect(
                    Rect::new(at.x + inset, at.y + c * 0.06, c - inset - out, c * 0.1),
                    lighter(color(o as usize), 0.3),
                );
            }
        }
    }
}

fn polyline(pen: &mut Pen, pts: &[Vec2], width: impl Fn(usize) -> f32, col: Color) {
    if pts.len() < 2 {
        return;
    }
    let h = Vec2::splat(0.5);
    let mut verts = Vec::with_capacity(pts.len() * 2);
    for k in 0..pts.len() {
        let prev = pts[k.saturating_sub(1)];
        let next = pts[(k + 1).min(pts.len() - 1)];
        let n = (next - prev).normalize_or(Vec2::X).perp();
        let w = width(k) / 2.0;
        verts.push(Vert {
            pos: pts[k] + n * w,
            uv: h,
            color: col,
        });
        verts.push(Vert {
            pos: pts[k] - n * w,
            uv: h,
            color: col,
        });
    }
    let idx: Vec<u32> = (0..pts.len() as u32 - 1)
        .flat_map(|i| {
            let a = i * 2;
            [a, a + 1, a + 3, a, a + 3, a + 2]
        })
        .collect();
    pen.mesh(Tex::White, &verts, &idx);
    pen.circle(pts[0], width(0) / 2.0, col);
}

fn draw_trails(g: &Paper, pen: &mut Pen, cam: &Cam) {
    let c = cam.cell;
    for (id, p) in g.w.players.iter().enumerate() {
        if !p.alive || p.trail.is_empty() {
            continue;
        }
        let pts: Vec<Vec2> = p.trail_points().into_iter().map(|q| cam.at(q)).collect();
        let n = pts.len();
        let clock = g.clock;
        let w = |k: usize| {
            let from_tail = (n - 1 - k) as f32;
            let wave = (clock * 9.0 - from_tail * 0.35).sin();
            c * TRAIL_W * (1.0 + 0.1 * wave)
        };
        let base = color(id);
        let shadow: Vec<Vec2> = pts.iter().map(|q| *q + vec2(0.0, c * 0.14)).collect();
        polyline(pen, &shadow, w, base.darker(0.45).with_alpha(0.45));
        polyline(pen, &pts, w, lighter(base, 0.32));
        polyline(pen, &pts, |k| w(k) * 0.42, lighter(base, 0.62));
    }
}

fn draw_heads(g: &Paper, pen: &mut Pen, cam: &Cam) {
    let c = cam.cell;
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
        let at = cam.at(p.look());
        let s = c * HEAD;
        let since_turn = g.w.clock - p.turned_at;
        let squash = if since_turn < 0.25 {
            (since_turn / 0.25 * PI).sin() * 0.14
        } else {
            0.0
        };
        let spawn = ease_out_back((p.age / 0.4).min(1.0)).max(0.05);
        let bob = (g.clock * 10.0 + id as f32).sin() * c * 0.05;
        let head = p.heading();
        pen.soft_shadow(at + vec2(0.0, s * 0.42), s * 0.42, s * 0.16, 0.7);
        let w = s * (1.0 + squash) * spawn;
        let h = s * (1.0 - squash) * spawn;
        let fs = crate::art::size(SHEET, HEADS[id % HEADS.len()]);
        let w = w * fs.x / fs.y.max(1.0);
        let centre = at + vec2(0.0, bob - s * 0.18);
        pen.sprite(
            SHEET,
            HEADS[id % HEADS.len()],
            Rect::new(centre.x - w / 2.0, centre.y - h / 2.0, w, h),
            Draw {
                rotation: head.x * 0.2,
                flip_x: head.x < -0.3,
                ..Default::default()
            },
        );
        let (tag, size, fill) = if id == 0 {
            ("You", 26.0, GOLD_TEXT)
        } else {
            (p.name, 21.0, WHITE)
        };
        pen.text_outline(
            tag,
            centre.x,
            centre.y - h * 0.62,
            size,
            fill,
            color(id).darker(0.45),
        );
    }
}

fn draw_world_fx(g: &Paper, pen: &mut Pen, cam: &Cam) {
    for p in &g.particles.list {
        let q = Particle {
            pos: cam.at(p.pos),
            size: p.size * cam.cell,
            ..*p
        };
        pen.particle(&q);
    }
    for p in &g.popups.list {
        let (pop, fade) = popup_scale(p);
        let rise = ease_out_cubic((p.age / p.life).min(1.0)) * 1.4;
        let at = cam.at(p.pos - vec2(0.0, rise));
        let size = (p.size * cam.cell * pop).clamp(20.0, 90.0);
        pen.text_outline(
            &p.text,
            at.x,
            at.y,
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
        Color::new(0.0, 0.0, 0.0, 0.2),
    );
    pen.round_rect(r, r.h / 2.0, col);
}

fn draw_share(g: &Paper, pen: &mut Pen) {
    let pct = g.share(0) * 100.0;
    let k = g.bump.scale(0.15);
    let r = Rect::new(HUD_LEFT, 10.0, 200.0, 60.0);
    pill(pen, r, Color::new(0.08, 0.1, 0.2, 0.55));
    pen.circle(vec2(r.x + 30.0, r.center().y), 18.0, color(0));
    pen.circle(
        vec2(r.x + 30.0, r.center().y),
        18.0 * 0.45,
        lighter(color(0), 0.6),
    );
    let label = if pct < 10.0 {
        format!("{pct:.2}%")
    } else {
        format!("{pct:.1}%")
    };
    pen.text_outline(&label, r.x + 118.0, r.center().y, 38.0 * k, WHITE, INK);
    let best = format!("Best {}", score_label(g.best.max(score(g.run.peak))));
    pen.text_outline(
        &best,
        r.x + r.w / 2.0,
        r.y + r.h + 22.0,
        22.0,
        GOLD_TEXT,
        INK,
    );
    if let Some(m) = g.milestone.filter(|_| g.state == State::Playing) {
        let goal = format!("Next: {}%", m / 10);
        pen.text_outline(
            &goal,
            r.x + r.w / 2.0,
            r.y + r.h + 50.0,
            20.0,
            WHITE.with_alpha(0.85),
            INK,
        );
    }
}

pub fn board_rows(g: &Paper) -> Vec<(usize, usize)> {
    let ranks = g.w.ranking();
    let mut rows: Vec<(usize, usize)> = ranks.iter().take(5).copied().enumerate().collect();
    if let Some(me) = ranks.iter().position(|i| *i == 0).filter(|r| *r >= 5) {
        rows.push((me, 0));
    }
    rows
}

fn draw_board(g: &Paper, pen: &mut Pen, view: &View) {
    let rows = board_rows(g);
    let w = 236.0;
    let row_h = 34.0;
    let x = view.w - w - 14.0;
    let y = 12.0;
    let panel = Rect::new(x, y, w, rows.len() as f32 * row_h + 14.0);
    pen.round_rect(panel, 16.0, Color::new(0.08, 0.1, 0.2, 0.45));
    for (k, (rank, id)) in rows.iter().enumerate() {
        let cy = y + 7.0 + row_h * (k as f32 + 0.5);
        let mine = *id == 0;
        if mine {
            pen.round_rect(
                Rect::new(x + 4.0, cy - row_h / 2.0 + 2.0, w - 8.0, row_h - 4.0),
                12.0,
                color(0).with_alpha(0.75),
            );
        }
        let p = &g.w.players[*id];
        let pct = g.share(*id) * 100.0;
        let fill = if mine { WHITE } else { WHITE.with_alpha(0.92) };
        pen.text_outline(&format!("{}", rank + 1), x + 20.0, cy, 20.0, fill, INK);
        pen.circle(vec2(x + 44.0, cy), 9.0, color(*id));
        let name = if mine { "You" } else { p.name };
        pen.text_left(name, x + 60.0, cy, 21.0, fill);
        let s = format!("{pct:.1}%");
        let sw = measure(&s, 21.0);
        pen.text_left(&s, x + w - 14.0 - sw, cy, 21.0, fill);
    }
}

fn draw_feed(g: &Paper, pen: &mut Pen, view: &View) {
    for (k, f) in g.feed.iter().enumerate() {
        let a = (1.0 - ((f.age - (FEED_LIFE - 0.5)) / 0.5).clamp(0.0, 1.0))
            * ease_out_cubic((f.age / 0.15).min(1.0));
        let pop = ease_out_back((f.age / 0.3).min(1.0)).max(0.1);
        let size = if f.mine { 30.0 } else { 22.0 } * if k == 0 { pop } else { 1.0 };
        let w = measure(&f.text, size) + 48.0;
        let top = if view.w < view.h { 240.0 } else { 150.0 };
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
    Rect::new(
        view.w - MAP_PX - 16.0,
        view.h - MAP_PX - 16.0,
        MAP_PX,
        MAP_PX,
    )
}

fn draw_minimap(g: &Paper, pen: &mut Pen, view: &View) {
    let r = minimap_rect(view);
    let frame = Rect::new(r.x - 6.0, r.y - 6.0, r.w + 12.0, r.h + 12.0);
    pen.round_rect(
        frame.offset(vec2(0.0, 4.0)),
        18.0,
        Color::new(0.0, 0.0, 0.0, 0.2),
    );
    pen.round_rect(frame, 18.0, Color::new(1.0, 1.0, 1.0, 0.85));
    pen.rect(r, Color::from_rgba(200, 225, 190, 255));
    let step = 2;
    let k = r.w / GW as f32;
    for y in (0..GH).step_by(step) {
        let mut x = 0;
        while x < GW {
            let o = g.w.owner[idx(glam::ivec2(x, y))];
            let start = x;
            while x < GW && g.w.owner[idx(glam::ivec2(x, y))] == o {
                x += step as i32;
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
    let f = view.full();
    let cell = g.cell_px(view);
    let o = g.origin(view);
    let seen = Rect::new(
        (f.x - o.x) / cell,
        (f.y - o.y) / cell,
        f.w / cell,
        f.h / cell,
    );
    let vx0 = seen.x.clamp(0.0, GW as f32);
    let vy0 = seen.y.clamp(0.0, GH as f32);
    let vx1 = (seen.x + seen.w).clamp(0.0, GW as f32);
    let vy1 = (seen.y + seen.h).clamp(0.0, GH as f32);
    let box_r = Rect::new(
        r.x + vx0 * k,
        r.y + vy0 * k,
        (vx1 - vx0) * k,
        (vy1 - vy0) * k,
    );
    for (a, b) in [
        (vec2(box_r.x, box_r.y), vec2(box_r.x + box_r.w, box_r.y)),
        (
            vec2(box_r.x, box_r.y + box_r.h),
            vec2(box_r.x + box_r.w, box_r.y + box_r.h),
        ),
        (vec2(box_r.x, box_r.y), vec2(box_r.x, box_r.y + box_r.h)),
        (
            vec2(box_r.x + box_r.w, box_r.y),
            vec2(box_r.x + box_r.w, box_r.y + box_r.h),
        ),
    ] {
        pen.line(a, b, 2.0, Color::new(1.0, 1.0, 1.0, 0.8));
    }
    for (id, p) in g.w.players.iter().enumerate().rev() {
        if !p.alive {
            continue;
        }
        let at = vec2(r.x, r.y) + p.look() * k;
        if id == 0 {
            let pulse = 1.0 + 0.25 * (g.clock * 6.0).sin();
            pen.circle(at, 7.0 * pulse, WHITE);
            pen.circle(at, 5.0, color(0));
        } else {
            pen.circle(at, 4.5, color(id).darker(0.6));
            pen.circle(at, 3.2, color(id));
        }
    }
}

fn draw_arrows(g: &Paper, pen: &mut Pen, view: &View) {
    if !g.me().alive {
        return;
    }
    let c = vec2(view.w / 2.0, (view.h + TOP * 0.5) / 2.0);
    let half = vec2(view.w / 2.0 - 48.0, (view.h - TOP * 0.5) / 2.0 - 48.0);
    for (id, dir, dist) in g.threats(view) {
        let t = (half.x / dir.x.abs().max(1e-3)).min(half.y / dir.y.abs().max(1e-3));
        let at = c + dir * t;
        let near = (1.0 - dist / 26.0).clamp(0.0, 1.0);
        let pulse = 1.0 + 0.12 * (g.clock * 8.0).sin() * near;
        let size = (22.0 + 14.0 * near) * pulse;
        let col = color(id);
        let tip = at + dir * size;
        let side = dir.perp() * size * 0.75;
        pen.circle(at, size * 0.95, Color::new(1.0, 1.0, 1.0, 0.85));
        pen.tri(tip, at + side, at - side, col);
        pen.circle(at, size * 0.55, col);
    }
}

fn draw_ready(g: &Paper, pen: &mut Pen, view: &View) {
    pen.fill_screen(view, Color::new(0.05, 0.12, 0.08, 0.3));
    let c = vec2(view.w / 2.0, view.h * 0.32);
    pen.ribbon(view, "Paper Land", c, g.clock);
    pen.text_outline(
        "Grab land, dodge trails!",
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
    pen.hint("Swipe to start", vec2(view.w / 2.0, view.h - 70.0), pulse);
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
    let title = if g.new_best { "New best!" } else { "Oh no!" };
    pen.ribbon(view, title, vec2(c.x, pr.y + 8.0 * s), age);
    pen.text_center(&cause_line(g), c.x, pr.y + 96.0 * s, 28.0 * s, BODY);
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
    let cam = Cam::new(g, view);
    pen.offset = g.shake.offset();
    draw_ground(pen, view, &cam);
    draw_land(g, pen, &cam);
    draw_fence(pen, view, &cam);
    draw_trails(g, pen, &cam);
    draw_heads(g, pen, &cam);
    draw_world_fx(g, pen, &cam);
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
        State::Dead => {
            draw_board(g, pen, view);
            draw_feed(g, pen, view);
            draw_dead(g, pen, view);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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

    fn pages(pen: &Pen, sheet: &str) -> bool {
        pen.used()
            .iter()
            .filter_map(|b| b.tex)
            .any(|t| matches!(t, Tex::Page(s, _) if s == sheet))
    }

    #[test]
    fn the_start_screen_draws_the_lawn_players_and_text() {
        let v = View::ipad_landscape();
        let p = fresh();
        let pen = drawn(&p, &v);
        assert!(pages(&pen, "paper") && pages(&pen, "ui"));
        assert!(pen.used().iter().any(|b| b.tex == Some(Tex::Glyphs)));
    }

    #[test]
    fn every_state_draws_in_both_orientations() {
        for v in [View::ipad_landscape(), View::ipad_portrait()] {
            let mut p = fresh();
            assert!(drawn(&p, &v).vertex_count() > 300);
            p.update(&Input::key(kg_core::engine::Dir::Up), &v, 1.0 / 60.0);
            assert_eq!(p.g.state, State::Playing);
            for _ in 0..30 {
                p.update(&Input::default(), &v, 1.0 / 60.0);
            }
            assert!(drawn(&p, &v).vertex_count() > 300);
            p.g.w.kill(0, Some(1), Cause::Trail);
            for _ in 0..90 {
                p.update(&Input::default(), &v, 1.0 / 60.0);
            }
            assert_eq!(p.g.state, State::Dead);
            assert!(drawn(&p, &v).vertex_count() > 300);
        }
    }

    #[test]
    fn the_ground_covers_the_screen_past_the_safe_area() {
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
        assert!(lo <= f.x + 0.01 && hi >= f.x + f.w, "{lo}..{hi} vs {f:?}");
        assert!(top <= f.y + 0.5 && bot >= f.y + f.h - 0.5);
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
    fn the_board_shows_the_top_five_and_your_place() {
        let p = fresh();
        let rows = board_rows(&p.g);
        assert!(rows.len() == 5 || rows.len() == 6);
        assert!(rows.iter().any(|(_, id)| *id == 0));
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
    fn the_play_screen_stays_under_the_vertex_budget() {
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
        assert!(n < 120_000, "{n} vertices");
    }
}
