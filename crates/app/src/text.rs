use kg_core::outline::{Cache, Glyph};
use std::cell::RefCell;

thread_local! {
    static CACHE: RefCell<Option<Cache>> = const { RefCell::new(None) };
}

fn with<R>(f: impl FnOnce(&mut Cache) -> R) -> Option<R> {
    CACHE.with(|c| {
        let mut c = c.borrow_mut();
        if c.is_none() {
            *c = Cache::new(kg_core::font::FONT_BYTES);
        }
        c.as_mut().map(f)
    })
}

pub struct Line {
    pub glyphs: Vec<(Glyph, f32)>,
    pub width: f32,
    pub atlas: usize,
}

pub fn layout(s: &str, px: u16) -> Option<Line> {
    with(|c| {
        let (placed, w) = c.layout(s, px);
        Line {
            glyphs: placed,
            width: w,
            atlas: c.atlas.size,
        }
    })
}

pub fn atlas_alpha() -> (usize, Vec<u8>) {
    with(|c| (c.atlas.size, c.atlas.alpha.clone())).unwrap_or((1, vec![0]))
}

pub fn atlas_generation() -> u64 {
    with(|c| c.atlas.generation).unwrap_or(0)
}

pub fn prewarm() {
    let chars: String = (32u8..127).map(char::from).collect();
    for px in [24u16, 36, 52, 72] {
        layout(&chars, px);
    }
}
