use crate::engine::*;
use crate::feel::{self, Impact};
use crate::fx::*;
use crate::sfx::{self, Sfx};
use macroquad::prelude::*;

pub const W: usize = 10;
pub const H: usize = 20;
pub const LOCK_DELAY: f32 = 0.5;
pub const CLEAR_TIME: f32 = 0.35;
pub const LINES_PER_LEVEL: u32 = 5;
pub const GAME: &str = "blocks";
const EASY_LEVELS: u32 = 3;
const MAX_RESCUES: u32 = 3;
const GENTLE_PIECES: [u8; 5] = [0, 1, 2, 3, 5];

pub type Cells = [[Option<u8>; W]; H];

const SHAPES: [[(i32, i32); 4]; 7] = [
    [(0, 1), (1, 1), (2, 1), (3, 1)],
    [(0, 0), (0, 1), (1, 1), (2, 1)],
    [(2, 0), (0, 1), (1, 1), (2, 1)],
    [(1, 0), (2, 0), (1, 1), (2, 1)],
    [(1, 0), (2, 0), (0, 1), (1, 1)],
    [(1, 0), (0, 1), (1, 1), (2, 1)],
    [(0, 0), (1, 0), (1, 1), (2, 1)],
];

const SPRITES: [&str; 7] = [
    "blk_i", "blk_j", "blk_l", "blk_o", "blk_s", "blk_t", "blk_z",
];

mod art {
    crate::sprites!("blocks");
}

const COLORS: [(u8, u8, u8); 7] = [
    (80, 210, 240),
    (80, 120, 240),
    (250, 160, 60),
    (250, 215, 60),
    (110, 210, 100),
    (180, 100, 230),
    (240, 80, 90),
];

