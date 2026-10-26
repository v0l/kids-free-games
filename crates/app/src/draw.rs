use crate::paint::{Pen, Tex, Vert};
use glam::{vec2, Vec2};
use kg_core::engine::View;
use kg_core::font::font_params;
use kg_core::geom::{Color, Rect, WHITE};
use std::f32::consts::{FRAC_PI_2, PI, TAU};

pub const INK: Color = Color::new(0.16, 0.12, 0.24, 1.0);
pub const PLUM: Color = Color::from_rgba(40, 30, 60, 255);

#[derive(Clone, Copy, Debug)]
pub struct Draw {
    pub tint: Color,
    pub rotation: f32,
    pub flip_x: bool,
    pub flip_y: bool,
    pub pivot: Vec2,
}

impl Default for Draw {
    fn default() -> Self {
        Self {
            tint: WHITE,
            rotation: 0.0,
            flip_x: false,
            flip_y: false,
            pivot: vec2(0.5, 0.5),
        }
    }
}

impl Draw {
    pub fn tint(c: Color) -> Self {
        Self {
            tint: c,
            ..Default::default()
        }
    }

    pub fn rot(a: f32) -> Self {
        Self {
            rotation: a,
            ..Default::default()
        }
    }
}

impl Pen {
    pub fn rect(&mut self, r: Rect, c: Color) {
        let h = Vec2::splat(0.5);
        self.quad(
            Tex::White,
            [
                vec2(r.x, r.y),
                vec2(r.x + r.w, r.y),
                vec2(r.x + r.w, r.y + r.h),
                vec2(r.x, r.y + r.h),
            ],
            [h; 4],
            c,
        );
    }

    pub fn gradient(&mut self, r: Rect, top: Color, bottom: Color) {
        let h = Vec2::splat(0.5);
        let v = |p: Vec2, c: Color| Vert { pos: p, uv: h, color: c };
        self.mesh(
            Tex::White,
            &[
                v(vec2(r.x, r.y), top),
                v(vec2(r.x + r.w, r.y), top),
                v(vec2(r.x + r.w, r.y + r.h), bottom),
                v(vec2(r.x, r.y + r.h), bottom),
            ],
            &[0, 1, 2, 0, 2, 3],
        );
    }

    pub fn fill_screen(&mut self, view: &View, c: Color) {
        self.rect(view.full(), c);
    }

    pub fn sky(&mut self, view: &View, top: Color, bottom: Color) {
        self.gradient(view.full(), top, bottom);
    }

    pub fn fan(&mut self, c: Vec2, rx: f32, ry: f32, rot: f32, from: f32, to: f32, col: Color) {
        let n = self.sides(rx.abs().max(ry.abs())).max(3);
        let segs = ((n as f32) * ((to - from).abs() / TAU)).ceil().max(2.0) as usize;
        let h = Vec2::splat(0.5);
        let (s, k) = rot.sin_cos();
        let mut verts = Vec::with_capacity(segs + 2);
        verts.push(Vert { pos: c, uv: h, color: col });
        for i in 0..=segs {
            let a = from + (to - from) * i as f32 / segs as f32;
            let (px, py) = (rx * a.cos(), ry * a.sin());
            verts.push(Vert {
                pos: c + vec2(px * k - py * s, px * s + py * k),
                uv: h,
                color: col,
            });
        }
        let idx: Vec<u32> = (0..segs as u32).flat_map(|i| [0, i + 1, i + 2]).collect();
        self.mesh(Tex::White, &verts, &idx);
    }

    pub fn circle(&mut self, c: Vec2, r: f32, col: Color) {
        self.fan(c, r, r, 0.0, 0.0, TAU, col);
    }

    pub fn ellipse(&mut self, c: Vec2, rx: f32, ry: f32, rot: f32, col: Color) {
        self.fan(c, rx, ry, rot, 0.0, TAU, col);
    }

