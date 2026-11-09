use crate::draw::{measure, Draw};
use crate::games::{self, Entry, Play};
use crate::paint::Pen;
use crate::ui::{ReportHit, Tone};
use glam::{vec2, Vec2};
use kg_core::engine::*;
use kg_core::feel::Feel;
use kg_core::fx::*;
use kg_core::games::Kind;
use kg_core::geom::{Color, Rect, WHITE};
use kg_core::sfx::{self, Sfx};

pub fn home_rect(view: &View) -> Rect {
    view.home()
}

pub fn music_rect(view: &View) -> Rect {
    view.home()
}
const LAUNCH_TIME: f32 = 0.22;
const WAVE: f32 = 6.0;
const TAG_SIZE: f32 = 26.0;
pub const TAGLINE: &str = "No ads. No tracking. Just play.";
pub const TITLE: &str = "Learn and Play Kids Games";
pub const LABEL_H: f32 = 46.0;
pub const SCROLL_ROOM: f32 = 1.3;
pub const TILE_MM: f32 = 22.0;

pub fn mute_rect(view: &View) -> Rect {
    view.top_right(0)
}

pub fn label_h(view: &View) -> f32 {
    view.tap(LABEL_H)
}

pub struct Header {
    pub title_size: f32,
    pub title_y: f32,
    pub title_band: (f32, f32),
    pub tag: Rect,
    pub player: Rect,
    pub bottom: f32,
}

pub fn header(view: &View) -> Header {
    let fit = view.w * 0.9 / measure(TITLE, 1.0);
    let title_size = (view.w / 13.0).min(64.0).min(fit);
    let reach = title_size * (0.5 + kg_core::outline::OUTLINE_FRAC + 0.08) + WAVE;
    let corner = view.home();
    let clear = corner.y + corner.h + 4.0;
    let wide = view.w / 2.0 + measure(TITLE, title_size) / 2.0 + 8.0 < view.top_right(0).x;
    let title_y = if wide { 14.0 + reach } else { clear + reach };
    let title_band = (title_y - reach, title_y + reach);
    let tag_size = TAG_SIZE.min(kg_core::sizing::fit_size(TAGLINE, TAG_SIZE, view.w - 80.0));
    let tag_h = tag_size + 14.0;
    let tag_w = measure(TAGLINE, tag_size) + 40.0;
    let mut tag = Rect::new(view.w / 2.0 - tag_w / 2.0, title_band.1 + 8.0, tag_w, tag_h);
    let mut player = crate::profiles::player_chip_at(view, tag.y + tag.h + 12.0);
    let gap = 20.0;
    if tag.w + player.w + gap <= view.w - 40.0 {
        let left = view.w / 2.0 - (tag.w + player.w + gap) / 2.0;
        let py = tag.y + tag.h / 2.0 - player.h / 2.0;
        if py >= title_band.1 {
            tag.x = left;
            player.x = left + tag.w + gap;
            player.y = py;
        }
    }
    Header {
        title_size,
        title_y,
        title_band,
        tag,
        player,
        bottom: (player.y + player.h).max(tag.y + tag.h),
    }
}

pub struct Layout {
    pub tiles: Vec<Rect>,
    pub labels: Vec<(Rect, Kind)>,
}

fn sections(kinds: &[Kind]) -> Vec<(Kind, usize)> {
    let mut out: Vec<(Kind, usize)> = vec![];
    for k in kinds {
        match out.last_mut() {
            Some((last, n)) if last == k => *n += 1,
            _ => out.push((*k, 1)),
        }
    }
    out
}

pub fn layout(view: &View, kinds: &[Kind]) -> Layout {
    let pad = 22.0;
    let top = header(view).bottom + pad * 0.5;
    let secs = sections(kinds);
    let label_space = secs.len() as f32 * label_h(view);
    let min_tile = view.from_mm(TILE_MM).max(view.short() * 0.16);
    let by_w = |cols: usize| (view.w - pad * (cols as f32 + 1.0)) / cols as f32;
    let fit = |cols: usize| {
        let rows: usize = secs.iter().map(|(_, n)| n.div_ceil(cols)).sum();
        let by_h = ((view.h * SCROLL_ROOM - top - label_space - pad) / rows as f32 - pad) / 1.18;
        by_w(cols).min(by_h.max(min_tile))
    };
    let cols = (2..=6)
        .filter(|c| *c == 2 || by_w(*c) >= min_tile)
        .max_by(|a, b| fit(*a).total_cmp(&fit(*b)))
        .unwrap();
    let tw = fit(cols);
    let th = tw * 1.18;
    let left = (view.w - cols as f32 * tw - (cols as f32 - 1.0) * pad) / 2.0;
    let mut y = top;
    let mut tiles = vec![];
    let mut labels = vec![];
    for (kind, n) in secs {
        labels.push((Rect::new(left, y, view.w - left * 2.0, label_h(view)), kind));
        y += label_h(view);
        for i in 0..n {
            let (c, r) = (i % cols, i / cols);
            tiles.push(Rect::new(
                left + c as f32 * (tw + pad),
                y + r as f32 * (th + pad),
                tw,
                th,
            ));
        }
        y += n.div_ceil(cols) as f32 * (th + pad);
    }
    Layout { tiles, labels }
}

pub fn next_focus(tiles: &[Rect], from: usize, dir: Dir) -> usize {
    let c = tiles[from].center();
    let d = dir.delta().as_vec2();
    tiles
        .iter()
        .enumerate()
        .filter(|(i, _)| *i != from)
        .filter_map(|(i, r)| {
            let v = r.center() - c;
            let along = v.dot(d);
            if along <= 1.0 {
                return None;
            }
            let across = (v - d * along).length();
            Some((i, along + across * 2.0))
        })
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(i, _)| i)
        .unwrap_or(from)
}

pub struct App {
    pub entries: Vec<Entry>,
    pub current: Option<(usize, Box<dyn Play>)>,
    scroll: f32,
    drag_from: Option<(Vec2, f32)>,
    dragged: bool,
    t: f32,
    since_menu: f32,
    pressed_tile: Option<usize>,
    press: Vec<f32>,
    pub launch: Option<(usize, f32)>,
    curtain: f32,
    home_press: f32,
    fx: Particles,
    pub feel: Feel,
    pub focus: Option<usize>,
    pub profiles: Option<crate::profiles::Profiles>,
    soon: Option<(usize, f32)>,
    pub loading: Option<usize>,
    pub reporting: Option<(ReportHit, f32)>,
    ui_owned: bool,
    pub locked: Option<LockSheet>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct LockSheet {
    pub game: usize,
    pub age: f32,
    pub hold: f32,
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}

impl App {
    pub fn new() -> Self {
        let entries = games::all();
        let n = entries.len();
        Self {
            entries,
            current: None,
            scroll: 0.0,
            drag_from: None,
            dragged: false,
            t: 0.0,
            since_menu: 0.0,
            pressed_tile: None,
            press: vec![0.0; n],
            launch: None,
            curtain: 0.0,
            home_press: 0.0,
            fx: Particles::default(),
            feel: Feel::default(),
            focus: None,
            profiles: kg_core::profile::current_id()
                .is_none()
                .then(crate::profiles::Profiles::new),
            soon: None,
            loading: None,
            reporting: None,
            ui_owned: false,
            locked: None,
        }
    }

