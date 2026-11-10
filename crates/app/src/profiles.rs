use crate::draw::measure;
use crate::paint::Pen;
use crate::ui::{Tone, BODY};
use glam::vec2;
use kg_core::engine::*;
use kg_core::fx::*;
use kg_core::geom::{Color, Rect, WHITE};
use kg_core::profile::{self, Profile, AVATARS, MAX_NAME, MAX_PROFILES};
use kg_core::sfx::{self, Sfx};

pub const BACKSPACE: char = '\u{8}';

#[derive(Clone, Debug, PartialEq)]
pub enum Mode {
    Welcome,
    Pick,
    Edit {
        id: Option<String>,
        name: String,
        avatar: usize,
        confirm_delete: bool,
    },
}

pub struct Profiles {
    pub mode: Mode,
    pub t: f32,
    pub done: bool,
    pub link: Option<crate::link::Link>,
    press: Option<Rect>,
    pub gate_hold: f32,
}

pub const KEYS: [&str; 3] = ["QWERTYUIOP", "ASDFGHJKL", "ZXCVBNM"];

impl Default for Profiles {
    fn default() -> Self {
        Self::new()
    }
}

impl Profiles {
    pub fn new() -> Self {
        let mode = if profile::list().is_empty() {
            Mode::Welcome
        } else {
            Mode::Pick
        };
        Self {
            mode,
            t: 0.0,
            done: false,
            link: None,
            press: None,
            gate_hold: 0.0,
        }
    }

    pub fn welcome_rects(view: &View) -> WelcomeRects {
        let mut s = kg_core::sizing::Stack::new(view, 580.0).ribbon(WELCOME_TITLE);
        s.text(26.0, WELCOME_NOTE);
        let art = s.block_min(120.0, view.tap(60.0));
        let b = s.buttons(84.0, &[MAKE_LABEL, LINK_DEVICE_LABEL]);
        let sheet = s.layout();
        WelcomeRects {
            make: sheet.button(b, 0),
            link: sheet.button(b, 1),
            art: sheet.rect(art),
            sheet,
        }
    }

    pub fn gate_rect(view: &View) -> Rect {
        Self::edit_layout(view, 0, true).gate
    }

    pub fn link_rect(view: &View) -> Rect {
        Self::pick_bar(view).right.unwrap_or_default()
    }

    pub fn pick_bar(view: &View) -> kg_core::sizing::TitleBar {
        kg_core::sizing::title_bar(view, PICK_TITLE, None, Some(LINK_LABEL))
    }

    pub fn gear_rect(view: &View, card: Rect) -> Rect {
        let d = view.tap(52.0);
        Rect::new(card.x + card.w - d * 0.75, card.y - d * 0.25, d, d)
    }

    pub fn card_rects(view: &View, n: usize) -> Vec<Rect> {
        let n = n + usize::from(n < MAX_PROFILES);
        let top_bar = Self::pick_bar(view).bottom;
        let gear = view.tap(52.0);
        let gap = 24.0f32.max(view.gap()).max(gear * 0.3);
        let label = view.text(30.0) * 1.35;
        let top = top_bar + gap + gear * 0.25;
        let room_h = view.h - top - view.edge().min(20.0);
        let room_w = view.w - 2.0 * view.edge().max(40.0) - gear * 0.25;
        let size_for = |cols: usize| {
            let rows = n.div_ceil(cols) as f32;
            let by_w = (room_w - gap * (cols as f32 - 1.0)) / cols as f32;
            let by_h = (room_h - gap * (rows - 1.0)) / rows - label;
            by_w.min(by_h).min(200.0 * view.text_k().min(1.6))
        };
        let cols = (1..=n.max(1))
            .max_by(|a, b| size_for(*a).total_cmp(&size_for(*b)))
            .unwrap_or(1);
        let size = size_for(cols).max(view.touch());
        let rows = n.div_ceil(cols);
        let grid_h = rows as f32 * (size + label) + (rows as f32 - 1.0) * gap;
        let top = top.max(top + (room_h - grid_h) / 2.0 * 0.6);
        (0..n)
            .map(|i| {
                let (c, r) = (i % cols, i / cols);
                let in_row = (n - r * cols).min(cols);
                let row_w = in_row as f32 * size + (in_row as f32 - 1.0) * gap;
                let left = view.w / 2.0 - row_w / 2.0;
                Rect::new(
                    left + c as f32 * (size + gap),
                    top + r as f32 * (size + label + gap),
                    size,
                    size + label,
                )
            })
            .collect()
    }

    pub fn edit_layout(view: &View, avatar: usize, editing: bool) -> EditLayout {
        edit_layout(view, avatar, editing)
    }

    pub fn avatar_rects(view: &View) -> Vec<Rect> {
        Self::edit_layout(view, 0, false)
            .avatars
            .iter()
            .map(|a| a.1)
            .collect()
    }

    pub fn name_rect(view: &View) -> Rect {
        Self::edit_layout(view, 0, false).name
    }

    pub fn key_rects(view: &View) -> Vec<(char, Rect)> {
        Self::edit_layout(view, 0, false).keys
    }

    pub fn action_rects(view: &View) -> (Rect, Rect, Rect) {
        let l = Self::edit_layout(view, 0, true);
        (l.ok, l.back, l.delete)
    }