pub fn piece_color(k: u8) -> Color {
    let (r, g, b) = COLORS[k as usize % 7];
    Color::from_rgba(r, g, b, 255)
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Piece {
    pub kind: u8,
    pub rot: u8,
    pub x: i32,
    pub y: i32,
}

impl Piece {
    pub fn cells(&self) -> [(i32, i32); 4] {
        let size = if self.kind == 0 {
            4
        } else if self.kind == 3 {
            0
        } else {
            3
        };
        SHAPES[self.kind as usize].map(|(cx, cy)| {
            let (mut x, mut y) = (cx, cy);
            if size > 0 {
                for _ in 0..self.rot % 4 {
                    let (nx, ny) = (size - 1 - y, x);
                    x = nx;
                    y = ny;
                }
            }
            (self.x + x, self.y + y)
        })
    }
}

pub fn fits(cells: &Cells, p: &Piece) -> bool {
    p.cells().iter().all(|(x, y)| {
        *x >= 0
            && *x < W as i32
            && *y < H as i32
            && (*y < 0 || cells[*y as usize][*x as usize].is_none())
    })
}

pub fn full_rows(cells: &Cells) -> Vec<usize> {
    (0..H)
        .filter(|y| cells[*y].iter().all(|c| c.is_some()))
        .collect()
}

pub fn remove_rows(cells: &mut Cells, rows: &[usize]) {
    let kept: Vec<[Option<u8>; W]> = (0..H)
        .filter(|y| !rows.contains(y))
        .map(|y| cells[y])
        .collect();
    let pad = H - kept.len();
    for y in 0..H {
        cells[y] = if y < pad { [None; W] } else { kept[y - pad] };
    }
}

pub fn level_for(lines: u32) -> u32 {
    1 + lines / LINES_PER_LEVEL
}

pub fn line_score(lines: usize, level: u32) -> u32 {
    [0, 40, 100, 300, 1200][lines.min(4)] * level
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum State {
    Ready,
    Playing,
    Over,
}

pub struct Blocks {
    pub cells: Cells,
    pub piece: Piece,
    pub next: u8,
    pub bag: Vec<u8>,
    pub score: u32,
    pub lines: u32,
    pub best: Best,
    pub state: State,
    pub clearing: Option<(Vec<usize>, f32)>,
    pub rank: Option<usize>,
    pub moved_once: bool,
    pub rescues: u32,
    fall: f32,
    lock: f32,
    seen_rescues: u32,
    drag_x: Option<f32>,
    drag_y: Option<f32>,
    moved: bool,
    t: f32,
    over_t: f32,
    fx: Particles,
    shake: Shake,
    pops: Popups,
    bump: Bump,
    land: f32,
}

impl Default for Blocks {
    fn default() -> Self {
        Self::new()
    }
}

impl Blocks {
    pub fn new() -> Self {
        let mut g = Self {
            cells: [[None; W]; H],
            piece: Piece {
                kind: 0,
                rot: 0,
                x: 3,
                y: -1,
            },
            next: 0,
            bag: vec![],
            score: 0,
            lines: 0,
            best: Best::load(GAME),
            state: State::Ready,
            clearing: None,
            rank: None,
            moved_once: false,
            rescues: 0,
            fall: 0.0,
            lock: 0.0,
            seen_rescues: 0,
            drag_x: None,
            drag_y: None,
            moved: false,
            t: 0.0,
            over_t: 0.0,
            fx: Particles::default(),
            shake: Shake::default(),
            pops: Popups::default(),
            bump: Bump::default(),
            land: 0.0,
        };
        g.next = g.draw_bag();
        g.spawn();
        g
    }

    fn draw_bag(&mut self) -> u8 {
        if self.bag.is_empty() {
            self.bag = if self.level() < EASY_LEVELS {
                GENTLE_PIECES.to_vec()
            } else {
                (0..7).collect()
            };
            for i in (1..self.bag.len()).rev() {
                self.bag.swap(i, rand::gen_range(0, i + 1));
            }
        }
        self.bag.pop().unwrap()
    }

    pub fn level(&self) -> u32 {
        level_for(self.lines)
    }

    pub fn lines_to_next(&self) -> u32 {
        LINES_PER_LEVEL - self.lines % LINES_PER_LEVEL
    }

    pub fn stack_height(&self) -> usize {
        (0..H)
            .find(|y| self.cells[*y].iter().any(|c| c.is_some()))
            .map_or(0, |y| H - y)
    }

    pub fn gravity(&self) -> f32 {
        let base = 1.2 * 0.87f32.powi(self.level() as i32 - 1);
        let relief = match self.stack_height() {
            h if h >= 15 => 1.6,
            h if h >= 12 => 1.3,
            _ => 1.0,
        };
        let calm = if self.first_timer() { 1.4 } else { 1.0 };
        (base * relief * calm).max(0.08)
    }

    pub fn first_timer(&self) -> bool {
        self.lines == 0 && self.rescues < MAX_RESCUES && crate::profile::get(GAME, "cleared") == 0
    }

    fn end_run(&mut self) {
        self.state = State::Over;
        self.over_t = 0.0;
        self.best.offer(self.score);
        self.rank = crate::profile::submit(GAME, self.score as i64, crate::profile::Better::Higher);
        if crate::profile::get(GAME, "bestlines") < self.lines as i64 {
            crate::profile::set(GAME, "bestlines", self.lines as i64);
        }
    }

    fn spawn(&mut self) {
        let kind = self.next;
        self.next = self.draw_bag();
        self.piece = Piece {
            kind,
            rot: 0,
            x: 3,
            y: -1,
        };
        self.lock = 0.0;
        if !fits(&self.cells, &self.piece) {
            if self.first_timer() {
                self.cells = [[None; W]; H];
                self.rescues += 1;
                return;
            }
            sfx::play(Sfx::Wrong);
            self.shake.add(0.6);
            self.end_run();
        }
    }

    pub fn shift(&mut self, dx: i32) -> bool {
        let p = Piece {
            x: self.piece.x + dx,
            ..self.piece
        };
        if fits(&self.cells, &p) {
            self.piece = p;
            self.lock = 0.0;
            self.moved_once = true;
            true
        } else {
            false
        }
    }

    pub fn rotate(&mut self) -> bool {
        let r = Piece {
            rot: (self.piece.rot + 1) % 4,
            ..self.piece
        };
        for kick in [0, -1, 1, -2, 2] {
            let p = Piece { x: r.x + kick, ..r };
            if fits(&self.cells, &p) {
                self.piece = p;
                self.lock = 0.0;
                self.moved_once = true;
                return true;
            }
        }
        false
    }

    pub fn ghost(&self) -> Piece {
        let mut g = self.piece;
        while fits(&self.cells, &Piece { y: g.y + 1, ..g }) {
            g.y += 1;
        }
        g
    }

    pub fn hard_drop(&mut self) {
        let g = self.ghost();
        self.score += (g.y - self.piece.y).max(0) as u32 * 2;
        self.piece = g;
        sfx::play(Sfx::Hit);
        self.shake.add(0.25);
        self.place();
    }

    pub fn place(&mut self) {
        for (x, y) in self.piece.cells() {
            if y < 0 {
                if self.first_timer() {
                    self.cells = [[None; W]; H];
                    self.rescues += 1;
                    self.spawn();
                    return;
                }
                sfx::play(Sfx::Lose);
                if self.score > self.best.0 && self.best.0 > 0 {
                    sfx::say(sfx::Say::Best);
                }
                self.end_run();
                return;
            }
            self.cells[y as usize][x as usize] = Some(self.piece.kind);
        }
        self.land = 1.0;
        let rows = full_rows(&self.cells);
        if rows.is_empty() {
            self.spawn();
        } else {
            self.clearing = Some((rows, 0.0));
        }
    }

    fn finish_clear(&mut self) {
        if let Some((rows, _)) = self.clearing.take() {
            let gained = line_score(rows.len(), self.level());
            remove_rows(&mut self.cells, &rows);
            self.lines += rows.len() as u32;
            self.score += gained;
            crate::profile::set(GAME, "cleared", 1);
            crate::profile::unlock(GAME, self.level());
            self.bump.kick();
            self.spawn();
        }
    }

    pub fn tick(&mut self, dt: f32) {
        if let Some((_, t)) = &mut self.clearing {
            *t += dt;
            if *t >= CLEAR_TIME {
                self.finish_clear();
            }
            return;
        }
        let down = Piece {
            y: self.piece.y + 1,
            ..self.piece
        };
        if fits(&self.cells, &down) {
            self.fall += dt;
            if self.fall >= self.gravity() {
                self.fall = 0.0;
                self.piece = down;
            }
        } else {
            self.lock += dt;
            if self.lock >= LOCK_DELAY {
                self.place();
            }
        }
    }

    pub fn hint_text(&self) -> Option<&'static str> {
        if !self.first_timer() {
            return None;
        }
        if !self.moved_once {
            Some("Drag to move, tap to turn")
        } else if self.stack_height() == 0 {
            Some("Swipe down to drop")
        } else {
            Some("Fill a whole row!")
        }
    }

    fn draw_progress(&self, bar: Rect) {
        if self.state == State::Ready {
            return;
        }
        let done = (LINES_PER_LEVEL - self.lines_to_next()) as f32 / LINES_PER_LEVEL as f32;
        round_rect(bar, bar.h / 2.0, Color::new(0.0, 0.0, 0.0, 0.35));
        if done > 0.0 {
            round_rect(
                Rect::new(
                    bar.x + 2.0,
                    bar.y + 2.0,
                    ((bar.w - 4.0) * done).max(bar.h - 4.0),
                    bar.h - 4.0,
                ),
                (bar.h - 4.0) / 2.0,
                Color::from_rgba(255, 210, 80, 255),
            );
        }
        let label = format!("{} to Level {}", self.lines_to_next(), self.level() + 1);
        text_center(&label, bar.center().x, bar.center().y, bar.h * 0.7, WHITE);
    }

    fn layout(view: &View) -> (f32, Vec2, bool) {
        let side = view.w > view.h;
        let avail_h = view.h - 110.0;
        let avail_w = if side { view.w * 0.5 } else { view.w - 40.0 };
        let cell = (avail_h / H as f32).min(avail_w / W as f32).floor();
        let o = vec2((view.w - cell * W as f32) / 2.0, 90.0);
        (cell, o, side)
    }
}

pub fn sprite_for(kind: u8) -> &'static str {
    SPRITES[kind as usize % 7]
}