    pub fn open(&mut self, i: usize) {
        let Some(make) = self.entries[i].make else {
            return;
        };
        if !crate::packs::ready(self.entries[i].info.key) {
            self.loading = Some(i);
            return;
        }
        self.loading = None;
        kg_core::net::begin_play(self.entries[i].info.key);
        self.current = Some((i, make()));
        self.curtain = 1.0;
    }

    fn go_home(&mut self) {
        sfx::play(Sfx::Whoosh);
        kg_core::net::end_play();
        self.current = None;
        self.loading = None;
        self.since_menu = 0.0;
        self.curtain = 1.0;
        self.pressed_tile = None;
    }

    fn kinds(&self) -> Vec<Kind> {
        self.entries.iter().map(|e| e.info.kind).collect()
    }

    pub fn layout(&self, view: &View) -> Layout {
        layout(view, &self.kinds())
    }

    pub fn scroll_to_end(&mut self, view: &View) {
        self.scroll = self.max_scroll(view);
    }

    pub fn max_scroll(&self, view: &View) -> f32 {
        let rects = self.layout(view).tiles;
        let bottom = rects.last().map(|r| r.y + r.h + 30.0).unwrap_or(0.0);
        (bottom - view.h).max(0.0)
    }

    fn pick(&mut self, i: usize, rects: &[Rect]) {
        if self.entries[i].ready() && !kg_core::gate::open(self.entries[i].info.key) {
            sfx::play(Sfx::Tap);
            self.locked = Some(LockSheet {
                game: i,
                age: 0.0,
                hold: 0.0,
            });
            return;
        }
        let r = rects[i];
        let at = vec2(r.center().x, r.center().y - self.scroll);
        if self.entries[i].ready() {
            sfx::play(Sfx::Pop);
            if crate::packs::ready(self.entries[i].info.key) {
                self.launch = Some((i, 0.0));
            } else {
                self.loading = Some(i);
            }
            self.fx.sparkle(at, self.entries[i].info.color);
        } else {
            self.soon = Some((i, 0.0));
            sfx::play(Sfx::Wrong);
        }
    }

    pub fn update(&mut self, input: &Input, view: &View, dt: f32) {
        for (i, c) in kg_core::feel::take() {
            self.feel.hit(i, c);
        }
        let game_dt = self.feel.game_dt(dt);
        kg_core::net::update(dt, input.active());
        sfx::music(match &self.current {
            Some((i, _)) => self.entries[*i].info.kind.mood(),
            None => sfx::Mood::Menu,
        });
        self.t += dt;
        self.since_menu += dt;
        self.curtain = (self.curtain - dt * 5.0).max(0.0);
        self.fx.update(dt);
        if let Some(i) = self.loading {
            if self.current.is_none()
                && self.launch.is_none()
                && crate::packs::ready(self.entries[i].info.key)
            {
                self.loading = None;
                self.launch = Some((i, 0.0));
            }
        }
        if let Some((_, t)) = &mut self.soon {
            *t += dt;
            if *t > 1.2 {
                self.soon = None;
            }
        }
        if let Some((_, game)) = &mut self.current {
            self.home_press = approach(
                self.home_press,
                if input.down && home_rect(view).contains(input.pos) {
                    1.0
                } else {
                    0.0
                },
                20.0,
                dt,
            );
            if self.reporting.is_none() && input.tapped_in(home_rect(view)) {
                self.ui_owned = true;
            }
            if self.reporting.is_none() && (input.tapped_in(home_rect(view)) || input.back) {
                self.go_home();
                return;
            }
            if let Some((hit, age)) = &mut self.reporting {
                *age += dt;
                let r = report_rects(view);
                if input.pressed {
                    self.ui_owned = true;
                }
                if input.tapped_in(r.yes) {
                    kg_core::net::block_player(&hit.player, &hit.game);
                    sfx::play(Sfx::Pop);
                    self.reporting = None;
                } else if input.tapped_in(r.no) || input.back {
                    sfx::play(Sfx::Tap);
                    self.reporting = None;
                }
                return;
            }
            if input.pressed {
                self.ui_owned = home_rect(view).contains(input.pos)
                    || crate::ui::report_hit_at(input.pos).is_some();
                if let Some(hit) = crate::ui::report_hit_at(input.pos) {
                    sfx::play(Sfx::Tap);
                    self.reporting = Some((hit, 0.0));
                    return;
                }
            }
            if self.ui_owned {
                if input.released || !input.down {
                    self.ui_owned = false;
                }
                game.update(&input.masked(), view, game_dt);
                return;
            }
            game.update(input, view, game_dt);
            return;
        }
        if let Some(sheet) = self.locked.clone() {
            self.update_lock(sheet, input, view, dt);
            return;
        }
        if let Some(p) = &mut self.profiles {
            p.update(input, view, dt);
            if p.done {
                self.profiles = None;
                self.since_menu = 0.0;
                self.curtain = 1.0;
            }
            return;
        }
        if let Some((i, t)) = &mut self.launch {
            *t += dt;
            if *t >= LAUNCH_TIME {
                let i = *i;
                self.launch = None;
                sfx::play(Sfx::Whoosh);
                self.open(i);
            }
            return;
        }
        if input.tapped_in(mute_rect(view).offset(vec2(0.0, -self.scroll))) {
            sfx::toggle_mute();
            sfx::play(Sfx::Tap);
            return;
        }
        if input.tapped_in(music_rect(view).offset(vec2(0.0, -self.scroll))) {
            sfx::toggle_music();
            sfx::play(Sfx::Tap);
            return;
        }
        let chip = header(view).player;
        if input.tapped_in(chip.offset(vec2(0.0, -self.scroll))) {
            sfx::play(Sfx::Tap);
            let mut p = crate::profiles::Profiles::new();
            p.mode = crate::profiles::Mode::Pick;
            self.profiles = Some(p);
            return;
        }
        let rects = self.layout(view).tiles;
        if let Some(d) = input.key_dir {
            let next = match self.focus {
                None => 0,
                Some(f) => next_focus(&rects, f, d),
            };
            if Some(next) != self.focus {
                sfx::play(Sfx::Click);
            }
            self.focus = Some(next);
            let r = rects[next];
            if r.y - self.scroll < header(view).bottom {
                self.scroll = r.y - header(view).bottom - label_h(view);
            } else if r.y + r.h - self.scroll > view.h - 16.0 {
                self.scroll = r.y + r.h - view.h + 16.0;
            }
        }
        if input.action {
            if let Some(i) = self.focus {
                self.pick(i, &rects);
            }
        }
        let hit =
            |p: Vec2, scroll: f32| rects.iter().position(|r| r.contains(p + vec2(0.0, scroll)));
        if input.pressed {
            self.focus = None;
            self.drag_from = Some((input.pos, self.scroll));
            self.dragged = false;
            self.pressed_tile = hit(input.pos, self.scroll);
        }
        if let (true, Some((start, s0))) = (input.down, self.drag_from) {
            let dy = input.pos.y - start.y;
            if dy.abs() > TAP_SLOP_MM * input.mm() {
                self.dragged = true;
                self.pressed_tile = None;
            }
            if self.dragged {
                self.scroll = (s0 - dy).clamp(0.0, self.max_scroll(view));
            }
        }
        if input.released {
            if !self.dragged {
                if let Some(i) =
                    hit(input.pos, self.scroll).filter(|i| Some(*i) == self.pressed_tile)
                {
                    self.pick(i, &rects);
                }
            }
            self.drag_from = None;
            self.pressed_tile = None;
        }
        for (i, p) in self.press.iter_mut().enumerate() {
            let held = self.pressed_tile == Some(i) && input.down;
            *p = approach(*p, if held { 1.0 } else { 0.0 }, 18.0, dt);
        }
        self.scroll = self.scroll.clamp(0.0, self.max_scroll(view));
    }

