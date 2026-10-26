use glam::{vec2, Vec2, Vec4};

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Rect {
    pub const fn new(x: f32, y: f32, w: f32, h: f32) -> Self {
        Self { x, y, w, h }
    }

    pub fn point(&self) -> Vec2 {
        vec2(self.x, self.y)
    }

    pub fn size(&self) -> Vec2 {
        vec2(self.w, self.h)
    }

    pub fn center(&self) -> Vec2 {
        vec2(self.x + self.w / 2.0, self.y + self.h / 2.0)
    }

    pub fn left(&self) -> f32 {
        self.x
    }

    pub fn right(&self) -> f32 {
        self.x + self.w
    }

    pub fn top(&self) -> f32 {
        self.y
    }

    pub fn bottom(&self) -> f32 {
        self.y + self.h
    }

    pub fn contains(&self, p: Vec2) -> bool {
        p.x >= self.x && p.x <= self.x + self.w && p.y >= self.y && p.y <= self.y + self.h
    }

    pub fn overlaps(&self, o: &Rect) -> bool {
        self.left() <= o.right()
            && self.right() >= o.left()
            && self.top() <= o.bottom()
            && self.bottom() >= o.top()
    }

    pub fn offset(self, by: Vec2) -> Rect {
        Rect::new(self.x + by.x, self.y + by.y, self.w, self.h)
    }

    pub fn scaled_about_center(self, k: f32) -> Rect {
        let c = self.center();
        Rect::new(
            c.x - self.w * k / 2.0,
            c.y - self.h * k / 2.0,
            self.w * k,
            self.h * k,
        )
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Color {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl Color {
    pub const fn new(r: f32, g: f32, b: f32, a: f32) -> Self {
        Self { r, g, b, a }
    }

    pub const fn from_rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self::new(
            r as f32 / 255.0,
            g as f32 / 255.0,
            b as f32 / 255.0,
            a as f32 / 255.0,
        )
    }

    pub fn from_vec(v: Vec4) -> Self {
        Self::new(v.x, v.y, v.z, v.w)
    }

    pub fn to_vec(self) -> Vec4 {
        Vec4::new(self.r, self.g, self.b, self.a)
    }

    pub fn with_alpha(self, a: f32) -> Self {
        Self { a, ..self }
    }

    pub fn darker(self, k: f32) -> Self {
        Self::new(self.r * k, self.g * k, self.b * k, self.a)
    }

    pub fn lerp(self, o: Color, t: f32) -> Self {
        Color::from_vec(self.to_vec().lerp(o.to_vec(), t))
    }
}

pub const WHITE: Color = Color::new(1.0, 1.0, 1.0, 1.0);
pub const BLACK: Color = Color::new(0.0, 0.0, 0.0, 1.0);
pub const BLANK: Color = Color::new(0.0, 0.0, 0.0, 0.0);
pub const GOLD: Color = Color::new(1.0, 0.8, 0.0, 1.0);
pub const MAGENTA: Color = Color::new(1.0, 0.0, 1.0, 1.0);
pub const PINK: Color = Color::new(1.0, 0.43, 0.76, 1.0);
pub const RED: Color = Color::new(0.9, 0.16, 0.22, 1.0);

pub fn with_alpha(c: Color, a: f32) -> Color {
    c.with_alpha(a)
}

pub fn darker(c: Color, k: f32) -> Color {
    c.darker(k)
}

pub fn hsv(h: f32, s: f32, v: f32) -> Color {
    let h = h.rem_euclid(1.0) * 6.0;
    let i = h.floor();
    let f = h - i;
    let (p, q, t) = (v * (1.0 - s), v * (1.0 - s * f), v * (1.0 - s * (1.0 - f)));
    let (r, g, b) = match i as i32 {
        0 => (v, t, p),
        1 => (q, v, p),
        2 => (p, v, t),
        3 => (p, q, v),
        4 => (t, p, v),
        _ => (v, p, q),
    };
    Color::new(r, g, b, 1.0)
}

pub fn segment_hits_circle(a: Vec2, b: Vec2, c: Vec2, r: f32) -> bool {
    let ab = b - a;
    let len2 = ab.length_squared();
    let t = if len2 == 0.0 {
        0.0
    } else {
        ((c - a).dot(ab) / len2).clamp(0.0, 1.0)
    };
    (a + ab * t).distance(c) <= r
}

pub fn circle_hits_rect(c: Vec2, r: f32, rect: Rect) -> bool {
    let nx = c.x.clamp(rect.x, rect.x + rect.w);
    let ny = c.y.clamp(rect.y, rect.y + rect.h);
    vec2(nx, ny).distance(c) < r
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rects_contain_their_edges_and_overlap_when_touching() {
        let r = Rect::new(10.0, 10.0, 20.0, 10.0);
        assert!(r.contains(vec2(10.0, 10.0)) && r.contains(vec2(30.0, 20.0)));
        assert!(!r.contains(vec2(31.0, 15.0)));
        assert!(r.overlaps(&Rect::new(30.0, 20.0, 5.0, 5.0)));
        assert!(!r.overlaps(&Rect::new(31.0, 10.0, 5.0, 5.0)));
        assert_eq!(r.center(), vec2(20.0, 15.0));
    }

    #[test]
    fn colours_from_bytes_and_hsv_hit_their_primaries() {
        assert_eq!(
            Color::from_rgba(255, 0, 0, 255),
            Color::new(1.0, 0.0, 0.0, 1.0)
        );
        assert_eq!(hsv(0.0, 1.0, 1.0), Color::new(1.0, 0.0, 0.0, 1.0));
        let g = hsv(1.0 / 3.0, 1.0, 1.0);
        assert!(g.g > 0.99 && g.r < 0.01);
    }

    #[test]
    fn segment_circle_intersection() {
        assert!(segment_hits_circle(
            vec2(0.0, 0.0),
            vec2(100.0, 0.0),
            vec2(50.0, 5.0),
            10.0
        ));
        assert!(!segment_hits_circle(
            vec2(0.0, 0.0),
            vec2(100.0, 0.0),
            vec2(50.0, 30.0),
            10.0
        ));
        assert!(!segment_hits_circle(
            vec2(0.0, 0.0),
            vec2(10.0, 0.0),
            vec2(50.0, 0.0),
            10.0
        ));
    }
}
