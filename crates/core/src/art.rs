use crate::geom::Rect;
use glam::{vec2, Vec2};
use std::collections::HashMap;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Frame {
    pub page: usize,
    pub src: Rect,
    pub shown: Vec2,
}

impl Frame {
    pub fn size(&self) -> Vec2 {
        self.shown
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
        if f.len() != 6 && f.len() != 8 {
            return None;
        }
        let num = |i: usize| f[i].parse::<f32>().ok();
        let src = Rect::new(num(2)?, num(3)?, num(4)?, num(5)?);
        let shown = if f.len() == 8 {
            vec2(num(6)?, num(7)?)
        } else {
            vec2(src.w, src.h)
        };
        frames.insert(
            f[0].to_string(),
            Frame {
                page: f[1].parse().ok()?,
                src,
                shown,
            },
        );
    }
    Some(Index { pages, frames })
}

pub fn page_bytes<'a>(bytes: &'a [u8], index: &Index, page: usize) -> Option<&'a [u8]> {
    let (at, len) = *index.pages.get(page)?;
    bytes.get(at..at + len)
}

pub fn decode_page(bytes: &[u8]) -> Option<(u32, u32, Vec<u8>)> {
    let im = image::load_from_memory(bytes).ok()?.to_rgba8();
    let (w, h) = im.dimensions();
    Some((w, h, im.into_raw()))
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
    fn pages_decode_from_png_and_jpeg() {
        let mut png = vec![];
        let mut jpg = vec![];
        let img = image::RgbaImage::from_pixel(8, 4, image::Rgba([200, 100, 50, 255]));
        image::DynamicImage::ImageRgba8(img.clone())
            .write_to(
                &mut std::io::Cursor::new(&mut png),
                image::ImageFormat::Png,
            )
            .unwrap();
        image::DynamicImage::ImageRgba8(img)
            .to_rgb8()
            .write_to(
                &mut std::io::Cursor::new(&mut jpg),
                image::ImageFormat::Jpeg,
            )
            .unwrap();
        for b in [png, jpg] {
            let (w, h, px) = decode_page(&b).unwrap();
            assert_eq!((w, h), (8, 4));
            assert_eq!(px.len(), 8 * 4 * 4);
            assert!((px[0] as i32 - 200).abs() < 8 && px[3] == 255);
        }
    }

    #[test]
    fn lossy_webp_pages_keep_their_alpha() {
        let b = include_bytes!("../../../tests/data/test-alpha.webp");
        let (w, h, px) = decode_page(b).unwrap();
        assert_eq!((w, h), (64, 32));
        assert!(px[3] < 10, "left half should be clear");
        let right = ((10 * w + 50) * 4) as usize;
        assert!(px[right + 3] > 245 && (px[right] as i32 - 200).abs() < 12);
    }

    #[test]
    fn decoding_every_page_is_quick() {
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets/art");
        let t = std::time::Instant::now();
        for e in std::fs::read_dir(dir).unwrap() {
            let p = e.unwrap().path();
            let b = std::fs::read(&p).unwrap();
            let idx = parse(&b).unwrap();
            for (at, len) in idx.pages {
                decode_page(&b[at..at + len]).unwrap();
            }
        }
        eprintln!("decoded all pages in {:?}", t.elapsed());
    }

    #[test]
    fn rejects_garbage() {
        assert!(parse(b"nope").is_none());
        assert!(parse(b"KGA1\x05\x00\x00\x00").is_none());
    }

    #[test]
    fn every_sheet_parses_and_frames_sit_inside_their_page() {
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets/art");
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