fn block(x: f32, y: f32, s: f32, kind: u8, wash: f32) {
    art::sprite(
        sprite_for(kind),
        Rect::new(x + 0.5, y + 0.5, s - 1.0, s - 1.0),
    );
    if wash > 0.0 {
        round_rect(
            Rect::new(x + 1.0, y + 1.0, s - 2.0, s - 2.0),
            s * 0.2,
            Color::new(1.0, 1.0, 1.0, wash * 0.9),
        );
    }
}

fn backdrop(view: &View) {
    let f = view.full();
    let name = if view.w > view.h { "bg" } else { "bg_p" };
    let s = art::size(name);
    let k = (f.w / s.x).max(f.h / s.y);
    let (w, h) = (s.x * k, s.y * k);
    art::sprite(
        name,
        Rect::new(f.x + (f.w - w) / 2.0, f.y + (f.h - h) / 2.0, w, h),
    );
}

impl Game for Blocks {
    fn update(&mut self, input: &Input, view: &View, dt: f32) {
        self.t += dt;
        self.fx.update(dt);
        self.shake.update(dt);
        self.pops.update(dt);
        self.bump.update(dt);
        self.land = (self.land - dt * 5.0).max(0.0);
        match self.state {
            State::Ready => {
                if input.tap() {
                    self.state = State::Playing;
                }
                return;
            }
            State::Over => {
                self.over_t += dt;
                if self.over_t > 0.8 && input.tap() {
                    let best = self.best;
                    *self = Self::new();
                    self.best = best;
                    self.state = State::Playing;
                }
                return;
            }
            State::Playing => {}
        }
        let (cell, o, _) = Self::layout(view);
        let lines_before = self.lines;
        let level_before = self.level();
        if self.clearing.is_none() {
            if input.pressed {
                self.drag_x = Some(input.pos.x);
                self.drag_y = Some(input.pos.y);
                self.moved = false;
            }
            if input.down {
                if let (Some(dx0), Some(dy0)) = (self.drag_x, self.drag_y) {
                    let step = cell * 0.9;
                    let slop = TAP_SLOP_MM * input.mm();
                    let need = |moved: bool| if moved { step } else { step.max(slop) };
                    while input.pos.x - self.drag_x.unwrap() > need(self.moved) {
                        self.shift(1);
                        self.drag_x = Some(self.drag_x.unwrap() + step);
                        self.moved = true;
                    }
                    while self.drag_x.unwrap() - input.pos.x > need(self.moved) {
                        self.shift(-1);
                        self.drag_x = Some(self.drag_x.unwrap() - step);
                        self.moved = true;
                    }
                    if input.pos.y - dy0 > cell * 1.5
                        && (input.pos.x - dx0).abs() < cell * 2.0
                        && input.pos.y - input.prev_pos.y > cell * 0.5
                    {
                        self.hard_drop();
                        self.drag_x = None;
                        self.drag_y = None;
                        self.moved = true;
                    }
                }
            }
            if input.released {
                if !self.moved && self.drag_x.is_some() {
                    self.rotate();
                }
                self.drag_x = None;
                self.drag_y = None;
            }
            match input.key_dir {
                Some(Dir::Left) => {
                    self.shift(-1);
                }
                Some(Dir::Right) => {
                    self.shift(1);
                }
                Some(Dir::Up) => {
                    self.rotate();
                }
                Some(Dir::Down) => self.hard_drop(),
                None => {}
            }
        }
        if self.state == State::Playing {
            self.tick(dt);
        }
        if self.rescues > self.seen_rescues {
            self.seen_rescues = self.rescues;
            self.pops.add(
                o + vec2(cell * W as f32 / 2.0, cell * H as f32 * 0.4),
                "Fresh start!",
                WHITE,
                44.0,
            );
            sfx::play(Sfx::LevelUp);
        }
        if let Some((rows, t)) = &self.clearing {
            if *t < dt * 1.5 {
                for y in rows {
                    for x in 0..W {
                        let c = self.cells[*y][x].map(piece_color).unwrap_or(WHITE);
                        let at = o + vec2((x as f32 + 0.5) * cell, (*y as f32 + 0.5) * cell);
                        self.fx.emit(
                            at,
                            &[c, WHITE],
                            Burst {
                                count: 3,
                                shape: Shape::Square,
                                speed: (100.0, 300.0),
                                ..Default::default()
                            },
                        );
                    }
                }
                self.shake.add(0.15 * rows.len() as f32);
            }
        }
        if self.lines > lines_before {
            let n = self.lines - lines_before;
            sfx::play(if n >= 4 { Sfx::Win } else { Sfx::LevelUp });
            feel::impact(if n >= 4 {
                Impact::Large
            } else {
                Impact::Medium
            });
            let word = ["", "Nice!", "Double!", "Triple!", "AMAZING!"][n.min(4) as usize];
            self.pops.add(
                o + vec2(cell * W as f32 / 2.0, cell * H as f32 * 0.4),
                word,
                hsv(self.t * 0.3, 0.6, 1.0),
                40.0 + n as f32 * 10.0,
            );
            if n >= 4 {
                self.fx.confetti(view);
            }
            if lines_before == 0 && n < 4 {
                sfx::say(sfx::Say::Great);
            }
        }
        if self.level() > level_before {
            crate::profile::unlock(GAME, self.level());
            sfx::play(Sfx::Win);
            sfx::say(sfx::Say::LevelUp);
            feel::impact_tinted(Impact::Large, hsv(self.t * 0.3, 0.6, 1.0));
            self.pops.add(
                o + vec2(cell * W as f32 / 2.0, cell * H as f32 * 0.25),
                format!("Level {}!", self.level()),
                Color::from_rgba(255, 220, 90, 255),
                60.0,
            );
            self.fx.confetti(view);
        }
    }

