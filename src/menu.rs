use crate::engine::draw_circle;
use crate::engine::draw_ellipse;
use crate::engine::*;
use crate::fx::*;
use crate::games::{self, Entry, Kind};
use crate::sfx::{self, Sfx};
use macroquad::prelude::*;

pub const HOME: Rect = Rect {
    x: 14.0,
    y: 12.0,
    w: 60.0,
    h: 48.0,
};

pub struct App {
    pub entries: Vec<Entry>,
    pub current: Option<(usize, Box<dyn Game>)>,
    scroll: f32,
    drag_from: Option<(Vec2, f32)>,
    dragged: bool,
    t: f32,
    since_menu: f32,
    pressed_tile: Option<usize>,
    press: Vec<f32>,
    launch: Option<(usize, f32)>,
    curtain: f32,
    home_press: f32,
    fx: Particles,
    feel: crate::feel::Feel,
    focus: Option<usize>,
}

const LAUNCH_TIME: f32 = 0.22;

pub fn mute_rect(view: &View) -> Rect {
    Rect::new(view.w - 74.0, 12.0, 60.0, 48.0)
}

pub const MUSIC: Rect = HOME;

fn draw_mute(view: &View, scroll: f32) {
    let r = mute_rect(view);
    let r = Rect::new(r.x, r.y - scroll, r.w, r.h);
    round_rect(
        Rect::new(r.x, r.y + 4.0, r.w, r.h),
        14.0,
        Color::from_rgba(0, 0, 0, 50),
    );
    round_rect(r, 14.0, with_alpha(WHITE, 0.94));
    let c = r.center();
    let col = Color::from_rgba(80, 80, 110, 255);
    draw_rectangle(c.x - 16.0, c.y - 6.0, 8.0, 12.0, col);
    draw_triangle(
        vec2(c.x - 9.0, c.y - 6.0),
        vec2(c.x + 2.0, c.y - 15.0),
        vec2(c.x + 2.0, c.y + 15.0),
        col,
    );
    draw_triangle(
        vec2(c.x - 9.0, c.y - 6.0),
        vec2(c.x - 9.0, c.y + 6.0),
        vec2(c.x + 2.0, c.y + 15.0),
        col,
    );
    if sfx::muted() {
        draw_line(
            c.x + 7.0,
            c.y - 8.0,
            c.x + 19.0,
            c.y + 8.0,
            4.0,
            Color::from_rgba(230, 70, 80, 255),
        );
        draw_line(
            c.x + 19.0,
            c.y - 8.0,
            c.x + 7.0,
            c.y + 8.0,
            4.0,
            Color::from_rgba(230, 70, 80, 255),
        );
    } else {
        for (k, rad) in [8.0, 15.0].iter().enumerate() {
            let a = 0.7 - k as f32 * 0.05;
            let mut prev = c + vec2(4.0 + rad * (-a).cos(), rad * (-a).sin());
            for j in 1..=8 {
                let t = -a + 2.0 * a * j as f32 / 8.0;
                let p = c + vec2(4.0 + rad * t.cos(), rad * t.sin());
                draw_line(prev.x, prev.y, p.x, p.y, 3.0, col);
                prev = p;
            }
        }
    }
}

pub struct Header {
    pub title_size: f32,
    pub title_y: f32,
    pub title_band: (f32, f32),
    pub tag: Rect,
    pub bottom: f32,
}

const WAVE: f32 = 6.0;
const TAG_SIZE: f32 = 26.0;

pub fn measure_width(s: &str, size: f32) -> f32 {
    crate::outline::measure(FONT_BYTES, s, size)
}

pub fn header(view: &View) -> Header {
    let title_size = (view.w / 13.0).min(64.0);
    let reach = title_size * (0.5 + crate::outline::OUTLINE_FRAC + 0.08) + WAVE;
    let title_y = 14.0 + reach;
    let title_band = (title_y - reach, title_y + reach);
    let tag_h = TAG_SIZE + 14.0;
    let tag_w = measure_width("No ads. No tracking. Just play.", TAG_SIZE) + 40.0;
    let tag = Rect::new(view.w / 2.0 - tag_w / 2.0, title_band.1 + 8.0, tag_w, tag_h);
    Header {
        title_size,
        title_y,
        title_band,
        tag,
        bottom: tag.y + tag.h,
    }
}

