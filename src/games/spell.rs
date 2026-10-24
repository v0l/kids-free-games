use crate::engine::*;
use crate::fx::*;
use crate::learn::{tall, ui_scale, PRAISE};
use macroquad::prelude::*;

pub const ROUND: usize = 8;
const WIN_TIME: f32 = 1.6;
const SLIDE_TIME: f32 = 0.22;
const MIN_TILE: f32 = 96.0;

type Draw = Picture;

pub const WORDS: [(&str, Draw); 30] = [
    ("cat", cat),
    ("dog", dog),
    ("sun", sun),
    ("bus", bus),
    ("car", car),
    ("hat", hat),
    ("cup", cup),
    ("egg", egg),
    ("bee", bee),
    ("pig", pig),
    ("fish", fish),
    ("tree", tree),
    ("cake", cake),
    ("star", star_pic),
    ("ball", ball),
    ("duck", duck),
    ("frog", frog),
    ("moon", moon),
    ("boat", boat),
    ("kite", kite),
    ("house", house),
    ("apple", apple),
    ("heart", heart),
    ("train", train),
    ("cloud", cloud_pic),
    ("snail", snail),
    ("flower", flower),
    ("rocket", rocket),
    ("banana", banana),
    ("rainbow", rainbow),
];

pub fn band(word: &str) -> usize {
    match word.len() {
        0..=3 => 0,
        4 => 1,
        _ => 2,
    }
}

pub fn decoys_for(band: usize) -> usize {
    band.min(2)
}

pub fn words_in(band_index: usize) -> Vec<&'static str> {
    WORDS
        .iter()
        .map(|(w, _)| *w)
        .filter(|w| band(w) == band_index)
        .collect()
}

pub fn picture(word: &str) -> Option<Draw> {
    WORDS.iter().find(|(w, _)| *w == word).map(|(_, d)| *d)
}

fn shuffle<T>(v: &mut [T]) {
    for i in (1..v.len()).rev() {
        v.swap(i, rand::gen_range(0, i + 1));
    }
}

pub fn make_tiles(word: &str, decoys: usize) -> Vec<char> {
    let letters: Vec<char> = word.chars().collect();
    let mut spare: Vec<char> = ('a'..='z').filter(|c| !letters.contains(c)).collect();
    shuffle(&mut spare);
    let mut tiles = letters.clone();
    tiles.extend(spare.into_iter().take(decoys));
    for _ in 0..8 {
        shuffle(&mut tiles);
        if tiles[..letters.len()] != letters[..] || letters.iter().all(|c| *c == letters[0]) {
            break;
        }
    }
    tiles
}

pub fn round_words() -> Vec<&'static str> {
    let mut out = vec![];
    for (band_index, take) in [(0, 3), (1, 3), (2, 2)] {
        let mut pool = words_in(band_index);
        shuffle(&mut pool);
        out.extend(pool.into_iter().take(take));
    }
    out
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Place {
    Right,
    Wrong,
    Done,
    Ignored,
}

#[derive(Clone, Debug)]
pub struct Board {
    pub word: Vec<char>,
    pub tiles: Vec<char>,
    pub used: Vec<Option<usize>>,
    pub filled: usize,
    pub mistakes: u32,
    pub hints: u32,
}

impl Board {
    pub fn new(word: &str) -> Self {
        Self::with_tiles(word, make_tiles(word, decoys_for(band(word))))
    }

    pub fn with_tiles(word: &str, tiles: Vec<char>) -> Self {
        let n = tiles.len();
        Self {
            word: word.chars().collect(),
            tiles,
            used: vec![None; n],
            filled: 0,
            mistakes: 0,
            hints: 0,
        }
    }

    pub fn needed(&self) -> Option<char> {
        self.word.get(self.filled).copied()
    }

    pub fn place(&mut self, tile: usize) -> Place {
        let Some(want) = self.needed() else {
            return Place::Ignored;
        };
        if tile >= self.tiles.len() || self.used[tile].is_some() {
            return Place::Ignored;
        }
        if self.tiles[tile] != want {
            self.mistakes += 1;
            return Place::Wrong;
        }
        self.used[tile] = Some(self.filled);
        self.filled += 1;
        if self.filled == self.word.len() {
            Place::Done
        } else {
            Place::Right
        }
    }

    pub fn hint(&self) -> Option<usize> {
        let want = self.needed()?;
        (0..self.tiles.len()).find(|i| self.used[*i].is_none() && self.tiles[*i] == want)
    }

    pub fn complete(&self) -> bool {
        self.filled == self.word.len()
    }

