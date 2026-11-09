use crate::paint::Pen;
use crate::ui::{Tone, BODY};
use glam::vec2;
use kg_core::engine::*;
use kg_core::geom::{Color, Rect, WHITE};
use kg_core::net::{self, Code, Redeem};
use kg_core::sfx::{self, Sfx};

const CODE_KEYS: [&str; 4] = ["ABCDEFGH", "JKLMNPQR", "STUVWXYZ", "23456789"];
const CODE_LEN: usize = 8;
pub const TITLE: &str = "Link a device";
const SHARE_NOTE: &str =
    "On the other device, tap Type code and enter this. It works for 10 minutes.";

#[derive(Clone, Debug, Default)]
pub struct LinkLayout {
    pub ribbon: Rect,
    pub back: Rect,
    pub tabs: (Rect, Rect),
    pub code: Rect,
    pub del: Rect,
    pub note_y: f32,
    pub keys: Vec<(char, Rect)>,
    pub action: Rect,
    pub share_action: Rect,
}

impl LinkLayout {
    pub fn buttons(&self) -> Vec<Rect> {
        vec![self.back, self.tabs.0, self.tabs.1, self.del, self.action]
    }
}

fn link_try(view: &View, split: bool, max_cols: usize, compact: bool) -> LinkLayout {
    let gap = view.gap();
    let m = view.edge().min(16.0);
    let (left, right) = if split {
        let rows = kg_core::sizing::key_rows_that_fit(view, view.h - 2.0 * m).max(1);
        let cols = 32usize.div_ceil(rows).max(max_cols.min(8)) as f32;
        let need = cols * view.key_w().max(28.0) + (cols - 1.0) * view.key_gap() + 2.0 * m;
        let w = (view.w - need).min(view.w * 0.55).max(view.w * 0.3);
        (
            Rect::new(0.0, 0.0, w, view.h),
            Rect::new(w, 0.0, view.w - w, view.h),
        )
    } else {
        (view.rect(), view.rect())
    };
    let inner = Rect::new(left.x + m, 0.0, left.w - 2.0 * m, 0.0);
    let th = view.tap(64.0);
    let cx = left.x + left.w / 2.0;
    let (ribbon, back, tabs, mut y) = if compact {
        let back = Rect::new(inner.x, 8.0, view.touch(), th);
        let rest = Rect::new(
            back.x + back.w + gap,
            8.0,
            inner.x + inner.w - back.x - back.w - gap,
            th,
        );
        if (rest.w - gap) / 2.0 >= view.touch() * 1.6 {
            let r = kg_core::sizing::row(rest, 2, gap);
            (Rect::default(), back, (r[0], r[1]), 8.0 + th + gap)
        } else {
            let r = kg_core::sizing::row(Rect::new(inner.x, 8.0 + th + gap, inner.w, th), 2, gap);
            (Rect::default(), back, (r[0], r[1]), 8.0 + 2.0 * (th + gap))
        }
    } else {
        let bar = kg_core::sizing::title_bar_in(view, left, TITLE, Some("Back"), None);
        let y = bar.bottom + gap;
        let tw = ((inner.w - gap) / 2.0).min(280.0 * view.text_k());
        let tabs = (
            Rect::new(cx - tw - gap / 2.0, y, tw, th),
            Rect::new(cx + gap / 2.0, y, tw, th),
        );
        (bar.ribbon, bar.left.unwrap_or_default(), tabs, y + th + gap)
    };
    let cw = if view.portrait() {
        inner.w
    } else {
        inner.w.min(560.0 * view.text_k())
    };
    let ch = view.touch() + 2.0 * view.key_gap();
    let code = Rect::new(cx - cw / 2.0, y, cw, ch);
    let dw = view.tap(80.0).max(view.touch());
    let dh = view.touch();
    let del = Rect::new(
        code.x + code.w - dw - view.key_gap(),
        code.y + (ch - dh) / 2.0,
        dw,
        dh,
    );
    y += ch + gap;
    let note_y = y + view.text(26.0) * 0.7;
    let (ah_w, ah) = kg_core::sizing::label_button(view, "Checking...", 72.0);
    let (keys, action) = if split {
        let pad = Rect::new(right.x + m, m, right.w - 2.0 * m, view.h);
        let k = kg_core::sizing::keypad(view, pad, 32, max_cols, 70.0);
        let dy = ((view.h - (k.bottom - m)) / 2.0 - m).max(0.0);
        let keys = k.keys.iter().map(|r| r.offset(vec2(0.0, dy))).collect();
        let aw = ah_w.max(inner.w * 0.6).min(inner.w);
        let action = Rect::new(cx - aw / 2.0, view.h - m - ah, aw, ah);
        (keys, action)
    } else {
        let ky = y + if compact { 0.0 } else { view.text(26.0) * 1.4 };
        let pad = Rect::new(inner.x, ky, inner.w, view.h);
        let k = kg_core::sizing::keypad(view, pad, 32, max_cols, 70.0);
        let aw = ah_w.max(inner.w * 0.6).min(inner.w);
        let action = Rect::new(cx - aw / 2.0, k.bottom + gap, aw, ah);
        (k.keys, action)
    };
    let chars = CODE_KEYS.concat();
    let ns = view.text(26.0);
    let note_lines = kg_core::sizing::wrap(SHARE_NOTE, ns, code.w).len() as f32;
    let sy = (note_y + note_lines * ns * 1.2 + gap).min(view.h - ah - m);
    let sw = ah_w.max(inner.w * 0.6).min(inner.w);
    let share_action = Rect::new(cx - sw / 2.0, sy, sw, ah);
    LinkLayout {
        ribbon,
        back,
        tabs,
        code,
        del,
        note_y,
        keys: chars.chars().zip(keys).collect(),
        action,
        share_action,
    }
}

