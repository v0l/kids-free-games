use crate::engine::draw_circle;
use crate::engine::*;
use crate::fx::*;
use macroquad::prelude::*;

pub const W: usize = 10;
pub const H: usize = 20;
pub const LOCK_DELAY: f32 = 0.5;
pub const CLEAR_TIME: f32 = 0.35;

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
    fall: f32,
    lock: f32,
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
            best: Best::default(),
            state: State::Ready,
            clearing: None,
            fall: 0.0,
            lock: 0.0,
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
            self.bag = (0..7).collect();
            for i in (1..7).rev() {
                self.bag.swap(i, rand::gen_range(0, i + 1));
            }
        }
        self.bag.pop().unwrap()
    }

    pub fn level(&self) -> u32 {
        1 + self.lines / 10
    }

    fn gravity(&self) -> f32 {
        (0.8 * 0.85f32.powi(self.level() as i32 - 1)).max(0.08)
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
            self.state = State::Over;
            self.over_t = 0.0;
            self.best.offer(self.score);
            self.shake.add(0.6);
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
        self.shake.add(0.25);
        self.place();
    }

    pub fn place(&mut self) {
        for (x, y) in self.piece.cells() {
            if y < 0 {
                self.state = State::Over;
                self.over_t = 0.0;
                self.best.offer(self.score);
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

    fn layout(view: &View) -> (f32, Vec2, bool) {
        let side = view.w > view.h;
        let avail_h = view.h - 110.0;
        let avail_w = if side { view.w * 0.5 } else { view.w - 40.0 };
        let cell = (avail_h / H as f32).min(avail_w / W as f32).floor();
        let o = vec2((view.w - cell * W as f32) / 2.0, 90.0);
        (cell, o, side)
    }
}

fn block(x: f32, y: f32, s: f32, c: Color) {
    let r = Rect::new(x + 1.0, y + 1.0, s - 2.0, s - 2.0);
    round_rect(r, s * 0.18, darker(c, 0.7));
    round_rect(Rect::new(r.x, r.y, r.w, r.h - s * 0.12), s * 0.18, c);
    round_rect(
        Rect::new(r.x + s * 0.15, r.y + s * 0.12, r.w - s * 0.3, s * 0.18),
        s * 0.09,
        Color::new(1.0, 1.0, 1.0, 0.35),
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
        if self.clearing.is_none() {
            if input.pressed {
                self.drag_x = Some(input.pos.x);
                self.drag_y = Some(input.pos.y);
                self.moved = false;
            }
            if input.down {
                if let (Some(dx0), Some(dy0)) = (self.drag_x, self.drag_y) {
                    let step = cell * 0.9;
                    while input.pos.x - self.drag_x.unwrap() > step {
                        self.shift(1);
                        self.drag_x = Some(self.drag_x.unwrap() + step);
                        self.moved = true;
                    }
                    while self.drag_x.unwrap() - input.pos.x > step {
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
        }
    }

    fn draw(&self, view: &View) {
        sky(
            view,
            Color::from_rgba(40, 40, 90, 255),
            Color::from_rgba(90, 50, 130, 255),
        );
        for k in 0..30 {
            let x = ((k * 97) % 1000) as f32 / 1000.0 * view.w;
            let y = ((k * 61) % 1000) as f32 / 1000.0 * view.h;
            let tw = 0.3 + 0.3 * (self.t * 2.0 + k as f32).sin();
            draw_circle(x, y, 1.5 + (k % 3) as f32, Color::new(1.0, 1.0, 1.0, tw));
        }
        let (cell, o, side) = Self::layout(view);
        let sh = self.shake.offset();
        let o = o + sh + vec2(0.0, self.land * 3.0);
        let bw = cell * W as f32;
        let bh = cell * H as f32;
        round_rect(
            Rect::new(o.x - 10.0, o.y - 10.0, bw + 20.0, bh + 20.0),
            18.0,
            Color::from_rgba(30, 25, 60, 255),
        );
        round_rect(
            Rect::new(o.x - 4.0, o.y - 4.0, bw + 8.0, bh + 8.0),
            12.0,
            Color::from_rgba(50, 45, 95, 255),
        );
        for x in 1..W {
            draw_line(
                o.x + x as f32 * cell,
                o.y,
                o.x + x as f32 * cell,
                o.y + bh,
                1.0,
                Color::new(1.0, 1.0, 1.0, 0.05),
            );
        }
        let flash = self
            .clearing
            .as_ref()
            .map(|(rows, t)| (rows.clone(), *t / CLEAR_TIME));
        for y in 0..H {
            for x in 0..W {
                if let Some(k) = self.cells[y][x] {
                    let mut c = piece_color(k);
                    let mut s = cell;
                    if let Some((rows, p)) = &flash {
                        if rows.contains(&y) {
                            let d = (x as f32 - 4.5).abs() / 5.0;
                            let k2 = ((p - d * 0.4) / 0.6).clamp(0.0, 1.0);
                            c = Color::new(
                                c.r + (1.0 - c.r) * k2,
                                c.g + (1.0 - c.g) * k2,
                                c.b + (1.0 - c.b) * k2,
                                1.0,
                            );
                            s = cell * (1.0 - k2 * 0.9);
                        }
                    }
                    let off = (cell - s) / 2.0;
                    block(
                        o.x + x as f32 * cell + off,
                        o.y + y as f32 * cell + off,
                        s,
                        c,
                    );
                }
            }
        }
        if self.state == State::Playing && self.clearing.is_none() {
            let g = self.ghost();
            let c = piece_color(self.piece.kind);
            for (x, y) in g.cells() {
                if y >= 0 {
                    let r = Rect::new(
                        o.x + x as f32 * cell + 3.0,
                        o.y + y as f32 * cell + 3.0,
                        cell - 6.0,
                        cell - 6.0,
                    );
                    round_rect(r, cell * 0.18, with_alpha(c, 0.18));
                    draw_rectangle_lines(r.x, r.y, r.w, r.h, 2.0, with_alpha(c, 0.5));
                }
            }
            for (x, y) in self.piece.cells() {
                if y >= 0 {
                    block(o.x + x as f32 * cell, o.y + y as f32 * cell, cell, c);
                }
            }
        }
        let panel_x = if side { o.x + bw + 30.0 } else { o.x };
        let panel_y = if side { o.y } else { 20.0 };
        if side {
            round_rect(
                Rect::new(panel_x, panel_y, 170.0, 150.0),
                18.0,
                Color::new(1.0, 1.0, 1.0, 0.12),
            );
            text_center(
                "NEXT",
                panel_x + 85.0,
                panel_y + 24.0,
                24.0,
                Color::new(1.0, 1.0, 1.0, 0.8),
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
                    piece_color(self.next),
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
                text(label, panel_x, y, 22.0, Color::new(1.0, 1.0, 1.0, 0.6));
                let sc = if k == 0 { self.bump.scale(0.25) } else { 1.0 };
                text(&v.to_string(), panel_x, y + 40.0, 40.0 * sc, WHITE);
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
        self.fx.draw();
        self.pops.draw();
        match self.state {
            State::Ready => overlay(
                view,
                "Block Drop",
                &[
                    "Drag to move, tap to turn",
                    "Swipe down to drop",
                    "Tap to start",
                ],
            ),
            State::Over if self.over_t > 0.5 => {
                let s = format!("Score {}   Lines {}", self.score, self.lines);
                overlay(view, "Stacked up!", &[&s, "Tap to play again"]);
            }
            _ => {}
        }
    }
}

pub fn icon(r: Rect) {
    draw_rectangle(r.x, r.y, r.w, r.h, Color::from_rgba(50, 45, 95, 255));
    let s = r.w / 5.0;
    for (x, y, k) in [
        (0, 4, 1),
        (1, 4, 1),
        (2, 4, 1),
        (3, 4, 4),
        (4, 4, 4),
        (0, 3, 1),
        (3, 3, 4),
        (4, 3, 2),
        (2, 3, 6),
        (4, 2, 2),
        (1, 1, 5),
        (1, 0, 5),
        (0, 1, 5),
        (2, 1, 5),
    ] {
        block(r.x + x as f32 * s, r.y + y as f32 * s, s, piece_color(k));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn empty() -> Blocks {
        let mut g = Blocks::new();
        g.state = State::Playing;
        g
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
        let mut g = empty();
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
        g.bag.clear();
        let mut seen: Vec<u8> = (0..7).map(|_| g.draw_bag()).collect();
        seen.sort();
        assert_eq!(seen, (0..7).collect::<Vec<u8>>());
    }
}