    pub fn ring(&mut self, c: Vec2, r: f32, width: f32, col: Color) {
        let n = self.sides(r);
        let h = Vec2::splat(0.5);
        let mut verts = vec![];
        for i in 0..=n {
            let a = TAU * i as f32 / n as f32;
            let d = vec2(a.cos(), a.sin());
            verts.push(Vert { pos: c + d * (r - width / 2.0), uv: h, color: col });
            verts.push(Vert { pos: c + d * (r + width / 2.0), uv: h, color: col });
        }
        let idx: Vec<u32> = (0..n as u32)
            .flat_map(|i| {
                let a = i * 2;
                [a, a + 1, a + 3, a, a + 3, a + 2]
            })
            .collect();
        self.mesh(Tex::White, &verts, &idx);
    }

    pub fn line(&mut self, a: Vec2, b: Vec2, width: f32, col: Color) {
        let d = (b - a).normalize_or_zero().perp() * width / 2.0;
        let h = Vec2::splat(0.5);
        self.quad(Tex::White, [a + d, b + d, b - d, a - d], [h; 4], col);
    }

    pub fn round_rect(&mut self, r: Rect, radius: f32, col: Color) {
        let rad = radius.min(r.w / 2.0).min(r.h / 2.0).max(0.0);
        if rad <= 0.0 {
            self.rect(r, col);
            return;
        }
        let segs = ((rad * self.detail * 0.25) as usize).clamp(3, 24);
        let h = Vec2::splat(0.5);
        let corners = [
            (vec2(r.x + r.w - rad, r.y + r.h - rad), 0.0),
            (vec2(r.x + rad, r.y + r.h - rad), FRAC_PI_2),
            (vec2(r.x + rad, r.y + rad), PI),
            (vec2(r.x + r.w - rad, r.y + rad), PI * 1.5),
        ];
        let mut verts = vec![Vert { pos: r.center(), uv: h, color: col }];
        for (c, start) in corners {
            for k in 0..=segs {
                let a = start + FRAC_PI_2 * k as f32 / segs as f32;
                verts.push(Vert {
                    pos: c + vec2(a.cos(), a.sin()) * rad,
                    uv: h,
                    color: col,
                });
            }
        }
        let n = verts.len() as u32 - 1;
        let idx: Vec<u32> = (0..n).flat_map(|i| [0, i + 1, (i + 1) % n + 1]).collect();
        self.mesh(Tex::White, &verts, &idx);
    }

    pub fn star(&mut self, c: Vec2, r: f32, angle: f32, col: Color) {
        for k in 0..5 {
            let a = angle + k as f32 * TAU / 5.0 - FRAC_PI_2;
            let b0 = a - PI / 5.0;
            let b1 = a + PI / 5.0;
            self.tri(
                c + vec2(a.cos(), a.sin()) * r,
                c + vec2(b0.cos(), b0.sin()) * r * 0.42,
                c + vec2(b1.cos(), b1.sin()) * r * 0.42,
                col,
            );
        }
        self.fan(c, r * 0.45, r * 0.45, 0.0, 0.0, TAU, col);
    }

    pub fn soft_shadow(&mut self, c: Vec2, rx: f32, ry: f32, alpha: f32) {
        for i in 0..4 {
            let k = 1.0 - i as f32 * 0.18;
            self.ellipse(c, rx * k, ry * k, 0.0, Color::new(0.0, 0.0, 0.0, alpha * 0.3));
        }
    }

    pub fn gloss(&mut self, r: Rect, radius: f32) {
        self.round_rect(
            Rect::new(r.x + r.w * 0.08, r.y + r.h * 0.06, r.w * 0.84, r.h * 0.32),
            radius * 0.6,
            Color::new(1.0, 1.0, 1.0, 0.22),
        );
    }