    pub fn perfect(&self) -> bool {
        self.mistakes == 0 && self.hints == 0
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Phase {
    Playing,
    Won(f32),
    Finished(f32),
}

pub struct Spell {
    pub deck: Vec<&'static str>,
    pub index: usize,
    pub board: Board,
    pub results: Vec<bool>,
    pub phase: Phase,
    pub hint_on: Option<usize>,
    wobble: Vec<f32>,
    pop: Vec<f32>,
    slide: Vec<f32>,
    held: Option<(usize, Vec2)>,
    t: f32,
    fx: Particles,
    pops: Popups,
    shake: Shake,
    picture_bump: Bump,
}

impl Default for Spell {
    fn default() -> Self {
        Self::new()
    }
}

impl Spell {
    pub fn new() -> Self {
        Self::with_deck(round_words())
    }

    pub fn with_deck(deck: Vec<&'static str>) -> Self {
        let board = Board::new(deck[0]);
        let n = board.tiles.len();
        Self {
            deck,
            index: 0,
            board,
            results: vec![],
            phase: Phase::Playing,
            hint_on: None,
            wobble: vec![0.0; n],
            pop: vec![0.0; 8],
            slide: vec![1.0; n],
            held: None,
            t: 0.0,
            fx: Particles::default(),
            pops: Popups::default(),
            shake: Shake::default(),
            picture_bump: Bump::default(),
        }
    }

    pub fn word(&self) -> &'static str {
        self.deck[self.index]
    }

    pub fn stars(&self) -> usize {
        self.results.iter().filter(|p| **p).count()
    }

    fn load(&mut self) {
        self.board = Board::new(self.word());
        let n = self.board.tiles.len();
        self.wobble = vec![0.0; n];
        self.slide = vec![1.0; n];
        self.pop = vec![0.0; self.board.word.len()];
        self.hint_on = None;
        self.held = None;
        self.phase = Phase::Playing;
        self.picture_bump.kick();
    }

    pub fn advance(&mut self) {
        if self.index + 1 >= self.deck.len() {
            self.phase = Phase::Finished(0.0);
            return;
        }
        self.index += 1;
        self.load();
    }

    pub fn use_hint(&mut self) {
        if self.phase != Phase::Playing {
            return;
        }
        if let Some(i) = self.board.hint() {
            if self.hint_on != Some(i) {
                self.board.hints += 1;
            }
            self.hint_on = Some(i);
        }
    }

    pub fn tap_tile(&mut self, i: usize, layout: &Layout) -> Place {
        if self.phase != Phase::Playing {
            return Place::Ignored;
        }
        let slot = self.board.filled;
        let r = self.board.place(i);
        match r {
            Place::Wrong => {
                self.wobble[i] = 1.0;
                self.shake.add(0.2);
            }
            Place::Right | Place::Done => {
                self.slide[i] = 0.0;
                if slot < self.pop.len() {
                    self.pop[slot] = 1.0;
                }
                if self.hint_on == Some(i) {
                    self.hint_on = None;
                }
                let at = layout.slots[slot].center();
                self.fx.sparkle(at, letter_color(slot));
            }
            Place::Ignored => {}
        }
        if r == Place::Done {
            let perfect = self.board.perfect();
            self.results.push(perfect);
            self.phase = Phase::Won(0.0);
            let msg = if perfect {
                "Perfect!"
            } else {
                PRAISE[rand::gen_range(0, PRAISE.len())]
            };
            self.pops.add(
                layout.picture.center(),
                msg,
                Color::from_rgba(255, 210, 60, 255),
                64.0,
            );
            for s in &layout.slots {
                self.fx
                    .sparkle(s.center(), Color::from_rgba(255, 220, 90, 255));
            }
        }
        r
    }
}

pub struct Layout {
    pub picture: Rect,
    pub slots: Vec<Rect>,
    pub tiles: Vec<Rect>,
    pub hint: Rect,
    pub stars_y: f32,
}

pub fn layout(view: &View, letters: usize, tiles: usize) -> Layout {
    let k = ui_scale(view);
    let top = 84.0 * k;
    let tall = tall(view);
    let max_tile = if tall { 200.0 * k } else { 130.0 * k };
    let gap = 14.0 * k;
    let slot_size = ((view.w - 60.0 - gap * (letters as f32 - 1.0)) / letters as f32).min(max_tile);
    let fits = |n: usize| (view.w - 60.0 - gap * (n as f32 - 1.0)) / n as f32 >= MIN_TILE;
    let mut per_row = if tall { tiles.min(4) } else { tiles };
    while per_row > 1 && !fits(per_row) {
        per_row -= 1;
    }
    let per_row = tiles.div_ceil(tiles.div_ceil(per_row));
    let rows = tiles.div_ceil(per_row);
    let tile_size = ((view.w - 60.0 - gap * (per_row as f32 - 1.0)) / per_row as f32).min(max_tile);
    let tiles_h = rows as f32 * tile_size + (rows as f32 - 1.0) * gap;
    let bottom = view.h - 30.0;
    let tiles_top = bottom - tiles_h;
    let slots_top = tiles_top - 40.0 * k - slot_size;
    let hs = 96.0 * k;
    let band_h = (slots_top - 30.0 * k - top).max(120.0);
    let side = band_h.min(view.w - 2.0 * (hs + 40.0)).max(120.0);
    let picture = Rect::new(
        (view.w - side) / 2.0,
        top + (band_h - side) / 2.0,
        side,
        side,
    );
    let row = |count: usize, size: f32, y: f32| -> Vec<Rect> {
        let w = count as f32 * size + (count as f32 - 1.0) * gap;
        let x0 = (view.w - w) / 2.0;
        (0..count)
            .map(|i| Rect::new(x0 + i as f32 * (size + gap), y, size, size))
            .collect()
    };
    let slots = row(letters, slot_size, slots_top);
    let mut tile_rects = vec![];
    for r in 0..rows {
        let count = (tiles - r * per_row).min(per_row);
        tile_rects.extend(row(
            count,
            tile_size,
            tiles_top + r as f32 * (tile_size + gap),
        ));
    }
    let hint = Rect::new(
        (picture.x + picture.w + view.w) / 2.0 - hs / 2.0,
        picture.y + picture.h - hs,
        hs,
        hs,
    );
    Layout {
        picture,
        slots,
        tiles: tile_rects,
        hint,
        stars_y: 40.0,
    }
}

fn letter_color(i: usize) -> Color {
    hsv(0.02 + i as f32 * 0.13, 0.6, 0.98)
}

fn tile(r: Rect, ch: char, color: Color, alpha: f32) {
    let rad = r.w * 0.22;
    round_rect(
        Rect::new(r.x, r.y + r.h * 0.08, r.w, r.h),
        rad,
        with_alpha(darker(color, 0.65), alpha),
    );
    round_rect(r, rad, with_alpha(color, alpha));
    gloss(r, rad);
    text_outline(
        &ch.to_string(),
        r.x + r.w / 2.0,
        r.y + r.h * 0.47,
        r.h * 0.62,
        with_alpha(WHITE, alpha),
        with_alpha(darker(color, 0.45), alpha),
    );
}

impl Game for Spell {
    fn update(&mut self, input: &Input, view: &View, dt: f32) {
        self.t += dt;
        self.fx.update(dt);
        self.pops.update(dt);
        self.shake.update(dt);
        self.picture_bump.update(dt);
        for w in &mut self.wobble {
            *w = (*w - dt * 2.5).max(0.0);
        }
        for p in &mut self.pop {
            *p = (*p - dt * 3.0).max(0.0);
        }
        for s in &mut self.slide {
            *s = (*s + dt / SLIDE_TIME).min(1.0);
        }
        let l = layout(view, self.board.word.len(), self.board.tiles.len());
        match self.phase {
            Phase::Won(t) => {
                let t = t + dt;
                self.phase = Phase::Won(t);
                if t > WIN_TIME || (t > 0.6 && input.pressed) {
                    self.advance();
                    if let Phase::Finished(_) = self.phase {
                        self.fx.confetti(view);
                    }
                }
                return;
            }
            Phase::Finished(t) => {
                let t = t + dt;
                self.phase = Phase::Finished(t);
                if t > 1.0 && input.pressed {
                    *self = Self::new();
                }
                return;
            }
            Phase::Playing => {}
        }
        if input.pressed {
            if l.hint.contains(input.pos) {
                self.use_hint();
                return;
            }
            if l.picture.contains(input.pos) {
                self.picture_bump.kick();
            }
            if let Some(i) = l.tiles.iter().position(|r| r.contains(input.pos)) {
                if self.board.used[i].is_none() {
                    self.held = Some((i, input.pos));
                }
            }
        }
        if input.released {
            if let Some((i, from)) = self.held.take() {
                let moved = input.pos.distance(from);
                let near_slot = l.slots.get(self.board.filled).is_some_and(|s| {
                    Rect::new(s.x - s.w * 0.4, s.y - s.h * 0.4, s.w * 1.8, s.h * 1.8)
                        .contains(input.pos)
                });
                if moved < 30.0 || near_slot {
                    self.tap_tile(i, &l);
                } else {
                    self.wobble[i] = 0.5;
                }
            }
        }
    }

