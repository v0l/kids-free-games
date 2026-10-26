use crate::engine::*;
use crate::feel::{self, Impact};
use crate::fx::*;
pub use crate::games::spell_words::{words, Word};
use crate::geom::*;
use crate::learn::{tall, ui_scale, PRAISE};
use crate::mastery;
use crate::profile;
use crate::rng as rand;
use crate::sfx::{self, Sfx};
use glam::vec2;

pub const KEY: &str = "spell";
pub const QUESTIONS: usize = 5;
pub const DOWN_SLIPS: u32 = 3;
const WIN_TIME: f32 = 1.6;
const SLIDE_TIME: f32 = 0.22;
const MIN_TILE: f32 = 96.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Guide {
    Always,
    Idle(f32),
    Off,
}

#[derive(Clone, Copy, Debug)]
pub struct Level {
    pub name: &'static str,
    pub bands: &'static [u8],
    pub min_len: usize,
    pub max_len: usize,
    pub decoys: usize,
    pub hints: u32,
    pub guide: Guide,
}

const fn lv(
    name: &'static str,
    bands: &'static [u8],
    lens: (usize, usize),
    decoys: usize,
    hints: u32,
    guide: Guide,
) -> Level {
    Level {
        name,
        bands,
        min_len: lens.0,
        max_len: lens.1,
        decoys,
        hints,
        guide,
    }
}

pub const LEVELS: [Level; 9] = [
    lv("Cat words", &[0], (3, 3), 0, 0, Guide::Always),
    lv("Blends", &[1], (4, 4), 1, 3, Guide::Idle(5.0)),
    lv("Blends and digraphs", &[1, 2], (4, 5), 1, 3, Guide::Off),
    lv("Sh, ch, th, ck", &[2, 3], (4, 5), 1, 3, Guide::Off),
    lv("Silent e and pairs", &[3], (5, 6), 2, 2, Guide::Off),
    lv("Mixed fives and sixes", &[3, 4], (5, 6), 2, 2, Guide::Off),
    lv("Tricky patterns", &[4], (5, 6), 2, 2, Guide::Off),
    lv("Long words", &[4], (6, 8), 2, 1, Guide::Off),
    lv("Word champion", &[3, 4], (6, 9), 3, 1, Guide::Off),
];

pub fn words_in(level: usize) -> Vec<&'static str> {
    let l = LEVELS[level.min(LEVELS.len() - 1)];
    words()
        .iter()
        .filter(|w| l.bands.contains(&w.band) && (l.min_len..=l.max_len).contains(&w.text.len()))
        .map(|w| w.text)
        .collect()
}

fn shuffle<T>(v: &mut [T]) {
    for i in (1..v.len()).rev() {
        v.swap(i, rand::gen_range(0, i + 1));
    }
}

fn confusables(c: char) -> &'static str {
    match c {
        'b' => "dp",
        'd' => "bg",
        'p' => "qb",
        'q' => "pg",
        'm' => "nw",
        'n' => "mh",
        'a' => "eo",
        'e' => "ai",
        'i' => "el",
        'o' => "ua",
        'u' => "ov",
        'c' => "ks",
        'k' => "ch",
        's' => "cz",
        'g' => "jq",
        'j' => "g",
        't' => "fl",
        'f' => "t",
        'l' => "it",
        'v' => "uw",
        'w' => "mv",
        'h' => "n",
        _ => "",
    }
}

pub fn make_tiles(word: &str, decoys: usize) -> Vec<char> {
    let letters: Vec<char> = word.chars().collect();
    let mut likely: Vec<char> = vec![];
    for c in &letters {
        for x in confusables(*c).chars() {
            if !letters.contains(&x) && !likely.contains(&x) {
                likely.push(x);
            }
        }
    }
    shuffle(&mut likely);
    let mut spare: Vec<char> = ('a'..='z')
        .filter(|c| !letters.contains(c) && !likely.contains(c))
        .collect();
    shuffle(&mut spare);
    let mut tiles = letters.clone();
    tiles.extend(likely.into_iter().chain(spare).take(decoys));
    for _ in 0..8 {
        shuffle(&mut tiles);
        if tiles[..letters.len()] != letters[..] || letters.iter().all(|c| *c == letters[0]) {
            break;
        }
    }
    tiles
}

