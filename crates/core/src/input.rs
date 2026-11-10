use crate::engine::{Game, View};
use crate::geom::Rect;
use glam::{ivec2, vec2, IVec2, Vec2};

pub const SWIPE_DIST: f32 = 36.0;
pub const SWIPE_MM: f32 = 6.0;
pub const TAP_SLOP_MM: f32 = 4.0;
pub const FALLBACK_MM: f32 = 6.0;
const AXIS_RATIO: f32 = 1.3;

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
    pub axis: Vec2,
    pub action: bool,
    pub back: bool,
    pub start: Vec2,
    pub travel: f32,
    pub swiped: bool,
    pub mm: f32,
    pub typed: Vec<char>,
    pub hold: f32,
    pub cancelled: bool,
    pub scroll: f32,
    pub pinch: f32,
    pub pointers: Vec<Touch>,
    pub path: Vec<Vec2>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Touch {
    pub id: u64,
    pub pos: Vec2,
    pub screen: Vec2,
    pub primary: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InputKind {
    Tap,
    Drag,
    Hold,
    Swipe,
    Aim,
    Stick,
    Key,
    Scroll,
    Pinch,
}

pub const HOLD_SECS: f32 = 0.45;
pub const PAD_DEAD: f32 = 0.2;
pub const STICK_MM: f32 = 9.0;
pub const STICK_DEAD: f32 = 0.12;
pub const AXIS_ON: f32 = 0.5;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Direction {
    pub vec: Vec2,
}

impl Direction {
    pub const NONE: Direction = Direction { vec: Vec2::ZERO };

    pub fn new(vec: Vec2) -> Self {
        let len = vec.length();
        Self {
            vec: if len > 1.0 { vec / len } else { vec },
        }
    }

    pub fn is_zero(&self) -> bool {
        self.vec == Vec2::ZERO
    }

    pub fn opt(&self) -> Option<Vec2> {
        (!self.is_zero()).then_some(self.vec)
    }

    pub fn strength(&self) -> f32 {
        self.vec.length()
    }

    pub fn unit(&self) -> Option<Vec2> {
        (!self.is_zero()).then(|| self.vec.normalize())
    }

    pub fn angle(&self) -> Option<f32> {
        (!self.is_zero()).then(|| self.vec.y.atan2(self.vec.x))
    }

    pub fn four(&self) -> Option<Dir> {
        (!self.is_zero()).then(|| Dir::from_vec(self.vec))
    }

    pub fn eight(&self) -> Option<IVec2> {
        if self.is_zero() {
            return None;
        }
        let a = self.vec.y.atan2(self.vec.x);
        let step = (a / std::f32::consts::FRAC_PI_4).round() as i32;
        let (s, c) = (
            (step as f32 * std::f32::consts::FRAC_PI_4).sin(),
            (step as f32 * std::f32::consts::FRAC_PI_4).cos(),
        );
        Some(ivec2(c.round() as i32, s.round() as i32))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DirectionMode {
    Stick,
    Swipe,
    Keys,
}

#[derive(Clone, Copy, Debug)]
pub struct DirControl {
    pub mode: DirectionMode,
    pub origin: Vec2,
    pub knob: Vec2,
    pub active: bool,
    pub radius: f32,
    pub smoothing: f32,
    smooth: Vec2,
}

impl DirControl {
    pub fn new(mode: DirectionMode) -> Self {
        Self {
            mode,
            origin: Vec2::ZERO,
            knob: Vec2::ZERO,
            active: false,
            radius: 0.0,
            smoothing: 0.0,
            smooth: Vec2::ZERO,
        }
    }

    pub fn smoothed(mut self, rate: f32) -> Self {
        self.smoothing = rate;
        self
    }

    pub fn reset(&mut self) {
        self.active = false;
        self.knob = Vec2::ZERO;
        self.smooth = Vec2::ZERO;
    }

    pub fn radius(input: &Input) -> f32 {
        STICK_MM * input.mm()
    }

    pub fn read(&mut self, input: &Input, dt: f32) -> Direction {
        let raw = self.raw(input);
        if self.smoothing <= 0.0 {
            return raw;
        }
        let k = 1.0 - (-self.smoothing * dt).exp();
        self.smooth = self.smooth.lerp(raw.vec, k);
        if raw.is_zero() && self.smooth.length() < 0.05 {
            self.smooth = Vec2::ZERO;
        }
        Direction::new(self.smooth)
    }

    fn raw(&mut self, input: &Input) -> Direction {
        if input.axis != Vec2::ZERO {
            self.active = false;
            return Direction::new(input.axis);
        }
        match self.mode {
            DirectionMode::Keys => Direction::NONE,
            DirectionMode::Swipe => input
                .swipe
                .map_or(Direction::NONE, |d| Direction::new(d.delta().as_vec2())),
            DirectionMode::Stick => self.stick(input),
        }
    }

    fn stick(&mut self, input: &Input) -> Direction {
        let radius = Self::radius(input);
        self.radius = radius;
        if input.pressed {
            self.origin = input.pos;
            self.active = true;
        }
        if !input.down || input.cancelled {
            self.active = false;
            self.knob = Vec2::ZERO;
            return Direction::NONE;
        }
        if !self.active {
            self.origin = input.pos;
            self.active = true;
        }
        let d = input.pos - self.origin;
        let len = d.length();
        if len > radius {
            self.origin += d / len * (len - radius);
        }
        self.knob = input.pos - self.origin;
        if self.knob.length() > radius * STICK_DEAD {
            Direction::new(self.knob / radius)
        } else {
            Direction::NONE
        }
    }
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

    pub fn held(axis: Vec2) -> Self {
        Self {
            axis,
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
            || self.axis != Vec2::ZERO
            || self.action
    }

    pub fn direction(&self) -> Direction {
        Direction::new(self.axis)
    }

    pub fn drag_delta(&self) -> Vec2 {
        if self.down && !self.pressed {
            self.pos - self.prev_pos
        } else {
            Vec2::ZERO
        }
    }

    pub fn dragging(&self) -> bool {
        self.down && self.travel >= TAP_SLOP_MM * self.mm()
    }

    pub fn holding(&self) -> bool {
        self.down && self.hold >= HOLD_SECS && !self.dragging()
    }

    pub fn kinds(&self) -> Vec<InputKind> {
        let mut k = Vec::new();
        if self.pressed || self.action {
            k.push(InputKind::Tap);
        }
        if self.dragging() {
            k.push(InputKind::Drag);
        }
        if self.holding() {
            k.push(InputKind::Hold);
        }
        if self.swipe.is_some() {
            k.push(InputKind::Swipe);
        }
        if self.axis != Vec2::ZERO {
            k.push(InputKind::Stick);
        }
        if self.key_dir.is_some() || self.action || self.back {
            k.push(InputKind::Key);
        }
        if self.scroll != 0.0 {
            k.push(InputKind::Scroll);
        }
        if self.pinch != 0.0 {
            k.push(InputKind::Pinch);
        }
        k
    }

    pub fn masked(&self) -> Input {
        Input {
            mm: self.mm,
            typed: self.typed.clone(),
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
    held: Vec<(u64, Vec2)>,
    used: Option<u64>,
    path: Vec<Vec2>,
}

impl PrimaryFinger {
    pub fn update(&mut self, events: &[(u64, Finger, Vec2)]) -> Pointer {
        let touches = self.with_held(events);
        self.used = None;
        let out = self.step(&touches);
        self.path = match self.used {
            Some(id) => events.iter().filter(|e| e.0 == id).map(|e| e.2).collect(),
            None => Vec::new(),
        };
        out
    }

    pub fn path(&self) -> &[Vec2] {
        &self.path
    }

    pub fn clear_path(&mut self) {
        self.path.clear();
    }

    fn with_held(&mut self, events: &[(u64, Finger, Vec2)]) -> Vec<(u64, Finger, Vec2)> {
        let mut out: Vec<(u64, Finger, Vec2)> = self
            .held
            .iter()
            .map(|(id, p)| (*id, Finger::Held, *p))
            .collect();
        for &(id, f, p) in events {
            match out.iter_mut().find(|x| x.0 == id) {
                Some(slot) => {
                    if !(slot.1 == Finger::Started && f == Finger::Held) {
                        slot.1 = f;
                    }
                    slot.2 = p;
                }
                None => out.push((id, f, p)),
            }
        }
        self.held = out
            .iter()
            .filter(|t| t.1 != Finger::Ended)
            .map(|t| (t.0, t.2))
            .collect();
        out
    }

    fn step(&mut self, touches: &[(u64, Finger, Vec2)]) -> Pointer {
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
            self.used = Some(id);
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
                self.used = Some(id);
                out = Pointer {
                    pos: p,
                    down: true,
                    pressed: true,
                    released: false,
                };
            } else if let Some((tap_id, _, p)) = tap {
                self.used = Some(tap_id);
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
        self.id.is_some() || self.pending.is_some() || !self.held.is_empty()
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
    NumUpLeft,
    NumUpRight,
    NumDownLeft,
    NumDownRight,
    Space,
    Enter,
    Escape,
    Backspace,
}

impl Key {
    pub fn axis(self) -> Option<Vec2> {
        Some(match self {
            Key::Up | Key::W => vec2(0.0, -1.0),
            Key::Down | Key::S => vec2(0.0, 1.0),
            Key::Left | Key::A => vec2(-1.0, 0.0),
            Key::Right | Key::D => vec2(1.0, 0.0),
            Key::NumUpLeft => vec2(-1.0, -1.0),
            Key::NumUpRight => vec2(1.0, -1.0),
            Key::NumDownLeft => vec2(-1.0, 1.0),
            Key::NumDownRight => vec2(1.0, 1.0),
            _ => return None,
        })
    }
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
    pub pad: Vec2,
    pub cancel: bool,
    pub scroll: f32,
    pub pinch: f32,
    pub dt: f32,
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
    hold: f32,
    was_down: bool,
}

pub fn axis_of(raw: &Raw) -> Vec2 {
    let keys = raw
        .keys_down
        .iter()
        .filter_map(|k| k.axis())
        .fold(Vec2::ZERO, |a, b| a + b);
    let keys = keys.clamp(Vec2::splat(-1.0), Vec2::splat(1.0));
    if keys != Vec2::ZERO {
        return Direction::new(keys).vec;
    }
    let pad = vec2(raw.pad.x.clamp(-1.0, 1.0), raw.pad.y.clamp(-1.0, 1.0));
    if pad.length() < PAD_DEAD {
        return Vec2::ZERO;
    }
    Direction::new(pad).vec
}

impl InputTracker {
    fn pointer(&mut self, raw: &Raw) -> Pointer {
        if !raw.touches.is_empty() || self.fingers.active() {
            return self.fingers.update(&raw.touches);
        }
        self.fingers.clear_path();
        raw.mouse
    }

    fn cancel(&mut self) -> Pointer {
        let was = self.was_down;
        self.fingers = PrimaryFinger::default();
        Pointer {
            pos: self.last,
            down: false,
            pressed: false,
            released: was,
        }
    }

    pub fn poll(&mut self, raw: &Raw, view: &View) -> Input {
        let per_mm = raw
            .units_per_mm
            .unwrap_or_else(|| guess_units_per_mm(raw.screen.x, raw.screen.y));
        let mm = per_mm / view.scale;
        let cancelled = raw.cancel && self.was_down;
        let ptr = if raw.cancel {
            self.cancel()
        } else {
            self.pointer(raw)
        };
        let Pointer {
            pos,
            down,
            pressed,
            released,
        } = ptr;
        let pos = if raw.cancel && !pressed {
            self.last
        } else {
            view.to_view(pos)
        };
        let prev_pos = if pressed { pos } else { self.last };
        self.last = pos;
        if pressed {
            self.hold = 0.0;
        }
        if down {
            self.hold += raw.dt;
        } else if !released {
            self.hold = 0.0;
        }
        self.was_down = down;
        let swipe = if cancelled {
            self.swipes.track(pos, false, false, SWIPE_MM * mm);
            None
        } else {
            let mut path: Vec<Vec2> = if raw.touches.is_empty() && !self.fingers.active() {
                Vec::new()
            } else {
                self.fingers
                    .path()
                    .iter()
                    .map(|p| view.to_view(*p))
                    .collect()
            };
            if path.is_empty() && down {
                path.push(pos);
            }
            let mut found = None;
            for (i, p) in path.iter().enumerate() {
                let first = i == 0 && pressed;
                found = found.or(self.swipes.track(*p, true, first, SWIPE_MM * mm));
            }
            if !down {
                self.swipes.track(pos, false, false, SWIPE_MM * mm);
            }
            found
        };
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
        let pointers = if raw.touches.is_empty() {
            vec![Touch {
                id: 0,
                pos,
                screen: view.to_screen(pos),
                primary: true,
            }]
            .into_iter()
            .filter(|_| down)
            .collect()
        } else {
            raw.touches
                .iter()
                .filter(|t| t.1 != Finger::Ended)
                .enumerate()
                .map(|(i, t)| Touch {
                    id: t.0,
                    pos: view.to_view(t.2),
                    screen: t.2,
                    primary: i == 0,
                })
                .collect()
        };
        Input {
            pos,
            prev_pos,
            down,
            pressed,
            released,
            swipe,
            key_dir,
            axis: axis_of(raw),
            action: raw.pressed(Key::Space) || raw.pressed(Key::Enter),
            back: raw.pressed(Key::Escape) || raw.pressed(Key::Backspace),
            start: self.swipes.start,
            travel: self.swipes.travel,
            swiped: self.swipes.swiped || cancelled,
            mm,
            typed: raw.typed.clone(),
            hold: self.hold,
            cancelled,
            scroll: raw.scroll,
            pinch: raw.pinch,
            pointers,
            path: if cancelled {
                Vec::new()
            } else {
                self.fingers
                    .path()
                    .iter()
                    .map(|p| view.to_view(*p))
                    .collect()
            },
        }
    }
}

pub struct Script {
    view: View,
    tracker: InputTracker,
    mm: f32,
    pos: Vec2,
    down: bool,
    dt: f32,
}

impl Script {
    pub fn new(view: View) -> Self {
        Self {
            view,
            tracker: InputTracker::default(),
            mm: 6.0,
            pos: Vec2::ZERO,
            down: false,
            dt: 1.0 / 60.0,
        }
    }

    pub fn mm(mut self, units_per_mm_px: f32) -> Self {
        self.mm = units_per_mm_px;
        self
    }

    fn raw(&self) -> Raw {
        Raw {
            units_per_mm: Some(self.mm),
            screen: vec2(self.view.w, self.view.h) * self.view.scale,
            dt: self.dt,
            ..Default::default()
        }
    }

    fn pointer(&mut self, raw: Raw, pos: Vec2, down: bool, pressed: bool, released: bool) -> Input {
        self.pos = pos;
        self.down = down;
        let raw = Raw {
            mouse: Pointer {
                pos: self.view.to_screen(pos),
                down,
                pressed,
                released,
            },
            ..raw
        };
        self.tracker.poll(&raw, &self.view)
    }

    pub fn idle(&mut self) -> Input {
        let raw = self.raw();
        let (pos, down) = (self.pos, self.down);
        self.pointer(raw, pos, down, false, false)
    }

    pub fn press(&mut self, at: Vec2) -> Input {
        let raw = self.raw();
        self.pointer(raw, at, true, true, false)
    }

    pub fn drag_to(&mut self, at: Vec2) -> Input {
        let raw = self.raw();
        self.pointer(raw, at, true, false, false)
    }

    pub fn release(&mut self) -> Input {
        let raw = self.raw();
        let pos = self.pos;
        self.pointer(raw, pos, false, false, true)
    }

    pub fn cancel(&mut self) -> Input {
        let mut raw = self.raw();
        raw.cancel = true;
        let pos = self.pos;
        self.down = false;
        self.pointer(raw, pos, false, false, false)
    }

    pub fn tap(&mut self, at: Vec2) -> Vec<Input> {
        vec![self.press(at), self.release()]
    }

    pub fn swipe(&mut self, from: Vec2, dir: Dir, steps: usize) -> Vec<Input> {
        let step = dir.delta().as_vec2() * SWIPE_MM * self.mm / self.view.scale;
        let mut out = vec![self.press(from)];
        for i in 1..=steps {
            out.push(self.drag_to(from + step * i as f32));
        }
        out.push(self.release());
        out
    }

    pub fn hold(&mut self, at: Vec2, secs: f32) -> Vec<Input> {
        let n = (secs / self.dt).ceil() as usize;
        let mut out = vec![self.press(at)];
        for _ in 0..n {
            out.push(self.drag_to(at));
        }
        out
    }

    pub fn keys(&mut self, keys: &[Key], frames: usize) -> Vec<Input> {
        (0..frames)
            .map(|i| {
                let mut raw = self.raw();
                raw.keys_down = keys.to_vec();
                if i == 0 {
                    raw.keys_pressed = keys.to_vec();
                }
                let (pos, down) = (self.pos, self.down);
                self.pointer(raw, pos, down, false, false)
            })
            .collect()
    }

    pub fn pad(&mut self, stick: Vec2, frames: usize) -> Vec<Input> {
        (0..frames)
            .map(|_| {
                let mut raw = self.raw();
                raw.pad = stick;
                let (pos, down) = (self.pos, self.down);
                self.pointer(raw, pos, down, false, false)
            })
            .collect()
    }

    pub fn feed<G: Game>(game: &mut G, view: &View, frames: &[Input]) {
        for f in frames {
            game.update(f, view, 1.0 / 60.0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::View;
    use crate::safe::Insets;

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
    fn a_finger_held_still_between_events_is_one_press() {
        let mut t = InputTracker::default();
        let v = View::ipad_landscape();
        let a = vec2(200.0, 300.0);
        let frame = |touches: Vec<(u64, Finger, Vec2)>| Raw {
            touches,
            screen: vec2(v.w, v.h),
            ..Default::default()
        };
        let mut presses = 0;
        let mut releases = 0;
        let script: Vec<Vec<(u64, Finger, Vec2)>> = vec![
            vec![(7, Finger::Started, a)],
            vec![],
            vec![],
            vec![(7, Finger::Held, a + vec2(1.0, 0.0))],
            vec![],
            vec![(7, Finger::Held, a + vec2(1.0, 1.0))],
            vec![],
            vec![],
            vec![(7, Finger::Ended, a + vec2(1.0, 1.0))],
            vec![],
        ];
        for touches in script {
            let i = t.poll(&frame(touches), &v);
            presses += i.pressed as u32;
            releases += i.released as u32;
        }
        assert_eq!((presses, releases), (1, 1));
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
        assert!(i.axis == vec2(-1.0, 0.0) && i.action && !i.back);
    }

    fn script() -> (Script, View) {
        let v = View::ipad_landscape();
        (Script::new(v), v)
    }

    #[test]
    fn eight_way_quantising_picks_the_nearest_octant() {
        let d = |x: f32, y: f32| Direction::new(vec2(x, y)).eight();
        assert_eq!(d(1.0, 0.1), Some(ivec2(1, 0)));
        assert_eq!(d(1.0, 0.9), Some(ivec2(1, 1)));
        assert_eq!(d(-0.2, -1.0), Some(ivec2(0, -1)));
        assert_eq!(d(-1.0, -1.0), Some(ivec2(-1, -1)));
        assert_eq!(d(0.0, 0.0), None);
    }

    #[test]
    fn four_way_quantising_matches_dir() {
        assert_eq!(Direction::new(vec2(0.2, 0.9)).four(), Some(Dir::Down));
        assert_eq!(Direction::new(vec2(-0.9, 0.3)).four(), Some(Dir::Left));
        assert_eq!(Direction::NONE.four(), None);
    }

    #[test]
    fn a_direction_never_exceeds_unit_length() {
        assert!((Direction::new(vec2(3.0, 4.0)).strength() - 1.0).abs() < 1e-5);
        assert!((Direction::new(vec2(0.3, 0.0)).strength() - 0.3).abs() < 1e-5);
    }

    #[test]
    fn stick_dead_zone_ignores_small_movement() {
        let (mut s, _) = script();
        let mut c = DirControl::new(DirectionMode::Stick);
        let i = s.press(vec2(300.0, 300.0));
        assert!(c.read(&i, 0.016).is_zero());
        let r = DirControl::radius(&i);
        let i = s.drag_to(vec2(300.0 + r * STICK_DEAD * 0.5, 300.0));
        assert!(c.read(&i, 0.016).is_zero());
        let i = s.drag_to(vec2(300.0 + r * 0.5, 300.0));
        let d = c.read(&i, 0.016);
        assert!((d.vec.x - 0.5).abs() < 0.01 && d.vec.y == 0.0);
    }

    #[test]
    fn the_stick_angle_is_the_drag_angle_from_where_it_landed() {
        let (mut s, _) = script();
        let mut c = DirControl::new(DirectionMode::Stick);
        let i = s.press(vec2(700.0, 200.0));
        c.read(&i, 0.016);
        let r = DirControl::radius(&i);
        for (to, deg) in [
            (vec2(1.0, 0.0), 0.0),
            (vec2(0.0, 1.0), 90.0),
            (vec2(-1.0, 0.0), 180.0),
            (vec2(0.0, -1.0), -90.0),
            (vec2(1.0, 1.0), 45.0),
        ] {
            let i = s.drag_to(vec2(700.0, 200.0) + to.normalize() * r * 0.9);
            let a = c.read(&i, 0.016).angle().unwrap().to_degrees();
            assert!((a - deg).abs() < 1.0, "{to:?} {a}");
        }
    }

    #[test]
    fn releasing_the_stick_returns_to_neutral() {
        let (mut s, _) = script();
        let mut c = DirControl::new(DirectionMode::Stick);
        let i = s.press(vec2(300.0, 300.0));
        c.read(&i, 0.016);
        let r = DirControl::radius(&i);
        let i = s.drag_to(vec2(300.0 + r, 300.0));
        assert!(!c.read(&i, 0.016).is_zero() && c.active);
        let i = s.release();
        assert!(c.read(&i, 0.016).is_zero());
        assert!(!c.active && c.knob == Vec2::ZERO);
        let i = s.idle();
        assert!(c.read(&i, 0.016).is_zero());
    }

    #[test]
    fn the_stick_lands_where_the_finger_lands() {
        let (mut s, _) = script();
        let mut c = DirControl::new(DirectionMode::Stick);
        let at = vec2(812.0, 95.0);
        let i = s.press(at);
        c.read(&i, 0.016);
        assert!(c.active && c.origin == at && c.radius > 0.0);
    }

    #[test]
    fn the_stick_origin_follows_a_long_drag() {
        let (mut s, _) = script();
        let mut c = DirControl::new(DirectionMode::Stick);
        let i = s.press(vec2(300.0, 300.0));
        c.read(&i, 0.016);
        let r = DirControl::radius(&i);
        let i = s.drag_to(vec2(300.0 + r * 4.0, 300.0));
        let d = c.read(&i, 0.016);
        assert!((d.strength() - 1.0).abs() < 1e-4);
        let i = s.drag_to(vec2(300.0 + r * 3.0, 300.0));
        let d = c.read(&i, 0.016);
        assert!(d.is_zero());
        let i = s.drag_to(vec2(300.0 + r * 2.0, 300.0));
        assert!(c.read(&i, 0.016).vec.x < 0.0);
    }

    #[test]
    fn keys_beat_the_stick_and_wasd_equals_arrows() {
        let (mut s, _) = script();
        let w = s.keys(&[Key::W], 1)[0].axis;
        let up = s.keys(&[Key::Up], 1)[0].axis;
        assert_eq!(w, up);
        assert_eq!(w, vec2(0.0, -1.0));
        let both = s.keys(&[Key::Up, Key::Right], 1)[0].clone();
        assert!((both.axis.length() - 1.0).abs() < 1e-5);
        let mut c = DirControl::new(DirectionMode::Stick);
        let d = c.read(&both, 0.016);
        assert_eq!(d.vec, both.axis);
    }

    #[test]
    fn opposite_keys_cancel_and_numpad_diagonals_are_unit() {
        let (mut s, _) = script();
        assert_eq!(s.keys(&[Key::Left, Key::Right], 1)[0].axis, Vec2::ZERO);
        let d = s.keys(&[Key::NumUpLeft], 1)[0].axis;
        assert!((d.length() - 1.0).abs() < 1e-5 && d.x < 0.0 && d.y < 0.0);
    }

    #[test]
    fn a_gamepad_stick_has_a_dead_zone() {
        let (mut s, _) = script();
        assert_eq!(s.pad(vec2(0.1, 0.1), 1)[0].axis, Vec2::ZERO);
        let a = s.pad(vec2(1.0, 0.0), 1)[0].axis;
        assert_eq!(a, vec2(1.0, 0.0));
    }

    #[test]
    fn swipe_mode_reports_one_direction_per_swipe() {
        let (mut s, _) = script();
        let mut c = DirControl::new(DirectionMode::Swipe);
        let frames = s.swipe(vec2(300.0, 300.0), Dir::Left, 4);
        let hits: Vec<_> = frames
            .iter()
            .map(|f| c.read(f, 0.016))
            .filter(|d| !d.is_zero())
            .collect();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].four(), Some(Dir::Left));
    }

    #[test]
    fn smoothing_eases_towards_the_target_and_settles_at_zero() {
        let (mut s, _) = script();
        let mut c = DirControl::new(DirectionMode::Keys).smoothed(10.0);
        let held = s.keys(&[Key::Right], 1)[0].clone();
        let d1 = c.read(&held, 0.016);
        assert!(d1.vec.x > 0.0 && d1.vec.x < 0.5);
        for _ in 0..120 {
            c.read(&held, 0.016);
        }
        assert!(c.read(&held, 0.016).vec.x > 0.99);
        for _ in 0..240 {
            c.read(&Input::default(), 0.016);
        }
        assert!(c.read(&Input::default(), 0.016).is_zero());
    }

    #[test]
    fn swipe_threshold_is_in_millimetres() {
        let (mut s, _) = script();
        let mm = s.mm / View::ipad_landscape().scale;
        let short = s.swipe(vec2(300.0, 300.0), Dir::Right, 0);
        assert!(short.iter().all(|f| f.swipe.is_none()));
        let (mut s2, _) = script();
        s2.press(vec2(300.0, 300.0));
        let almost = s2.drag_to(vec2(300.0 + SWIPE_MM * mm * 0.8, 300.0));
        assert_eq!(almost.swipe, None);
        let enough = s2.drag_to(vec2(300.0 + SWIPE_MM * mm * 1.1, 300.0));
        assert_eq!(enough.swipe, Some(Dir::Right));
    }

    #[test]
    fn mm_scales_to_view_units_by_the_view_scale() {
        let v = View::fit(2048.0, 1536.0);
        let mut t = InputTracker::default();
        let raw = Raw {
            units_per_mm: Some(12.0),
            screen: vec2(2048.0, 1536.0),
            ..Default::default()
        };
        let i = t.poll(&raw, &v);
        assert!((i.mm() - 12.0 / v.scale).abs() < 1e-4);
        assert_eq!(Input::default().mm(), FALLBACK_MM);
    }

    #[test]
    fn hold_time_counts_while_still_and_resets_on_release() {
        let (mut s, _) = script();
        let frames = s.hold(vec2(300.0, 300.0), 0.6);
        assert!(!frames[10].holding());
        assert!(frames.last().unwrap().holding());
        assert!(frames.last().unwrap().kinds().contains(&InputKind::Hold));
        s.release();
        assert_eq!(s.idle().hold, 0.0);
    }

    #[test]
    fn a_moving_finger_is_a_drag_not_a_hold() {
        let (mut s, _) = script();
        s.hold(vec2(300.0, 300.0), 0.6);
        let i = s.drag_to(vec2(500.0, 300.0));
        assert!(i.dragging() && !i.holding());
        assert!(i.kinds().contains(&InputKind::Drag));
        assert!(i.drag_delta().x > 100.0);
    }

    #[test]
    fn cancel_releases_without_a_tap_or_swipe() {
        let (mut s, _) = script();
        s.press(vec2(300.0, 300.0));
        s.drag_to(vec2(320.0, 300.0));
        let i = s.cancel();
        assert!(i.cancelled && i.released && !i.down && i.swipe.is_none());
        assert!(!i.tap_released());
        let mut c = DirControl::new(DirectionMode::Stick);
        assert!(c.read(&i, 0.016).is_zero());
        let after = s.idle();
        assert!(!after.down && !after.cancelled);
    }

    #[test]
    fn cancel_with_nothing_held_is_a_no_op() {
        let (mut s, _) = script();
        let i = s.cancel();
        assert!(!i.cancelled && !i.released);
    }

    #[test]
    fn cancelled_touches_drop_the_owner_and_the_next_finger_starts_clean() {
        let v = View::ipad_landscape();
        let mut t = InputTracker::default();
        let base = Raw {
            screen: vec2(1024.0, 768.0),
            ..Default::default()
        };
        t.poll(
            &Raw {
                touches: vec![(1, Finger::Started, vec2(100.0, 100.0))],
                ..base.clone()
            },
            &v,
        );
        let i = t.poll(
            &Raw {
                cancel: true,
                ..base.clone()
            },
            &v,
        );
        assert!(i.cancelled && !i.down);
        let i = t.poll(
            &Raw {
                touches: vec![(2, Finger::Started, vec2(400.0, 400.0))],
                ..base
            },
            &v,
        );
        assert!(i.pressed && i.down);
    }

    #[test]
    fn mouse_and_touch_produce_the_same_press_and_swipe() {
        let v = View::ipad_landscape();
        let at = vec2(500.0, 400.0);
        let mut mouse = Script::new(v);
        let m = mouse.swipe(at, Dir::Up, 3);
        let mut t = InputTracker::default();
        let mut touch = Vec::new();
        let step = Dir::Up.delta().as_vec2() * SWIPE_MM * 6.0;
        let base = Raw {
            units_per_mm: Some(6.0),
            screen: vec2(v.w, v.h) * v.scale,
            ..Default::default()
        };
        let s0 = v.to_screen(at);
        touch.push(t.poll(
            &Raw {
                touches: vec![(1, Finger::Started, s0)],
                ..base.clone()
            },
            &v,
        ));
        for i in 1..=3 {
            touch.push(t.poll(
                &Raw {
                    touches: vec![(1, Finger::Held, s0 + step * i as f32)],
                    ..base.clone()
                },
                &v,
            ));
        }
        touch.push(t.poll(
            &Raw {
                touches: vec![(1, Finger::Ended, s0 + step * 3.0)],
                ..base
            },
            &v,
        ));
        let swipes = |f: &[Input]| f.iter().filter_map(|i| i.swipe).collect::<Vec<_>>();
        assert_eq!(swipes(&m), swipes(&touch));
        assert_eq!(swipes(&m), vec![Dir::Up]);
        assert_eq!(m[0].pressed, touch[0].pressed);
        assert_eq!(m.last().unwrap().released, touch.last().unwrap().released);
    }

    #[test]
    fn a_swipe_inside_one_frame_still_reports_its_direction() {
        let v = View::ipad_landscape();
        let mut t = InputTracker::default();
        let from = v.to_screen(vec2(500.0, 400.0));
        let step = vec2(-SWIPE_MM * 6.0, 0.0) * v.scale;
        let i = t.poll(
            &Raw {
                touches: vec![
                    (1, Finger::Started, from),
                    (1, Finger::Held, from + step),
                    (1, Finger::Held, from + step * 2.0),
                    (1, Finger::Ended, from + step * 2.0),
                ],
                units_per_mm: Some(6.0),
                screen: vec2(v.w, v.h) * v.scale,
                ..Default::default()
            },
            &v,
        );
        assert_eq!(i.swipe, Some(Dir::Left));
        assert!(i.pressed && i.released && i.swiped);
        assert!(!i.tap_released());
        assert!((i.start - vec2(500.0, 400.0)).length() < 0.01);
    }

    #[test]
    fn a_swipe_inside_one_frame_fires_once_not_every_frame_after() {
        let v = View::ipad_landscape();
        let mut t = InputTracker::default();
        let from = v.to_screen(vec2(500.0, 400.0));
        let step = vec2(-SWIPE_MM * 6.0, 0.0) * v.scale;
        let base = Raw {
            units_per_mm: Some(6.0),
            screen: vec2(v.w, v.h) * v.scale,
            ..Default::default()
        };
        let first = t.poll(
            &Raw {
                touches: vec![
                    (1, Finger::Started, from),
                    (1, Finger::Held, from + step * 2.0),
                    (1, Finger::Ended, from + step * 2.0),
                ],
                ..base.clone()
            },
            &v,
        );
        assert_eq!(first.swipe, Some(Dir::Left));
        for _ in 0..5 {
            let next = t.poll(&base, &v);
            assert_eq!(next.swipe, None);
            assert!(next.path.is_empty());
        }
    }

    #[test]
    fn a_tap_inside_one_frame_is_still_a_tap() {
        let v = View::ipad_landscape();
        let mut t = InputTracker::default();
        let at = v.to_screen(vec2(500.0, 400.0));
        let i = t.poll(
            &Raw {
                touches: vec![(1, Finger::Started, at), (1, Finger::Ended, at)],
                units_per_mm: Some(6.0),
                screen: vec2(v.w, v.h) * v.scale,
                ..Default::default()
            },
            &v,
        );
        assert_eq!(i.swipe, None);
        assert!(i.tap_released());
    }

    #[test]
    fn a_press_and_move_in_one_frame_swipes_while_the_finger_is_down() {
        let v = View::ipad_landscape();
        let mut t = InputTracker::default();
        let from = v.to_screen(vec2(500.0, 400.0));
        let step = vec2(0.0, SWIPE_MM * 6.0 * 1.5) * v.scale;
        let i = t.poll(
            &Raw {
                touches: vec![(1, Finger::Started, from), (1, Finger::Held, from + step)],
                units_per_mm: Some(6.0),
                screen: vec2(v.w, v.h) * v.scale,
                ..Default::default()
            },
            &v,
        );
        assert_eq!(i.swipe, Some(Dir::Down));
        assert!(i.down && i.pressed);
    }

    #[test]
    fn a_second_finger_never_steals_the_first_in_the_tracker() {
        let v = View::ipad_landscape();
        let mut t = InputTracker::default();
        let base = Raw {
            screen: vec2(1024.0, 768.0),
            ..Default::default()
        };
        let a = vec2(100.0, 100.0);
        let p = t.poll(
            &Raw {
                touches: vec![(1, Finger::Started, a)],
                ..base.clone()
            },
            &v,
        );
        assert!(p.pressed);
        let p = t.poll(
            &Raw {
                touches: vec![
                    (1, Finger::Held, a),
                    (2, Finger::Started, vec2(800.0, 600.0)),
                ],
                ..base
            },
            &v,
        );
        assert!(!p.pressed && p.down && (p.pos - v.to_view(a)).length() < 0.01);
        assert_eq!(p.pointers.len(), 2);
        assert!(p.pointers[0].primary && !p.pointers[1].primary);
    }

    #[test]
    fn masked_input_keeps_scale_and_text_but_no_pointer() {
        let i = Input {
            pressed: true,
            down: true,
            mm: 5.0,
            typed: vec!['a'],
            axis: vec2(1.0, 0.0),
            ..Default::default()
        };
        let m = i.masked();
        assert!(!m.pressed && !m.down && m.axis == Vec2::ZERO);
        assert_eq!((m.mm, m.typed.len()), (5.0, 1));
    }

    #[test]
    fn kinds_lists_what_happened() {
        let (mut s, _) = script();
        let k = s.press(vec2(10.0, 10.0)).kinds();
        assert_eq!(k, vec![InputKind::Tap]);
        let keys = s.keys(&[Key::Left], 1)[0].kinds();
        assert!(keys.contains(&InputKind::Stick) && keys.contains(&InputKind::Key));
    }

    #[test]
    fn scripted_replay_is_deterministic() {
        let run = || {
            let (mut s, _) = script();
            let mut f = s.swipe(vec2(300.0, 300.0), Dir::Down, 3);
            f.extend(s.tap(vec2(50.0, 50.0)));
            f.extend(s.keys(&[Key::Up], 2));
            f.iter()
                .map(|i| (i.pressed, i.released, i.swipe, i.axis.to_array()))
                .collect::<Vec<_>>()
        };
        assert_eq!(run(), run());
    }

    #[test]
    fn scroll_and_pinch_pass_through() {
        let v = View::ipad_landscape();
        let mut t = InputTracker::default();
        let i = t.poll(
            &Raw {
                scroll: 3.0,
                pinch: -1.0,
                screen: vec2(1024.0, 768.0),
                ..Default::default()
            },
            &v,
        );
        let k = i.kinds();
        assert!(k.contains(&InputKind::Scroll) && k.contains(&InputKind::Pinch));
    }
}