    fn draw(&self, view: &View) {
        sky(
            view,
            Color::from_rgba(255, 236, 170, 255),
            Color::from_rgba(255, 190, 200, 255),
        );
        let l = layout(view, self.board.word.len(), self.board.tiles.len());
        let k = ui_scale(view);
        set_view_camera(view, self.shake.offset());
        for i in 0..ROUND.min(self.deck.len()) {
            let c = vec2(
                view.w / 2.0 + (i as f32 - (ROUND as f32 - 1.0) / 2.0) * 44.0 * k,
                l.stars_y,
            );
            let col = match self.results.get(i) {
                Some(true) => Color::from_rgba(255, 200, 40, 255),
                Some(false) => Color::from_rgba(200, 210, 230, 255),
                None if i == self.index => with_alpha(WHITE, 0.9),
                None => with_alpha(WHITE, 0.4),
            };
            let s = if i == self.index && self.results.len() == i {
                1.0 + (self.t * 4.0).sin() * 0.08
            } else {
                1.0
            };
            star(
                c + vec2(0.0, 2.0),
                19.0 * k * s,
                0.0,
                Color::new(0.4, 0.2, 0.3, 0.25),
            );
            star(c, 17.0 * k * s, 0.0, col);
        }
        let p = l.picture;
        round_rect(
            Rect::new(p.x, p.y + 10.0, p.w, p.h),
            36.0,
            Color::new(0.5, 0.2, 0.3, 0.18),
        );
        round_rect(p, 36.0, WHITE);
        let bob = (self.t * 2.0).sin() * p.h * 0.015;
        let sc = self.picture_bump.scale(0.12);
        if let Some(draw) = picture(self.word()) {
            draw(&Pen {
                c: p.center() + vec2(0.0, bob),
                s: p.w.min(p.h) * 0.42 * sc,
                t: self.t,
            });
        }
        let won = match self.phase {
            Phase::Won(t) => Some(t),
            _ => None,
        };
        for (i, r) in l.slots.iter().enumerate() {
            let filled = i < self.board.filled;
            if !filled {
                let next = i == self.board.filled;
                round_rect(
                    *r,
                    r.w * 0.22,
                    Color::new(1.0, 1.0, 1.0, if next { 0.75 } else { 0.45 }),
                );
                let line = if next {
                    Color::from_rgba(255, 140, 120, 255)
                } else {
                    Color::new(0.5, 0.3, 0.4, 0.3)
                };
                let pulse = if next {
                    1.0 + (self.t * 5.0).sin() * 0.04
                } else {
                    1.0
                };
                draw_rectangle(
                    r.center().x - r.w * 0.3 * pulse,
                    r.y + r.h * 0.8,
                    r.w * 0.6 * pulse,
                    6.0,
                    line,
                );
                continue;
            }
            let wave = won
                .map(|t| ((t * 9.0 - i as f32 * 0.9).sin().max(0.0)) * r.h * 0.25)
                .unwrap_or(0.0);
            let pop = self.pop.get(i).copied().unwrap_or(0.0);
            let s = 1.0 + pop * 0.25;
            let rr = Rect::new(
                r.center().x - r.w * s / 2.0,
                r.center().y - r.h * s / 2.0 - wave,
                r.w * s,
                r.h * s,
            );
            tile(
                rr,
                self.board.word[i].to_ascii_uppercase(),
                letter_color(i),
                1.0,
            );
        }
        for (i, r) in l.tiles.iter().enumerate() {
            if let Some(slot) = self.board.used[i] {
                let k2 = ease_out_cubic(self.slide[i]);
                if k2 < 1.0 {
                    let to = l.slots[slot];
                    let at = Rect::new(
                        r.x + (to.x - r.x) * k2,
                        r.y + (to.y - r.y) * k2,
                        r.w + (to.w - r.w) * k2,
                        r.h + (to.h - r.h) * k2,
                    );
                    tile(
                        at,
                        self.board.tiles[i].to_ascii_uppercase(),
                        letter_color(slot),
                        1.0,
                    );
                }
                let inner = Rect::new(r.x + r.w * 0.1, r.y + r.h * 0.1, r.w * 0.8, r.h * 0.8);
                round_rect(inner, r.w * 0.18, Color::new(0.55, 0.3, 0.4, 0.12));
                continue;
            }
            let mut rr = *r;
            if self.held.is_some_and(|(h, _)| h == i) {
                rr.y -= rr.h * 0.08;
            }
            let w = self.wobble[i];
            rr.x += (w * 30.0).sin() * w * 14.0;
            let hinted = self.hint_on == Some(i);
            if hinted {
                let g = 8.0 + (self.t * 6.0).sin().abs() * 8.0;
                round_rect(
                    Rect::new(rr.x - g, rr.y - g, rr.w + g * 2.0, rr.h + g * 2.0),
                    rr.w * 0.3,
                    Color::new(1.0, 0.95, 0.4, 0.7),
                );
            }
            let col = if w > 0.0 {
                Color::from_rgba(240, 90, 90, 255)
            } else {
                Color::from_rgba(110, 140, 240, 255)
            };
            tile(rr, self.board.tiles[i].to_ascii_uppercase(), col, 1.0);
        }
        let h = l.hint;
        let hp = 1.0
            + if self.hint_on.is_some() {
                0.0
            } else {
                (self.t * 2.0).sin() * 0.03
            };
        let hr = Rect::new(
            h.center().x - h.w * hp / 2.0,
            h.center().y - h.h * hp / 2.0,
            h.w * hp,
            h.h * hp,
        );
        round_rect(
            Rect::new(hr.x, hr.y + 6.0, hr.w, hr.h),
            hr.w / 2.0,
            Color::from_rgba(200, 140, 20, 255),
        );
        round_rect(hr, hr.w / 2.0, Color::from_rgba(255, 200, 50, 255));
        let bulb = hr.center() - vec2(0.0, hr.h * 0.08);
        draw_circle(bulb.x, bulb.y, hr.w * 0.22, WHITE);
        draw_rectangle(
            bulb.x - hr.w * 0.1,
            bulb.y + hr.h * 0.16,
            hr.w * 0.2,
            hr.h * 0.14,
            WHITE,
        );
        draw_rectangle(
            bulb.x - hr.w * 0.08,
            bulb.y + hr.h * 0.32,
            hr.w * 0.16,
            hr.h * 0.05,
            Color::from_rgba(255, 245, 220, 255),
        );
        self.fx.draw();
        self.pops.draw();
        set_view_camera(view, Vec2::ZERO);
        if let Phase::Finished(_) = self.phase {
            let s = format!("{} of {} perfect words", self.stars(), self.deck.len());
            overlay(view, "You did it!", &[&s, "Tap to spell more"]);
        }
    }
}

pub struct Pen {
    pub c: Vec2,
    pub s: f32,
    pub t: f32,
}

pub type Picture = fn(&Pen);

impl Pen {
    fn p(&self, x: f32, y: f32) -> Vec2 {
        self.c + vec2(x, y) * self.s
    }
    fn circle(&self, x: f32, y: f32, r: f32, col: Color) {
        let p = self.p(x, y);
        draw_circle(p.x, p.y, r * self.s, col);
    }
    fn ring(&self, x: f32, y: f32, r: f32, w: f32, col: Color) {
        let p = self.p(x, y);
        draw_circle_lines(p.x, p.y, r * self.s, w * self.s, col);
    }
    fn ellipse(&self, x: f32, y: f32, w: f32, h: f32, rot: f32, col: Color) {
        let p = self.p(x, y);
        draw_ellipse(p.x, p.y, w * self.s, h * self.s, rot, col);
    }
    fn rect(&self, x: f32, y: f32, w: f32, h: f32, col: Color) {
        let p = self.p(x, y);
        draw_rectangle(p.x, p.y, w * self.s, h * self.s, col);
    }
    fn rrect(&self, x: f32, y: f32, w: f32, h: f32, r: f32, col: Color) {
        let p = self.p(x, y);
        round_rect(Rect::new(p.x, p.y, w * self.s, h * self.s), r * self.s, col);
    }
    fn tri(&self, a: (f32, f32), b: (f32, f32), c: (f32, f32), col: Color) {
        draw_triangle(self.p(a.0, a.1), self.p(b.0, b.1), self.p(c.0, c.1), col);
    }
    fn line(&self, a: (f32, f32), b: (f32, f32), w: f32, col: Color) {
        let (p, q) = (self.p(a.0, a.1), self.p(b.0, b.1));
        draw_line(p.x, p.y, q.x, q.y, w * self.s, col);
    }
    fn eyes(&self, x: f32, y: f32, gap: f32, r: f32) {
        for s in [-1.0, 1.0] {
            self.circle(x + s * gap, y, r, WHITE);
            self.circle(
                x + s * gap + r * 0.15,
                y + r * 0.1,
                r * 0.55,
                Color::from_rgba(40, 30, 50, 255),
            );
            self.circle(x + s * gap + r * 0.3, y - r * 0.2, r * 0.2, WHITE);
        }
    }
    fn smile(&self, x: f32, y: f32, w: f32) {
        let col = Color::from_rgba(60, 40, 50, 255);
        let n = 8;
        for k in 0..n {
            let a0 = std::f32::consts::PI * (0.15 + 0.7 * k as f32 / n as f32);
            let a1 = std::f32::consts::PI * (0.15 + 0.7 * (k + 1) as f32 / n as f32);
            self.line(
                (x + a0.cos() * w, y + a0.sin() * w * 0.6),
                (x + a1.cos() * w, y + a1.sin() * w * 0.6),
                0.04,
                col,
            );
        }
    }
    fn cheeks(&self, x: f32, y: f32, gap: f32) {
        for s in [-1.0, 1.0] {
            self.ellipse(
                x + s * gap,
                y,
                0.09,
                0.055,
                0.0,
                Color::new(1.0, 0.45, 0.5, 0.45),
            );
        }
    }
}

fn rgb(r: u8, g: u8, b: u8) -> Color {
    Color::from_rgba(r, g, b, 255)
}

fn cat(p: &Pen) {
    let fur = rgb(250, 170, 80);
    p.tri(
        (-0.62, -0.25),
        (-0.2, -0.55),
        (-0.68, -0.85),
        darker(fur, 0.85),
    );
    p.tri(
        (0.62, -0.25),
        (0.2, -0.55),
        (0.68, -0.85),
        darker(fur, 0.85),
    );
    p.tri(
        (-0.55, -0.35),
        (-0.3, -0.5),
        (-0.6, -0.72),
        rgb(255, 180, 190),
    );
    p.tri((0.55, -0.35), (0.3, -0.5), (0.6, -0.72), rgb(255, 180, 190));
    p.circle(0.0, 0.05, 0.7, fur);
    p.ellipse(0.0, 0.32, 0.38, 0.26, 0.0, rgb(255, 235, 210));
    p.eyes(0.0, -0.08, 0.27, 0.13);
    p.tri((-0.07, 0.17), (0.07, 0.17), (0.0, 0.25), rgb(240, 110, 130));
    p.smile(0.0, 0.25, 0.12);
    for s in [-1.0, 1.0] {
        for dy in [-0.03, 0.06] {
            p.line(
                (s * 0.3, 0.25 + dy),
                (s * 0.85, 0.18 + dy * 2.0),
                0.02,
                rgb(120, 70, 40),
            );
        }
    }
    p.cheeks(0.0, 0.18, 0.38);
}

fn dog(p: &Pen) {
    let fur = rgb(200, 150, 100);
    p.ellipse(-0.6, 0.0, 0.2, 0.42, 15.0, darker(fur, 0.65));
    p.ellipse(0.6, 0.0, 0.2, 0.42, -15.0, darker(fur, 0.65));
    p.circle(0.0, 0.0, 0.62, fur);
    p.ellipse(0.22, -0.2, 0.2, 0.22, 0.0, darker(fur, 0.8));
    p.ellipse(0.0, 0.3, 0.32, 0.24, 0.0, rgb(250, 230, 200));
    p.eyes(0.0, -0.15, 0.24, 0.12);
    p.ellipse(0.0, 0.18, 0.12, 0.08, 0.0, rgb(50, 35, 40));
    p.smile(0.0, 0.3, 0.12);
    p.ellipse(0.0, 0.45, 0.07, 0.09, 0.0, rgb(240, 100, 120));
}

fn sun(p: &Pen) {
    let y = rgb(255, 200, 40);
    for k in 0..12 {
        let a = k as f32 / 12.0 * std::f32::consts::TAU + p.t * 0.3;
        let (c, s) = (a.cos(), a.sin());
        p.tri(
            (c * 0.55 - s * 0.1, s * 0.55 + c * 0.1),
            (c * 0.55 + s * 0.1, s * 0.55 - c * 0.1),
            (c * 0.92, s * 0.92),
            rgb(255, 160, 40),
        );
    }
    p.circle(0.0, 0.0, 0.58, y);
    p.eyes(0.0, -0.08, 0.2, 0.1);
    p.smile(0.0, 0.12, 0.2);
    p.cheeks(0.0, 0.12, 0.33);
}

fn wheel(p: &Pen, x: f32, y: f32, r: f32) {
    p.circle(x, y, r, rgb(50, 50, 60));
    p.circle(x, y, r * 0.45, rgb(200, 200, 210));
}

fn bus(p: &Pen) {
    p.rrect(-0.85, -0.5, 1.7, 0.85, 0.15, rgb(255, 190, 30));
    p.rect(-0.85, 0.12, 1.7, 0.08, rgb(220, 120, 30));
    for k in 0..4 {
        p.rrect(
            -0.72 + k as f32 * 0.36,
            -0.38,
            0.28,
            0.3,
            0.05,
            rgb(170, 220, 255),
        );
    }
    p.rrect(0.55, -0.38, 0.22, 0.6, 0.04, rgb(150, 200, 240));
    wheel(p, -0.5, 0.38, 0.17);
    wheel(p, 0.5, 0.38, 0.17);
    p.circle(0.82, 0.22, 0.05, rgb(255, 250, 200));
}

fn car(p: &Pen) {
    p.rrect(-0.45, -0.45, 0.85, 0.4, 0.15, rgb(230, 60, 70));
    p.rrect(-0.35, -0.38, 0.3, 0.28, 0.06, rgb(180, 225, 255));
    p.rrect(0.02, -0.38, 0.3, 0.28, 0.06, rgb(180, 225, 255));
    p.rrect(-0.85, -0.12, 1.7, 0.42, 0.16, rgb(240, 70, 80));
    p.rect(-0.85, 0.05, 1.7, 0.05, rgb(200, 40, 60));
    wheel(p, -0.48, 0.32, 0.19);
    wheel(p, 0.48, 0.32, 0.19);
    p.circle(0.78, 0.0, 0.06, rgb(255, 245, 180));
}

fn hat(p: &Pen) {
    p.ellipse(0.0, 0.35, 0.9, 0.18, 0.0, rgb(60, 60, 90));
    p.rrect(-0.5, -0.55, 1.0, 0.9, 0.12, rgb(70, 70, 105));
    p.rect(-0.5, 0.08, 1.0, 0.16, rgb(230, 70, 90));
    p.circle(0.3, 0.16, 0.1, rgb(255, 210, 60));
}

fn cup(p: &Pen) {
    p.ring(0.6, 0.0, 0.22, 0.09, rgb(80, 160, 230));
    p.rrect(-0.55, -0.45, 1.1, 0.95, 0.2, rgb(90, 170, 240));
    p.ellipse(0.0, -0.45, 0.55, 0.1, 0.0, rgb(130, 80, 50));
    p.circle(-0.15, 0.05, 0.12, rgb(255, 255, 255));
    p.circle(0.15, 0.05, 0.12, rgb(255, 255, 255));
    p.tri((-0.27, 0.08), (0.27, 0.08), (0.0, 0.3), rgb(255, 255, 255));
    for k in 0..3 {
        let x = -0.25 + k as f32 * 0.25;
        let w = (p.t * 2.0 + k as f32).sin() * 0.05;
        p.line(
            (x, -0.6),
            (x + w, -0.85),
            0.04,
            Color::new(0.7, 0.7, 0.8, 0.6),
        );
    }
}

fn egg(p: &Pen) {
    p.ellipse(0.0, 0.05, 0.52, 0.7, 0.0, rgb(250, 245, 230));
    p.ellipse(
        -0.15,
        -0.25,
        0.12,
        0.2,
        -20.0,
        Color::new(1.0, 1.0, 1.0, 0.8),
    );
    p.eyes(0.0, 0.05, 0.18, 0.09);
    p.smile(0.0, 0.22, 0.12);
    p.cheeks(0.0, 0.22, 0.28);
}

fn bee(p: &Pen) {
    let flap = (p.t * 18.0).sin() * 0.05;
    p.ellipse(
        -0.2,
        -0.45 + flap,
        0.25,
        0.35,
        -25.0,
        Color::new(0.85, 0.95, 1.0, 0.85),
    );
    p.ellipse(
        0.2,
        -0.45 - flap,
        0.25,
        0.35,
        25.0,
        Color::new(0.85, 0.95, 1.0, 0.85),
    );
    p.ellipse(0.0, 0.1, 0.72, 0.5, 0.0, rgb(255, 205, 40));
    for x in [-0.25, 0.15] {
        p.rect(x, -0.35, 0.16, 0.9, rgb(50, 40, 40));
    }
    p.circle(0.58, 0.05, 0.25, rgb(255, 205, 40));
    p.eyes(0.62, 0.0, 0.1, 0.06);
    p.tri((-0.72, 0.05), (-0.95, 0.12), (-0.72, 0.2), rgb(50, 40, 40));
}

fn pig(p: &Pen) {
    let pink = rgb(255, 170, 185);
    p.tri(
        (-0.55, -0.3),
        (-0.2, -0.55),
        (-0.6, -0.75),
        darker(pink, 0.85),
    );
    p.tri((0.55, -0.3), (0.2, -0.55), (0.6, -0.75), darker(pink, 0.85));
    p.circle(0.0, 0.0, 0.65, pink);
    p.ellipse(0.0, 0.18, 0.26, 0.18, 0.0, rgb(255, 130, 155));
    p.circle(-0.09, 0.18, 0.05, rgb(180, 70, 100));
    p.circle(0.09, 0.18, 0.05, rgb(180, 70, 100));
    p.eyes(0.0, -0.18, 0.24, 0.1);
    p.cheeks(0.0, 0.1, 0.4);
}

fn fish(p: &Pen) {
    let wag = (p.t * 4.0).sin() * 0.06;
    p.tri(
        (0.45, 0.0),
        (0.95, -0.4 + wag),
        (0.95, 0.4 + wag),
        rgb(255, 120, 60),
    );
    p.ellipse(-0.05, 0.0, 0.62, 0.42, 0.0, rgb(255, 150, 60));
    p.tri(
        (-0.1, -0.38),
        (0.25, -0.38),
        (0.1, -0.65),
        rgb(255, 120, 60),
    );
    for k in 0..3 {
        p.ring(
            0.05 + k as f32 * 0.17,
            0.0,
            0.14,
            0.03,
            Color::new(1.0, 0.85, 0.6, 0.7),
        );
    }
    p.circle(-0.38, -0.08, 0.1, WHITE);
    p.circle(-0.4, -0.06, 0.05, rgb(40, 30, 50));
    p.smile(-0.45, 0.1, 0.08);
    for k in 0..3 {
        let y = -0.55 - ((p.t * 0.8 + k as f32 * 0.33).fract()) * 0.4;
        p.ring(
            -0.75 + k as f32 * 0.05,
            y,
            0.05,
            0.02,
            Color::new(0.4, 0.7, 1.0, 0.7),
        );
    }
}

fn tree(p: &Pen) {
    p.rrect(-0.12, 0.1, 0.24, 0.75, 0.04, rgb(140, 90, 50));
    p.circle(0.0, -0.3, 0.45, rgb(80, 180, 80));
    p.circle(-0.35, -0.05, 0.33, rgb(70, 165, 75));
    p.circle(0.35, -0.05, 0.33, rgb(70, 165, 75));
    p.circle(0.0, -0.55, 0.3, rgb(100, 200, 90));
    for (x, y) in [(-0.2, -0.35), (0.25, -0.15), (0.05, -0.65), (-0.35, 0.05)] {
        p.circle(x, y, 0.07, rgb(230, 60, 60));
    }
}

fn cake(p: &Pen) {
    p.ellipse(0.0, 0.62, 0.85, 0.12, 0.0, rgb(220, 220, 235));
    p.rrect(-0.7, 0.0, 1.4, 0.6, 0.08, rgb(250, 200, 150));
    p.rrect(-0.7, -0.08, 1.4, 0.2, 0.08, rgb(255, 150, 190));
    for k in 0..6 {
        p.circle(-0.6 + k as f32 * 0.24, 0.12, 0.08, rgb(255, 150, 190));
    }
    p.rect(-0.7, 0.32, 1.4, 0.08, rgb(255, 230, 240));
    p.rect(-0.04, -0.5, 0.08, 0.42, rgb(120, 190, 250));
    let flick = (p.t * 12.0).sin() * 0.02;
    p.ellipse(flick, -0.6, 0.07, 0.12, 0.0, rgb(255, 180, 40));
    p.ellipse(flick, -0.58, 0.035, 0.06, 0.0, rgb(255, 245, 160));
}

fn star_pic(p: &Pen) {
    let c = p.p(0.0, 0.0);
    star(c + vec2(0.0, 0.04 * p.s), 0.9 * p.s, 0.0, rgb(230, 160, 30));
    star(c, 0.88 * p.s, 0.0, rgb(255, 210, 50));
    p.eyes(0.0, -0.02, 0.16, 0.08);
    p.smile(0.0, 0.14, 0.12);
    p.cheeks(0.0, 0.14, 0.26);
}

fn ball(p: &Pen) {
    p.circle(0.0, 0.0, 0.7, rgb(240, 70, 80));
    p.ellipse(0.0, 0.0, 0.7, 0.22, 0.0, WHITE);
    p.ellipse(0.0, 0.0, 0.7, 0.12, 0.0, rgb(60, 140, 240));
    p.circle(-0.25, -0.35, 0.13, Color::new(1.0, 1.0, 1.0, 0.5));
}

fn duck(p: &Pen) {
    let y = rgb(255, 215, 60);
    p.ellipse(0.05, 0.25, 0.65, 0.42, 0.0, y);
    p.ellipse(0.15, 0.2, 0.3, 0.18, -15.0, darker(y, 0.9));
    p.circle(-0.35, -0.3, 0.33, y);
    p.tri(
        (-0.62, -0.32),
        (-0.62, -0.18),
        (-0.92, -0.24),
        rgb(255, 140, 40),
    );
    p.circle(-0.42, -0.38, 0.07, rgb(40, 30, 50));
    p.circle(-0.4, -0.4, 0.025, WHITE);
    p.ellipse(0.0, 0.7, 0.9, 0.08, 0.0, Color::new(0.4, 0.7, 1.0, 0.6));
}

fn frog(p: &Pen) {
    let g = rgb(110, 200, 90);
    p.circle(-0.32, -0.42, 0.22, g);
    p.circle(0.32, -0.42, 0.22, g);
    p.ellipse(0.0, 0.05, 0.75, 0.55, 0.0, g);
    p.ellipse(0.0, 0.25, 0.5, 0.3, 0.0, rgb(200, 240, 160));
    p.eyes(0.0, -0.44, 0.32, 0.13);
    p.smile(0.0, 0.0, 0.32);
    p.cheeks(0.0, 0.05, 0.45);
}

fn moon(p: &Pen) {
    p.circle(0.0, 0.0, 0.7, rgb(255, 230, 120));
    p.circle(0.35, -0.2, 0.62, rgb(255, 255, 255));
    p.circle(-0.3, 0.2, 0.08, rgb(240, 205, 100));
    p.circle(-0.45, -0.15, 0.06, rgb(240, 205, 100));
    p.circle(-0.38, -0.02, 0.05, rgb(60, 50, 60));
    for (x, y) in [(0.55, 0.45), (0.7, -0.6), (0.15, 0.6)] {
        star(p.p(x, y), 0.09 * p.s, p.t, rgb(255, 210, 60));
    }
}

fn boat(p: &Pen) {
    p.tri((0.0, -0.8), (0.0, 0.1), (0.6, 0.1), rgb(255, 255, 255));
    p.tri(
        (-0.05, -0.65),
        (-0.05, 0.1),
        (-0.5, 0.1),
        rgb(255, 120, 110),
    );
    p.rect(-0.03, -0.85, 0.06, 0.95, rgb(120, 80, 50));
    p.tri((-0.85, 0.15), (0.85, 0.15), (0.6, 0.5), rgb(220, 90, 60));
    p.tri((-0.85, 0.15), (-0.6, 0.5), (0.6, 0.5), rgb(220, 90, 60));
    let w = (p.t * 2.0).sin() * 0.03;
    p.ellipse(0.0, 0.58 + w, 1.0, 0.1, 0.0, rgb(90, 170, 240));
}

fn kite(p: &Pen) {
    let sway = (p.t * 1.5).sin() * 0.05;
    let top = (0.0 + sway, -0.85);
    p.tri(
        top,
        (-0.5 + sway, -0.25),
        (0.0 + sway, 0.25),
        rgb(240, 80, 100),
    );
    p.tri(
        top,
        (0.5 + sway, -0.25),
        (0.0 + sway, 0.25),
        rgb(80, 160, 240),
    );
    p.line(top, (sway, 0.25), 0.02, rgb(255, 255, 255));
    p.line(
        (-0.5 + sway, -0.25),
        (0.5 + sway, -0.25),
        0.02,
        rgb(255, 255, 255),
    );
    let mut prev = (sway, 0.25);
    for k in 1..7 {
        let f = k as f32 / 6.0;
        let pt = (sway + (p.t * 3.0 + f * 5.0).sin() * 0.12, 0.25 + f * 0.6);
        p.line(prev, pt, 0.02, rgb(120, 90, 80));
        if k % 2 == 0 {
            p.tri(
                (pt.0 - 0.06, pt.1),
                (pt.0 + 0.06, pt.1 - 0.04),
                (pt.0 + 0.06, pt.1 + 0.04),
                rgb(255, 200, 50),
            );
        }
        prev = pt;
    }
}

fn house(p: &Pen) {
    p.rect(-0.6, -0.15, 1.2, 0.85, rgb(255, 220, 160));
    p.tri((-0.8, -0.1), (0.8, -0.1), (0.0, -0.8), rgb(220, 80, 70));
    p.rect(0.35, -0.65, 0.15, 0.3, rgb(160, 90, 70));
    p.rrect(-0.15, 0.25, 0.3, 0.45, 0.06, rgb(150, 95, 60));
    p.circle(0.08, 0.48, 0.03, rgb(255, 210, 60));
    for x in [-0.48, 0.25] {
        p.rect(x, 0.0, 0.23, 0.2, rgb(150, 210, 255));
        p.rect(x + 0.105, 0.0, 0.02, 0.2, WHITE);
    }
}

fn apple(p: &Pen) {
    p.circle(-0.22, 0.1, 0.5, rgb(235, 55, 60));
    p.circle(0.22, 0.1, 0.5, rgb(235, 55, 60));
    p.circle(0.0, 0.3, 0.48, rgb(235, 55, 60));
    p.circle(-0.28, -0.08, 0.13, Color::new(1.0, 1.0, 1.0, 0.45));
    p.rect(-0.03, -0.62, 0.07, 0.3, rgb(120, 80, 40));
    p.ellipse(0.2, -0.52, 0.2, 0.09, -25.0, rgb(80, 180, 70));
}

fn heart(p: &Pen) {
    let beat = 1.0 + (p.t * 4.0).sin().max(0.0) * 0.06;
    let pp = Pen {
        c: p.c,
        s: p.s * beat,
        t: p.t,
    };
    pp.circle(-0.3, -0.18, 0.38, rgb(240, 70, 110));
    pp.circle(0.3, -0.18, 0.38, rgb(240, 70, 110));
    pp.tri(
        (-0.66, -0.05),
        (0.66, -0.05),
        (0.0, 0.72),
        rgb(240, 70, 110),
    );
    pp.circle(-0.38, -0.3, 0.1, Color::new(1.0, 1.0, 1.0, 0.5));
}

fn train(p: &Pen) {
    p.rrect(-0.85, -0.2, 1.1, 0.55, 0.08, rgb(60, 140, 230));
    p.rrect(0.1, -0.6, 0.7, 0.95, 0.08, rgb(230, 70, 70));
    p.rrect(0.2, -0.5, 0.5, 0.3, 0.05, rgb(180, 225, 255));
    p.rect(-0.7, -0.5, 0.18, 0.32, rgb(70, 70, 80));
    p.rect(-0.75, -0.55, 0.28, 0.08, rgb(70, 70, 80));
    for (k, r) in [(-0.6, 0.15), (-0.2, 0.15), (0.3, 0.17), (0.65, 0.17)] {
        wheel(p, k, 0.42, r);
    }
    for k in 0..3 {
        let f = (p.t * 0.6 + k as f32 * 0.33).fract();
        p.circle(
            -0.61 - f * 0.3,
            -0.62 - f * 0.4,
            0.08 + f * 0.1,
            Color::new(0.9, 0.9, 0.95, 0.8 - f * 0.6),
        );
    }
}

fn cloud_pic(p: &Pen) {
    let puffs = [
        (-0.45, 0.05, 0.3),
        (-0.1, -0.2, 0.4),
        (0.35, -0.1, 0.34),
        (0.6, 0.1, 0.24),
        (0.05, 0.12, 0.35),
    ];
    for (x, y, r) in puffs {
        p.circle(x, y, r + 0.05, rgb(120, 170, 230));
    }
    for (x, y, r) in puffs {
        p.circle(x, y, r, rgb(200, 225, 255));
    }
    for (x, y, r) in puffs {
        p.circle(x - 0.03, y - 0.04, r * 0.88, rgb(235, 245, 255));
    }
    p.eyes(0.05, -0.05, 0.18, 0.08);
    p.smile(0.05, 0.1, 0.1);
    p.cheeks(0.05, 0.1, 0.3);
    for k in 0..3 {
        let f = (p.t * 0.9 + k as f32 * 0.33).fract();
        p.ellipse(
            -0.35 + k as f32 * 0.35,
            0.55 + f * 0.3,
            0.04,
            0.07,
            0.0,
            Color::new(0.3, 0.55, 1.0, 1.0 - f),
        );
    }
}

fn snail(p: &Pen) {
    p.ellipse(0.05, 0.42, 0.85, 0.18, 0.0, rgb(160, 210, 130));
    p.circle(0.15, -0.05, 0.48, rgb(220, 140, 80));
    p.ring(0.15, -0.05, 0.32, 0.07, rgb(180, 100, 60));
    p.ring(0.15, -0.05, 0.15, 0.06, rgb(180, 100, 60));
    p.circle(-0.55, 0.15, 0.2, rgb(160, 210, 130));
    p.line((-0.6, 0.0), (-0.68, -0.35), 0.04, rgb(140, 190, 110));
    p.line((-0.48, 0.0), (-0.45, -0.35), 0.04, rgb(140, 190, 110));
    p.circle(-0.68, -0.38, 0.07, rgb(40, 30, 50));
    p.circle(-0.45, -0.38, 0.07, rgb(40, 30, 50));
    p.smile(-0.55, 0.18, 0.08);
}

fn flower(p: &Pen) {
    p.rect(-0.04, 0.1, 0.08, 0.8, rgb(90, 170, 70));
    p.ellipse(0.2, 0.55, 0.2, 0.08, -30.0, rgb(100, 190, 80));
    for k in 0..6 {
        let a = k as f32 / 6.0 * std::f32::consts::TAU + p.t * 0.2;
        p.circle(
            a.cos() * 0.32,
            -0.25 + a.sin() * 0.32,
            0.25,
            rgb(255, 120, 170),
        );
    }
    p.circle(0.0, -0.25, 0.22, rgb(255, 210, 60));
    p.eyes(0.0, -0.28, 0.08, 0.05);
    p.smile(0.0, -0.18, 0.06);
}

fn rocket(p: &Pen) {
    let f = (p.t * 20.0).sin() * 0.05;
    p.tri(
        (-0.18, 0.5),
        (0.18, 0.5),
        (0.0, 0.95 + f),
        rgb(255, 160, 40),
    );
    p.tri((-0.1, 0.5), (0.1, 0.5), (0.0, 0.78 + f), rgb(255, 240, 120));
    p.tri((-0.3, 0.1), (-0.3, 0.55), (-0.6, 0.6), rgb(230, 70, 70));
    p.tri((0.3, 0.1), (0.3, 0.55), (0.6, 0.6), rgb(230, 70, 70));
    p.ellipse(0.0, 0.0, 0.32, 0.65, 0.0, rgb(235, 235, 245));
    p.tri((-0.26, -0.38), (0.26, -0.38), (0.0, -0.9), rgb(230, 70, 70));
    p.circle(0.0, -0.08, 0.15, rgb(90, 160, 240));
    p.ring(0.0, -0.08, 0.15, 0.04, rgb(170, 170, 190));
}

fn banana(p: &Pen) {
    let n = 28;
    let arc = |f: f32, r: f32| {
        let a = std::f32::consts::PI * (0.12 + 0.76 * f);
        (a.cos() * r, -0.55 + a.sin() * r)
    };
    for k in 0..n {
        let f0 = k as f32 / n as f32;
        let f1 = (k + 1) as f32 / n as f32;
        let thick = |f: f32| 0.05 + 0.3 * (1.0 - (f * 2.0 - 1.0).powi(2)).max(0.0);
        let (o0, o1) = (arc(f0, 1.0), arc(f1, 1.0));
        let (i0, i1) = (arc(f0, 1.0 - thick(f0)), arc(f1, 1.0 - thick(f1)));
        let edge = rgb(230, 180, 40);
        p.tri(o0, o1, i0, rgb(255, 220, 70));
        p.tri(o1, i1, i0, rgb(255, 220, 70));
        let (m0, m1) = (
            arc(f0, 1.0 - thick(f0) * 0.35),
            arc(f1, 1.0 - thick(f1) * 0.35),
        );
        p.tri(o0, o1, m0, edge);
        p.tri(o1, m1, m0, edge);
    }
    let tip = arc(0.0, 0.97);
    let end = arc(1.0, 0.97);
    p.circle(tip.0, tip.1, 0.05, rgb(110, 80, 40));
    p.rrect(
        end.0 - 0.05,
        end.1 - 0.12,
        0.1,
        0.16,
        0.03,
        rgb(110, 140, 50),
    );
    p.eyes(0.0, 0.2, 0.12, 0.06);
    p.smile(0.0, 0.3, 0.07);
}

fn rainbow(p: &Pen) {
    let cols = [
        rgb(240, 70, 70),
        rgb(255, 160, 50),
        rgb(255, 220, 60),
        rgb(100, 200, 90),
        rgb(80, 150, 240),
        rgb(160, 100, 220),
    ];
    for (i, c) in cols.iter().enumerate() {
        let r = 0.85 - i as f32 * 0.1;
        let n = 24;
        for k in 0..n {
            let a0 = std::f32::consts::PI * (1.0 + k as f32 / n as f32);
            let a1 = std::f32::consts::PI * (1.0 + (k + 1) as f32 / n as f32);
            let ri = r - 0.1;
            p.tri(
                (a0.cos() * r, 0.3 + a0.sin() * r),
                (a1.cos() * r, 0.3 + a1.sin() * r),
                (a0.cos() * ri, 0.3 + a0.sin() * ri),
                *c,
            );
            p.tri(
                (a1.cos() * r, 0.3 + a1.sin() * r),
                (a1.cos() * ri, 0.3 + a1.sin() * ri),
                (a0.cos() * ri, 0.3 + a0.sin() * ri),
                *c,
            );
        }
    }
    for x in [-0.7, 0.7] {
        for (dx, r) in [(-0.15, 0.15), (0.0, 0.2), (0.15, 0.15)] {
            p.circle(x + dx, 0.35, r, WHITE);
        }
    }
}

pub fn icon(r: Rect) {
    draw_rectangle(r.x, r.y, r.w, r.h, rgb(255, 225, 170));
    let s = r.w * 0.28;
    for (i, ch) in ['C', 'A', 'T'].iter().enumerate() {
        let x = r.x + r.w * 0.06 + i as f32 * (s + r.w * 0.04);
        tile(
            Rect::new(x, r.y + r.h * 0.6, s, s),
            *ch,
            letter_color(i),
            1.0,
        );
    }
    cat(&Pen {
        c: vec2(r.x + r.w / 2.0, r.y + r.h * 0.32),
        s: r.w * 0.24,
        t: 0.0,
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn there_are_enough_words_in_every_band() {
        assert!(WORDS.len() >= 24);
        assert!(words_in(0).len() >= 6);
        assert!(words_in(1).len() >= 6);
        assert!(words_in(2).len() >= 6);
        let mut seen: Vec<&str> = WORDS.iter().map(|(w, _)| *w).collect();
        seen.sort();
        seen.dedup();
        assert_eq!(seen.len(), WORDS.len());
        assert!(WORDS
            .iter()
            .all(|(w, _)| w.chars().all(|c| c.is_ascii_lowercase())));
    }

    #[test]
    fn every_word_has_a_picture() {
        for (w, _) in WORDS {
            assert!(picture(w).is_some(), "{w}");
        }
    }

    #[test]
    fn tiles_hold_every_letter_including_repeats() {
        for (w, _) in WORDS {
            for decoys in 0..3 {
                let tiles = make_tiles(w, decoys);
                assert_eq!(tiles.len(), w.len() + decoys);
                for c in w.chars() {
                    let need = w.chars().filter(|x| *x == c).count();
                    let have = tiles.iter().filter(|x| **x == c).count();
                    assert_eq!(have, need, "{w} {c}");
                }
            }
        }
    }

    #[test]
    fn decoys_never_share_a_letter_with_the_word() {
        for (w, _) in WORDS {
            let tiles = make_tiles(w, 2);
            let extra: Vec<char> = tiles.iter().copied().filter(|c| !w.contains(*c)).collect();
            assert_eq!(extra.len(), 2, "{w}");
        }
    }

    #[test]
    fn wrong_letter_does_not_advance() {
        let mut b = Board::with_tiles("cat", vec!['t', 'a', 'c', 'z']);
        assert_eq!(b.place(3), Place::Wrong);
        assert_eq!(b.place(0), Place::Wrong);
        assert_eq!(b.filled, 0);
        assert_eq!(b.mistakes, 2);
        assert_eq!(b.place(2), Place::Right);
        assert_eq!(b.place(2), Place::Ignored);
        assert_eq!(b.place(1), Place::Right);
        assert_eq!(b.place(0), Place::Done);
        assert!(b.complete());
        assert!(!b.perfect());
    }

    #[test]
    fn either_copy_of_a_repeated_letter_works() {
        let mut b = Board::with_tiles("egg", vec!['g', 'e', 'g']);
        assert_eq!(b.place(1), Place::Right);
        assert_eq!(b.place(2), Place::Right);
        assert_eq!(b.place(0), Place::Done);
        assert!(b.perfect());
    }

    #[test]
    fn hint_points_at_the_next_letter() {
        let mut b = Board::with_tiles("dog", vec!['g', 'o', 'd']);
        assert_eq!(b.hint(), Some(2));
        b.place(2);
        assert_eq!(b.hint(), Some(1));
    }

    #[test]
    fn finishing_a_word_moves_to_the_next() {
        let deck = vec!["cat", "dog"];
        let mut g = Spell::with_deck(deck);
        let l = layout(&View::ipad_landscape(), 3, g.board.tiles.len());
        for ch in "cat".chars() {
            let i = (0..g.board.tiles.len())
                .find(|i| g.board.used[*i].is_none() && g.board.tiles[*i] == ch)
                .unwrap();
            g.tap_tile(i, &l);
        }
        assert!(matches!(g.phase, Phase::Won(_)));
        assert_eq!(g.stars(), 1);
        g.advance();
        assert_eq!(g.word(), "dog");
        assert_eq!(g.phase, Phase::Playing);
        g.advance();
        assert!(matches!(g.phase, Phase::Finished(_)));
    }

    #[test]
    fn hint_costs_the_star() {
        let mut g = Spell::with_deck(vec!["sun"]);
        let l = layout(&View::ipad_landscape(), 3, g.board.tiles.len());
        g.use_hint();
        g.use_hint();
        assert_eq!(g.board.hints, 1);
        for ch in "sun".chars() {
            let i = (0..g.board.tiles.len())
                .find(|i| g.board.used[*i].is_none() && g.board.tiles[*i] == ch)
                .unwrap();
            g.tap_tile(i, &l);
        }
        assert_eq!(g.stars(), 0);
    }

    #[test]
    fn round_goes_from_short_to_long_words() {
        for _ in 0..20 {
            let r = round_words();
            assert_eq!(r.len(), ROUND);
            assert!(r.windows(2).all(|w| band(w[0]) <= band(w[1])));
            let mut u = r.clone();
            u.sort();
            u.dedup();
            assert_eq!(u.len(), ROUND);
        }
    }

    #[test]
    fn tiles_and_slots_are_big_and_on_screen() {
        for v in [
            View::ipad_landscape(),
            View::ipad_portrait(),
            View::fit(1080.0, 2400.0),
        ] {
            for (letters, tiles) in [(3, 3), (4, 5), (7, 9)] {
                let l = layout(&v, letters, tiles);
                for r in l.tiles.iter().chain(&l.slots).chain([&l.hint]) {
                    assert!(r.w >= 90.0, "{r:?} in {v:?} for {letters}/{tiles}");
                    assert!(
                        r.x >= 0.0 && r.x + r.w <= v.w && r.y >= 0.0 && r.y + r.h <= v.h,
                        "{r:?} off {v:?}"
                    );
                }
                for (a, b) in l.tiles.iter().zip(l.tiles.iter().skip(1)) {
                    assert!(!a.overlaps(b) || a.y != b.y);
                }
                assert!(l.picture.y + l.picture.h <= l.slots[0].y);
                assert!(
                    (l.picture.w - l.picture.h).abs() < 0.5,
                    "picture card is square"
                );
                assert!(
                    !l.hint.overlaps(&l.picture),
                    "hint {:?} covers picture {:?}",
                    l.hint,
                    l.picture
                );
                if v.h > v.w * 1.8 && letters == 3 {
                    assert!(
                        l.tiles[0].w >= 200.0,
                        "phone tiles too small: {:?}",
                        l.tiles[0]
                    );
                }
                assert!(l.slots[0].y + l.slots[0].h <= l.tiles[0].y);
            }
        }
    }
}
