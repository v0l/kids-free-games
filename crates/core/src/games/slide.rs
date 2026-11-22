use crate::engine::*;
use crate::feel::{self, Impact};
use crate::fx::*;
use crate::geom::*;
use crate::sfx::{self, Say, Sfx};
use glam::{vec2, Vec2};
use std::collections::HashMap;
use std::sync::OnceLock;

pub const GAME: &str = "slide";
pub const MAX_LEVEL: u32 = 12;
pub const PICTURES: [&str; 6] = ["lion", "panda", "penguin", "whale", "rocket", "frog"];
pub const WIN_DELAY: f32 = 1.4;
pub const STAR_START: f32 = 0.3;
pub const STAR_GAP: f32 = 0.25;
pub const PEEK_SECS: f32 = 1.6;
pub const HUD_TOP: f32 = 112.0;
pub const PAD: f32 = 12.0;
pub const WIN_LABELS: [&str; 2] = ["Next level", "Levels"];
pub const WIN_SKIP_LABELS: [&str; 3] = ["Next level", "Skip ahead", "Levels"];
const SLIDE_RATE: f32 = 20.0;
pub const LIMIT_SLACK_PCT: usize = 30;
pub const LIMIT_SLACK_FLAT: usize = 3;
const AWAY_PCT: usize = 100;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Spec {
    pub n: usize,
    pub depth: usize,
    pub min_par: usize,
}

const SPECS: [Spec; MAX_LEVEL as usize] = [
    Spec { n: 2, depth: 3, min_par: 2 },
    Spec { n: 2, depth: 7, min_par: 5 },
    Spec { n: 3, depth: 8, min_par: 6 },
    Spec { n: 3, depth: 18, min_par: 12 },
    Spec { n: 3, depth: 30, min_par: 18 },
    Spec { n: 3, depth: 70, min_par: 22 },
    Spec { n: 4, depth: 12, min_par: 0 },
    Spec { n: 4, depth: 30, min_par: 0 },
    Spec { n: 4, depth: 60, min_par: 0 },
    Spec { n: 5, depth: 16, min_par: 0 },
    Spec { n: 5, depth: 40, min_par: 0 },
    Spec { n: 5, depth: 80, min_par: 0 },
];

pub fn spec(level: u32) -> Spec {
    SPECS[level.clamp(1, MAX_LEVEL) as usize - 1]
}

fn solved_tiles(n: usize) -> Vec<u8> {
    (0..n * n).map(|i| ((i + 1) % (n * n)) as u8).collect()
}

pub fn is_solved(tiles: &[u8]) -> bool {
    let last = tiles.len() - 1;
    tiles[last] == 0 && tiles[..last].iter().enumerate().all(|(i, v)| *v as usize == i + 1)
}

fn side_of(len: usize) -> usize {
    (len as f32).sqrt().round() as usize
}

pub fn neighbours(n: usize, i: usize) -> Vec<usize> {
    let (c, r) = (i % n, i / n);
    let mut out = vec![];
    if r > 0 {
        out.push(i - n);
    }
    if c > 0 {
        out.push(i - 1);
    }
    if c + 1 < n {
        out.push(i + 1);
    }
    if r + 1 < n {
        out.push(i + n);
    }
    out
}

fn key(t: &[u8]) -> u64 {
    t.iter().fold(0u64, |a, v| (a << 4) | *v as u64)
}

fn unkey(mut k: u64, len: usize) -> Vec<u8> {
    let mut v = vec![0u8; len];
    for i in (0..len).rev() {
        v[i] = (k & 15) as u8;
        k >>= 4;
    }
    v
}

fn build_table(n: usize) -> HashMap<u64, u8> {
    let len = n * n;
    let start = solved_tiles(n);
    let mut seen = HashMap::new();
    seen.insert(key(&start), 0u8);
    let mut frontier = vec![key(&start)];
    let mut d = 0u8;
    while !frontier.is_empty() {
        d += 1;
        let mut next = vec![];
        for k in frontier {
            let t = unkey(k, len);
            let b = t.iter().position(|v| *v == 0).unwrap();
            for nb in neighbours(n, b) {
                let mut u = t.clone();
                u.swap(b, nb);
                let uk = key(&u);
                if let std::collections::hash_map::Entry::Vacant(e) = seen.entry(uk) {
                    e.insert(d);
                    next.push(uk);
                }
            }
        }
        frontier = next;
    }
    seen
}

fn table(n: usize) -> &'static HashMap<u64, u8> {
    static T: [OnceLock<HashMap<u64, u8>>; 2] = [OnceLock::new(), OnceLock::new()];
    T[n - 2].get_or_init(|| build_table(n))
}

