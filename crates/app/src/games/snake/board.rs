use super::SHEET;
use crate::draw::Draw;
use crate::paint::{Pen, Tex, Vert};
use glam::{vec2, Vec2};
use kg_core::engine::View;
use kg_core::fx::{ease_out_back, ease_out_cubic, ease_out_elastic};
use kg_core::games::snake::*;
use kg_core::geom::{Color, Rect};

const LAWN_A: Color = Color::new(0.56, 0.83, 0.32, 1.0);
const LAWN_B: Color = Color::new(0.47, 0.76, 0.26, 1.0);
const WOOD: Color = Color::new(0.74, 0.47, 0.24, 1.0);
const WOOD_LIGHT: Color = Color::new(0.90, 0.64, 0.36, 1.0);
const WOOD_DARK: Color = Color::new(0.45, 0.26, 0.12, 1.0);

pub fn meadow(pen: &mut Pen, view: &View) {
    let f = view.full();
    let side = f.w.max(f.h);
    pen.sprite(
        SHEET,
        "bg",
        Rect::new(f.x + (f.w - side) / 2.0, f.y + (f.h - side) / 2.0, side, side),
        Draw::default(),
    );
}

pub fn frame(pen: &mut Pen, board: Rect, cell: f32) {
    let t = (cell * 0.55).clamp(12.0, 26.0);
    let outer = Rect::new(board.x - t, board.y - t, board.w + t * 2.0, board.h + t * 2.0);
    for (grow, alpha) in [(14.0, 0.07), (8.0, 0.1), (3.0, 0.14)] {
        pen.round_rect(
            Rect::new(outer.x - grow * 0.5, outer.y + grow * 0.7, outer.w + grow, outer.h + grow * 0.6),
            t + grow,
            Color::new(0.0, 0.1, 0.0, alpha),
        );
    }
    pen.round_rect(outer, t * 0.9, WOOD_DARK);
    pen.round_rect(
        Rect::new(outer.x + 2.0, outer.y + 2.0, outer.w - 4.0, outer.h - t * 0.25),
        t * 0.85,
        WOOD,
    );
    pen.round_rect(
        Rect::new(outer.x + t * 0.25, outer.y + 3.0, outer.w - t * 0.5, t * 0.35),
        t * 0.2,
        WOOD_LIGHT.with_alpha(0.7),
    );
    let planks = ((outer.w + outer.h) / (cell * 1.6)).max(4.0) as i32;
    let per = (outer.w * 2.0 + outer.h * 2.0) / planks as f32;
    for i in 1..planks {
        let d = i as f32 * per;
        let (a, b) = if d < outer.w {
            (vec2(outer.x + d, outer.y + 3.0), vec2(outer.x + d, outer.y + t - 2.0))
        } else if d < outer.w + outer.h {
            let y = outer.y + d - outer.w;
            (vec2(outer.x + outer.w - 3.0, y), vec2(outer.x + outer.w - t + 2.0, y))
        } else if d < outer.w * 2.0 + outer.h {
            let x = outer.x + outer.w - (d - outer.w - outer.h);
            (vec2(x, outer.y + outer.h - 3.0), vec2(x, outer.y + outer.h - t + 2.0))
        } else {
            let y = outer.y + outer.h - (d - outer.w * 2.0 - outer.h);
            (vec2(outer.x + 3.0, y), vec2(outer.x + t - 2.0, y))
        };
        pen.line(a, b, 2.5, WOOD_DARK.with_alpha(0.45));
    }
    pen.round_rect(
        Rect::new(board.x - 3.0, board.y - 3.0, board.w + 6.0, board.h + 6.0),
        6.0,
        WOOD_DARK,
    );
}