    fn update_lock(&mut self, mut sheet: LockSheet, input: &Input, view: &View, dt: f32) {
        sheet.age += dt;
        let r = lock_rects(view, self.entries[sheet.game].info.name);
        let game = self.entries[sheet.game].info.key;
        sheet.hold = if input.down && r.hold.contains(input.pos) {
            sheet.hold + dt
        } else {
            0.0
        };
        if sheet.hold >= kg_core::gate::HOLD_SECS {
            kg_core::gate::set_enabled(false);
            sfx::play(Sfx::Right);
            self.locked = None;
            return;
        }
        let cost = kg_core::gate::cost();
        if input.back || input.tapped_in(r.close) {
            sfx::play(Sfx::Tap);
            self.locked = None;
            return;
        }
        if input.tapped_in(r.primary) {
            if kg_core::gate::can_afford() {
                kg_core::gate::unlock(game);
                sfx::play(Sfx::Right);
                self.locked = None;
                self.launch = Some((sheet.game, 0.0));
            } else if let Some(l) = self
                .entries
                .iter()
                .position(|e| e.info.key == kg_core::gate::first_learning_game() && e.ready())
            {
                sfx::play(Sfx::Pop);
                self.locked = None;
                self.launch = Some((l, 0.0));
            }
            let _ = cost;
            return;
        }
        self.locked = Some(sheet);
    }

    pub fn draw(&self, pen: &mut Pen, view: &View) {
        crate::ui::clear_report_hits();
        if let Some((_, game)) = &self.current {
            game.draw(pen, view);
            pen.offset = Vec2::ZERO;
            if let Some((hit, age)) = &self.reporting {
                draw_report_sheet(pen, view, hit, *age);
            }
            if self.feel.flash_alpha() > 0.0 {
                pen.fill_screen(
                    view,
                    self.feel.flash_color().with_alpha(self.feel.flash_alpha()),
                );
            }
            pen.painted_round(home_rect(view), "icon_home", self.home_press);
            self.draw_curtain(pen, view);
            return;
        }
        self.draw_backdrop(pen, view);
        if let Some(p) = &self.profiles {
            pen.fill_screen(view, Color::new(0.1, 0.04, 0.25, 0.45));
            p.draw(pen, view);
            self.draw_curtain(pen, view);
            return;
        }
        self.draw_title(pen, view);
        let up = vec2(0.0, -self.scroll);
        pen.painted_round(
            mute_rect(view).offset(up),
            if sfx::muted() {
                "icon_audio_off"
            } else {
                "icon_audio"
            },
            0.0,
        );
        pen.painted_round(
            music_rect(view).offset(up),
            if sfx::music_on() {
                "icon_music"
            } else {
                "icon_music_off"
            },
            0.0,
        );
        crate::profiles::draw_player_chip(pen, header(view).player, self.scroll);
        let lay = self.layout(view);
        for (r, kind) in &lay.labels {
            let r = r.offset(vec2(0.0, -self.scroll));
            if r.y > view.h || r.y + r.h < 0.0 {
                continue;
            }
            let a = ((self.since_menu - 0.1) / 0.25).clamp(0.0, 1.0);
            let (text, tone) = match kind {
                Kind::Arcade => ("Play", Tone::Orange),
                Kind::Learn => ("Learn", Tone::Green),
            };
            let w = (measure(text, 30.0) + 80.0).max(view.touch() * 1.6);
            pen.painted_button(Rect::new(r.x, r.y + 2.0, w, r.h - 4.0), text, tone, 0.0, a);
        }
        for (i, (r, e)) in lay.tiles.iter().zip(&self.entries).enumerate() {
            let r = r.offset(vec2(0.0, -self.scroll));
            if r.y > view.h || r.y + r.h < 0.0 {
                continue;
            }
            let intro =
                ease_out_back(((self.since_menu - 0.05 - i as f32 * 0.02) / 0.3).clamp(0.0, 1.0));
            if intro <= 0.0 {
                continue;
            }
            let focused = self.focus == Some(i);
            let bob = (self.t * 2.0 + i as f32 * 0.7).sin() * 2.0;
            let mut scale = intro * (1.0 - self.press[i] * 0.08) * if focused { 1.06 } else { 1.0 };
            if let Some((li, lt)) = self.launch {
                if li == i {
                    scale *= 1.0 + ease_out_back(lt / LAUNCH_TIME) * 0.15;
                } else {
                    scale *= 1.0 - ease_out_cubic(lt / LAUNCH_TIME) * 0.15;
                }
            }
            if let Some((si, st)) = self.soon {
                if si == i {
                    scale *= 1.0 + (st * 30.0).sin() * 0.04 * (1.0 - st / 1.2).max(0.0);
                }
            }
            let c = r.center() + vec2(0.0, bob + (1.0 - intro) * 40.0);
            let r = Rect::new(
                c.x - r.w * scale / 2.0,
                c.y - r.h * scale / 2.0,
                r.w * scale,
                r.h * scale,
            );
            if focused {
                let g = 6.0 + (self.t * 5.0).sin() * 2.0;
                pen.round_rect(
                    Rect::new(r.x - g, r.y - g, r.w + g * 2.0, r.h + g * 2.0 + r.w * 0.03),
                    r.w * 0.14 + g,
                    WHITE,
                );
            }
            draw_tile(
                pen,
                r,
                e,
                (focused || self.press[i] > 0.01).then_some(self.t),
            );
            if self.loading == Some(i) {
                let dots = ".".repeat(1 + (self.t * 3.0) as usize % 3);
                pen.hint(
                    &format!("Loading{dots}"),
                    vec2(r.center().x, r.y + r.w * 0.5),
                    1.0,
                );
            }
            if let Some((si, st)) = self.soon.filter(|(si, _)| *si == i) {
                let a = (1.0 - (st - 0.8).max(0.0) / 0.4).clamp(0.0, 1.0);
                let _ = si;
                pen.hint(
                    "Coming soon",
                    vec2(r.center().x, r.y + r.w * 0.5),
                    a.max(0.3),
                );
            }
        }
        pen.particles(&self.fx);
        if let Some((i, lt)) = self.launch {
            let k =
                ease_out_cubic(((lt - LAUNCH_TIME * 0.4) / (LAUNCH_TIME * 0.6)).clamp(0.0, 1.0));
            if k > 0.0 {
                let c = lay.tiles[i].center() - vec2(0.0, self.scroll);
                let rad = k * view.full().w.hypot(view.full().h);
                pen.circle(c, rad, self.entries[i].info.color);
            }
        }
        if let Some(sheet) = &self.locked {
            draw_lock_sheet(pen, view, sheet, self.entries[sheet.game].info.name);
        }
        self.draw_curtain(pen, view);
    }

