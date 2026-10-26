use crate::engine::*;
use crate::feel::{self, Impact};
use crate::fx::*;
use crate::geom::*;
use crate::rng as rand;
use crate::sfx::{self, Sfx};
use glam::{vec2, Vec2};

pub const SLIDE: f32 = 0.12;

pub const N: usize = 4;
pub type Board = [[u32; N]; N];

pub fn slide_row(row: [u32; N]) -> ([u32; N], u32) {
    let tiles: Vec<u32> = row.into_iter().filter(|v| *v != 0).collect();
    let mut out = [0; N];
    let mut gained = 0;
    let mut i = 0;
    let mut o = 0;
    while i < tiles.len() {
        if i + 1 < tiles.len() && tiles[i] == tiles[i + 1] {
            out[o] = tiles[i] * 2;
            gained += out[o];
            i += 2;
        } else {
            out[o] = tiles[i];
            i += 1;
        }
        o += 1;
    }
    (out, gained)
}

fn line_cells(dir: Dir, line: usize, n: usize) -> Vec<(usize, usize)> {
    (0..n)
        .map(|k| match dir {
            Dir::Left => (line, k),
            Dir::Right => (line, n - 1 - k),
            Dir::Up => (k, line),
            Dir::Down => (n - 1 - k, line),
        })
        .collect()
}