    pub fn update(&mut self, input: &Input, view: &View, dt: f32) {
        self.t += dt;
        if let Some(l) = &mut self.link {
            l.update(input, view, dt);
            if l.done {
                self.link = None;
                if self.mode == Mode::Welcome {
                    if !profile::list().is_empty() {
                        kg_core::net::clear_redeem();
                        self.mode = Mode::Pick;
                    }
                    return;
                }
                self.mode = Mode::Pick;
                if profile::current_id().is_some()
                    && kg_core::net::redeem_state() == kg_core::net::Redeem::Done
                {
                    kg_core::net::clear_redeem();
                    self.done = true;
                }
            }
            return;
        }
        self.press = input
            .down
            .then_some(input.pos)
            .and_then(|p| self.hit_rect(view, p));
        self.update_gate(input, view, dt);
        match self.mode.clone() {
            Mode::Welcome => self.update_welcome(input, view),
            Mode::Pick => self.update_pick(input, view),
            Mode::Edit {
                id,
                name,
                avatar,
                confirm_delete,
            } => self.update_edit(input, view, id, name, avatar, confirm_delete),
        }
    }

    fn update_gate(&mut self, input: &Input, view: &View, dt: f32) {
        let Mode::Edit { id: Some(id), .. } = &self.mode else {
            self.gate_hold = 0.0;
            return;
        };
        if input.down && Self::gate_rect(view).contains(input.pos) {
            self.gate_hold += dt;
            if self.gate_hold >= kg_core::gate::HOLD_SECS {
                self.gate_hold = f32::NEG_INFINITY;
                kg_core::gate::set_enabled_for(id, !kg_core::gate::enabled_for(id));
                sfx::play(Sfx::Pop);
            }
        } else {
            self.gate_hold = 0.0;
        }
    }

    fn hit_rect(&self, view: &View, p: glam::Vec2) -> Option<Rect> {
        let mut all: Vec<Rect> = match &self.mode {
            Mode::Welcome => {
                let w = Self::welcome_rects(view);
                vec![w.make, w.link]
            }
            Mode::Pick => {
                let mut v = Self::card_rects(view, profile::list().len());
                v.push(Self::link_rect(view));
                v
            }
            Mode::Edit { avatar, id, .. } => {
                let l = Self::edit_layout(view, *avatar, id.is_some());
                let mut v: Vec<Rect> = l.keys.iter().map(|k| k.1).collect();
                v.extend(l.buttons());
                v
            }
        };
        all.retain(|r| r.contains(p));
        all.first().copied()
    }

    fn update_welcome(&mut self, input: &Input, view: &View) {
        if !input.pressed {
            return;
        }
        let w = Self::welcome_rects(view);
        if input.tapped_in(w.make) {
            sfx::play(Sfx::Tap);
            self.mode = Mode::Edit {
                id: None,
                name: String::new(),
                avatar: 0,
                confirm_delete: false,
            };
        } else if input.tapped_in(w.link) {
            sfx::play(Sfx::Tap);
            self.link = Some(crate::link::Link::new());
        }
    }

    fn update_pick(&mut self, input: &Input, view: &View) {
        let list = profile::list();
        let cards = Self::card_rects(view, list.len());
        if input.back && profile::current_id().is_some() {
            self.done = true;
            return;
        }
        if !input.pressed {
            return;
        }
        if input.tapped_in(Self::link_rect(view)) {
            sfx::play(Sfx::Tap);
            self.link = Some(crate::link::Link::new());
            return;
        }
        for (i, r) in cards.iter().enumerate() {
            let edit = Self::gear_rect(view, *r);
            if i < list.len() && edit.contains(input.pos) {
                sfx::play(Sfx::Tap);
                self.mode = Mode::Edit {
                    id: Some(list[i].id.clone()),
                    name: list[i].name.clone(),
                    avatar: list[i].avatar,
                    confirm_delete: false,
                };
                return;
            }
            if r.contains(input.pos) {
                if i < list.len() {
                    profile::select(&list[i].id);
                    sfx::play(Sfx::Pop);
                    self.done = true;
                } else {
                    sfx::play(Sfx::Tap);
                    self.mode = Mode::Edit {
                        id: None,
                        name: String::new(),
                        avatar: list.len() % AVATARS.len(),
                        confirm_delete: false,
                    };
                }
                return;
            }
        }
    }

    fn update_edit(
        &mut self,
        input: &Input,
        view: &View,
        id: Option<String>,
        mut name: String,
        mut avatar: usize,
        mut confirm_delete: bool,
    ) {
        let erasing = input.typed.contains(&BACKSPACE);
        if input.back && !erasing && id.is_none() && profile::list().is_empty() {
            self.mode = Mode::Welcome;
            return;
        }
        if input.back && !erasing && !profile::list().is_empty() {
            self.mode = Mode::Pick;
            return;
        }
        for ch in input.typed.iter().copied() {
            if ch == BACKSPACE {
                name.pop();
            } else if name.chars().count() < MAX_NAME && (ch.is_alphabetic() || ch == ' ') {
                name.push(if name.is_empty() {
                    ch.to_ascii_uppercase()
                } else {
                    ch.to_ascii_lowercase()
                });
            }
        }
        let submit = input.action && !name.trim().is_empty();
        let lay = Self::edit_layout(view, avatar, id.is_some());
        if input.pressed || submit {
            for (i, r) in &lay.avatars {
                if input.pressed && r.contains(input.pos) {
                    avatar = *i;
                    sfx::play(Sfx::Pop);
                }
            }
            if let Some((prev, next)) = lay.arrows {
                let n = AVATARS.len();
                if input.tapped_in(prev) {
                    avatar = (avatar + n - 1) % n;
                    sfx::play(Sfx::Click);
                } else if input.tapped_in(next) {
                    avatar = (avatar + 1) % n;
                    sfx::play(Sfx::Click);
                }
            }
            for (ch, r) in lay.keys.iter().copied() {
                if input.pressed && r.contains(input.pos) {
                    sfx::play(Sfx::Click);
                    if ch == '<' {
                        name.pop();
                    } else if name.chars().count() < MAX_NAME {
                        name.push(if name.is_empty() {
                            ch
                        } else {
                            ch.to_ascii_lowercase()
                        });
                    }
                }
            }
            let (ok, back, delete) = (lay.ok, lay.back, lay.delete);
            if (input.pressed && ok.contains(input.pos) || submit) && !name.trim().is_empty() {
                match &id {
                    Some(id) => profile::rename(id, &name, avatar),
                    None => {
                        profile::create(&name, avatar);
                    }
                }
                sfx::play(Sfx::Right);
                self.mode = Mode::Pick;
                if id.is_none() {
                    self.done = true;
                }
                return;
            }
            if input.pressed && back.contains(input.pos) {
                sfx::play(Sfx::Tap);
                self.mode = if profile::list().is_empty() {
                    Mode::Welcome
                } else {
                    Mode::Pick
                };
                return;
            }
            if let (true, Some(pid)) = (input.pressed && delete.contains(input.pos), &id) {
                if confirm_delete {
                    profile::delete(pid);
                    sfx::play(Sfx::Crunch);
                    self.mode = if profile::list().is_empty() {
                        Mode::Welcome
                    } else {
                        Mode::Pick
                    };
                    return;
                }
                confirm_delete = true;
                sfx::play(Sfx::Wrong);
            }
        }
        self.mode = Mode::Edit {
            id,
            name,
            avatar,
            confirm_delete,
        };
    }

