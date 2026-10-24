use crate::engine::*;
use macroquad::prelude::*;

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

fn line_cells(dir: Dir, line: usize) -> [(usize, usize); N] {
    std::array::from_fn(|k| match dir {
        Dir::Left => (line, k),
        Dir::Right => (line, N - 1 - k),
        Dir::Up => (k, line),
        Dir::Down => (N - 1 - k, line),
    })
}

pub fn shift(board: &Board, dir: Dir) -> (Board, u32) {
    let mut out = [[0; N]; N];
    let mut gained = 0;
    for line in 0..N {
        let cells = line_cells(dir, line);
        let (row, g) = slide_row(cells.map(|(r, c)| board[r][c]));
        gained += g;
        for (k, (r, c)) in cells.iter().enumerate() {
            out[*r][*c] = row[k];
        }
    }
    (out, gained)
}

pub fn can_move(board: &Board) -> bool {
    Dir::ALL.iter().any(|d| shift(board, *d).0 != *board)
}

pub struct Merge {
    pub board: Board,
    pub score: u32,
    pub best: Best,
    pub won_shown: bool,
    pub over: bool,
    pop: [[f32; N]; N],
}

impl Merge {
    pub fn new() -> Self {
        let mut g = Self {
            board: [[0; N]; N],
            score: 0,
            best: Best::default(),
            won_shown: false,
            over: false,
            pop: [[0.0; N]; N],
        };
        g.spawn();
        g.spawn();
        g
    }

    fn spawn(&mut self) {
        let empty: Vec<(usize, usize)> = (0..N)
            .flat_map(|r| (0..N).map(move |c| (r, c)))
            .filter(|(r, c)| self.board[*r][*c] == 0)
            .collect();
        if empty.is_empty() {
            return;
        }
        let (r, c) = empty[rand::gen_range(0, empty.len())];
        self.board[r][c] = if rand::gen_range(0, 10) == 0 { 4 } else { 2 };
        self.pop[r][c] = 1.0;
    }

    pub fn play(&mut self, dir: Dir) -> bool {
        let (next, gained) = shift(&self.board, dir);
        if next == self.board {
            return false;
        }
        self.board = next;
        self.score += gained;
        self.best.offer(self.score);
        self.spawn();
        self.over = !can_move(&self.board);
        true
    }

    pub fn max_tile(&self) -> u32 {
        self.board.iter().flatten().copied().max().unwrap_or(0)
    }
}

impl Default for Merge {
    fn default() -> Self {
        Self::new()
    }
}

fn tile_color(v: u32) -> (Color, Color) {
    let k = (v.max(2) as f32).log2();
    let bg = match v {
        2 => Color::from_rgba(238, 228, 218, 255),
        4 => Color::from_rgba(237, 224, 200, 255),
        _ => hsv(0.1 - k * 0.07, 0.65, 0.97),
    };
    (
        bg,
        if v <= 4 {
            Color::from_rgba(119, 110, 101, 255)
        } else {
            WHITE
        },
    )
}

impl Game for Merge {
    fn update(&mut self, input: &Input, _view: &View, dt: f32) {
        for p in self.pop.iter_mut().flatten() {
            *p = (*p - dt * 4.0).max(0.0);
        }
        if self.over || (self.max_tile() >= 2048 && !self.won_shown) {
            if input.tap() {
                if self.over {
                    let best = self.best;
                    *self = Self::new();
                    self.best = best;
                } else {
                    self.won_shown = true;
                }
            }
            return;
        }
        if let Some(d) = input.dir() {
            self.play(d);
        }
    }