fn fits(view: &View, l: &LinkLayout) -> bool {
    let bottom = l
        .keys
        .iter()
        .map(|k| k.1.y + k.1.h)
        .fold(l.action.y + l.action.h, f32::max);
    let above = l.code.y + l.code.h <= l.action.y;
    bottom <= view.h - 4.0 && above
}

fn link_layout(view: &View, split: bool) -> Option<LinkLayout> {
    [false, true].iter().find_map(|c| {
        [8, 11, 16]
            .into_iter()
            .map(|n| link_try(view, split, n, *c))
            .find(|l| fits(view, l))
    })
}

fn link_layout_any(view: &View) -> LinkLayout {
    link_try(view, true, 16, true)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tab {
    Share,
    Join,
}

pub struct Link {
    pub tab: Tab,
    pub typed: String,
    pub done: bool,
    pub t: f32,
    asked: bool,
}

impl Link {
    pub fn new() -> Self {
        net::clear_code();
        net::clear_redeem();
        Self {
            tab: Tab::Share,
            typed: String::new(),
            done: false,
            t: 0.0,
            asked: false,
        }
    }

    pub fn layout(view: &View) -> LinkLayout {
        [false, true]
            .iter()
            .find_map(|s| link_layout(view, *s))
            .unwrap_or_else(|| link_layout_any(view))
    }

    pub fn tab_rects(view: &View) -> (Rect, Rect) {
        Self::layout(view).tabs
    }

    pub fn back_rect(view: &View) -> Rect {
        Self::layout(view).back
    }

    pub fn action_rect(view: &View) -> Rect {
        Self::layout(view).action
    }

    pub fn code_rect(view: &View) -> Rect {
        Self::layout(view).code
    }

    pub fn key_rects(view: &View) -> Vec<(char, Rect)> {
        Self::layout(view).keys
    }

    pub fn del_rect(view: &View) -> Rect {
        Self::layout(view).del
    }

    pub fn shown_code(&self) -> String {
        let mut s = self.typed.clone();
        if s.len() > 4 {
            s.insert(4, '-');
        }
        s
    }

    pub fn update(&mut self, input: &Input, view: &View, dt: f32) {
        self.t += dt;
        let erasing = self.tab == Tab::Join && input.typed.contains(&crate::profiles::BACKSPACE);
        if erasing {
            self.typed.pop();
        }
        if input.back && !erasing {
            self.done = true;
            return;
        }
        let (share, join) = Self::tab_rects(view);
        if input.tapped_in(share) && self.tab != Tab::Share {
            self.tab = Tab::Share;
            sfx::play(Sfx::Tap);
        }
        if input.tapped_in(join) && self.tab != Tab::Join {
            self.tab = Tab::Join;
            sfx::play(Sfx::Tap);
        }
        if input.tapped_in(Self::back_rect(view)) {
            sfx::play(Sfx::Tap);
            self.done = true;
            return;
        }
        match self.tab {
            Tab::Share => self.update_share(input, view),
            Tab::Join => self.update_join(input, view),
        }
    }

    fn update_share(&mut self, input: &Input, view: &View) {
        if !self.asked {
            self.asked = true;
            net::request_code();
        }
        if input.tapped_in(Self::layout(view).share_action) {
            sfx::play(Sfx::Pop);
            net::request_code();
        }
    }

    fn update_join(&mut self, input: &Input, view: &View) {
        if net::redeem_state() == Redeem::Done {
            self.done = true;
            return;
        }
        if net::redeem_state() == Redeem::Working {
            return;
        }
        if input.tapped_in(Self::del_rect(view)) {
            self.typed.pop();
            sfx::play(Sfx::Click);
        }
        for (ch, r) in Self::key_rects(view) {
            if input.tapped_in(r) && self.typed.len() < CODE_LEN {
                self.typed.push(ch);
                sfx::play(Sfx::Click);
            }
        }
        for ch in &input.typed {
            let ch = ch.to_ascii_uppercase();
            if CODE_KEYS.iter().any(|row| row.contains(ch)) && self.typed.len() < CODE_LEN {
                self.typed.push(ch);
                sfx::play(Sfx::Click);
            }
        }
        if input.tapped_in(Self::action_rect(view)) && self.typed.len() == CODE_LEN {
            sfx::play(Sfx::Pop);
            net::redeem(&self.typed);
        }
    }

    pub fn draw(&self, pen: &mut Pen, view: &View) {
        let lay = Self::layout(view);
        if lay.ribbon.w > 0.0 {
            pen.ribbon_in(view, TITLE, lay.ribbon, self.t);
        }
        let (share, join) = Self::tab_rects(view);
        let tab = |on: bool| {
            if on {
                (Tone::Green, 1.0)
            } else {
                (Tone::Blue, 0.7)
            }
        };
        let (t, a) = tab(self.tab == Tab::Share);
        pen.painted_button(share, "Show code", t, 0.0, a);
        let (t, a) = tab(self.tab == Tab::Join);
        pen.painted_button(join, "Type code", t, 0.0, a);
        let code = lay.code;
        pen.painted_panel(code, WHITE);
        let action = lay.action;
        match self.tab {
            Tab::Share => {
                let (text, size, note) = match net::code() {
                    Code::Ready { code, .. } => (code, 64.0, SHARE_NOTE),
                    Code::Failed => ("No internet".to_string(), 44.0, "Try again in a moment."),
                    _ => ("...".to_string(), 64.0, "Getting your code"),
                };
                let size = kg_core::sizing::fit_size(
                    &text,
                    view.text(size).min(code.h * 0.6),
                    code.w - 30.0,
                );
                pen.text_center(&text, code.center().x, code.center().y, size, BODY);
                let ns = view.text(26.0);
                let lines = kg_core::sizing::wrap(note, ns, code.w);
                for (n, l) in lines.iter().enumerate() {
                    pen.text_center(
                        l,
                        code.center().x,
                        lay.note_y + n as f32 * ns * 1.2,
                        ns,
                        WHITE,
                    );
                }
                pen.painted_button(lay.share_action, "New code", Tone::Orange, 0.0, 1.0);
            }
            Tab::Join => {
                let empty = self.typed.is_empty();
                let shown = if empty {
                    "Type the code".to_string()
                } else {
                    self.shown_code()
                };
                let shown_size = if empty { 40.0 } else { 60.0 };
                let room = code.w - lay.del.w - 40.0;
                let shown_size = kg_core::sizing::fit_size(
                    &shown,
                    view.text(shown_size).min(code.h * 0.55),
                    room,
                );
                pen.text_center(
                    &shown,
                    code.x + room / 2.0 + 20.0,
                    code.center().y,
                    shown_size,
                    if empty {
                        Color::new(0.4, 0.35, 0.5, 0.6)
                    } else {
                        BODY
                    },
                );
                let del = Self::del_rect(view);
                pen.round_rect(del, 12.0, Color::from_rgba(255, 190, 120, 255));
                pen.text_center("DEL", del.center().x, del.center().y, del.h * 0.4, BODY);
                for (ch, r) in Self::key_rects(view) {
                    pen.key_cap(r, &ch.to_string(), 0.0);
                }
                let ready = self.typed.len() == CODE_LEN;
                let (label, alpha) = match net::redeem_state() {
                    Redeem::Working => ("Checking...", 0.6),
                    _ => ("Join", if ready { 1.0 } else { 0.45 }),
                };
                pen.painted_button(action, label, Tone::Green, 0.0, alpha);
                if let Redeem::Failed(why) = net::redeem_state() {
                    pen.text_center(
                        why,
                        code.center().x,
                        lay.note_y,
                        kg_core::sizing::fit_size(why, view.text(26.0), code.w),
                        Color::from_rgba(255, 230, 120, 255),
                    );
                }
            }
        }
        if lay.back.w < lay.back.h * 1.2 {
            pen.painted_round(lay.back, "icon_home", 0.0);
        } else {
            pen.painted_button(lay.back, "Back", Tone::Blue, 0.0, 1.0);
        }
    }
}

impl Default for Link {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tap(l: &mut Link, v: &View, at: glam::Vec2) {
        l.update(&Input::press_at(at), v, 1.0 / 60.0);
        l.update(&Input::default(), v, 1.0 / 60.0);
    }

    fn views() -> [View; 4] {
        [
            View::ipad_landscape(),
            View::ipad_portrait(),
            View::fit(390.0, 844.0),
            View::fit(844.0, 390.0),
        ]
    }

    #[test]
    fn the_keyboard_only_has_letters_the_server_issues() {
        let all: String = CODE_KEYS.concat();
        assert_eq!(all.len(), 32);
        for bad in ['0', '1', 'O', 'I'] {
            assert!(!all.contains(bad));
        }
    }

    #[test]
    fn typing_builds_a_dashed_code_up_to_eight_characters() {
        let v = View::ipad_landscape();
        let mut l = Link::new();
        tap(&mut l, &v, Link::tab_rects(&v).1.center());
        assert_eq!(l.tab, Tab::Join);
        for want in "K7QM2XPDZ".chars() {
            if let Some((_, r)) = Link::key_rects(&v).into_iter().find(|(c, _)| *c == want) {
                tap(&mut l, &v, r.center());
            }
        }
        assert_eq!(l.typed, "K7QM2XPD");
        assert_eq!(l.shown_code(), "K7QM-2XPD");
        tap(&mut l, &v, Link::del_rect(&v).center());
        assert_eq!(l.typed, "K7QM2XP");
    }

    #[test]
    fn phone_portrait_link_is_one_column() {
        for v in [kg_core::sizing::phone_portrait(), View::fit(390.0, 844.0)] {
            let l = Link::layout(&v);
            assert!(l.tabs.0.w > v.w * 0.3, "{v:?} {l:?}");
        }
    }

    #[test]
    fn link_screen_is_twelve_mm_on_every_screen() {
        use kg_core::sizing::*;
        for (name, v) in test_views() {
            let l = Link::layout(&v);
            let keys: Vec<Rect> = l.keys.iter().map(|k| k.1).collect();
            check_keys(&v, name, &keys).unwrap();
            check_buttons(&v, name, &l.buttons()).unwrap();
            check_buttons(&v, name, &[l.share_action]).unwrap();
            let mut all = keys.clone();
            all.extend([l.back, l.tabs.0, l.tabs.1, l.code, l.action]);
            if l.ribbon.w > 0.0 {
                all.push(l.ribbon.scaled_about_center(0.9));
            }
            check_apart(name, &all).unwrap();
            check_apart(name, &[l.back, l.tabs.0, l.tabs.1, l.code, l.share_action]).unwrap();
            check_inside(&v, name, l.ribbon).unwrap();
            check_inside(&v, name, l.code).unwrap();
            assert!(l.code.contains(l.del.center()), "{name}");
            if v.portrait() {
                assert!(l.code.w >= v.short() * 0.85, "{name} code box is narrow");
            }
        }
    }

    #[test]
    fn layout_fits_every_screen_without_overlap() {
        for v in views() {
            let keys = Link::key_rects(&v);
            let action = Link::action_rect(&v);
            let code = Link::code_rect(&v);
            let back = Link::back_rect(&v);
            assert!(!action.overlaps(&back), "{v:?}");
            assert!(back.y + back.h <= v.h + 1.0, "{v:?}");
            for (i, (_, a)) in keys.iter().enumerate() {
                assert!(a.x >= 0.0 && a.x + a.w <= v.w, "key off screen in {v:?}");
                assert!(!a.overlaps(&code), "key over the code box in {v:?}");
                assert!(!a.overlaps(&action), "key over the button in {v:?}");
                for (_, b) in &keys[i + 1..] {
                    assert!(!a.overlaps(b), "keys overlap in {v:?}");
                }
            }
        }
    }

    #[test]
    fn typed_letters_fill_the_code_too() {
        let v = View::ipad_landscape();
        let mut l = Link::new();
        tap(&mut l, &v, Link::tab_rects(&v).1.center());
        l.update(
            &Input {
                typed: "k7qm2xpd".chars().collect(),
                ..Default::default()
            },
            &v,
            0.016,
        );
        assert_eq!(l.typed, "K7QM2XPD");
    }

    #[test]
    fn back_leaves_the_screen() {
        let v = View::ipad_portrait();
        let mut l = Link::new();
        tap(&mut l, &v, Link::back_rect(&v).center());
        assert!(l.done);
    }
}