pub fn lawn(pen: &mut Pen, g: &Snake, o: Vec2, cell: f32, board: Rect) {
    pen.rect(board, LAWN_B);
    for y in 0..g.size.y {
        for x in 0..g.size.x {
            if (x + y) % 2 == 0 {
                pen.rect(Rect::new(o.x + x as f32 * cell, o.y + y as f32 * cell, cell, cell), LAWN_A);
            }
        }
    }
    for i in 0..(g.size.x * g.size.y / 7) {
        let h = (i as u32).wrapping_mul(2654435761);
        let x = (h % 1000) as f32 / 1000.0 * board.w;
        let y = ((h / 1000) % 1000) as f32 / 1000.0 * board.h;
        let p = vec2(board.x + x, board.y + y);
        let k = cell * 0.12;
        let tint = Color::new(0.30, 0.58, 0.16, 0.35);
        pen.line(p, p + vec2(-k * 0.4, -k), 2.0, tint);
        pen.line(p, p + vec2(k * 0.4, -k * 0.9), 2.0, tint);
    }
    inner_shadow(pen, board, cell);
}

fn inner_shadow(pen: &mut Pen, board: Rect, cell: f32) {
    let edge = Color::new(0.0, 0.15, 0.0, 0.3);
    let clear = Color::new(0.0, 0.15, 0.0, 0.0);
    let d = cell * 0.5;
    let (x0, y0, x1, y1) = (board.x, board.y, board.x + board.w, board.y + board.h);
    let mut quad = |a: Vec2, b: Vec2, c: Vec2, e: Vec2, strong: f32| {
        let v = |p: Vec2, color: Color| Vert {
            pos: p,
            uv: Vec2::splat(0.5),
            color,
        };
        let edge = edge.with_alpha(edge.a * strong);
        pen.mesh(Tex::White, &[v(a, edge), v(b, edge), v(c, clear), v(e, clear)], &[0, 1, 2, 0, 2, 3]);
    };
    quad(vec2(x0, y0), vec2(x1, y0), vec2(x1, y0 + d), vec2(x0, y0 + d), 1.0);
    quad(vec2(x0, y1), vec2(x1, y1), vec2(x1, y1 - d * 0.5), vec2(x0, y1 - d * 0.5), 0.5);
    quad(vec2(x0, y0), vec2(x0, y1), vec2(x0 + d * 0.7, y1), vec2(x0 + d * 0.7, y0), 0.7);
    quad(vec2(x1, y0), vec2(x1, y1), vec2(x1 - d * 0.7, y1), vec2(x1 - d * 0.7, y0), 0.7);
}

pub fn fruit(pen: &mut Pen, g: &Snake, o: Vec2, cell: f32) {
    let f = o + g.food.as_vec2() * cell + vec2(cell, cell) / 2.0;
    let pop = ease_out_elastic(g.food_age / 0.7).max(0.01);
    let wave = (g.clock * 3.2).sin();
    let lift = (wave * 0.5 + 0.5) * cell * 0.14;
    let drop = (1.0 - ease_out_cubic(g.food_age / 0.35)) * cell * 0.9;
    let squash = 1.0 + (g.clock * 6.4).cos() * 0.04;
    let h = cell * 1.45 * pop;
    let shadow = 1.0 - lift / (cell * 0.3);
    pen.soft_shadow(f + vec2(cell * 0.06, cell * 0.36), h * 0.36 * shadow, h * 0.12 * shadow, 0.7);
    let c = f - vec2(0.0, lift + drop + cell * 0.05);
    let glow = 0.18 + 0.08 * (g.clock * 4.0).sin();
    pen.circle(c, h * 0.62, Color::new(1.0, 1.0, 0.7, glow * pop.min(1.0)));
    let name = g.fruit();
    let s = crate::art::size(SHEET, name);
    let w = h * s.x / s.y.max(1.0);
    pen.sprite(
        SHEET,
        name,
        Rect::new(c.x - w * squash / 2.0, c.y - h / squash / 2.0, w * squash, h / squash),
        Draw::default(),
    );
    let tw = (g.clock * 1.3).rem_euclid(2.6);
    if tw < 0.5 {
        let k = (tw / 0.5 * std::f32::consts::PI).sin();
        let p = c + vec2(-w * 0.18, -h * 0.2);
        pen.star(p, cell * 0.2 * k, g.clock, Color::new(1.0, 1.0, 1.0, 0.9));
    }
}