    pub fn draw(&self, pen: &mut Pen, view: &View) {
        if let Some(l) = &self.link {
            l.draw(pen, view);
            return;
        }
        let pressed = |r: Rect| (self.press == Some(r)) as i32 as f32;
        let plum = Color::from_rgba(50, 15, 80, 255);
        match &self.mode {
            Mode::Welcome => {
                let w = Self::welcome_rects(view);
                pen.painted_panel(w.sheet.panel, WHITE);
                if let Some((rr, _)) = w.sheet.ribbon {
                    pen.ribbon_in(view, WELCOME_TITLE, rr, self.t);
                }
                pen.sheet_text(&w.sheet, 0, None, BODY);
                let d = w.art.h.min(w.art.w / 4.0);
                for (i, avatar) in [0usize, 3, 6].into_iter().enumerate() {
                    let x = w.art.center().x + (i as f32 - 1.0) * d * 1.15;
                    let bob = ((self.t * 3.0 + i as f32 * 1.7).sin() * 0.04 + 1.0) * d;
                    pen.avatar(avatar, vec2(x, w.art.center().y), bob * 0.9);
                }
                pen.painted_button(w.make, MAKE_LABEL, Tone::Green, pressed(w.make), 1.0);
                pen.painted_button(w.link, LINK_DEVICE_LABEL, Tone::Blue, pressed(w.link), 1.0);
            }
            Mode::Pick => {
                let bar = Self::pick_bar(view);
                pen.ribbon_in(view, PICK_TITLE, bar.ribbon, self.t);
                let link = Self::link_rect(view);
                pen.painted_button(link, LINK_LABEL, Tone::Blue, pressed(link), 1.0);
                let list = profile::list();
                let current = profile::current_id();
                for (i, r) in Self::card_rects(view, list.len()).iter().enumerate() {
                    let pop = ease_out_back(((self.t - i as f32 * 0.05) / 0.3).clamp(0.0, 1.0));
                    let c = vec2(r.center().x, r.y + r.w / 2.0);
                    let d = r.w * pop * (1.0 - pressed(*r) * 0.06);
                    if i < list.len() {
                        let p: &Profile = &list[i];
                        if current.as_deref() == Some(&p.id) {
                            pen.circle(c, d * 0.56, Color::new(1.0, 0.9, 0.4, 0.55));
                        }
                        pen.avatar(p.avatar, c, d * 0.9);
                        let ns = kg_core::sizing::fit_size(&p.name, view.text(30.0), r.w + 20.0);
                        pen.text_outline(&p.name, c.x, r.y + r.h - ns * 0.6, ns, WHITE, plum);
                        pen.painted_round(Self::gear_rect(view, *r), "icon_gear", 0.0);
                    } else {
                        pen.circle(c, d * 0.45, Color::new(1.0, 1.0, 1.0, 0.25));
                        pen.text_outline("+", c.x, c.y - 4.0, d * 0.5, WHITE, plum);
                        let ns =
                            kg_core::sizing::fit_size("New player", view.text(28.0), r.w + 20.0);
                        pen.text_outline("New player", c.x, r.y + r.h - ns * 0.6, ns, WHITE, plum);
                    }
                }
            }
            Mode::Edit {
                id,
                name,
                avatar: sel,
                confirm_delete,
            } => {
                let title = if id.is_some() {
                    "Edit player"
                } else {
                    "New player"
                };
                let lay = Self::edit_layout(view, *sel, id.is_some());
                if let Some((rr, _)) = lay.ribbon {
                    pen.ribbon_in(view, title, rr, self.t);
                }
                if let Some(y) = lay.prompt {
                    let cx = lay.name.center().x;
                    pen.text_center("Pick your animal", cx, y, view.text(30.0), WHITE);
                }
                for (i, r) in &lay.avatars {
                    let on = i == sel;
                    let k = if on {
                        1.12 + (self.t * 5.0).sin() * 0.03
                    } else {
                        0.92
                    };
                    if on {
                        pen.circle(r.center(), r.w * 0.62, Color::new(1.0, 0.9, 0.3, 0.7));
                    }
                    pen.avatar(*i, r.center(), r.w * k);
                }
                if let Some((a, b)) = lay.arrows {
                    pen.key_cap(a, "<", pressed(a));
                    pen.key_cap(b, ">", pressed(b));
                }
                let nr = lay.name;
                pen.painted_panel(nr, WHITE);
                let shown = if name.is_empty() {
                    "Type your name".to_string()
                } else {
                    let caret = if (self.t * 2.0).fract() < 0.5 {
                        "|"
                    } else {
                        " "
                    };
                    format!("{name}{caret}")
                };
                pen.text_center(
                    &shown,
                    nr.center().x,
                    nr.center().y,
                    kg_core::sizing::fit_size(&shown, nr.h * 0.52, nr.w - 40.0),
                    if name.is_empty() {
                        Color::new(0.4, 0.35, 0.5, 0.6)
                    } else {
                        BODY
                    },
                );
                for (ch, r) in &lay.keys {
                    let label = if *ch == '<' {
                        "DEL".to_string()
                    } else {
                        ch.to_string()
                    };
                    pen.key_cap(*r, &label, pressed(*r));
                }
                let (ok, back, delete) = (lay.ok, lay.back, lay.delete);
                let ready = !name.trim().is_empty();
                pen.painted_button(
                    ok,
                    if id.is_some() { "Save" } else { "Let's go!" },
                    Tone::Green,
                    pressed(ok),
                    if ready { 1.0 } else { 0.45 },
                );
                pen.painted_button(back, BACK_LABEL, Tone::Blue, pressed(back), 1.0);
                if let Some(pid) = id {
                    if let Some(y) = lay.note {
                        let s =
                            kg_core::sizing::fit_size(SOURCE_NOTE, view.text(18.0), view.w - 24.0);
                        pen.text_center(
                            SOURCE_NOTE,
                            view.w / 2.0,
                            y,
                            s,
                            Color::new(1.0, 1.0, 1.0, 0.75),
                        );
                    }
                    let on = kg_core::gate::enabled_for(pid);
                    pen.painted_button(
                        lay.gate,
                        if on { GATE_LABEL } else { "Hold: lock again" },
                        Tone::Blue,
                        (self.gate_hold / kg_core::gate::HOLD_SECS).clamp(0.0, 1.0),
                        0.85,
                    );
                    pen.painted_button(
                        delete,
                        if *confirm_delete { "Sure?" } else { "Delete" },
                        Tone::Orange,
                        pressed(delete),
                        1.0,
                    );
                }
            }
        }
    }
}