    pub fn shine_sweep(&mut self, r: Rect, t: f32) {
        let x = r.x - r.w * 0.5 + (t.rem_euclid(3.0) / 3.0) * r.w * 2.0;
        if x > r.x + r.w || x + r.w * 0.2 < r.x {
            return;
        }
        let a = x.max(r.x);
        let b = (x + r.w * 0.18).min(r.x + r.w);
        if b > a {
            self.rect(Rect::new(a, r.y, b - a, r.h), Color::new(1.0, 1.0, 1.0, 0.18));
        }
    }

    pub fn vignette(&mut self, view: &View, strength: f32) {
        let f = view.full();
        let inset = view.w.min(view.h) * 0.22;
        let edge = Color::new(0.0, 0.0, 0.05, strength * 0.45);
        let clear = Color::new(0.0, 0.0, 0.05, 0.0);
        let outer = [
            vec2(f.x, f.y),
            vec2(f.x + f.w, f.y),
            vec2(f.x + f.w, f.y + f.h),
            vec2(f.x, f.y + f.h),
        ];
        let inner = [
            vec2(f.x + inset, f.y + inset),
            vec2(f.x + f.w - inset, f.y + inset),
            vec2(f.x + f.w - inset, f.y + f.h - inset),
            vec2(f.x + inset, f.y + f.h - inset),
        ];
        let h = Vec2::splat(0.5);
        let mut verts = vec![];
        for k in 0..4 {
            verts.push(Vert { pos: outer[k], uv: h, color: edge });
            verts.push(Vert { pos: inner[k], uv: h, color: clear });
        }
        let mut idx = vec![];
        for k in 0..4u32 {
            let (o0, i0, o1, i1) = (k * 2, k * 2 + 1, ((k + 1) % 4) * 2, ((k + 1) % 4) * 2 + 1);
            idx.extend_from_slice(&[o0, o1, i1, o0, i1, i0]);
        }
        self.mesh(Tex::White, &verts, &idx);
    }

    pub fn sprite(&mut self, sheet: &'static str, name: &str, dest: Rect, d: Draw) {
        let Some(f) = crate::art::frame(sheet, name) else {
            self.rect(dest, kg_core::geom::MAGENTA);
            return;
        };
        let (pw, ph) = crate::art::page_size(sheet, f.page);
        let (mut u0, mut v0) = (f.src.x / pw, f.src.y / ph);
        let (mut u1, mut v1) = ((f.src.x + f.src.w) / pw, (f.src.y + f.src.h) / ph);
        if d.flip_x {
            std::mem::swap(&mut u0, &mut u1);
        }
        if d.flip_y {
            std::mem::swap(&mut v0, &mut v1);
        }
        let pivot = vec2(dest.x + dest.w * d.pivot.x, dest.y + dest.h * d.pivot.y);
        let (s, c) = d.rotation.sin_cos();
        let rot = |p: Vec2| {
            let q = p - pivot;
            pivot + vec2(q.x * c - q.y * s, q.x * s + q.y * c)
        };
        self.quad(
            Tex::Page(sheet, f.page as u16),
            [
                rot(vec2(dest.x, dest.y)),
                rot(vec2(dest.x + dest.w, dest.y)),
                rot(vec2(dest.x + dest.w, dest.y + dest.h)),
                rot(vec2(dest.x, dest.y + dest.h)),
            ],
            [vec2(u0, v0), vec2(u1, v0), vec2(u1, v1), vec2(u0, v1)],
            d.tint,
        );
    }

    pub fn sprite_at(&mut self, sheet: &'static str, name: &str, center: Vec2, height: f32, d: Draw) {
        let s = crate::art::size(sheet, name);
        let w = height * s.x / s.y.max(1.0);
        self.sprite(sheet, name, Rect::new(center.x - w / 2.0, center.y - height / 2.0, w, height), d);
    }

    pub fn sprite_w(&mut self, sheet: &'static str, name: &str, center: Vec2, width: f32, d: Draw) {
        let s = crate::art::size(sheet, name);
        let h = width * s.y / s.x.max(1.0);
        self.sprite(sheet, name, Rect::new(center.x - width / 2.0, center.y - h / 2.0, width, h), d);
    }

