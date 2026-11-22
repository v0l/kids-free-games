use crate::engine::*;
use crate::feel::{self, Impact};
use crate::fx::*;
use crate::games::spell::{self, Memory};
use crate::geom::*;
use crate::learn::*;
use crate::mastery;
use crate::profile;
use crate::rng as rand;
use crate::sfx::{self, Sfx};
use glam::{vec2, Vec2};

pub const GAME: &str = "build";
pub const LEVELS: usize = 5;
pub const RECENT: usize = 6;
pub const CELEBRATE: f32 = 1.6;
pub const HUD_TITLE: &str = "Level 5";
pub const HUD_GOAL: &str = "Get 4 right in a row to reach Middle letter";
const SELECT_ROWS: usize = 3;

pub fn level_name(level: usize) -> &'static str {
    [
        "First letter",
        "Last letter",
        "Middle letter",
        "Two letters",
        "Longer words",
    ][level.min(LEVELS - 1)]
}

pub fn level_example(level: usize) -> &'static str {
    ["_at", "ca_", "c_t", "_a_e", "s_a_l"][level.min(LEVELS - 1)]
}

pub fn gap_count(level: usize) -> usize {
    if level >= 3 {
        2
    } else {
        1
    }
}

pub fn decoys(level: usize) -> usize {
    [2, 2, 3, 3, 4][level.min(LEVELS - 1)]
}

pub fn target_secs(level: usize) -> f32 {
    [7.0, 7.0, 8.0, 12.0, 14.0][level.min(LEVELS - 1)]
}

pub fn stars_score(s: i64) -> String {
    format!("{s} stars")
}

const DROPPED: &[&str] = &[
    "bat", "ram", "cob", "cot", "cab", "vet", "yam", "elf", "gem", "hog", "rib", "saw", "fin",
    "peg", "tap", "hug", "can", "cap", "tub", "cub", "pup", "bib", "ink", "jet", "net", "mat",
    "lid", "bun", "yak", "zip", "wig", "web", "nut", "bin", "rug", "mop", "gum", "cob", "hut",
    "swim", "jump", "drink", "drop", "trap", "thick", "bright", "flight", "night", "three",
    "climb", "knock", "write", "crack", "track", "check", "chunk", "crumb", "match", "patch",
    "ditch", "punch", "lunch", "spring", "splash", "float", "quack", "bloom", "blast", "stamp",
    "clamp", "stump", "plank", "ramp", "glass", "sink", "golf", "wand", "crisp", "scoop", "flask",
    "drill", "grill", "slug", "clam", "crib", "tusk", "blimp", "skunk", "trunk", "tram", "raft",
    "twig", "stem", "stick", "snack", "block", "plug", "plate", "tank", "bank", "gift", "belt",
    "witch", "gnome", "chess", "shrub", "wrist", "finch", "sloth", "prince", "knight", "beads",
    "beans", "jeans", "shorts", "shower", "trash", "chimp", "church", "branch", "bunch", "thumb",
    "father", "whisk", "shrimp", "squid", "quilt", "sponge", "chain", "chalk", "cherry", "brush",
    "clock", "jacket", "pocket", "ticket", "bucket", "chunk", "tennis", "carpet", "bridge",
    "sunset", "photo", "stove", "shell", "shelf", "swing", "string",
];

fn dictionary() -> &'static std::collections::HashSet<&'static str> {
    static D: std::sync::OnceLock<std::collections::HashSet<&'static str>> =
        std::sync::OnceLock::new();
    D.get_or_init(|| spell::words().iter().map(|w| w.text).collect())
}

pub fn is_word(s: &str) -> bool {
    dictionary().contains(s)
}

pub fn pool(level: usize) -> Vec<&'static str> {
    let (bands, lens): (&[u8], (usize, usize)) = match level {
        0..=2 => (&[0], (3, 3)),
        3 => (&[0, 1], (4, 5)),
        _ => (&[1, 2, 3], (5, 6)),
    };
    spell::words()
        .iter()
        .filter(|w| bands.contains(&w.band) && (lens.0..=lens.1).contains(&w.text.len()))
        .filter(|w| w.text.chars().all(|c| c.is_ascii_lowercase()))
        .filter(|w| !DROPPED.contains(&w.text))
        .map(|w| w.text)
        .collect()
}

pub fn completes_a_word(letters: &[char], gaps: &[usize], gap: usize, c: char) -> bool {
    let mut l = letters.to_vec();
    l[gaps[gap]] = c;
    let s: String = l.into_iter().collect();
    is_word(&s)
}

pub fn crosses_into_a_word(letters: &[char], gaps: &[usize]) -> bool {
    (0..gaps.len()).any(|k| {
        let other = letters[gaps[1 - k]];
        other != letters[gaps[k]] && completes_a_word(letters, gaps, k, other)
    })
}

