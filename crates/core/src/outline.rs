use crate::geom::Rect;
use std::collections::HashMap;

pub const OUTLINE_FRAC: f32 = 0.11;
const ATLAS: usize = 2048;

#[derive(Clone, Copy)]
pub struct Glyph {
    pub fill: Rect,
    pub edge: Rect,
    pub left: f32,
    pub top: f32,
    pub w: f32,
    pub h: f32,
    pub advance: f32,
}

fn edt_1d(f: &[f32], out: &mut [f32], v: &mut [usize], z: &mut [f32]) {
    let n = f.len();
    if n == 0 {
        return;
    }
    let mut k = 0usize;
    v[0] = 0;
    z[0] = f32::NEG_INFINITY;
    z[1] = f32::INFINITY;
    for q in 1..n {
        let mut s;
        loop {
            let p = v[k];
            s = ((f[q] + (q * q) as f32) - (f[p] + (p * p) as f32)) / (2.0 * (q as f32 - p as f32));
            if s <= z[k] && k > 0 {
                k -= 1;
            } else {
                break;
            }
        }
        if s <= z[k] {
            v[0] = q;
            z[0] = f32::NEG_INFINITY;
            z[1] = f32::INFINITY;
            k = 0;
        } else {
            k += 1;
            v[k] = q;
            z[k] = s;
            z[k + 1] = f32::INFINITY;
        }
    }
    k = 0;
    for (q, o) in out.iter_mut().enumerate().take(n) {
        while z[k + 1] < q as f32 {
            k += 1;
        }
        let p = v[k];
        *o = (q as f32 - p as f32).powi(2) + f[p];
    }
}

pub fn distance_to_ink(cov: &[u8], w: usize, h: usize) -> Vec<f32> {
    let inf = 1e12f32;
    let mut d: Vec<f32> = cov
        .iter()
        .map(|c| if *c >= 128 { 0.0 } else { inf })
        .collect();
    let n = w.max(h);
    let mut f = vec![0.0f32; n];
    let mut o = vec![0.0f32; n];
    let mut v = vec![0usize; n];
    let mut z = vec![0.0f32; n + 1];
    for x in 0..w {
        for y in 0..h {
            f[y] = d[y * w + x];
        }
        edt_1d(&f[..h], &mut o[..h], &mut v, &mut z);
        for y in 0..h {
            d[y * w + x] = o[y];
        }
    }
    for y in 0..h {
        f[..w].copy_from_slice(&d[y * w..y * w + w]);
        edt_1d(&f[..w], &mut o[..w], &mut v, &mut z);
        d[y * w..y * w + w].copy_from_slice(&o[..w]);
    }
    d.iter().map(|x| x.sqrt()).collect()
}

