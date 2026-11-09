use super::mesh::{chaikin, resample, Clipped, Sample};
use crate::paint::Pen;
use glam::{vec2, Vec2};
use kg_core::games::snake::*;
use kg_core::geom::{Color, Rect};
use std::f32::consts::TAU;

const BODY: Color = Color::new(0.20, 0.50, 0.90, 1.0);
const BODY_DEEP: Color = Color::new(0.13, 0.34, 0.72, 1.0);
const BODY_LIGHT: Color = Color::new(0.52, 0.78, 1.0, 1.0);
const BELLY: Color = Color::new(1.0, 0.93, 0.74, 1.0);
const OUTLINE: Color = Color::new(0.07, 0.17, 0.40, 1.0);
const SPOT: Color = Color::new(1.0, 0.82, 0.18, 1.0);
const SPOT_EDGE: Color = Color::new(0.95, 0.52, 0.10, 1.0);
const DEAD_BODY: Color = Color::new(0.55, 0.60, 0.72, 1.0);
const DEAD_DEEP: Color = Color::new(0.40, 0.44, 0.56, 1.0);
const DEAD_LIGHT: Color = Color::new(0.78, 0.82, 0.90, 1.0);
const SHADOW: Color = Color::new(0.0, 0.18, 0.02, 0.28);
const BLUSH: Color = Color::new(1.0, 0.45, 0.55, 0.55);
const WHITE: Color = Color::new(1.0, 1.0, 1.0, 1.0);
const PUPIL: Color = Color::new(0.05, 0.06, 0.12, 1.0);
const TONGUE: Color = Color::new(0.92, 0.18, 0.32, 1.0);
const MOUTH_IN: Color = Color::new(0.45, 0.06, 0.16, 1.0);

struct Palette {
    body: Color,
    deep: Color,
    light: Color,
}

fn palette(g: &Snake) -> Palette {
    let k = if g.state == State::Dead {
        (g.dead_age / 0.4).clamp(0.0, 1.0)
    } else {
        0.0
    };
    Palette {
        body: BODY.lerp(DEAD_BODY, k),
        deep: BODY_DEEP.lerp(DEAD_DEEP, k),
        light: BODY_LIGHT.lerp(DEAD_LIGHT, k),
    }
}

fn path(g: &Snake) -> Vec<Vec2> {
    let mut chain = g.chain(g.frac());
    if let Some(h) = chain.first_mut() {
        *h += g.nudge;
    }
    let mut pts: Vec<Vec2> = Vec::with_capacity(chain.len() * 2);
    for (i, p) in chain.iter().enumerate() {
        pts.push(*p);
        if i + 1 < chain.len() {
            let corner = chain.get(i + 2).is_some_and(|q| {
                let a = chain[i + 1] - *p;
                let b = *q - chain[i + 1];
                a.perp_dot(b).abs() > 0.1
            });
            if !corner {
                pts.push((*p + chain[i + 1]) * 0.5);
            }
        }
    }
    chaikin(&pts, 3)
}

fn radius_at(g: &Snake, s: f32, total: f32, cell: f32) -> f32 {
    let base = cell * 0.44;
    let tail = 1.8_f32.min(total * 0.5);
    let taper = if s > total - tail {
        let t = (total - s) / tail;
        0.22 + 0.78 * (t * (2.0 - t)).sqrt()
    } else {
        1.0
    };
    let neck = 1.0 - 0.08 * (1.0 - ((s - 0.35) / 0.6).clamp(0.0, 1.0)).max(0.0) * ((s - 0.1) / 0.3).clamp(0.0, 1.0);
    let swell = g
        .bulges
        .iter()
        .map(|b| {
            let at = *b as f32 + g.frac();
            1.0 + 0.45 * (1.0 - ((s - at).abs() / 0.9)).max(0.0).powf(1.5)
        })
        .fold(1.0, f32::max);
    base * taper * neck * swell
}

struct Ring {
    c: Vec2,
    n: Vec2,
    r: f32,
    s: f32,
    dir: Vec2,
}

fn rings(g: &Snake, cell: f32, o: Vec2) -> Vec<Ring> {
    let pts = path(g);
    let samples: Vec<Sample> = resample(&pts, 0.12);
    let total = samples.last().map_or(0.0, |s| s.s);
    let kick = g.turn_kick;
    samples
        .iter()
        .map(|sm| {
            let stretch = 1.0 + 0.08 * kick * (1.0 - (sm.s / 1.2).min(1.0));
            let r = radius_at(g, sm.s, total, cell) / stretch.sqrt();
            Ring {
                c: o + (sm.pos + vec2(0.5, 0.5)) * cell,
                n: sm.dir.perp(),
                r,
                s: sm.s,
                dir: sm.dir,
            }
        })
        .collect()
}