pub struct WelcomeRects {
    pub sheet: kg_core::sizing::Sheet,
    pub art: Rect,
    pub make: Rect,
    pub link: Rect,
}

const WELCOME_TITLE: &str = "Welcome!";
const WELCOME_NOTE: &str = "Make a player, or link this device to another one.";
const MAKE_LABEL: &str = "Make a player";
const LINK_DEVICE_LABEL: &str = "Link to another device";

pub const CHIP_H: f32 = 60.0;
pub const SOURCE_NOTE: &str = "Free software (GPL-3.0). Source: github.com/v0l/kids-free-games";

pub fn chip_label(name: &str) -> String {
    format!("Playing as {name}")
}

pub const PICK_TITLE: &str = "Who's playing?";
pub const LINK_LABEL: &str = "Link device";
pub const GATE_LABEL: &str = "Hold: unlock all";
pub const BACK_LABEL: &str = "Back";
const ABC: &str = "ABCDEFGHIJKLMNOPQRSTUVWXYZ";

#[derive(Clone, Debug, Default)]
pub struct EditLayout {
    pub ribbon: Option<(Rect, f32)>,
    pub prompt: Option<f32>,
    pub avatars: Vec<(usize, Rect)>,
    pub arrows: Option<(Rect, Rect)>,
    pub name: Rect,
    pub keys: Vec<(char, Rect)>,
    pub ok: Rect,
    pub back: Rect,
    pub delete: Rect,
    pub gate: Rect,
    pub note: Option<f32>,
    pub bottom: f32,
}

