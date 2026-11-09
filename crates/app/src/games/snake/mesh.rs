use crate::paint::{Pen, Tex, Vert};
use glam::{vec2, Vec2};
use kg_core::geom::{Color, Rect};

pub struct Clipped {
    clip: Rect,
    verts: Vec<Vert>,
    idx: Vec<u32>,
}

fn v(pos: Vec2, color: Color) -> Vert {
    Vert {
        pos,
        uv: Vec2::splat(0.5),
        color,
    }
}

fn lerp_vert(a: &Vert, b: &Vert, t: f32) -> Vert {
    v(a.pos.lerp(b.pos, t), a.color.lerp(b.color, t))
}

fn clip_edge(poly: &[Vert], inside: impl Fn(Vec2) -> f32) -> Vec<Vert> {
    let mut out = Vec::with_capacity(poly.len() + 2);
    for i in 0..poly.len() {
        let a = &poly[i];
        let b = &poly[(i + 1) % poly.len()];
        let (da, db) = (inside(a.pos), inside(b.pos));
        if da >= 0.0 {
            out.push(*a);
        }
        if (da >= 0.0) != (db >= 0.0) {
            out.push(lerp_vert(a, b, da / (da - db)));
        }
    }
    out
}

impl Clipped {
    pub fn new(clip: Rect) -> Self {
        Self {
            clip,
            verts: vec![],
            idx: vec![],
        }
    }

    pub fn tri(&mut self, a: Vert, b: Vert, c: Vert) {
        let r = self.clip;
        let inside = |p: Vec2| p.x >= r.x && p.y >= r.y && p.x <= r.x + r.w && p.y <= r.y + r.h;
        let all_in = inside(a.pos) && inside(b.pos) && inside(c.pos);
        let poly = if all_in {
            vec![a, b, c]
        } else {
            let lo = a.pos.min(b.pos).min(c.pos);
            let hi = a.pos.max(b.pos).max(c.pos);
            if hi.x < r.x || hi.y < r.y || lo.x > r.x + r.w || lo.y > r.y + r.h {
                return;
            }
            let mut p = vec![a, b, c];
            p = clip_edge(&p, |q| q.x - r.x);
            p = clip_edge(&p, |q| r.x + r.w - q.x);
            p = clip_edge(&p, |q| q.y - r.y);
            p = clip_edge(&p, |q| r.y + r.h - q.y);
            p
        };
        if poly.len() < 3 {
            return;
        }
        let base = self.verts.len() as u32;
        self.verts.extend(poly.iter().copied());
        for i in 1..poly.len() as u32 - 1 {
            self.idx.extend_from_slice(&[base, base + i, base + i + 1]);
        }
    }

    pub fn flat_tri(&mut self, a: Vec2, b: Vec2, c: Vec2, col: Color) {
        self.tri(v(a, col), v(b, col), v(c, col));
    }

    pub fn quad(&mut self, p: [Vec2; 4], c: [Color; 4]) {
        self.tri(v(p[0], c[0]), v(p[1], c[1]), v(p[2], c[2]));
        self.tri(v(p[0], c[0]), v(p[2], c[2]), v(p[3], c[3]));
    }

    pub fn ellipse(&mut self, c: Vec2, radii: Vec2, rot: f32, col: Color) {
        self.ellipse_shaded(c, radii, rot, col, col);
    }

    pub fn ellipse_shaded(&mut self, c: Vec2, radii: Vec2, rot: f32, centre: Color, rim: Color) {
        let n = ((radii.max_element() * 0.5) as usize).clamp(16, 48);
        let f = Vec2::from_angle(rot);
        let at = |i: usize| {
            let a = std::f32::consts::TAU * i as f32 / n as f32;
            c + f.rotate(vec2(a.cos() * radii.x, a.sin() * radii.y))
        };
        for i in 0..n {
            self.tri(v(c, centre), v(at(i), rim), v(at(i + 1), rim));
        }
    }

    pub fn line(&mut self, a: Vec2, b: Vec2, width: f32, col: Color) {
        let d = (b - a).normalize_or_zero().perp() * width / 2.0;
        self.quad([a + d, b + d, b - d, a - d], [col; 4]);
    }