    fn draw_backdrop(&self, pen: &mut Pen, view: &View) {
        let f = view.full();
        let name = if f.h > f.w * 1.05 {
            "bg_tall"
        } else {
            "bg_wide"
        };
        let sz = crate::art::size("menu", name);
        let k = (f.w / sz.x).max(f.h / sz.y) * 1.04;
        let (w, h) = (sz.x * k, sz.y * k);
        let drift = vec2(
            (self.t * 0.15).sin() * 8.0,
            (self.t * 0.11).cos() * 6.0 - self.scroll * 0.15,
        );
        pen.sprite(
            "menu",
            name,
            Rect::new(
                f.x + (f.w - w) / 2.0 + drift.x,
                f.y + (f.h - h) / 2.0 + drift.y,
                w,
                h,
            ),
            Draw::default(),
        );
        pen.gradient(
            Rect::new(f.x, f.y, f.w, f.h * 0.35),
            Color::new(0.05, 0.1, 0.3, 0.25),
            Color::new(0.05, 0.1, 0.3, 0.0),
        );
        pen.gradient(
            Rect::new(f.x, f.y + f.h * 0.45, f.w, f.h * 0.55),
            Color::new(0.1, 0.05, 0.25, 0.0),
            Color::new(0.1, 0.05, 0.25, 0.45),
        );
    }

    fn draw_title(&self, pen: &mut Pen, view: &View) {
        let title = TITLE;
        let h = header(view);
        let size = h.title_size;
        let total = measure(title, size);
        let mut x = view.w / 2.0 - total / 2.0;
        let y = h.title_y - self.scroll;
        for (i, ch) in title.chars().enumerate() {
            let s = ch.to_string();
            let w = measure(&s, size);
            let intro = ease_out_back(((self.since_menu - i as f32 * 0.015) / 0.3).clamp(0.0, 1.0));
            let wave = (self.t * 3.0 - i as f32 * 0.45).sin() * WAVE;
            let col = [
                Color::from_rgba(255, 214, 64, 255),
                Color::from_rgba(255, 140, 70, 255),
                Color::from_rgba(255, 100, 140, 255),
                Color::from_rgba(120, 200, 255, 255),
                Color::from_rgba(140, 230, 110, 255),
            ][i % 5];
            if ch != ' ' {
                let shadow = Color::new(0.2, 0.05, 0.3, 0.45);
                let cy = y + wave - (1.0 - intro) * 60.0;
                pen.text_outline(
                    &s,
                    x + w / 2.0,
                    cy + size * 0.06,
                    size * intro.max(0.01),
                    shadow,
                    shadow,
                );
                pen.text_outline(
                    &s,
                    x + w / 2.0,
                    cy,
                    size * intro.max(0.01),
                    col,
                    Color::from_rgba(70, 20, 90, 255),
                );
            }
            x += w;
        }
        let a = ((self.since_menu - 0.15) / 0.25).clamp(0.0, 1.0);
        let r = h.tag.offset(vec2(0.0, -self.scroll));
        pen.round_rect(r, r.h / 2.0, Color::new(0.15, 0.05, 0.3, 0.45 * a));
        pen.text_center(
            TAGLINE,
            r.center().x,
            r.y + r.h / 2.0,
            TAG_SIZE,
            WHITE.with_alpha(a),
        );
    }