impl EditLayout {
    pub fn buttons(&self) -> Vec<Rect> {
        let mut v: Vec<Rect> = self.avatars.iter().map(|a| a.1).collect();
        if let Some((a, b)) = self.arrows {
            v.extend([a, b]);
        }
        v.extend([self.ok, self.back, self.delete, self.gate]);
        v.retain(|r| r.w > 0.0);
        v
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Avatars {
    Grid,
    Strip,
    Small,
}

fn avatar_grid(
    view: &View,
    area: Rect,
    y: f32,
    sel: usize,
    mode: Avatars,
) -> (Vec<(usize, Rect)>, Option<(Rect, Rect)>, f32) {
    let gap = view.key_gap().max(10.0);
    let d = view.kid_tap(96.0).min(area.w / 3.0).max(view.touch());
    let n = AVATARS.len();
    match mode {
        Avatars::Grid => {
            let cols = (((area.w + gap) / (d + gap)).floor() as usize).clamp(1, 6);
            let rows = n.div_ceil(cols);
            let row_w = cols as f32 * (d + gap) - gap;
            let left = area.x + area.w / 2.0 - row_w / 2.0;
            let rects = (0..n)
                .map(|i| {
                    let (c, r) = (i % cols, i / cols);
                    (
                        i,
                        Rect::new(left + c as f32 * (d + gap), y + r as f32 * (d + gap), d, d),
                    )
                })
                .collect();
            (rects, None, y + rows as f32 * (d + gap) - gap)
        }
        Avatars::Strip => avatar_grid_sized(view, area, y, sel, d),
        Avatars::Small => avatar_grid_sized(view, area, y, sel, view.touch()),
    }
}

fn name_keys(view: &View, area: Rect, y: f32) -> (Vec<(char, Rect)>, f32) {
    let m = view.edge().min(16.0);
    let pad = Rect::new(area.x + m, y, area.w - 2.0 * m, view.h);
    let k = kg_core::sizing::keypad(view, pad, ABC.len() + 1, 10, 68.0);
    let mut out: Vec<(char, Rect)> = ABC.chars().zip(k.keys.iter().copied()).collect();
    let mut del = k.keys[ABC.len()];
    let in_last = (ABC.len() + 1) % k.cols;
    if in_last != 0 {
        let free = (k.cols - in_last).min(1);
        del.w += free as f32 * (del.w + view.key_gap());
    }
    out.push(('<', del));
    (out, k.bottom)
}

fn edit_try(
    view: &View,
    sel: usize,
    editing: bool,
    mode: Avatars,
    split: bool,
    prompt: bool,
) -> EditLayout {
    let gap = view.gap();
    let m = view.edge().min(16.0);
    let (left, right) = if split {
        let w = view.w / 2.0;
        (Rect::new(0.0, 0.0, w, view.h), Rect::new(w, 0.0, w, view.h))
    } else {
        (view.rect(), view.rect())
    };
    let bar = kg_core::sizing::title_bar_in(
        view,
        left,
        "Edit player",
        Some(BACK_LABEL),
        editing.then_some(GATE_LABEL),
    );
    let ribbon = Some((
        bar.ribbon,
        kg_core::sizing::ribbon_font(view, "Edit player", bar.ribbon),
    ));
    let mut y = bar.bottom + gap;
    let prompt_y = prompt.then(|| {
        let s = view.text(30.0);
        y += s * 1.3;
        y - s * 0.65
    });
    let (avatars, arrows, bottom) = avatar_grid(view, left, y, sel, mode);
    y = bottom + gap;
    let nw = (left.w - 2.0 * m).min(520.0 * view.text_k());
    let name = Rect::new(left.x + left.w / 2.0 - nw / 2.0, y, nw, view.tap(76.0));
    y = name.y + name.h + gap;
    let (keys, keys_bottom) = if split {
        let (keys, b) = name_keys(view, right, 0.0);
        let h = b;
        let dy = ((view.h - h) / 2.0).max(m);
        let keys: Vec<(char, Rect)> = keys
            .into_iter()
            .map(|(c, r)| (c, r.offset(vec2(0.0, dy))))
            .collect();
        (keys, b + dy)
    } else {
        let (keys, b) = name_keys(view, left, y);
        y = b + gap;
        (keys, b)
    };
    let (bw, bh) = kg_core::sizing::label_button(view, "Let's go!", 84.0);
    let row = Rect::new(left.x + m, y, left.w - 2.0 * m, bh);
    let (ok, delete) = if editing {
        let r = kg_core::sizing::row(row, 2, gap);
        (r[0], r[1])
    } else {
        let w = bw.max(row.w * 0.6).min(row.w);
        (
            Rect::new(row.x + (row.w - w) / 2.0, y, w, bh),
            Rect::default(),
        )
    };
    let actions_bottom = y + bh;
    let note_h = 18.0 * view.text_k() * 1.4;
    let bottom = actions_bottom.max(keys_bottom);
    let note =
        (editing && actions_bottom + note_h <= view.h).then_some(actions_bottom + note_h * 0.6);
    EditLayout {
        ribbon,
        prompt: prompt_y,
        avatars,
        arrows,
        name,
        keys,
        ok,
        back: bar.left.unwrap_or_default(),
        delete,
        gate: bar.right.unwrap_or_default(),
        note,
        bottom,
    }
}

fn edit_compact(view: &View, sel: usize, editing: bool) -> EditLayout {
    let gap = view.gap();
    let m = view.edge().min(16.0);
    let t = view.touch();
    let top = 8.0;
    let (bw, _) = kg_core::sizing::label_button(view, BACK_LABEL, 56.0);
    let back = Rect::new(m, top, bw, t);
    let gate = if editing {
        let (gw, _) = kg_core::sizing::label_button(view, GATE_LABEL, 56.0);
        let gw = gw.min(view.w * 0.3);
        Rect::new(view.w - m - gw, top, gw, t)
    } else {
        Rect::default()
    };
    let right_edge = if editing { gate.x - gap } else { view.w - m };
    let del_w = view.key_w().max(t);
    let tall = view.portrait();
    let (name, del, y2) = if tall {
        let ny = top + t + gap;
        let del = Rect::new(view.w - m - del_w, ny, del_w, t);
        (Rect::new(m, ny, del.x - gap - m, t), del, ny + t + gap)
    } else {
        let del = Rect::new(right_edge - del_w, top, del_w, t);
        let name = Rect::new(
            back.x + back.w + gap,
            top,
            del.x - gap - back.x - back.w - gap,
            t,
        );
        (name, del, top + t + gap)
    };
    let actions_w = if tall {
        view.w - 2.0 * m
    } else if editing {
        view.w * 0.36
    } else {
        view.w * 0.22
    };
    let strip_w = if tall {
        view.w
    } else {
        view.w - actions_w - gap - m
    };
    let strip = Rect::new(0.0, 0.0, strip_w, view.h);
    let (avatars, arrows, _) = avatar_grid_sized(view, strip, y2, sel, t);
    let ay = if tall { y2 + t + view.key_gap() } else { y2 };
    let act = Rect::new(view.w - m - actions_w, ay, actions_w, t);
    let (ok, delete) = if editing {
        let r = kg_core::sizing::row(act, 2, gap);
        (r[0], r[1])
    } else {
        (act, Rect::default())
    };
    let pad = Rect::new(m, ay + t + gap, view.w - 2.0 * m, view.h);
    let k = kg_core::sizing::keypad(view, pad, ABC.len(), 13, 68.0);
    let mut keys: Vec<(char, Rect)> = ABC.chars().zip(k.keys.iter().copied()).collect();
    keys.push(('<', del));
    EditLayout {
        ribbon: None,
        prompt: None,
        avatars,
        arrows,
        name,
        keys,
        ok,
        back,
        delete,
        gate,
        note: None,
        bottom: k.bottom,
    }
}

fn avatar_grid_sized(
    view: &View,
    area: Rect,
    y: f32,
    sel: usize,
    d: f32,
) -> (Vec<(usize, Rect)>, Option<(Rect, Rect)>, f32) {
    let gap = view.key_gap().max(10.0);
    let n = AVATARS.len();
    let a = view.touch();
    let inner = area.w - 2.0 * (a + gap) - 2.0 * view.edge().min(16.0);
    let shown = (((inner + gap) / (d + gap)).floor() as usize).clamp(1, n);
    let start = sel.saturating_sub(shown / 2).min(n - shown);
    let row_w = shown as f32 * (d + gap) - gap;
    let left = area.x + area.w / 2.0 - row_w / 2.0;
    let rects = (0..shown)
        .map(|j| (start + j, Rect::new(left + j as f32 * (d + gap), y, d, d)))
        .collect();
    let ay = y + (d - a) / 2.0;
    let arrows = (shown < n).then(|| {
        (
            Rect::new(left - gap - a, ay, a, a),
            Rect::new(left + row_w + gap, ay, a, a),
        )
    });
    (rects, arrows, y + d)
}

pub fn edit_layout(view: &View, sel: usize, editing: bool) -> EditLayout {
    let splits: &[bool] = if view.portrait() {
        &[false]
    } else {
        &[false, true]
    };
    for split in splits.iter().copied() {
        for (mode, prompt) in [
            (Avatars::Grid, true),
            (Avatars::Grid, false),
            (Avatars::Strip, true),
            (Avatars::Strip, false),
            (Avatars::Small, false),
        ] {
            let l = edit_try(view, sel, editing, mode, split, prompt);
            let rows = l
                .avatars
                .iter()
                .map(|a| (a.1.y * 10.0) as i64)
                .collect::<std::collections::BTreeSet<_>>()
                .len();
            if l.bottom <= view.h - view.edge().min(16.0) && rows <= 2 {
                return l;
            }
        }
    }
    edit_compact(view, sel, editing)
}

pub fn player_chip_at(view: &View, y: f32) -> Rect {
    let name = profile::current()
        .map(|p| p.name)
        .unwrap_or_else(|| "Guest".into());
    let w = measure(&chip_label(&name), 26.0) + 130.0;
    Rect::new(view.w / 2.0 - w / 2.0, y, w, view.tap(CHIP_H))
}

pub fn draw_player_chip(pen: &mut Pen, r: Rect, scroll: f32) {
    let r = r.offset(vec2(0.0, -scroll));
    let Some(p) = profile::current() else {
        return;
    };
    pen.chip_box(Rect::new(r.x + 20.0, r.y + 6.0, r.w - 20.0, r.h - 12.0));
    pen.avatar(p.avatar, vec2(r.x + 28.0, r.center().y), r.h);
    let label = chip_label(&p.name);
    pen.text_outline(
        &label,
        r.x + 66.0 + measure(&label, 26.0) / 2.0,
        r.center().y,
        26.0,
        WHITE,
        Color::from_rgba(50, 15, 80, 255),
    );
    pen.painted_round(
        Rect::new(r.x + r.w - 46.0, r.center().y - 22.0, 44.0, 44.0),
        "icon_gear",
        0.0,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tap(p: &mut Profiles, v: &View, at: glam::Vec2) {
        p.update(&Input::press_at(at), v, 1.0 / 60.0);
        p.update(&Input::default(), v, 1.0 / 60.0);
    }

    #[test]
    fn first_launch_types_a_name_and_creates_a_profile() {
        kg_core::save::reset_for_test();
        let v = View::ipad_landscape();
        let mut p = Profiles::new();
        assert_eq!(p.mode, Mode::Welcome);
        tap(&mut p, &v, Profiles::welcome_rects(&v).make.center());
        assert!(matches!(p.mode, Mode::Edit { id: None, .. }));
        let av = Profiles::avatar_rects(&v);
        tap(&mut p, &v, av[3].center());
        for want in "MIA".chars() {
            let (_, r) = Profiles::key_rects(&v)
                .into_iter()
                .find(|(c, _)| *c == want)
                .unwrap();
            tap(&mut p, &v, r.center());
        }
        match &p.mode {
            Mode::Edit { name, avatar, .. } => {
                assert_eq!(name, "Mia");
                assert_eq!(*avatar, 3);
            }
            _ => panic!(),
        }
        tap(&mut p, &v, Profiles::action_rects(&v).0.center());
        assert!(p.done);
        let cur = profile::current().unwrap();
        assert_eq!(cur.name, "Mia");
        assert_eq!(cur.avatar, 3);
        assert!(profile::is_uuid(&cur.id));
    }

    #[test]
    fn a_fresh_state_offers_both_choices() {
        kg_core::save::reset_for_test();
        for (name, v) in kg_core::sizing::test_views() {
            let p = Profiles::new();
            assert_eq!(p.mode, Mode::Welcome, "{name}");
            let w = Profiles::welcome_rects(&v);
            kg_core::sizing::check_buttons(&v, name, &[w.make, w.link]).unwrap();
            kg_core::sizing::check_apart(name, &[w.make, w.link]).unwrap();
            kg_core::sizing::check_sheet(&v, name, &w.sheet).unwrap();
        }
    }

    #[test]
    fn linking_from_the_empty_state_imports_profiles_without_making_one() {
        kg_core::save::reset_for_test();
        let v = View::ipad_landscape();
        let mut p = Profiles::new();
        tap(&mut p, &v, Profiles::welcome_rects(&v).link.center());
        assert!(p.link.is_some());
        assert!(profile::list().is_empty());
        let id = "0f8fad5b-d9cb-469f-a165-70867728950e";
        profile::adopt(id, "Mia", 2).unwrap();
        p.link.as_mut().unwrap().done = true;
        p.update(&Input::default(), &v, 1.0 / 60.0);
        assert!(p.link.is_none());
        assert_eq!(p.mode, Mode::Pick);
        assert!(!p.done);
        assert_eq!(profile::list().len(), 1);
        assert_eq!(profile::list()[0].id, id);
    }

    #[test]
    fn cancelling_the_link_returns_to_the_empty_state() {
        kg_core::save::reset_for_test();
        let v = View::ipad_landscape();
        let mut p = Profiles::new();
        tap(&mut p, &v, Profiles::welcome_rects(&v).link.center());
        tap(&mut p, &v, crate::link::Link::back_rect(&v).center());
        assert!(p.link.is_none());
        assert_eq!(p.mode, Mode::Welcome);
        assert!(profile::list().is_empty());
        assert!(!p.done);
    }

    #[test]
    fn backing_out_of_the_new_player_form_returns_to_the_empty_state() {
        kg_core::save::reset_for_test();
        let v = View::ipad_landscape();
        let mut p = Profiles::new();
        tap(&mut p, &v, Profiles::welcome_rects(&v).make.center());
        tap(&mut p, &v, Profiles::action_rects(&v).1.center());
        assert_eq!(p.mode, Mode::Welcome);
    }

    #[test]
    fn a_hardware_keyboard_types_the_name_and_enter_saves() {
        kg_core::save::reset_for_test();
        let v = View::ipad_landscape();
        let mut p = Profiles::new();
        tap(&mut p, &v, Profiles::welcome_rects(&v).make.center());
        p.update(
            &Input {
                typed: "leox".chars().collect(),
                ..Default::default()
            },
            &v,
            0.016,
        );
        p.update(
            &Input {
                typed: vec![BACKSPACE],
                back: true,
                ..Default::default()
            },
            &v,
            0.016,
        );
        p.update(
            &Input {
                action: true,
                ..Default::default()
            },
            &v,
            0.016,
        );
        assert!(p.done);
        assert_eq!(profile::current().unwrap().name, "Leo");
    }

    #[test]
    fn backspace_while_typing_a_name_erases_instead_of_leaving() {
        kg_core::save::reset_for_test();
        profile::create("Mia", 0).unwrap();
        let v = View::ipad_landscape();
        let mut p = Profiles::new();
        tap(
            &mut p,
            &v,
            *Profiles::card_rects(&v, 1)
                .last()
                .map(|r| r.center())
                .as_ref()
                .unwrap(),
        );
        assert!(matches!(p.mode, Mode::Edit { id: None, .. }));
        p.update(
            &Input {
                typed: "Leo".chars().collect(),
                ..Default::default()
            },
            &v,
            0.016,
        );
        p.update(
            &Input {
                typed: vec![BACKSPACE],
                back: true,
                ..Default::default()
            },
            &v,
            0.016,
        );
        assert!(matches!(&p.mode, Mode::Edit { name, .. } if name == "Le"));
        p.update(
            &Input {
                back: true,
                ..Default::default()
            },
            &v,
            0.016,
        );
        assert_eq!(p.mode, Mode::Pick);
    }

    #[test]
    fn an_empty_name_cannot_be_saved() {
        kg_core::save::reset_for_test();
        let v = View::ipad_landscape();
        let mut p = Profiles::new();
        tap(&mut p, &v, Profiles::action_rects(&v).0.center());
        assert!(!p.done);
        assert!(profile::list().is_empty());
    }

    #[test]
    fn picking_a_card_switches_player() {
        kg_core::save::reset_for_test();
        let a = profile::create("Mia", 0).unwrap();
        profile::create("Leo", 1).unwrap();
        let v = View::ipad_portrait();
        let mut p = Profiles::new();
        assert_eq!(p.mode, Mode::Pick);
        let cards = Profiles::card_rects(&v, 2);
        tap(&mut p, &v, cards[0].center());
        assert!(p.done);
        assert_eq!(profile::current_id(), Some(a.id));
    }

    #[test]
    fn holding_the_grown_up_button_toggles_gating_for_that_player_only() {
        kg_core::save::reset_for_test();
        let mia = profile::create("Mia", 0).unwrap();
        let leo = profile::create("Leo", 0).unwrap();
        let v = View::ipad_landscape();
        let mut p = Profiles::new();
        p.mode = Mode::Edit {
            id: Some(mia.id.clone()),
            name: "Mia".into(),
            avatar: 0,
            confirm_delete: false,
        };
        let at = Profiles::gate_rect(&v).center();
        p.update(&Input::press_at(at), &v, 0.5);
        assert!(kg_core::gate::enabled_for(&mia.id));
        for _ in 0..4 {
            let mut held = Input::press_at(at);
            held.pressed = false;
            p.update(&held, &v, 0.5);
        }
        assert!(!kg_core::gate::enabled_for(&mia.id));
        assert!(kg_core::gate::enabled_for(&leo.id));
    }

    #[test]
    fn delete_needs_a_second_tap() {
        kg_core::save::reset_for_test();
        profile::create("Mia", 0).unwrap();
        let b = profile::create("Leo", 1).unwrap();
        let v = View::ipad_landscape();
        let mut p = Profiles::new();
        let cards = Profiles::card_rects(&v, 2);
        let edit = vec2(cards[1].x + cards[1].w - 22.0, cards[1].y + 22.0);
        tap(&mut p, &v, edit);
        assert!(matches!(&p.mode, Mode::Edit { id: Some(i), .. } if *i == b.id));
        let del = Profiles::action_rects(&v).2.center();
        tap(&mut p, &v, del);
        assert_eq!(profile::list().len(), 2);
        tap(&mut p, &v, del);
        assert_eq!(profile::list().len(), 1);
        assert_eq!(p.mode, Mode::Pick);
    }

    #[test]
    fn the_link_button_never_sits_on_a_player_card() {
        for v in [
            View::ipad_landscape(),
            View::ipad_portrait(),
            View::fit(390.0, 844.0),
            View::fit(844.0, 390.0),
        ] {
            let link = Profiles::link_rect(&v);
            assert!(link.y + link.h <= v.h + 1.0, "{v:?}");
            for n in 0..=MAX_PROFILES {
                for c in Profiles::card_rects(&v, n) {
                    assert!(
                        !c.overlaps(&link),
                        "card hits link button with {n} players in {v:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn tapping_link_opens_the_link_screen_and_back_returns() {
        kg_core::save::reset_for_test();
        profile::create("Mia", 0).unwrap();
        let v = View::ipad_landscape();
        let mut p = Profiles::new();
        tap(&mut p, &v, Profiles::link_rect(&v).center());
        assert!(p.link.is_some());
        tap(&mut p, &v, crate::link::Link::back_rect(&v).center());
        assert!(p.link.is_none());
        assert_eq!(p.mode, Mode::Pick);
        assert!(!p.done);
    }

    #[test]
    fn profile_screens_are_twelve_mm_on_every_screen() {
        use kg_core::sizing::*;
        kg_core::save::reset_for_test();
        for (name, v) in test_views() {
            for editing in [false, true] {
                for sel in [0, 5, 11] {
                    let l = Profiles::edit_layout(&v, sel, editing);
                    let what = format!("{name} edit={editing} sel={sel}");
                    let keys: Vec<Rect> = l.keys.iter().map(|k| k.1).collect();
                    assert_eq!(keys.len(), 27, "{what}");
                    check_keys(&v, &what, &keys).unwrap();
                    check_buttons(&v, &what, &l.buttons()).unwrap();
                    let mut all = keys.clone();
                    all.extend(l.buttons());
                    all.push(l.name);
                    if let Some((r, _)) = l.ribbon {
                        all.push(r.scaled_about_center(0.9));
                        check_inside(&v, &what, r).unwrap();
                    }
                    check_apart(&what, &all).unwrap();
                    check_inside(&v, &what, l.name).unwrap();
                    assert!(
                        l.avatars.iter().any(|a| a.0 == sel),
                        "{what}: selected animal hidden"
                    );
                    if name == "phone portrait" {
                        let cols = keys
                            .iter()
                            .filter(|k| (k.y - keys[0].y).abs() < 0.5)
                            .count();
                        assert_eq!(cols, 7, "{what}");
                    }
                }
            }
            let w = Profiles::welcome_rects(&v);
            kg_core::sizing::check_buttons(&v, name, &[w.make, w.link]).unwrap();
            for n in 0..=MAX_PROFILES {
                let cards = Profiles::card_rects(&v, n);
                let what = format!("{name} {n} players");
                let mut all = cards.clone();
                check_buttons(&v, &what, &cards).unwrap();
                let gears: Vec<Rect> = cards
                    .iter()
                    .take(n)
                    .map(|c| Profiles::gear_rect(&v, *c))
                    .collect();
                check_buttons(&v, &what, &gears).unwrap();
                check_apart(&what, &gears).unwrap();
                all.push(Profiles::link_rect(&v));
                check_apart(&what, &all).unwrap();
                check_buttons(&v, &what, &[Profiles::link_rect(&v)]).unwrap();
                let bar = Profiles::pick_bar(&v);
                assert!(!bar.ribbon.overlaps(&Profiles::link_rect(&v)), "{what}");
            }
        }
    }

    #[test]
    fn portrait_phones_keep_one_column_with_seven_keys_a_row() {
        for (v, want) in [
            (kg_core::sizing::phone_portrait(), 7),
            (View::fit(390.0, 844.0), 6),
        ] {
            for editing in [false, true] {
                let l = Profiles::edit_layout(&v, 0, editing);
                let k = &l.keys;
                let cols = k.iter().filter(|c| (c.1.y - k[0].1.y).abs() < 0.5).count();
                assert!(cols >= want, "{cols} columns in {v:?} editing={editing}");
                let keys: Vec<Rect> = k.iter().map(|c| c.1).collect();
                kg_core::sizing::check_keys(&v, "phone", &keys).unwrap();
                kg_core::sizing::check_buttons(&v, "phone", &l.buttons()).unwrap();
                let mut all = keys.clone();
                all.extend(l.buttons());
                all.push(l.name);
                kg_core::sizing::check_apart("phone", &all).unwrap();
            }
        }
    }

    #[test]
    fn edit_layout_fits_every_screen_without_overlap() {
        for v in [
            View::ipad_landscape(),
            View::ipad_portrait(),
            View::fit(390.0, 844.0),
            View::fit(844.0, 390.0),
        ] {
            let l = Profiles::edit_layout(&v, 0, false);
            let keys = l.keys.clone();
            let ok = l.ok;
            let name = l.name;
            let last_av = l.avatars.last().unwrap().1;
            assert!(
                last_av.y + last_av.h <= name.y || !last_av.overlaps(&name),
                "avatars hit the name box in {v:?}"
            );
            for (i, (_, a)) in keys.iter().enumerate() {
                for (_, b) in &keys[i + 1..] {
                    assert!(!a.overlaps(b), "keys overlap in {v:?}");
                }
            }
            for (_, k) in &keys {
                assert!(k.x >= 0.0 && k.x + k.w <= v.w, "key off screen in {v:?}");
                assert!(!k.overlaps(&name), "key under the name box in {v:?}");
                assert!(!k.overlaps(&ok), "key under the button in {v:?}");
            }
            assert!(ok.y + ok.h <= v.h + 1.0, "button off screen in {v:?}");
        }
    }
}
