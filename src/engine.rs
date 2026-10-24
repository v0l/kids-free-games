use macroquad::prelude::*;
use std::cell::Cell;

pub const MIN_W: f32 = 960.0;
pub const MIN_H: f32 = 720.0;
pub const SWIPE_DIST: f32 = 36.0;
pub const HUD_LEFT: f32 = 90.0;

pub trait Game {
    fn update(&mut self, input: &Input, view: &View, dt: f32);
    fn draw(&self, view: &View);
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct View {
    pub w: f32,
    pub h: f32,
    pub scale: f32,
}

impl View {
    pub fn fit(screen_w: f32, screen_h: f32) -> Self {
        let scale = (screen_w / MIN_W).min(screen_h / MIN_H).max(0.01);
        Self {
            w: screen_w / scale,
            h: screen_h / scale,
            scale,
        }
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
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Dir {
    Up,
    Down,
    Left,
    Right,
}

impl Dir {
    pub fn delta(self) -> IVec2 {
        match self {
            Dir::Up => ivec2(0, -1),
            Dir::Down => ivec2(0, 1),
            Dir::Left => ivec2(-1, 0),
            Dir::Right => ivec2(1, 0),
        }
    }

    pub fn opposite(self) -> Dir {
        match self {
            Dir::Up => Dir::Down,
            Dir::Down => Dir::Up,
            Dir::Left => Dir::Right,
            Dir::Right => Dir::Left,
        }
    }

    pub fn from_vec(v: Vec2) -> Dir {
        if v.x.abs() > v.y.abs() {
            if v.x > 0.0 {
                Dir::Right
            } else {
                Dir::Left
            }
        } else if v.y > 0.0 {
            Dir::Down
        } else {
            Dir::Up
        }
    }

    pub const ALL: [Dir; 4] = [Dir::Up, Dir::Down, Dir::Left, Dir::Right];
}

pub fn swipe_dir(delta: Vec2, threshold: f32) -> Option<Dir> {
    (delta.length() >= threshold).then(|| Dir::from_vec(delta))
}

#[derive(Clone, Debug, Default)]
pub struct Input {
    pub pos: Vec2,
    pub prev_pos: Vec2,
    pub down: bool,
    pub pressed: bool,
    pub released: bool,
    pub swipe: Option<Dir>,
    pub key_dir: Option<Dir>,
    pub left: bool,
    pub right: bool,
    pub action: bool,
}

impl Input {
    pub fn dir(&self) -> Option<Dir> {
        self.key_dir.or(self.swipe)
    }

    pub fn tap(&self) -> bool {
        self.pressed || self.action
    }

    pub fn tapped_in(&self, r: Rect) -> bool {
        self.pressed && r.contains(self.pos)
    }

    pub fn press_at(pos: Vec2) -> Self {
        Self {
            pos,
            prev_pos: pos,
            down: true,
            pressed: true,
            ..Default::default()
        }
    }

    pub fn key(dir: Dir) -> Self {
        Self {
            key_dir: Some(dir),
            ..Default::default()
        }
    }
}

#[derive(Default)]
pub struct SwipeTracker {
    anchor: Option<Vec2>,
}

impl SwipeTracker {
    pub fn track(&mut self, pos: Vec2, down: bool, pressed: bool) -> Option<Dir> {
        if pressed {
            self.anchor = Some(pos);
        }
        if !down {
            self.anchor = None;
            return None;
        }
        let dir = swipe_dir(pos - self.anchor?, SWIPE_DIST)?;
        self.anchor = Some(pos);
        Some(dir)
    }
}

#[derive(Default)]
pub struct InputTracker {
    swipes: SwipeTracker,
    last: Vec2,
}

impl InputTracker {
    pub fn poll(&mut self, view: &View) -> Input {
        let (mx, my) = mouse_position();
        let pos = vec2(mx, my) / view.scale;
        let down = is_mouse_button_down(MouseButton::Left);
        let pressed = is_mouse_button_pressed(MouseButton::Left);
        let released = is_mouse_button_released(MouseButton::Left);
        let prev_pos = if pressed { pos } else { self.last };
        self.last = pos;
        let swipe = self.swipes.track(pos, down, pressed);
        let key_dir = [
            (KeyCode::Up, Dir::Up),
            (KeyCode::W, Dir::Up),
            (KeyCode::Down, Dir::Down),
            (KeyCode::S, Dir::Down),
            (KeyCode::Left, Dir::Left),
            (KeyCode::A, Dir::Left),
            (KeyCode::Right, Dir::Right),
            (KeyCode::D, Dir::Right),
        ]
        .into_iter()
        .find(|(k, _)| is_key_pressed(*k))
        .map(|(_, d)| d);
        Input {
            pos,
            prev_pos,
            down,
            pressed,
            released,
            swipe,
            key_dir,
            left: is_key_down(KeyCode::Left) || is_key_down(KeyCode::A),
            right: is_key_down(KeyCode::Right) || is_key_down(KeyCode::D),
            action: is_key_pressed(KeyCode::Space) || is_key_pressed(KeyCode::Enter),
        }
    }
}

thread_local! {
    static TEXT_SCALE: Cell<f32> = const { Cell::new(1.0) };
    static FONT: std::cell::OnceCell<Option<Font>> = const { std::cell::OnceCell::new() };
}

static FONT_BYTES: &[u8] = include_bytes!("../assets/Fredoka-SemiBold.ttf");

fn font() -> Option<Font> {
    FONT.with(|f| {
        f.get_or_init(|| load_ttf_font_from_bytes(FONT_BYTES).ok())
            .clone()
    })
}

pub fn set_text_scale(scale: f32) {
    TEXT_SCALE.with(|s| s.set(scale));
}

fn font_params(size: f32) -> (u16, f32) {
    let scale = TEXT_SCALE.with(|s| s.get());
    let px = (size * scale).round().clamp(4.0, 400.0) as u16;
    (px, size / px as f32)
}

pub fn text_size(s: &str, size: f32) -> TextDimensions {
    let (px, k) = font_params(size);
    measure_text(s, font().as_ref(), px, k)
}

pub fn text(s: &str, x: f32, baseline: f32, size: f32, color: Color) {
    let (px, k) = font_params(size);
    draw_text_ex(
        s,
        x,
        baseline,
        TextParams {
            font: font().as_ref(),
            font_size: px,
            font_scale: k,
            color,
            ..Default::default()
        },
    );
}

pub fn text_center(s: &str, cx: f32, cy: f32, size: f32, color: Color) {
    let d = text_size(s, size);
    text(s, cx - d.width / 2.0, cy + size * 0.33, size, color);
}

pub fn text_shadow(s: &str, cx: f32, cy: f32, size: f32, color: Color) {
    let outline = if color.r + color.g + color.b > 2.2 {
        Color::from_rgba(40, 30, 60, 255)
    } else {
        Color::new(1.0, 1.0, 1.0, 0.9)
    };
    crate::fx::text_outline(s, cx, cy, size, color, with_alpha(outline, color.a));
}

pub fn round_rect(r: Rect, radius: f32, color: Color) {
    let rad = radius.min(r.w / 2.0).min(r.h / 2.0).max(0.0);
    draw_rectangle(r.x + rad, r.y, r.w - 2.0 * rad, r.h, color);
    draw_rectangle(r.x, r.y + rad, rad, r.h - 2.0 * rad, color);
    draw_rectangle(r.x + r.w - rad, r.y + rad, rad, r.h - 2.0 * rad, color);
    if rad <= 0.0 {
        return;
    }
    let segs = ((rad * 0.5) as usize).clamp(3, 12);
    for (c, start) in [
        (vec2(r.x + rad, r.y + rad), std::f32::consts::PI),
        (vec2(r.x + r.w - rad, r.y + rad), std::f32::consts::PI * 1.5),
        (vec2(r.x + r.w - rad, r.y + r.h - rad), 0.0),
        (
            vec2(r.x + rad, r.y + r.h - rad),
            std::f32::consts::FRAC_PI_2,
        ),
    ] {
        for k in 0..segs {
            let a0 = start + std::f32::consts::FRAC_PI_2 * k as f32 / segs as f32;
            let a1 = start + std::f32::consts::FRAC_PI_2 * (k + 1) as f32 / segs as f32;
            draw_triangle(
                c,
                c + vec2(a0.cos(), a0.sin()) * rad,
                c + vec2(a1.cos(), a1.sin()) * rad,
                color,
            );
        }
    }
}

pub fn button(r: Rect, label: &str, color: Color, enabled: bool) {
    let color = if enabled {
        color
    } else {
        Color::new(0.6, 0.6, 0.6, 1.0)
    };
    round_rect(
        Rect::new(r.x, r.y + 5.0, r.w, r.h),
        14.0,
        darker(color, 0.65),
    );
    round_rect(r, 14.0, color);
    crate::fx::gloss(r, 14.0);
    let size = (r.h * 0.42).min(r.w / (label.len().max(1) as f32 * 0.5));
    text_shadow(label, r.x + r.w / 2.0, r.y + r.h / 2.0, size, WHITE);
}

pub fn overlay(view: &View, title: &str, lines: &[&str]) {
    let key = title.to_string();
    let age = OVERLAY.with(|o| {
        let mut o = o.borrow_mut();
        o.seen = true;
        if o.title != key {
            o.title = key;
            o.age = 0.0;
        }
        o.age
    });
    let accent = ACCENT.with(|a| a.get());
    crate::fx::panel(view, title, lines, age, accent);
}

#[derive(Default)]
struct OverlayClock {
    title: String,
    age: f32,
    seen: bool,
}

thread_local! {
    static OVERLAY: std::cell::RefCell<OverlayClock> = std::cell::RefCell::new(OverlayClock::default());
    static ACCENT: Cell<Color> = const { Cell::new(Color::new(0.95, 0.45, 0.4, 1.0)) };
}

pub fn set_accent(c: Color) {
    ACCENT.with(|a| a.set(c));
}

pub fn tick_overlay(dt: f32) {
    OVERLAY.with(|o| {
        let mut o = o.borrow_mut();
        if o.seen {
            o.age += dt;
        } else {
            o.title.clear();
            o.age = 0.0;
        }
        o.seen = false;
    });
}

pub fn darker(c: Color, k: f32) -> Color {
    Color::new(c.r * k, c.g * k, c.b * k, c.a)
}

pub fn with_alpha(c: Color, a: f32) -> Color {
    Color::new(c.r, c.g, c.b, a)
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

pub fn sky(view: &View, top: Color, bottom: Color) {
    let bands = 24;
    let bh = view.h / bands as f32;
    for i in 0..bands {
        let t = i as f32 / (bands - 1) as f32;
        let c = Color::new(
            top.r + (bottom.r - top.r) * t,
            top.g + (bottom.g - top.g) * t,
            top.b + (bottom.b - top.b) * t,
            1.0,
        );
        draw_rectangle(0.0, i as f32 * bh, view.w, bh + 1.0, c);
    }
}

pub fn cloud(x: f32, y: f32, s: f32, color: Color) {
    draw_circle(x, y, 26.0 * s, color);
    draw_circle(x + 30.0 * s, y - 12.0 * s, 32.0 * s, color);
    draw_circle(x + 62.0 * s, y, 24.0 * s, color);
    draw_rectangle(x, y - 4.0 * s, 62.0 * s, 28.0 * s, color);
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

#[derive(Clone, Copy, Debug, Default)]
pub struct Best(pub u32);

impl Best {
    pub fn offer(&mut self, score: u32) {
        self.0 = self.0.max(score);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn short_drag_is_not_a_swipe() {
        assert_eq!(swipe_dir(vec2(10.0, 5.0), SWIPE_DIST), None);
        assert_eq!(swipe_dir(vec2(-50.0, 10.0), SWIPE_DIST), Some(Dir::Left));
        assert_eq!(swipe_dir(vec2(5.0, 60.0), SWIPE_DIST), Some(Dir::Down));
    }

    #[test]
    fn one_drag_can_chain_two_swipes() {
        let mut t = SwipeTracker::default();
        assert_eq!(t.track(vec2(100.0, 100.0), true, true), None);
        assert_eq!(t.track(vec2(150.0, 100.0), true, false), Some(Dir::Right));
        assert_eq!(t.track(vec2(160.0, 100.0), true, false), None);
        assert_eq!(t.track(vec2(160.0, 40.0), true, false), Some(Dir::Up));
        assert_eq!(t.track(vec2(160.0, 0.0), false, false), None);
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

    #[test]
    fn opposite_dirs() {
        for d in Dir::ALL {
            assert_eq!(d.opposite().opposite(), d);
            assert_eq!(d.delta() + d.opposite().delta(), IVec2::ZERO);
        }
    }
}