    fn draw(&self, view: &View) {
        clear_background(Color::from_rgba(250, 248, 239, 255));
        let size = (view.h - 150.0).min(view.w - 40.0);
        let o = vec2(
            (view.w - size) / 2.0,
            120.0 + ((view.h - 150.0 - size) / 2.0).max(0.0),
        );
        text_center(
            "2048",
            view.w / 2.0,
            o.y - 64.0,
            64.0,
            Color::from_rgba(119, 110, 101, 255),
        );
        for (i, (label, v)) in [("SCORE", self.score), ("BEST", self.best.0)]
            .iter()
            .enumerate()
        {
            let r = Rect::new(
                if i == 0 {
                    o.x.max(HUD_LEFT)
                } else {
                    o.x + size - 160.0
                },
                o.y - 100.0,
                160.0,
                80.0,
            );
            round_rect(r, 10.0, Color::from_rgba(187, 173, 160, 255));
            text_center(
                label,
                r.x + r.w / 2.0,
                r.y + 22.0,
                22.0,
                Color::from_rgba(238, 228, 218, 255),
            );
            text_center(&v.to_string(), r.x + r.w / 2.0, r.y + 54.0, 36.0, WHITE);
        }
        round_rect(
            Rect::new(o.x, o.y, size, size),
            16.0,
            Color::from_rgba(187, 173, 160, 255),
        );
        let gap = size * 0.03;
        let cs = (size - gap * (N as f32 + 1.0)) / N as f32;
        for r in 0..N {
            for c in 0..N {
                let x = o.x + gap + c as f32 * (cs + gap);
                let y = o.y + gap + r as f32 * (cs + gap);
                round_rect(
                    Rect::new(x, y, cs, cs),
                    10.0,
                    Color::from_rgba(205, 193, 180, 255),
                );
                let v = self.board[r][c];
                if v == 0 {
                    continue;
                }
                let p = self.pop[r][c];
                let s = if p > 0.6 {
                    1.0 - (p - 0.6) * 2.0
                } else {
                    1.0 + p * 0.15
                };
                let s = s.max(0.2);
                let (bg, fg) = tile_color(v);
                let d = cs * (1.0 - s) / 2.0;
                round_rect(Rect::new(x + d, y + d, cs * s, cs * s), 10.0, bg);
                let label = v.to_string();
                let fs = cs
                    * s
                    * if label.len() <= 2 {
                        0.5
                    } else if label.len() == 3 {
                        0.4
                    } else {
                        0.32
                    };
                text_center(&label, x + cs / 2.0, y + cs / 2.0, fs, fg);
            }
        }
        if self.over {
            let s = format!("Score {}", self.score);
            overlay(view, "No more moves", &[&s, "Tap to start over"]);
        } else if self.max_tile() >= 2048 && !self.won_shown {
            overlay(view, "2048!", &["You did it!", "Tap to keep going"]);
        }
    }
}

pub fn icon(r: Rect) {
    draw_rectangle(r.x, r.y, r.w, r.h, Color::from_rgba(187, 173, 160, 255));
    let g = r.w * 0.06;
    let cs = (r.w - g * 3.0) / 2.0;
    for (i, v) in [2u32, 4, 8, 2048].iter().enumerate() {
        let x = r.x + g + (i % 2) as f32 * (cs + g);
        let y = r.y + g + (i / 2) as f32 * (cs + g);
        let (bg, fg) = tile_color(*v);
        draw_rectangle(x, y, cs, cs, bg);
        text_center(
            &v.to_string(),
            x + cs / 2.0,
            y + cs / 2.0,
            cs * if *v > 999 { 0.32 } else { 0.55 },
            fg,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
        let (r, g) = shift(&b, Dir::Right);
        assert_eq!(r[0], [0, 0, 0, 4]);
        assert_eq!(g, 4);
        let (d, g) = shift(&b, Dir::Down);
        assert_eq!(d[3], [4, 2, 0, 0]);
        assert_eq!(g, 4);
    }

    #[test]
    fn useless_move_does_not_spawn() {
        let mut g = Merge::new();
        g.board = [[2, 0, 0, 0], [0; 4], [0; 4], [0; 4]];
        assert!(!g.play(Dir::Left));
        assert!(!g.play(Dir::Up));
        assert_eq!(g.board.iter().flatten().filter(|v| **v > 0).count(), 1);
    }

    #[test]
    fn stuck_board_detected() {
        let b = [[2, 4, 2, 4], [4, 2, 4, 2], [2, 4, 2, 4], [4, 2, 4, 2]];
        assert!(!can_move(&b));
        let mut b2 = b;
        b2[0][0] = 4;
        assert!(can_move(&b2));
    }
}