    fn draw_curtain(&self, pen: &mut Pen, view: &View) {
        if self.curtain <= 0.0 {
            return;
        }
        let color = self
            .current
            .as_ref()
            .map(|(i, _)| self.entries[*i].info.color)
            .unwrap_or(Color::from_rgba(255, 160, 140, 255));
        let k = ease_out_cubic(self.curtain);
        let rad = k * view.full().w.hypot(view.full().h) * 0.6;
        pen.circle(view.center(), rad, color);
    }
}

pub struct ReportRects {
    pub sheet: kg_core::sizing::Sheet,
    pub yes: Rect,
    pub no: Rect,
}

const REPORT_NOTE: &str = "We hide them from your boards and tell a grown up.";

pub fn report_rects(view: &View) -> ReportRects {
    let mut s = kg_core::sizing::Stack::new(view, 560.0);
    s.text(38.0, "Report this player?");
    s.text(22.0, REPORT_NOTE);
    let b = s.buttons(78.0, &["Hide and report", "Cancel"]);
    let sheet = s.layout();
    ReportRects {
        yes: sheet.button(b, 0),
        no: sheet.button(b, 1),
        sheet,
    }
}

fn draw_report_sheet(pen: &mut Pen, view: &View, hit: &ReportHit, age: f32) {
    let fade = ease_out_cubic(age / 0.15);
    pen.fill_screen(view, Color::new(0.06, 0.04, 0.16, 0.6 * fade));
    let r = report_rects(view);
    pen.painted_panel(r.sheet.panel, WHITE);
    let title = format!("Report {}?", hit.name);
    pen.sheet_text(&r.sheet, 0, Some(&title), crate::ui::INK);
    pen.sheet_text(&r.sheet, 1, None, crate::ui::BODY);
    pen.painted_button(r.yes, "Hide and report", Tone::Orange, 0.0, 1.0);
    pen.painted_button(r.no, "Cancel", Tone::Blue, 0.0, 1.0);
}

pub struct LockRects {
    pub sheet: kg_core::sizing::Sheet,
    pub panel: Rect,
    pub primary: Rect,
    pub close: Rect,
    pub hold: Rect,
}

fn lock_line(cost: i64, have: i64) -> String {
    if cost == 0 {
        "Your first arcade game is free.".to_string()
    } else if have >= cost {
        format!("Spend {cost} learning stars to play it. You have {have}.")
    } else {
        format!("Learn to earn {cost} stars. You have {have}.")
    }
}

const LOCK_HOLD: &str = "Grown up? Hold to open all";

pub fn lock_rects(view: &View, name: &str) -> LockRects {
    lock_rects_for(view, name)
}

fn lock_rects_for(view: &View, name: &str) -> LockRects {
    let cost = kg_core::gate::cost();
    let have = kg_core::gate::available();
    let mut s = kg_core::sizing::Stack::new(view, 580.0);
    s.text(38.0, &format!("{name} is locked"));
    s.text(24.0, &lock_line(cost, have));
    s.text(22.0, "Get questions right, quickly, to earn stars.");
    let b = s.buttons(70.0, &["Learn to unlock", "Not now", LOCK_HOLD]);
    let sheet = s.layout();
    LockRects {
        panel: sheet.panel,
        primary: sheet.button(b, 0),
        close: sheet.button(b, 1),
        hold: sheet.button(b, 2),
        sheet,
    }
}

fn draw_lock_sheet(pen: &mut Pen, view: &View, sheet: &LockSheet, name: &str) {
    let fade = ease_out_cubic(sheet.age / 0.15);
    pen.fill_screen(view, Color::new(0.06, 0.04, 0.16, 0.6 * fade));
    let r = lock_rects_for(view, name);
    pen.painted_panel(r.panel, WHITE);
    let cost = kg_core::gate::cost();
    let have = kg_core::gate::available();
    pen.sheet_text(&r.sheet, 0, None, crate::ui::INK);
    pen.sheet_text(&r.sheet, 1, None, crate::ui::BODY);
    pen.sheet_text(&r.sheet, 2, None, crate::ui::BODY);
    if have >= cost {
        pen.painted_button(r.primary, "Unlock and play", Tone::Green, 0.0, 1.0);
    } else {
        pen.painted_button(r.primary, "Learn to unlock", Tone::Green, 0.0, 1.0);
    }
    pen.painted_button(r.close, "Not now", Tone::Blue, 0.0, 1.0);
    let held = (sheet.hold / kg_core::gate::HOLD_SECS).clamp(0.0, 1.0);
    pen.painted_button(r.hold, LOCK_HOLD, Tone::Orange, held, 0.8);
}

fn progress_bar(pen: &mut Pen, ir: Rect, progress: f32) {
    let h = ir.h * 0.1;
    let bar = Rect::new(ir.x + ir.w * 0.12, ir.y + ir.h - h * 1.7, ir.w * 0.76, h);
    pen.round_rect(bar, h / 2.0, Color::new(0.1, 0.03, 0.22, 0.6));
    if progress > 0.0 {
        let fill = Rect::new(bar.x, bar.y, (bar.w * progress).max(h), bar.h);
        pen.round_rect(fill, h / 2.0, Color::from_rgba(255, 210, 60, 255));
    }
}

fn draw_tile(pen: &mut Pen, r: Rect, e: &Entry, shine: Option<f32>) {
    let k = r.w / 160.0;
    let ir = Rect::new(r.x, r.y, r.w, r.w);
    pen.round_rect(
        Rect::new(ir.x + 2.0 * k, ir.y + 9.0 * k, ir.w, ir.h),
        ir.w * 0.22,
        Color::new(0.08, 0.02, 0.2, 0.35),
    );
    let tint = if e.ready() {
        WHITE
    } else {
        Color::new(0.75, 0.72, 0.8, 1.0)
    };
    pen.sprite(
        "icons",
        &format!("icon_{}", e.info.key),
        ir,
        Draw::tint(tint),
    );
    if let Some(t) = shine {
        pen.shine_sweep(ir, t);
    }
    if let Some(bands) = kg_core::mastery::bands_of(e.info.key) {
        progress_bar(pen, ir, kg_core::mastery::game_progress(e.info.key, bands));
    }
    if e.ready() && !kg_core::gate::open(e.info.key) {
        pen.round_rect(ir, ir.w * 0.22, Color::new(0.1, 0.03, 0.22, 0.55));
        let c = ir.center();
        let u = ir.w * 0.1;
        pen.round_rect(
            Rect::new(c.x - u * 1.5, c.y - u * 2.2, u * 3.0, u * 3.0),
            u * 1.5,
            Color::from_rgba(255, 210, 60, 255),
        );
        pen.round_rect(
            Rect::new(c.x - u * 0.9, c.y - u * 1.6, u * 1.8, u * 2.4),
            u * 0.9,
            Color::new(0.1, 0.03, 0.22, 0.9),
        );
        pen.round_rect(
            Rect::new(c.x - u * 2.2, c.y - u * 0.4, u * 4.4, u * 3.2),
            u * 0.6,
            Color::from_rgba(255, 190, 40, 255),
        );
        pen.circle(
            vec2(c.x, c.y + u * 1.0),
            u * 0.45,
            Color::new(0.35, 0.2, 0.05, 1.0),
        );
        pen.text_outline(
            "Learn to unlock",
            c.x,
            ir.y + ir.h - u * 1.2,
            (ir.w * 0.13).min(26.0),
            WHITE,
            Color::from_rgba(50, 15, 80, 255),
        );
    }
    let label_y = ir.y + ir.h + (r.h - ir.h) * 0.5;
    let size = (r.w * 0.9 / measure(e.info.name, 10.0) * 10.0).min(r.w * 0.16);
    pen.text_outline(
        e.info.name,
        r.center().x,
        label_y,
        size,
        WHITE,
        Color::from_rgba(50, 15, 80, 255),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    fn menu() -> App {
        kg_core::save::reset_for_test();
        kg_core::profile::create("Mia", 0);
        kg_core::gate::set_enabled(false);
        App::new()
    }

    #[test]
    fn game_code_on_another_thread_never_syncs_during_tests() {
        let _ = menu();
        let synced = std::thread::spawn(|| {
            kg_core::profile::create("Mia", 0);
            kg_core::net::sync_enabled()
        })
        .join()
        .unwrap();
        assert!(!synced);
    }

    #[test]
    fn tests_never_sync_to_the_live_server() {
        let _ = menu();
        assert!(!kg_core::net::sync_enabled());
        kg_core::net::update(1.0, true);
        assert!(!kg_core::net::is_online());
    }

    #[test]
    fn title_tagline_and_tiles_do_not_overlap() {
        for v in [
            View::ipad_landscape(),
            View::ipad_portrait(),
            View::fit(1300.0, 950.0),
            View::fit(1920.0, 1080.0),
            View::fit(1080.0, 2400.0),
        ] {
            let h = header(&v);
            assert!(h.title_band.0 >= 0.0, "title clipped at top in {v:?}");
            assert!(
                h.tag.y + h.tag.h <= h.player.y || h.tag.x + h.tag.w <= h.player.x,
                "tagline hits the player chip in {v:?}"
            );
            assert!(
                h.title_band.1 <= h.tag.y,
                "title runs into tagline in {v:?}"
            );
            let tiles = App::new().layout(&v).tiles;
            assert!(
                h.bottom <= tiles[0].y - 8.0,
                "tagline touches tiles in {v:?}"
            );
            assert!(
                h.title_band.0 >= home_rect(&v).y + home_rect(&v).h
                    || mute_rect(&v).x > v.w / 2.0 + measure(TITLE, h.title_size) / 2.0,
                "title runs under the mute button in {v:?}"
            );
        }
    }

    #[test]
    fn the_title_always_fits_inside_the_screen_width() {
        for v in [
            View::fit(375.0, 667.0),
            View::fit(1024.0, 768.0),
            View::ipad_portrait(),
            View::ipad_landscape(),
            View::fit(2992.0 / 2.625, 1344.0 / 2.625),
        ] {
            let h = header(&v);
            assert!(
                measure(TITLE, h.title_size) <= v.w,
                "title overflows {v:?}: {} > {}",
                measure(TITLE, h.title_size),
                v.w
            );
        }
    }

    #[test]
    fn a_tile_tap_that_wobbles_three_mm_still_opens_the_game() {
        let v = View::fit(2992.0 / 2.625, 1344.0 / 2.625);
        let mm = (495.0 / 25.4 / 2.625) / v.scale;
        let mut app = menu();
        let at = app.layout(&v).tiles[0].center();
        app.update(
            &Input {
                mm,
                ..Input::press_at(at)
            },
            &v,
            1.0 / 60.0,
        );
        let moved = at + vec2(0.0, 3.0 * mm);
        app.update(
            &Input {
                pos: moved,
                prev_pos: at,
                down: true,
                mm,
                ..Default::default()
            },
            &v,
            1.0 / 60.0,
        );
        app.update(
            &Input {
                pos: moved,
                prev_pos: moved,
                released: true,
                mm,
                ..Default::default()
            },
            &v,
            1.0 / 60.0,
        );
        assert!(app.launch.is_some());
    }

    #[test]
    fn a_drag_scrolls_instead_of_launching() {
        let v = View::ipad_portrait();
        let mut app = menu();
        let at = app.layout(&v).tiles[4].center();
        app.update(&Input::press_at(at), &v, 1.0 / 60.0);
        let to = at - vec2(0.0, 80.0);
        app.update(
            &Input {
                pos: to,
                down: true,
                mm: 5.0,
                ..Default::default()
            },
            &v,
            1.0 / 60.0,
        );
        app.update(
            &Input {
                pos: to,
                released: true,
                mm: 5.0,
                ..Default::default()
            },
            &v,
            1.0 / 60.0,
        );
        assert!(app.launch.is_none());
        assert!(app.scroll > 0.0 || app.max_scroll(&v) == 0.0);
    }

    #[test]
    fn every_tile_is_reachable_by_scrolling() {
        for v in [
            View::ipad_landscape(),
            View::ipad_portrait(),
            View::fit(390.0, 844.0),
        ] {
            let app = App::new();
            let ms = app.max_scroll(&v);
            for r in app.layout(&v).tiles {
                assert!(r.x >= 0.0 && r.x + r.w <= v.w);
                assert!(r.y + r.h - ms <= v.h);
            }
        }
    }

    #[test]
    fn sections_split_play_from_learn_without_overlap() {
        for v in [
            View::ipad_landscape(),
            View::ipad_portrait(),
            View::fit(390.0, 844.0),
        ] {
            let app = App::new();
            let lay = app.layout(&v);
            assert_eq!(
                lay.labels.iter().map(|l| l.1).collect::<Vec<_>>(),
                vec![Kind::Arcade, Kind::Learn]
            );
            let learn = lay.labels[1].0;
            for (r, e) in lay.tiles.iter().zip(&app.entries) {
                match e.info.kind {
                    Kind::Arcade => assert!(r.y + r.h <= learn.y),
                    Kind::Learn => assert!(r.y >= learn.y + learn.h),
                }
            }
        }
    }

    #[test]
    fn sections_keep_tiles_big_on_an_ipad() {
        for v in [View::ipad_landscape(), View::ipad_portrait()] {
            let tiles = App::new().layout(&v).tiles;
            assert!(tiles[0].w >= v.w.min(v.h) * 0.16);
        }
    }

    #[test]
    fn arrow_keys_walk_the_grid() {
        let v = View::ipad_landscape();
        let tiles = App::new().layout(&v).tiles;
        assert_eq!(next_focus(&tiles, 0, Dir::Right), 1);
        assert_eq!(next_focus(&tiles, 1, Dir::Left), 0);
        assert_eq!(next_focus(&tiles, 0, Dir::Left), 0);
        let below = next_focus(&tiles, 0, Dir::Down);
        assert!(tiles[below].y > tiles[0].y);
        assert_eq!(next_focus(&tiles, below, Dir::Up), 0);
    }

    #[test]
    fn keyboard_alone_can_launch_a_game() {
        let v = View::ipad_landscape();
        let mut app = menu();
        app.update(&Input::key(Dir::Right), &v, 1.0 / 60.0);
        assert_eq!(app.focus, Some(0));
        app.update(
            &Input {
                action: true,
                ..Default::default()
            },
            &v,
            1.0 / 60.0,
        );
        for _ in 0..30 {
            app.update(&Input::default(), &v, 1.0 / 60.0);
        }
        assert_eq!(app.current.as_ref().map(|c| c.0), Some(0));
        app.update(
            &Input {
                back: true,
                ..Default::default()
            },
            &v,
            1.0 / 60.0,
        );
        assert!(app.current.is_none());
    }

    fn tile_center(app: &App, v: &View, i: usize) -> glam::Vec2 {
        app.layout(v).tiles[i].center()
    }

    #[test]
    fn a_game_tapped_before_its_pack_arrives_shows_loading_then_plays() {
        let v = View::ipad_landscape();
        let i = app_index("flappy");
        crate::art::hold_for_test("flappy", true);
        let mut app = menu();
        for _ in 0..90 {
            app.update(&Input::default(), &v, 1.0 / 60.0);
        }
        app.update(&Input::press_at(tile_center(&app, &v, i)), &v, 1.0 / 60.0);
        app.update(
            &Input {
                pos: tile_center(&app, &v, i),
                released: true,
                ..Default::default()
            },
            &v,
            1.0 / 60.0,
        );
        assert_eq!(app.loading, Some(i));
        assert!(app.launch.is_none() && app.current.is_none());
        for _ in 0..120 {
            app.update(&Input::default(), &v, 1.0 / 60.0);
        }
        assert!(app.current.is_none(), "must wait for the pack");
        let mut pen = Pen::default();
        pen.begin(v, 1.0);
        app.draw(&mut pen, &v);
        crate::art::arrive_for_test("flappy");
        for _ in 0..60 {
            app.update(&Input::default(), &v, 1.0 / 60.0);
        }
        assert_eq!(app.current.as_ref().map(|c| c.0), Some(i));
        assert!(app.loading.is_none());
        for _ in 0..120 {
            app.update(&Input::default(), &v, 1.0 / 60.0);
        }
        let mut pen = Pen::default();
        pen.begin(v, 1.0);
        app.draw(&mut pen, &v);
        assert!(pen.vertex_count() > 100);
    }

    fn app_index(key: &str) -> usize {
        crate::games::all()
            .iter()
            .position(|e| e.info.key == key)
            .unwrap()
    }

    fn hit(player: &str) -> ReportHit {
        ReportHit {
            rect: Rect::new(0.0, 0.0, 1.0, 1.0),
            player: player.into(),
            name: "Leo".into(),
            game: "snake".into(),
        }
    }

    #[test]
    fn confirming_a_report_hides_the_player_and_queues_it() {
        let v = View::ipad_landscape();
        let mut app = menu();
        app.open(app_index("snake"));
        let leo = "123e4567-e89b-42d3-a456-426614174000";
        app.reporting = Some((hit(leo), 0.0));
        let yes = report_rects(&v).yes;
        app.update(&Input::press_at(yes.center()), &v, 1.0 / 60.0);
        assert!(app.reporting.is_none());
        assert!(kg_core::net::is_blocked(leo));
        assert!(app.current.is_some());
    }

    #[test]
    fn cancelling_a_report_changes_nothing_and_keeps_the_game_open() {
        let v = View::ipad_landscape();
        let mut app = menu();
        app.open(app_index("snake"));
        let leo = "123e4567-e89b-42d3-a456-426614174000";
        app.reporting = Some((hit(leo), 0.0));
        let no = report_rects(&v).no;
        app.update(&Input::press_at(no.center()), &v, 1.0 / 60.0);
        assert!(app.reporting.is_none());
        assert!(!kg_core::net::is_blocked(leo));
        assert!(app.current.is_some());
    }

    #[test]
    fn the_report_sheet_swallows_taps_meant_for_home() {
        let v = View::ipad_landscape();
        let mut app = menu();
        app.open(app_index("snake"));
        app.reporting = Some((hit("123e4567-e89b-42d3-a456-426614174000"), 0.0));
        app.update(&Input::press_at(home_rect(&v).center()), &v, 1.0 / 60.0);
        assert!(app.current.is_some());
        assert!(app.reporting.is_some());
    }

    #[test]
    fn a_finger_that_dismissed_the_report_sheet_does_not_drag_in_the_game() {
        let v = View::ipad_landscape();
        let mut app = menu();
        app.open(app_index("fruit"));
        app.reporting = Some((hit("123e4567-e89b-42d3-a456-426614174000"), 0.0));
        let no = report_rects(&v).no;
        let mut s = kg_core::engine::Script::new(v);
        app.update(&s.press(no.center()), &v, 1.0 / 60.0);
        assert!(app.reporting.is_none());
        let play = |app: &mut App| -> usize {
            let (_, g) = app.current.as_mut().unwrap();
            g.as_any()
                .downcast_mut::<crate::games::fruit::FruitPlay>()
                .unwrap()
                .g
                .blade
                .len()
        };
        for _ in 0..5 {
            app.update(&s.drag_to(vec2(400.0, 400.0)), &v, 1.0 / 60.0);
        }
        assert_eq!(play(&mut app), 0);
        app.update(&s.release(), &v, 1.0 / 60.0);
        app.update(&s.press(vec2(400.0, 400.0)), &v, 1.0 / 60.0);
        assert_eq!(play(&mut app), 1);
    }

    #[test]
    fn open_never_starts_a_game_whose_pack_is_missing() {
        crate::art::hold_for_test("paper", true);
        let mut app = menu();
        let i = app_index("paper");
        app.open(i);
        assert!(app.current.is_none());
        assert_eq!(app.loading, Some(i));
        crate::art::hold_for_test("paper", false);
        app.open(i);
        assert!(app.current.is_some());
    }

    #[test]
    fn spell_it_waits_for_the_word_voices_too() {
        crate::packs::hold_words_for_test(true);
        let mut app = menu();
        let i = app_index("spell");
        app.open(i);
        assert!(app.current.is_none());
        crate::packs::hold_words_for_test(false);
        app.open(i);
        assert!(app.current.is_some());
    }

    #[test]
    fn the_tile_draws_a_loading_hint_while_waiting() {
        let v = View::ipad_landscape();
        let mut app = menu();
        for _ in 0..90 {
            app.update(&Input::default(), &v, 1.0 / 60.0);
        }
        let mut pen = Pen::default();
        pen.begin(v, 1.0);
        app.draw(&mut pen, &v);
        let before = pen.vertex_count();
        app.loading = Some(0);
        let mut pen = Pen::default();
        pen.begin(v, 1.0);
        app.draw(&mut pen, &v);
        assert!(pen.vertex_count() > before);
    }

    #[test]
    fn every_game_on_the_menu_is_ported_and_opens() {
        let v = View::ipad_landscape();
        let all = crate::games::all();
        assert_eq!(all.len(), kg_core::games::CATALOG.len());
        for (i, e) in all.iter().enumerate() {
            assert!(e.ready(), "{} has no Bevy port", e.info.name);
            let mut app = menu();
            app.open(i);
            assert_eq!(
                app.current.as_ref().map(|c| c.0),
                Some(i),
                "{}",
                e.info.name
            );
            for _ in 0..3 {
                app.update(&Input::default(), &v, 1.0 / 60.0);
            }
        }
    }

    #[test]
    fn menu_asks_for_menu_music_and_games_for_their_mood() {
        let v = View::ipad_landscape();
        let mut app = menu();
        app.update(&Input::default(), &v, 1.0 / 60.0);
        assert_eq!(sfx::wanted_music(), Some(sfx::Mood::Menu));
        app.open(0);
        app.update(&Input::default(), &v, 1.0 / 60.0);
        assert_eq!(sfx::wanted_music(), Some(sfx::Mood::Arcade));
    }

    #[test]
    fn the_mute_buttons_toggle_music_and_sound_separately() {
        let v = View::ipad_landscape();
        let mut app = menu();
        let (music, sound) = (sfx::music_on(), sfx::muted());
        app.update(&Input::press_at(music_rect(&v).center()), &v, 1.0 / 60.0);
        assert_eq!(sfx::music_on(), !music);
        assert_eq!(sfx::muted(), sound);
        app.update(&Input::press_at(mute_rect(&v).center()), &v, 1.0 / 60.0);
        assert_eq!(sfx::muted(), !sound);
        sfx::toggle_music();
        sfx::toggle_mute();
    }

    #[test]
    fn a_large_impact_freezes_the_game_briefly() {
        let v = View::ipad_landscape();
        let mut app = menu();
        app.open(0);
        kg_core::feel::impact(kg_core::feel::Impact::Large);
        app.update(&Input::default(), &v, 1.0 / 60.0);
        assert!(app.feel.frozen());
        for _ in 0..12 {
            app.update(&Input::default(), &v, 1.0 / 60.0);
        }
        assert!(!app.feel.frozen());
    }

    #[test]
    fn learning_tiles_draw_a_progress_bar_and_play_tiles_do_not() {
        let v = View::ipad_landscape();
        let app = menu();
        let lay = app.layout(&v);
        let count = |e: &Entry, r: Rect| {
            let mut pen = Pen::default();
            pen.begin(v, 1.0);
            draw_tile(&mut pen, r, e, None);
            pen.vertex_count()
        };
        let learn_i = app
            .entries
            .iter()
            .position(|e| e.info.kind == Kind::Learn)
            .unwrap();
        let learn = &app.entries[learn_i];
        let as_play: &'static kg_core::games::Info = Box::leak(Box::new(kg_core::games::Info {
            key: "not_a_learning_game",
            ..*learn.info
        }));
        let plain = Entry {
            info: as_play,
            make: None,
        };
        assert!(count(learn, lay.tiles[0]) > count(&plain, lay.tiles[0]));
    }
}