pub fn distance(tiles: &[u8]) -> Option<usize> {
    let n = side_of(tiles.len());
    if !(2..=3).contains(&n) {
        return None;
    }
    table(n).get(&key(tiles)).map(|d| *d as usize)
}

pub fn best_move(tiles: &[u8]) -> Option<usize> {
    let n = side_of(tiles.len());
    let d = distance(tiles)?;
    if d == 0 {
        return None;
    }
    let b = tiles.iter().position(|v| *v == 0)?;
    neighbours(n, b).into_iter().find(|nb| {
        let mut u = tiles.to_vec();
        u.swap(b, *nb);
        distance(&u) == Some(d - 1)
    })
}

struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (self.0 >> 33) ^ (self.0 >> 13)
    }

    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
}

pub struct Puzzle {
    pub n: usize,
    pub tiles: Vec<u8>,
    pub trail: Vec<usize>,
    pub par: usize,
    pub limit: usize,
}

pub fn manhattan(tiles: &[u8]) -> usize {
    let n = side_of(tiles.len());
    tiles
        .iter()
        .enumerate()
        .filter(|(_, v)| **v != 0)
        .map(|(i, v)| {
            let home = *v as usize - 1;
            (i % n).abs_diff(home % n) + (i / n).abs_diff(home / n)
        })
        .sum()
}

pub fn star_limit(par: usize, lower_bound: usize) -> usize {
    par + lower_bound * LIMIT_SLACK_PCT / 100 + LIMIT_SLACK_FLAT
}

fn pick_next(rng: &mut Lcg, tiles: &[u8], blank: usize, options: &[usize], n: usize) -> usize {
    if n >= 4 && rng.below(100) < AWAY_PCT {
        let before = manhattan(tiles);
        let away: Vec<usize> = options
            .iter()
            .copied()
            .filter(|o| {
                let mut u = tiles.to_vec();
                u.swap(blank, *o);
                manhattan(&u) > before
            })
            .collect();
        if !away.is_empty() {
            return away[rng.below(away.len())];
        }
    }
    options[rng.below(options.len())]
}

pub fn scramble(n: usize, depth: usize, seed: u64) -> Puzzle {
    let mut rng = Lcg(seed);
    let mut tiles = solved_tiles(n);
    let mut blank = n * n - 1;
    let mut trail = vec![blank];
    for _ in 0..depth {
        let back = if trail.len() >= 2 {
            Some(trail[trail.len() - 2])
        } else {
            None
        };
        let options: Vec<usize> = neighbours(n, blank)
            .into_iter()
            .filter(|o| Some(*o) != back)
            .collect();
        let next = pick_next(&mut rng, &tiles, blank, &options, n);
        tiles.swap(blank, next);
        blank = next;
        trail.push(blank);
    }
    let lower = manhattan(&tiles);
    let par = distance(&tiles).unwrap_or(lower);
    Puzzle {
        n,
        tiles,
        trail,
        par,
        limit: star_limit(par, lower),
    }
}

pub fn make(level: u32, variant: u32) -> Puzzle {
    let level = level.clamp(1, MAX_LEVEL);
    let s = spec(level);
    let mut best: Option<Puzzle> = None;
    for attempt in 0..300u64 {
        let seed = level as u64 * 7919 + variant as u64 * 104729 + attempt * 31 + 17;
        let p = scramble(s.n, s.depth, seed);
        if is_solved(&p.tiles) || p.par < s.min_par.max(1) {
            continue;
        }
        if p.trail.len() - 1 <= p.limit {
            return p;
        }
        let over = |q: &Puzzle| q.trail.len() - 1 - q.limit.min(q.trail.len() - 1);
        if best.as_ref().is_none_or(|b| over(&p) < over(b)) {
            best = Some(p);
        }
    }
    best.unwrap_or_else(|| scramble(s.n, s.depth.max(2), 99))
}

pub fn stars_for(moves: usize, limit: usize) -> u8 {
    if moves <= limit {
        3
    } else if moves <= limit * 2 {
        2
    } else {
        1
    }
}

pub fn score_for(level: u32, stars: u8) -> u32 {
    level * 60 + stars as u32 * 40
}

pub fn reached() -> u32 {
    crate::profile::unlocked(GAME).clamp(1, MAX_LEVEL)
}

