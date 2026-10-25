use crate::engine::*;
use crate::feel::{self, Impact};
use crate::fx::*;
use crate::learn::{tall, ui_scale, PRAISE};
use crate::profile;
use crate::sfx::{self, Sfx};
use macroquad::prelude::*;

mod art {
    crate::sprites!("spell");
}

pub const KEY: &str = "spell";
pub const QUESTIONS: usize = 5;
pub const DOWN_SLIPS: u32 = 3;
const WIN_TIME: f32 = 1.6;
const SLIDE_TIME: f32 = 0.22;
const MIN_TILE: f32 = 96.0;
const GOLD: Color = Color::new(1.0, 0.82, 0.24, 1.0);

pub type Picture = fn(&Pen);

macro_rules! words {
    ($($w:ident),* $(,)?) => {
        pub const WORDS: [(&str, Picture); 63] = [
            $((stringify!($w), |p: &Pen| draw_picture(stringify!($w), p)),)*
        ];
    };
}

words!(
    cat, dog, sun, bus, car, hat, cup, egg, bee, pig, cow, fox, owl, hen, key, bed, fish, tree,
    cake, star, ball, duck, frog, moon, boat, kite, bear, bird, lion, shoe, drum, bell, house,
    apple, heart, train, cloud, snail, horse, mouse, zebra, tiger, clock, chair, lemon, robot,
    whale, plane, flower, rocket, banana, rainbow, monkey, rabbit, orange, turtle, spider, guitar,
    castle, pencil, penguin, giraffe, dolphin,
);

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Guide {
    Always,
    Idle(f32),
    Off,
}

#[derive(Clone, Copy, Debug)]
pub struct Level {
    pub name: &'static str,
    pub min_len: usize,
    pub max_len: usize,
    pub decoys: usize,
    pub hints: u32,
    pub guide: Guide,
}

const fn lv(
    name: &'static str,
    min_len: usize,
    max_len: usize,
    decoys: usize,
    hints: u32,
    guide: Guide,
) -> Level {
    Level {
        name,
        min_len,
        max_len,
        decoys,
        hints,
        guide,
    }
}

pub const LEVELS: [Level; 9] = [
    lv("3 letters", 3, 3, 0, 0, Guide::Always),
    lv("Tricky 3s", 3, 3, 2, 3, Guide::Idle(5.0)),
    lv("4 letters", 4, 4, 1, 3, Guide::Off),
    lv("Tricky 4s", 4, 4, 3, 3, Guide::Off),
    lv("5 letters", 5, 5, 2, 2, Guide::Off),
    lv("Tricky 5s", 5, 5, 4, 2, Guide::Off),
    lv("6 letters", 6, 6, 3, 2, Guide::Off),
    lv("Tricky 6s", 6, 6, 4, 1, Guide::Off),
    lv("Big words", 5, 7, 4, 1, Guide::Off),
];

pub fn words_in(level: usize) -> Vec<&'static str> {
    let l = LEVELS[level.min(LEVELS.len() - 1)];
    WORDS
        .iter()
        .map(|(w, _)| *w)
        .filter(|w| (l.min_len..=l.max_len).contains(&w.len()))
        .collect()
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

pub fn level_words(level: usize, avoid: &[&str]) -> Vec<&'static str> {
    let mut pool = words_in(level);
    let fresh: Vec<&'static str> = pool
        .iter()
        .copied()
        .filter(|w| !avoid.contains(w))
        .collect();
    if fresh.len() >= QUESTIONS {
        pool = fresh;
    }
    shuffle(&mut pool);
    pool.truncate(QUESTIONS);
    pool
}