fn strip(m: &mut Clipped, rings: &[Ring], off: Vec2, inner: impl Fn(&Ring) -> (f32, f32), cols: impl Fn(&Ring) -> (Color, Color)) {
    for w in rings.windows(2) {
        let (a, b) = (&w[0], &w[1]);
        let (a0, a1) = inner(a);
        let (b0, b1) = inner(b);
        let (ca0, ca1) = cols(a);
        let (cb0, cb1) = cols(b);
        m.quad(
            [
                a.c + off + a.n * a0,
                b.c + off + b.n * b0,
                b.c + off + b.n * b1,
                a.c + off + a.n * a1,
            ],
            [ca0, cb0, cb1, ca1],
        );
    }
}

fn caps(m: &mut Clipped, rings: &[Ring], off: Vec2, grow: f32, col: Color, head_r: f32) {
    if let Some(t) = rings.last() {
        m.ellipse(t.c + off, Vec2::splat(t.r + grow), 0.0, col);
    }
    if let Some(h) = rings.first() {
        m.ellipse(h.c + off, Vec2::splat(head_r + grow), 0.0, col);
    }
}

fn tube(m: &mut Clipped, g: &Snake, rings: &[Ring], cell: f32, head: &Head) {
    let pal = palette(g);
    let line = cell * 0.07;
    let shadow = vec2(cell * 0.16, cell * 0.24);
    let soft = SHADOW.with_alpha(SHADOW.a * 0.5);
    strip(m, rings, shadow, |r| (-r.r - line * 2.5, r.r + line * 2.5), |_| (soft, soft));
    caps(m, rings, shadow, line * 2.5, soft, head.r);
    strip(m, rings, shadow, |r| (-r.r, r.r), |_| (SHADOW, SHADOW));
    caps(m, rings, shadow, 0.0, SHADOW, head.r);
    m.ellipse(head.c + shadow, head.radii, head.angle, SHADOW);
    strip(m, rings, Vec2::ZERO, |r| (-r.r - line, r.r + line), |_| (OUTLINE, OUTLINE));
    caps(m, rings, Vec2::ZERO, line, OUTLINE, 0.0);
    m.ellipse(head.c, head.radii + Vec2::splat(line), head.angle, OUTLINE);
    strip(m, rings, Vec2::ZERO, |r| (-r.r, r.r), |_| (pal.deep, pal.deep));
    if let Some(t) = rings.last() {
        m.ellipse(t.c, Vec2::splat(t.r), 0.0, pal.deep);
    }
    strip(
        m,
        rings,
        Vec2::ZERO,
        |r| (r.r * 0.45, r.r * 0.92),
        |_| (BELLY.with_alpha(0.0), BELLY),
    );
    strip(m, rings, Vec2::ZERO, |r| (-r.r * 0.86, r.r * 0.55), |_| (pal.body, pal.body));
    strip(
        m,
        rings,
        Vec2::ZERO,
        |r| (-r.r * 0.70, -r.r * 0.12),
        |_| (pal.light.with_alpha(0.0), pal.light.with_alpha(0.75)),
    );
    spots(m, g, rings, cell);
    strip(
        m,
        rings,
        Vec2::ZERO,
        |r| (-r.r * 0.62, -r.r * 0.42),
        |_| (WHITE.with_alpha(0.0), WHITE.with_alpha(0.35)),
    );
}

fn spots(m: &mut Clipped, g: &Snake, rings: &[Ring], cell: f32) {
    let total = rings.last().map_or(0.0, |r| r.s);
    let dead = g.state == State::Dead;
    let spacing = 0.62;
    let first = 1.05;
    let mut s = first;
    while s < total - 0.5 {
        if let Some(r) = rings.iter().find(|r| r.s >= s) {
            let size = r.r * 0.55;
            let along = r.dir * size * 1.15;
            let across = r.n * size * 0.85;
            let c = r.c - r.n * r.r * 0.12;
            let edge = cell * 0.04;
            let (fill, rim) = if dead {
                (Color::new(0.85, 0.85, 0.8, 1.0), Color::new(0.6, 0.6, 0.6, 1.0))
            } else {
                (SPOT, SPOT_EDGE)
            };
            let diamond = |k: f32| [c + along * k, c + across * k, c - along * k, c - across * k];
            let big = diamond(1.0);
            let o = (big[0] - c).normalize_or_zero() * edge;
            let o2 = (big[1] - c).normalize_or_zero() * edge;
            m.quad([big[0] + o, big[1] + o2, big[2] - o, big[3] - o2], [rim; 4]);
            m.quad(big, [fill; 4]);
            let hi = diamond(0.45);
            let shift = -r.n * size * 0.2 + r.dir * size * 0.2;
            m.quad(hi.map(|p| p + shift), [Color::new(1.0, 0.97, 0.75, 0.8); 4]);
        }
        s += spacing;
    }
}

