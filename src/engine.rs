use macroquad::prelude::*;
use std::cell::Cell;

pub const MIN_W: f32 = 960.0;
pub const MIN_H: f32 = 720.0;
pub const SWIPE_DIST: f32 = 36.0;
pub const SWIPE_MM: f32 = 6.0;
pub const TAP_SLOP_MM: f32 = 4.0;
pub const FALLBACK_MM: f32 = 6.0;
const AXIS_RATIO: f32 = 1.3;
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
    pub inset: crate::safe::Insets,
}

impl View {
    pub fn fit(screen_w: f32, screen_h: f32) -> Self {
        Self::fit_safe(screen_w, screen_h, crate::safe::Insets::default())
    }

    pub fn fit_safe(screen_w: f32, screen_h: f32, px: crate::safe::Insets) -> Self {
        let safe_w = (screen_w - px.left - px.right).max(1.0);
        let safe_h = (screen_h - px.top - px.bottom).max(1.0);
        let scale = (safe_w / MIN_W).min(safe_h / MIN_H).max(0.01);
        Self {
            w: safe_w / scale,
            h: safe_h / scale,
            scale,
            inset: crate::safe::Insets {
                left: px.left / scale,
                top: px.top / scale,
                right: px.right / scale,
                bottom: px.bottom / scale,
            },
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
    pub back: bool,
    pub start: Vec2,
    pub travel: f32,
    pub swiped: bool,
    pub mm: f32,
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

    pub fn mm(&self) -> f32 {
        if self.mm > 0.0 {
            self.mm
        } else {
            FALLBACK_MM
        }
    }

    pub fn tap_released(&self) -> bool {
        self.released && !self.swiped && self.travel < TAP_SLOP_MM * self.mm()
    }

    pub fn press_at(pos: Vec2) -> Self {
        Self {
            pos,
            prev_pos: pos,
            start: pos,
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

pub fn clear_swipe(delta: Vec2, threshold: f32) -> Option<Dir> {
    let len = delta.length();
    if len < threshold {
        return None;
    }
    let (major, minor) = if delta.x.abs() > delta.y.abs() {
        (delta.x.abs(), delta.y.abs())
    } else {
        (delta.y.abs(), delta.x.abs())
    };
    (major >= minor * AXIS_RATIO || len >= threshold * 2.0).then(|| Dir::from_vec(delta))
}

#[derive(Default)]
pub struct SwipeTracker {
    anchor: Option<Vec2>,
    last: Option<Dir>,
    pub start: Vec2,
    pub travel: f32,
    pub swiped: bool,
}

impl SwipeTracker {
    pub fn track(&mut self, pos: Vec2, down: bool, pressed: bool, threshold: f32) -> Option<Dir> {
        if pressed {
            self.anchor = Some(pos);
            self.last = None;
            self.start = pos;
            self.travel = 0.0;
            self.swiped = false;
        }
        if self.anchor.is_some() {
            self.travel = self.travel.max(pos.distance(self.start));
        }
        if !down {
            self.anchor = None;
            return None;
        }
        let dir = clear_swipe(pos - self.anchor?, threshold)?;
        self.anchor = Some(pos);
        if self.last == Some(dir) {
            return None;
        }
        self.last = Some(dir);
        self.swiped = true;
        Some(dir)
    }
}

pub fn guess_units_per_mm(screen_w: f32, screen_h: f32) -> f32 {
    let short = screen_w.min(screen_h);
    let dpi = if short < 520.0 {
        160.0
    } else if short < 1050.0 {
        132.0
    } else {
        96.0
    };
    dpi / 25.4
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Finger {
    Started,
    Held,
    Ended,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Pointer {
    pub pos: Vec2,
    pub down: bool,
    pub pressed: bool,
    pub released: bool,
}

#[derive(Default)]
pub struct PrimaryFinger {
    id: Option<u64>,
    pending: Option<u64>,
    known: Vec<u64>,
    pos: Vec2,
}

impl PrimaryFinger {
    pub fn update(&mut self, touches: &[(u64, Finger, Vec2)]) -> Pointer {
        let find = |id: u64| touches.iter().find(|t| t.0 == id).copied();
        let fresh = |known: &[u64], skip: Option<u64>| {
            touches
                .iter()
                .find(|t| Some(t.0) != skip && t.1 != Finger::Ended && !known.contains(&t.0))
                .or_else(|| {
                    touches
                        .iter()
                        .find(|t| Some(t.0) != skip && t.1 == Finger::Started)
                })
                .map(|t| t.0)
        };
        let mut out = Pointer {
            pos: self.pos,
            ..Default::default()
        };
        if let Some(id) = self.id {
            match find(id) {
                Some((_, f, p)) if f != Finger::Ended => {
                    out.pos = p;
                    out.down = true;
                }
                hit => {
                    out.pos = hit.map_or(self.pos, |t| t.2);
                    out.released = true;
                    self.id = None;
                    self.pending = fresh(&self.known, Some(id));
                }
            }
        } else {
            let pending = self
                .pending
                .take()
                .filter(|id| find(*id).is_some_and(|t| t.1 != Finger::Ended));
            let tap = touches
                .iter()
                .find(|t| t.1 == Finger::Ended && !self.known.contains(&t.0))
                .copied();
            if let Some((id, _, p)) = pending
                .and_then(find)
                .or_else(|| fresh(&self.known, None).and_then(find))
            {
                self.id = Some(id);
                out = Pointer {
                    pos: p,
                    down: true,
                    pressed: true,
                    released: false,
                };
            } else if let Some((_, _, p)) = tap {
                out = Pointer {
                    pos: p,
                    down: false,
                    pressed: true,
                    released: true,
                };
            }
        }
        self.known = touches
            .iter()
            .filter(|t| t.1 != Finger::Ended)
            .map(|t| t.0)
            .collect();
        self.pos = out.pos;
        out
    }

    pub fn active(&self) -> bool {
        self.id.is_some() || self.pending.is_some()
    }
}

#[derive(Default)]
pub struct InputTracker {
    swipes: SwipeTracker,
    last: Vec2,
    units_per_mm: Option<f32>,
    fingers: PrimaryFinger,
}

impl InputTracker {
    fn view_mm(&mut self, view: &View) -> f32 {
        let per_mm = *self.units_per_mm.get_or_insert_with(|| {
            crate::safe::screen_units_per_mm()
                .map(|px| px / screen_dpi_scale())
                .unwrap_or_else(|| guess_units_per_mm(screen_width(), screen_height()))
        });
        per_mm / view.scale
    }

    fn pointer(&mut self) -> Pointer {
        let dpi = screen_dpi_scale();
        let touches: Vec<(u64, Finger, Vec2)> = touches()
            .into_iter()
            .map(|t| {
                let f = match t.phase {
                    TouchPhase::Started => Finger::Started,
                    TouchPhase::Ended | TouchPhase::Cancelled => Finger::Ended,
                    _ => Finger::Held,
                };
                (t.id, f, t.position / dpi)
            })
            .collect();
        if !touches.is_empty() || self.fingers.active() {
            return self.fingers.update(&touches);
        }
        let (mx, my) = mouse_position();
        Pointer {
            pos: vec2(mx, my),
            down: is_mouse_button_down(MouseButton::Left),
            pressed: is_mouse_button_pressed(MouseButton::Left),
            released: is_mouse_button_released(MouseButton::Left),
        }
    }

    pub fn poll(&mut self, view: &View) -> Input {
        let mm = self.view_mm(view);
        let Pointer {
            pos,
            down,
            pressed,
            released,
        } = self.pointer();
        let pos = view.to_view(pos);
        let prev_pos = if pressed { pos } else { self.last };
        self.last = pos;
        let swipe = self.swipes.track(pos, down, pressed, SWIPE_MM * mm);
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
            back: is_key_pressed(KeyCode::Escape) || is_key_pressed(KeyCode::Backspace),
            start: self.swipes.start,
            travel: self.swipes.travel,
            swiped: self.swipes.swiped,
            mm,
        }
    }
}

thread_local! {
    static TEXT_SCALE: Cell<f32> = const { Cell::new(1.0) };
    static FONT: std::cell::OnceCell<Option<Font>> = const { std::cell::OnceCell::new() };
}

pub static FONT_BYTES: &[u8] = include_bytes!("../assets/Fredoka-SemiBold.ttf");

fn font() -> Option<Font> {
    FONT.with(|f| {
        f.get_or_init(|| {
            let font = load_ttf_font_from_bytes(FONT_BYTES).ok()?;
            let chars: Vec<char> = (32u8..127).map(char::from).collect();
            for px in GLYPH_SIZES {
                font.populate_font_cache(&chars, px as u16);
            }

            Some(font)
        })
        .clone()
    })
}

pub fn set_text_scale(scale: f32) {
    TEXT_SCALE.with(|s| s.set(scale));
}

const GLYPH_SIZES: [f32; 7] = [24.0, 36.0, 52.0, 72.0, 100.0, 140.0, 200.0];

pub fn font_params(size: f32) -> (u16, f32) {
    let scale = TEXT_SCALE.with(|s| s.get());
    let want = (size * scale).max(1.0);
    let px = GLYPH_SIZES
        .iter()
        .copied()
        .find(|g| *g >= want * 0.9)
        .unwrap_or(GLYPH_SIZES[GLYPH_SIZES.len() - 1]);
    (px as u16, size / px)
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
    let segs = ((rad * TEXT_SCALE.with(|s| s.get()) * 0.25) as usize).clamp(3, 24);
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

pub fn results(view: &View, r: &crate::ui::Results) {
    let key = format!("results:{}", r.title);
    let age = OVERLAY.with(|o| {
        let mut o = o.borrow_mut();
        o.seen = true;
        if o.title != key {
            o.title = key;
            o.age = 0.0;
        }
        o.age
    });
    crate::ui::results(view, r, age);
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

pub fn set_view_camera(view: &View, shift: Vec2) {
    let f = view.full();
    set_camera(&Camera2D::from_display_rect(Rect::new(
        f.x - shift.x,
        f.y + f.h - shift.y,
        f.w,
        -f.h,
    )));
}

pub fn clip_px(view: &View, r: Rect, dpi: f32) -> (i32, i32, i32, i32) {
    let k = view.scale * dpi;
    let x = ((r.x + view.inset.left) * k).floor();
    let y = ((r.y + view.inset.top) * k).floor();
    let w = (r.w * k).ceil().max(0.0);
    let h = (r.h * k).ceil().max(0.0);
    (x as i32, y as i32, w as i32, h as i32)
}

pub fn clip(view: &View, r: Rect) {
    let px = clip_px(view, r, screen_dpi_scale());
    unsafe { get_internal_gl().quad_gl.scissor(Some(px)) };
}

pub fn unclip() {
    unsafe { get_internal_gl().quad_gl.scissor(None) };
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

pub fn circle_sides(radius_px: f32) -> u8 {
    (radius_px * 0.6).clamp(20.0, 96.0) as u8
}

pub fn draw_circle(x: f32, y: f32, r: f32, color: Color) {
    let px = r * TEXT_SCALE.with(|s| s.get());
    draw_poly(x, y, circle_sides(px), r, 0.0, color);
}

pub fn draw_ellipse(x: f32, y: f32, w: f32, h: f32, rotation: f32, color: Color) {
    use macroquad::models::{draw_mesh, Mesh, Vertex};
    let sides = circle_sides(w.abs().max(h.abs()) * TEXT_SCALE.with(|s| s.get())) as usize;
    let (sr, cr) = rotation.to_radians().sin_cos();
    let mut vertices = Vec::with_capacity(sides + 2);
    vertices.push(Vertex::new(x, y, 0.0, 0.0, 0.0, color));
    for i in 0..=sides {
        let a = i as f32 / sides as f32 * std::f32::consts::TAU;
        let (px, py) = (w * a.cos(), h * a.sin());
        vertices.push(Vertex::new(
            x + px * cr - py * sr,
            y + py * cr + px * sr,
            0.0,
            0.0,
            0.0,
            color,
        ));
    }
    let indices = (0..sides as u16).flat_map(|i| [0, i + 1, i + 2]).collect();
    draw_mesh(&Mesh {
        vertices,
        indices,
        texture: None,
    });
}

pub fn fill_screen(view: &View, color: Color) {
    let f = view.full();
    draw_rectangle(f.x, f.y, f.w, f.h, color);
}

pub fn sky(view: &View, top: Color, bottom: Color) {
    gradient_rect(view.full(), top, bottom);
}

pub fn gradient_rect(r: Rect, top: Color, bottom: Color) {
    use macroquad::models::{draw_mesh, Mesh, Vertex};
    let v = |x: f32, y: f32, c: Color| Vertex::new2(vec3(x, y, 0.0), Vec2::ZERO, c);
    draw_mesh(&Mesh {
        vertices: vec![
            v(r.x, r.y, top),
            v(r.x + r.w, r.y, top),
            v(r.x + r.w, r.y + r.h, bottom),
            v(r.x, r.y + r.h, bottom),
        ],
        indices: vec![0, 1, 2, 0, 2, 3],
        texture: None,
    });
}

const CLOUD_PUFFS: [(f32, f32, f32); 6] = [
    (34.0, 6.0, 20.0),
    (58.0, -8.0, 27.0),
    (88.0, -2.0, 23.0),
    (110.0, 8.0, 16.0),
    (72.0, 12.0, 22.0),
    (48.0, 14.0, 18.0),
];
const CLOUD_W: f32 = 140.0;
const CLOUD_H: f32 = 80.0;
const CLOUD_TOP: f32 = 44.0;

fn smooth_min(a: f32, b: f32, k: f32) -> f32 {
    let h = (0.5 + 0.5 * (b - a) / k).clamp(0.0, 1.0);
    b + (a - b) * h - k * h * (1.0 - h)
}

pub fn cloud_mask(px: f32, py: f32) -> f32 {
    let p = vec2(px, py - CLOUD_TOP);
    let mut d = f32::MAX;
    for (cx, cy, r) in CLOUD_PUFFS {
        d = smooth_min(d.min(1e4), (p - vec2(cx, cy)).length() - r, 9.0);
    }
    let base = vec2((p.x - 72.0) / 46.0, (p.y - 18.0) / 12.0);
    d = smooth_min(d, (base.length() - 1.0) * 12.0, 8.0);
    (0.5 - d).clamp(0.0, 1.0)
}

fn cloud_texture() -> Texture2D {
    thread_local! {
        static TEX: std::cell::OnceCell<Texture2D> = const { std::cell::OnceCell::new() };
    }
    TEX.with(|t| {
        t.get_or_init(|| {
            let k = 4.0;
            let (w, h) = ((CLOUD_W * k) as u16, (CLOUD_H * k) as u16);
            let mut bytes = Vec::with_capacity(w as usize * h as usize * 4);
            for py in 0..h {
                for px in 0..w {
                    let mut a = 0.0;
                    for (ox, oy) in [(0.25, 0.25), (0.75, 0.25), (0.25, 0.75), (0.75, 0.75)] {
                        a += cloud_mask((px as f32 + ox) / k, (py as f32 + oy) / k) * k;
                    }
                    let a = (a / 4.0).clamp(0.0, 1.0);
                    let t = (py as f32 / h as f32 - 0.5).max(0.0) * 2.0;
                    let r = (255.0 - t * 24.0) as u8;
                    let g = (255.0 - t * 18.0) as u8;
                    bytes.extend_from_slice(&[r, g, 255, (a * 255.0) as u8]);
                }
            }
            let tex = Texture2D::from_rgba8(w, h, &bytes);
            tex.set_filter(FilterMode::Linear);
            tex
        })
        .clone()
    })
}

pub fn cloud(x: f32, y: f32, s: f32, color: Color) {
    let w = CLOUD_W * s * 0.85;
    let h = CLOUD_H * s * 0.85;
    draw_texture_ex(
        &cloud_texture(),
        x - 14.0 * s,
        y - CLOUD_TOP * s * 0.85,
        color,
        DrawTextureParams {
            dest_size: Some(vec2(w, h)),
            ..Default::default()
        },
    );
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
    fn clip_rects_land_on_screen_pixels_past_the_cutout() {
        let v = View::fit_safe(
            2400.0,
            1080.0,
            crate::safe::Insets {
                left: 120.0,
                ..Default::default()
            },
        );
        let (x, y, w, h) = clip_px(&v, Rect::new(0.0, 0.0, v.w, v.h), 1.0);
        assert_eq!((x, y), (120, 0));
        assert!((w - 2280).abs() <= 1 && (h - 1080).abs() <= 1);
        let (_, _, w2, _) = clip_px(&v, Rect::new(10.0, 10.0, 100.0, 50.0), 2.0);
        assert_eq!(w2, (100.0 * v.scale * 2.0).ceil() as i32);
    }

    #[test]
    fn safe_area_shrinks_the_layout_and_offsets_touches() {
        let px = crate::safe::Insets {
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
    }

    #[test]
    fn landscape_cutout_on_the_left() {
        let px = crate::safe::Insets {
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

    #[test]
    fn panel_pop_in_does_not_rasterise_new_glyph_sizes() {
        for scale in [1.0, 1.33, 2.625] {
            set_text_scale(scale);
            let mut sizes: Vec<u16> = vec![];
            for frame in 0..60 {
                let s = crate::fx::ease_out_back(frame as f32 / 120.0 / 0.28).max(0.01);
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
    fn cloud_is_one_solid_shape_with_soft_edges() {
        let inside = cloud_mask(70.0, CLOUD_TOP + 6.0);
        assert_eq!(inside, 1.0);
        assert_eq!(cloud_mask(2.0, 2.0), 0.0);
        assert_eq!(cloud_mask(CLOUD_W - 2.0, CLOUD_H - 2.0), 0.0);
        let mut max = 0.0f32;
        for y in 0..(CLOUD_H as i32) {
            for x in 0..(CLOUD_W as i32) {
                max = max.max(cloud_mask(x as f32, y as f32));
            }
        }
        assert!(max <= 1.0);
        for x in 0..(CLOUD_W as i32) {
            assert_eq!(cloud_mask(x as f32, 0.0), 0.0, "top edge clipped at x {x}");
            assert_eq!(
                cloud_mask(x as f32, CLOUD_H - 0.5),
                0.0,
                "bottom edge clipped at x {x}"
            );
        }
        for y in 0..(CLOUD_H as i32) {
            assert_eq!(cloud_mask(0.0, y as f32), 0.0);
            assert_eq!(cloud_mask(CLOUD_W - 0.5, y as f32), 0.0);
        }
    }

    #[test]
    fn big_circles_get_more_sides() {
        assert_eq!(circle_sides(5.0), 20);
        assert!(circle_sides(150.0) >= 80);
        assert_eq!(circle_sides(10_000.0), 96);
    }

    #[test]
    fn short_drag_is_not_a_swipe() {
        assert_eq!(swipe_dir(vec2(10.0, 5.0), SWIPE_DIST), None);
        assert_eq!(swipe_dir(vec2(-50.0, 10.0), SWIPE_DIST), Some(Dir::Left));
        assert_eq!(swipe_dir(vec2(5.0, 60.0), SWIPE_DIST), Some(Dir::Down));
    }

    #[test]
    fn one_drag_can_turn_but_never_repeats_a_direction() {
        let mut t = SwipeTracker::default();
        assert_eq!(t.track(vec2(100.0, 100.0), true, true, 36.0), None);
        assert_eq!(
            t.track(vec2(150.0, 100.0), true, false, 36.0),
            Some(Dir::Right)
        );
        assert_eq!(t.track(vec2(200.0, 100.0), true, false, 36.0), None);
        assert_eq!(t.track(vec2(300.0, 100.0), true, false, 36.0), None);
        assert_eq!(t.track(vec2(300.0, 40.0), true, false, 36.0), Some(Dir::Up));
        assert_eq!(t.track(vec2(300.0, 0.0), false, false, 36.0), None);
        assert!(t.swiped);
    }

    #[test]
    fn a_phone_tap_wobble_is_not_a_swipe() {
        let phone = View::fit_safe(2992.0, 1344.0, crate::safe::Insets::default());
        let mm = (495.0 / 25.4) / phone.scale;
        let mut t = SwipeTracker::default();
        t.track(vec2(500.0, 400.0), true, true, SWIPE_MM * mm);
        let wobble = vec2(500.0 - 3.5 * mm, 400.0 + 1.0 * mm);
        assert_eq!(t.track(wobble, true, false, SWIPE_MM * mm), None);
        t.track(wobble, false, false, SWIPE_MM * mm);
        let input = Input {
            released: true,
            travel: t.travel,
            swiped: t.swiped,
            mm,
            ..Default::default()
        };
        assert!(input.tap_released());
    }

    #[test]
    fn a_diagonal_drag_waits_until_it_picks_an_axis() {
        let mut t = SwipeTracker::default();
        t.track(vec2(0.0, 0.0), true, true, 40.0);
        assert_eq!(t.track(vec2(32.0, -30.0), true, false, 40.0), None);
        assert_eq!(t.track(vec2(36.0, -70.0), true, false, 40.0), Some(Dir::Up));
    }

    #[test]
    fn a_long_drag_is_not_a_tap() {
        let mut t = SwipeTracker::default();
        t.track(vec2(0.0, 0.0), true, true, 40.0);
        t.track(vec2(20.0, 20.0), true, false, 40.0);
        t.track(vec2(0.0, 0.0), false, false, 40.0);
        let input = Input {
            released: true,
            travel: t.travel,
            mm: 5.0,
            ..Default::default()
        };
        assert!(!input.tap_released());
    }

    #[test]
    fn a_second_finger_cannot_move_or_release_the_first() {
        let mut f = PrimaryFinger::default();
        let a = vec2(100.0, 100.0);
        let b = vec2(900.0, 700.0);
        let p = f.update(&[(1, Finger::Started, a)]);
        assert!(p.pressed && p.down && p.pos == a);
        let p = f.update(&[(1, Finger::Held, a), (2, Finger::Started, b)]);
        assert!(!p.pressed && p.down && p.pos == a);
        let p = f.update(&[(1, Finger::Held, a), (2, Finger::Ended, b)]);
        assert!(!p.released && p.down && p.pos == a);
        let p = f.update(&[(1, Finger::Ended, a)]);
        assert!(p.released && !p.down && p.pos == a);
    }

    #[test]
    fn a_finger_left_behind_is_not_picked_up_again() {
        let mut f = PrimaryFinger::default();
        let (a, b) = (vec2(10.0, 10.0), vec2(500.0, 500.0));
        f.update(&[(1, Finger::Started, a)]);
        f.update(&[(1, Finger::Held, a), (2, Finger::Started, b)]);
        let p = f.update(&[(1, Finger::Ended, a), (2, Finger::Held, b)]);
        assert!(p.released);
        let p = f.update(&[(2, Finger::Held, b)]);
        assert!(!p.pressed && !p.down);
    }

    #[test]
    fn a_new_tap_as_the_old_finger_lifts_is_kept_for_the_next_frame() {
        let mut f = PrimaryFinger::default();
        let (a, b) = (vec2(10.0, 10.0), vec2(500.0, 500.0));
        f.update(&[(1, Finger::Started, a)]);
        let p = f.update(&[(1, Finger::Ended, a), (2, Finger::Started, b)]);
        assert!(p.released && p.pos == a);
        let p = f.update(&[(2, Finger::Held, b)]);
        assert!(p.pressed && p.down && p.pos == b);
    }

    #[test]
    fn a_tap_inside_one_frame_presses_and_releases() {
        let mut f = PrimaryFinger::default();
        let a = vec2(42.0, 24.0);
        let p = f.update(&[(7, Finger::Ended, a)]);
        assert!(p.pressed && p.released && p.pos == a);
        let p = f.update(&[]);
        assert!(!p.pressed && !p.released && !p.down);
    }

    #[test]
    fn density_guess_is_sane() {
        assert!((guess_units_per_mm(412.0, 915.0) - 160.0 / 25.4).abs() < 0.01);
        assert!((guess_units_per_mm(1024.0, 768.0) - 132.0 / 25.4).abs() < 0.01);
        assert!((guess_units_per_mm(1920.0, 1080.0) - 96.0 / 25.4).abs() < 0.01);
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