pub const TARGET_SECS: f32 = 12.0;

pub fn mistakes_before_glow(level: usize) -> Option<u32> {
    match level {
        0..=3 => Some(2),
        4..=5 => Some(3),
        _ => None,
    }
}

pub fn shows_next_slot(level: usize) -> bool {
    level < 4
}

pub const GAPS: [u32; 5] = [6, 12, 30, 80, 200];
pub const MAX_DUE: usize = 2;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Memory {
    pub at: u32,
    pub due: u32,
    pub rung: u32,
}

impl Memory {
    pub fn seen(&self) -> bool {
        self.at > 0
    }

    pub fn after(self, round: u32, perfect: bool) -> Memory {
        let rung = if perfect {
            (self.rung + 1).min(GAPS.len() as u32 - 1)
        } else {
            0
        };
        Memory {
            at: round,
            due: round + GAPS[rung as usize],
            rung,
        }
    }
}

pub fn memory_of(word: &str) -> Memory {
    Memory {
        at: profile::get(KEY, &format!("w.{word}.at")) as u32,
        due: profile::get(KEY, &format!("w.{word}.due")) as u32,
        rung: profile::get(KEY, &format!("w.{word}.rung")) as u32,
    }
}

fn remember(word: &str, m: Memory) {
    profile::set(KEY, &format!("w.{word}.at"), m.at as i64);
    profile::set(KEY, &format!("w.{word}.due"), m.due as i64);
    profile::set(KEY, &format!("w.{word}.rung"), m.rung as i64);
}

pub fn round() -> u32 {
    profile::get(KEY, "round") as u32
}

pub fn choose(
    pool: &[&'static str],
    memory: &dyn Fn(&str) -> Memory,
    round: u32,
    avoid: &[&str],
    n: usize,
) -> Vec<&'static str> {
    let mut open: Vec<&'static str> = pool
        .iter()
        .copied()
        .filter(|w| !avoid.contains(w))
        .collect();
    if open.len() < n {
        open = pool.to_vec();
    }
    shuffle(&mut open);
    let mut due: Vec<&'static str> = open
        .iter()
        .copied()
        .filter(|w| {
            let m = memory(w);
            m.seen() && round >= m.due
        })
        .collect();
    due.sort_by_key(|w| memory(w).due);
    due.truncate(MAX_DUE);
    let mut deck = due;
    for w in open.iter().filter(|w| !memory(w).seen()) {
        if deck.len() >= n {
            break;
        }
        if !deck.contains(w) {
            deck.push(w);
        }
    }
    let mut rest: Vec<&'static str> = open.iter().copied().filter(|w| !deck.contains(w)).collect();
    rest.sort_by_key(|w| memory(w).at);
    for w in rest {
        if deck.len() >= n {
            break;
        }
        deck.push(w);
    }
    deck.truncate(n);
    shuffle(&mut deck);
    deck
}

