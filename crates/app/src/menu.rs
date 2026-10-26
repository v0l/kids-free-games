use crate::draw::{measure, Draw};
use crate::games::{self, Entry, Play};
use crate::paint::Pen;
use crate::ui::Tone;
use glam::{vec2, Vec2};
use kg_core::engine::*;
use kg_core::feel::Feel;
use kg_core::fx::*;
use kg_core::games::Kind;
use kg_core::geom::{Color, Rect, WHITE};
use kg_core::sfx::{self, Sfx};

pub const HOME: Rect = Rect::new(14.0, 12.0, 60.0, 48.0);
pub const MUSIC: Rect = HOME;
const LAUNCH_TIME: f32 = 0.22;
const WAVE: f32 = 6.0;
const TAG_SIZE: f32 = 26.0;
pub const TAGLINE: &str = "No ads. No tracking. Just play.";
pub const LABEL_H: f32 = 46.0;
pub const SCROLL_ROOM: f32 = 1.3;

pub fn mute_rect(view: &View) -> Rect {
    Rect::new(view.w - 74.0, 12.0, 60.0, 48.0)
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
    let title_size = (view.w / 13.0).min(64.0);
    let reach = title_size * (0.5 + kg_core::outline::OUTLINE_FRAC + 0.08) + WAVE;
    let title_y = 14.0 + reach;
    let title_band = (title_y - reach, title_y + reach);
    let tag_h = TAG_SIZE + 14.0;
    let tag_w = measure(TAGLINE, TAG_SIZE) + 40.0;
    let mut tag = Rect::new(view.w / 2.0 - tag_w / 2.0, title_band.1 + 8.0, tag_w, tag_h);
    let mut player = crate::profiles::player_chip_at(view, tag.y + tag.h + 12.0);
    let gap = 20.0;
    if tag.w + player.w + gap <= view.w - 40.0 {
        let left = view.w / 2.0 - (tag.w + player.w + gap) / 2.0;
        tag.x = left;
        player.x = left + tag.w + gap;
        player.y = tag.y + tag.h / 2.0 - player.h / 2.0;
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
    let label_space = secs.len() as f32 * LABEL_H;
    let fit = |cols: usize| {
        let rows: usize = secs.iter().map(|(_, n)| n.div_ceil(cols)).sum();
        let by_w = (view.w - pad * (cols as f32 + 1.0)) / cols as f32;
        let by_h = ((view.h * SCROLL_ROOM - top - label_space - pad) / rows as f32 - pad) / 1.18;
        by_w.min(by_h)
    };
    let cols = (2..=6).max_by(|a, b| fit(*a).total_cmp(&fit(*b))).unwrap();
    let tw = fit(cols);
    let th = tw * 1.18;
    let left = (view.w - cols as f32 * tw - (cols as f32 - 1.0) * pad) / 2.0;
    let mut y = top;
    let mut tiles = vec![];
    let mut labels = vec![];
    for (kind, n) in secs {
        labels.push((Rect::new(left, y, view.w - left * 2.0, LABEL_H), kind));
        y += LABEL_H;
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
            if self.current.is_none() && self.launch.is_none() && crate::packs::ready(self.entries[i].info.key) {
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
                if input.down && HOME.contains(input.pos) {
                    1.0
                } else {
                    0.0
                },
                20.0,
                dt,
            );
            if input.tapped_in(HOME) || input.back {
                self.go_home();
                return;
            }
            game.update(input, view, game_dt);
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
        if input.tapped_in(mute_rect(view)) {
            sfx::toggle_mute();
            sfx::play(Sfx::Tap);
            return;
        }
        if input.tapped_in(MUSIC) {
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
                self.scroll = r.y - header(view).bottom - LABEL_H;
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

    pub fn draw(&self, pen: &mut Pen, view: &View) {
        if let Some((_, game)) = &self.current {
            game.draw(pen, view);
            pen.offset = Vec2::ZERO;
            if self.feel.flash_alpha() > 0.0 {
                pen.fill_screen(
                    view,
                    self.feel.flash_color().with_alpha(self.feel.flash_alpha()),
                );
            }
            let r = Rect::new(HOME.x, HOME.y - 4.0, HOME.h + 8.0, HOME.h + 8.0);
            pen.painted_round(r, "icon_home", self.home_press);
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
        let m = mute_rect(view);
        pen.painted_round(
            Rect::new(m.x + 4.0, m.y - 4.0 - self.scroll, m.h + 8.0, m.h + 8.0),
            if sfx::muted() {
                "icon_audio_off"
            } else {
                "icon_audio"
            },
            0.0,
        );
        pen.painted_round(
            Rect::new(
                MUSIC.x,
                MUSIC.y - 4.0 - self.scroll,
                MUSIC.h + 8.0,
                MUSIC.h + 8.0,
            ),
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
            let w = measure(text, 30.0) + 80.0;
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
        let title = "Kids Free Games";
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
        App::new()
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
                h.title_band.1 < HOME.y + HOME.h
                    || mute_rect(&v).x > v.w / 2.0 + measure("Kids Free Games", h.title_size) / 2.0,
                "title runs under the mute button in {v:?}"
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
        app.update(&Input {
                pos: tile_center(&app, &v, i),
                released: true,
                ..Default::default()
            }, &v, 1.0 / 60.0);
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
        assert_eq!(all.len(), 16);
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
        app.update(&Input::press_at(MUSIC.center()), &v, 1.0 / 60.0);
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
