use super::SLAB;
use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, Mesh, PrimitiveTopology};
use glam::{vec2, vec3, IVec2, Vec2, Vec3};
use kg_core::games::paper::world::{BLOCK, BLOCKS, FRESH_FLASH};
use kg_core::games::paper::*;
use kg_core::geom::{Color, WHITE};

const WALL_SHADE: f32 = 0.66;

pub fn block_cells(b: usize) -> (i32, i32, i32, i32) {
    let (bx, by) = ((b % BLOCKS) as i32, (b / BLOCKS) as i32);
    let span = BLOCK as i32;
    let lo = |k: i32| if k == 0 { -1 } else { k * span };
    let hi = |k: i32| {
        if k == BLOCKS as i32 - 1 {
            N
        } else {
            (k + 1) * span
        }
    };
    (lo(bx), lo(by), hi(bx), hi(by))
}

pub fn near_blocks(b: usize) -> impl Iterator<Item = usize> {
    let (bx, by) = ((b % BLOCKS) as i32, (b / BLOCKS) as i32);
    (-1..=1).flat_map(move |dy| {
        (-1..=1).filter_map(move |dx| {
            let (x, y) = (bx + dx, by + dy);
            let ok = (0..BLOCKS as i32).contains(&x) && (0..BLOCKS as i32).contains(&y);
            ok.then(|| y as usize * BLOCKS + x as usize)
        })
    })
}

pub fn flashing(g: &Paper, b: usize) -> bool {
    near_blocks(b).any(|n| g.w.clock < g.w.block_until[n])
}

pub fn stamp(g: &Paper, b: usize) -> u64 {
    let mut h = g.w.uid.wrapping_mul(0x9e37_79b9_7f4a_7c15);
    for n in near_blocks(b) {
        h = (h ^ g.w.block_serial[n] as u64).wrapping_mul(0x0100_0000_01b3);
    }
    h
}

pub fn has_land(g: &Paper, b: usize) -> bool {
    near_blocks(b).any(|n| g.w.block_owner[n].iter().any(|c| *c > 0))
}

#[derive(Default)]
pub struct Builder {
    pos: Vec<[f32; 3]>,
    nor: Vec<[f32; 3]>,
    col: Vec<[f32; 4]>,
    idx: Vec<u32>,
}

fn paint(c: Color) -> [f32; 4] {
    let l = crate::paint::linear(c);
    [l[0], l[1], l[2], 1.0]
}

impl Builder {
    pub fn vertices(&self) -> usize {
        self.pos.len()
    }

    fn vert(&mut self, p: Vec3, n: Vec3, c: [f32; 4]) {
        self.pos.push(p.to_array());
        self.nor.push(n.to_array());
        self.col.push(c);
    }

    fn top(&mut self, pts: &[Vec2], col: Color) {
        let base = self.pos.len() as u32;
        let c = paint(col);
        for p in pts {
            self.vert(vec3(p.x, SLAB, p.y), Vec3::Y, c);
        }
        let area: f32 = (0..pts.len())
            .map(|i| {
                let (a, b) = (pts[i], pts[(i + 1) % pts.len()]);
                a.x * b.y - b.x * a.y
            })
            .sum();
        for k in 1..pts.len() as u32 - 1 {
            if area > 0.0 {
                self.idx.extend([base, base + k + 1, base + k]);
            } else {
                self.idx.extend([base, base + k, base + k + 1]);
            }
        }
    }

    fn rect(&mut self, a: Vec2, b: Vec2, col: Color) {
        self.top(&[a, vec2(b.x, a.y), b, vec2(a.x, b.y)], col);
    }

    fn wall(&mut self, a: Vec2, b: Vec2, col: Color) {
        let d = (b - a).normalize_or_zero();
        let n = vec3(d.y, 0.0, -d.x);
        let base = self.pos.len() as u32;
        let c = paint(col.darker(WALL_SHADE));
        for (p, y) in [(a, SLAB), (b, SLAB), (b, 0.0), (a, 0.0)] {
            self.vert(vec3(p.x, y, p.y), n, c);
        }
        let (pa, pb, pc) = (
            vec3(a.x, SLAB, a.y),
            vec3(b.x, SLAB, b.y),
            vec3(b.x, 0.0, b.y),
        );
        if (pb - pa).cross(pc - pa).dot(n) >= 0.0 {
            self.idx
                .extend([base, base + 1, base + 2, base, base + 2, base + 3]);
        } else {
            self.idx
                .extend([base, base + 2, base + 1, base, base + 3, base + 2]);
        }
    }

    pub fn mesh(self) -> Mesh {
        let mut m = Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::RENDER_WORLD,
        );
        m.insert_attribute(Mesh::ATTRIBUTE_POSITION, self.pos);
        m.insert_attribute(Mesh::ATTRIBUTE_NORMAL, self.nor);
        m.insert_attribute(Mesh::ATTRIBUTE_COLOR, self.col);
        m.insert_indices(Indices::U32(self.idx));
        m
    }
}

struct Owners {
    x0: i32,
    y0: i32,
    w: i32,
    h: i32,
    cells: Vec<Option<u8>>,
}

impl Owners {
    fn new(g: &Paper, x0: i32, y0: i32, x1: i32, y1: i32) -> Self {
        let (x0, y0) = (x0 - 2, y0 - 2);
        let (w, h) = (x1 - x0 + 3, y1 - y0 + 3);
        let mut cells = Vec::with_capacity((w * h) as usize);
        for y in 0..h {
            for x in 0..w {
                let c = IVec2::new(x0 + x, y0 + y);
                cells.push(if in_grid(c) { g.w.shown(idx(c)) } else { None });
            }
        }
        Self {
            x0,
            y0,
            w,
            h,
            cells,
        }
    }

