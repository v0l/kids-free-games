use macroquad::prelude::*;
use std::cell::RefCell;
use std::collections::HashMap;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Frame {
    pub page: usize,
    pub src: Rect,
}

impl Frame {
    pub fn size(&self) -> Vec2 {
        vec2(self.src.w, self.src.h)
    }

    pub fn aspect(&self) -> f32 {
        self.src.w / self.src.h.max(1.0)
    }
}

pub struct Index {
    pub pages: Vec<(usize, usize)>,
    pub frames: HashMap<String, Frame>,
}

fn u32_at(b: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(b.get(at..at + 4)?.try_into().ok()?))
}

pub fn parse(bytes: &[u8]) -> Option<Index> {
    if bytes.get(0..4)? != b"KGA1" {
        return None;
    }
    let n = u32_at(bytes, 4)? as usize;
    let mut at = 8;
    let mut pages = vec![];
    for _ in 0..n {
        let len = u32_at(bytes, at)? as usize;
        pages.push((at + 4, len));
        at += 4 + len;
    }
    let len = u32_at(bytes, at)? as usize;
    let text = std::str::from_utf8(bytes.get(at + 4..at + 4 + len)?).ok()?;
    let mut frames = HashMap::new();
    for line in text.lines() {
        let f: Vec<&str> = line.split_whitespace().collect();
        if f.len() != 6 {
            return None;
        }
        let num = |i: usize| f[i].parse::<f32>().ok();
        frames.insert(
            f[0].to_string(),
            Frame {
                page: f[1].parse().ok()?,
                src: Rect::new(num(2)?, num(3)?, num(4)?, num(5)?),
            },
        );
    }
    Some(Index { pages, frames })
}

pub struct Sheet {
    pub index: Index,
    bytes: &'static [u8],
    textures: Vec<Option<Texture2D>>,
}

impl Sheet {
    pub fn new(bytes: &'static [u8]) -> Self {
        let index = parse(bytes).expect("bad sprite sheet");
        let n = index.pages.len();
        Self {
            index,
            bytes,
            textures: vec![None; n],
        }
    }

    pub fn frame(&self, name: &str) -> Option<Frame> {
        self.index.frames.get(name).copied()
    }

    fn texture(&mut self, page: usize) -> Texture2D {
        self.textures[page]
            .get_or_insert_with(|| {
                let (at, len) = self.index.pages[page];
                let t = Texture2D::from_file_with_format(
                    &self.bytes[at..at + len],
                    Some(ImageFormat::Png),
                );
                t.set_filter(FilterMode::Linear);
                t
            })
            .clone()
    }
}

thread_local! {
    static SHEETS: RefCell<HashMap<&'static str, Sheet>> = RefCell::new(HashMap::new());
}

pub fn with_sheet<R>(
    name: &'static str,
    bytes: &'static [u8],
    f: impl FnOnce(&mut Sheet) -> R,
) -> R {
    SHEETS.with(|s| {
        let mut s = s.borrow_mut();
        let sheet = s.entry(name).or_insert_with(|| Sheet::new(bytes));
        f(sheet)
    })
}

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

pub fn draw_in(sheet: &mut Sheet, name: &str, dest: Rect, d: Draw) {
    let Some(f) = sheet.frame(name) else {
        draw_rectangle(dest.x, dest.y, dest.w, dest.h, MAGENTA);
        return;
    };
    let tex = sheet.texture(f.page);
    draw_texture_ex(
        &tex,
        dest.x,
        dest.y,
        d.tint,
        DrawTextureParams {
            dest_size: Some(vec2(dest.w, dest.h)),
            source: Some(f.src),
            rotation: d.rotation,
            flip_x: d.flip_x,
            flip_y: d.flip_y,
            pivot: Some(vec2(
                dest.x + dest.w * d.pivot.x,
                dest.y + dest.h * d.pivot.y,
            )),
        },
    );
}

pub fn nine_in(sheet: &mut Sheet, name: &str, dest: Rect, border: f32, scale: f32, tint: Color) {
    let Some(f) = sheet.frame(name) else {
        draw_rectangle(dest.x, dest.y, dest.w, dest.h, MAGENTA);
        return;
    };
    let tex = sheet.texture(f.page);
    for (sx, sw, dx, dw) in nine_spans(f.src.x, f.src.w, dest.x, dest.w, border, scale) {
        for (sy, sh, dy, dh) in nine_spans(f.src.y, f.src.h, dest.y, dest.h, border, scale) {
            if dw <= 0.0 || dh <= 0.0 {
                continue;
            }
            draw_texture_ex(
                &tex,
                dx,
                dy,
                tint,
                DrawTextureParams {
                    dest_size: Some(vec2(dw, dh)),
                    source: Some(Rect::new(sx, sy, sw, sh)),
                    ..Default::default()
                },
            );
        }
    }
}