pub fn start_level() -> u32 {
    match crate::profile::level(GAME) {
        0 => 1,
        n => n.clamp(1, reached()),
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum State {
    Select,
    Playing,
    Won,
}

pub struct Slide {
    pub state: State,
    pub level: u32,
    pub variant: u32,
    pub n: usize,
    pub picture: usize,
    pub par: usize,
    pub limit: usize,
    pub tiles: Vec<u8>,
    pub start: Vec<u8>,
    pub start_trail: Vec<usize>,
    pub trail: Vec<usize>,
    pub vis: Vec<Vec2>,
    pub moves: usize,
    pub restarts: u32,
    pub stars: u8,
    pub score: u32,
    pub best: Best,
    pub rank: Option<usize>,
    pub hint: Option<usize>,
    pub idle: f32,
    pub taught: bool,
    pub pending_win: bool,
    pub peek: f32,
    pub t: f32,
    pub end_t: f32,
    pub fx: Particles,
    pub shake: Shake,
    pub pops: Popups,
    pub bump: Bump,
}

impl Default for Slide {
    fn default() -> Self {
        Self::new()
    }
}

impl Slide {
    pub fn new() -> Self {
        let mut g = Self::blank();
        g.best = Best::load(GAME);
        g.enter(start_level());
        g
    }

    pub fn with_level(level: u32) -> Self {
        let mut g = Self::blank();
        g.enter(level);
        g
    }

    fn blank() -> Self {
        Self {
            state: State::Playing,
            level: 1,
            variant: 0,
            n: 2,
            picture: 0,
            par: 0,
            limit: 0,
            tiles: vec![],
            start: vec![],
            start_trail: vec![],
            trail: vec![],
            vis: vec![],
            moves: 0,
            restarts: 0,
            stars: 0,
            score: 0,
            best: Best::default(),
            rank: None,
            hint: None,
            idle: 0.0,
            taught: crate::profile::get(GAME, "taught") == 1,
            pending_win: false,
            peek: 0.0,
            t: 0.0,
            end_t: 0.0,
            fx: Particles::default(),
            shake: Shake::default(),
            pops: Popups::default(),
            bump: Bump::default(),
        }
    }

    fn seen_key(level: u32) -> String {
        format!("seen.{level}")
    }

    pub fn enter(&mut self, level: u32) {
        let level = level.clamp(1, MAX_LEVEL);
        crate::profile::set_level(GAME, level);
        self.level = level;
        self.variant = crate::profile::get(GAME, &Self::seen_key(level)).max(0) as u32;
        let p = make(level, self.variant);
        self.n = p.n;
        self.par = p.par;
        self.limit = p.limit;
        self.picture = (level as usize - 1 + self.variant as usize) % PICTURES.len();
        self.start = p.tiles;
        self.start_trail = p.trail;
        self.restarts = 0;
        self.reset_board();
        self.state = State::Playing;
    }

    fn reset_board(&mut self) {
        self.tiles = self.start.clone();
        self.trail = self.start_trail.clone();
        self.vis = vec![Vec2::ZERO; self.n * self.n];
        for (i, v) in self.tiles.iter().enumerate() {
            self.vis[*v as usize] = self.cell_pos(i);
        }
        self.moves = 0;
        self.stars = 0;
        self.hint = None;
        self.idle = 0.0;
        self.pending_win = false;
        self.peek = 0.0;
        self.end_t = 0.0;
    }

    pub fn cell_pos(&self, i: usize) -> Vec2 {
        vec2((i % self.n) as f32, (i / self.n) as f32)
    }

    pub fn restart(&mut self) {
        self.restarts += 1;
        self.reset_board();
        sfx::play(Sfx::Whoosh);
    }

    pub fn open_select(&mut self) {
        self.state = State::Select;
        self.end_t = 0.0;
    }

    pub fn blank_at(&self) -> usize {
        self.tiles.iter().position(|v| *v == 0).unwrap_or(0)
    }

    pub fn solved(&self) -> bool {
        is_solved(&self.tiles)
    }

    pub fn stars_now(&self) -> u8 {
        stars_for(self.moves, self.limit)
    }

    pub fn can_skip(&self) -> bool {
        self.stars == 3 && self.level + 2 <= MAX_LEVEL
    }

    pub fn at_home(&self, i: usize) -> bool {
        let v = self.tiles[i] as usize;
        v != 0 && v == i + 1
    }

    pub fn slide_to(&mut self, cell: usize) -> bool {
        if self.pending_win || cell >= self.tiles.len() {
            return false;
        }
        let b = self.blank_at();
        if cell == b {
            return false;
        }
        let (bc, br, cc, cr) = (b % self.n, b / self.n, cell % self.n, cell / self.n);
        let step: isize = if br == cr {
            if cc > bc {
                1
            } else {
                -1
            }
        } else if bc == cc {
            if cr > br {
                self.n as isize
            } else {
                -(self.n as isize)
            }
        } else {
            return false;
        };
        let mut blank = b;
        while blank != cell {
            let next = (blank as isize + step) as usize;
            self.tiles.swap(blank, next);
            blank = next;
            self.moves += 1;
            let l = self.trail.len();
            if l >= 2 && self.trail[l - 2] == blank {
                self.trail.pop();
            } else {
                self.trail.push(blank);
            }
        }
        self.hint = None;
        self.idle = 0.0;
        sfx::play(Sfx::Swish);
        self.bump.kick();
        if self.solved() {
            self.pending_win = true;
        }
        true
    }

    pub fn compute_hint(&mut self) {
        self.hint = if self.n <= 3 {
            best_move(&self.tiles)
        } else if self.trail.len() >= 2 {
            Some(self.trail[self.trail.len() - 2])
        } else {
            None
        };
    }

    pub fn settled(&self) -> bool {
        self.tiles
            .iter()
            .enumerate()
            .all(|(i, v)| (self.vis[*v as usize] - self.cell_pos(i)).length() < 0.02)
    }

    fn animate(&mut self, dt: f32) {
        for i in 0..self.tiles.len() {
            let v = self.tiles[i] as usize;
            let to = self.cell_pos(i);
            let at = self.vis[v];
            self.vis[v] = vec2(
                approach(at.x, to.x, SLIDE_RATE, dt),
                approach(at.y, to.y, SLIDE_RATE, dt),
            );
        }
        self.peek = (self.peek - dt).max(0.0);
    }

    fn hint_delay(&self) -> f32 {
        if self.level <= 2 && !self.taught {
            3.0
        } else if self.moves > self.limit * 2 || self.restarts > 0 {
            6.0
        } else {
            14.0
        }
    }

    fn finish_level(&mut self, view: &View) {
        self.state = State::Won;
        self.end_t = 0.0;
        self.stars = stars_for(self.moves, self.limit);
        crate::profile::award_stars(GAME, self.level, self.stars);
        let skip = if self.stars == 3 { 2 } else { 1 };
        crate::profile::unlock(GAME, (self.level + skip).min(MAX_LEVEL));
        crate::profile::set_level(GAME, (self.level + 1).min(MAX_LEVEL));
        crate::profile::set(GAME, &Self::seen_key(self.level), self.variant as i64 + 1);
        crate::profile::set(GAME, "taught", 1);
        self.taught = true;
        self.score += score_for(self.level, self.stars);
        self.rank = self.best.submit(GAME, self.score);
        sfx::play(Sfx::Win);
        sfx::play(Sfx::LevelUp);
        sfx::say(if self.stars == 3 {
            Say::Congrats
        } else {
            Say::Win
        });
        feel::impact_tinted(Impact::Large, Color::from_rgba(255, 220, 60, 255));
        self.fx.confetti(view);
        self.shake.add(0.5);
        self.bump.kick();
    }

    pub fn cell_at(&self, view: &View, p: Vec2) -> Option<usize> {
        let b = board_rect(view);
        if !b.contains(p) {
            return None;
        }
        let cell = b.w / self.n as f32;
        let c = (((p.x - b.x) / cell) as usize).min(self.n - 1);
        let r = (((p.y - b.y) / cell) as usize).min(self.n - 1);
        Some(r * self.n + c)
    }

    fn press(&mut self, input: &Input, view: &View) {
        if !input.pressed {
            return;
        }
        self.idle = 0.0;
        self.hint = None;
        let b = buttons(view);
        if b.restart.contains(input.pos) {
            self.restart();
        } else if b.hint.contains(input.pos) {
            sfx::play(Sfx::Magic);
            self.compute_hint();
        } else if b.peek.contains(input.pos) {
            sfx::play(Sfx::Tap);
            self.peek = PEEK_SECS;
        } else if b.levels.contains(input.pos) {
            sfx::play(Sfx::Tap);
            self.open_select();
        } else if let Some(i) = self.cell_at(view, input.pos) {
            if !self.slide_to(i) {
                sfx::play(Sfx::Tick);
                if self.tiles[i] != 0 {
                    self.shake.add(0.08);
                }
            }
        }
    }
}

impl Game for Slide {
    fn update(&mut self, input: &Input, view: &View, dt: f32) {
        self.t += dt;
        self.fx.update(dt);
        self.shake.update(dt);
        self.pops.update(dt);
        self.bump.update(dt);
        self.animate(dt);
        match self.state {
            State::Select => {
                self.end_t += dt;
                if input.pressed {
                    let l = select_layout(view);
                    if l.play.contains(input.pos) {
                        sfx::play(Sfx::Tap);
                        self.enter(start_level());
                    } else if let Some(i) = l.cells.iter().position(|c| c.contains(input.pos)) {
                        let n = i as u32 + 1;
                        if n <= reached() {
                            sfx::play(Sfx::Tap);
                            self.enter(n);
                        } else {
                            sfx::play(Sfx::Wrong);
                        }
                    }
                }
            }
            State::Playing => {
                if self.pending_win && self.settled() {
                    self.finish_level(view);
                    return;
                }
                if !self.pending_win {
                    self.press(input, view);
                    if input.active() {
                        self.idle = 0.0;
                    } else {
                        self.idle += dt;
                        if self.idle > self.hint_delay() && self.hint.is_none() {
                            self.compute_hint();
                        }
                    }
                }
            }
            State::Won => {
                self.end_t += dt;
                for k in 0..self.stars as usize {
                    let at = WIN_DELAY + STAR_START + STAR_GAP * k as f32;
                    if self.end_t - dt < at && self.end_t >= at {
                        sfx::play_step(Sfx::Coin, k * 2);
                    }
                }
                if self.end_t > WIN_DELAY + 0.7 && input.pressed {
                    let w = win_layout(view, self.can_skip(), self.rank);
                    if w.levels.contains(input.pos) {
                        sfx::play(Sfx::Tap);
                        self.open_select();
                    } else if self.can_skip() && w.skip.contains(input.pos) {
                        sfx::play(Sfx::Tap);
                        self.enter(self.level + 2);
                    } else {
                        self.enter((self.level + 1).min(MAX_LEVEL));
                    }
                }
            }
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Buttons {
    pub restart: Rect,
    pub hint: Rect,
    pub peek: Rect,
    pub levels: Rect,
}

pub fn buttons(view: &View) -> Buttons {
    let r = crate::sizing::bottom_row(view, &["Restart", "Hint", "Peek", "Levels"], 190.0, 62.0);
    Buttons {
        restart: r[0],
        hint: r[1],
        peek: r[2],
        levels: r[3],
    }
}

pub fn hint_gap(view: &View) -> f32 {
    if view.portrait() {
        80.0 * view.text_k()
    } else {
        60.0
    }
}

pub fn hint_pos(view: &View) -> Vec2 {
    let b = buttons(view);
    if view.portrait() {
        vec2(view.w / 2.0, b.restart.y - hint_gap(view) / 2.0 - 4.0 * view.text_k())
    } else {
        vec2(view.w / 2.0, b.restart.y - 36.0)
    }
}

pub fn board_rect(view: &View) -> Rect {
    let b = buttons(view);
    let bottom = b.restart.y - hint_gap(view) - if view.portrait() { 16.0 } else { 0.0 };
    let avail_h = (bottom - HUD_TOP).max(120.0);
    let avail_w = view.w - 2.0 * view.edge();
    let s = (avail_w.min(avail_h) - 2.0 * PAD).max(100.0);
    Rect::new(
        (view.w - s) / 2.0,
        HUD_TOP + (avail_h - s) / 2.0 - PAD * 0.5,
        s,
        s,
    )
}

pub fn cell_rect(view: &View, n: usize, pos: Vec2) -> Rect {
    let b = board_rect(view);
    let cell = b.w / n as f32;
    let gap = 3.0;
    Rect::new(
        b.x + pos.x * cell + gap / 2.0,
        b.y + pos.y * cell + gap / 2.0,
        cell - gap,
        cell - gap,
    )
}

pub fn tile_rects(view: &View, n: usize) -> Vec<Rect> {
    (0..n * n)
        .map(|i| cell_rect(view, n, vec2((i % n) as f32, (i / n) as f32)))
        .collect()
}

pub struct WinLayout {
    pub skip: Rect,
    pub levels: Rect,
}

pub fn win_layout(view: &View, skip: bool, rank: Option<usize>) -> WinLayout {
    let labels: &[&str] = if skip { &WIN_SKIP_LABELS } else { &WIN_LABELS };
    let b = win_results(view, labels, rank).buttons();
    WinLayout {
        skip: b[1],
        levels: *b.last().unwrap(),
    }
}

pub fn win_results(view: &View, labels: &[&str], rank: Option<usize>) -> crate::sizing::Results {
    let (rows, reportable) = crate::sizing::board_shape(GAME, rank);
    crate::sizing::results_layout_ex(
        view,
        "Solved!",
        "Level 10 done, score 100",
        labels,
        rows,
        reportable,
        true,
    )
}

pub use super::ballsort::SelectLayout;

pub fn select_layout(view: &View) -> SelectLayout {
    super::ballsort::select_layout_n(view, MAX_LEVEL as usize)
}

#[cfg(test)]
mod tests {
    use super::*;

    const DT: f32 = 1.0 / 60.0;

    fn setup() {
        crate::save::reset_for_test();
        crate::profile::create("Mia", 0);
    }

    fn views() -> [View; 2] {
        [View::ipad_landscape(), View::ipad_portrait()]
    }

    fn tap_cell(g: &mut Slide, v: &View, i: usize) {
        let at = cell_rect(v, g.n, g.cell_pos(i)).center();
        g.update(&Input::press_at(at), v, DT);
        g.update(&Input::default(), v, DT);
    }

    fn settle(g: &mut Slide, v: &View) {
        for _ in 0..200 {
            if g.settled() {
                break;
            }
            g.update(&Input::default(), v, DT);
        }
    }

    fn play_best(g: &mut Slide, v: &View) {
        let mut guard = 0;
        while g.state == State::Playing && guard < 400 {
            guard += 1;
            g.compute_hint();
            let Some(i) = g.hint else { break };
            tap_cell(g, v, i);
            settle(g, v);
            for _ in 0..4 {
                g.update(&Input::default(), v, DT);
            }
        }
    }

    fn wander(g: &mut Slide, v: &View, taps: usize) {
        let mut rng = Lcg(5);
        for _ in 0..taps {
            let b = g.blank_at();
            let opts = neighbours(g.n, b);
            tap_cell(g, v, opts[rng.below(opts.len())]);
            settle(g, v);
            if g.state != State::Playing || g.pending_win {
                break;
            }
        }
    }

    #[test]
    fn every_level_starts_scrambled_and_can_be_solved() {
        for level in 1..=MAX_LEVEL {
            for variant in 0..3 {
                let p = make(level, variant);
                assert_eq!(p.n, spec(level).n);
                assert!(!is_solved(&p.tiles), "level {level}");
                assert!(p.par >= 1);
                let mut t = p.tiles.clone();
                let mut trail = p.trail.clone();
                while trail.len() >= 2 {
                    let b = t.iter().position(|v| *v == 0).unwrap();
                    assert_eq!(b, *trail.last().unwrap());
                    let to = trail[trail.len() - 2];
                    t.swap(b, to);
                    trail.pop();
                }
                assert!(is_solved(&t), "level {level} variant {variant}");
            }
        }
    }

    #[test]
    fn level_steps_grow_the_board_and_never_get_easier_within_a_size() {
        let mut sizes = vec![];
        for level in 1..=MAX_LEVEL {
            sizes.push(spec(level).n);
        }
        assert_eq!(sizes[0], 2);
        assert_eq!(*sizes.last().unwrap(), 5);
        assert!(sizes.windows(2).all(|w| w[0] <= w[1]));
        for n in 2..=5 {
            let depths: Vec<usize> = (1..=MAX_LEVEL)
                .filter(|l| spec(*l).n == n)
                .map(|l| spec(l).depth)
                .collect();
            assert!(!depths.is_empty(), "no level for {n}x{n}");
            assert!(depths.windows(2).all(|w| w[0] < w[1]), "{n}x{n} {depths:?}");
        }
        let pars: Vec<usize> = (1..=6).map(|l| make(l, 0).par).collect();
        assert!(pars.windows(2).all(|w| w[0] <= w[1]), "{pars:?}");
        assert!(pars[0] <= 3, "the first puzzle is a couple of moves");
    }

    #[test]
    fn small_boards_know_their_exact_par() {
        assert_eq!(distance(&solved_tiles(3)), Some(0));
        let p = make(4, 0);
        assert_eq!(Some(p.par), distance(&p.tiles));
        let mut t = p.tiles.clone();
        let mut steps = 0;
        while let Some(i) = best_move(&t) {
            let b = t.iter().position(|v| *v == 0).unwrap();
            t.swap(b, i);
            steps += 1;
        }
        assert_eq!(steps, p.par);
        assert!(is_solved(&t));
    }

    #[test]
    fn stars_fall_as_the_move_count_rises() {
        for limit in [2, 6, 22, 60] {
            assert_eq!(stars_for(limit, limit), 3);
            assert_eq!(stars_for(limit + 1, limit), 2);
            assert_eq!(stars_for(limit * 2, limit), 2);
            assert_eq!(stars_for(limit * 2 + 1, limit), 1);
            let mut last = 3;
            for m in 0..limit * 4 {
                let s = stars_for(m, limit);
                assert!(s <= last);
                last = s;
            }
        }
    }

    fn random_walk(g: &mut Slide, steps: usize, seed: u64) {
        let mut rng = Lcg(seed);
        for _ in 0..steps {
            let opts = neighbours(g.n, g.blank_at());
            let to = opts[rng.below(opts.len())];
            g.slide_to(to);
        }
    }

    #[test]
    fn an_optimal_solution_always_earns_three_stars_and_a_wandering_one_does_not() {
        for level in 1..=6 {
            for variant in 0..4 {
                let p = make(level, variant);
                assert!(p.limit >= p.par, "level {level}");
                assert_eq!(stars_for(p.par, p.limit), 3, "level {level}");
                assert!(
                    p.limit <= p.par + p.par * LIMIT_SLACK_PCT / 100 + LIMIT_SLACK_FLAT,
                    "level {level} limit {} par {}",
                    p.limit,
                    p.par
                );
                assert!(stars_for(p.par * 3, p.limit) < 3 || p.par * 3 <= p.limit);
            }
        }
        for level in [4, 5, 6] {
            let p = make(level, 0);
            assert!(stars_for(p.par * 3, p.limit) < 3, "level {level}");
        }
    }

    #[test]
    fn the_star_limit_on_big_boards_tracks_the_manhattan_lower_bound() {
        for level in 7..=MAX_LEVEL {
            for variant in 0..4 {
                let p = make(level, variant);
                let lower = manhattan(&p.tiles);
                assert_eq!(p.par, lower);
                assert_eq!(p.limit, star_limit(lower, lower));
                assert!(p.trail.len() - 1 >= lower, "scramble is no shorter than the bound");
                assert!(
                    p.limit * 2 <= (lower + 3) * 3,
                    "level {level}: limit {} for bound {lower}",
                    p.limit
                );
            }
        }
    }

    #[test]
    fn aimlessly_wandering_costs_stars_on_every_board_size() {
        setup();
        for level in [3, 7, 10] {
            let mut g = Slide::with_level(level);
            let limit = g.limit;
            random_walk(&mut g, limit * 3 + 10, 11);
            if g.pending_win {
                continue;
            }
            assert!(g.stars_now() < 3, "level {level}: {} moves, limit {limit}", g.moves);
        }
    }

    #[test]
    fn tapping_a_piece_next_to_the_gap_slides_it_and_a_far_one_does_nothing() {
        setup();
        for v in views() {
            let mut g = Slide::with_level(3);
            let b = g.blank_at();
            let near = neighbours(g.n, b)[0];
            let piece = g.tiles[near];
            tap_cell(&mut g, &v, near);
            assert_eq!(g.tiles[b], piece);
            assert_eq!(g.tiles[near], 0);
            assert_eq!(g.moves, 1);
            settle(&mut g, &v);
            let b2 = g.blank_at();
            let off = (0..g.tiles.len())
                .find(|i| *i != b2 && i % g.n != b2 % g.n && i / g.n != b2 / g.n)
                .unwrap();
            let before = g.tiles.clone();
            tap_cell(&mut g, &v, off);
            assert_eq!(g.tiles, before);
            assert_eq!(g.moves, 1);
        }
    }

    #[test]
    fn tapping_down_the_row_shifts_every_piece_between() {
        setup();
        let v = View::ipad_landscape();
        let mut g = Slide::with_level(7);
        g.tiles = solved_tiles(4);
        g.tiles.swap(15, 12);
        g.trail = vec![12];
        g.vis = vec![Vec2::ZERO; 16];
        let before = g.tiles.clone();
        tap_cell(&mut g, &v, 15);
        assert_eq!(g.moves, 3);
        assert_eq!(g.tiles[12], before[13]);
        assert_eq!(g.tiles[13], before[14]);
        assert_eq!(g.tiles[14], before[15]);
        assert_eq!(g.tiles[15], 0);
    }

    #[test]
    fn a_strong_player_clears_every_level_with_three_stars() {
        setup();
        for v in views() {
            for l in 1..=MAX_LEVEL {
                let mut g = Slide::with_level(l);
                play_best(&mut g, &v);
                assert_eq!(g.state, State::Won, "level {l}");
                assert_eq!(g.stars, 3, "level {l}: {} moves, par {}", g.moves, g.par);
                assert_eq!(crate::profile::stars(GAME, l), 3);
            }
        }
    }

    #[test]
    fn a_strong_player_is_offered_a_skip_ahead_after_three_stars() {
        setup();
        let v = View::ipad_landscape();
        let mut g = Slide::with_level(1);
        play_best(&mut g, &v);
        assert!(g.can_skip());
        assert_eq!(reached(), 3);
    }

    #[test]
    fn a_weak_player_who_wanders_still_finishes_and_progresses() {
        setup();
        let v = View::ipad_landscape();
        let mut g = Slide::with_level(3);
        wander(&mut g, &v, 40);
        play_best(&mut g, &v);
        assert_eq!(g.state, State::Won);
        assert!(g.stars >= 1 && g.stars < 3, "{}", g.stars);
        assert_eq!(reached(), 4);
        assert_eq!(crate::profile::level(GAME), 4);
        assert!(!g.can_skip());
    }

    #[test]
    fn finishing_a_level_saves_stars_unlocks_and_posts_the_score() {
        setup();
        let v = View::ipad_landscape();
        let mut g = Slide::new();
        assert_eq!(g.level, 1);
        play_best(&mut g, &v);
        assert!(crate::profile::stars(GAME, 1) >= 1);
        assert!(g.rank.is_some());
        assert!(g.score > 0);
        assert!(crate::profile::personal_best(GAME).unwrap_or(0) > 0);
        let fresh = Slide::new();
        assert_eq!(fresh.level, 2, "resumes where the child left off");
    }

    #[test]
    fn the_same_level_replayed_is_a_new_puzzle() {
        setup();
        let v = View::ipad_landscape();
        let mut g = Slide::with_level(3);
        let first = g.start.clone();
        play_best(&mut g, &v);
        g.enter(3);
        assert_ne!(first, g.start);
    }

    #[test]
    fn restart_puts_the_pieces_back_and_the_hint_still_works() {
        setup();
        let v = View::ipad_landscape();
        let mut g = Slide::with_level(8);
        let start = g.tiles.clone();
        wander(&mut g, &v, 6);
        assert!(g.moves > 0);
        g.restart();
        assert_eq!(g.tiles, start);
        assert_eq!(g.moves, 0);
        g.compute_hint();
        assert!(g.hint.is_some());
    }

    #[test]
    fn an_idle_child_on_the_first_level_is_shown_a_hint_soon_and_a_veteran_later() {
        setup();
        let v = View::ipad_landscape();
        let mut g = Slide::with_level(1);
        for _ in 0..(3.2 / DT) as usize {
            g.update(&Input::default(), &v, DT);
        }
        let h = g.hint.expect("hint shown");
        assert!(neighbours(g.n, g.blank_at()).contains(&h));
        crate::profile::set(GAME, "taught", 1);
        let mut vet = Slide::with_level(1);
        for _ in 0..(5.0 / DT) as usize {
            vet.update(&Input::default(), &v, DT);
        }
        assert!(vet.hint.is_none());
    }

    #[test]
    fn peek_shows_the_picture_for_a_moment() {
        setup();
        let v = View::ipad_landscape();
        let mut g = Slide::with_level(3);
        let b = buttons(&v);
        g.update(&Input::press_at(b.peek.center()), &v, DT);
        assert!(g.peek > 1.0);
        for _ in 0..(2.0 / DT) as usize {
            g.update(&Input::default(), &v, DT);
        }
        assert_eq!(g.peek, 0.0);
        assert_eq!(g.moves, 0);
    }

    #[test]
    fn pieces_are_big_enough_to_tap_and_stay_on_the_board_on_every_screen() {
        for (name, v) in crate::sizing::test_views() {
            let board = board_rect(&v);
            let b = buttons(&v);
            assert!(board.y >= HUD_TOP - 1.0, "{name}");
            assert!(board.bottom() <= b.restart.y, "{name}");
            assert!(board.x >= 0.0 && board.right() <= v.w, "{name}");
            for n in 2..=5usize {
                let min_mm = match n {
                    2 | 3 => 9.0,
                    4 => 7.0,
                    _ => 5.5,
                };
                for r in tile_rects(&v, n) {
                    assert!(
                        v.to_mm(r.w) >= min_mm - 0.01,
                        "{name} {n}x{n} piece {:.1} mm",
                        v.to_mm(r.w)
                    );
                }
            }
        }
    }

    #[test]
    fn the_hint_pill_sits_between_the_board_and_the_buttons_on_every_screen() {
        for (name, v) in crate::sizing::test_views() {
            if !v.portrait() {
                continue;
            }
            let b = buttons(&v);
            let at = hint_pos(&v);
            let half = 32.0 * v.text_k() * 1.05;
            let board = board_rect(&v);
            assert!(at.y + half <= b.restart.y, "{name}: pill reaches the buttons");
            assert!(at.y - half >= board.bottom() + 10.0, "{name}: pill over the board frame");
        }
    }

    #[test]
    fn every_piece_is_found_by_a_tap_on_its_centre() {
        setup();
        for v in views() {
            let g = Slide::with_level(10);
            for (i, r) in tile_rects(&v, g.n).iter().enumerate() {
                assert_eq!(g.cell_at(&v, r.center()), Some(i));
            }
        }
    }
}
