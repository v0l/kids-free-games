use crate::engine::*;
use crate::net::{self, Code, Redeem};
use crate::sfx::{self, Sfx};
use crate::ui;
use macroquad::prelude::*;

const CODE_KEYS: [&str; 4] = ["ABCDEFGH", "JKLMNPQR", "STUVWXYZ", "23456789"];
const CODE_LEN: usize = 8;

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

    pub fn tab_rects(view: &View) -> (Rect, Rect) {
        let w = ((view.w - 60.0) / 2.0).min(280.0);
        let y = if view.w > view.h { 110.0 } else { 150.0 };
        (
            Rect::new(view.w / 2.0 - w - 8.0, y, w, 64.0),
            Rect::new(view.w / 2.0 + 8.0, y, w, 64.0),
        )
    }

    pub fn back_rect(view: &View) -> Rect {
        Rect::new(view.w / 2.0 - 110.0, view.h - 96.0, 220.0, 72.0)
    }

    pub fn action_rect(view: &View) -> Rect {
        let back = Self::back_rect(view);
        Rect::new(back.x, back.y - 84.0, back.w, 72.0)
    }

    pub fn code_rect(view: &View) -> Rect {
        let tabs = Self::tab_rects(view).0;
        let w = (view.w - 60.0).min(560.0);
        Rect::new(view.w / 2.0 - w / 2.0, tabs.y + tabs.h + 24.0, w, 96.0)
    }

    pub fn key_rects(view: &View) -> Vec<(char, Rect)> {
        let code = Self::code_rect(view);
        let top = code.y + code.h + 16.0;
        let action = Self::action_rect(view);
        let room = action.y - top - 12.0;
        let kw = ((view.w - 40.0) / 8.0 - 6.0).min(70.0);
        let kh = kw.min(room / 4.0 - 6.0).max(32.0);
        let mut out = vec![];
        for (r, row) in CODE_KEYS.iter().enumerate() {
            let left = view.w / 2.0 - (8.0 * (kw + 6.0) - 6.0) / 2.0;
            for (i, ch) in row.chars().enumerate() {
                out.push((
                    ch,
                    Rect::new(
                        left + i as f32 * (kw + 6.0),
                        top + r as f32 * (kh + 6.0),
                        kw,
                        kh,
                    ),
                ));
            }
        }
        out
    }

    pub fn del_rect(view: &View) -> Rect {
        let code = Self::code_rect(view);
        Rect::new(code.x + code.w - 90.0, code.y + 16.0, 80.0, code.h - 32.0)
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
        if input.back {
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
        if input.tapped_in(Self::action_rect(view)) {
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
        if input.tapped_in(Self::action_rect(view)) && self.typed.len() == CODE_LEN {
            sfx::play(Sfx::Pop);
            net::redeem(&self.typed);
        }
    }

    pub fn draw(&self, view: &View) {
        let ry = if view.w > view.h { 56.0 } else { 90.0 };
        ui::ribbon(view, "Link a device", vec2(view.w / 2.0, ry), self.t);
        let (share, join) = Self::tab_rects(view);
        ui::painted_button(
            share,
            "Show code",
            if self.tab == Tab::Share {
                "green"
            } else {
                "blue"
            },
            0.0,
            if self.tab == Tab::Share { 1.0 } else { 0.7 },
        );
        ui::painted_button(
            join,
            "Type code",
            if self.tab == Tab::Join {
                "green"
            } else {
                "blue"
            },
            0.0,
            if self.tab == Tab::Join { 1.0 } else { 0.7 },
        );
        let code = Self::code_rect(view);
        ui::painted_panel(code, WHITE);
        let action = Self::action_rect(view);
        match self.tab {
            Tab::Share => {
                let (text, size, note) = match net::code() {
                    Code::Ready { code, .. } => (
                        code,
                        64.0,
                        "On the other device, tap Type code and enter this. It works for 10 minutes.",
                    ),
                    Code::Failed => ("No internet".to_string(), 44.0, "Try again in a moment."),
                    _ => ("...".to_string(), 64.0, "Getting your code"),
                };
                text_center(&text, code.center().x, code.center().y, size, ui::BODY);
                text_center(note, view.w / 2.0, code.y + code.h + 36.0, 26.0, WHITE);
                ui::painted_button(action, "New code", "orange", 0.0, 1.0);
            }
            Tab::Join => {
                let shown = if self.typed.is_empty() {
                    "Type the code".to_string()
                } else {
                    self.shown_code()
                };
                text_center(
                    &shown,
                    code.center().x - 30.0,
                    code.center().y,
                    if self.typed.is_empty() { 40.0 } else { 60.0 },
                    if self.typed.is_empty() {
                        Color::new(0.4, 0.35, 0.5, 0.6)
                    } else {
                        ui::BODY
                    },
                );
                let del = Self::del_rect(view);
                round_rect(del, 12.0, Color::from_rgba(255, 190, 120, 255));
                text_center("DEL", del.center().x, del.center().y, del.h * 0.4, ui::BODY);
                for (ch, r) in Self::key_rects(view) {
                    round_rect(
                        Rect::new(r.x, r.y + 4.0, r.w, r.h),
                        12.0,
                        Color::new(0.1, 0.03, 0.25, 0.35),
                    );
                    round_rect(r, 12.0, Color::new(1.0, 1.0, 1.0, 0.95));
                    text_center(
                        &ch.to_string(),
                        r.center().x,
                        r.center().y,
                        r.h * 0.5,
                        ui::BODY,
                    );
                }
                let ready = self.typed.len() == CODE_LEN;
                let (label, alpha) = match net::redeem_state() {
                    Redeem::Working => ("Checking...", 0.6),
                    _ => ("Join", if ready { 1.0 } else { 0.45 }),
                };
                ui::painted_button(action, label, "green", 0.0, alpha);
                if let Redeem::Failed(why) = net::redeem_state() {
                    text_center(
                        why,
                        view.w / 2.0,
                        Self::key_rects(view)
                            .first()
                            .map(|k| k.1.y - 14.0)
                            .unwrap_or(0.0),
                        26.0,
                        Color::from_rgba(255, 230, 120, 255),
                    );
                }
            }
        }
        ui::painted_button(Self::back_rect(view), "Back", "blue", 0.0, 1.0);
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

    fn tap(l: &mut Link, v: &View, at: Vec2) {
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
    fn back_leaves_the_screen() {
        let v = View::ipad_portrait();
        let mut l = Link::new();
        tap(&mut l, &v, Link::back_rect(&v).center());
        assert!(l.done);
    }
}