#[cfg(test)]
mod gate_tests {
    use super::*;

    #[test]
    fn menu_buttons_and_sheets_are_twelve_mm_on_every_screen() {
        use kg_core::sizing::*;
        kg_core::save::reset_for_test();
        kg_core::profile::create("Mia", 0);
        let app = App::new();
        for (name, v) in test_views() {
            let corners = [home_rect(&v), mute_rect(&v)];
            check_buttons(&v, name, &corners).unwrap();
            check_apart(name, &corners).unwrap();
            let h = header(&v);
            check_buttons(&v, &format!("{name} player chip"), &[h.player]).unwrap();
            assert!(h.title_band.0 >= 0.0, "{name}");
            for (r, _) in app.layout(&v).labels {
                assert!(v.to_mm(r.h) >= TOUCH_MM - 0.01, "{name} label {r:?}");
            }
            for t in app.layout(&v).tiles {
                check_buttons(
                    &v,
                    &format!("{name} tile"),
                    &[Rect::new(0.0, 0.0, t.w, t.h)],
                )
                .unwrap();
            }
            let r = report_rects(&v);
            check_sheet(&v, &format!("{name} report"), &r.sheet).unwrap();
            let l = lock_rects(&v, "Road Hopper");
            check_sheet(&v, &format!("{name} lock"), &l.sheet).unwrap();
            check_apart(name, &[l.primary, l.close, l.hold]).unwrap();
            if v.portrait() {
                assert!(l.panel.w >= v.short() * 0.85, "{name} lock sheet is narrow");
                assert!(
                    r.sheet.panel.w >= v.short() * 0.85,
                    "{name} report sheet is narrow"
                );
            }
            for sheet in [&r.sheet, &l.sheet] {
                check_sheet_text(&v, dpr_of(name), name, sheet).unwrap();
            }
        }
    }

