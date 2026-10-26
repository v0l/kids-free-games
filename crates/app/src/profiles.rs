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
            Mode::Edit {
                id: None,
                name: String::new(),
                avatar: 0,
                confirm_delete: false,
            }
        } else {
            Mode::Pick
        };
        Self {
            mode,
            t: 0.0,
            done: false,
            link: None,
            press: None,
        }
    }

    pub fn link_rect(view: &View) -> Rect {
        Rect::new(view.w - 196.0, 10.0, 186.0, 52.0)
    }

    pub fn card_rects(view: &View, n: usize) -> Vec<Rect> {
        let n = n + usize::from(n < MAX_PROFILES);
        let cols = if view.w > view.h { 4 } else { 3 }.min(n.max(1));
        let size = ((view.w - 80.0) / cols as f32 - 24.0).min(200.0);
        let rows = n.div_ceil(cols);
        let top = 190.0f32.max(view.h * 0.5 - rows as f32 * (size + 60.0) / 2.0 + 30.0);
        (0..n)
            .map(|i| {
                let (c, r) = (i % cols, i / cols);
                let in_row = (n - r * cols).min(cols);
                let row_w = in_row as f32 * size + (in_row as f32 - 1.0) * 24.0;
                let left = view.w / 2.0 - row_w / 2.0;
                Rect::new(
                    left + c as f32 * (size + 24.0),
                    top + r as f32 * (size + 60.0),
                    size,
                    size + 40.0,
                )
            })
            .collect()
    }

    pub fn avatar_rects(view: &View) -> Vec<Rect> {
        let wide = view.w > view.h;
        let cols = 6;
        let d = ((view.w - 60.0) / cols as f32 - 10.0).min(if wide { 62.0 } else { 96.0 });
        let row_w = cols as f32 * (d + 10.0) - 10.0;
        let top = if wide { 160.0 } else { 240.0 };
        (0..AVATARS.len())
            .map(|i| {
                let (c, r) = (i % cols, i / cols);
                Rect::new(
                    view.w / 2.0 - row_w / 2.0 + c as f32 * (d + 10.0),
                    top + r as f32 * (d + 10.0),
                    d,
                    d,
                )
            })
            .collect()
    }

    pub fn name_rect(view: &View) -> Rect {
        let rows = AVATARS.len().div_ceil(6);
        let av = Self::avatar_rects(view);
        let below = av[(rows - 1) * av.len() / rows].y + av[0].h + 24.0;
        let w = (view.w - 80.0).min(520.0);
        let h = if view.w > view.h { 64.0 } else { 76.0 };
        Rect::new(view.w / 2.0 - w / 2.0, below - 8.0, w, h)
    }

    pub fn key_rects(view: &View) -> Vec<(char, Rect)> {
        let name = Self::name_rect(view);
        let top = name.y + name.h + 18.0;
        let kw = ((view.w - 40.0) / 10.0 - 6.0).min(72.0);
        let room = view.h - top - 110.0;
        let kh = kw.min(68.0).min(room / 3.0 - 8.0).max(36.0);
        let mut out = vec![];
        for (r, row) in KEYS.iter().enumerate() {
            let n = row.len() as f32;
            let left = view.w / 2.0 - (n * (kw + 6.0) - 6.0) / 2.0;
            for (i, ch) in row.chars().enumerate() {
                out.push((
                    ch,
                    Rect::new(
                        left + i as f32 * (kw + 6.0),
                        top + r as f32 * (kh + 8.0),
                        kw,
                        kh,
                    ),
                ));
            }
        }
        let last = out.last().unwrap().1;
        out.push(('<', Rect::new(last.x + last.w + 6.0, last.y, kw * 1.7, kh)));
        out
    }

    pub fn action_rects(view: &View) -> (Rect, Rect, Rect) {
        let keys = Self::key_rects(view);
        let bottom = keys.iter().map(|(_, r)| r.y + r.h).fold(0.0, f32::max) + 16.0;
        let y = bottom.min(view.h - 92.0);
        let w = 260.0f32.min(view.w / 2.0 - 30.0);
        (
            Rect::new(view.w / 2.0 - w / 2.0, y, w, 84.0),
            Rect::new(view.w / 2.0 - w / 2.0 - w - 20.0, y + 10.0, w * 0.8, 70.0),
            Rect::new(view.w / 2.0 + w / 2.0 + 20.0, y + 10.0, w * 0.8, 70.0),
        )
    }

    pub fn update(&mut self, input: &Input, view: &View, dt: f32) {
        self.t += dt;
        if let Some(l) = &mut self.link {
            l.update(input, view, dt);
            if l.done {
                self.link = None;
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
        match self.mode.clone() {
            Mode::Pick => self.update_pick(input, view),
            Mode::Edit {
                id,
                name,
                avatar,
                confirm_delete,
            } => self.update_edit(input, view, id, name, avatar, confirm_delete),
        }
    }

    fn hit_rect(&self, view: &View, p: glam::Vec2) -> Option<Rect> {
        let mut all: Vec<Rect> = match &self.mode {
            Mode::Pick => {
                let mut v = Self::card_rects(view, profile::list().len());
                v.push(Self::link_rect(view));
                v
            }
            Mode::Edit { .. } => {
                let mut v: Vec<Rect> = Self::key_rects(view).into_iter().map(|k| k.1).collect();
                let (a, b, c) = Self::action_rects(view);
                v.extend([a, b, c]);
                v.extend(Self::avatar_rects(view));
                v
            }
        };
        all.retain(|r| r.contains(p));
        all.first().copied()
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
            let edit = Rect::new(r.x + r.w - 50.0, r.y - 6.0, 56.0, 56.0);
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
        if input.pressed || submit {
            for (i, r) in Self::avatar_rects(view).iter().enumerate() {
                if input.pressed && r.contains(input.pos) {
                    avatar = i;
                    sfx::play(Sfx::Pop);
                }
            }
            for (ch, r) in Self::key_rects(view) {
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
            let (ok, back, delete) = Self::action_rects(view);
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
            if input.pressed && back.contains(input.pos) && !profile::list().is_empty() {
                sfx::play(Sfx::Tap);
                self.mode = Mode::Pick;
                return;
            }
            if let (true, Some(pid)) = (input.pressed && delete.contains(input.pos), &id) {
                if confirm_delete {
                    profile::delete(pid);
                    sfx::play(Sfx::Crunch);
                    self.mode = if profile::list().is_empty() {
                        Mode::Edit {
                            id: None,
                            name: String::new(),
                            avatar: 0,
                            confirm_delete: false,
                        }
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
            Mode::Pick => {
                pen.ribbon(view, "Who's playing?", vec2(view.w / 2.0, 100.0), self.t);
                let link = Self::link_rect(view);
                pen.painted_button(link, "Link device", Tone::Blue, pressed(link), 1.0);
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
                        pen.text_outline(&p.name, c.x, r.y + r.h - 4.0, 30.0, WHITE, plum);
                        let e = vec2(r.x + r.w - 22.0, r.y + 22.0);
                        pen.painted_round(
                            Rect::new(e.x - 26.0, e.y - 26.0, 52.0, 52.0),
                            "icon_gear",
                            0.0,
                        );
                    } else {
                        pen.circle(c, d * 0.45, Color::new(1.0, 1.0, 1.0, 0.25));
                        pen.text_outline("+", c.x, c.y - 4.0, d * 0.5, WHITE, plum);
                        pen.text_outline("New player", c.x, r.y + r.h - 4.0, 28.0, WHITE, plum);
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
                let ry = if view.w > view.h { 66.0 } else { 90.0 };
                pen.ribbon(view, title, vec2(view.w / 2.0, ry), self.t);
                pen.text_center(
                    "Pick your animal",
                    view.w / 2.0,
                    Self::avatar_rects(view)[0].y - 28.0,
                    30.0,
                    WHITE,
                );
                for (i, r) in Self::avatar_rects(view).iter().enumerate() {
                    let on = i == *sel;
                    let k = if on {
                        1.12 + (self.t * 5.0).sin() * 0.03
                    } else {
                        0.92
                    };
                    if on {
                        pen.circle(r.center(), r.w * 0.62, Color::new(1.0, 0.9, 0.3, 0.7));
                    }
                    pen.avatar(i, r.center(), r.w * k);
                }
                let nr = Self::name_rect(view);
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
                    40.0,
                    if name.is_empty() {
                        Color::new(0.4, 0.35, 0.5, 0.6)
                    } else {
                        BODY
                    },
                );
                for (ch, r) in Self::key_rects(view) {
                    let label = if ch == '<' {
                        "DEL".to_string()
                    } else {
                        ch.to_string()
                    };
                    pen.key_cap(r, &label, pressed(r));
                }
                let (ok, back, delete) = Self::action_rects(view);
                let ready = !name.trim().is_empty();
                pen.painted_button(
                    ok,
                    if id.is_some() { "Save" } else { "Let's go!" },
                    Tone::Green,
                    pressed(ok),
                    if ready { 1.0 } else { 0.45 },
                );
                if !profile::list().is_empty() {
                    pen.painted_button(back, "Back", Tone::Blue, pressed(back), 1.0);
                }
                if id.is_some() {
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

pub const CHIP_H: f32 = 60.0;

pub fn chip_label(name: &str) -> String {
    format!("Playing as {name}")
}

pub fn player_chip_at(view: &View, y: f32) -> Rect {
    let name = profile::current()
        .map(|p| p.name)
        .unwrap_or_else(|| "Guest".into());
    let w = measure(&chip_label(&name), 26.0) + 130.0;
    Rect::new(view.w / 2.0 - w / 2.0, y, w, CHIP_H)
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
    fn a_hardware_keyboard_types_the_name_and_enter_saves() {
        kg_core::save::reset_for_test();
        let v = View::ipad_landscape();
        let mut p = Profiles::new();
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
    fn edit_layout_fits_every_screen_without_overlap() {
        for v in [
            View::ipad_landscape(),
            View::ipad_portrait(),
            View::fit(390.0, 844.0),
            View::fit(844.0, 390.0),
        ] {
            let keys = Profiles::key_rects(&v);
            let (ok, _, _) = Profiles::action_rects(&v);
            let name = Profiles::name_rect(&v);
            let last_av = *Profiles::avatar_rects(&v).last().unwrap();
            assert!(
                last_av.y + last_av.h <= name.y,
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