pub const LABEL_H: f32 = 46.0;
pub const SCROLL_ROOM: f32 = 1.3;

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
            feel: crate::feel::Feel::default(),
            focus: None,
        }
    }

    pub fn open(&mut self, i: usize) {
        set_accent(self.entries[i].color);
        self.current = Some((i, (self.entries[i].make)()));
        self.curtain = 1.0;
    }

    fn go_home(&mut self) {
        sfx::play(Sfx::Whoosh);
        self.current = None;
        self.since_menu = 0.0;
        self.curtain = 1.0;
        self.pressed_tile = None;
    }

    fn kinds(&self) -> Vec<Kind> {
        self.entries.iter().map(|e| e.kind).collect()
    }

    pub fn layout(&self, view: &View) -> Layout {
        layout(view, &self.kinds())
    }

    fn max_scroll(&self, view: &View) -> f32 {
        let rects = self.layout(view).tiles;
        let bottom = rects.last().map(|r| r.y + r.h + 30.0).unwrap_or(0.0);
        (bottom - view.h).max(0.0)
    }

    pub fn update(&mut self, input: &Input, view: &View, dt: f32) {
        for (i, c) in crate::feel::take() {
            self.feel.hit(i, c);
        }
        let game_dt = self.feel.game_dt(dt);
        sfx::music(match &self.current {
            Some((i, _)) => self.entries[*i].kind.mood(),
            None => sfx::Mood::Menu,
        });
        self.t += dt;
        self.since_menu += dt;
        self.curtain = (self.curtain - dt * 5.0).max(0.0);
        self.fx.update(dt);
        tick_overlay(dt);
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
                self.launch = Some((i, 0.0));
                sfx::play(Sfx::Pop);
                let r = rects[i];
                self.fx.sparkle(
                    vec2(r.center().x, r.center().y - self.scroll),
                    self.entries[i].color,
                );
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
            if dy.abs() > 12.0 {
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
                    self.launch = Some((i, 0.0));
                    sfx::play(Sfx::Pop);
                    let r = rects[i];
                    self.fx.sparkle(
                        vec2(r.center().x, r.center().y - self.scroll),
                        self.entries[i].color,
                    );
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

    fn draw_home_button(&self) {
        let s = 1.0 - self.home_press * 0.12;
        let r = Rect::new(
            HOME.center().x - HOME.w * s / 2.0,
            HOME.center().y - HOME.h * s / 2.0,
            HOME.w * s,
            HOME.h * s,
        );
        round_rect(
            Rect::new(r.x, r.y + 4.0, r.w, r.h),
            14.0,
            Color::from_rgba(0, 0, 0, 60),
        );
        round_rect(r, 14.0, with_alpha(WHITE, 0.94));
        let c = r.center();
        let col = Color::from_rgba(80, 80, 110, 255);
        draw_triangle(
            vec2(c.x - 16.0 * s, c.y - 1.0),
            vec2(c.x + 16.0 * s, c.y - 1.0),
            vec2(c.x, c.y - 16.0 * s),
            col,
        );
        draw_rectangle(c.x - 11.0 * s, c.y - 2.0, 22.0 * s, 16.0 * s, col);
        draw_rectangle(c.x - 4.0 * s, c.y + 5.0 * s, 8.0 * s, 9.0 * s, WHITE);
    }

    fn draw_backdrop(&self, view: &View) {
        sky(
            view,
            Color::from_rgba(255, 190, 110, 255),
            Color::from_rgba(255, 120, 165, 255),
        );
        let rays = 14;
        let c = vec2(view.w / 2.0, -view.h * 0.15 - self.scroll * 0.3);
        let len = view.w.max(view.h) * 1.6;
        for k in 0..rays {
            let a0 = self.t * 0.08 + k as f32 * std::f32::consts::TAU / rays as f32;
            let a1 = a0 + std::f32::consts::TAU / rays as f32 * 0.5;
            draw_triangle(
                c,
                c + vec2(a0.cos(), a0.sin()) * len,
                c + vec2(a1.cos(), a1.sin()) * len,
                Color::new(1.0, 1.0, 1.0, 0.06),
            );
        }
        for k in 0..18 {
            let seed = k as f32 * 12.9898;
            let x = (seed.sin() * 43758.545).fract().abs() * view.w;
            let speed = 12.0 + (k % 5) as f32 * 6.0;
            let y = view.h + 40.0 - (self.t * speed + k as f32 * 97.0).rem_euclid(view.h + 120.0);
            let wob = (self.t * 1.3 + k as f32).sin() * 14.0;
            let r = 6.0 + (k % 4) as f32 * 5.0;
            let col = [
                Color::new(1.0, 1.0, 1.0, 0.22),
                Color::new(1.0, 0.95, 0.6, 0.25),
                Color::new(0.7, 0.9, 1.0, 0.22),
            ][k % 3];
            if k % 2 == 0 {
                draw_circle(x + wob, y, r, col);
                draw_circle(
                    x + wob - r * 0.3,
                    y - r * 0.3,
                    r * 0.3,
                    Color::new(1.0, 1.0, 1.0, 0.25),
                );
            } else {
                star(vec2(x + wob, y), r * 1.2, self.t * 0.6 + k as f32, col);
            }
        }
    }

    fn draw_title(&self, view: &View) {
        let title = "Kids Free Games";
        let h = header(view);
        let size = h.title_size;
        let total = measure_width(title, size);
        let mut x = view.w / 2.0 - total / 2.0;
        let y = h.title_y - self.scroll;
        for (i, ch) in title.chars().enumerate() {
            let s = ch.to_string();
            let w = measure_width(&s, size);
            let intro = ease_out_back(((self.since_menu - i as f32 * 0.015) / 0.3).clamp(0.0, 1.0));
            let wave = (self.t * 3.0 - i as f32 * 0.45).sin() * WAVE;
            let col = hsv(i as f32 * 0.07 + self.t * 0.05, 0.55, 1.0);
            if ch != ' ' {
                text_outline(
                    &s,
                    x + w / 2.0,
                    y + wave - (1.0 - intro) * 60.0,
                    size * intro.max(0.01),
                    col,
                    Color::from_rgba(120, 40, 70, 255),
                );
            }
            x += w;
        }
        let tag = "No ads. No tracking. Just play.";
        let a = ((self.since_menu - 0.15) / 0.25).clamp(0.0, 1.0);
        let r = Rect::new(h.tag.x, h.tag.y - self.scroll, h.tag.w, h.tag.h);
        round_rect(r, r.h / 2.0, Color::new(1.0, 1.0, 1.0, 0.35 * a));
        text_center(
            tag,
            view.w / 2.0,
            r.y + r.h / 2.0,
            TAG_SIZE,
            with_alpha(Color::from_rgba(130, 40, 70, 255), a),
        );
    }

    pub fn draw(&self, view: &View) {
        if let Some((_, game)) = &self.current {
            game.draw(view);
            set_view_camera(view, Vec2::ZERO);
            self.feel.draw(view);
            self.draw_home_button();
            self.draw_curtain(view);
            return;
        }
        self.draw_backdrop(view);
        self.draw_title(view);
        draw_mute(view, self.scroll);
        draw_music(self.scroll);
        let lay = self.layout(view);
        for (r, kind) in &lay.labels {
            let r = Rect::new(r.x, r.y - self.scroll, r.w, r.h);
            if r.y > view.h || r.y + r.h < 0.0 {
                continue;
            }
            let a = ((self.since_menu - 0.1) / 0.25).clamp(0.0, 1.0);
            draw_section_label(r, *kind, a);
        }
        for (i, (r, e)) in lay.tiles.iter().zip(&self.entries).enumerate() {
            let r = Rect::new(r.x, r.y - self.scroll, r.w, r.h);
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
            let c = r.center() + vec2(0.0, bob + (1.0 - intro) * 40.0);
            let r = Rect::new(
                c.x - r.w * scale / 2.0,
                c.y - r.h * scale / 2.0,
                r.w * scale,
                r.h * scale,
            );
            if focused {
                let g = 6.0 + (self.t * 5.0).sin() * 2.0;
                round_rect(
                    Rect::new(r.x - g, r.y - g, r.w + g * 2.0, r.h + g * 2.0 + r.w * 0.03),
                    r.w * 0.14 + g,
                    WHITE,
                );
            }
            draw_tile(r, e, (focused || self.press[i] > 0.01).then_some(self.t));
        }
        self.fx.draw();
        if let Some((i, lt)) = self.launch {
            let k =
                ease_out_cubic(((lt - LAUNCH_TIME * 0.4) / (LAUNCH_TIME * 0.6)).clamp(0.0, 1.0));
            if k > 0.0 {
                let c = lay.tiles[i].center() - vec2(0.0, self.scroll);
                let rad = k * view.full().w.hypot(view.full().h);
                draw_circle(c.x, c.y, rad, self.entries[i].color);
            }
        }
        self.draw_curtain(view);
    }

    fn draw_curtain(&self, view: &View) {
        if self.curtain <= 0.0 {
            return;
        }
        let color = self
            .current
            .as_ref()
            .map(|(i, _)| self.entries[*i].color)
            .unwrap_or(Color::from_rgba(255, 160, 140, 255));
        let k = ease_out_cubic(self.curtain);
        let rad = k * view.full().w.hypot(view.full().h) * 0.6;
        let c = view.center();
        draw_circle(c.x, c.y, rad, color);
    }
}

fn draw_section_label(r: Rect, kind: Kind, a: f32) {
    let (text, color) = match kind {
        Kind::Arcade => ("Play", Color::from_rgba(255, 255, 255, 255)),
        Kind::Learn => ("Learn", Color::from_rgba(255, 250, 200, 255)),
    };
    let size = 30.0;
    let w = measure_width(text, size);
    let cy = r.y + r.h / 2.0 + 4.0;
    text_outline(
        text,
        r.x + 6.0 + w / 2.0,
        cy,
        size,
        with_alpha(color, a),
        with_alpha(Color::from_rgba(150, 50, 90, 255), a),
    );
    let x0 = r.x + w + 24.0;
    round_rect(
        Rect::new(x0, cy - 2.0, (r.x + r.w - x0).max(0.0), 4.0),
        2.0,
        Color::new(1.0, 1.0, 1.0, 0.35 * a),
    );
}

fn draw_music(scroll: f32) {
    let r = Rect::new(MUSIC.x, MUSIC.y - scroll, MUSIC.w, MUSIC.h);
    round_rect(
        Rect::new(r.x, r.y + 4.0, r.w, r.h),
        14.0,
        Color::from_rgba(0, 0, 0, 50),
    );
    round_rect(r, 14.0, with_alpha(WHITE, 0.94));
    let c = r.center();
    let col = Color::from_rgba(80, 80, 110, 255);
    draw_ellipse(c.x - 9.0, c.y + 9.0, 7.0, 5.5, -20.0, col);
    draw_ellipse(c.x + 9.0, c.y + 5.0, 7.0, 5.5, -20.0, col);
    draw_rectangle(c.x - 4.0, c.y - 14.0, 3.5, 23.0, col);
    draw_rectangle(c.x + 14.0, c.y - 18.0, 3.5, 23.0, col);
    draw_triangle(
        vec2(c.x - 4.0, c.y - 14.0),
        vec2(c.x + 17.5, c.y - 18.0),
        vec2(c.x + 17.5, c.y - 11.0),
        col,
    );
    draw_triangle(
        vec2(c.x - 4.0, c.y - 14.0),
        vec2(c.x - 4.0, c.y - 7.0),
        vec2(c.x + 17.5, c.y - 11.0),
        col,
    );
    if !sfx::music_on() {
        draw_line(
            c.x - 18.0,
            c.y + 16.0,
            c.x + 20.0,
            c.y - 18.0,
            4.0,
            Color::from_rgba(230, 70, 80, 255),
        );
    }
}

fn draw_tile(r: Rect, e: &Entry, shine: Option<f32>) {
    let rad = r.w * 0.14;
    round_rect(
        Rect::new(r.x + 2.0, r.y + r.w * 0.05, r.w, r.h),
        rad,
        Color::from_rgba(120, 30, 70, 60),
    );
    round_rect(
        Rect::new(r.x, r.y + r.w * 0.03, r.w, r.h),
        rad,
        darker(e.color, 0.7),
    );
    round_rect(r, rad, WHITE);
    let ir = Rect::new(r.x + r.w * 0.07, r.y + r.w * 0.07, r.w * 0.86, r.w * 0.86);
    round_rect(ir, rad * 0.75, e.color);
    let inner = Rect::new(ir.x + ir.w * 0.1, ir.y + ir.h * 0.1, ir.w * 0.8, ir.h * 0.8);
    (e.icon)(inner);
    gloss(ir, rad * 0.75);
    if let Some(t) = shine {
        shine_sweep(ir, t);
    }
    let size = (r.w * 0.86 / text_size(e.name, 10.0).width * 10.0).min(r.w * 0.14);
    text_center(
        e.name,
        r.x + r.w / 2.0,
        ir.y + ir.h + (r.y + r.h - ir.y - ir.h) / 2.0,
        size,
        darker(e.color, 0.6),
    );
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
                h.title_band.1 <= h.tag.y,
                "title {:?} runs into tagline {:?} in {v:?}",
                h.title_band,
                h.tag
            );
            let tiles = App::new().layout(&v).tiles;
            assert!(
                h.bottom <= tiles[0].y - 8.0,
                "tagline bottom {} touches tiles at {} in {v:?}",
                h.bottom,
                tiles[0].y
            );
            assert!(
                h.title_band.1 < HOME.y + HOME.h
                    || mute_rect(&v).x
                        > v.w / 2.0 + measure_width("Kids Free Games", h.title_size) / 2.0,
                "title runs under the mute button in {v:?}"
            );
        }
    }

    #[test]
    fn every_tile_is_reachable_by_scrolling() {
        for v in [
            View::ipad_landscape(),
            View::ipad_portrait(),
            View::fit(390.0, 844.0),
        ] {
            let app = App::new();
            let rects = app.layout(&v).tiles;
            let ms = app.max_scroll(&v);
            for r in rects {
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
                match e.kind {
                    Kind::Arcade => {
                        assert!(r.y + r.h <= learn.y, "{} under the Learn label", e.name)
                    }
                    Kind::Learn => assert!(r.y >= learn.y + learn.h, "{} above its label", e.name),
                }
                for (l, _) in &lay.labels {
                    let apart = r.y >= l.y + l.h || r.y + r.h <= l.y;
                    assert!(apart, "{} overlaps a label in {v:?}", e.name);
                }
            }
        }
    }

    #[test]
    fn sections_keep_tiles_big_on_an_ipad() {
        for v in [View::ipad_landscape(), View::ipad_portrait()] {
            let tiles = App::new().layout(&v).tiles;
            assert!(
                tiles[0].w >= v.w.min(v.h) * 0.16,
                "tiles shrank to {} in {v:?}",
                tiles[0].w
            );
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
        assert!((tiles[below].x - tiles[0].x).abs() < 1.0);
        assert_eq!(next_focus(&tiles, below, Dir::Up), 0);
    }

    #[test]
    fn keyboard_alone_can_launch_a_game() {
        let v = View::ipad_landscape();
        let mut app = App::new();
        app.update(&Input::key(Dir::Right), &v, 1.0 / 60.0);
        assert_eq!(app.focus, Some(0));
        app.update(&Input::key(Dir::Right), &v, 1.0 / 60.0);
        assert_eq!(app.focus, Some(1));
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
        assert_eq!(app.current.as_ref().map(|c| c.0), Some(1));
    }

    #[test]
    fn menu_asks_for_menu_music_and_games_for_their_mood() {
        let v = View::ipad_landscape();
        let mut app = App::new();
        app.update(&Input::default(), &v, 1.0 / 60.0);
        assert_eq!(sfx::wanted_music(), Some(sfx::Mood::Menu));
        let learn = app
            .entries
            .iter()
            .position(|e| e.kind == Kind::Learn)
            .unwrap();
        app.open(learn);
        app.update(&Input::default(), &v, 1.0 / 60.0);
        assert_eq!(sfx::wanted_music(), Some(sfx::Mood::Learn));
        app.open(0);
        app.update(&Input::default(), &v, 1.0 / 60.0);
        assert_eq!(sfx::wanted_music(), Some(sfx::Mood::Arcade));
    }

    #[test]
    fn a_large_impact_freezes_the_game_briefly() {
        let v = View::ipad_landscape();
        let mut app = App::new();
        app.open(0);
        crate::feel::impact(crate::feel::Impact::Large);
        app.update(&Input::default(), &v, 1.0 / 60.0);
        assert!(app.feel.frozen());
        for _ in 0..12 {
            app.update(&Input::default(), &v, 1.0 / 60.0);
        }
        assert!(!app.feel.frozen());
    }

    #[test]
    fn every_game_survives_a_minute_of_random_input() {
        rand::srand(1);
        for v in [View::ipad_landscape(), View::ipad_portrait()] {
            for e in games::all() {
                let mut g = (e.make)();
                for f in 0..3600 {
                    let pos = vec2(rand::gen_range(0.0, v.w), rand::gen_range(0.0, v.h));
                    let mut input = Input {
                        pos,
                        prev_pos: pos - vec2(30.0, 10.0),
                        down: f % 7 < 4,
                        ..Default::default()
                    };
                    input.pressed = f % 7 == 0;
                    input.released = f % 7 == 4;
                    if f % 11 == 0 {
                        input.swipe = Some(Dir::ALL[rand::gen_range(0, 4)]);
                    }
                    g.update(&input, &v, 1.0 / 60.0);
                }
            }
        }
    }
}