    fn menu() -> App {
        kg_core::save::reset_for_test();
        kg_core::profile::create("Mia", 0);
        App::new()
    }

    fn arcade_index(app: &App) -> usize {
        app.entries
            .iter()
            .position(|e| e.info.kind == Kind::Arcade && e.ready())
            .unwrap()
    }

    fn tap(app: &mut App, v: &View, at: Vec2) {
        app.update(&Input::press_at(at), v, 1.0 / 60.0);
        app.update(&Input::default(), v, 1.0 / 60.0);
    }

    #[test]
    fn tapping_a_locked_tile_shows_the_sheet_and_offers_learning() {
        let v = View::ipad_landscape();
        let mut app = menu();
        let i = arcade_index(&app);
        kg_core::gate::unlock(
            app.entries[(i + 1)..]
                .iter()
                .find(|e| e.info.kind == Kind::Arcade)
                .unwrap()
                .info
                .key,
        );
        let c = app.layout(&v).tiles[i].center();
        app.pick(i, &app.layout(&v).tiles);
        assert!(app.locked.is_some());
        assert!(app.launch.is_none());
        let _ = c;
        let r = lock_rects(&v, app.entries[i].info.name);
        tap(&mut app, &v, r.primary.center());
        assert!(app.locked.is_none());
        let (l, _) = app.launch.unwrap();
        assert_eq!(app.entries[l].info.kind, Kind::Learn);
    }