pub fn shift(board: &Board, n: usize, dir: Dir) -> (Board, u32) {
    let mut out = [[0; N]; N];
    let mut gained = 0;
    for line in 0..n {
        let cells = line_cells(dir, line, n);
        let mut row = [0; N];
        for (k, (r, c)) in cells.iter().enumerate() {
            row[k] = board[*r][*c];
        }
        let (slid, g) = slide_row(row);
        gained += g;
        for (k, (r, c)) in cells.iter().enumerate() {
            out[*r][*c] = slid[k];
        }
    }
    (out, gained)
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TileMove {
    pub from: (usize, usize),
    pub to: (usize, usize),
    pub value: u32,
    pub merged: bool,
}

pub fn plan(board: &Board, n: usize, dir: Dir) -> Vec<TileMove> {
    let mut moves = vec![];
    for line in 0..n {
        let cells = line_cells(dir, line, n);
        let tiles: Vec<(usize, u32)> = (0..n)
            .map(|k| (k, board[cells[k].0][cells[k].1]))
            .filter(|(_, v)| *v != 0)
            .collect();
        let mut i = 0;
        let mut o = 0;
        while i < tiles.len() {
            let merge = i + 1 < tiles.len() && tiles[i].1 == tiles[i + 1].1;
            for t in &tiles[i..i + 1 + merge as usize] {
                moves.push(TileMove {
                    from: cells[t.0],
                    to: cells[o],
                    value: t.1,
                    merged: merge,
                });
            }
            i += 1 + merge as usize;
            o += 1;
        }
    }
    moves
}

pub fn can_move(board: &Board, n: usize) -> bool {
    Dir::ALL.iter().any(|d| shift(board, n, *d).0 != *board)
}

pub const GAME: &str = "merge";
pub const UNDO_HISTORY: usize = 3;
pub const EASY_LEVELS: u32 = 2;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LevelDef {
    pub size: usize,
    pub target: u32,
}

pub const LEVELS: [LevelDef; 8] = [
    LevelDef {
        size: 3,
        target: 16,
    },
    LevelDef {
        size: 3,
        target: 32,
    },
    LevelDef {
        size: 4,
        target: 64,
    },
    LevelDef {
        size: 4,
        target: 128,
    },
    LevelDef {
        size: 4,
        target: 256,
    },
    LevelDef {
        size: 4,
        target: 512,
    },
    LevelDef {
        size: 4,
        target: 1024,
    },
    LevelDef {
        size: 4,
        target: 2048,
    },
];

pub fn last_level() -> u32 {
    LEVELS.len() as u32
}

pub fn level_def(level: u32) -> LevelDef {
    LEVELS[(level.clamp(1, last_level()) - 1) as usize]
}

pub fn four_chance(level: u32, fails: u32) -> f32 {
    if level <= EASY_LEVELS || fails >= 2 {
        0.0
    } else if level <= 4 {
        0.05
    } else {
        0.1
    }
}

pub fn undos_for(level: u32, fails: u32) -> Option<u32> {
    if level <= EASY_LEVELS {
        None
    } else {
        Some(3 + fails.min(3))
    }
}

pub fn forgiving(level: u32, fails: u32) -> bool {
    level <= EASY_LEVELS || fails >= 3
}

pub fn stars_for(undos_used: u32) -> u8 {
    match undos_used {
        0 => 3,
        1..=2 => 2,
        _ => 1,
    }
}

pub fn open_levels(unlocked: u32) -> u32 {
    unlocked.clamp(1, last_level())
}

pub fn resume_level(saved: u32, unlocked: u32) -> u32 {
    saved.clamp(1, open_levels(unlocked))
}

pub fn progress(max_tile: u32, target: u32) -> f32 {
    if max_tile < 2 {
        return 0.0;
    }
    (max_tile.ilog2() as f32 / target.ilog2() as f32).min(1.0)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Screen {
    Select,
    Play,
    Complete,
    Lost,
}

#[derive(Clone)]
struct Snapshot {
    board: Board,
    score: u32,
    moves: u32,
}

pub struct Merge {
    pub board: Board,
    pub n: usize,
    pub level: u32,
    pub screen: Screen,
    pub score: u32,
    pub moves: u32,
    pub best: Best,
    pub rank: Option<usize>,
    pub over: bool,
    pub won: bool,
    pub record_broken: bool,
    pub fails: u32,
    pub stars_earned: u8,
    pub undos_used: u32,
    pub undos_left: Option<u32>,
    pub learned: bool,
    history: Vec<Snapshot>,
    rescued: bool,
    end_t: f32,
    pub bar: f32,
    pub screen_t: f32,
    pub slides: Vec<TileMove>,
    pub slide_t: f32,
    pub born: [[f32; N]; N],
    pub merged_at: [[f32; N]; N],
    reveal: Vec<(usize, usize, u32)>,
    pub particles: Particles,
    pub popups: Popups,
    pub shake: Shake,
    pub score_bump: Bump,
    pub t: f32,
}

impl Merge {
    pub fn new() -> Self {
        let saved = crate::profile::level(GAME);
        let unlocked = crate::profile::unlocked(GAME);
        Self::at(resume_level(saved, unlocked))
    }

    pub fn at(level: u32) -> Self {
        Self::with_fails(level, 0)
    }

    fn with_fails(level: u32, fails: u32) -> Self {
        let level = level.clamp(1, last_level());
        let mut g = Self {
            board: [[0; N]; N],
            n: level_def(level).size,
            level,
            screen: Screen::Play,
            score: 0,
            moves: 0,
            best: Best::load(GAME),
            rank: None,
            over: false,
            won: false,
            record_broken: false,
            fails,
            stars_earned: 0,
            undos_used: 0,
            undos_left: undos_for(level, fails),
            learned: crate::profile::get(GAME, "learned") != 0
                || crate::profile::unlocked(GAME) > 1,
            history: vec![],
            rescued: false,
            end_t: 0.0,
            bar: 0.0,
            screen_t: 0.0,
            slides: vec![],
            slide_t: SLIDE,
            born: [[1.0; N]; N],
            merged_at: [[1.0; N]; N],
            reveal: vec![],
            particles: Particles::default(),
            popups: Popups::default(),
            shake: Shake::default(),
            score_bump: Bump::default(),
            t: 0.0,
        };
        g.deal();
        crate::profile::set_level(GAME, level);
        g
    }

    pub fn target(&self) -> u32 {
        level_def(self.level).target
    }

    fn deal(&mut self) {
        self.board = [[0; N]; N];
        if self.level == 1 && !self.learned {
            for c in [0, 2] {
                self.board[1][c] = 2;
                self.born[1][c] = 0.0;
            }
        } else {
            self.spawn();
            self.spawn();
        }
    }

    pub fn begin(&mut self, level: u32) {
        let fails = if level == self.level { self.fails } else { 0 };
        let best = self.best;
        let (particles, popups) = (
            std::mem::take(&mut self.particles),
            std::mem::take(&mut self.popups),
        );
        let t = self.t;
        *self = Self::with_fails(level, fails);
        self.best = best;
        self.particles = particles;
        self.popups = popups;
        self.t = t;
        sfx::say(sfx::Say::Go);
    }

    fn open_level(&mut self, level: u32) {
        let in_progress = self.level == level && self.moves > 0 && !self.over && !self.won;
        if in_progress {
            self.screen = Screen::Play;
        } else {
            self.begin(level);
        }
    }

    fn spawn(&mut self) -> Option<(usize, usize)> {
        let n = self.n;
        let empty: Vec<(usize, usize)> = (0..n)
            .flat_map(|r| (0..n).map(move |c| (r, c)))
            .filter(|(r, c)| self.board[*r][*c] == 0)
            .collect();
        if empty.is_empty() {
            return None;
        }
        let (r, c) = empty[rand::gen_range(0, empty.len())];
        let four = rand::gen_range(0.0, 1.0) < four_chance(self.level, self.fails);
        self.board[r][c] = if four { 4 } else { 2 };
        self.born[r][c] = 0.0;
        Some((r, c))
    }

    pub fn sliding(&self) -> bool {
        self.slide_t < SLIDE
    }

    pub fn play(&mut self, dir: Dir) -> bool {
        let (next, gained) = shift(&self.board, self.n, dir);
        if next == self.board {
            return false;
        }
        self.history.push(Snapshot {
            board: self.board,
            score: self.score,
            moves: self.moves,
        });
        if self.history.len() > UNDO_HISTORY {
            self.history.remove(0);
        }
        self.moves += 1;
        self.slides = plan(&self.board, self.n, dir);
        self.slide_t = 0.0;
        self.reveal = self
            .slides
            .iter()
            .filter(|m| m.merged)
            .map(|m| (m.to.0, m.to.1, m.value * 2))
            .collect();
        self.reveal.dedup();
        for (r, c, _) in &self.reveal {
            self.merged_at[*r][*c] = -SLIDE;
        }
        if !self.reveal.is_empty() && !self.learned {
            self.learned = true;
            crate::profile::set(GAME, "learned", 1);
        }
        self.board = next;
        self.score += gained;
        if gained > 0 {
            self.score_bump.kick();
        }
        if self.best.0 > 0 && self.score > self.best.0 {
            self.record_broken = true;
        }
        self.best.offer(self.score);
        if let Some((r, c)) = self.spawn() {
            self.born[r][c] = -SLIDE;
        }
        self.won = self.max_tile() >= self.target();
        if !self.won && !can_move(&self.board, self.n) {
            if forgiving(self.level, self.fails) {
                self.rescue();
            } else {
                self.over = true;
            }
        }
        true
    }

    fn rescue(&mut self) {
        let n = self.n;
        let smallest = (0..n)
            .flat_map(|r| (0..n).map(move |c| (r, c)))
            .map(|(r, c)| self.board[r][c])
            .filter(|v| *v > 0)
            .min()
            .unwrap_or(0);
        for r in 0..n {
            for c in 0..n {
                if self.board[r][c] == smallest {
                    self.board[r][c] = 0;
                }
            }
        }
        self.rescued = true;
    }

    pub fn can_undo(&self) -> bool {
        !self.history.is_empty() && self.undos_left != Some(0)
    }

    pub fn undo(&mut self) -> bool {
        if !self.can_undo() {
            return false;
        }
        let Some(s) = self.history.pop() else {
            return false;
        };
        self.board = s.board;
        self.score = s.score;
        self.moves = s.moves;
        self.slides.clear();
        self.reveal.clear();
        self.slide_t = SLIDE;
        self.over = false;
        self.won = false;
        self.born = [[1.0; N]; N];
        self.merged_at = [[1.0; N]; N];
        self.undos_used += 1;
        if let Some(left) = &mut self.undos_left {
            *left -= 1;
        }
        true
    }

    pub fn max_tile(&self) -> u32 {
        self.board.iter().flatten().copied().max().unwrap_or(0)
    }

    pub fn ended(&self) -> bool {
        self.won || self.over
    }

    fn finish(&mut self, view: &View) {
        self.rank = self.best.submit(GAME, self.score);
        self.screen_t = 0.0;
        if self.won {
            self.complete(view);
        } else {
            self.fail();
        }
    }

    fn complete(&mut self, view: &View) {
        let last = last_level();
        self.stars_earned = stars_for(self.undos_used);
        crate::profile::award_stars(GAME, self.level, self.stars_earned);
        if self.level < last {
            crate::profile::unlock(GAME, self.level + 1);
        }
        crate::profile::set_level(GAME, (self.level + 1).min(last));
        self.fails = 0;
        self.screen = Screen::Complete;
        sfx::play(Sfx::Win);
        sfx::say(if self.level == last {
            sfx::Say::Win
        } else {
            sfx::Say::Congrats
        });
        feel::impact_tinted(Impact::Large, Color::from_rgba(255, 210, 60, 255));
        self.particles.confetti(view);
    }

    fn fail(&mut self) {
        self.fails += 1;
        self.screen = Screen::Lost;
        sfx::play(Sfx::Lose);
        sfx::say(if self.record_broken {
            sfx::Say::Best
        } else {
            sfx::Say::GameOver
        });
        self.shake.add(0.5);
    }
}

impl Default for Merge {
    fn default() -> Self {
        Self::new()
    }
}

pub fn tile_tint(v: u32) -> Color {
    let k = v.max(2).ilog2() as usize;
    const HUES: [f32; 11] = [
        0.58, 0.5, 0.38, 0.25, 0.15, 0.09, 0.0, 0.92, 0.83, 0.75, 0.68,
    ];
    hsv(HUES[(k - 1).min(HUES.len() - 1)], 0.55, 1.0)
}

pub struct Layout {
    pub origin: Vec2,
    pub size: f32,
    pub gap: f32,
    pub cell: f32,
}

impl Layout {
    pub fn new(view: &View, n: usize) -> Self {
        let top = 160.0;
        let avail = (view.h - top - 70.0).max(100.0);
        let size = (avail / 1.15).min(view.w - 140.0);
        let origin = vec2((view.w - size) / 2.0, top + (avail - size) / 2.0);
        let gap = size * if n <= 3 { 0.04 } else { 0.03 };
        Self {
            origin,
            size,
            gap,
            cell: (size - gap * (n as f32 + 1.0)) / n as f32,
        }
    }

    pub fn pos(&self, r: f32, c: f32) -> Vec2 {
        self.origin
            + vec2(
                self.gap + c * (self.cell + self.gap),
                self.gap + r * (self.cell + self.gap),
            )
    }

    pub fn center(&self, r: usize, c: usize) -> Vec2 {
        self.pos(r as f32, c as f32) + Vec2::splat(self.cell / 2.0)
    }
}

pub fn chip_x(view: &View) -> f32 {
    (view.w / 2.0 - 360.0).max(HUD_LEFT)
}

pub fn bottom_buttons(view: &View) -> (Rect, Rect) {
    let (w, h) = (150.0, 56.0);
    let y = view.h - h - 12.0;
    let x = chip_x(view);
    (Rect::new(x, y, w, h), Rect::new(view.w - x - w, y, w, h))
}

pub fn level_rects(view: &View) -> Vec<Rect> {
    let gap = 20.0;
    let w = ((view.w - 60.0 - gap * 3.0) / 4.0).min(210.0);
    let h = (w * 1.25).min(260.0);
    let total_w = w * 4.0 + gap * 3.0;
    let total_h = h * 2.0 + gap;
    let x0 = (view.w - total_w) / 2.0;
    let y0 = 150.0 + ((view.h - 150.0 - 20.0 - total_h) / 2.0).max(0.0);
    (0..LEVELS.len())
        .map(|i| {
            Rect::new(
                x0 + (i % 4) as f32 * (w + gap),
                y0 + (i / 4) as f32 * (h + gap),
                w,
                h,
            )
        })
        .collect()
}

pub fn complete_buttons(view: &View) -> (Rect, Rect) {
    let (_, next, levels) = complete_layout(view);
    (next, levels)
}

pub fn complete_layout(view: &View) -> (Rect, Rect, Rect) {
    let pw = 640.0f32.min(view.w - 40.0);
    let ph = 480.0;
    let c = view.center() + vec2(0.0, 20.0);
    let panel = Rect::new(c.x - pw / 2.0, c.y - ph / 2.0, pw, ph);
    let y = panel.y + ph - 112.0;
    (
        panel,
        Rect::new(c.x - 280.0, y, 310.0, 82.0),
        Rect::new(c.x + 50.0, y, 220.0, 82.0),
    )
}

pub fn merge_scale(age: f32) -> f32 {
    if (0.0..0.2).contains(&age) {
        1.0 + 0.22 * (age / 0.2 * std::f32::consts::PI).sin()
    } else {
        1.0
    }
}

pub fn born_scale(age: f32) -> f32 {
    if age < 0.0 {
        0.0
    } else {
        ease_out_back(age / 0.25)
    }
}

impl Game for Merge {
    fn update(&mut self, input: &Input, view: &View, dt: f32) {
        self.t += dt;
        self.screen_t += dt;
        for a in self.born.iter_mut().flatten() {
            *a = (*a + dt).min(1.0);
        }
        for a in self.merged_at.iter_mut().flatten() {
            *a = (*a + dt).min(1.0);
        }
        if self.sliding() {
            self.slide_t += dt;
            if !self.sliding() {
                self.land(view);
            }
        }
        self.bar = approach(self.bar, progress(self.max_tile(), self.target()), 8.0, dt);
        self.score_bump.update(dt);
        self.particles.update(dt);
        self.popups.update(dt);
        self.shake.update(dt);
        match self.screen {
            Screen::Select => self.update_select(input, view),
            Screen::Play => self.update_play(input, view, dt),
            Screen::Complete => self.update_complete(input, view),
            Screen::Lost => self.update_lost(input, view),
        }
    }
}

impl Merge {
    fn update_select(&mut self, input: &Input, view: &View) {
        if input.back {
            self.screen = Screen::Play;
            return;
        }
        if !input.pressed {
            return;
        }
        let open = open_levels(crate::profile::unlocked(GAME));
        for (i, r) in level_rects(view).iter().enumerate() {
            if r.contains(input.pos) {
                let level = i as u32 + 1;
                if level <= open {
                    sfx::play(Sfx::Click);
                    self.open_level(level);
                } else {
                    sfx::play(Sfx::Wrong);
                }
            }
        }
    }

    fn update_play(&mut self, input: &Input, view: &View, dt: f32) {
        if self.ended() {
            if !self.sliding() {
                self.end_t += dt;
                if self.end_t > 0.7 {
                    self.finish(view);
                }
            }
            return;
        }
        let (levels, undo) = bottom_buttons(view);
        if input.tapped_in(levels) {
            sfx::play(Sfx::Click);
            self.screen = Screen::Select;
            return;
        }
        if input.tapped_in(undo) {
            if self.undo() {
                sfx::play(Sfx::Swish);
            } else {
                sfx::play(Sfx::Wrong);
            }
            return;
        }
        if let Some(d) = input.dir() {
            if self.sliding() {
                self.slide_t = SLIDE;
                self.land(view);
            }
            self.play(d);
        }
    }

    fn update_complete(&mut self, input: &Input, view: &View) {
        if self.screen_t < 0.6 {
            return;
        }
        let (_, next, levels) = complete_layout(view);
        if input.tapped_in(levels) {
            sfx::play(Sfx::Click);
            self.screen = Screen::Select;
        } else if input.tapped_in(next) || input.action {
            let level = (self.level + 1).min(last_level());
            self.begin(level);
        }
    }

    fn update_lost(&mut self, input: &Input, view: &View) {
        if self.screen_t < 0.6 {
            return;
        }
        let (levels, _) = bottom_buttons(view);
        if input.tapped_in(levels) {
            sfx::play(Sfx::Click);
            self.screen = Screen::Select;
        } else if input.tap() {
            self.begin(self.level);
        }
    }

    fn land(&mut self, view: &View) {
        let g = Layout::new(view, self.n);
        for (r, c, v) in std::mem::take(&mut self.reveal) {
            self.merged_at[r][c] = 0.0;
            let at = g.center(r, c);
            let bg = tile_tint(v);
            self.particles.emit(
                at,
                &[bg, WHITE, darker(bg, 0.8)],
                Burst {
                    count: 8 + (v.ilog2() as usize).min(8) * 2,
                    speed: (80.0, 260.0),
                    size: (4.0, 8.0),
                    life: (0.3, 0.6),
                    gravity: 300.0,
                    shape: Shape::Square,
                    ..Default::default()
                },
            );
            sfx::play_step(
                if v >= 128 { Sfx::Coin } else { Sfx::Pop },
                v.ilog2().saturating_sub(2) as usize,
            );
            if v >= 256 {
                feel::impact(Impact::Medium);
            }
            self.popups.add(
                at - vec2(0.0, g.cell * 0.3),
                format!("+{v}"),
                Color::from_rgba(255, 220, 80, 255),
                30.0 + (v.ilog2() as f32) * 1.5,
            );
            if v >= 128 {
                self.shake.add(0.15 + v.ilog2() as f32 * 0.03);
            }
        }
        if self.rescued {
            self.rescued = false;
            self.popups.add(
                view.center(),
                "Fresh start!",
                Color::from_rgba(255, 255, 255, 255),
                64.0,
            );
            sfx::play(Sfx::Magic);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fresh() -> Merge {
        crate::save::reset_for_test();
        crate::profile::create("Mia", 0);
        Merge::new()
    }

    fn settle(g: &mut Merge, v: &View) {
        g.update(&Input::default(), v, SLIDE + 0.01);
    }

    fn swipe(g: &mut Merge, v: &View, d: Dir) {
        let i = Input {
            swipe: Some(d),
            ..Default::default()
        };
        g.update(&i, v, SLIDE + 0.01);
    }

    fn tap(g: &mut Merge, v: &View, p: Vec2) {
        g.update(&Input::press_at(p), v, 0.01);
    }

    fn run_out_the_clock(g: &mut Merge, v: &View) {
        for _ in 0..90 {
            g.update(&Input::default(), v, 0.02);
        }
    }

    fn stuck4() -> Board {
        [[2, 4, 2, 4], [4, 2, 4, 2], [2, 4, 2, 4], [4, 2, 4, 2]]
    }

    #[test]
    fn merge_sprites_all_exist() {
        let b = std::fs::read(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../assets/art/merge.kga"
        ))
        .unwrap();
        let have: Vec<String> = crate::art::parse(&b)
            .unwrap()
            .frames
            .keys()
            .cloned()
            .collect();
        for n in ["bg", "tile", "cell", "board", "crown"] {
            assert!(have.iter().any(|h| h == n), "missing {n}");
        }
    }

    #[test]
    fn every_tile_value_has_its_own_colour() {
        let mut seen: Vec<Color> = vec![];
        for k in 1..=11 {
            let c = tile_tint(1 << k);
            assert!(!seen.contains(&c), "2^{k} repeats a colour");
            seen.push(c);
        }
    }

    #[test]
    fn row_merges_each_tile_once() {
        assert_eq!(slide_row([2, 2, 2, 2]), ([4, 4, 0, 0], 8));
        assert_eq!(slide_row([2, 2, 4, 0]), ([4, 4, 0, 0], 4));
        assert_eq!(slide_row([0, 4, 4, 8]), ([8, 8, 0, 0], 8));
        assert_eq!(slide_row([2, 0, 0, 2]), ([4, 0, 0, 0], 4));
        assert_eq!(slide_row([2, 4, 2, 4]), ([2, 4, 2, 4], 0));
    }

    #[test]
    fn shifting_right_and_down() {
        let b = [[2, 2, 0, 0], [0, 0, 0, 0], [0, 0, 0, 0], [2, 0, 0, 0]];
        let (r, g) = shift(&b, 4, Dir::Right);
        assert_eq!(r[0], [0, 0, 0, 4]);
        assert_eq!(g, 4);
        let (d, g) = shift(&b, 4, Dir::Down);
        assert_eq!(d[3], [4, 2, 0, 0]);
        assert_eq!(g, 4);
    }

    #[test]
    fn a_three_by_three_board_slides_inside_its_own_edges() {
        let b = [[2, 0, 2, 0], [0, 2, 0, 0], [0, 0, 0, 0], [0; 4]];
        let (r, g) = shift(&b, 3, Dir::Right);
        assert_eq!(r[0], [0, 0, 4, 0]);
        assert_eq!(r[1], [0, 0, 2, 0]);
        assert_eq!(g, 4);
        let (d, _) = shift(&b, 3, Dir::Down);
        assert_eq!(d[2], [2, 2, 2, 0]);
        assert_eq!(d[3], [0; 4]);
    }

    #[test]
    fn plan_tracks_every_tile_and_merges() {
        let b = [[2, 2, 4, 0], [0, 0, 0, 0], [0, 2, 0, 2], [8, 0, 0, 0]];
        let moves = plan(&b, 4, Dir::Left);
        assert_eq!(moves.len(), 6);
        let find = |from: (usize, usize)| *moves.iter().find(|m| m.from == from).unwrap();
        assert_eq!(find((0, 0)).to, (0, 0));
        assert!(find((0, 0)).merged && find((0, 1)).merged);
        assert_eq!(find((0, 1)).to, (0, 0));
        assert_eq!(find((0, 2)).to, (0, 1));
        assert!(!find((0, 2)).merged);
        assert_eq!(find((2, 3)).to, (2, 0));
        assert_eq!(find((3, 0)).to, (3, 0));
    }

    #[test]
    fn plan_agrees_with_shift_for_every_direction() {
        let b = [[2, 4, 4, 2], [0, 2, 0, 2], [8, 8, 8, 8], [0, 0, 0, 2]];
        for n in [3, 4] {
            for d in Dir::ALL {
                let (next, _) = shift(&b, n, d);
                let mut rebuilt = [[0u32; N]; N];
                for m in plan(&b, n, d) {
                    rebuilt[m.to.0][m.to.1] += m.value;
                }
                assert_eq!(rebuilt, next, "{n} {d:?}");
            }
        }
    }

    #[test]
    fn stuck_board_detected() {
        let b = stuck4();
        assert!(!can_move(&b, 4));
        let mut b2 = b;
        b2[0][0] = 4;
        assert!(can_move(&b2, 4));
    }

    #[test]
    fn useless_move_does_not_spawn() {
        let mut g = fresh();
        g.board = [[2, 0, 0, 0], [0; 4], [0; 4], [0; 4]];
        assert!(!g.play(Dir::Left));
        assert!(!g.play(Dir::Up));
        assert_eq!(g.board.iter().flatten().filter(|v| **v > 0).count(), 1);
    }

    #[test]
    fn move_starts_a_slide_that_finishes_and_pops() {
        let v = View::ipad_landscape();
        let mut g = fresh();
        g.board = [[2, 2, 0, 0], [0; 4], [0; 4], [0; 4]];
        assert!(g.play(Dir::Left));
        assert!(g.sliding());
        assert_eq!(g.reveal.len(), 1);
        settle(&mut g, &v);
        assert!(!g.sliding());
        assert!(!g.popups.list.is_empty());
        assert!(!g.particles.list.is_empty());
        assert!(g.merged_at[0][0] >= 0.0);
    }

    #[test]
    fn scales_start_hidden_and_overshoot() {
        assert_eq!(born_scale(-0.05), 0.0);
        assert!(born_scale(0.1) > 0.5);
        assert!((born_scale(1.0) - 1.0).abs() < 1e-4);
        assert_eq!(merge_scale(1.0), 1.0);
        assert!(merge_scale(0.1) > 1.15);
    }

    #[test]
    fn beating_a_standing_best_is_remembered() {
        let mut g = fresh();
        g.best = Best(4);
        g.board = [[4, 4, 0, 0], [0; 4], [0; 4], [0; 4]];
        assert!(g.play(Dir::Left));
        assert!(g.record_broken);
    }

    #[test]
    fn a_new_child_starts_on_a_small_board_with_a_merge_ready() {
        let g = fresh();
        assert_eq!(g.level, 1);
        assert_eq!(g.n, 3);
        assert_eq!(g.target(), 16);
        let tiles: Vec<u32> = g
            .board
            .iter()
            .flatten()
            .copied()
            .filter(|v| *v > 0)
            .collect();
        assert_eq!(tiles, vec![2, 2]);
        assert!(!g.learned);
    }

    #[test]
    fn any_first_swipe_moves_something_and_a_horizontal_one_merges() {
        let g = fresh();
        for d in Dir::ALL {
            assert_ne!(shift(&g.board, g.n, d).0, g.board, "{d:?}");
        }
        for d in [Dir::Left, Dir::Right] {
            assert_eq!(shift(&g.board, g.n, d).1, 4);
        }
    }

    #[test]
    fn the_first_levels_never_deal_a_four() {
        for level in 1..=EASY_LEVELS {
            assert_eq!(four_chance(level, 0), 0.0);
        }
        assert!(four_chance(3, 0) > 0.0);
        assert!(four_chance(8, 0) > four_chance(3, 0));
        assert_eq!(four_chance(8, 2), 0.0);
        let mut g = fresh();
        for _ in 0..200 {
            g.board = [[0; N]; N];
            g.spawn();
            assert!(g.board.iter().flatten().all(|v| *v == 0 || *v == 2));
        }
    }

    #[test]
    fn levels_grow_the_board_then_the_target() {
        assert_eq!(LEVELS[0].size, 3);
        assert!(LEVELS.iter().all(|l| l.size == 3 || l.size == 4));
        for w in LEVELS.windows(2) {
            assert!(w[1].target > w[0].target);
            assert!(w[1].size >= w[0].size);
        }
        assert_eq!(LEVELS.last().unwrap().target, 2048);
        assert!(LEVELS.iter().all(|l| l.target.is_power_of_two()));
    }

    fn nearly_stuck3() -> Board {
        [[0, 4, 8, 0], [8, 4, 8, 0], [4, 8, 4, 0], [0; 4]]
    }

    #[test]
    fn a_stuck_board_on_level_one_is_rescued_not_lost() {
        let v = View::ipad_landscape();
        let mut g = fresh();
        g.board = nearly_stuck3();
        assert!(g.play(Dir::Left));
        assert!(!g.over);
        assert!(g.rescued);
        assert!(can_move(&g.board, 3));
        settle(&mut g, &v);
        assert!(g.popups.list.iter().any(|p| p.text == "Fresh start!"));
        assert_eq!(g.screen, Screen::Play);
    }

    #[test]
    fn rescue_clears_the_smallest_tiles_and_frees_a_move() {
        let mut g = fresh();
        g.board = [[4, 8, 2, 0], [8, 4, 8, 0], [4, 8, 4, 0], [0; 4]];
        assert!(!can_move(&g.board, 3));
        g.rescue();
        assert!(can_move(&g.board, 3));
        assert_eq!(g.board[0][2], 0);
        assert_eq!(g.board[1][1], 4);
    }

    #[test]
    fn a_hard_level_ends_when_stuck() {
        let mut g = fresh();
        g.begin(3);
        for seed in 0..64 {
            rand::srand(seed);
            g.begin(3);
            g.board = [[0, 4, 8, 16], [16, 4, 8, 32], [4, 8, 16, 8], [8, 16, 4, 16]];
            assert!(g.play(Dir::Left));
            assert!(g.over, "seed {seed}");
            assert!(!g.rescued);
        }
    }

    #[test]
    fn reaching_the_target_wins_and_stars_unlock_the_next_level() {
        let v = View::ipad_landscape();
        let mut g = fresh();
        g.board = [[8, 8, 0, 0], [0; 4], [0; 4], [0; 4]];
        crate::sfx::take_pending();
        assert!(g.play(Dir::Left));
        assert!(g.won);
        settle(&mut g, &v);
        run_out_the_clock(&mut g, &v);
        assert_eq!(g.screen, Screen::Complete);
        assert_eq!(g.stars_earned, 3);
        assert_eq!(crate::profile::stars(GAME, 1), 3);
        assert_eq!(crate::profile::unlocked(GAME), 2);
        assert_eq!(crate::profile::level(GAME), 2);
        assert!(crate::sfx::take_pending().contains(&Sfx::Win));
        assert_eq!(crate::sfx::take_voice(), Some(sfx::Say::Congrats));
        assert!(!g.particles.list.is_empty());
    }

    #[test]
    fn using_undo_costs_stars() {
        assert_eq!(stars_for(0), 3);
        assert_eq!(stars_for(1), 2);
        assert_eq!(stars_for(2), 2);
        assert_eq!(stars_for(3), 1);
        assert_eq!(stars_for(30), 1);
    }

    #[test]
    fn undo_takes_back_a_move() {
        let mut g = fresh();
        g.board = [[2, 0, 0, 0], [0, 2, 0, 0], [0; 4], [0; 4]];
        let before = g.board;
        assert!(g.play(Dir::Left));
        assert_ne!(g.board, before);
        assert!(g.undo());
        assert_eq!(g.board, before);
        assert_eq!(g.moves, 0);
        assert_eq!(g.undos_used, 1);
        assert!(!g.undo());
    }

    #[test]
    fn undo_supply_is_unlimited_early_and_limited_later() {
        assert_eq!(undos_for(1, 0), None);
        assert_eq!(undos_for(2, 0), None);
        assert_eq!(undos_for(3, 0), Some(3));
        assert!(undos_for(3, 2).unwrap() > 3);
        let mut g = fresh();
        g.begin(3);
        g.undos_left = Some(1);
        g.board = [[2, 0, 0, 0], [0, 2, 0, 0], [0; 4], [0; 4]];
        g.play(Dir::Left);
        g.play(Dir::Up);
        assert!(g.undo());
        assert!(!g.can_undo());
        assert!(!g.undo());
    }

    #[test]
    fn undo_is_tappable_and_levels_button_opens_the_picker() {
        let v = View::ipad_landscape();
        let mut g = fresh();
        g.board = [[2, 0, 0, 0], [0, 2, 0, 0], [0; 4], [0; 4]];
        swipe(&mut g, &v, Dir::Left);
        let after = g.board;
        let (levels, undo) = bottom_buttons(&v);
        tap(&mut g, &v, undo.center());
        assert_ne!(g.board, after);
        tap(&mut g, &v, levels.center());
        assert_eq!(g.screen, Screen::Select);
    }

    #[test]
    fn losing_ends_the_run_with_a_ranked_results_screen() {
        let v = View::ipad_landscape();
        let mut g = fresh();
        g.begin(3);
        g.board = [[0, 4, 8, 16], [16, 4, 8, 32], [4, 8, 16, 8], [8, 16, 4, 16]];
        g.score = 100;
        crate::sfx::take_pending();
        assert!(g.play(Dir::Left));
        settle(&mut g, &v);
        run_out_the_clock(&mut g, &v);
        assert_eq!(g.screen, Screen::Lost);
        assert_eq!(g.rank, Some(0));
        assert_eq!(g.fails, 1);
        assert!(crate::sfx::take_pending().contains(&Sfx::Lose));
        assert_eq!(crate::sfx::take_voice(), Some(sfx::Say::GameOver));
        assert_eq!(Best::load(GAME).0, g.score);
    }

    #[test]
    fn the_lost_screen_retries_the_same_level() {
        let v = View::ipad_landscape();
        let mut g = fresh();
        g.begin(3);
        g.screen = Screen::Lost;
        g.fails = 1;
        g.over = true;
        g.screen_t = 1.0;
        tap(&mut g, &v, v.center());
        assert_eq!(g.screen, Screen::Play);
        assert_eq!(g.level, 3);
        assert_eq!(g.fails, 1);
        assert!(!g.over);
    }

    #[test]
    fn failing_eases_the_next_attempt() {
        let mut g = fresh();
        g.begin(4);
        g.fails = 2;
        g.begin(4);
        assert_eq!(g.fails, 2);
        assert!(g.undos_left.unwrap() > 3);
        assert_eq!(four_chance(4, g.fails), 0.0);
        g.fails = 3;
        assert!(forgiving(4, g.fails));
        assert!(!forgiving(4, 0));
        g.begin(5);
        assert_eq!(g.fails, 0);
    }

    #[test]
    fn locked_levels_stay_locked_and_unlocked_ones_open() {
        let v = View::ipad_landscape();
        let mut g = fresh();
        g.screen = Screen::Select;
        let rects = level_rects(&v);
        tap(&mut g, &v, rects[4].center());
        assert_eq!(g.screen, Screen::Select);
        crate::profile::unlock(GAME, 5);
        tap(&mut g, &v, rects[4].center());
        assert_eq!(g.screen, Screen::Play);
        assert_eq!(g.level, 5);
        assert_eq!(g.n, 4);
    }

    #[test]
    fn progress_saves_per_player_and_resumes() {
        crate::save::reset_for_test();
        let mia = crate::profile::create("Mia", 0).unwrap();
        crate::profile::unlock(GAME, 4);
        crate::profile::set_level(GAME, 4);
        let leo = crate::profile::create("Leo", 1).unwrap();
        assert_eq!(Merge::new().level, 1);
        crate::profile::select(&mia.id);
        let g = Merge::new();
        assert_eq!(g.level, 4);
        assert_eq!(g.n, 4);
        crate::profile::select(&leo.id);
        assert_eq!(Merge::new().level, 1);
    }

    #[test]
    fn resuming_never_jumps_past_what_is_unlocked() {
        assert_eq!(resume_level(0, 0), 1);
        assert_eq!(resume_level(6, 3), 3);
        assert_eq!(resume_level(2, 5), 2);
        assert_eq!(resume_level(99, 99), last_level());
    }

    #[test]
    fn the_hint_stops_once_the_child_has_merged() {
        let v = View::ipad_landscape();
        let mut g = fresh();
        assert!(!g.learned);
        swipe(&mut g, &v, Dir::Left);
        assert!(g.learned);
        assert_eq!(crate::profile::get(GAME, "learned"), 1);
        assert!(Merge::new().learned);
    }

    #[test]
    fn a_returning_child_does_not_get_the_teaching_deal() {
        crate::save::reset_for_test();
        crate::profile::create("Mia", 0);
        crate::profile::set(GAME, "learned", 1);
        let g = Merge::new();
        assert!(g.learned);
    }

    #[test]
    fn progress_bar_tracks_the_biggest_tile() {
        assert_eq!(progress(0, 64), 0.0);
        assert_eq!(progress(2, 64), 1.0 / 6.0);
        assert_eq!(progress(64, 64), 1.0);
        assert_eq!(progress(128, 64), 1.0);
    }

    #[test]
    fn the_last_level_celebrates_2048() {
        let v = View::ipad_landscape();
        let mut g = fresh();
        crate::profile::unlock(GAME, last_level());
        g.begin(last_level());
        g.board = [[1024, 1024, 0, 0], [0; 4], [0; 4], [0; 4]];
        g.play(Dir::Left);
        settle(&mut g, &v);
        run_out_the_clock(&mut g, &v);
        assert_eq!(g.screen, Screen::Complete);
        assert_eq!(crate::profile::level(GAME), last_level());
        assert_eq!(crate::sfx::take_voice(), Some(sfx::Say::Win));
    }

    #[test]
    fn next_level_button_moves_on() {
        let v = View::ipad_landscape();
        let mut g = fresh();
        g.board = [[8, 8, 0, 0], [0; 4], [0; 4], [0; 4]];
        g.play(Dir::Left);
        settle(&mut g, &v);
        run_out_the_clock(&mut g, &v);
        assert_eq!(g.screen, Screen::Complete);
        let (_, next, _) = complete_layout(&v);
        tap(&mut g, &v, next.center());
        assert_eq!(g.screen, Screen::Play);
        assert_eq!(g.level, 2);
        assert_eq!(g.moves, 0);
    }
}