    pub fn nine(&mut self, sheet: &'static str, name: &str, dest: Rect, border: f32, scale: f32, tint: Color) {
        let Some(f) = crate::art::frame(sheet, name) else {
            self.rect(dest, kg_core::geom::MAGENTA);
            return;
        };
        let (pw, ph) = crate::art::page_size(sheet, f.page);
        let tex = Tex::Page(sheet, f.page as u16);
        let xs = kg_core::art::nine_spans(f.src.x, f.src.w, dest.x, dest.w, border, scale);
        let ys = kg_core::art::nine_spans(f.src.y, f.src.h, dest.y, dest.h, border, scale);
        for (sx, sw, dx, dw) in xs {
            for (sy, sh, dy, dh) in ys {
                if dw <= 0.0 || dh <= 0.0 {
                    continue;
                }
                let (u0, v0, u1, v1) = (sx / pw, sy / ph, (sx + sw) / pw, (sy + sh) / ph);
                self.quad(
                    tex,
                    [vec2(dx, dy), vec2(dx + dw, dy), vec2(dx + dw, dy + dh), vec2(dx, dy + dh)],
                    [vec2(u0, v0), vec2(u1, v0), vec2(u1, v1), vec2(u0, v1)],
                    tint,
                );
            }
        }
    }

    pub fn glyphs(&mut self, s: &str, at: Vec2, size: f32, fill: Color, edge: Color, shadow: Color) {
        let (px, k) = font_params(size);
        let Some((placed, width, atlas)) = crate::text::layout(s, px) else {
            return;
        };
        let start_x = at.x - width * k / 2.0;
        let base_y = at.y + px as f32 * 0.36 * k;
        let drop = px as f32 * 0.07 * k;
        for (col, off, use_edge) in [(shadow, drop, true), (edge, 0.0, true), (fill, 0.0, false)] {
            if col.a <= 0.0 {
                continue;
            }
            for (g, gx) in &placed {
                if g.w <= 0.0 || g.fill.w <= 0.0 {
                    continue;
                }
                let src = if use_edge { g.edge } else { g.fill };
                let x = start_x + (gx + g.left) * k;
                let y = base_y + g.top * k + off;
                let (w, h) = (g.w * k, g.h * k);
                let a = atlas as f32;
                let (u0, v0, u1, v1) = (src.x / a, src.y / a, (src.x + src.w) / a, (src.y + src.h) / a);
                self.quad(
                    Tex::Glyphs,
                    [vec2(x, y), vec2(x + w, y), vec2(x + w, y + h), vec2(x, y + h)],
                    [vec2(u0, v0), vec2(u1, v0), vec2(u1, v1), vec2(u0, v1)],
                    col,
                );
            }
        }
    }

    pub fn text_outline(&mut self, s: &str, cx: f32, cy: f32, size: f32, fill: Color, outline: Color) {
        let shadow = Color::new(outline.r * 0.6, outline.g * 0.6, outline.b * 0.6, outline.a * 0.55);
        self.glyphs(s, vec2(cx, cy), size, fill, outline, shadow);
    }

    pub fn text_center(&mut self, s: &str, cx: f32, cy: f32, size: f32, col: Color) {
        self.glyphs(s, vec2(cx, cy), size, col, Color::new(0.0, 0.0, 0.0, 0.0), Color::new(0.0, 0.0, 0.0, 0.0));
    }

    pub fn text_left(&mut self, s: &str, x: f32, cy: f32, size: f32, col: Color) {
        let w = measure(s, size);
        self.text_center(s, x + w / 2.0, cy, size, col);
    }

    pub fn text_shadow(&mut self, s: &str, cx: f32, cy: f32, size: f32, col: Color) {
        let outline = if col.r + col.g + col.b > 2.2 {
            PLUM
        } else {
            Color::new(1.0, 1.0, 1.0, 0.9)
        };
        self.text_outline(s, cx, cy, size, col, outline.with_alpha(col.a));
    }

