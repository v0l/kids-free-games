use crate::engine::draw_circle;
use crate::engine::*;
use crate::feel::{self, Impact};
use crate::fx::*;
use crate::learn::{tall, ui_scale, PRAISE};
use crate::sfx::{self, Sfx};
use macroquad::prelude::*;

mod art {
    crate::sprites!("spell");
}

pub const ROUND: usize = 8;
const WIN_TIME: f32 = 1.6;
const SLIDE_TIME: f32 = 0.22;
const MIN_TILE: f32 = 96.0;

type Draw = Picture;

#[cfg(test)]
const SPRITES: [&str; 26] = [
    "cat", "dog", "bus", "car", "hat", "cup", "egg", "pig", "tree", "cake", "ball", "duck", "frog",
    "moon", "boat", "house", "apple", "heart", "train", "cloud", "snail", "rocket", "banana",
    "sun", "bee", "fish",
];

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
            sfx::play(Sfx::Tick);
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
                sfx::play(Sfx::Wrong);
                self.wobble[i] = 1.0;
                self.shake.add(0.2);
            }
            Place::Right | Place::Done => {
                sfx::play_step(Sfx::Pop, slot);
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
            sfx::play(Sfx::Right);
            sfx::say(if perfect {
                sfx::Say::Great
            } else {
                sfx::Say::Correct
            });
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
    art::sprite_ex(
        "tile",
        Rect::new(r.x, r.y + r.h * 0.06, r.w, r.h),
        crate::art::Draw {
            tint: Color::new(0.0, 0.0, 0.0, 0.2 * alpha),
            ..Default::default()
        },
    );
    art::sprite_ex(
        "tile",
        r,
        crate::art::Draw {
            tint: with_alpha(color, alpha),
            ..Default::default()
        },
    );
    text_outline(
        &ch.to_string(),
        r.x + r.w / 2.0,
        r.y + r.h * 0.47,
        r.h * 0.62,
        with_alpha(WHITE, alpha),
        with_alpha(darker(color, 0.45), alpha),
    );
}