pub fn gaps_for(level: usize, word: &str) -> Vec<usize> {
    let n = word.chars().count();
    match level {
        0 => vec![0],
        1 => vec![n - 1],
        2 => vec![1],
        _ => loop {
            let a = rand::gen_range(0, n);
            let mut b = rand::gen_range(0, n - 1);
            if b >= a {
                b += 1;
            }
            let mut g = vec![a, b];
            g.sort_unstable();
            let l: Vec<char> = word.chars().collect();
            if l[g[0]] != l[g[1]] && !crosses_into_a_word(&l, &g) {
                return g;
            }
        },
    }
}

fn memory_key(word: &str, field: &str) -> String {
    format!("w.{word}.{field}")
}

pub fn memory_of(word: &str) -> Memory {
    Memory {
        at: profile::get(GAME, &memory_key(word, "at")) as u32,
        due: profile::get(GAME, &memory_key(word, "due")) as u32,
        rung: profile::get(GAME, &memory_key(word, "rung")) as u32,
    }
}

fn remember(word: &str, m: Memory) {
    profile::set(GAME, &memory_key(word, "at"), m.at as i64);
    profile::set(GAME, &memory_key(word, "due"), m.due as i64);
    profile::set(GAME, &memory_key(word, "rung"), m.rung as i64);
}

pub fn round() -> u32 {
    profile::get(GAME, "round") as u32
}

