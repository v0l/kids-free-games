use crate::engine::*;
use crate::feel::{self, Impact};
use crate::fx::*;
use crate::profile::{self, Better};
use crate::sfx::{self, Sfx};
use macroquad::prelude::*;

pub const KEY: &str = "memory";
pub const LEVELS: [usize; 5] = [3, 4, 6, 8, 10];
pub const MAX_LEVEL: u32 = LEVELS.len() as u32;
pub const MAX_PAIRS: usize = 10;
pub const PEEK: f32 = 0.8;
const PEEKS: [f32; 5] = [1.6, 1.3, 1.1, 0.95, PEEK];
const CARD_TALL: f32 = 1.45;
const MAX_TALL: f32 = 300.0;
const TOP: f32 = 84.0;
const BOTTOM: f32 = 76.0;
const DONE_DELAY: f32 = 0.9;
const DONE_TAPPABLE: f32 = 1.5;
const ANIMALS: [&str; MAX_PAIRS] = [
    "giraffe", "hippo", "monkey", "panda", "parrot", "penguin", "pig", "elephant", "lion", "koala",
];

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Face {
    Down,
    Up,
    Matched,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Event {
    Match(usize, usize),
    Miss(usize, usize),
    Win,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Screen {
    Select,
    Play,
}

pub fn pairs_for(level: u32) -> usize {
    LEVELS[(level.clamp(1, MAX_LEVEL) - 1) as usize]
}

pub fn peek_for(level: u32) -> f32 {
    PEEKS[(level.clamp(1, MAX_LEVEL) - 1) as usize]
}

pub fn three_star_moves(pairs: usize) -> u32 {
    (pairs as f32 * 1.6).ceil() as u32
}

pub fn two_star_moves(pairs: usize) -> u32 {
    (pairs as f32 * 2.6).ceil() as u32
}

pub fn stars_for(pairs: usize, moves: u32) -> u8 {
    if moves <= three_star_moves(pairs) {
        3
    } else if moves <= two_star_moves(pairs) {
        2
    } else {
        1
    }
}

pub fn struggle_threshold(level: u32) -> u32 {
    if level <= 2 {
        2
    } else {
        3
    }
}

pub fn flip_shape(progress: f32) -> (f32, f32) {
    let k = progress.clamp(0.0, 1.0);
    let eased = k * k * (3.0 - 2.0 * k);
    let a = eased * std::f32::consts::PI;
    (a.cos().abs(), a.sin())
}

pub fn wobble_offset(wobble: f32) -> f32 {
    (wobble * 36.0).sin() * 12.0 * wobble.clamp(0.0, 1.0)
}

pub fn moves_label(moves: i64) -> String {
    format!("{moves} moves")
}

fn best_key(level: u32) -> String {
    format!("best.{level}")
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Progress {
    pub unlocked: u32,
    pub stars: [u8; LEVELS.len()],
    pub resume: u32,
}

impl Progress {
    pub fn load() -> Self {
        let unlocked = profile::unlocked(KEY).clamp(1, MAX_LEVEL);
        let mut stars = [0; LEVELS.len()];
        for (i, s) in stars.iter_mut().enumerate() {
            *s = profile::stars(KEY, i as u32 + 1);
        }
        Self {
            unlocked,
            stars,
            resume: profile::level(KEY).clamp(1, unlocked),
        }
    }

    pub fn total(&self) -> u32 {
        self.stars.iter().map(|s| *s as u32).sum()
    }

    pub fn is_new_player(&self) -> bool {
        self.unlocked <= 1 && self.total() == 0
    }
}

pub struct Memory {
    pub screen: Screen,
    pub level: u32,
    pub pairs: usize,
    pub cards: Vec<(u8, Face)>,
    pub moves: u32,
    pub best: Option<u32>,
    pub wait: f32,
    pub peek: f32,
    pub events: Vec<Event>,
    pub miss_streak: u32,
    pub stars: u8,
    pub rank: Option<usize>,
    pub new_best: bool,
    pub teach: bool,
    pub progress: Progress,
    flip: Vec<f32>,
    wobble: Vec<f32>,
    pop: Vec<Bump>,
    moves_bump: Bump,
    stars_bump: Bump,
    shown_stars: u8,
    particles: Particles,
    popups: Popups,
    shake: Shake,
    celebrated: f32,
    t: f32,
}

impl Memory {
    pub fn new() -> Self {
        let progress = Progress::load();
        if progress.is_new_player() {
            Self::for_level(1)
        } else {
            let mut m = Self::for_level(progress.resume);
            m.screen = Screen::Select;
            m
        }
    }

    pub fn for_level(level: u32) -> Self {
        let level = level.clamp(1, MAX_LEVEL);
        let pairs = pairs_for(level);
        let mut ids: Vec<u8> = (0..MAX_PAIRS as u8).collect();
        for i in (1..ids.len()).rev() {
            ids.swap(i, rand::gen_range(0, i + 1));
        }
        let mut cards: Vec<(u8, Face)> = ids[..pairs]
            .iter()
            .flat_map(|i| [(*i, Face::Down), (*i, Face::Down)])
            .collect();
        for i in (1..cards.len()).rev() {
            cards.swap(i, rand::gen_range(0, i + 1));
        }
        let n = cards.len();
        let best = Some(profile::get(KEY, &best_key(level)))
            .filter(|b| *b > 0)
            .map(|b| b as u32);
        Self {
            screen: Screen::Play,
            level,
            pairs,
            cards,
            moves: 0,
            best,
            wait: 0.0,
            peek: peek_for(level),
            events: vec![],
            miss_streak: 0,
            stars: 0,
            rank: None,
            new_best: false,
            teach: level == 1 && profile::get(KEY, "taught") == 0,
            progress: Progress::load(),
            flip: vec![0.0; n],
            wobble: vec![0.0; n],
            pop: (0..n).map(|_| Bump::default()).collect(),
            moves_bump: Bump::default(),
            stars_bump: Bump::default(),
            shown_stars: 3,
            particles: Particles::default(),
            popups: Popups::default(),
            shake: Shake::default(),
            celebrated: -1.0,
            t: 0.0,
        }
    }

    pub fn begin(&mut self, level: u32) {
        let level = level.clamp(1, MAX_LEVEL);
        profile::set_level(KEY, level);
        *self = Self::for_level(level);
    }

    pub fn open_select(&mut self) {
        let level = self.level;
        *self = Self::for_level(level);
        self.screen = Screen::Select;
    }

    fn up(&self) -> Vec<usize> {
        (0..self.cards.len())
            .filter(|i| self.cards[*i].1 == Face::Up)
            .collect()
    }

    pub fn won(&self) -> bool {
        self.screen == Screen::Play && self.cards.iter().all(|c| c.1 == Face::Matched)
    }

    pub fn matched_pairs(&self) -> usize {
        self.cards.iter().filter(|c| c.1 == Face::Matched).count() / 2
    }

    pub fn helper(&self) -> Option<usize> {
        if self.miss_streak < struggle_threshold(self.level) {
            return None;
        }
        let up = self.up();
        if up.len() != 1 || self.wait > 0.0 {
            return None;
        }
        let a = up[0];
        (0..self.cards.len()).find(|i| {
            *i != a && self.cards[*i].1 == Face::Down && self.cards[*i].0 == self.cards[a].0
        })
    }

    pub fn pick(&mut self, i: usize) {
        if self.screen != Screen::Play || self.wait > 0.0 || self.cards[i].1 != Face::Down {
            return;
        }
        self.cards[i].1 = Face::Up;
        sfx::play(Sfx::Card);
        let up = self.up();
        if up.len() == 2 {
            self.moves += 1;
            if self.cards[up[0]].0 == self.cards[up[1]].0 {
                for j in &up {
                    self.cards[*j].1 = Face::Matched;
                }
                self.miss_streak = 0;
                if self.teach {
                    self.teach = false;
                    profile::set(KEY, "taught", 1);
                }
                self.events.push(Event::Match(up[0], up[1]));
                if self.won() {
                    self.complete();
                    self.events.push(Event::Win);
                }
            } else {
                self.miss_streak += 1;
                self.wait = self.peek;
                self.events.push(Event::Miss(up[0], up[1]));
            }
        }
    }

    fn complete(&mut self) {
        self.stars = stars_for(self.pairs, self.moves);
        profile::award_stars(KEY, self.level, self.stars);
        self.new_best = self.best.is_none_or(|b| self.moves < b);
        if self.new_best {
            self.best = Some(self.moves);
            profile::set(KEY, &best_key(self.level), self.moves as i64);
        }
        profile::set(KEY, "taught", 1);
        if self.level < MAX_LEVEL {
            profile::unlock(KEY, self.level + 1);
            profile::set_level(KEY, self.level + 1);
        } else {
            self.rank = profile::submit(KEY, self.moves as i64, Better::Lower);
        }
        self.progress = Progress::load();
    }

    pub fn tick(&mut self, dt: f32) {
        if self.wait > 0.0 {
            self.wait -= dt;
            if self.wait <= 0.0 {
                self.wait = 0.0;
                for c in &mut self.cards {
                    if c.1 == Face::Up {
                        c.1 = Face::Down;
                    }
                }
            }
        }
    }

    fn card_size(view: &View, cols: usize, rows: usize) -> (f32, f32) {
        let avail_h = view.h - TOP - BOTTOM;
        let wide = (view.w - 40.0) / cols as f32;
        let tall = (avail_h / rows as f32).min(wide * CARD_TALL).min(MAX_TALL);
        ((tall / CARD_TALL * 1.08).min(wide), tall)
    }

    pub fn layout(view: &View, pairs: usize) -> Vec<Rect> {
        let n = pairs * 2;
        let mut cols = 2;
        let mut best_tall = 0.0;
        for c in (2..=10).filter(|c| n.is_multiple_of(*c)) {
            let tall = Self::card_size(view, c, n / c).1;
            if tall > best_tall + 0.5 {
                best_tall = tall;
                cols = c;
            }
        }
        let rows = n / cols;
        let (cw, ch) = Self::card_size(view, cols, rows);
        let ox = (view.w - cw * cols as f32) / 2.0;
        let oy = TOP + (view.h - TOP - BOTTOM - ch * rows as f32) / 2.0;
        (0..n)
            .map(|i| {
                Rect::new(
                    ox + (i % cols) as f32 * cw + 6.0,
                    oy + (i / cols) as f32 * ch + 6.0,
                    cw - 12.0,
                    ch - 12.0,
                )
            })
            .collect()
    }

    pub fn levels_button(view: &View) -> Rect {
        Rect::new(view.w - 14.0 - 140.0, 12.0, 140.0, 48.0)
    }

    pub fn tiles(view: &View) -> Vec<Rect> {
        let wide = view.w >= view.h;
        let cols = if wide { 5 } else { 3 };
        let gap = 16.0;
        let w = ((view.w - 60.0 - gap * (cols as f32 - 1.0)) / cols as f32).min(200.0);
        let h = w * 1.3;
        let rows = LEVELS.len().div_ceil(cols);
        let top = view.h / 2.0 - (rows as f32 * h + (rows as f32 - 1.0) * gap) / 2.0 + 20.0;
        (0..LEVELS.len())
            .map(|i| {
                let row = i / cols;
                let in_row = (LEVELS.len() - row * cols).min(cols);
                let left = (view.w - (in_row as f32 * w + (in_row as f32 - 1.0) * gap)) / 2.0;
                Rect::new(
                    left + (i % cols) as f32 * (w + gap),
                    top + row as f32 * (h + gap),
                    w,
                    h,
                )
            })
            .collect()
    }

    pub fn play_button(view: &View) -> Rect {
        let bottom = Self::tiles(view)
            .iter()
            .map(|r| r.y + r.h)
            .fold(0.0, f32::max);
        Rect::new(view.w / 2.0 - 190.0, bottom + 40.0, 380.0, 90.0)
    }

    fn done_panel(view: &View) -> Rect {
        let w = 620.0f32.min(view.w - 30.0);
        let h = 430.0;
        Rect::new((view.w - w) / 2.0, view.h / 2.0 - h / 2.0 + 20.0, w, h)
    }

    pub fn done_buttons(view: &View) -> [Rect; 3] {
        let p = Self::done_panel(view);
        let gap = 14.0;
        let bw = (p.w - 60.0 - gap * 2.0) / 3.0;
        let y = p.y + p.h - 104.0;
        [0, 1, 2].map(|k| Rect::new(p.x + 30.0 + k as f32 * (bw + gap), y, bw, 76.0))
    }
}

impl Default for Memory {
    fn default() -> Self {
        Self::new()
    }
}

mod art {
    crate::sprites!("memory");
}

impl Memory {
    fn react(&mut self, view: &View) {
        let rects = Self::layout(view, self.pairs);
        for e in std::mem::take(&mut self.events) {
            match e {
                Event::Match(a, b) => {
                    let matched = self.matched_pairs();
                    sfx::play_step(Sfx::Right, matched.saturating_sub(1));
                    for i in [a, b] {
                        let c = rects[i].center();
                        let col = hsv(self.cards[i].0 as f32 / MAX_PAIRS as f32, 0.6, 1.0);
                        self.particles.sparkle(c, col);
                        self.pop[i].kick();
                    }
                    let mid = (rects[a].center() + rects[b].center()) / 2.0;
                    self.popups
                        .add(mid, "Match!", Color::from_rgba(255, 220, 70, 255), 44.0);
                }
                Event::Miss(a, b) => {
                    sfx::play(Sfx::Wrong);
                    self.wobble[a] = 1.0;
                    self.wobble[b] = 1.0;
                    self.shake.add(0.3);
                }
                Event::Win => {
                    let last = self.level >= MAX_LEVEL;
                    sfx::play(if last { Sfx::Win } else { Sfx::LevelUp });
                    sfx::say(if last {
                        sfx::Say::Congrats
                    } else if self.stars == 3 {
                        sfx::Say::Great
                    } else {
                        sfx::Say::LevelUp
                    });
                    feel::impact_tinted(Impact::Large, Color::from_rgba(255, 230, 120, 255));
                    self.celebrated = 0.0
                }
            }
        }
    }

    fn update_select(&mut self, input: &Input, view: &View, dt: f32) {
        self.t += dt;
        self.popups.update(dt);
        if !input.pressed {
            return;
        }
        if Self::play_button(view).contains(input.pos) {
            self.begin(self.progress.resume);
            return;
        }
        if let Some(i) = Self::tiles(view).iter().position(|r| r.contains(input.pos)) {
            let level = i as u32 + 1;
            if level <= self.progress.unlocked {
                self.begin(level);
            } else {
                sfx::play(Sfx::Wrong);
                self.popups.add(
                    Self::tiles(view)[i].center(),
                    format!("Finish level {} first", level - 1),
                    Color::from_rgba(255, 220, 70, 255),
                    30.0,
                );
            }
        }
    }

    fn update_done(&mut self, input: &Input, view: &View) {
        if self.celebrated < DONE_TAPPABLE || !input.tap() {
            return;
        }
        if Self::levels_button(view).contains(input.pos) {
            self.open_select();
        } else if self.level >= MAX_LEVEL {
            self.begin(self.level);
        } else {
            let [levels, again, next] = Self::done_buttons(view);
            if levels.contains(input.pos) {
                self.open_select();
            } else if again.contains(input.pos) {
                self.begin(self.level);
            } else if next.contains(input.pos) || input.action {
                self.begin(self.level + 1);
            }
        }
    }
}

impl Game for Memory {
    fn update(&mut self, input: &Input, view: &View, dt: f32) {
        if self.screen == Screen::Select {
            self.update_select(input, view, dt);
            return;
        }
        self.t += dt;
        let moves = self.moves;
        self.tick(dt);
        let won = self.won();
        for (i, f) in self.flip.iter_mut().enumerate() {
            let target = if self.cards[i].1 == Face::Down {
                0.0
            } else {
                1.0
            };
            *f += (target - *f).clamp(-dt * 4.5, dt * 4.5);
            if *f > 0.95 && self.wobble[i] > 0.0 {
                self.wobble[i] = (self.wobble[i] - dt * 2.2).max(0.0);
            }
            self.pop[i].update(dt);
        }
        self.moves_bump.update(dt);
        self.stars_bump.update(dt);
        if input.pressed && Self::levels_button(view).contains(input.pos) && !won {
            self.open_select();
            return;
        }
        if won {
            self.update_done(input, view);
            if self.screen == Screen::Select {
                return;
            }
        } else if input.pressed {
            if let Some(i) = Self::layout(view, self.pairs)
                .iter()
                .position(|r| r.contains(input.pos))
            {
                self.pick(i);
            }
        }
        if self.moves != moves {
            self.moves_bump.kick();
            let now = stars_for(self.pairs, self.moves);
            if now != self.shown_stars {
                self.shown_stars = now;
                self.stars_bump.kick();
            }
        }
        self.react(view);
        if self.celebrated >= 0.0 {
            let before = self.celebrated;
            self.celebrated += dt;
            for at in [0.0, 1.2, 2.4] {
                if before <= at && self.celebrated > at {
                    self.particles.confetti(view);
                }
            }
        }
        self.particles.update(dt);
        self.popups.update(dt);
        self.shake.update(dt);
    }

    fn draw(&self, view: &View) {
        let f = view.full();
        let side = f.w.max(f.h);
        art::sprite(
            "bg",
            Rect::new(f.x + (f.w - side) / 2.0, f.y + f.h - side, side, side),
        );
        if self.screen == Screen::Select {
            self.draw_select(view);
            return;
        }
        self.draw_hud(view);
        let sh = self.shake.offset();
        let helper = self.helper();
        let teach_pulse = self.teach && self.moves == 0 && self.up().is_empty();
        for (i, r) in Self::layout(view, self.pairs).iter().enumerate() {
            let pulse = if teach_pulse {
                1.0 + 0.04 * (self.t * 5.0 + i as f32 * 0.5).sin()
            } else {
                1.0
            };
            self.draw_card(i, *r, sh, pulse);
            if helper == Some(i) {
                let k = 1.0 + 0.2 * (self.t * 6.0).sin();
                crate::ui::spark(
                    vec2(r.center().x, r.y - 6.0),
                    r.w * 0.45 * k,
                    self.t * 1.5,
                    Color::from_rgba(255, 220, 70, 255),
                );
            }
        }
        self.particles.draw();
        self.popups.draw();
        self.draw_hint(view);
        if self.won() && self.celebrated >= DONE_DELAY {
            self.draw_done(view, self.celebrated - DONE_DELAY);
        }
    }
}

fn star_row(c: Vec2, size: f32, lit: f32, scale: f32) {
    for i in 0..3 {
        let x = c.x + (i as f32 - 1.0) * size * 1.15;
        let on = (i as f32) < lit;
        let tint = if on {
            WHITE
        } else {
            Color::new(0.25, 0.2, 0.4, 0.45)
        };
        crate::ui::sprite_w(
            "p_star",
            vec2(x, c.y),
            size * if on { scale } else { 1.0 },
            crate::art::Draw {
                tint,
                ..Default::default()
            },
        );
    }
}

impl Memory {
    fn draw_hud(&self, view: &View) {
        chip(
            view,
            Side::Left,
            "MOVES",
            &self.moves.to_string(),
            self.moves_bump.t,
        );
        crate::ui::painted_button(Self::levels_button(view), "Levels", "blue", 0.0, 1.0);
        let lit = stars_for(self.pairs, self.moves);
        star_row(
            vec2(view.w / 2.0, 34.0),
            34.0,
            lit as f32,
            self.stars_bump.scale(0.3),
        );
        text_outline(
            &format!("{} of {} pairs", self.matched_pairs(), self.pairs),
            view.w / 2.0,
            68.0,
            24.0,
            WHITE,
            crate::ui::INK,
        );
    }

    fn draw_hint(&self, view: &View) {
        if !self.teach || self.won() {
            return;
        }
        let label = if self.wait > 0.0 {
            "Not a match. Try again!"
        } else if self.up().len() == 1 {
            "Now find its twin!"
        } else {
            "Tap a card to flip it"
        };
        let pulse = 1.0 + 0.04 * (self.t * 4.0).sin();
        crate::ui::hint(view, label, vec2(view.w / 2.0, view.h - 40.0), pulse);
    }

    fn draw_select(&self, view: &View) {
        crate::ui::ribbon(
            view,
            "Memory Match",
            vec2(view.w / 2.0, Self::tiles(view)[0].y - 70.0),
            self.t,
        );
        chip(
            view,
            Side::Right,
            "STARS",
            &format!("{}/{}", self.progress.total(), LEVELS.len() * 3),
            1.0,
        );
        for (i, r) in Self::tiles(view).iter().enumerate() {
            self.draw_tile(i, *r);
        }
        let play = Self::play_button(view);
        let pulse = 1.0 + (self.t * 4.5).sin() * 0.03;
        let grown = Rect::new(
            play.center().x - play.w * pulse / 2.0,
            play.center().y - play.h * pulse / 2.0,
            play.w * pulse,
            play.h * pulse,
        );
        crate::ui::painted_button(
            grown,
            &format!("Play level {}", self.progress.resume),
            "green",
            0.0,
            1.0,
        );
        self.popups.draw();
    }

    fn draw_tile(&self, i: usize, r: Rect) {
        let level = i as u32 + 1;
        let open = level <= self.progress.unlocked;
        let current = level == self.progress.resume;
        let bob = if current {
            (self.t * 4.0).sin() * 3.0
        } else {
            0.0
        };
        let r = Rect::new(r.x, r.y + bob, r.w, r.h);
        let tint = if open {
            WHITE
        } else {
            Color::new(0.7, 0.68, 0.8, 1.0)
        };
        crate::ui::painted_panel(r, tint);
        let animal = ANIMALS[i % MAX_PAIRS];
        let d = r.w * 0.62;
        art::sprite_w(
            animal,
            vec2(r.center().x, r.y + r.h * 0.28),
            d,
            crate::art::Draw {
                tint: if open {
                    WHITE
                } else {
                    Color::new(0.25, 0.2, 0.35, 0.7)
                },
                ..Default::default()
            },
        );
        text_outline(
            &level.to_string(),
            r.center().x,
            r.y + r.h * 0.55,
            r.w * 0.3,
            Color::from_rgba(255, 200, 40, 255),
            crate::ui::INK,
        );
        text_center(
            &format!("{} pairs", pairs_for(level)),
            r.center().x,
            r.y + r.h * 0.69,
            (r.w * 0.125).max(16.0),
            crate::ui::BODY,
        );
        if open {
            star_row(
                vec2(r.center().x, r.y + r.h * 0.86),
                r.w * 0.24,
                self.progress.stars[i] as f32,
                1.0,
            );
        } else {
            let s = r.w * 0.26;
            crate::ui::sprite_ex(
                "icon_lock",
                Rect::new(r.center().x - s / 2.0, r.y + r.h * 0.86 - s / 2.0, s, s),
                crate::art::Draw {
                    tint: Color::from_rgba(120, 100, 150, 255),
                    ..Default::default()
                },
            );
        }
    }

    fn draw_done(&self, view: &View, age: f32) {
        if self.level >= MAX_LEVEL {
            let line = format!(
                "{} pairs, {} moves, {} stars",
                self.pairs, self.moves, self.stars
            );
            results(
                view,
                &crate::ui::Results {
                    game: KEY,
                    title: "All done!",
                    line: &line,
                    rank: self.rank,
                    action: "Play again",
                    format: moves_label,
                },
            );
            return;
        }
        fill_screen(
            view,
            Color::new(0.06, 0.04, 0.16, 0.55 * ease_out_cubic(age / 0.2)),
        );
        let p = Self::done_panel(view);
        let s = ease_out_back(age / 0.32).max(0.01);
        let c = p.center() + vec2(0.0, (1.0 - ease_out_cubic(age / 0.32)) * 90.0);
        let sp = Rect::new(c.x - p.w * s / 2.0, c.y - p.h * s / 2.0, p.w * s, p.h * s);
        crate::ui::painted_panel(sp, WHITE);
        crate::ui::ribbon(
            view,
            &format!("Level {} clear!", self.level),
            vec2(c.x, sp.y + 8.0 * s),
            age,
        );
        let size = 100.0 * s;
        for i in 0..3 {
            let x = c.x + (i as f32 - 1.0) * size * 1.15;
            let y = c.y - p.h * 0.5 * s + 170.0 * s;
            let earned = i < self.stars as usize;
            let k = ease_out_back(((age - 0.3 - i as f32 * 0.25) / 0.3).clamp(0.0, 1.0));
            crate::ui::sprite_w(
                "p_star",
                vec2(x, y),
                size * if earned { k } else { 1.0 },
                crate::art::Draw {
                    tint: if earned {
                        WHITE
                    } else {
                        Color::new(0.25, 0.2, 0.4, 0.4)
                    },
                    rotation: if earned { (1.0 - k) * 0.8 } else { 0.0 },
                    ..Default::default()
                },
            );
        }
        let a = |d: f32| ((age - d) / 0.2).clamp(0.0, 1.0);
        let moves = format!("Done in {} moves", self.moves);
        text_center(
            &moves,
            c.x,
            c.y - p.h * 0.5 * s + 262.0 * s,
            34.0 * s,
            with_alpha(crate::ui::BODY, a(0.5)),
        );
        let sub = if self.new_best {
            "New best!".to_string()
        } else {
            let best = self.best.unwrap_or(self.moves);
            format!("Best: {best} moves")
        };
        text_center(
            &sub,
            c.x,
            c.y - p.h * 0.5 * s + 304.0 * s,
            26.0 * s,
            with_alpha(
                if self.new_best {
                    Color::from_rgba(230, 140, 0, 255)
                } else {
                    crate::ui::BODY
                },
                a(0.6),
            ),
        );
        let [levels, again, next] = Self::done_buttons(view);
        let pulse = 1.0 + (age * 4.5).sin() * 0.03;
        let grow = |r: Rect, k: f32| {
            Rect::new(
                r.center().x - r.w * k * s / 2.0,
                r.center().y - r.h * k * s / 2.0,
                r.w * k * s,
                r.h * k * s,
            )
        };
        let shift = c - p.center();
        let place = |r: Rect, k: f32| {
            let g = grow(r, k);
            Rect::new(g.x + shift.x, g.y + shift.y, g.w, g.h)
        };
        crate::ui::painted_button(place(levels, 1.0), "Levels", "blue", 0.0, a(0.7));
        crate::ui::painted_button(place(again, 1.0), "Again", "orange", 0.0, a(0.75));
        crate::ui::painted_button(place(next, pulse), "Next", "green", 0.0, a(0.8));
    }
}

fn fit_card(r: Rect, aspect: f32) -> Rect {
    let w = r.w.min(r.h * aspect);
    Rect::new(r.center().x - w / 2.0, r.y, w, r.h)
}

impl Memory {
    fn draw_card(&self, i: usize, r: Rect, shake: Vec2, pulse: f32) {
        let r = fit_card(r, art::size("back").x / art::size("back").y);
        let (w, lift) = flip_shape(self.flip[i]);
        let matched = self.cards[i].1 == Face::Matched;
        let pop = self.pop[i].scale(0.18);
        let pop = if matched { pop } else { pulse };
        let c = r.center() + shake + vec2(wobble_offset(self.wobble[i]), -lift * 14.0);
        let hs = 1.0 + 0.1 * lift;
        let rr = Rect::new(
            c.x - r.w * w * pop / 2.0,
            c.y - r.h * hs * pop / 2.0,
            r.w * w * pop,
            r.h * hs * pop,
        );
        soft_shadow(
            vec2(r.center().x + shake.x, r.y + r.h - 2.0 + shake.y),
            r.w * 0.5 * (0.4 + 0.6 * w),
            r.h * 0.04,
            0.6 - lift * 0.3,
        );
        if self.flip[i] < 0.5 {
            art::sprite("back", rr);
            return;
        }
        let tint = if matched {
            Color::from_rgba(214, 255, 214, 255)
        } else {
            WHITE
        };
        art::sprite_ex(
            "face",
            rr,
            crate::art::Draw {
                tint,
                ..Default::default()
            },
        );
        if w > 0.2 {
            let bounce = if matched {
                (self.t * 4.0 + i as f32).sin() * 3.0
            } else {
                0.0
            };
            let d = r.w * 0.82;
            art::sprite_w(
                ANIMALS[self.cards[i].0 as usize % MAX_PAIRS],
                vec2(rr.x + rr.w / 2.0, rr.y + rr.h / 2.0 + bounce),
                d * w,
                Default::default(),
            );
            if matched {
                shine_sweep(rr, self.t * 1.5 + i as f32 * 0.2);
            }
        }
    }
}

pub fn icon(r: Rect) {
    crate::ui::icon("memory", r);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fresh() -> Memory {
        crate::save::reset_for_test();
        crate::profile::create("Mia", 0);
        Memory::new()
    }

    fn ids(m: &Memory) -> Vec<u8> {
        let mut v: Vec<u8> = m.cards.iter().map(|c| c.0).collect();
        v.sort();
        v.dedup();
        v
    }

    fn pair_positions(m: &Memory, id: u8) -> (usize, usize) {
        let v: Vec<usize> = (0..m.cards.len()).filter(|i| m.cards[*i].0 == id).collect();
        (v[0], v[1])
    }

    fn clear_perfectly(m: &mut Memory) {
        for id in ids(m) {
            let (a, b) = pair_positions(m, id);
            m.pick(a);
            m.pick(b);
        }
    }

    fn wasteful_clear(m: &mut Memory, extra_misses: u32) {
        let all = ids(m);
        for k in 0..extra_misses as usize {
            let (a, _) = pair_positions(m, all[0]);
            let (b, _) = pair_positions(m, all[1 + k % (all.len() - 1)]);
            m.pick(a);
            m.pick(b);
            m.tick(m.peek + 0.01);
        }
        clear_perfectly(m);
    }

    fn tick_view() -> View {
        View::ipad_landscape()
    }

    fn tap(m: &mut Memory, p: Vec2) {
        m.update(&Input::press_at(p), &tick_view(), 1.0 / 60.0);
    }

    fn settle(m: &mut Memory, seconds: f32) {
        let n = (seconds * 60.0) as usize;
        for _ in 0..n {
            m.update(&Input::default(), &tick_view(), 1.0 / 60.0);
        }
    }

    #[test]
    fn memory_sprites_all_exist() {
        let have = art::names();
        let mut want = vec!["bg", "back", "face"];
        want.extend(ANIMALS);
        for n in want {
            assert!(have.iter().any(|h| h == n), "missing {n}");
        }
    }

    #[test]
    fn flipping_a_card_plays_the_card_sound() {
        let mut m = fresh();
        crate::sfx::take_pending();
        m.pick(0);
        assert!(crate::sfx::take_pending().contains(&Sfx::Card));
    }

    #[test]
    fn deck_has_two_of_each() {
        for level in 1..=MAX_LEVEL {
            let m = Memory::for_level(level);
            assert_eq!(m.cards.len(), pairs_for(level) * 2);
            for id in ids(&m) {
                assert_eq!(m.cards.iter().filter(|c| c.0 == id).count(), 2);
            }
        }
    }

    #[test]
    fn matching_pair_stays_up() {
        let mut m = fresh();
        let (a, b) = pair_positions(&m, ids(&m)[0]);
        m.pick(a);
        m.pick(b);
        assert_eq!(m.cards[a].1, Face::Matched);
        assert_eq!(m.moves, 1);
    }

    #[test]
    fn mismatch_flips_back_and_blocks_third_pick() {
        let mut m = fresh();
        let all = ids(&m);
        let (a, _) = pair_positions(&m, all[0]);
        let (b, c) = pair_positions(&m, all[1]);
        m.pick(a);
        m.pick(b);
        m.pick(c);
        assert_eq!(m.cards[c].1, Face::Down);
        m.tick(m.peek + 0.01);
        assert_eq!(m.cards[a].1, Face::Down);
        assert_eq!(m.cards[b].1, Face::Down);
    }

    #[test]
    fn clearing_all_pairs_wins_and_records_best() {
        let mut m = fresh();
        clear_perfectly(&mut m);
        assert!(m.won());
        assert_eq!(m.best, Some(3));
        assert_eq!(profile::get(KEY, &best_key(1)), 3);
    }

    #[test]
    fn flip_is_edge_on_halfway_and_lifts() {
        assert_eq!(flip_shape(0.0).0, 1.0);
        assert!((flip_shape(1.0).0 - 1.0).abs() < 1e-4);
        let (w, lift) = flip_shape(0.5);
        assert!(w < 1e-4 && (lift - 1.0).abs() < 1e-4);
        assert!(flip_shape(0.25).0 > flip_shape(0.4).0);
    }

    #[test]
    fn wobble_dies_out() {
        assert_eq!(wobble_offset(0.0), 0.0);
        assert!(wobble_offset(0.03).abs() < 1.0);
        assert!((0..100).any(|k| wobble_offset(1.0 - k as f32 * 0.005).abs() > 5.0));
    }

    #[test]
    fn picks_record_match_miss_and_win_events() {
        let mut m = fresh();
        let all = ids(&m);
        let (a, b) = pair_positions(&m, all[0]);
        let (c, _) = pair_positions(&m, all[1]);
        m.pick(a);
        m.pick(c);
        assert_eq!(m.events, vec![Event::Miss(a.min(c), a.max(c))]);
        m.tick(m.peek + 0.01);
        m.events.clear();
        m.pick(a);
        m.pick(b);
        assert_eq!(m.events, vec![Event::Match(a.min(b), a.max(b))]);
        m.events.clear();
        for id in &all[1..] {
            let (x, y) = pair_positions(&m, *id);
            m.pick(x);
            m.pick(y);
        }
        assert_eq!(m.events.last(), Some(&Event::Win));
    }

    #[test]
    fn mismatch_shakes_and_match_sparkles() {
        let v = View::ipad_landscape();
        let mut m = fresh();
        let all = ids(&m);
        let (a, b) = pair_positions(&m, all[0]);
        let (c, _) = pair_positions(&m, all[1]);
        m.pick(a);
        m.pick(c);
        m.update(&Input::default(), &v, 1.0 / 60.0);
        assert!(m.shake.active());
        assert_eq!(m.wobble[a], 1.0);
        m.tick(m.peek + 0.01);
        m.pick(a);
        m.pick(b);
        m.update(&Input::default(), &v, 1.0 / 60.0);
        assert!(!m.particles.list.is_empty());
        assert_eq!(m.popups.list.len(), 1);
    }

    #[test]
    fn winning_rains_confetti_once_per_wave() {
        let v = View::ipad_landscape();
        let mut m = fresh();
        clear_perfectly(&mut m);
        m.update(&Input::default(), &v, 1.0 / 60.0);
        assert!(m.particles.list.iter().any(|p| p.shape == Shape::Confetti));
        assert!(m.celebrated >= 0.0);
    }

    #[test]
    fn every_card_fits_on_screen_at_every_level() {
        for v in [
            View::ipad_landscape(),
            View::ipad_portrait(),
            View::fit(1920.0, 1080.0),
            View::fit(1080.0, 1920.0),
        ] {
            for level in 1..=MAX_LEVEL {
                let pairs = pairs_for(level);
                let rects = Memory::layout(&v, pairs);
                assert_eq!(rects.len(), pairs * 2);
                for r in &rects {
                    assert!(
                        r.x >= 0.0
                            && r.y >= TOP - 1.0
                            && r.x + r.w <= v.w
                            && r.y + r.h <= v.h - BOTTOM + 12.0,
                        "level {level} {r:?} outside {v:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn cards_are_big_enough_to_tap_at_the_hardest_level() {
        for v in [View::ipad_landscape(), View::ipad_portrait()] {
            let r = Memory::layout(&v, pairs_for(MAX_LEVEL))[0];
            assert!(r.w >= 70.0 && r.h >= 100.0, "{r:?}");
        }
    }

    #[test]
    fn levels_grow_the_deck_and_shorten_the_peek() {
        let pairs: Vec<usize> = (1..=MAX_LEVEL).map(pairs_for).collect();
        assert_eq!(pairs, vec![3, 4, 6, 8, 10]);
        for l in 1..MAX_LEVEL {
            assert!(pairs_for(l + 1) > pairs_for(l));
            assert!(peek_for(l + 1) <= peek_for(l));
        }
        assert_eq!(peek_for(MAX_LEVEL), PEEK);
    }

    #[test]
    fn a_new_player_starts_on_three_pairs_with_a_hint() {
        let m = fresh();
        assert_eq!(m.screen, Screen::Play);
        assert_eq!(m.level, 1);
        assert_eq!(m.cards.len(), 6);
        assert!(m.teach);
    }

    #[test]
    fn the_first_level_cannot_be_lost() {
        let mut m = fresh();
        let all = ids(&m);
        for _ in 0..6 {
            let (a, _) = pair_positions(&m, all[0]);
            let (b, _) = pair_positions(&m, all[1]);
            m.pick(a);
            m.pick(b);
            m.tick(m.peek + 0.01);
        }
        assert!(!m.won());
        assert!(m.cards.iter().all(|c| c.1 == Face::Down));
        clear_perfectly(&mut m);
        assert!(m.won());
        assert!(m.stars >= 1);
    }

    #[test]
    fn the_hint_goes_away_after_the_first_match() {
        let mut m = fresh();
        assert!(m.teach);
        let (a, b) = pair_positions(&m, ids(&m)[0]);
        m.pick(a);
        m.pick(b);
        assert!(!m.teach);
        assert!(!Memory::for_level(1).teach);
    }

    #[test]
    fn stars_reward_fewer_moves() {
        assert_eq!(stars_for(3, 3), 3);
        assert_eq!(stars_for(3, three_star_moves(3)), 3);
        assert_eq!(stars_for(3, three_star_moves(3) + 1), 2);
        assert_eq!(stars_for(3, two_star_moves(3)), 2);
        assert_eq!(stars_for(3, two_star_moves(3) + 1), 1);
        for level in 1..=MAX_LEVEL {
            let p = pairs_for(level);
            assert_eq!(stars_for(p, p as u32), 3);
            assert!(two_star_moves(p) > three_star_moves(p));
            assert_eq!(stars_for(p, 500), 1);
        }
    }

    #[test]
    fn finishing_a_level_awards_stars_and_unlocks_the_next() {
        let mut m = fresh();
        clear_perfectly(&mut m);
        assert_eq!(m.stars, 3);
        assert_eq!(profile::stars(KEY, 1), 3);
        assert_eq!(profile::unlocked(KEY), 2);
        assert_eq!(profile::level(KEY), 2);
    }

    #[test]
    fn a_sloppy_run_keeps_the_best_stars_ever_earned() {
        let mut m = fresh();
        clear_perfectly(&mut m);
        let mut again = Memory::for_level(1);
        wasteful_clear(&mut again, 8);
        assert_eq!(again.stars, 1);
        assert_eq!(profile::stars(KEY, 1), 3);
    }

    #[test]
    fn best_moves_are_kept_per_level() {
        let mut m = fresh();
        wasteful_clear(&mut m, 2);
        assert_eq!(m.best, Some(5));
        let mut m2 = Memory::for_level(1);
        assert_eq!(m2.best, Some(5));
        clear_perfectly(&mut m2);
        assert!(m2.new_best);
        assert_eq!(Memory::for_level(1).best, Some(3));
        assert_eq!(Memory::for_level(2).best, None);
    }

    #[test]
    fn progress_resumes_per_player() {
        crate::save::reset_for_test();
        crate::profile::create("Mia", 0);
        let mut m = Memory::new();
        clear_perfectly(&mut m);
        let mut m = Memory::for_level(2);
        clear_perfectly(&mut m);
        let resumed = Memory::new();
        assert_eq!(resumed.screen, Screen::Select);
        assert_eq!(resumed.progress.unlocked, 3);
        assert_eq!(resumed.progress.resume, 3);
        assert_eq!(resumed.progress.total(), 6);
        crate::profile::create("Leo", 1);
        let other = Memory::new();
        assert_eq!(other.screen, Screen::Play);
        assert_eq!(other.level, 1);
        assert!(other.teach);
        assert_eq!(other.progress.unlocked, 1);
    }

    #[test]
    fn the_last_level_submits_moves_to_the_leaderboard() {
        let mut m = fresh();
        m.begin(MAX_LEVEL);
        clear_perfectly(&mut m);
        assert_eq!(m.rank, Some(0));
        assert_eq!(profile::board(KEY)[0].score, 10);
        assert_eq!(profile::personal_best(KEY), Some(10));
        let mut worse = Memory::for_level(MAX_LEVEL);
        wasteful_clear(&mut worse, 3);
        assert_eq!(worse.rank, Some(1));
        assert_eq!(profile::board(KEY)[0].score, 10);
    }

    #[test]
    fn earlier_levels_do_not_touch_the_leaderboard() {
        let mut m = fresh();
        clear_perfectly(&mut m);
        assert!(profile::board(KEY).is_empty());
        assert_eq!(m.rank, None);
    }

    #[test]
    fn repeated_misses_reveal_the_twin_of_the_flipped_card() {
        let mut m = fresh();
        let all = ids(&m);
        let (a, a2) = pair_positions(&m, all[0]);
        let (b, _) = pair_positions(&m, all[1]);
        for _ in 0..struggle_threshold(1) {
            m.pick(a);
            m.pick(b);
            m.tick(m.peek + 0.01);
        }
        assert_eq!(m.helper(), None);
        m.pick(a);
        assert_eq!(m.helper(), Some(a2));
    }

    #[test]
    fn a_match_cancels_the_helper() {
        let mut m = fresh();
        let all = ids(&m);
        let (a, a2) = pair_positions(&m, all[0]);
        let (b, _) = pair_positions(&m, all[1]);
        for _ in 0..struggle_threshold(1) {
            m.pick(a);
            m.pick(b);
            m.tick(m.peek + 0.01);
        }
        m.pick(a);
        m.pick(a2);
        let (c, _) = pair_positions(&m, all[1]);
        m.pick(c);
        assert_eq!(m.helper(), None);
    }

    #[test]
    fn early_levels_are_more_forgiving_than_late_ones() {
        assert!(struggle_threshold(1) <= struggle_threshold(MAX_LEVEL));
        assert!(peek_for(1) > peek_for(MAX_LEVEL));
    }

    #[test]
    fn tapping_a_locked_level_does_not_start_it() {
        let v = tick_view();
        let mut m = fresh();
        clear_perfectly(&mut m);
        m.open_select();
        let tiles = Memory::tiles(&v);
        tap(&mut m, tiles[4].center());
        assert_eq!(m.screen, Screen::Select);
        tap(&mut m, tiles[1].center());
        assert_eq!(m.screen, Screen::Play);
        assert_eq!(m.level, 2);
        assert_eq!(m.cards.len(), 8);
    }

    #[test]
    fn replaying_an_earlier_level_from_the_select_screen_works() {
        let v = tick_view();
        let mut m = fresh();
        clear_perfectly(&mut m);
        let mut m2 = Memory::for_level(2);
        clear_perfectly(&mut m2);
        m2.open_select();
        tap(&mut m2, Memory::tiles(&v)[0].center());
        assert_eq!(m2.level, 1);
        assert_eq!(m2.screen, Screen::Play);
        assert_eq!(m2.moves, 0);
    }

    #[test]
    fn the_play_button_continues_at_the_unlocked_level() {
        let v = tick_view();
        let mut m = fresh();
        clear_perfectly(&mut m);
        let mut m = Memory::for_level(2);
        clear_perfectly(&mut m);
        let mut s = Memory::new();
        assert_eq!(s.screen, Screen::Select);
        tap(&mut s, Memory::play_button(&v).center());
        assert_eq!(s.screen, Screen::Play);
        assert_eq!(s.level, 3);
    }

    #[test]
    fn the_level_clear_screen_offers_next_again_and_levels() {
        let v = tick_view();
        let mut m = fresh();
        clear_perfectly(&mut m);
        settle(&mut m, DONE_TAPPABLE + 0.1);
        let [_, _, next] = Memory::done_buttons(&v);
        tap(&mut m, next.center());
        assert_eq!(m.level, 2);
        assert_eq!(m.screen, Screen::Play);

        let mut m = Memory::for_level(1);
        clear_perfectly(&mut m);
        settle(&mut m, DONE_TAPPABLE + 0.1);
        let [levels, _, _] = Memory::done_buttons(&v);
        tap(&mut m, levels.center());
        assert_eq!(m.screen, Screen::Select);

        let mut m = Memory::for_level(1);
        clear_perfectly(&mut m);
        settle(&mut m, DONE_TAPPABLE + 0.1);
        let [_, again, _] = Memory::done_buttons(&v);
        tap(&mut m, again.center());
        assert_eq!(m.level, 1);
        assert_eq!(m.moves, 0);
        assert!(!m.won());
    }

    #[test]
    fn a_stray_tap_during_the_celebration_does_not_skip_it() {
        let v = tick_view();
        let mut m = fresh();
        clear_perfectly(&mut m);
        settle(&mut m, 0.3);
        let [_, _, next] = Memory::done_buttons(&v);
        tap(&mut m, next.center());
        assert!(m.won());
        assert_eq!(m.level, 1);
    }

    #[test]
    fn the_levels_button_leaves_a_game_in_progress() {
        let v = tick_view();
        let mut m = fresh();
        tap(&mut m, Memory::levels_button(&v).center());
        assert_eq!(m.screen, Screen::Select);
    }

    #[test]
    fn winning_celebrates_with_the_level_up_sound() {
        let mut m = fresh();
        crate::sfx::take_pending();
        clear_perfectly(&mut m);
        m.update(&Input::default(), &tick_view(), 1.0 / 60.0);
        assert!(crate::sfx::take_pending().contains(&Sfx::LevelUp));
        assert_eq!(crate::sfx::take_voice(), Some(sfx::Say::Great));
    }

    #[test]
    fn tapping_cards_through_the_layout_plays_a_whole_level() {
        let v = tick_view();
        let mut m = fresh();
        let rects = Memory::layout(&v, m.pairs);
        for id in ids(&m) {
            let (a, b) = pair_positions(&m, id);
            tap(&mut m, rects[a].center());
            tap(&mut m, rects[b].center());
        }
        assert!(m.won());
    }
}