    fn draw(&self, view: &View) {
        backdrop(view);
        let (cell, o, side) = Self::layout(view);
        let sh = self.shake.offset();
        let o = o + sh + vec2(0.0, self.land * 3.0);
        let bw = cell * W as f32;
        let bh = cell * H as f32;
        round_rect(
            Rect::new(o.x - 4.0, o.y - 2.0, bw + 8.0, bh + 12.0),
            26.0,
            Color::from_rgba(255, 215, 120, 255),
        );
        round_rect(
            Rect::new(o.x - 4.0, o.y - 4.0, bw + 8.0, bh + 8.0),
            24.0,
            Color::from_rgba(186, 120, 235, 255),
        );
        round_rect(
            Rect::new(o.x - 1.0, o.y - 1.0, bw + 2.0, bh + 2.0),
            20.0,
            Color::from_rgba(24, 14, 62, 255),
        );
        round_rect(
            Rect::new(o.x, o.y, bw, bh),
            19.0,
            Color::from_rgba(40, 26, 92, 245),
        );
        for y in 0..H {
            for x in 0..W {
                if (x + y) % 2 == 0 {
                    draw_rectangle(
                        o.x + x as f32 * cell,
                        o.y + y as f32 * cell,
                        cell,
                        cell,
                        Color::new(1.0, 1.0, 1.0, 0.035),
                    );
                }
            }
        }
        let flash = self
            .clearing
            .as_ref()
            .map(|(rows, t)| (rows.clone(), *t / CLEAR_TIME));
        for y in 0..H {
            for x in 0..W {
                if let Some(k) = self.cells[y][x] {
                    let mut wash = 0.0;
                    let mut s = cell;
                    if let Some((rows, p)) = &flash {
                        if rows.contains(&y) {
                            let d = (x as f32 - 4.5).abs() / 5.0;
                            let k2 = ((p - d * 0.4) / 0.6).clamp(0.0, 1.0);
                            wash = k2;
                            s = cell * (1.0 - k2 * 0.9);
                        }
                    }
                    let off = (cell - s) / 2.0;
                    block(
                        o.x + x as f32 * cell + off,
                        o.y + y as f32 * cell + off,
                        s,
                        k,
                        wash,
                    );
                }
            }
        }
        if self.state == State::Playing && self.clearing.is_none() {
            let g = self.ghost();
            for (x, y) in g.cells() {
                if y >= 0 {
                    let r = Rect::new(
                        o.x + x as f32 * cell + 3.0,
                        o.y + y as f32 * cell + 3.0,
                        cell - 6.0,
                        cell - 6.0,
                    );
                    art::sprite_ex(
                        sprite_for(self.piece.kind),
                        Rect::new(r.x - 3.0, r.y - 3.0, cell, cell),
                        crate::art::Draw {
                            tint: Color::new(1.0, 1.0, 1.0, 0.3),
                            ..Default::default()
                        },
                    );
                }
            }
            for (x, y) in self.piece.cells() {
                if y >= 0 {
                    block(
                        o.x + x as f32 * cell,
                        o.y + y as f32 * cell,
                        cell,
                        self.piece.kind,
                        0.0,
                    );
                }
            }
        }
        let panel_x = if side { o.x + bw + 30.0 } else { o.x };
        let panel_y = if side { o.y } else { 20.0 };
        if side {
            crate::ui::painted_panel(
                Rect::new(panel_x, panel_y, 170.0, 150.0),
                Color::new(1.0, 1.0, 1.0, 0.95),
            );
            crate::fx::text_outline(
                "NEXT",
                panel_x + 85.0,
                panel_y + 28.0,
                26.0,
                WHITE,
                Color::from_rgba(110, 50, 170, 255),
            );
            let pc = Piece {
                kind: self.next,
                rot: 0,
                x: 0,
                y: 0,
            };
            let s = 28.0;
            let bob = (self.t * 3.0).sin() * 3.0;
            for (x, y) in pc.cells() {
                block(
                    panel_x + 85.0 - 2.0 * s
                        + x as f32 * s
                        + if self.next == 3 {
                            -s * 0.5 + s
                        } else if self.next == 0 {
                            0.0
                        } else {
                            s * 0.5
                        },
                    panel_y + 60.0 + y as f32 * s + bob,
                    s,
                    self.next,
                    0.0,
                );
            }
            for (k, (label, v)) in [
                ("SCORE", self.score),
                ("LINES", self.lines),
                ("LEVEL", self.level()),
                ("BEST", self.best.0),
            ]
            .iter()
            .enumerate()
            {
                let y = panel_y + 180.0 + k as f32 * 80.0;
                text(
                    label,
                    panel_x,
                    y,
                    22.0,
                    Color::from_rgba(255, 225, 150, 255),
                );
                let sc = if k == 0 { self.bump.scale(0.25) } else { 1.0 };
                let digits = v.to_string();
                let tw = crate::outline::measure(FONT_BYTES, &digits, 40.0 * sc);
                crate::fx::text_outline(
                    &digits,
                    panel_x + tw / 2.0,
                    y + 24.0,
                    40.0 * sc,
                    WHITE,
                    Color::from_rgba(70, 30, 120, 255),
                );
            }
        } else {
            text_shadow(
                &format!("{}", self.score),
                view.w / 2.0,
                50.0,
                52.0 * self.bump.scale(0.25),
                WHITE,
            );
            text(
                &format!("Lv {}", self.level()),
                view.w - 120.0,
                50.0,
                30.0,
                WHITE,
            );
        }
        let bar = if side {
            Rect::new(panel_x, panel_y + 486.0, 190.0, 28.0)
        } else {
            Rect::new(o.x, 34.0, bw * 0.32, 26.0)
        };
        self.draw_progress(bar);
        self.fx.draw();
        self.pops.draw();
        if self.state == State::Playing && self.clearing.is_none() {
            if let Some(h) = self.hint_text() {
                let pulse = 1.0 + (self.t * 4.0).sin() * 0.04;
                crate::ui::hint(
                    view,
                    h,
                    vec2(
                        o.x + bw / 2.0,
                        if side { o.y - 40.0 } else { o.y + bh * 0.3 },
                    ),
                    pulse,
                );
            }
        }
        match self.state {
            State::Ready => {
                let top = crate::profile::unlocked(GAME);
                let goal = "Fill rows to clear them".to_string();
                let best = format!("Your best: Level {top}");
                let mut lines: Vec<&str> = vec![&goal];
                if top > 1 {
                    lines = vec![&best];
                }
                lines.push("Tap to start");
                overlay(view, "Block Drop", &lines);
            }
            State::Over if self.over_t > 0.5 => {
                let line = format!(
                    "Score {}   Lines {}   Level {}",
                    self.score,
                    self.lines,
                    self.level()
                );
                results(
                    view,
                    &crate::ui::Results {
                        game: GAME,
                        title: "Stacked up!",
                        line: &line,
                        rank: self.rank,
                        action: "Play again",
                        format: crate::ui::plain_score,
                    },
                );
            }
            _ => {}
        }
    }
}