    #[test]
    fn an_affordable_locked_tile_unlocks_and_launches() {
        let v = View::ipad_landscape();
        let mut app = menu();
        let i = arcade_index(&app);
        app.pick(i, &app.layout(&v).tiles);
        assert!(app.locked.is_some());
        let at = lock_rects(&v, app.entries[i].info.name).primary.center();
        tap(&mut app, &v, at);
        assert_eq!(app.launch.map(|l| l.0), Some(i));
        assert!(kg_core::gate::open(app.entries[i].info.key));
    }

    #[test]
    fn not_now_closes_the_sheet_without_unlocking() {
        let v = View::ipad_landscape();
        let mut app = menu();
        let i = arcade_index(&app);
        app.pick(i, &app.layout(&v).tiles);
        let at = lock_rects(&v, app.entries[i].info.name).close.center();
        tap(&mut app, &v, at);
        assert!(app.locked.is_none());
        assert!(!kg_core::gate::open(app.entries[i].info.key));
    }

    #[test]
    fn holding_open_all_on_the_sheet_switches_gating_off() {
        let v = View::ipad_landscape();
        let mut app = menu();
        let i = arcade_index(&app);
        app.pick(i, &app.layout(&v).tiles);
        let at = lock_rects(&v, app.entries[i].info.name).hold.center();
        for _ in 0..4 {
            let mut held = Input::press_at(at);
            held.pressed = false;
            app.update(&held, &v, 0.5);
        }
        assert!(app.locked.is_none());
        assert!(kg_core::gate::open(app.entries[i].info.key));
    }
}