pub fn eat_ring(pen: &mut Pen, g: &Snake, o: Vec2, cell: f32) {
    let t = g.eat_age / 0.4;
    if t >= 1.0 {
        return;
    }
    let c = o + g.eaten_at.as_vec2() * cell + vec2(cell, cell) / 2.0;
    let r = cell * (0.4 + 1.4 * ease_out_cubic(t));
    pen.ring(c, r, cell * 0.16 * (1.0 - t), Color::new(1.0, 0.95, 0.6, 1.0 - t));
}

pub fn swipe_hint(pen: &mut Pen, view: &View, clock: f32, dir: Vec2) {
    let t = clock.rem_euclid(1.4) / 1.4;
    let move_t = ease_out_back((t / 0.55).min(1.0));
    let fade = if t < 0.1 {
        t / 0.1
    } else if t > 0.75 {
        (1.0 - (t - 0.75) / 0.25).max(0.0)
    } else {
        1.0
    };
    let base = vec2(view.w / 2.0, view.h - 150.0);
    let travel = 130.0;
    let p = base - dir * travel * 0.5 + dir * travel * move_t;
    let trail = base - dir * travel * 0.5;
    let dots = 6;
    for i in 0..dots {
        let k = i as f32 / dots as f32;
        let q = trail.lerp(p, k);
        pen.circle(q, 6.0 + 4.0 * k, Color::new(1.0, 1.0, 1.0, 0.5 * k * fade));
    }
    let tip = base + dir * (travel * 0.5 + 46.0);
    let side = dir.perp() * 22.0;
    let ink = Color::new(0.10, 0.20, 0.45, 0.85 * fade);
    pen.tri(tip + dir * 4.0, tip - dir * 34.0 + side * 1.2, tip - dir * 34.0 - side * 1.2, ink);
    pen.tri(tip, tip - dir * 30.0 + side, tip - dir * 30.0 - side, Color::new(1.0, 1.0, 1.0, 0.95 * fade));
    hand(pen, p, fade);
}

fn hand(pen: &mut Pen, tip: Vec2, fade: f32) {
    let skin = Color::new(1.0, 0.84, 0.68, fade);
    let edge = Color::new(0.45, 0.25, 0.18, 0.9 * fade);
    let shade = Color::new(0.93, 0.70, 0.55, fade);
    let palm = tip + vec2(18.0, 64.0);
    let finger = |pen: &mut Pen, a: Vec2, b: Vec2, w: f32, col: Color| {
        pen.line(a, b, w, col);
        pen.circle(a, w / 2.0, col);
        pen.circle(b, w / 2.0, col);
    };
    pen.ellipse(palm + vec2(6.0, 10.0), 38.0, 34.0, 0.0, Color::new(0.0, 0.0, 0.0, 0.15 * fade));
    finger(pen, tip, palm, 30.0, edge);
    pen.ellipse(palm, 38.0, 34.0, 0.0, edge);
    finger(pen, tip, palm, 24.0, skin);
    pen.ellipse(palm, 35.0, 31.0, 0.0, skin);
    for k in 0..3 {
        let a = palm + vec2(-6.0 + k as f32 * 14.0, -24.0);
        pen.circle(a, 11.0, edge);
        pen.circle(a, 9.0, shade);
    }
    pen.ellipse(palm + vec2(-30.0, 2.0), 12.0, 20.0, -0.5, edge);
    pen.ellipse(palm + vec2(-30.0, 2.0), 10.0, 18.0, -0.5, skin);
    pen.ellipse(tip + vec2(0.0, 6.0), 7.0, 9.0, 0.0, Color::new(1.0, 0.95, 0.92, fade));
    pen.ring(tip, 26.0 + 8.0 * (1.0 - fade), 4.0, Color::new(1.0, 1.0, 1.0, 0.6 * fade));
}
