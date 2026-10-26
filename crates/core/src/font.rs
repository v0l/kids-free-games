use std::cell::{Cell, RefCell};

pub static FONT_BYTES: &[u8] = include_bytes!("../../../assets/Fredoka-SemiBold.ttf");

pub const GLYPH_SIZES: [f32; 7] = [24.0, 36.0, 52.0, 72.0, 100.0, 140.0, 200.0];

thread_local! {
    static TEXT_SCALE: Cell<f32> = const { Cell::new(1.0) };
    static METRICS: RefCell<Option<fontdue::Font>> = const { RefCell::new(None) };
}

pub fn set_text_scale(scale: f32) {
    TEXT_SCALE.with(|s| s.set(scale));
}

pub fn text_scale() -> f32 {
    TEXT_SCALE.with(|s| s.get())
}

pub fn font_params(size: f32) -> (u16, f32) {
    let want = (size * text_scale()).max(1.0);
    let px = GLYPH_SIZES
        .iter()
        .copied()
        .find(|g| *g >= want * 0.9)
        .unwrap_or(GLYPH_SIZES[GLYPH_SIZES.len() - 1]);
    (px as u16, size / px)
}

pub fn measure(s: &str, px: f32) -> f32 {
    METRICS.with(|f| {
        let mut f = f.borrow_mut();
        if f.is_none() {
            *f = fontdue::Font::from_bytes(FONT_BYTES, fontdue::FontSettings::default()).ok();
        }
        let Some(font) = f.as_ref() else { return 0.0 };
        let mut w = 0.0;
        let mut prev = None;
        for ch in s.chars() {
            if let Some(p) = prev {
                w += font.horizontal_kern(p, ch, px).unwrap_or(0.0);
            }
            w += font.metrics(ch, px).advance_width;
            prev = Some(ch);
        }
        w
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fx::ease_out_back;

    #[test]
    fn panel_pop_in_does_not_rasterise_new_glyph_sizes() {
        for scale in [1.0, 1.33, 2.625] {
            set_text_scale(scale);
            let mut sizes: Vec<u16> = vec![];
            for frame in 0..60 {
                let s = ease_out_back(frame as f32 / 120.0 / 0.28).max(0.01);
                for base in [58.0, 36.0, 30.0, 32.0] {
                    sizes.push(font_params(base * s).0);
                }
            }
            for px in sizes {
                assert!(
                    GLYPH_SIZES.contains(&(px as f32)),
                    "scale {scale}: {px}px is not prewarmed and rasterises mid-animation"
                );
            }
        }
        set_text_scale(1.0);
    }

    #[test]
    fn animated_text_reuses_a_few_glyph_sizes() {
        set_text_scale(2.625);
        let bounce = |i: usize| 72.0 * (1.0 + 0.35 * (i as f32 / 2000.0));
        let mut sizes: Vec<u16> = (0..2000).map(|i| font_params(bounce(i)).0).collect();
        sizes.sort();
        sizes.dedup();
        assert!(
            sizes.len() <= 2,
            "{} distinct glyph sizes for one bouncing label",
            sizes.len()
        );
        for i in 0..2000 {
            let size = 8.0 + i as f32 * 0.05;
            let (px, k) = font_params(size);
            assert!((px as f32 * k - size).abs() < 1e-3);
            let want = size * 2.625;
            if (24.0..=200.0).contains(&want) {
                assert!(
                    px as f32 >= want * 0.9,
                    "{want}px drawn from a {px}px glyph would blur"
                );
                assert!(
                    px as f32 <= want * 1.6,
                    "{want}px drawn from a {px}px glyph wastes atlas"
                );
            }
        }
        set_text_scale(1.0);
    }

    #[test]
    fn measuring_grows_with_the_string_and_the_size() {
        let a = measure("Kids", 30.0);
        assert!(a > 30.0);
        assert!(measure("Kids Free", 30.0) > a);
        assert!((measure("Kids", 60.0) - 2.0 * a).abs() < 1.0);
    }
}