struct Head {
    c: Vec2,
    radii: Vec2,
    angle: f32,
    r: f32,
}

fn head_shape(g: &Snake, rings: &[Ring], cell: f32) -> Head {
    let c = rings.first().map_or(Vec2::ZERO, |r| r.c);
    let r = cell * 0.74;
    let squash = 1.0 + 0.16 * g.turn_kick - 0.10 * g.gulp;
    let mouth = g.mouth();
    let radii = vec2(r * squash * (1.0 + 0.06 * mouth), r / squash.sqrt());
    Head {
        c: c + Vec2::from_angle(g.face) * cell * 0.08,
        radii,
        angle: g.face,
        r,
    }
}

fn face(m: &mut Clipped, g: &Snake, head: &Head, cell: f32) {
    let pal = palette(g);
    let f = Vec2::from_angle(head.angle);
    let side = f.perp();
    let dead = g.state == State::Dead;
    m.ellipse_shaded(head.c, head.radii, head.angle, pal.light, pal.body);
    m.ellipse(
        head.c - side * head.r * 0.32 + f * head.r * 0.05,
        vec2(head.radii.x * 0.55, head.radii.y * 0.26),
        head.angle,
        WHITE.with_alpha(0.28),
    );
    let snout = head.c + f * head.radii.x * 0.48;
    m.ellipse(snout, vec2(head.r * 0.42, head.r * 0.62), head.angle, BELLY.lerp(pal.body, if dead { 0.6 } else { 0.0 }));
    for s in [-1.0, 1.0] {
        m.ellipse(snout + f * head.r * 0.22 + side * s * head.r * 0.2, Vec2::splat(head.r * 0.05), 0.0, OUTLINE.with_alpha(0.7));
    }
    let mouth = g.mouth();
    let mc = head.c + f * head.radii.x * 0.86;
    if !dead {
        let tongue = tongue_len(g.clock) * (1.0 - mouth);
        if tongue > 0.05 {
            let base = mc;
            let tip = base + f * cell * 0.55 * tongue;
            m.line(base, tip, cell * 0.09, TONGUE);
            for s in [-1.0, 1.0] {
                m.line(tip, tip + (f + side * s * 0.8).normalize() * cell * 0.16, cell * 0.07, TONGUE);
            }
        }
        if mouth > 0.05 {
            let w = head.r * (0.40 + 0.40 * mouth);
            let h = head.r * 0.58 * mouth;
            m.ellipse(mc - f * h * 0.3, vec2(h + head.r * 0.05, w + head.r * 0.05), head.angle, OUTLINE);
            m.ellipse(mc - f * h * 0.3, vec2(h, w), head.angle, MOUTH_IN);
            m.ellipse(mc - f * h * 0.5, vec2(h * 0.45, w * 0.55), head.angle, TONGUE);
        } else {
            let a = mc - f * head.r * 0.12;
            for s in [-1.0, 1.0] {
                let p0 = a + side * s * head.r * 0.30;
                let p1 = a + f * head.r * 0.10;
                m.line(p0, p1, cell * 0.05, OUTLINE);
            }
        }
    } else {
        let tip = mc + f * cell * 0.35 + side * cell * 0.12;
        m.line(mc, tip, cell * 0.12, TONGUE);
        m.ellipse(tip, Vec2::splat(cell * 0.06), 0.0, TONGUE);
    }
    for s in [-1.0, 1.0] {
        m.ellipse(head.c + side * s * head.r * 0.62 + f * head.r * 0.15, vec2(head.r * 0.2, head.r * 0.14), head.angle, BLUSH.with_alpha(if dead { 0.0 } else { 0.55 }));
    }
    eyes(m, g, head, cell);
}