pub fn dilate(cov: &[u8], w: usize, h: usize, r: f32, pad: usize) -> (Vec<u8>, usize, usize) {
    let ow = w + pad * 2;
    let oh = h + pad * 2;
    let mut padded = vec![0u8; ow * oh];
    for y in 0..h {
        padded[(y + pad) * ow + pad..(y + pad) * ow + pad + w]
            .copy_from_slice(&cov[y * w..y * w + w]);
    }
    let dist = distance_to_ink(&padded, ow, oh);
    let out = dist
        .iter()
        .zip(&padded)
        .map(|(d, c)| {
            let ring = (r + 0.5 - d).clamp(0.0, 1.0);
            let own = *c as f32 / 255.0;
            (ring.max(own) * 255.0) as u8
        })
        .collect();
    (out, ow, oh)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Region {
    pub x: usize,
    pub y: usize,
    pub w: usize,
    pub h: usize,
}

impl Region {
    fn union(self, o: Region) -> Region {
        let x = self.x.min(o.x);
        let y = self.y.min(o.y);
        Region {
            x,
            y,
            w: (self.x + self.w).max(o.x + o.w) - x,
            h: (self.y + self.h).max(o.y + o.h) - y,
        }
    }
}

pub struct Atlas {
    pub size: usize,
    pub alpha: Vec<u8>,
    pub generation: u64,
    dirty: Option<Region>,
    x: usize,
    y: usize,
    row: usize,
}

impl Atlas {
    fn new() -> Self {
        Self {
            size: ATLAS,
            alpha: vec![0; ATLAS * ATLAS],
            generation: 1,
            dirty: Some(Region {
                x: 0,
                y: 0,
                w: ATLAS,
                h: ATLAS,
            }),
            x: 1,
            y: 1,
            row: 0,
        }
    }

    fn put(&mut self, alpha: &[u8], w: usize, h: usize) -> Option<Rect> {
        if w + 2 > ATLAS {
            return None;
        }
        if self.x + w + 1 > ATLAS {
            self.x = 1;
            self.y += self.row + 2;
            self.row = 0;
        }
        if self.y + h + 1 > ATLAS {
            return None;
        }
        let (x0, y0) = (self.x, self.y);
        for row in 0..h {
            let at = (y0 + row) * ATLAS + x0;
            self.alpha[at..at + w].copy_from_slice(&alpha[row * w..row * w + w]);
        }
        self.generation += 1;
        let r = Region { x: x0, y: y0, w, h };
        self.dirty = Some(self.dirty.map_or(r, |d| d.union(r)));
        self.x += w + 2;
        self.row = self.row.max(h);
        Some(Rect::new(x0 as f32, y0 as f32, w as f32, h as f32))
    }

    pub fn take_dirty(&mut self) -> Option<Region> {
        self.dirty.take()
    }
}

pub struct Cache {
    font: fontdue::Font,
    glyphs: HashMap<(char, u16), Glyph>,
    pub atlas: Atlas,
}

impl Cache {
    pub fn new(bytes: &[u8]) -> Option<Self> {
        let font = fontdue::Font::from_bytes(bytes, fontdue::FontSettings::default()).ok()?;
        Some(Self {
            font,
            glyphs: HashMap::new(),
            atlas: Atlas::new(),
        })
    }

    fn make(&mut self, ch: char, px: u16) -> Glyph {
        let (m, cov) = self.font.rasterize(ch, px as f32);
        let r = px as f32 * OUTLINE_FRAC;
        let pad = r.ceil() as usize + 1;
        let (edge, ow, oh) = dilate(&cov, m.width, m.height, r, pad);
        let mut fill = vec![0u8; ow * oh];
        for y in 0..m.height {
            fill[(y + pad) * ow + pad..(y + pad) * ow + pad + m.width]
                .copy_from_slice(&cov[y * m.width..(y + 1) * m.width]);
        }
        let placed = match (self.atlas.put(&fill, ow, oh), self.atlas.put(&edge, ow, oh)) {
            (Some(f), Some(e)) => (f, e),
            _ => {
                self.glyphs.clear();
                let generation = self.atlas.generation;
                self.atlas = Atlas::new();
                self.atlas.generation = generation + 1;
                (
                    self.atlas.put(&fill, ow, oh).unwrap_or_default(),
                    self.atlas.put(&edge, ow, oh).unwrap_or_default(),
                )
            }
        };
        Glyph {
            fill: placed.0,
            edge: placed.1,
            left: m.xmin as f32 - pad as f32,
            top: -(m.ymin as f32 + m.height as f32) - pad as f32,
            w: ow as f32,
            h: oh as f32,
            advance: m.advance_width,
        }
    }

    pub fn glyph(&mut self, ch: char, px: u16) -> Glyph {
        if let Some(g) = self.glyphs.get(&(ch, px)) {
            return *g;
        }
        let g = self.make(ch, px);
        self.glyphs.insert((ch, px), g);
        g
    }

    pub fn kern(&self, a: char, b: char, px: u16) -> f32 {
        self.font.horizontal_kern(a, b, px as f32).unwrap_or(0.0)
    }

    pub fn layout(&mut self, s: &str, px: u16) -> (Vec<(Glyph, f32)>, f32) {
        let mut placed = vec![];
        let mut x = 0.0;
        let mut prev = None;
        for ch in s.chars() {
            if let Some(p) = prev {
                x += self.kern(p, ch, px);
            }
            let g = self.glyph(ch, px);
            placed.push((g, x));
            x += g.advance;
            prev = Some(ch);
        }
        (placed, x)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dilation_grows_a_dot_into_a_round_disc() {
        let mut cov = vec![0u8; 9];
        cov[4] = 255;
        let (out, w, h) = dilate(&cov, 3, 3, 3.0, 3);
        assert_eq!((w, h), (9, 9));
        let at = |x: usize, y: usize| out[y * w + x];
        assert_eq!(at(4, 4), 255);
        assert_eq!(at(6, 4), 255);
        assert_eq!(at(4, 2), 255);
        assert!(at(7, 4) > 100 && at(7, 4) < 200, "soft rim {}", at(7, 4));
        assert_eq!(at(8, 4), 0);
        assert!(
            at(6, 6) > 120 && at(6, 6) < 230,
            "diagonal is round, not square: {}",
            at(6, 6)
        );
        assert_eq!(at(0, 0), 0);
    }

    #[test]
    fn distance_transform_matches_brute_force() {
        let (w, h) = (23, 17);
        let ink = [(3usize, 4usize), (15, 9), (20, 2), (8, 14)];
        let mut cov = vec![0u8; w * h];
        for (x, y) in ink {
            cov[y * w + x] = 255;
        }
        let d = distance_to_ink(&cov, w, h);
        for y in 0..h {
            for x in 0..w {
                let brute = ink
                    .iter()
                    .map(|(a, b)| {
                        ((x as f32 - *a as f32).powi(2) + (y as f32 - *b as f32).powi(2)).sqrt()
                    })
                    .fold(f32::MAX, f32::min);
                assert!(
                    (d[y * w + x] - brute).abs() < 1e-3,
                    "({x},{y}) {} vs {brute}",
                    d[y * w + x]
                );
            }
        }
    }

    #[test]
    fn big_glyph_outline_is_fast() {
        let (w, h) = (160, 200);
        let cov: Vec<u8> = (0..w * h)
            .map(|i| {
                if (i % w) > 40 && (i % w) < 120 && i / w > 30 && i / w < 170 {
                    255
                } else {
                    0
                }
            })
            .collect();
        let r = 200.0 * OUTLINE_FRAC;
        let t = std::time::Instant::now();
        dilate(&cov, w, h, r, r.ceil() as usize + 1);
        assert!(t.elapsed().as_millis() < 60, "took {:?}", t.elapsed());
    }

    #[test]
    fn outlining_all_ascii_at_every_size_is_quick() {
        let font =
            fontdue::Font::from_bytes(crate::font::FONT_BYTES, fontdue::FontSettings::default())
                .unwrap();
        let t = std::time::Instant::now();
        let mut n = 0;
        for px in [24.0f32, 36.0, 52.0, 72.0, 100.0, 140.0, 200.0] {
            for ch in (32u8..127).map(char::from) {
                let (m, cov) = font.rasterize(ch, px);
                let r = px * OUTLINE_FRAC;
                dilate(&cov, m.width, m.height, r, r.ceil() as usize + 1);
                n += 1;
            }
        }
        let ms = t.elapsed().as_millis();
        eprintln!("{n} glyphs outlined in {ms}ms");
        assert!(ms < 3000);
    }

    #[test]
    fn a_new_glyph_uploads_only_its_own_patch_of_the_atlas() {
        let mut c = Cache::new(crate::font::FONT_BYTES).unwrap();
        let chars: String = (32u8..127).map(char::from).collect();
        c.layout(&chars, 52);
        assert_eq!(c.atlas.take_dirty().map(|r| r.w), Some(c.atlas.size));
        assert_eq!(c.atlas.take_dirty(), None, "nothing new, nothing to upload");
        c.layout("K", 140);
        let r = c.atlas.take_dirty().expect("the new glyph is uploaded");
        let bytes = r.w * r.h * 4;
        assert!(
            bytes < 512 * 1024,
            "one 140px glyph re-sends {bytes} bytes; the whole atlas is {}",
            c.atlas.size * c.atlas.size * 4
        );
        let ink = (r.y..r.y + r.h)
            .flat_map(|y| (r.x..r.x + r.w).map(move |x| (x, y)))
            .filter(|(x, y)| c.atlas.alpha[y * c.atlas.size + x] > 0)
            .count();
        assert!(ink > 500);
    }

    #[test]
    fn glyphs_land_in_the_atlas_with_ink_and_an_edge() {
        let mut c = Cache::new(crate::font::FONT_BYTES).unwrap();
        let g0 = c.atlas.generation;
        let (placed, width) = c.layout("Hi", 52);
        assert_eq!(placed.len(), 2);
        assert!(width > 30.0);
        assert!(c.atlas.generation > g0);
        let g = placed[0].0;
        let ink = |r: Rect| {
            let mut n = 0;
            for y in r.y as usize..(r.y + r.h) as usize {
                for x in r.x as usize..(r.x + r.w) as usize {
                    n += (c.atlas.alpha[y * c.atlas.size + x] > 128) as usize;
                }
            }
            n
        };
        assert!(ink(g.edge) > ink(g.fill), "the edge is wider than the fill");
        assert!(ink(g.fill) > 50);
        let again = c.atlas.generation;
        c.layout("Hi", 52);
        assert_eq!(c.atlas.generation, again, "cached glyphs are not redrawn");
    }

    #[test]
    fn dilated_edge_is_smooth_not_stepped() {
        let w = 40;
        let mut cov = vec![0u8; w * w];
        for y in 0..w {
            for x in 0..w {
                let d = (((x as f32 - 20.0).powi(2) + (y as f32 - 20.0).powi(2)).sqrt() - 10.0)
                    .clamp(-0.5, 0.5);
                cov[y * w + x] = ((0.5 - d) * 255.0) as u8;
            }
        }
        let (out, ow, _) = dilate(&cov, w, w, 4.0, 5);
        let row: Vec<u8> = (0..ow).map(|x| out[(20 + 5) * ow + x]).collect();
        let partial = row.iter().filter(|v| **v > 10 && **v < 245).count();
        assert!(partial >= 1, "edge should be anti-aliased: {row:?}");
        assert!(
            row[..ow / 2].windows(2).all(|p| p[1] >= p[0]),
            "row through a dilated disc must not dip: {row:?}"
        );
    }
}