    pub fn star(&mut self, c: Vec2, r: f32, angle: f32, col: Color) {
        use std::f32::consts::{FRAC_PI_2, PI, TAU};
        for k in 0..5 {
            let a = angle + k as f32 * TAU / 5.0 - FRAC_PI_2;
            let p = |ang: f32, rad: f32| c + vec2(ang.cos(), ang.sin()) * rad;
            self.flat_tri(p(a, r), p(a - PI / 5.0, r * 0.45), p(a + PI / 5.0, r * 0.45), col);
            self.flat_tri(c, p(a - PI / 5.0, r * 0.45), p(a + PI / 5.0, r * 0.45), col);
        }
    }

    pub fn flush(&mut self, pen: &mut Pen) {
        if !self.idx.is_empty() {
            pen.mesh(Tex::White, &self.verts, &self.idx);
        }
        self.verts.clear();
        self.idx.clear();
    }

    pub fn set_clip(&mut self, clip: Rect) {
        self.clip = clip;
    }
}

pub struct Sample {
    pub pos: Vec2,
    pub dir: Vec2,
    pub s: f32,
}

pub fn chaikin(points: &[Vec2], rounds: usize) -> Vec<Vec2> {
    let mut p = points.to_vec();
    for _ in 0..rounds {
        if p.len() < 3 {
            return p;
        }
        let mut out = Vec::with_capacity(p.len() * 2);
        out.push(p[0]);
        for w in p.windows(2) {
            out.push(w[0].lerp(w[1], 0.25));
            out.push(w[0].lerp(w[1], 0.75));
        }
        out.push(*p.last().unwrap());
        p = out;
    }
    p
}

pub fn resample(points: &[Vec2], step: f32) -> Vec<Sample> {
    let mut out = vec![];
    if points.len() < 2 {
        if let Some(p) = points.first() {
            out.push(Sample {
                pos: *p,
                dir: Vec2::X,
                s: 0.0,
            });
        }
        return out;
    }
    let mut s = 0.0;
    let mut next = 0.0;
    for w in points.windows(2) {
        let seg = w[1] - w[0];
        let len = seg.length();
        if len < 1e-4 {
            continue;
        }
        let dir = seg / len;
        while next <= s + len {
            out.push(Sample {
                pos: w[0] + dir * (next - s),
                dir,
                s: next,
            });
            next += step;
        }
        s += len;
    }
    let last = *points.last().unwrap();
    let dir = out.last().map_or(Vec2::X, |l| l.dir);
    if out.last().is_none_or(|l| (l.pos - last).length() > step * 0.25) {
        out.push(Sample { pos: last, dir, s });
    }
    let n = out.len();
    for i in 0..n {
        let a = out[i.saturating_sub(1)].pos;
        let b = out[(i + 1).min(n - 1)].pos;
        let d = (b - a).normalize_or_zero();
        if d != Vec2::ZERO {
            out[i].dir = d;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_triangle_half_outside_is_cut_at_the_edge() {
        let mut m = Clipped::new(Rect::new(0.0, 0.0, 10.0, 10.0));
        m.flat_tri(vec2(5.0, 5.0), vec2(15.0, 5.0), vec2(5.0, 8.0), Color::new(1.0, 1.0, 1.0, 1.0));
        assert!(m.verts.iter().all(|p| p.pos.x <= 10.0 + 1e-4));
        assert!(m.verts.len() >= 4);
        let mut out = Clipped::new(Rect::new(0.0, 0.0, 10.0, 10.0));
        out.flat_tri(vec2(20.0, 5.0), vec2(25.0, 5.0), vec2(22.0, 8.0), Color::new(1.0, 1.0, 1.0, 1.0));
        assert!(out.idx.is_empty());
    }

    #[test]
    fn a_rounded_corner_stays_close_to_the_cells_and_resamples_evenly() {
        let pts = [vec2(0.0, 0.0), vec2(2.0, 0.0), vec2(2.0, 2.0)];
        let smooth = chaikin(&pts, 3);
        assert_eq!(smooth[0], pts[0]);
        assert_eq!(*smooth.last().unwrap(), pts[2]);
        let s = resample(&smooth, 0.1);
        for w in s.windows(2) {
            assert!((w[1].pos - w[0].pos).length() <= 0.1 + 1e-3);
            assert!(w[0].dir.dot(w[1].dir) > 0.8);
        }
    }
}