pub fn nine_spans(
    src: f32,
    src_len: f32,
    dst: f32,
    dst_len: f32,
    border: f32,
    scale: f32,
) -> [(f32, f32, f32, f32); 3] {
    let b = border.min(src_len / 2.0 - 1.0).max(0.0);
    let edge = (b * scale).min(dst_len / 2.0);
    [
        (src, b, dst, edge),
        (src + b, src_len - 2.0 * b, dst + edge, dst_len - 2.0 * edge),
        (src + src_len - b, b, dst + dst_len - edge, edge),
    ]
}

#[macro_export]
macro_rules! sprites {
    ($file:literal) => {
        static SHEET_BYTES: &[u8] = include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/assets/art/",
            $file,
            ".kga"
        ));

        #[allow(dead_code)]
        pub fn frame(name: &str) -> Option<$crate::art::Frame> {
            $crate::art::with_sheet($file, SHEET_BYTES, |s| s.frame(name))
        }

        #[allow(dead_code)]
        pub fn size(name: &str) -> macroquad::prelude::Vec2 {
            frame(name)
                .map(|f| f.size())
                .unwrap_or(macroquad::prelude::vec2(1.0, 1.0))
        }

        #[allow(dead_code)]
        pub fn sprite(name: &str, dest: macroquad::prelude::Rect) {
            sprite_ex(name, dest, $crate::art::Draw::default());
        }

        #[allow(dead_code)]
        pub fn sprite_ex(name: &str, dest: macroquad::prelude::Rect, d: $crate::art::Draw) {
            $crate::art::with_sheet($file, SHEET_BYTES, |s| {
                $crate::art::draw_in(s, name, dest, d)
            });
        }

        #[allow(dead_code)]
        pub fn sprite_at(
            name: &str,
            center: macroquad::prelude::Vec2,
            height: f32,
            d: $crate::art::Draw,
        ) {
            let s = size(name);
            let w = height * s.x / s.y.max(1.0);
            sprite_ex(
                name,
                macroquad::prelude::Rect::new(
                    center.x - w / 2.0,
                    center.y - height / 2.0,
                    w,
                    height,
                ),
                d,
            );
        }

        #[allow(dead_code)]
        pub fn sprite_w(
            name: &str,
            center: macroquad::prelude::Vec2,
            width: f32,
            d: $crate::art::Draw,
        ) {
            let s = size(name);
            let h = width * s.y / s.x.max(1.0);
            sprite_ex(
                name,
                macroquad::prelude::Rect::new(center.x - width / 2.0, center.y - h / 2.0, width, h),
                d,
            );
        }

        #[allow(dead_code)]
        pub fn nine(
            name: &str,
            dest: macroquad::prelude::Rect,
            border: f32,
            scale: f32,
            tint: macroquad::prelude::Color,
        ) {
            $crate::art::with_sheet($file, SHEET_BYTES, |s| {
                $crate::art::nine_in(s, name, dest, border, scale, tint)
            });
        }

        #[allow(dead_code)]
        pub fn names() -> Vec<String> {
            $crate::art::with_sheet($file, SHEET_BYTES, |s| {
                s.index.frames.keys().cloned().collect()
            })
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nine_slice_keeps_corners_and_fills_the_rect() {
        let s = nine_spans(10.0, 64.0, 100.0, 300.0, 16.0, 1.5);
        assert_eq!(s[0], (10.0, 16.0, 100.0, 24.0));
        assert_eq!(s[2].2 + s[2].3, 400.0);
        assert_eq!(s[1].2, 124.0);
        assert_eq!(s[1].3, 252.0);
        let tiny = nine_spans(0.0, 64.0, 0.0, 20.0, 16.0, 2.0);
        assert_eq!(tiny[0].3 + tiny[1].3 + tiny[2].3, 20.0);
        assert!(tiny[1].3 >= 0.0);
    }

    #[test]
    fn rejects_garbage() {
        assert!(parse(b"nope").is_none());
        assert!(parse(b"KGA1\x05\x00\x00\x00").is_none());
    }

    #[test]
    fn every_sheet_parses_and_frames_sit_inside_their_page() {
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/assets/art");
        let mut seen = 0;
        for e in std::fs::read_dir(dir).unwrap() {
            let p = e.unwrap().path();
            if p.extension().and_then(|x| x.to_str()) != Some("kga") {
                continue;
            }
            seen += 1;
            let b = std::fs::read(&p).unwrap();
            let idx = parse(&b).unwrap_or_else(|| panic!("{p:?} does not parse"));
            assert!(!idx.frames.is_empty(), "{p:?} is empty");
            for (name, f) in &idx.frames {
                let (at, len) = idx.pages[f.page];
                let png = &b[at..at + len];
                let w = u32::from_be_bytes(png[16..20].try_into().unwrap()) as f32;
                let h = u32::from_be_bytes(png[20..24].try_into().unwrap()) as f32;
                assert!(
                    f.src.x >= 0.0
                        && f.src.y >= 0.0
                        && f.src.x + f.src.w <= w
                        && f.src.y + f.src.h <= h,
                    "{name} in {p:?} spills off its page"
                );
            }
        }
        assert!(seen > 0);
    }
}