    fn get(&self, x: i32, y: i32) -> Option<u8> {
        let (lx, ly) = (x - self.x0, y - self.y0);
        if lx < 0 || ly < 0 || lx >= self.w || ly >= self.h {
            return None;
        }
        self.cells[(ly * self.w + lx) as usize]
    }

    fn density(&self, o: u8, x: i32, y: i32) -> f32 {
        const W: [f32; 3] = [1.0, 2.0, 1.0];
        let mut sum = 0.0;
        for (j, wy) in W.iter().enumerate() {
            for (i, wx) in W.iter().enumerate() {
                if self.get(x + i as i32 - 1, y + j as i32 - 1) == Some(o) {
                    sum += wx * wy;
                }
            }
        }
        sum / 16.0
    }
}

fn contour(corner: [Vec2; 4], f: [f32; 4]) -> Vec<(Vec2, bool)> {
    let mut pts = Vec::with_capacity(8);
    for k in 0..4 {
        let n = (k + 1) % 4;
        let (a, b) = (f[k] >= 0.5, f[n] >= 0.5);
        if a {
            pts.push((corner[k], false));
        }
        if a != b {
            let t = ((0.5 - f[k]) / (f[n] - f[k])).clamp(0.0, 1.0);
            pts.push((corner[k].lerp(corner[n], t), true));
        }
    }
    pts
}

fn fresh_at(g: &Paper, x: i32, y: i32) -> f32 {
    let c = IVec2::new(x, y);
    if !in_grid(c) {
        return 0.0;
    }
    let age = g.w.age(idx(c));
    if (0.0..FRESH_FLASH).contains(&age) {
        1.0 - age / FRESH_FLASH
    } else {
        0.0
    }
}

pub fn build(g: &Paper, b: usize) -> Builder {
    let (x0, y0, x1, y1) = block_cells(b);
    let own = Owners::new(g, x0, y0, x1, y1);
    let mut out = Builder::default();
    let corner = |x: i32, y: i32| cell_center(IVec2::new(x, y));
    for y in y0..y1 {
        let mut x = x0;
        while x < x1 {
            let k = [
                own.get(x, y),
                own.get(x + 1, y),
                own.get(x + 1, y + 1),
                own.get(x, y + 1),
            ];
            let tint = |o: u8, x: i32| {
                let base = color(o as usize);
                let f = [(x, y), (x + 1, y), (x + 1, y + 1), (x, y + 1)]
                    .iter()
                    .zip(k)
                    .filter(|(_, c)| *c == Some(o))
                    .map(|((cx, cy), _)| fresh_at(g, *cx, *cy))
                    .fold(0.0, f32::max);
                base.lerp(WHITE, 0.45 * f)
            };
            let solid = |o: u8, x: i32| {
                (-1..=2).all(|dy| (-1..=2).all(|dx| own.get(x + dx, y + dy) == Some(o)))
            };
            if let Some(o) = k[0].filter(|o| solid(*o, x)) {
                let col = tint(o, x);
                let start = x;
                x += 1;
                while x < x1 && solid(o, x) && tint(o, x) == col {
                    x += 1;
                }
                out.rect(corner(start, y), corner(x, y + 1), col);
                continue;
            }
            let cs = [
                corner(x, y),
                corner(x + 1, y),
                corner(x + 1, y + 1),
                corner(x, y + 1),
            ];
            let mut near: Vec<u8> = vec![];
            for dy in -1..=2 {
                for dx in -1..=2 {
                    if let Some(o) = own.get(x + dx, y + dy) {
                        if !near.contains(&o) {
                            near.push(o);
                        }
                    }
                }
            }
            for o in near {
                let f = [
                    own.density(o, x, y),
                    own.density(o, x + 1, y),
                    own.density(o, x + 1, y + 1),
                    own.density(o, x, y + 1),
                ];
                if f.iter().all(|v| *v < 0.5) {
                    continue;
                }
                let pts = contour(cs, f);
                if pts.len() < 3 {
                    continue;
                }
                let col = tint(o, x);
                let flat: Vec<Vec2> = pts.iter().map(|p| p.0).collect();
                out.top(&flat, col);
                for i in 0..pts.len() {
                    let (a, b) = (pts[i], pts[(i + 1) % pts.len()]);
                    if a.1 && b.1 {
                        out.wall(a.0, b.0, col);
                    }
                }
            }
            x += 1;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blocks_tile_the_whole_dual_grid_without_gaps() {
        let mut covered = vec![0u8; ((N + 1) * (N + 1)) as usize];
        for b in 0..BLOCKS * BLOCKS {
            let (x0, y0, x1, y1) = block_cells(b);
            for y in y0..y1 {
                for x in x0..x1 {
                    covered[((y + 1) * (N + 1) + x + 1) as usize] += 1;
                }
            }
        }
        assert!(covered.iter().all(|c| *c == 1));
    }

    #[test]
    fn a_start_disc_builds_a_raised_slab_with_a_wall_all_round() {
        kg_core::save::reset_for_test();
        let mut g = Paper::with_best(0);
        g.w.clock = 5.0;
        let at = cell_at(g.me().pos);
        let b = kg_core::games::paper::world::block_of(idx(at));
        let m = build(&g, b);
        assert!(m.vertices() > 100);
        let walls = m.nor.iter().filter(|n| n[1].abs() < 0.01).count();
        assert!(walls > 60, "{walls}");
    }

    #[test]
    fn empty_ground_builds_nothing() {
        kg_core::save::reset_for_test();
        let g = Paper::with_best(0);
        let empty = (0..BLOCKS * BLOCKS)
            .find(|b| !has_land(&g, *b))
            .expect("some block is empty");
        assert_eq!(build(&g, empty).vertices(), 0);
    }
}
