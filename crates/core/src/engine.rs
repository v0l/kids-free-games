use crate::geom::Rect;
use crate::safe::Insets;
use glam::{ivec2, vec2, IVec2, Vec2};

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
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct View {
    pub w: f32,
    pub h: f32,
    pub scale: f32,
    pub inset: Insets,
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

    pub fn to_screen(&self, p: Vec2) -> Vec2 {
        (p + vec2(self.inset.left, self.inset.top)) * self.scale
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
    pub up: bool,
    pub down_key: bool,
    pub action: bool,
    pub back: bool,
    pub start: Vec2,
    pub travel: f32,
    pub swiped: bool,
    pub mm: f32,
    pub typed: Vec<char>,
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

    pub fn active(&self) -> bool {
        self.down
            || self.pressed
            || self.released
            || self.swipe.is_some()
            || self.key_dir.is_some()
            || self.left
            || self.right
            || self.action
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Key {
    Up,
    Down,
    Left,
    Right,
    W,
    A,
    S,
    D,
    Space,
    Enter,
    Escape,
    Backspace,
}

#[derive(Clone, Debug, Default)]
pub struct Raw {
    pub touches: Vec<(u64, Finger, Vec2)>,
    pub mouse: Pointer,
    pub keys_down: Vec<Key>,
    pub keys_pressed: Vec<Key>,
    pub typed: Vec<char>,
    pub units_per_mm: Option<f32>,
    pub screen: Vec2,
}

impl Raw {
    pub fn down(&self, k: Key) -> bool {
        self.keys_down.contains(&k)
    }

    pub fn pressed(&self, k: Key) -> bool {
        self.keys_pressed.contains(&k)
    }
}

#[derive(Default)]
pub struct InputTracker {
    swipes: SwipeTracker,
    last: Vec2,
    fingers: PrimaryFinger,
}

impl InputTracker {
    fn pointer(&mut self, raw: &Raw) -> Pointer {
        if !raw.touches.is_empty() || self.fingers.active() {
            return self.fingers.update(&raw.touches);
        }
        raw.mouse
    }

    pub fn poll(&mut self, raw: &Raw, view: &View) -> Input {
        let per_mm = raw
            .units_per_mm
            .unwrap_or_else(|| guess_units_per_mm(raw.screen.x, raw.screen.y));
        let mm = per_mm / view.scale;
        let Pointer {
            pos,
            down,
            pressed,
            released,
        } = self.pointer(raw);
        let pos = view.to_view(pos);
        let prev_pos = if pressed { pos } else { self.last };
        self.last = pos;
        let swipe = self.swipes.track(pos, down, pressed, SWIPE_MM * mm);
        let key_dir = [
            (Key::Up, Dir::Up),
            (Key::W, Dir::Up),
            (Key::Down, Dir::Down),
            (Key::S, Dir::Down),
            (Key::Left, Dir::Left),
            (Key::A, Dir::Left),
            (Key::Right, Dir::Right),
            (Key::D, Dir::Right),
        ]
        .into_iter()
        .find(|(k, _)| raw.pressed(*k))
        .map(|(_, d)| d);
        Input {
            pos,
            prev_pos,
            down,
            pressed,
            released,
            swipe,
            key_dir,
            left: raw.down(Key::Left) || raw.down(Key::A),
            right: raw.down(Key::Right) || raw.down(Key::D),
            up: raw.down(Key::Up) || raw.down(Key::W),
            down_key: raw.down(Key::Down) || raw.down(Key::S),
            action: raw.pressed(Key::Space) || raw.pressed(Key::Enter),
            back: raw.pressed(Key::Escape) || raw.pressed(Key::Backspace),
            start: self.swipes.start,
            travel: self.swipes.travel,
            swiped: self.swipes.swiped,
            mm,
            typed: raw.typed.clone(),
        }
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
        let phone = View::fit_safe(2992.0, 1344.0, Insets::default());
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
    fn opposite_dirs() {
        for d in Dir::ALL {
            assert_eq!(d.opposite().opposite(), d);
            assert_eq!(d.delta() + d.opposite().delta(), IVec2::ZERO);
        }
    }

    #[test]
    fn the_tracker_follows_the_first_finger_and_scales_to_view_units() {
        let v = View::fit_safe(
            2000.0,
            1000.0,
            Insets {
                left: 100.0,
                ..Default::default()
            },
        );
        let mut t = InputTracker::default();
        let raw = Raw {
            touches: vec![(3, Finger::Started, vec2(100.0, 0.0))],
            units_per_mm: Some(20.0),
            screen: vec2(2000.0, 1000.0),
            ..Default::default()
        };
        let i = t.poll(&raw, &v);
        assert!(i.pressed && i.down);
        assert!(i.pos.length() < 0.01);
        assert!((i.mm - 20.0 / v.scale).abs() < 1e-4);
        let raw = Raw {
            touches: vec![
                (3, Finger::Held, vec2(100.0 + 20.0 * 7.0, 0.0)),
                (4, Finger::Started, vec2(1500.0, 500.0)),
            ],
            units_per_mm: Some(20.0),
            screen: vec2(2000.0, 1000.0),
            ..Default::default()
        };
        let i = t.poll(&raw, &v);
        assert_eq!(i.swipe, Some(Dir::Right));
        assert!(!i.pressed);
    }

    #[test]
    fn keys_map_to_directions_and_actions() {
        let v = View::ipad_landscape();
        let mut t = InputTracker::default();
        let raw = Raw {
            keys_pressed: vec![Key::A, Key::Enter],
            keys_down: vec![Key::A],
            screen: vec2(1024.0, 768.0),
            ..Default::default()
        };
        let i = t.poll(&raw, &v);
        assert_eq!(i.key_dir, Some(Dir::Left));
        assert!(i.left && i.action && !i.back);
    }
}