    pub fn score_text(&mut self, s: &str, cx: f32, cy: f32, size: f32, bump: f32) {
        let k = kg_core::fx::bump_scale(bump, 0.35);
        self.text_outline(s, cx, cy, size * k, WHITE, PLUM);
    }

    pub fn big_score(&mut self, view: &View, value: &str, y: f32, bump: f32) {
        let k = kg_core::fx::bump_scale(bump, 0.3);
        self.text_outline(value, view.w / 2.0, y, 88.0 * k, WHITE, Color::from_rgba(50, 20, 70, 255));
    }
}

pub fn measure(s: &str, size: f32) -> f32 {
    kg_core::font::measure(s, size)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pen() -> Pen {
        let mut p = Pen::default();
        p.begin(View::ipad_landscape(), 1.0);
        p
    }

    #[test]
    fn round_rects_stay_inside_their_box_and_close_up() {
        let mut p = pen();
        let r = Rect::new(10.0, 20.0, 100.0, 40.0);
        p.round_rect(r, 12.0, WHITE);
        let b = &p.used()[0];
        for v in &b.pos {
            assert!(v[0] >= 10.0 - 1e-3 && v[0] <= 110.0 + 1e-3);
            assert!(-v[1] >= 20.0 - 1e-3 && -v[1] <= 60.0 + 1e-3);
        }
        assert!(b.idx.iter().all(|i| (*i as usize) < b.pos.len()));
        assert_eq!(b.idx.len() % 3, 0);
    }

    #[test]
    fn big_circles_get_more_sides() {
        let mut p = pen();
        p.circle(Vec2::ZERO, 5.0, WHITE);
        let small = p.vertex_count();
        p.begin(View::ipad_landscape(), 3.0);
        p.circle(Vec2::ZERO, 150.0, WHITE);
        assert!(p.vertex_count() > small * 3);
    }

    #[test]
    fn a_missing_sprite_shows_magenta_instead_of_vanishing() {
        let mut p = pen();
        p.sprite("ui", "no_such_sprite", Rect::new(0.0, 0.0, 10.0, 10.0), Draw::default());
        assert_eq!(p.used()[0].tex, Some(Tex::White));
        assert_eq!(p.used()[0].color[0], crate::paint::linear(kg_core::geom::MAGENTA));
    }

    #[test]
    fn a_sprite_maps_its_frame_into_page_uvs() {
        let mut p = pen();
        p.sprite("ui", "p_panel", Rect::new(0.0, 0.0, 10.0, 10.0), Draw::default());
        let b = &p.used()[0];
        assert!(matches!(b.tex, Some(Tex::Page("ui", _))));
        for uv in &b.uv {
            assert!((0.0..=1.0).contains(&uv[0]) && (0.0..=1.0).contains(&uv[1]));
        }
    }

    #[test]
    fn outlined_text_draws_shadow_edge_and_fill_per_glyph() {
        let mut p = pen();
        p.text_outline("Hi", 100.0, 100.0, 40.0, WHITE, PLUM);
        let b = &p.used()[0];
        assert_eq!(b.tex, Some(Tex::Glyphs));
        assert_eq!(b.pos.len(), 2 * 3 * 4);
        p.begin(View::ipad_landscape(), 1.0);
        p.text_center("Hi", 100.0, 100.0, 40.0, WHITE);
        assert_eq!(p.vertex_count(), 2 * 4);
    }

    #[test]
    fn text_is_centred_on_its_point() {
        let mut p = pen();
        p.text_center("Kids", 300.0, 200.0, 40.0, WHITE);
        let xs: Vec<f32> = p.used()[0].pos.iter().map(|v| v[0]).collect();
        let (lo, hi) = xs.iter().fold((f32::MAX, f32::MIN), |(a, b), x| (a.min(*x), b.max(*x)));
        assert!(((lo + hi) / 2.0 - 300.0).abs() < 6.0, "{lo} {hi}");
    }
}
