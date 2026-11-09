use crate::geom::Rect;
use crate::safe::Insets;
use glam::{vec2, Vec2};

pub const MIN_W: f32 = 960.0;
pub const MIN_H: f32 = 720.0;
pub use crate::input::*;
pub const HUD_LEFT: f32 = 90.0;

pub trait Game {
    fn update(&mut self, input: &Input, view: &View, dt: f32);
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct View {
    pub w: f32,
    pub h: f32,
    pub scale: f32,
    pub inset: Insets,
    pub mm: f32,
}

impl View {
    pub fn fit(screen_w: f32, screen_h: f32) -> Self {
        Self::fit_safe(screen_w, screen_h, Insets::default())
    }

    pub fn fit_safe(screen_w: f32, screen_h: f32, px: Insets) -> Self {
        let safe_w = (screen_w - px.left - px.right).max(1.0);
        let safe_h = (screen_h - px.top - px.bottom).max(1.0);
        let scale = (safe_w / MIN_W).min(safe_h / MIN_H).max(0.01);
        Self {
            w: safe_w / scale,
            h: safe_h / scale,
            scale,
            inset: Insets {
                left: px.left / scale,
                top: px.top / scale,
                right: px.right / scale,
                bottom: px.bottom / scale,
            },
            mm: guess_units_per_mm(screen_w, screen_h) / scale,
        }
    }

    pub fn with_density(mut self, units_per_mm: f32) -> Self {
        if units_per_mm.is_finite() && units_per_mm > 0.0 {
            self.mm = units_per_mm / self.scale;
        }
        self
    }

    pub fn device(w: f32, h: f32, dpr: f32, ppi: f32, px: Insets) -> Self {
        Self::fit_safe(w, h, px).with_density(ppi / 25.4 / dpr)
    }

    pub fn ipad_landscape() -> Self {
        Self::fit(1024.0, 768.0)
    }

    pub fn ipad_portrait() -> Self {
        Self::fit(768.0, 1024.0)
    }

    pub fn center(&self) -> Vec2 {
        vec2(self.w / 2.0, self.h / 2.0)
    }

    pub fn rect(&self) -> Rect {
        Rect::new(0.0, 0.0, self.w, self.h)
    }

    pub fn full(&self) -> Rect {
        Rect::new(
            -self.inset.left,
            -self.inset.top,
            self.w + self.inset.left + self.inset.right,
            self.h + self.inset.top + self.inset.bottom,
        )
    }

    pub fn to_view(&self, screen: Vec2) -> Vec2 {
        screen / self.scale - vec2(self.inset.left, self.inset.top)
    }

    pub fn to_screen(&self, p: Vec2) -> Vec2 {
        (p + vec2(self.inset.left, self.inset.top)) * self.scale
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Best(pub u32);

impl Best {
    pub fn offer(&mut self, score: u32) {
        self.0 = self.0.max(score);
    }

    pub fn load(game: &str) -> Self {
        Self(crate::profile::personal_best(game).unwrap_or(0).max(0) as u32)
    }

    pub fn submit(&mut self, game: &str, score: u32) -> Option<usize> {
        self.offer(score);
        crate::profile::submit(game, score as i64, crate::profile::Better::Higher)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn saved_best_survives_a_restart() {
        crate::save::reset_for_test();
        crate::profile::create("mia", 0);
        let mut b = Best::load("t_engine");
        assert_eq!(b.0, 0);
        assert_eq!(b.submit("t_engine", 12), Some(0));
        assert_eq!(b.submit("t_engine", 5), Some(1));
        assert_eq!(Best::load("t_engine").0, 12);
    }

    #[test]
    fn safe_area_shrinks_the_layout_and_offsets_touches() {
        let px = Insets {
            left: 0.0,
            top: 149.0,
            right: 0.0,
            bottom: 54.0,
        };
        let v = View::fit_safe(1008.0, 2244.0, px);
        assert!((v.h * v.scale - (2244.0 - 149.0 - 54.0)).abs() < 0.5);
        assert!(v.w >= MIN_W - 0.01 && v.h >= MIN_H - 0.01);
        let top_left = v.to_view(vec2(0.0, 149.0));
        assert!(top_left.length() < 0.01);
        let f = v.full();
        assert!((f.y * v.scale + 149.0).abs() < 0.5);
        assert!(((f.y + f.h) * v.scale - 2244.0 + 149.0).abs() < 0.5);
        assert!((v.to_screen(top_left) - vec2(0.0, 149.0)).length() < 0.01);
    }

    #[test]
    fn landscape_cutout_on_the_left() {
        let px = Insets {
            left: 149.0,
            top: 0.0,
            right: 0.0,
            bottom: 54.0,
        };
        let v = View::fit_safe(2244.0, 1008.0, px);
        assert!((v.w * v.scale - (2244.0 - 149.0)).abs() < 0.5);
        assert!((v.to_view(vec2(149.0, 0.0))).length() < 0.01);
    }

    #[test]
    fn view_keeps_minimum_canvas_in_both_orientations() {
        let l = View::ipad_landscape();
        assert!((l.w - 960.0).abs() < 0.01 && (l.h - 720.0).abs() < 0.01);
        let p = View::ipad_portrait();
        assert!(p.w >= MIN_W - 0.01 && p.h >= MIN_H - 0.01);
        assert!(p.h > p.w);
        let wide = View::fit(1920.0, 1080.0);
        assert!((wide.h - 720.0).abs() < 0.01 && wide.w > 1200.0);
    }
}