pub fn level_words(level: usize, avoid: &[&str]) -> Vec<&'static str> {
    choose(&words_in(level), &memory_of, round(), avoid, QUESTIONS)
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
    pub word_secs: f32,
    pub recent: Vec<&'static str>,
    pub wobble: Vec<f32>,
    pub pop: Vec<f32>,
    pub slide: Vec<f32>,
    pub press: f32,
    pub t: f32,
    pub fx: Particles,
    pub pops: Popups,
    pub shake: Shake,
    pub picture_bump: Bump,
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
        sfx::say(sfx::Say::Word(deck[0]));
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
            word_secs: 0.0,
            wobble: vec![0.0; n],
            pop: vec![0.0; 8],
            slide: vec![1.0; n],
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

    pub fn say_word(&self) {
        sfx::say(sfx::Say::Word(self.word()));
    }

    fn load(&mut self) {
        self.say_word();
        self.board = Board::new(self.word(), LEVELS[self.level].decoys);
        let n = self.board.tiles.len();
        self.wobble = vec![0.0; n];
        self.slide = vec![1.0; n];
        self.pop = vec![0.0; self.board.word.len()];
        self.hint_on = None;
        self.idle = 0.0;
        self.word_secs = 0.0;
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
        let stuck = mistakes_before_glow(self.level).is_some_and(|m| self.board.mistakes >= m);
        if guided || stuck {
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

    fn record_word(&mut self, perfect: bool) {
        let word = self.word();
        let round = round() + 1;
        profile::set(KEY, "round", round as i64);
        remember(word, memory_of(word).after(round, perfect));
        let secs = self.word_secs.min(mastery::MAX_SAMPLE_SECS);
        mastery::record(
            KEY,
            self.level,
            mastery::Answer {
                first_try: self.board.mistakes == 0,
                secs,
                helped: self.board.hints > 0,
            },
            TARGET_SECS,
        );
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
            self.record_word(perfect);
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

pub fn letter_color(i: usize) -> Color {
    hsv(0.02 + i as f32 * 0.13, 0.6, 0.98)
}

pub fn levels_button(view: &View) -> Rect {
    let k = ui_scale(view);
    let w = 150.0 * k;
    Rect::new(view.w - 30.0 - w, 18.0, w, 46.0 * k)
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
        self.word_secs += dt;
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
                self.say_word();
            }
            if let Some(i) = l.tiles.iter().position(|r| r.contains(input.pos)) {
                if self.board.used[i].is_none() {
                    self.tap_tile(i, &l);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn there_are_enough_words_in_every_level() {
        for level in 0..LEVELS.len() {
            assert!(
                words_in(level).len() >= 40,
                "level {level}: {}",
                words_in(level).len()
            );
        }
        assert!(words().len() >= 600);
        let mut seen: Vec<&str> = words().iter().map(|w| w.text).collect();
        seen.sort();
        seen.dedup();
        assert_eq!(seen.len(), words().len());
        assert!(words()
            .iter()
            .all(|w| w.text.chars().all(|c| c.is_ascii_lowercase())));
    }

    #[test]
    fn spell_sprites_all_exist() {
        let b = std::fs::read(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../assets/art/spell.kga"
        ))
        .unwrap();
        let idx = crate::art::parse(&b).unwrap();
        for n in words().iter().map(|w| w.text).chain(["bg", "tile"]) {
            assert!(idx.frames.contains_key(n), "missing {n}");
        }
    }

    #[test]
    fn every_word_is_spoken() {
        let pack = sfx::packed();
        for w in words() {
            let key = sfx::Say::Word(w.text).key();
            assert!(
                pack.iter().any(|(n, _)| *n == key),
                "no voice for {}",
                w.text
            );
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
        for w in words().iter().map(|w| w.text) {
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
        for w in words().iter().map(|w| w.text) {
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

    fn quick_word(g: &mut Spell) -> (Layout, View, Vec<usize>) {
        crate::save::reset_for_test();
        *g = Spell::with_level_deck(2, vec!["cat"]);
        g.board = Board::with_tiles("cat", vec!['z', 'c', 'a', 't']);
        g.phase = Phase::Playing;
        let view = View::ipad_landscape();
        let l = layout(&view, 3, 4);
        (l, view, vec![1, 2, 3])
    }

    #[test]
    fn the_next_letter_tapped_before_the_last_one_lifts_is_still_right() {
        let mut g = Spell::with_level_deck(2, vec!["cat"]);
        let (l, view, order) = quick_word(&mut g);
        let (a, b) = (l.tiles[order[0]].center(), l.tiles[order[1]].center());
        g.update(
            &Input {
                mm: 5.0,
                ..Input::press_at(a)
            },
            &view,
            0.016,
        );
        g.update(
            &Input {
                mm: 5.0,
                ..Input::press_at(b)
            },
            &view,
            0.016,
        );
        g.update(
            &Input {
                pos: a,
                prev_pos: b,
                released: true,
                mm: 5.0,
                ..Default::default()
            },
            &view,
            0.016,
        );
        assert_eq!(g.board.filled, 2);
        assert_eq!(g.board.mistakes, 0);
        assert!(g.wobble.iter().all(|w| *w == 0.0));
    }

    #[test]
    fn a_lift_and_a_new_tap_in_one_frame_keep_their_order() {
        let mut g = Spell::with_level_deck(2, vec!["cat"]);
        let (l, view, order) = quick_word(&mut g);
        let (a, b) = (l.tiles[order[0]].center(), l.tiles[order[1]].center());
        g.update(
            &Input {
                mm: 5.0,
                ..Input::press_at(a)
            },
            &view,
            0.016,
        );
        g.update(
            &Input {
                released: true,
                mm: 5.0,
                ..Input::press_at(b)
            },
            &view,
            0.016,
        );
        assert_eq!(g.board.filled, 2);
        assert_eq!(g.board.mistakes, 0);
    }

    #[test]
    fn a_second_finger_lifting_elsewhere_does_not_reject_the_letter() {
        let mut g = Spell::with_level_deck(2, vec!["cat"]);
        let (l, view, order) = quick_word(&mut g);
        let a = l.tiles[order[0]].center();
        let thumb = vec2(view.w - 20.0, view.h - 20.0);
        g.update(
            &Input {
                mm: 5.0,
                ..Input::press_at(a)
            },
            &view,
            0.016,
        );
        g.update(
            &Input {
                mm: 5.0,
                ..Input::press_at(thumb)
            },
            &view,
            0.016,
        );
        g.update(
            &Input {
                pos: thumb,
                prev_pos: thumb,
                released: true,
                mm: 5.0,
                ..Default::default()
            },
            &view,
            0.016,
        );
        assert_eq!(g.board.filled, 1);
        assert!(g.wobble.iter().all(|w| *w == 0.0));
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
        assert_eq!(lens[1], (4, 4));
        assert_eq!(lens[4], (5, 6));
        assert_eq!(lens[7], (6, 8));
        assert!(lens
            .windows(2)
            .all(|w| w[0].0 <= w[1].0 && w[0].1 <= w[1].1));
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
        assert_eq!(g.glow(), None);
        slip(&mut g, &l);
        assert!(g.glow().is_some());
    }

    #[test]
    fn the_top_levels_never_glow() {
        let view = View::ipad_landscape();
        let mut g = fresh(7);
        let l = layout(&view, g.board.word.len(), g.board.tiles.len());
        for _ in 0..4 {
            slip(&mut g, &l);
        }
        for _ in 0..1200 {
            g.update(&Input::default(), &view, 1.0 / 60.0);
        }
        assert_eq!(g.glow(), None);
    }

    #[test]
    fn higher_levels_help_less_and_hide_the_next_slot() {
        assert!(shows_next_slot(0) && !shows_next_slot(8));
        let help: Vec<u32> = (0..9)
            .map(|l| mistakes_before_glow(l).unwrap_or(99))
            .collect();
        assert!(help.windows(2).all(|w| w[0] <= w[1]));
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

    #[test]
    fn a_big_pool_mostly_shows_new_words_across_replays() {
        crate::save::reset_for_test();
        crate::profile::create("Mia", 0);
        let mut seen = std::collections::HashSet::new();
        let mut total = 0;
        for _ in 0..6 {
            for w in level_words(4, &[]) {
                total += 1;
                seen.insert(w);
                remember(w, memory_of(w).after(round() + 1, true));
                profile::set(KEY, "round", round() as i64 + 1);
            }
        }
        assert!(
            seen.len() as f32 >= total as f32 * 0.75,
            "{} of {total}",
            seen.len()
        );
    }

    #[test]
    fn a_missed_word_comes_back_soon_and_a_mastered_one_waits() {
        let pool = ["aaa", "bbb", "ccc", "ddd", "eee", "fff", "ggg", "hhh"];
        let memory = |w: &str| match w {
            "aaa" => Memory {
                at: 1,
                due: 7,
                rung: 0,
            },
            "bbb" => Memory {
                at: 1,
                due: 201,
                rung: 4,
            },
            _ => Memory::default(),
        };
        let deck = choose(&pool, &memory, 10, &[], 5);
        assert!(deck.contains(&"aaa"));
        assert!(!deck.contains(&"bbb"));
        let early = choose(&pool, &memory, 3, &[], 5);
        assert!(!early.contains(&"aaa"));
    }

    #[test]
    fn spacing_grows_with_each_perfect_spelling_and_resets_on_a_slip() {
        let mut m = Memory::default();
        let mut gaps = vec![];
        for round in 1..=4 {
            m = m.after(round * 1000, true);
            gaps.push(m.due - round * 1000);
        }
        assert!(gaps.windows(2).all(|w| w[0] < w[1]));
        let slipped = m.after(9000, false);
        assert_eq!(slipped.due - 9000, GAPS[0]);
    }

    #[test]
    fn new_words_are_preferred_over_seen_ones() {
        let pool = ["aaa", "bbb", "ccc", "ddd", "eee", "fff"];
        let memory = |w: &str| {
            if w <= "ccc" {
                Memory {
                    at: 5,
                    due: 500,
                    rung: 2,
                }
            } else {
                Memory::default()
            }
        };
        let deck = choose(&pool, &memory, 10, &[], 3);
        assert!(deck.iter().all(|w| *w > "ccc"));
    }

    #[test]
    fn decoys_include_look_alike_letters() {
        let mut hit = 0;
        for _ in 0..40 {
            let t = make_tiles("bed", 2);
            if t.iter().any(|c| "dpgaiu".contains(*c)) {
                hit += 1;
            }
        }
        assert!(hit >= 38);
    }

    #[test]
    fn every_word_is_in_the_level_its_band_says() {
        for (i, l) in LEVELS.iter().enumerate() {
            for w in words_in(i) {
                assert!(l
                    .bands
                    .contains(&crate::games::spell_words::band_of(w).unwrap()));
            }
        }
        let covered: std::collections::HashSet<&str> =
            (0..LEVELS.len()).flat_map(words_in).collect();
        assert!(covered.len() >= 450, "{}", covered.len());
    }

    #[test]
    fn a_strong_player_masters_levels_a_weak_one_does_not() {
        let view = View::ipad_landscape();
        let strong = |level: usize| {
            let mut g = fresh(level);
            solve_deck(&mut g, &view, true);
            g.slips
        };
        let weak = |level: usize| {
            let mut g = fresh(level);
            solve_deck(&mut g, &view, false);
            g.slips
        };
        for level in [0, 4, 8] {
            assert_eq!(strong(level), 0, "level {level}");
            assert!(weak(level) >= 4, "level {level}");
        }
    }

    #[test]
    fn finishing_a_word_updates_its_memory_and_the_round() {
        let view = View::ipad_landscape();
        let mut g = fresh(0);
        let first = g.word();
        assert_eq!(memory_of(first), Memory::default());
        solve_deck(&mut g, &view, true);
        assert_eq!(round(), QUESTIONS as u32);
        assert!(memory_of(first).seen());
        assert!(memory_of(first).due > memory_of(first).at);
    }
}