pub fn stars_for(imperfect: u32) -> u8 {
    match imperfect {
        0..=1 => 3,
        2..=3 => 2,
        _ => 1,
    }
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
    pub fn new(word: &str, decoys: usize) -> Self {
        Self::with_tiles(word, make_tiles(word, decoys))
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
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Screen {
    Play,
    Levels,
    Done(f32),
    Final(f32),
}

pub struct Spell {
    pub level: usize,
    pub deck: Vec<&'static str>,
    pub index: usize,
    pub board: Board,
    pub results: Vec<bool>,
    pub phase: Phase,
    pub screen: Screen,
    pub hint_on: Option<usize>,
    pub hints_left: u32,
    pub slips: u32,
    pub slip_run: u32,
    pub finished: Option<u8>,
    pub rank: Option<usize>,
    pub teach: bool,
    pub idle: f32,
    recent: Vec<&'static str>,
    wobble: Vec<f32>,
    pop: Vec<f32>,
    slide: Vec<f32>,
    held: Option<(usize, Vec2)>,
    press: f32,
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
        let teach = profile::get(KEY, "played") == 0;
        let level = (profile::level(KEY) as usize)
            .min(profile::unlocked(KEY) as usize)
            .min(LEVELS.len() - 1);
        let mut deck = level_words(level, &[]);
        if teach && level == 0 {
            deck.retain(|w| *w != "cat");
            deck.insert(0, "cat");
            deck.truncate(QUESTIONS);
        }
        let mut g = Self::build(level, deck);
        g.teach = teach;
        g
    }

    pub fn with_deck(deck: Vec<&'static str>) -> Self {
        Self::build(1, deck)
    }

    pub fn with_level_deck(level: usize, deck: Vec<&'static str>) -> Self {
        Self::build(level, deck)
    }

    fn build(level: usize, deck: Vec<&'static str>) -> Self {
        let board = Board::new(deck[0], LEVELS[level].decoys);
        let n = board.tiles.len();
        Self {
            level,
            recent: deck.clone(),
            deck,
            index: 0,
            board,
            results: vec![],
            phase: Phase::Playing,
            screen: Screen::Play,
            hint_on: None,
            hints_left: LEVELS[level].hints,
            slips: 0,
            slip_run: 0,
            finished: None,
            rank: None,
            teach: false,
            idle: 0.0,
            wobble: vec![0.0; n],
            pop: vec![0.0; 8],
            slide: vec![1.0; n],
            held: None,
            press: 0.0,
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

    pub fn is_final_level(&self) -> bool {
        self.level + 1 == LEVELS.len()
    }

    pub fn start_level(&mut self, level: usize) {
        self.level = level.min(LEVELS.len() - 1);
        profile::set_level(KEY, self.level as u32);
        self.deck = level_words(self.level, &self.recent);
        self.recent = self.deck.clone();
        self.index = 0;
        self.results.clear();
        self.slips = 0;
        self.slip_run = 0;
        self.hints_left = LEVELS[self.level].hints;
        self.finished = None;
        self.rank = None;
        self.screen = Screen::Play;
        self.load();
    }

    fn load(&mut self) {
        self.board = Board::new(self.word(), LEVELS[self.level].decoys);
        let n = self.board.tiles.len();
        self.wobble = vec![0.0; n];
        self.slide = vec![1.0; n];
        self.pop = vec![0.0; self.board.word.len()];
        self.hint_on = None;
        self.held = None;
        self.idle = 0.0;
        self.phase = Phase::Playing;
        self.picture_bump.kick();
    }

    pub fn glow(&self) -> Option<usize> {
        if self.phase != Phase::Playing || self.screen != Screen::Play {
            return None;
        }
        if self.hint_on.is_some() {
            return self.hint_on;
        }
        let guided = match LEVELS[self.level].guide {
            Guide::Always => true,
            Guide::Idle(after) => self.idle >= after,
            Guide::Off => false,
        };
        if guided || self.board.mistakes >= 2 {
            self.board.hint()
        } else {
            None
        }
    }

    pub fn hint_available(&self) -> bool {
        LEVELS[self.level].guide != Guide::Always && self.hints_left > 0
    }

    fn complete_level(&mut self) {
        let stars = stars_for(self.slips);
        profile::award_stars(KEY, self.level as u32, stars);
        if !self.is_final_level() {
            profile::unlock(KEY, self.level as u32 + 1);
            profile::set_level(KEY, self.level as u32 + 1);
        } else {
            let total = profile::total_stars(KEY) as i64;
            if profile::personal_best(KEY).is_none_or(|b| total > b) {
                self.rank = profile::submit(KEY, total, profile::Better::Higher);
            }
        }
        self.finished = Some(stars);
    }

    pub fn advance(&mut self, view: &View) {
        if self.finished.take().is_some() {
            self.screen = if self.is_final_level() {
                Screen::Final(0.0)
            } else {
                Screen::Done(0.0)
            };
            sfx::play(Sfx::Win);
            sfx::say(sfx::Say::Congrats);
            feel::impact_tinted(Impact::Large, Color::from_rgba(255, 230, 120, 255));
            self.fx.confetti(view);
            return;
        }
        if self.slip_run >= DOWN_SLIPS && self.level > 0 {
            self.start_level(self.level - 1);
            self.pops.add(
                vec2(view.w / 2.0, view.h * 0.4),
                format!("Let's practise {}", LEVELS[self.level].name),
                WHITE,
                44.0,
            );
            return;
        }
        self.index += 1;
        self.load();
    }

    pub fn use_hint(&mut self) {
        if self.phase != Phase::Playing || !self.hint_available() {
            return;
        }
        if let Some(i) = self.board.hint() {
            sfx::play(Sfx::Tick);
            if self.hint_on != Some(i) {
                self.board.hints += 1;
                self.hints_left -= 1;
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
                self.idle = 0.0;
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
            if perfect {
                self.slip_run = 0;
            } else {
                self.slips += 1;
                self.slip_run += 1;
            }
            if self.teach {
                self.teach = false;
                profile::set(KEY, "played", 1);
            }
            if self.index + 1 >= self.deck.len() {
                self.complete_level();
            }
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

    fn levels_input(&mut self, input: &Input, view: &View) {
        if !input.pressed {
            return;
        }
        let l = levels_layout(view);
        if input.tapped_in(l.back) {
            sfx::play(Sfx::Click);
            if self.results.len() >= self.deck.len() {
                self.start_level((self.level + 1).min(LEVELS.len() - 1));
            }
            self.screen = Screen::Play;
            return;
        }
        let open = profile::unlocked(KEY) as usize;
        for (i, tile) in l.tiles.iter().enumerate() {
            if tile.contains(input.pos) && i <= open {
                sfx::play(Sfx::Click);
                self.start_level(i);
                return;
            }
        }
    }

    fn done_input(&mut self, input: &Input, view: &View, age: f32) {
        if !input.pressed || age < 0.45 {
            return;
        }
        let l = done_layout(view);
        if input.tapped_in(l.next) {
            sfx::play(Sfx::Click);
            self.start_level(self.level + 1);
        } else if input.tapped_in(l.again) {
            sfx::play(Sfx::Click);
            self.start_level(self.level);
        } else if input.tapped_in(l.levels) {
            sfx::play(Sfx::Click);
            self.screen = Screen::Levels;
        }
    }
}

pub struct LevelsLayout {
    pub panel: Rect,
    pub tiles: Vec<Rect>,
    pub back: Rect,
}

pub fn levels_layout(view: &View) -> LevelsLayout {
    let gap = 14.0;
    let pw = (view.w - 40.0).min(780.0);
    let tw = (pw - 80.0 - gap * 2.0) / 3.0;
    let th = ((view.h - 60.0 - 270.0 - gap * 3.0) / 3.0).clamp(80.0, tw * 0.95);
    let ph = 120.0 + 3.0 * (th + gap) + 150.0;
    let c = view.center() + vec2(0.0, 20.0);
    let panel = Rect::new(c.x - pw / 2.0, c.y - ph / 2.0, pw, ph);
    let tiles = (0..LEVELS.len())
        .map(|i| {
            Rect::new(
                panel.x + 40.0 + (i % 3) as f32 * (tw + gap),
                panel.y + 120.0 + (i / 3) as f32 * (th + gap),
                tw,
                th,
            )
        })
        .collect();
    let back = Rect::new(c.x - 150.0, panel.bottom() - 100.0, 300.0, 78.0);
    LevelsLayout { panel, tiles, back }
}

pub struct DoneLayout {
    pub panel: Rect,
    pub next: Rect,
    pub again: Rect,
    pub levels: Rect,
}

pub fn done_layout(view: &View) -> DoneLayout {
    let pw = 580.0f32.min(view.w - 40.0);
    let ph = 470.0;
    let c = view.center() + vec2(0.0, 20.0);
    let panel = Rect::new(c.x - pw / 2.0, c.y - ph / 2.0, pw, ph);
    DoneLayout {
        panel,
        next: Rect::new(c.x - 170.0, panel.y + 285.0, 340.0, 80.0),
        again: Rect::new(c.x - 215.0, panel.y + 380.0, 200.0, 60.0),
        levels: Rect::new(c.x + 15.0, panel.y + 380.0, 200.0, 60.0),
    }
}

pub fn levels_button(view: &View) -> Rect {
    let k = ui_scale(view);
    let w = 150.0 * k;
    Rect::new(view.w - 30.0 - w, 18.0, w, 46.0 * k)
}

fn plural_stars(n: i64) -> String {
    if n == 1 {
        "1 star".to_string()
    } else {
        format!("{n} stars")
    }
}

fn star_row(c: Vec2, r: f32, gap: f32, have: u8, total: u8, pop: f32) {
    for i in 0..total {
        let x = c.x + (i as f32 - (total as f32 - 1.0) / 2.0) * gap;
        let lit = i < have;
        let k = if lit { pop } else { 1.0 };
        star(
            vec2(x, c.y + 2.0),
            r * 1.08 * k,
            0.0,
            Color::new(0.1, 0.05, 0.25, 0.3),
        );
        star(
            vec2(x, c.y),
            r * k,
            0.0,
            if lit {
                GOLD
            } else {
                Color::new(0.75, 0.72, 0.82, 0.8)
            },
        );
    }
}

fn padlock(c: Vec2, r: f32) {
    let ink = Color::from_rgba(120, 112, 150, 255);
    draw_circle_lines(c.x, c.y - r * 0.55, r * 0.5, r * 0.18, ink);
    round_rect(
        Rect::new(c.x - r * 0.7, c.y - r * 0.3, r * 1.4, r * 1.0),
        r * 0.2,
        ink,
    );
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
    let top = 90.0 * k;
    let tall = tall(view);
    let max_tile = if tall { 200.0 * k } else { 130.0 * k };
    let gap = 14.0 * k;
    let slot_size = ((view.w - 60.0 - gap * (letters as f32 - 1.0)) / letters as f32).min(max_tile);
    let fits = |n: usize| (view.w - 60.0 - gap * (n as f32 - 1.0)) / n as f32 >= MIN_TILE;
    let mut per_row = if tall {
        tiles.min(if tiles > 6 { 5 } else { 4 })
    } else {
        tiles
    };
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
        self.press = approach(
            self.press,
            if input.down && levels_button(view).contains(input.pos) {
                1.0
            } else {
                0.0
            },
            20.0,
            dt,
        );
        match self.screen {
            Screen::Levels => {
                self.levels_input(input, view);
                return;
            }
            Screen::Done(age) => {
                self.screen = Screen::Done(age + dt);
                self.done_input(input, view, age);
                return;
            }
            Screen::Final(age) => {
                self.screen = Screen::Final(age + dt);
                if input.pressed && age > 0.8 {
                    self.start_level(self.level);
                    sfx::say(sfx::Say::Go);
                }
                return;
            }
            Screen::Play => {}
        }
        let l = layout(view, self.board.word.len(), self.board.tiles.len());
        if let Phase::Won(t) = self.phase {
            let t = t + dt;
            self.phase = Phase::Won(t);
            if t > WIN_TIME || (t > 0.6 && input.pressed) {
                self.advance(view);
            }
            return;
        }
        self.idle += dt;
        if input.tapped_in(levels_button(view)) {
            sfx::play(Sfx::Click);
            self.screen = Screen::Levels;
            return;
        }
        if input.pressed {
            if self.hint_available() && l.hint.contains(input.pos) {
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
                if moved < TAP_SLOP_MM * input.mm() || near_slot {
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
        for i in 0..self.deck.len() {
            let c = vec2(
                view.w / 2.0 + (i as f32 - (self.deck.len() as f32 - 1.0) / 2.0) * 44.0 * k,
                l.stars_y,
            );
            let s = if i == self.index && self.results.len() == i {
                1.0 + (self.t * 4.0).sin() * 0.08
            } else {
                1.0
            };
            let name = "p_star";
            let tint = match self.results.get(i) {
                Some(true) => WHITE,
                Some(false) => Color::new(0.45, 0.45, 0.5, 0.6),
                None if i == self.index => Color::new(0.7, 0.7, 0.75, 0.9),
                None => Color::new(0.5, 0.5, 0.55, 0.45),
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
        crate::ui::painted_panel(p, WHITE);
        let bob = (self.t * 2.0).sin() * p.h * 0.015;
        let sc = self.picture_bump.scale(0.12);
        draw_picture(
            self.word(),
            &Pen {
                c: p.center() + vec2(0.0, bob),
                s: p.w.min(p.h) * 0.42 * sc,
                t: self.t,
            },
        );
        let glow = self.glow();
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
            let hinted = glow == Some(i);
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
        if self.hint_available() {
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
            crate::ui::painted_round(hr, "icon_question", 0.0);
            text_outline(
                &self.hints_left.to_string(),
                h.x + h.w * 0.9,
                h.y + h.h * 0.12,
                h.h * 0.34,
                WHITE,
                Color::from_rgba(150, 60, 20, 255),
            );
        }
        self.fx.draw();
        self.pops.draw();
        self.draw_hud(view, &l);
        set_view_camera(view, Vec2::ZERO);
        match self.screen {
            Screen::Levels => self.draw_levels(view),
            Screen::Done(age) => self.draw_done(view, age),
            Screen::Final(_) => {
                let total = profile::total_stars(KEY);
                let line = format!("{total} of {} stars", LEVELS.len() * 3);
                results(
                    view,
                    &crate::ui::Results {
                        game: KEY,
                        title: "You did it!",
                        line: &line,
                        rank: self.rank,
                        action: "Play again",
                        format: plural_stars,
                    },
                );
            }
            Screen::Play => {}
        }
    }
}

impl Spell {
    fn draw_hud(&self, view: &View, l: &Layout) {
        let k = ui_scale(view);
        let label = format!("Level {}", self.level + 1);
        let tw = text_size(&label, 28.0 * k).width + 44.0 * k;
        let chip = Rect::new(HUD_LEFT, 18.0, tw, 46.0 * k);
        round_rect(
            Rect::new(chip.x, chip.y + 4.0, chip.w, chip.h),
            chip.h / 2.0,
            Color::new(0.0, 0.0, 0.0, 0.18),
        );
        round_rect(chip, chip.h / 2.0, with_alpha(WHITE, 0.94));
        text_center(
            &label,
            chip.center().x,
            chip.center().y,
            28.0 * k,
            Color::from_rgba(150, 60, 30, 255),
        );
        crate::ui::painted_button(levels_button(view), "Levels", "blue", self.press, 1.0);
        if self.teach && self.screen == Screen::Play && self.phase == Phase::Playing {
            let pulse = 1.0 + (self.t * 4.0).sin() * 0.03;
            crate::ui::hint(
                view,
                "Tap the letters to spell the word!",
                vec2(view.w / 2.0, l.stars_y + 44.0 * k),
                pulse,
            );
        } else {
            let goal = format!(
                "{}: spell {} words",
                LEVELS[self.level].name,
                self.deck.len()
            );
            text_outline(
                &goal,
                view.w / 2.0,
                l.stars_y + 40.0 * k,
                26.0 * k,
                WHITE,
                Color::from_rgba(120, 60, 40, 255),
            );
        }
    }

    fn draw_levels(&self, view: &View) {
        let l = levels_layout(view);
        fill_screen(view, Color::new(0.06, 0.04, 0.16, 0.55));
        crate::ui::painted_panel(l.panel, WHITE);
        crate::ui::ribbon(
            view,
            "Pick a level",
            vec2(view.w / 2.0, l.panel.y + 8.0),
            self.t,
        );
        let open = profile::unlocked(KEY) as usize;
        for (i, r) in l.tiles.iter().enumerate() {
            let locked = i > open;
            let current = i == self.level;
            let base = if locked {
                Color::from_rgba(200, 196, 214, 255)
            } else if current {
                Color::from_rgba(255, 214, 90, 255)
            } else {
                Color::from_rgba(255, 150, 120, 255)
            };
            round_rect(
                Rect::new(r.x, r.y + 5.0, r.w, r.h),
                16.0,
                darker(base, 0.65),
            );
            round_rect(*r, 16.0, base);
            text_outline(
                &(i + 1).to_string(),
                r.center().x,
                r.y + r.h * 0.33,
                r.h * 0.36,
                WHITE,
                with_alpha(darker(base, 0.45), if locked { 0.6 } else { 1.0 }),
            );
            if locked {
                padlock(vec2(r.center().x, r.y + r.h * 0.74), r.h * 0.2);
            } else {
                star_row(
                    vec2(r.center().x, r.y + r.h * 0.76),
                    r.h * 0.12,
                    r.h * 0.3,
                    profile::stars(KEY, i as u32),
                    3,
                    1.0,
                );
            }
        }
        text_center(
            LEVELS[self.level].name,
            view.w / 2.0,
            l.back.y - 26.0,
            30.0,
            crate::ui::BODY,
        );
        crate::ui::painted_button(l.back, "Back", "green", 0.0, 1.0);
    }

    fn draw_done(&self, view: &View, age: f32) {
        let l = done_layout(view);
        let fade = ease_out_cubic(age / 0.2);
        fill_screen(view, Color::new(0.06, 0.04, 0.16, 0.5 * fade));
        let s = ease_out_back(age / 0.32).max(0.01);
        let c = l.panel.center();
        let p = Rect::new(
            c.x - l.panel.w * s / 2.0,
            c.y - l.panel.h * s / 2.0,
            l.panel.w * s,
            l.panel.h * s,
        );
        crate::ui::painted_panel(p, WHITE);
        crate::ui::ribbon(view, "Level done!", vec2(c.x, p.y + 8.0 * s), age);
        let name = format!("{}: {}", self.level + 1, LEVELS[self.level].name);
        text_center(&name, c.x, p.y + 100.0 * s, 32.0 * s, crate::ui::BODY);
        let have = stars_for(self.slips);
        let pop = |i: u8| ease_out_back(((age - 0.25 - i as f32 * 0.18) / 0.3).clamp(0.0, 1.0));
        for i in 0..3u8 {
            let x = c.x + (i as f32 - 1.0) * 100.0 * s;
            let lit = i < have;
            let k = if lit { pop(i) } else { 1.0 };
            let r = 46.0 * s * k;
            star(
                vec2(x, p.y + 190.0 * s + 3.0),
                r * 1.05,
                0.0,
                Color::new(0.1, 0.05, 0.25, 0.3),
            );
            star(
                vec2(x, p.y + 190.0 * s),
                r,
                0.0,
                if lit {
                    GOLD
                } else {
                    Color::new(0.75, 0.72, 0.82, 0.8)
                },
            );
        }
        let a = ((age - 0.3) / 0.2).clamp(0.0, 1.0);
        let pulse = 1.0 + (age * 4.5).sin() * 0.03;
        let nb = l.next;
        let nb = Rect::new(
            nb.center().x - nb.w * pulse / 2.0,
            nb.center().y - nb.h * pulse / 2.0,
            nb.w * pulse,
            nb.h * pulse,
        );
        crate::ui::painted_button(nb, "Next level", "green", 0.0, a);
        crate::ui::painted_button(l.again, "Again", "orange", 0.0, a);
        crate::ui::painted_button(l.levels, "Levels", "blue", 0.0, a);
    }
}

pub struct Pen {
    pub c: Vec2,
    pub s: f32,
    pub t: f32,
}

fn draw_picture(word: &str, p: &Pen) {
    let rotation = match word {
        "sun" => p.t * 0.4,
        "bee" => (p.t * 6.0).sin() * 0.08,
        "fish" => (p.t * 4.0).sin() * 0.08,
        "star" => (p.t * 2.0).sin() * 0.15,
        "flower" | "kite" => (p.t * 1.5).sin() * 0.05,
        _ => 0.0,
    };
    let size = art::size(word);
    let k = 1.85 * p.s / size.x.max(size.y);
    let (w, h) = (size.x * k, size.y * k);
    soft_shadow(p.c + vec2(0.0, h * 0.5), w * 0.38, p.s * 0.07, 0.5);
    art::sprite_ex(
        word,
        Rect::new(p.c.x - w / 2.0, p.c.y - h / 2.0, w, h),
        crate::art::Draw {
            rotation,
            ..Default::default()
        },
    );
}

pub fn icon(r: Rect) {
    crate::ui::icon("spell", r);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn there_are_enough_words_in_every_level() {
        for level in 0..LEVELS.len() {
            assert!(words_in(level).len() >= QUESTIONS * 2, "level {level}");
        }
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
        for n in WORDS.iter().map(|(w, _)| w).chain(["bg", "tile"].iter()) {
            assert!(have.iter().any(|h| h == n), "missing {n}");
        }
    }

    #[test]
    fn every_word_has_a_picture() {
        for (w, _) in WORDS {
            assert!(art::frame(w).is_some(), "{w}");
        }
    }

    #[test]
    fn finishing_a_word_cheers_aloud() {
        let mut g = Spell::with_level_deck(0, vec!["cat", "dog"]);
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
    fn tiles_hold_every_letter_including_repeats() {
        for (w, _) in WORDS {
            for decoys in 0..5 {
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
            let tiles = make_tiles(w, 4);
            let extra: Vec<char> = tiles.iter().copied().filter(|c| !w.contains(*c)).collect();
            assert_eq!(extra.len(), 4, "{w}");
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

    fn spell(g: &mut Spell, word: &str, l: &Layout) {
        for ch in word.chars() {
            let i = (0..g.board.tiles.len())
                .find(|i| g.board.used[*i].is_none() && g.board.tiles[*i] == ch)
                .unwrap();
            g.tap_tile(i, l);
        }
    }

    fn slip(g: &mut Spell, l: &Layout) {
        let first = g.word().chars().next().unwrap();
        let wrong = (0..g.board.tiles.len())
            .find(|i| g.board.tiles[*i] != first)
            .unwrap();
        g.tap_tile(wrong, l);
    }

    fn solve_deck(g: &mut Spell, view: &View, perfect: bool) {
        while g.screen == Screen::Play {
            let word = g.word();
            let l = layout(view, word.len(), g.board.tiles.len());
            if !perfect {
                slip(g, &l);
            }
            spell(g, word, &l);
            g.advance(view);
        }
    }

    #[test]
    fn finishing_a_word_moves_to_the_next() {
        crate::save::reset_for_test();
        let view = View::ipad_landscape();
        let mut g = Spell::with_level_deck(0, vec!["cat", "dog"]);
        let l = layout(&view, 3, g.board.tiles.len());
        spell(&mut g, "cat", &l);
        assert!(matches!(g.phase, Phase::Won(_)));
        assert_eq!(g.stars(), 1);
        g.advance(&view);
        assert_eq!(g.word(), "dog");
        assert_eq!(g.phase, Phase::Playing);
    }

    #[test]
    fn right_and_wrong_letters_sound_different() {
        let mut g = Spell::with_level_deck(2, vec!["cat"]);
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
        let mut g = Spell::with_level_deck(2, vec!["duck", "dog"]);
        let l = layout(&View::ipad_landscape(), 4, g.board.tiles.len());
        g.use_hint();
        g.use_hint();
        assert_eq!(g.board.hints, 1);
        assert_eq!(g.hints_left, LEVELS[2].hints - 1);
        spell(&mut g, "duck", &l);
        assert_eq!(g.stars(), 0);
    }

    fn fresh(level: usize) -> Spell {
        crate::save::reset_for_test();
        crate::profile::create("Mia", 0);
        profile::unlock(KEY, level as u32);
        profile::set_level(KEY, level as u32);
        profile::set(KEY, "played", 1);
        let mut g = Spell::new();
        g.start_level(level);
        g
    }

    #[test]
    fn levels_step_through_word_lengths() {
        let lens: Vec<(usize, usize)> = LEVELS.iter().map(|l| (l.min_len, l.max_len)).collect();
        assert_eq!(lens[0], (3, 3));
        assert_eq!(lens[2], (4, 4));
        assert_eq!(lens[4], (5, 5));
        assert_eq!(lens[6], (6, 6));
        assert!(lens[..8].windows(2).all(|w| w[0].0 <= w[1].0));
        for (level, spec) in LEVELS.iter().enumerate() {
            for w in words_in(level) {
                assert!(
                    (spec.min_len..=LEVELS[level].max_len).contains(&w.len()),
                    "{w} in level {level}"
                );
            }
        }
    }

    #[test]
    fn decoys_rise_and_hints_shrink_with_level() {
        assert_eq!(LEVELS[0].decoys, 0);
        assert!(LEVELS[8].decoys > LEVELS[1].decoys);
        assert!(LEVELS.windows(2).skip(1).all(|w| w[0].hints >= w[1].hints));
        assert!(LEVELS[7].hints < LEVELS[2].hints);
        for (level, spec) in LEVELS.iter().enumerate() {
            let g = fresh(level);
            let need = g.word().len() + spec.decoys;
            assert_eq!(g.board.tiles.len(), need);
            assert_eq!(g.hints_left, spec.hints);
        }
    }

    #[test]
    fn first_level_shows_the_next_letter_and_has_no_traps() {
        let g = fresh(0);
        assert_eq!(g.board.tiles.len(), 3);
        let i = g.glow().expect("next letter glows");
        assert_eq!(g.board.tiles[i], g.board.word[0]);
        assert!(!g.hint_available());
    }

    #[test]
    fn later_levels_do_not_glow_until_a_child_is_stuck() {
        let view = View::ipad_landscape();
        let mut g = fresh(4);
        assert_eq!(g.glow(), None);
        let l = layout(&view, g.board.word.len(), g.board.tiles.len());
        slip(&mut g, &l);
        slip(&mut g, &l);
        assert!(g.glow().is_some());
    }

    #[test]
    fn level_two_hints_after_five_idle_seconds() {
        let view = View::ipad_landscape();
        let mut g = fresh(1);
        assert_eq!(g.glow(), None);
        for _ in 0..320 {
            g.update(&Input::default(), &view, 1.0 / 60.0);
        }
        assert!(g.glow().is_some());
    }

    #[test]
    fn a_new_player_starts_on_cat_with_a_hint_line() {
        crate::save::reset_for_test();
        crate::profile::create("Mia", 0);
        let g = Spell::new();
        assert_eq!(g.level, 0);
        assert_eq!(g.word(), "cat");
        assert!(g.teach);
        assert_eq!(g.deck.len(), QUESTIONS);
    }

    #[test]
    fn first_success_ends_the_teaching() {
        crate::save::reset_for_test();
        crate::profile::create("Mia", 0);
        let mut g = Spell::new();
        let l = layout(&View::ipad_landscape(), 3, g.board.tiles.len());
        spell(&mut g, "cat", &l);
        assert!(!g.teach);
        assert_eq!(profile::get(KEY, "played"), 1);
        assert!(!Spell::new().teach);
    }

    #[test]
    fn a_level_is_five_words_then_stars_and_unlock() {
        let view = View::ipad_landscape();
        let mut g = fresh(0);
        assert_eq!(g.deck.len(), QUESTIONS);
        solve_deck(&mut g, &view, true);
        assert!(matches!(g.screen, Screen::Done(_)));
        assert_eq!(profile::stars(KEY, 0), 3);
        assert_eq!(profile::unlocked(KEY), 1);
        assert_eq!(profile::level(KEY), 1);
    }

    #[test]
    fn messy_levels_earn_fewer_stars() {
        let view = View::ipad_landscape();
        let mut g = fresh(2);
        for round in 0..QUESTIONS {
            let word = g.word();
            let l = layout(&view, word.len(), g.board.tiles.len());
            if round != 2 {
                slip(&mut g, &l);
            }
            spell(&mut g, word, &l);
            g.advance(&view);
        }
        assert_eq!(profile::stars(KEY, 2), 1);
        assert_eq!(stars_for(0), 3);
        assert_eq!(stars_for(1), 3);
        assert_eq!(stars_for(2), 2);
        assert_eq!(stars_for(5), 1);
    }

    #[test]
    fn a_worse_run_keeps_the_best_stars() {
        let view = View::ipad_landscape();
        let mut g = fresh(0);
        solve_deck(&mut g, &view, true);
        g.start_level(0);
        solve_deck(&mut g, &view, false);
        assert_eq!(profile::stars(KEY, 0), 3);
    }

    #[test]
    fn restarting_resumes_at_the_saved_level_for_that_player() {
        let view = View::ipad_landscape();
        let mut g = fresh(0);
        solve_deck(&mut g, &view, true);
        let again = Spell::new();
        assert_eq!(again.level, 1);
        assert_eq!(again.deck.len(), QUESTIONS);
        crate::profile::create("Leo", 3);
        let other = Spell::new();
        assert_eq!(other.level, 0);
        assert_eq!(profile::unlocked(KEY), 0);
    }

    #[test]
    fn resume_never_jumps_past_the_unlocked_level() {
        crate::save::reset_for_test();
        crate::profile::create("Mia", 0);
        profile::set_level(KEY, 6);
        assert_eq!(Spell::new().level, 0);
        profile::unlock(KEY, 3);
        assert_eq!(Spell::new().level, 3);
    }

    #[test]
    fn three_slips_in_a_row_drop_back_a_level() {
        let view = View::ipad_landscape();
        let mut g = fresh(3);
        for _ in 0..DOWN_SLIPS {
            let word = g.word();
            let l = layout(&view, word.len(), g.board.tiles.len());
            slip(&mut g, &l);
            spell(&mut g, word, &l);
            g.advance(&view);
        }
        assert_eq!(g.level, 2);
        assert_eq!(g.index, 0);
        assert_eq!(profile::level(KEY), 2);
        assert_eq!(profile::unlocked(KEY), 3);
    }

    #[test]
    fn a_perfect_word_resets_the_slip_run() {
        let view = View::ipad_landscape();
        let mut g = fresh(3);
        for round in 0..DOWN_SLIPS {
            let word = g.word();
            let l = layout(&view, word.len(), g.board.tiles.len());
            if round != 1 {
                slip(&mut g, &l);
            }
            spell(&mut g, word, &l);
            g.advance(&view);
        }
        assert_eq!(g.level, 3);
    }

    #[test]
    fn level_one_never_drops_below_itself() {
        let view = View::ipad_landscape();
        let mut g = fresh(0);
        for _ in 0..4 {
            let word = g.word();
            let l = layout(&view, word.len(), g.board.tiles.len());
            slip(&mut g, &l);
            spell(&mut g, word, &l);
            g.advance(&view);
        }
        assert_eq!(g.level, 0);
    }

    #[test]
    fn a_level_does_not_repeat_the_previous_words() {
        let mut g = fresh(2);
        let first = g.deck.clone();
        g.start_level(2);
        assert!(g.deck.iter().all(|w| !first.contains(w)));
    }

    #[test]
    fn locked_levels_cannot_be_opened() {
        let view = View::ipad_landscape();
        let mut g = fresh(0);
        g.screen = Screen::Levels;
        let l = levels_layout(&view);
        g.update(&Input::press_at(l.tiles[5].center()), &view, 0.016);
        assert_eq!(g.level, 0);
        assert_eq!(g.screen, Screen::Levels);
        profile::unlock(KEY, 5);
        g.update(&Input::press_at(l.tiles[5].center()), &view, 0.016);
        assert_eq!(g.level, 5);
        assert_eq!(g.screen, Screen::Play);
    }

    #[test]
    fn finishing_the_last_level_submits_total_stars() {
        let view = View::ipad_landscape();
        let mut g = fresh(LEVELS.len() - 1);
        solve_deck(&mut g, &view, true);
        assert!(matches!(g.screen, Screen::Final(_)));
        assert_eq!(g.rank, Some(0));
        assert_eq!(profile::board(KEY)[0].score, 3);
    }

    #[test]
    fn level_screens_fit_both_orientations() {
        for v in [
            View::ipad_landscape(),
            View::ipad_portrait(),
            View::fit(1080.0, 2400.0),
        ] {
            let l = levels_layout(&v);
            for r in l.tiles.iter().chain([&l.back]) {
                assert!(r.x >= 0.0 && r.right() <= v.w && r.y >= 0.0 && r.bottom() <= v.h);
            }
            assert!(l.back.y > l.tiles.last().unwrap().bottom());
            let d = done_layout(&v);
            for r in [d.next, d.again, d.levels] {
                assert!(r.x >= 0.0 && r.right() <= v.w && r.bottom() <= v.h);
            }
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