fn draw_backdrop(view: &View, t: f32) {
    let f = view.full();
    let side = f.w.max(f.h);
    art::sprite(
        "bg",
        Rect::new(f.x + (f.w - side) / 2.0, f.y + f.h - side, side, side),
    );
    for i in 0..4usize {
        let span = view.w + 500.0;
        let x = (i as f32 * 0.27 * span + t * (6.0 + i as f32 * 3.0)) % span - 250.0;
        crate::ui::cloud(
            i * 2,
            vec2(x, view.h * (0.12 + 0.17 * (i as f32 * 0.61 % 1.0))),
            160.0 + (i % 3) as f32 * 50.0,
            with_alpha(WHITE, 0.85),
        );
    }
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
                        sfx::play(Sfx::Win);
                        sfx::say(sfx::Say::Congrats);
                        feel::impact_tinted(Impact::Large, Color::from_rgba(255, 230, 120, 255));
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
                    sfx::say(sfx::Say::Go);
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
        draw_backdrop(view, self.t);
        let l = layout(view, self.board.word.len(), self.board.tiles.len());
        let k = ui_scale(view);
        set_view_camera(view, self.shake.offset());
        for i in 0..ROUND.min(self.deck.len()) {
            let c = vec2(
                view.w / 2.0 + (i as f32 - (ROUND as f32 - 1.0) / 2.0) * 44.0 * k,
                l.stars_y,
            );
            let s = if i == self.index && self.results.len() == i {
                1.0 + (self.t * 4.0).sin() * 0.08
            } else {
                1.0
            };
            let name = match self.results.get(i) {
                Some(true) => "star",
                _ => "star_empty",
            };
            let tint = match self.results.get(i) {
                Some(_) => WHITE,
                None if i == self.index => with_alpha(WHITE, 0.95),
                None => with_alpha(WHITE, 0.45),
            };
            crate::ui::sprite_at(
                name,
                c,
                38.0 * k * s,
                crate::art::Draw {
                    tint,
                    ..Default::default()
                },
            );
        }
        let p = l.picture;
        crate::ui::card(p, WHITE);
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
                art::sprite_ex(
                    "tile",
                    *r,
                    crate::art::Draw {
                        tint: Color::new(1.0, 1.0, 1.0, if next { 0.7 } else { 0.35 }),
                        ..Default::default()
                    },
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
        crate::ui::round_button(hr, "icon_question", crate::ui::Tone::Yellow, 0.0);
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
    fn tri(&self, a: (f32, f32), b: (f32, f32), c: (f32, f32), col: Color) {
        draw_triangle(self.p(a.0, a.1), self.p(b.0, b.1), self.p(c.0, c.1), col);
    }
    fn line(&self, a: (f32, f32), b: (f32, f32), w: f32, col: Color) {
        let (p, q) = (self.p(a.0, a.1), self.p(b.0, b.1));
        draw_line(p.x, p.y, q.x, q.y, w * self.s, col);
    }
}

fn rgb(r: u8, g: u8, b: u8) -> Color {
    Color::from_rgba(r, g, b, 255)
}

fn draw_sprite(p: &Pen, name: &str, rotation: f32) {
    draw_tinted(p, name, rotation, WHITE);
}

fn draw_tinted(p: &Pen, name: &str, rotation: f32, tint: Color) {
    let size = art::size(name);
    let k = 1.55 * p.s / size.x.max(size.y);
    let (w, h) = (size.x * k, size.y * k);
    soft_shadow(p.c + vec2(0.0, p.s * 0.92), w * 0.4, p.s * 0.07, 0.5);
    art::sprite_ex(
        name,
        Rect::new(p.c.x - w / 2.0, p.c.y - h / 2.0, w, h),
        crate::art::Draw {
            rotation,
            tint,
            ..Default::default()
        },
    );
}

macro_rules! sprite_pic {
    ($($pic:ident => $name:literal),* $(,)?) => {
        $(fn $pic(p: &Pen) {
            draw_sprite(p, $name, 0.0);
        })*
    };
}

sprite_pic! {
    cat => "cat",
    dog => "dog",
    bus => "bus",
    car => "car",
    hat => "hat",
    cup => "cup",
    egg => "egg",
    pig => "pig",
    tree => "tree",
    cake => "cake",
    ball => "ball",
    duck => "duck",
    frog => "frog",
    boat => "boat",
    house => "house",
    apple => "apple",
    heart => "heart",
    train => "train",
    snail => "snail",
    rocket => "rocket",
    banana => "banana",
}

fn sun(p: &Pen) {
    draw_tinted(p, "sun", p.t * 0.4, rgb(255, 205, 40));
}

fn moon(p: &Pen) {
    draw_tinted(p, "moon", 0.0, rgb(255, 215, 90));
}

fn cloud_pic(p: &Pen) {
    draw_tinted(p, "cloud", 0.0, rgb(175, 205, 245));
}

fn bee(p: &Pen) {
    draw_sprite(p, "bee", (p.t * 6.0).sin() * 0.08);
}

fn fish(p: &Pen) {
    draw_sprite(p, "fish", (p.t * 4.0).sin() * 0.08);
}

fn star_pic(p: &Pen) {
    draw_sprite(p, "star", (p.t * 2.0).sin() * 0.15);
}

fn flower(p: &Pen) {
    draw_sprite(p, "flower", (p.t * 1.5).sin() * 0.05);
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
    art::sprite("bg", r);
    crate::ui::cloud(
        4,
        vec2(r.x + r.w * 0.8, r.y + r.h * 0.14),
        r.w * 0.3,
        with_alpha(WHITE, 0.9),
    );
    cat(&Pen {
        c: vec2(r.x + r.w / 2.0, r.y + r.h * 0.3),
        s: r.w * 0.2,
        t: 0.0,
    });
    let s = r.w * 0.28;
    for (i, ch) in ['C', 'A', 'T'].iter().enumerate() {
        let x = r.x + r.w * 0.06 + i as f32 * (s + r.w * 0.04);
        tile(
            Rect::new(x, r.y + r.h * 0.62, s, s),
            *ch,
            letter_color(i),
            1.0,
        );
    }
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
    fn spell_sprites_all_exist() {
        let have = art::names();
        for n in SPRITES.iter().chain(["bg", "tile"].iter()) {
            assert!(have.iter().any(|h| h == n), "missing {n}");
        }
    }

    #[test]
    fn only_kite_and_rainbow_are_still_hand_drawn() {
        let drawn: Vec<&str> = WORDS
            .iter()
            .filter(|(w, _)| {
                let pen = |draw: Picture| draw as usize;
                let hand = [pen(kite), pen(rainbow)];
                hand.contains(&pen(picture(w).unwrap()))
            })
            .map(|(w, _)| *w)
            .collect();
        assert_eq!(drawn, vec!["kite", "rainbow"]);
    }

    #[test]
    fn finishing_a_word_cheers_aloud() {
        let mut g = Spell::with_deck(vec!["cat", "dog"]);
        let l = layout(&View::ipad_landscape(), 3, g.board.tiles.len());
        crate::sfx::take_voice();
        for ch in "cat".chars() {
            let i = (0..g.board.tiles.len())
                .find(|i| g.board.used[*i].is_none() && g.board.tiles[*i] == ch)
                .unwrap();
            g.tap_tile(i, &l);
        }
        assert_eq!(crate::sfx::take_voice(), Some(sfx::Say::Great));
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
    fn right_and_wrong_letters_sound_different() {
        let mut g = Spell::with_deck(vec!["cat"]);
        g.board = Board::with_tiles("cat", vec!['z', 'c', 'a', 't']);
        let l = layout(&View::ipad_landscape(), 3, 4);
        crate::sfx::take_pending();
        g.tap_tile(0, &l);
        assert_eq!(crate::sfx::take_pending(), vec![Sfx::Wrong]);
        g.tap_tile(1, &l);
        assert_eq!(crate::sfx::take_pending(), vec![Sfx::Pop]);
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
