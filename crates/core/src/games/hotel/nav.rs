use crate::geom::*;
use glam::{vec2, Vec2};
use std::cmp::Reverse;
use std::collections::BinaryHeap;

pub const CELL: f32 = 0.25;

pub struct Nav {
    origin: Vec2,
    nx: usize,
    nz: usize,
    blocked: Vec<bool>,
    walls: Vec<Rect>,
    r: f32,
}

fn hits(p: Vec2, walls: &[Rect], r: f32) -> bool {
    walls.iter().any(|w| {
        let c = vec2(p.x.clamp(w.x, w.x + w.w), p.y.clamp(w.y, w.y + w.h));
        (c - p).length_squared() < r * r
    })
}

impl Nav {
    pub fn new(area: Rect, walls: &[Rect], r: f32) -> Self {
        let nx = (area.w / CELL).ceil() as usize;
        let nz = (area.h / CELL).ceil() as usize;
        let origin = vec2(area.x, area.y);
        let mut blocked = vec![false; nx * nz];
        for j in 0..nz {
            for i in 0..nx {
                let p = origin + vec2((i as f32 + 0.5) * CELL, (j as f32 + 0.5) * CELL);
                blocked[j * nx + i] = hits(p, walls, r);
            }
        }
        Self {
            origin,
            nx,
            nz,
            blocked,
            walls: walls.to_vec(),
            r,
        }
    }

    fn cell_of(&self, p: Vec2) -> (i32, i32) {
        let q = (p - self.origin) / CELL;
        (q.x.floor() as i32, q.y.floor() as i32)
    }

    fn center(&self, c: (i32, i32)) -> Vec2 {
        self.origin + vec2((c.0 as f32 + 0.5) * CELL, (c.1 as f32 + 0.5) * CELL)
    }

    pub fn free(&self, c: (i32, i32)) -> bool {
        c.0 >= 0
            && c.1 >= 0
            && (c.0 as usize) < self.nx
            && (c.1 as usize) < self.nz
            && !self.blocked[c.1 as usize * self.nx + c.0 as usize]
    }

    pub fn free_at(&self, p: Vec2) -> bool {
        self.free(self.cell_of(p))
    }

    fn nearest_free(&self, p: Vec2) -> Option<(i32, i32)> {
        let c = self.cell_of(p);
        if self.free(c) {
            return Some(c);
        }
        let mut best: Option<((i32, i32), f32)> = None;
        for r in 1i32..24 {
            for dx in -r..=r {
                for dz in -r..=r {
                    if dx.abs() != r && dz.abs() != r {
                        continue;
                    }
                    let q = (c.0 + dx, c.1 + dz);
                    if self.free(q) {
                        let d = (self.center(q) - p).length();
                        if best.is_none_or(|(_, b)| d < b) {
                            best = Some((q, d));
                        }
                    }
                }
            }
            if best.is_some() {
                return best.map(|b| b.0);
            }
        }
        None
    }

    pub fn line_free(&self, a: Vec2, b: Vec2) -> bool {
        let n = ((b - a).length() / (CELL * 0.4)).ceil().max(1.0) as usize;
        (0..=n).all(|i| !hits(a.lerp(b, i as f32 / n as f32), &self.walls, self.r))
    }

    pub fn path(&self, from: Vec2, to: Vec2) -> Vec<Vec2> {
        let (Some(s), Some(g)) = (self.nearest_free(from), self.nearest_free(to)) else {
            return vec![to];
        };
        let cells = self.astar(s, g);
        let Some(cells) = cells else {
            return vec![to];
        };
        let mut pts: Vec<Vec2> = cells.iter().map(|c| self.center(*c)).collect();
        if self.free_at(from) {
            pts[0] = from;
        } else {
            pts.insert(0, from);
        }
        let goal_free = self.free_at(to);
        if goal_free {
            *pts.last_mut().unwrap() = to;
        }
        let mut out = vec![];
        let mut i = 0;
        while i < pts.len() - 1 {
            let mut j = pts.len() - 1;
            while j > i + 1 && !self.line_free(pts[i], pts[j]) {
                j -= 1;
            }
            out.push(pts[j]);
            i = j;
        }
        if !goal_free {
            out.push(to);
        }
        out
    }

    fn astar(&self, s: (i32, i32), g: (i32, i32)) -> Option<Vec<(i32, i32)>> {
        let n = self.nx * self.nz;
        let idx = |c: (i32, i32)| c.1 as usize * self.nx + c.0 as usize;
        let mut cost = vec![u32::MAX; n];
        let mut prev = vec![u32::MAX; n];
        let h = |c: (i32, i32)| {
            let dx = (c.0 - g.0).unsigned_abs();
            let dz = (c.1 - g.1).unsigned_abs();
            10 * dx.max(dz) + 4 * dx.min(dz)
        };
        let mut open = BinaryHeap::new();
        cost[idx(s)] = 0;
        open.push(Reverse((h(s), 0u32, s.0, s.1)));
        while let Some(Reverse((_, c, x, z))) = open.pop() {
            let cur = (x, z);
            if cur == g {
                let mut out = vec![g];
                let mut k = idx(g);
                while prev[k] != u32::MAX {
                    k = prev[k] as usize;
                    out.push(((k % self.nx) as i32, (k / self.nx) as i32));
                }
                out.reverse();
                return Some(out);
            }
            if c > cost[idx(cur)] {
                continue;
            }
            for (dx, dz) in [
                (1, 0),
                (-1, 0),
                (0, 1),
                (0, -1),
                (1, 1),
                (1, -1),
                (-1, 1),
                (-1, -1),
            ] {
                let nb = (x + dx, z + dz);
                if !self.free(nb) {
                    continue;
                }
                if dx != 0 && dz != 0 && (!self.free((x + dx, z)) || !self.free((x, z + dz))) {
                    continue;
                }
                let step = if dx != 0 && dz != 0 { 14 } else { 10 };
                let nc = c + step;
                let k = idx(nb);
                if nc < cost[k] {
                    cost[k] = nc;
                    prev[k] = idx(cur) as u32;
                    open.push(Reverse((nc + h(nb), nc, nb.0, nb.1)));
                }
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_path_goes_round_a_wall_not_through_it() {
        let wall = Rect::new(-0.2, -3.0, 0.4, 6.0);
        let nav = Nav::new(Rect::new(-5.0, -5.0, 10.0, 10.0), &[wall], 0.3);
        let p = nav.path(vec2(-2.0, 0.0), vec2(2.0, 0.0));
        let mut at = vec2(-2.0, 0.0);
        for q in &p {
            assert!(nav.line_free(at, *q), "segment {at} -> {q} cuts the wall");
            at = *q;
        }
        assert_eq!(*p.last().unwrap(), vec2(2.0, 0.0));
        assert!(p.len() >= 2);
    }

    #[test]
    fn a_blocked_goal_is_reached_from_its_nearest_free_side() {
        let bed = Rect::new(-1.0, -1.0, 2.0, 2.0);
        let nav = Nav::new(Rect::new(-5.0, -5.0, 10.0, 10.0), &[bed], 0.3);
        let p = nav.path(vec2(-4.0, -4.0), vec2(0.0, 0.0));
        assert_eq!(*p.last().unwrap(), vec2(0.0, 0.0));
        let before = p[p.len() - 2];
        assert!(nav.free_at(before));
    }
}