pub fn pick_word(level: usize, recent: &[&'static str]) -> &'static str {
    let all = pool(level);
    let window = RECENT.min(all.len() / 2);
    let recent: Vec<&str> = recent[recent.len().saturating_sub(window)..].to_vec();
    spell::choose(&all, &memory_of, round(), &recent, 1)[0]
}

#[derive(Clone, Debug)]
pub struct Question {
    pub level: usize,
    pub word: &'static str,
    pub gaps: Vec<usize>,
    pub tiles: Vec<char>,
}

impl Question {
    pub fn new(level: usize, word: &'static str) -> Self {
        let gaps = gaps_for(level, word);
        Self::with_gaps(level, word, gaps)
    }

    pub fn with_gaps(level: usize, word: &'static str, gaps: Vec<usize>) -> Self {
        let letters: Vec<char> = word.chars().collect();
        let mut tiles: Vec<char> = gaps.iter().map(|g| letters[*g]).collect();
        let mut spare: Vec<char> = ('a'..='z')
            .filter(|c| !tiles.contains(c))
            .filter(|c| !(0..gaps.len()).any(|k| completes_a_word(&letters, &gaps, k, *c)))
            .collect();
        for _ in 0..decoys(level) {
            let i = rand::gen_range(0, spare.len());
            tiles.push(spare.swap_remove(i));
        }
        for _ in 0..8 {
            for i in (1..tiles.len()).rev() {
                tiles.swap(i, rand::gen_range(0, i + 1));
            }
            if tiles[..gaps.len()]
                .iter()
                .zip(gaps.iter().map(|g| letters[*g]))
                .any(|(a, b)| *a != b)
            {
                break;
            }
        }
        Self {
            level,
            word,
            gaps,
            tiles,
        }
    }

    pub fn letters(&self) -> Vec<char> {
        self.word.chars().collect()
    }

    pub fn answer_at(&self, gap_index: usize) -> char {
        self.letters()[self.gaps[gap_index]]
    }

    pub fn shown(&self, filled: usize) -> Vec<Option<char>> {
        let letters = self.letters();
        letters
            .iter()
            .enumerate()
            .map(|(i, c)| match self.gaps.iter().position(|g| *g == i) {
                Some(k) if k >= filled => None,
                _ => Some(*c),
            })
            .collect()
    }

    pub fn pattern(&self) -> String {
        self.shown(0)
            .iter()
            .map(|c| c.unwrap_or('_'))
            .collect::<String>()
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Mode {
    Play,
    Levels,
    Report(f32),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Report {
    Level { level: usize, stars: u8 },
    Cleared { rank: Option<usize> },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Drop {
    Right(Step),
    Next,
    Wrong,
    Ignored,
}

pub fn hud(view: &View) -> Hud {
    learn_hud(
        view,
        &HudSpec {
            title: HUD_TITLE,
            levels: LevelsAt::Goal,
            stars: LEVELS as u32,
            progress: 18.0,
            goal: HUD_GOAL,
        },
    )
}

pub fn levels_button(view: &View) -> Rect {
    hud(view).levels.unwrap_or_default()
}

pub struct Layout {
    pub picture: Rect,
    pub slots: Vec<Rect>,
    pub tiles: Vec<Rect>,
}

pub fn layout(view: &View, letters: usize, tiles: usize) -> Layout {
    if view.w > view.h * 1.6 {
        return wide_layout(view, letters, tiles);
    }
    let k = ui_scale(view);
    let top = hud(view).bottom;
    let gap = 14.0 * k;
    let max_tile = (if tall(view) { 170.0 * k } else { 120.0 * k })
        .min(view.h * 0.16)
        .max(view.touch());
    let fit = |n: usize| ((view.w - 60.0 - gap * (n as f32 - 1.0)) / n as f32).min(max_tile);
    let mut per_row = tiles;
    while per_row > 1 && fit(per_row) < view.touch() {
        per_row -= 1;
    }
    let per_row = tiles.div_ceil(tiles.div_ceil(per_row));
    let rows = tiles.div_ceil(per_row);
    let tile = fit(per_row).max(1.0);
    let room = view.h - top - view.edge() - 60.0 * k - 90.0;
    let tile =
        tile.min(((room - gap * (rows as f32 - 1.0)) / (rows as f32 + 0.8)).max(view.touch()));
    let slot = fit(letters).min(tile * 0.8).max(40.0);
    let tiles_h = rows as f32 * tile + (rows as f32 - 1.0) * gap;
    let tiles_top = view.h - view.edge() - tiles_h;
    let slots_top = tiles_top - 36.0 * k - slot;
    let band = (slots_top - 80.0 * k - top).max(80.0);
    let side = band.min(view.w - 80.0).max(80.0);
    let picture = Rect::new((view.w - side) / 2.0, top + (band - side) / 2.0, side, side);
    let row = |count: usize, size: f32, y: f32| -> Vec<Rect> {
        let w = count as f32 * size + (count as f32 - 1.0) * gap;
        let x0 = (view.w - w) / 2.0;
        (0..count)
            .map(|i| Rect::new(x0 + i as f32 * (size + gap), y, size, size))
            .collect()
    };
    let mut tile_rects = vec![];
    for r in 0..rows {
        let count = (tiles - r * per_row).min(per_row);
        tile_rects.extend(row(count, tile, tiles_top + r as f32 * (tile + gap)));
    }
    Layout {
        picture,
        slots: row(letters, slot, slots_top),
        tiles: tile_rects,
    }
}

fn wide_layout(view: &View, letters: usize, tiles: usize) -> Layout {
    let top = hud(view).bottom;
    let e = view.edge();
    let gap = view.gap();
    let bottom = view.h - e;
    let strip = tiles as f32 * view.touch() + gap * (tiles as f32 - 1.0);
    let side = (bottom - top)
        .min(view.w * 0.38)
        .min(view.w - 2.0 * e - 20.0 - gap * 2.0 - strip)
        .max(60.0);
    let picture = Rect::new(e + 20.0, top + (bottom - top - side) / 2.0, side, side);
    let left = picture.right() + gap * 2.0;
    let width = view.w - e - left;
    let fit = |n: usize| {
        ((width - gap * (n as f32 - 1.0)) / n as f32).min((view.h * 0.2).max(view.touch()))
    };
    let tile = fit(tiles).max(1.0);
    let slot = fit(letters).min(tile);
    let total = slot + gap + tile;
    let y0 = (top + 30.0).max(top + (bottom - top - total).max(0.0) / 2.0);
    let row = |count: usize, size: f32, y: f32| -> Vec<Rect> {
        let w = count as f32 * size + (count as f32 - 1.0) * gap;
        let x0 = left + (width - w) / 2.0;
        (0..count)
            .map(|i| Rect::new(x0 + i as f32 * (size + gap), y, size, size))
            .collect()
    };
    Layout {
        picture,
        slots: row(letters, slot, y0),
        tiles: row(tiles, tile, y0 + slot + gap),
    }
}

pub fn level_tiles(view: &View) -> Vec<Rect> {
    let n = LEVELS;
    let cols = if tall(view) { 2 } else { 3 };
    let rows = n.div_ceil(cols).max(SELECT_ROWS.min(n));
    let gap = 24.0;
    let top = 150.0;
    let bottom = view.h - 190.0;
    let w = ((view.w - 80.0 - gap * (cols as f32 - 1.0)) / cols as f32).min(300.0);
    let h = ((bottom - top - gap * (rows as f32 - 1.0)) / rows as f32).min(230.0);
    let in_last = n - (n.div_ceil(cols) - 1) * cols;
    (0..n)
        .map(|i| {
            let (r, c) = (i / cols, i % cols);
            let count = if r == n.div_ceil(cols) - 1 {
                in_last
            } else {
                cols
            };
            let left = (view.w - w * count as f32 - gap * (count as f32 - 1.0)) / 2.0;
            Rect::new(
                left + c as f32 * (w + gap),
                top + r as f32 * (h + gap),
                w,
                h,
            )
        })
        .collect()
}

pub fn play_button(view: &View) -> Rect {
    crate::sizing::bottom_button(view, "Play", 340.0, 100.0)
}

pub struct Build {
    pub mode: Mode,
    pub first_time: bool,
    pub report: Option<Report>,
    pub ladder: Ladder,
    pub question: Question,
    pub filled: usize,
    pub wrong: u32,
    pub used: Vec<bool>,
    pub held: Option<usize>,
    pub held_from: Vec2,
    pub held_pos: Vec2,
    pub wobble: Vec<f32>,
    pub pop: Vec<Bump>,
    pub celebrate: Option<f32>,
    pub picture_bump: Bump,
    pub streak_bump: Bump,
    pub shake: Shake,
    pub particles: Particles,
    pub popups: Popups,
    pub recent: Vec<&'static str>,
    pub t: f32,
    pub session: mastery::Session,
    pub summary: String,
}

impl Build {
    pub fn new() -> Self {
        let top = LEVELS - 1;
        let unlocked = (profile::unlocked(GAME) as usize).min(top);
        let mut level = (profile::level(GAME) as usize).min(unlocked);
        while level < unlocked && mastery::mastered(GAME, level) {
            level += 1;
        }
        let cleared = profile::get(GAME, "cleared") > 0;
        let mut ladder = Ladder::new(LEVELS);
        ladder.level = level;
        ladder.stars = if cleared { LEVELS } else { unlocked };
        let mut g = Self {
            mode: Mode::Play,
            first_time: profile::get(GAME, "played") == 0,
            report: None,
            ladder,
            question: Question::with_gaps(0, "cat", vec![0]),
            filled: 0,
            wrong: 0,
            used: vec![],
            held: None,
            held_from: Vec2::ZERO,
            held_pos: Vec2::ZERO,
            wobble: vec![],
            pop: vec![],
            celebrate: None,
            picture_bump: Bump::default(),
            streak_bump: Bump::default(),
            shake: Shake::default(),
            particles: Particles::default(),
            popups: Popups::default(),
            recent: vec![],
            t: 0.0,
            session: mastery::Session::default(),
            summary: String::new(),
        };
        g.next_question();
        g
    }

    pub fn unlocked_level(&self) -> usize {
        (profile::unlocked(GAME) as usize).min(LEVELS - 1)
    }

    pub fn start_level(&mut self, level: usize) {
        self.ladder.level = level.min(LEVELS - 1);
        self.ladder.streak = 0;
        self.ladder.slips = 0;
        self.session.reset();
        profile::set_level(GAME, self.ladder.level as u32);
        self.mode = Mode::Play;
        self.next_question();
    }

    pub fn next_question(&mut self) {
        let level = self.ladder.level;
        let word = pick_word(level, &self.recent);
        self.recent.push(word);
        self.load(Question::new(level, word));
    }

    pub fn load(&mut self, q: Question) {
        let n = q.tiles.len();
        self.question = q;
        self.filled = 0;
        self.wrong = 0;
        self.used = vec![false; n];
        self.held = None;
        self.wobble = vec![0.0; n];
        self.pop = (0..n).map(|_| Bump::default()).collect();
        self.celebrate = None;
        self.picture_bump.kick();
        self.session.start_question();
        self.say_word();
    }

    pub fn say_word(&self) {
        sfx::say(sfx::Say::Word(self.question.word));
    }

    pub fn glow(&self) -> Option<usize> {
        if self.celebrate.is_some() || self.wrong < 2 {
            return None;
        }
        let want = self.question.answer_at(self.filled);
        (0..self.used.len()).find(|i| !self.used[*i] && self.question.tiles[*i] == want)
    }

    pub fn finish_level(&mut self, level: usize) -> u8 {
        let stars = self.session.stars(target_secs(level));
        profile::award_stars(GAME, level as u32, stars);
        self.summary = self.session.summary();
        self.session.reset();
        stars
    }

    fn record(&mut self, step: Step) {
        self.first_time = false;
        profile::set(GAME, "played", 1);
        match step {
            Step::Up => {
                let level = self.ladder.level - 1;
                let stars = self.finish_level(level);
                profile::unlock(GAME, self.ladder.level as u32);
                profile::set_level(GAME, self.ladder.level as u32);
                self.report = Some(Report::Level { level, stars });
            }
            Step::Cleared => {
                let level = self.ladder.level;
                let stars = self.finish_level(level);
                if profile::get(GAME, "cleared") == 0 {
                    profile::set(GAME, "cleared", 1);
                    let total = profile::total_stars(GAME) as i64;
                    let rank = profile::submit(GAME, total, profile::Better::Higher);
                    self.report = Some(Report::Cleared { rank });
                } else {
                    self.report = Some(Report::Level { level, stars });
                }
            }
            Step::Down => {
                self.session.reset();
                profile::set_level(GAME, self.ladder.level as u32);
            }
            Step::Same => {}
        }
    }

    pub fn drop_tile(&mut self, view: &View, tile: usize) -> Drop {
        if self.celebrate.is_some() || tile >= self.used.len() || self.used[tile] {
            return Drop::Ignored;
        }
        let l = layout(view, self.question.word.len(), self.question.tiles.len());
        let slot = l.slots[self.question.gaps[self.filled]];
        let at = slot.center();
        if self.question.tiles[tile] != self.question.answer_at(self.filled) {
            self.wrong += 1;
            self.wobble[tile] = 1.0;
            sfx::play(Sfx::Wrong);
            self.shake.add(0.2);
            self.popups.add(
                at - vec2(0.0, slot.h),
                "Try again",
                Color::from_rgba(255, 170, 90, 255),
                46.0,
            );
            return Drop::Wrong;
        }
        self.used[tile] = true;
        self.filled += 1;
        self.pop[tile].kick();
        let gold = Color::from_rgba(255, 220, 70, 255);
        self.particles.sparkle(at, gold);
        if self.filled < self.question.gaps.len() {
            sfx::play(Sfx::Pop);
            sfx::play_step(Sfx::Tick, self.filled);
            return Drop::Next;
        }
        let first = self.wrong == 0;
        let step = if first {
            self.ladder.right()
        } else {
            self.ladder.slip()
        };
        self.on_right(view, first, step, at);
        Drop::Right(step)
    }

    fn on_right(&mut self, view: &View, first: bool, step: Step, at: Vec2) {
        let level = self.ladder.level;
        let answer = self.session.answer(first, false);
        mastery::record(GAME, level, answer, target_secs(level));
        let word = self.question.word;
        let r = round() + 1;
        profile::set(GAME, "round", r as i64);
        remember(word, memory_of(word).after(r, first));
        self.record(step);
        self.celebrate = Some(0.0);
        let gold = Color::from_rgba(255, 220, 70, 255);
        sfx::play(match step {
            Step::Up | Step::Cleared => Sfx::Win,
            _ => Sfx::Right,
        });
        if step == Step::Same {
            sfx::play_step(Sfx::Right, self.ladder.streak as usize * 2);
        }
        if matches!(step, Step::Up | Step::Cleared) {
            feel::impact_tinted(Impact::Large, Color::from_rgba(255, 230, 120, 255));
            self.particles.confetti(view);
        } else {
            feel::impact(Impact::Small);
        }
        self.particles.emit(
            at,
            &[gold, WHITE, hsv(0.08, 0.6, 1.0), hsv(0.9, 0.5, 1.0)],
            Burst {
                count: 26,
                speed: (200.0, 520.0),
                shape: Shape::Star,
                size: (6.0, 12.0),
                angle: -std::f32::consts::FRAC_PI_2,
                spread: 2.4,
                ..Default::default()
            },
        );
        self.streak_bump.kick();
        self.picture_bump.kick();
        self.say_word();
        let praise = if first {
            PRAISE[rand::gen_range(0, PRAISE.len())]
        } else {
            "That's it!"
        };
        let mid = vec2(view.w / 2.0, view.h * 0.4);
        self.popups.add(mid, praise, gold, 90.0);
        match step {
            Step::Up => self.popups.add(
                mid + vec2(0.0, 90.0),
                format!("Level up: {}", level_name(self.ladder.level)),
                WHITE,
                46.0,
            ),
            Step::Cleared => {
                self.popups
                    .add(mid + vec2(0.0, 90.0), "Word builder star!", WHITE, 50.0)
            }
            Step::Down => self.popups.add(
                mid + vec2(0.0, 90.0),
                format!("Let's practise: {}", level_name(self.ladder.level)),
                WHITE,
                42.0,
            ),
            Step::Same => {}
        }
    }

    pub fn goal_text(&self) -> String {
        if self.ladder.level + 1 < LEVELS {
            format!(
                "Get {UP_STREAK} right in a row to reach {}",
                level_name(self.ladder.level + 1)
            )
        } else {
            format!("Get {UP_STREAK} right in a row to finish")
        }
    }

    fn update_play(&mut self, input: &Input, view: &View, dt: f32) {
        if let Some(t) = self.celebrate {
            let t = t + dt;
            self.celebrate = Some(t);
            if t >= CELEBRATE {
                self.next_question();
                if self.report.is_some() {
                    self.mode = Mode::Report(0.0);
                }
            }
            return;
        }
        self.session.tick(dt);
        if input.pressed && levels_button(view).contains(input.pos) && !self.first_time {
            self.mode = Mode::Levels;
            return;
        }
        let l = layout(view, self.question.word.len(), self.question.tiles.len());
        if let Some(i) = self.held {
            self.held_pos = input.pos;
            if input.released || !input.down {
                self.held = None;
                let tile = l.tiles[i];
                let moved = (input.pos - self.held_from).length() > tile.w * 0.4;
                let slot = l.slots[self.question.gaps[self.filled]];
                let hit = Rect::new(
                    slot.x - slot.w * 0.3,
                    slot.y - slot.h * 0.3,
                    slot.w * 1.6,
                    slot.h * 1.6,
                )
                .contains(input.pos);
                if hit || !moved {
                    self.drop_tile(view, i);
                }
            }
            return;
        }
        if !input.pressed {
            return;
        }
        if l.picture.contains(input.pos) {
            self.picture_bump.kick();
            self.say_word();
            return;
        }
        if let Some(i) = l
            .tiles
            .iter()
            .enumerate()
            .position(|(i, r)| !self.used[i] && r.contains(input.pos))
        {
            self.held = Some(i);
            self.held_from = input.pos;
            self.held_pos = input.pos;
            sfx::play(Sfx::Tap);
        }
    }

    fn update_levels(&mut self, input: &Input, view: &View) {
        if !input.pressed {
            return;
        }
        if play_button(view).contains(input.pos) {
            self.mode = Mode::Play;
            return;
        }
        let unlocked = self.unlocked_level();
        let hit = level_tiles(view).iter().position(|r| r.contains(input.pos));
        if let Some(i) = hit.filter(|i| *i <= unlocked) {
            sfx::play(Sfx::Tick);
            self.start_level(i);
        }
    }

    fn update_report(&mut self, input: &Input, age: f32, dt: f32) {
        self.mode = Mode::Report(age + dt);
        if input.pressed && age > 0.8 {
            self.report = None;
            self.mode = Mode::Play;
            self.session.start_question();
        }
    }
}

impl Default for Build {
    fn default() -> Self {
        Self::new()
    }
}

impl Game for Build {
    fn update(&mut self, input: &Input, view: &View, dt: f32) {
        self.t += dt;
        self.streak_bump.update(dt);
        self.picture_bump.update(dt);
        for b in &mut self.pop {
            b.update(dt);
        }
        for w in &mut self.wobble {
            *w = (*w - dt * 1.8).max(0.0);
        }
        match self.mode {
            Mode::Levels => self.update_levels(input, view),
            Mode::Report(age) => self.update_report(input, age, dt),
            Mode::Play => self.update_play(input, view, dt),
        }
        self.shake.update(dt);
        self.particles.update(dt);
        self.popups.update(dt);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DT: f32 = 1.0 / 60.0;

    fn fresh() -> Build {
        crate::save::reset_for_test();
        profile::create("Mia", 0);
        rand::srand(11);
        Build::new()
    }

    fn views() -> [View; 2] {
        [View::ipad_landscape(), View::ipad_portrait()]
    }

    fn run(g: &mut Build, v: &View, secs: f32) {
        for _ in 0..(secs / DT) as usize {
            g.update(&Input::default(), v, DT);
        }
    }

    fn tile_for(g: &Build) -> usize {
        let want = g.question.answer_at(g.filled);
        (0..g.used.len())
            .find(|i| !g.used[*i] && g.question.tiles[*i] == want)
            .unwrap()
    }

    fn wrong_tile(g: &Build) -> usize {
        let want = g.question.answer_at(g.filled);
        (0..g.used.len())
            .find(|i| !g.used[*i] && g.question.tiles[*i] != want)
            .unwrap()
    }

    fn solve(g: &mut Build, v: &View, clean: bool) {
        if !clean {
            let w = wrong_tile(g);
            g.drop_tile(v, w);
        }
        while g.celebrate.is_none() {
            let t = tile_for(g);
            g.drop_tile(v, t);
        }
        run(g, v, CELEBRATE + 0.1);
        while let Mode::Report(_) = g.mode {
            g.update(&Input::press_at(Vec2::ZERO), v, 1.0);
            g.update(&Input::press_at(Vec2::ZERO), v, 1.0);
        }
    }

    #[test]
    fn exactly_one_tile_completes_a_real_word() {
        rand::srand(21);
        for l in 0..LEVELS {
            for w in pool(l) {
                for _ in 0..6 {
                    let q = Question::new(l, w);
                    let letters = q.letters();
                    for k in 0..q.gaps.len() {
                        let good: Vec<char> = q
                            .tiles
                            .iter()
                            .copied()
                            .filter(|c| completes_a_word(&letters, &q.gaps, k, *c))
                            .collect();
                        assert_eq!(good, vec![q.answer_at(k)], "{w} level {l} gap {k}");
                    }
                }
            }
        }
    }

    #[test]
    fn the_filter_keeps_thirty_words_a_level_and_drops_the_doubtful_ones() {
        for l in 0..LEVELS {
            assert!(pool(l).len() >= 30, "level {l}");
            for w in DROPPED {
                assert!(!pool(l).contains(w), "{w}");
            }
        }
    }

    #[test]
    fn every_level_has_a_name_example_and_a_big_pool() {
        for l in 0..LEVELS {
            assert!(!level_name(l).is_empty() && !level_example(l).is_empty());
            assert!(pool(l).len() >= 30, "level {l}: {}", pool(l).len());
        }
        let longest = (0..LEVELS)
            .map(|l| format!("Get {UP_STREAK} right in a row to reach {}", level_name(l)).len())
            .max()
            .unwrap();
        assert!(HUD_GOAL.len() >= longest);
    }

    #[test]
    fn the_level_plan_steps_up_as_written() {
        for w in pool(0) {
            assert_eq!(gaps_for(0, w), vec![0]);
            assert_eq!(gaps_for(1, w), vec![2]);
            assert_eq!(gaps_for(2, w), vec![1]);
        }
        for l in [3, 4] {
            for w in pool(l) {
                let g = gaps_for(l, w);
                assert_eq!(g.len(), 2);
                assert!(g[0] < g[1] && g[1] < w.len());
            }
        }
        assert!(pool(0).iter().all(|w| w.len() == 3));
        assert!(pool(4).iter().all(|w| w.len() >= 5));
        assert!(decoys(4) > decoys(0));
        assert_eq!(gap_count(2), 1);
        assert_eq!(gap_count(3), 2);
    }

    #[test]
    fn the_pattern_hides_only_the_gaps_and_tiles_hold_the_answers() {
        rand::srand(4);
        for l in 0..LEVELS {
            for w in pool(l).into_iter().take(40) {
                let q = Question::new(l, w);
                assert_eq!(q.pattern().matches('_').count(), gap_count(l));
                let want = gap_count(l) + decoys(l);
                assert_eq!(q.tiles.len(), want);
                let mut s = q.tiles.clone();
                s.sort_unstable();
                s.dedup();
                assert_eq!(s.len(), q.tiles.len(), "{w}");
                for k in 0..q.gaps.len() {
                    assert!(q.tiles.contains(&q.answer_at(k)));
                }
            }
        }
        let q = Question::with_gaps(2, "cat", vec![1]);
        assert_eq!(q.pattern(), "c_t");
        assert_eq!(q.shown(1), vec![Some('c'), Some('a'), Some('t')]);
    }

    #[test]
    fn words_do_not_repeat_inside_the_recent_window() {
        for level in 0..LEVELS {
            let mut g = fresh();
            g.start_level(level);
            let mut seen: Vec<&'static str> = vec![];
            for _ in 0..60 {
                let w = g.question.word;
                let window = &seen[seen.len().saturating_sub(RECENT.min(pool(level).len() / 2))..];
                assert!(!window.contains(&w), "level {level} repeated {w}");
                seen.push(w);
                solve(&mut g, &views()[0], true);
                g.start_level(level);
                g.recent = seen.clone();
            }
        }
    }

    #[test]
    fn a_missed_word_comes_back_after_its_gap_and_new_ones_come_first() {
        fresh();
        let p = pool(0)[0];
        remember(p, Memory::default().after(5, false));
        let due = memory_of(p).due;
        assert_eq!(due, 5 + spell::GAPS[0]);
        rand::srand(5);
        for _ in 0..20 {
            assert_ne!(pick_word(0, &[]), p);
        }
        profile::set(GAME, "round", due as i64);
        assert_eq!(pick_word(0, &[]), p);
    }

    #[test]
    fn a_wrong_letter_wobbles_and_two_misses_glow_the_right_one() {
        let v = views()[0];
        let mut g = fresh();
        g.load(Question::with_gaps(0, "cat", vec![0]));
        let w = wrong_tile(&g);
        assert_eq!(g.drop_tile(&v, w), Drop::Wrong);
        assert!(g.wobble[w] > 0.0);
        assert_eq!(g.glow(), None);
        let w2 = (0..g.used.len())
            .find(|i| *i != w && g.question.tiles[*i] != 'c')
            .unwrap();
        g.drop_tile(&v, w2);
        assert_eq!(g.glow(), Some(tile_for(&g)));
    }

    #[test]
    fn two_gaps_fill_in_order_and_the_last_one_finishes() {
        let v = views()[0];
        let mut g = fresh();
        g.load(Question::with_gaps(3, "lamp", vec![0, 2]));
        let t = tile_for(&g);
        assert_eq!(g.drop_tile(&v, t), Drop::Next);
        assert_eq!(g.filled, 1);
        let t = tile_for(&g);
        assert_eq!(g.drop_tile(&v, t), Drop::Right(Step::Same));
        assert!(g.celebrate.is_some());
    }

    #[test]
    fn dragging_a_tile_onto_the_gap_or_tapping_it_places_the_letter() {
        for v in views() {
            let mut g = fresh();
            g.first_time = false;
            g.load(Question::with_gaps(0, "cat", vec![0]));
            let l = layout(&v, 3, g.question.tiles.len());
            let t = tile_for(&g);
            g.update(&Input::press_at(l.tiles[t].center()), &v, DT);
            assert_eq!(g.held, Some(t));
            let mut up = Input::press_at(l.slots[0].center());
            up.pressed = false;
            up.down = false;
            up.released = true;
            g.update(&up, &v, DT);
            assert!(g.celebrate.is_some());

            let mut g = fresh();
            g.first_time = false;
            g.load(Question::with_gaps(0, "cat", vec![0]));
            let t = tile_for(&g);
            g.update(&Input::press_at(l.tiles[t].center()), &v, DT);
            let mut up = Input::press_at(l.tiles[t].center());
            up.pressed = false;
            up.down = false;
            up.released = true;
            g.update(&up, &v, DT);
            assert!(g.celebrate.is_some());
        }
    }

    #[test]
    fn a_strong_player_climbs_every_level_and_reaches_the_board() {
        let v = views()[0];
        let mut g = fresh();
        for _ in 0..(UP_STREAK as usize * LEVELS + 2) {
            solve(&mut g, &v, true);
        }
        assert_eq!(profile::get(GAME, "cleared"), 1);
        assert!(profile::personal_best(GAME).is_some());
        for l in 0..LEVELS {
            assert!(profile::stars(GAME, l as u32) >= 1, "level {l}");
        }
        assert_eq!(profile::unlocked(GAME) as usize, LEVELS - 1);
    }

    #[test]
    fn a_weak_player_steps_back_but_keeps_playing() {
        let v = views()[0];
        let mut g = fresh();
        profile::unlock(GAME, 3);
        g.start_level(3);
        for _ in 0..3 {
            solve(&mut g, &v, false);
        }
        assert_eq!(g.ladder.level, 2);
        for _ in 0..30 {
            solve(&mut g, &v, false);
        }
        assert!(g.ladder.level <= 2);
        assert_eq!(profile::get(GAME, "cleared"), 0);
    }

    #[test]
    fn the_first_minute_is_spoken_and_easy() {
        let g = fresh();
        assert!(g.first_time);
        assert_eq!(g.ladder.level, 0);
        assert_eq!(g.question.gaps, vec![0]);
        assert_eq!(g.question.tiles.len(), 3);
        assert_eq!(sfx::take_voice(), Some(sfx::Say::Word(g.question.word)));
    }

    #[test]
    fn level_progress_is_saved_per_player() {
        let v = views()[0];
        let mut g = fresh();
        for _ in 0..UP_STREAK {
            solve(&mut g, &v, true);
        }
        let mia = profile::current_id().unwrap();
        assert_eq!(profile::unlocked(GAME), 1);
        let leo = profile::create("Leo", 1).unwrap();
        profile::select(&leo.id);
        assert_eq!(profile::unlocked(GAME), 0);
        profile::select(&mia);
        assert_eq!(profile::unlocked(GAME), 1);
        assert_eq!(Build::new().ladder.level, 1);
    }

    #[test]
    fn layouts_keep_slots_tiles_and_picture_apart_on_every_view() {
        for (name, v) in crate::sizing::test_views() {
            for letters in 3..=6 {
                for tiles in 3..=6 {
                    let l = layout(&v, letters, tiles);
                    let h = hud(&v);
                    assert!(l.picture.y >= h.bottom - 0.5, "{name} picture under hud");
                    if v.w > v.h * 1.6 {
                        assert!(l.picture.right() <= l.slots[0].x + 0.5, "{name}");
                        assert!(
                            l.tiles[0].bottom() <= v.h - v.edge() + 0.5,
                            "{name} {tiles}"
                        );
                    } else {
                        assert!(
                            l.picture.bottom() <= l.slots[0].y + 0.5,
                            "{name} {letters} {tiles}"
                        );
                    }
                    assert!(l.slots[0].bottom() <= l.tiles[0].y + 0.5, "{name}");
                    for r in l.slots.iter().chain(&l.tiles) {
                        assert!(r.x >= -0.5 && r.right() <= v.w + 0.5, "{name} {letters}");
                        assert!(r.bottom() <= v.h + 0.5, "{name}");
                    }
                    assert!(l.tiles[0].w >= v.touch() - 0.5, "{name} tile too small");
                }
            }
        }
    }
}