fn eyes(m: &mut Clipped, g: &Snake, head: &Head, cell: f32) {
    let f = Vec2::from_angle(head.angle);
    let side = f.perp();
    let er = head.r * 0.40;
    let look = g.look();
    let dead = g.state == State::Dead;
    let lid = if dead { 0.0 } else { blink(g.clock + 0.9) };
    let wide = 1.0 + 0.12 * g.mouth();
    for s in [-1.0, 1.0] {
        let c = head.c + side * s * head.r * 0.44 - f * head.r * 0.10;
        m.ellipse(c + vec2(0.0, er * 0.12), Vec2::splat(er * wide + cell * 0.05), 0.0, OUTLINE);
        if dead {
            m.ellipse(c, Vec2::splat(er * wide), 0.0, WHITE);
            let k = er * 0.55;
            let spin = g.dead_age * 4.0;
            let a = Vec2::from_angle(spin) * k;
            m.line(c - a, c + a, cell * 0.07, PUPIL);
            m.line(c - a.perp(), c + a.perp(), cell * 0.07, PUPIL);
            continue;
        }
        let open = 1.0 - lid;
        if open < 0.15 {
            m.line(c - side * er * 0.9, c + side * er * 0.9, cell * 0.06, OUTLINE);
            continue;
        }
        m.ellipse(c, vec2(er * wide, er * wide * open), 0.0, WHITE);
        let p = c + look * er * 0.38;
        let pr = er * 0.55;
        m.ellipse(p, vec2(pr, pr * open), 0.0, PUPIL);
        m.ellipse(p + vec2(-pr * 0.35, -pr * 0.4 * open), Vec2::splat(pr * 0.32), 0.0, WHITE);
    }
}

fn dizzy(m: &mut Clipped, g: &Snake, head: &Head, cell: f32) {
    if g.state != State::Dead {
        return;
    }
    let grow = (g.dead_age / 0.3).clamp(0.0, 1.0);
    for k in 0..3 {
        let a = g.dead_age * 3.0 + k as f32 * TAU / 3.0;
        let p = head.c + vec2(a.cos() * head.r * 1.15, a.sin() * head.r * 0.5 - head.r * 0.9);
        m.star(p, cell * 0.2 * grow, a, Color::new(1.0, 0.85, 0.2, 1.0));
    }
}

pub fn draw_snake(pen: &mut Pen, g: &Snake, o: Vec2, cell: f32, board: Rect) {
    let size = g.size.as_vec2() * cell;
    let mut m = Clipped::new(board);
    let base = rings(g, cell, o);
    if base.is_empty() {
        return;
    }
    let shifts = [-1.0f32, 0.0, 1.0];
    for sx in shifts {
        for sy in shifts {
            let off = vec2(sx * size.x, sy * size.y);
            let rs: Vec<Ring> = base
                .iter()
                .map(|r| Ring {
                    c: r.c + off,
                    n: r.n,
                    r: r.r,
                    s: r.s,
                    dir: r.dir,
                })
                .collect();
            let lo = rs.iter().fold(Vec2::splat(f32::MAX), |a, r| a.min(r.c)) - Vec2::splat(cell * 2.0);
            let hi = rs.iter().fold(Vec2::splat(f32::MIN), |a, r| a.max(r.c)) + Vec2::splat(cell * 2.0);
            if hi.x < board.x || hi.y < board.y || lo.x > board.x + board.w || lo.y > board.y + board.h {
                continue;
            }
            let head = head_shape(g, &rs, cell);
            tube(&mut m, g, &rs, cell, &head);
            face(&mut m, g, &head, cell);
            m.flush(pen);
        }
    }
    let head = head_shape(g, &base, cell);
    m.set_clip(Rect::new(-1e5, -1e5, 2e5, 2e5));
    dizzy(&mut m, g, &head, cell);
    m.flush(pen);
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::ivec2;

    fn snake() -> Snake {
        kg_core::save::reset_for_test();
        kg_core::profile::create("Mia", 0);
        Snake::new()
    }

    #[test]
    fn the_body_tapers_to_a_tail_and_swells_at_a_swallow() {
        let mut g = snake();
        let total = 6.0;
        let mid = radius_at(&g, 2.0, total, 100.0);
        assert!(radius_at(&g, total, total, 100.0) < mid * 0.4);
        g.bulges = vec![2];
        assert!(radius_at(&g, 2.0 + g.frac(), total, 100.0) > mid * 1.3);
    }

    #[test]
    fn the_tube_follows_the_body_without_gaps() {
        let mut g = snake();
        g.body = [ivec2(5, 5), ivec2(5, 6), ivec2(6, 6), ivec2(7, 6)].into_iter().collect();
        let rs = rings(&g, 40.0, Vec2::ZERO);
        for w in rs.windows(2) {
            assert!((w[1].c - w[0].c).length() < 40.0 * 0.2);
        }
        let first = rs.first().unwrap().c;
        assert!((first - vec2(5.5, 5.5) * 40.0).length() < 1.0);
        let last = rs.last().unwrap().c;
        assert!((last - vec2(7.5, 6.5) * 40.0).length() < 1.0);
    }
}