pub fn icon(r: Rect) {
    crate::ui::icon("blocks", r);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blocks_sprites_all_exist() {
        let have = art::names();
        for n in SPRITES.iter().chain(&["bg", "bg_p"]) {
            assert!(have.iter().any(|h| h == n), "missing {n}");
        }
        for k in 0..7 {
            assert_eq!(sprite_for(k), SPRITES[k as usize]);
        }
    }

    fn empty() -> Blocks {
        let mut g = Blocks::new();
        g.state = State::Playing;
        g
    }

    #[test]
    fn a_phone_tap_that_slides_a_little_still_rotates() {
        let v = View::fit(2992.0 / 2.625, 1344.0 / 2.625);
        let mm = (495.0 / 25.4 / 2.625) / v.scale;
        let mut g = empty();
        g.piece.kind = 2;
        let (x, rot) = (g.piece.x, g.piece.rot);
        let at = v.center();
        g.update(
            &Input {
                mm,
                ..Input::press_at(at)
            },
            &v,
            0.001,
        );
        let slid = at + vec2(3.5 * mm, 0.0);
        g.update(
            &Input {
                pos: slid,
                prev_pos: at,
                down: true,
                mm,
                ..Default::default()
            },
            &v,
            0.001,
        );
        g.update(
            &Input {
                pos: slid,
                prev_pos: slid,
                released: true,
                travel: 3.5 * mm,
                mm,
                ..Default::default()
            },
            &v,
            0.001,
        );
        assert_eq!(g.piece.x, x);
        assert_ne!(g.piece.rot, rot);
    }

    #[test]
    fn every_rotation_keeps_four_distinct_cells() {
        for kind in 0..7 {
            for rot in 0..4 {
                let p = Piece {
                    kind,
                    rot,
                    x: 3,
                    y: 5,
                };
                let mut c = p.cells().to_vec();
                c.sort();
                c.dedup();
                assert_eq!(c.len(), 4);
            }
        }
    }

    #[test]
    fn four_rotations_return_to_start() {
        let mut g = empty();
        g.piece = Piece {
            kind: 1,
            rot: 0,
            x: 4,
            y: 5,
        };
        let start = g.piece.cells();
        for _ in 0..4 {
            assert!(g.rotate());
        }
        assert_eq!(g.piece.cells(), start);
    }

    #[test]
    fn walls_stop_shifting() {
        let mut g = empty();
        for _ in 0..20 {
            g.shift(-1);
        }
        assert!(g.piece.cells().iter().all(|(x, _)| *x >= 0));
        assert!(!g.shift(-1));
    }

    #[test]
    fn rotating_against_wall_kicks_inside() {
        let mut g = empty();
        g.piece = Piece {
            kind: 0,
            rot: 1,
            x: -2,
            y: 5,
        };
        assert!(fits(&g.cells, &g.piece));
        assert!(g.rotate());
        assert!(g
            .piece
            .cells()
            .iter()
            .all(|(x, _)| *x >= 0 && *x < W as i32));
    }

    #[test]
    fn hard_drop_lands_on_floor() {
        let mut g = empty();
        g.piece = Piece {
            kind: 3,
            rot: 0,
            x: 3,
            y: 0,
        };
        g.hard_drop();
        assert!(g.cells[H - 1][4].is_some() && g.cells[H - 2][5].is_some());
    }

    #[test]
    fn full_row_clears_and_scores() {
        let mut g = empty();
        for x in 0..W - 4 {
            g.cells[H - 1][x] = Some(1);
        }
        g.piece = Piece {
            kind: 0,
            rot: 0,
            x: W as i32 - 4,
            y: H as i32 - 2,
        };
        g.hard_drop();
        assert!(g.clearing.is_some());
        for _ in 0..30 {
            g.tick(1.0 / 60.0);
        }
        assert_eq!(g.lines, 1);
        assert!(g.score >= line_score(1, 1));
        assert!(g.cells[H - 1].iter().all(|c| c.is_none()));
    }

    #[test]
    fn rows_above_a_clear_fall_down() {
        let mut cells: Cells = [[None; W]; H];
        cells[H - 2][0] = Some(5);
        cells[H - 1] = [Some(1); W];
        let full = full_rows(&cells);
        remove_rows(&mut cells, &full);
        assert_eq!(cells[H - 1][0], Some(5));
        assert!(cells[H - 2].iter().all(|c| c.is_none()));
    }

    #[test]
    fn piece_locks_after_delay_and_next_spawns() {
        let mut g = empty();
        g.piece = Piece {
            kind: 3,
            rot: 0,
            x: 3,
            y: H as i32 - 2,
        };
        g.tick(LOCK_DELAY + 0.01);
        assert!(g.cells[H - 1][4].is_some());
        assert!(g.piece.y < 2);
    }

    #[test]
    fn blocked_spawn_ends_game() {
        crate::save::reset_for_test();
        let mut g = empty();
        g.lines = 1;
        crate::profile::set(GAME, "cleared", 1);
        for y in 0..H {
            for x in 0..W {
                if x != 0 {
                    g.cells[y][x] = Some(2);
                }
            }
        }
        g.spawn();
        assert_eq!(g.state, State::Over);
    }

    #[test]
    fn bag_deals_each_piece_once_per_seven() {
        let mut g = empty();
        g.lines = LINES_PER_LEVEL * EASY_LEVELS;
        g.bag.clear();
        let mut seen: Vec<u8> = (0..7).map(|_| g.draw_bag()).collect();
        seen.sort();
        assert_eq!(seen, (0..7).collect::<Vec<u8>>());
    }

    fn survive(g: &mut Blocks, seconds: f32) {
        let mut t = 0.0;
        while t < seconds && g.state == State::Playing {
            g.tick(1.0 / 60.0);
            t += 1.0 / 60.0;
        }
    }

    #[test]
    fn a_child_who_never_touches_it_survives_the_first_stretch() {
        crate::save::reset_for_test();
        crate::profile::create("Mia", 0);
        let mut g = empty();
        survive(&mut g, 40.0);
        assert_eq!(g.state, State::Playing);
    }

    #[test]
    fn first_timer_gets_a_fresh_board_instead_of_game_over() {
        crate::save::reset_for_test();
        crate::profile::create("Mia", 0);
        let mut g = empty();
        for y in 0..H {
            for x in 1..W {
                g.cells[y][x] = Some(2);
            }
        }
        g.spawn();
        assert_eq!(g.state, State::Playing);
        assert_eq!(g.rescues, 1);
        assert_eq!(g.stack_height(), 0);
    }

    #[test]
    fn after_the_first_line_the_game_can_end() {
        crate::save::reset_for_test();
        crate::profile::create("Mia", 0);
        let mut g = empty();
        for x in 0..W - 4 {
            g.cells[H - 1][x] = Some(1);
        }
        g.piece = Piece {
            kind: 0,
            rot: 0,
            x: W as i32 - 4,
            y: H as i32 - 2,
        };
        g.hard_drop();
        survive(&mut g, 1.0);
        assert!(!g.first_timer());
        for y in 0..H {
            for x in 1..W {
                g.cells[y][x] = Some(2);
            }
        }
        g.spawn();
        assert_eq!(g.state, State::Over);
    }

    #[test]
    fn gravity_speeds_up_with_level() {
        crate::save::reset_for_test();
        let mut g = empty();
        crate::profile::set(GAME, "cleared", 1);
        let mut last = g.gravity();
        for lines in [5, 10, 20, 40] {
            g.lines = lines;
            let now = g.gravity();
            assert!(now < last, "level {} not faster", g.level());
            last = now;
        }
    }

    #[test]
    fn first_level_is_slower_than_the_old_start() {
        crate::save::reset_for_test();
        let g = empty();
        assert!(g.gravity() >= 1.2);
    }

    #[test]
    fn a_tall_stack_slows_the_fall() {
        crate::save::reset_for_test();
        crate::profile::set(GAME, "cleared", 1);
        let mut g = empty();
        let low = g.gravity();
        for x in 0..W - 1 {
            g.cells[H - 13][x] = Some(1);
        }
        assert!(g.gravity() > low);
    }

    #[test]
    fn early_levels_only_deal_friendly_pieces() {
        let mut g = empty();
        for _ in 0..50 {
            let k = g.draw_bag();
            assert!(GENTLE_PIECES.contains(&k));
        }
        g.lines = LINES_PER_LEVEL * EASY_LEVELS;
        g.bag.clear();
        let all: Vec<u8> = (0..7).map(|_| g.draw_bag()).collect();
        assert!(all.contains(&4) && all.contains(&6));
    }

    #[test]
    fn levels_come_every_few_lines() {
        assert_eq!(level_for(0), 1);
        assert_eq!(level_for(LINES_PER_LEVEL - 1), 1);
        assert_eq!(level_for(LINES_PER_LEVEL), 2);
    }

    #[test]
    fn reaching_a_level_is_saved_per_player_and_game_over_submits() {
        crate::save::reset_for_test();
        let mia = crate::profile::create("Mia", 0).unwrap();
        let mut g = empty();
        g.lines = LINES_PER_LEVEL - 1;
        g.score = 500;
        for x in 0..W - 4 {
            g.cells[H - 1][x] = Some(1);
        }
        g.piece = Piece {
            kind: 0,
            rot: 0,
            x: W as i32 - 4,
            y: H as i32 - 2,
        };
        g.hard_drop();
        survive(&mut g, 1.0);
        assert_eq!(crate::profile::unlocked(GAME), 2);
        for y in 0..H {
            for x in 1..W {
                g.cells[y][x] = Some(2);
            }
        }
        g.spawn();
        assert_eq!(g.state, State::Over);
        assert_eq!(g.rank, Some(0));
        assert_eq!(Blocks::new().best.0, g.score);
        crate::profile::create("Leo", 1);
        assert_eq!(crate::profile::unlocked(GAME), 0);
        assert_eq!(Blocks::new().best.0, 0);
        crate::profile::select(&mia.id);
        assert_eq!(crate::profile::unlocked(GAME), 2);
    }
}
