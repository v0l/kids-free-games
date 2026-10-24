use crate::engine::*;
use crate::feel::{self, Impact};
use crate::fx::*;
use crate::sfx::{self, Sfx};
use macroquad::prelude::*;

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

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TileMove {
    pub from: (usize, usize),
    pub to: (usize, usize),
    pub value: u32,
    pub merged: bool,
}

pub fn plan(board: &Board, dir: Dir) -> Vec<TileMove> {
    let mut moves = vec![];
    for line in 0..N {
        let cells = line_cells(dir, line);
        let tiles: Vec<(usize, u32)> = (0..N)
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

pub fn can_move(board: &Board) -> bool {
    Dir::ALL.iter().any(|d| shift(board, *d).0 != *board)
}

mod art {
    crate::sprites!("merge");
}

pub struct Merge {
    pub board: Board,
    pub score: u32,
    pub best: Best,
    pub won_shown: bool,
    pub over: bool,
    pub record_broken: bool,
    slides: Vec<TileMove>,
    slide_t: f32,
    born: [[f32; N]; N],
    merged_at: [[f32; N]; N],
    reveal: Vec<(usize, usize, u32)>,
    particles: Particles,
    popups: Popups,
    shake: Shake,
    score_bump: Bump,
    t: f32,
}

impl Merge {
    pub fn new() -> Self {
        let mut g = Self {
            board: [[0; N]; N],
            score: 0,
            best: Best::default(),
            won_shown: false,
            over: false,
            record_broken: false,
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
        g.spawn();
        g.spawn();
        g
    }

    fn spawn(&mut self) -> Option<(usize, usize)> {
        let empty: Vec<(usize, usize)> = (0..N)
            .flat_map(|r| (0..N).map(move |c| (r, c)))
            .filter(|(r, c)| self.board[*r][*c] == 0)
            .collect();
        if empty.is_empty() {
            return None;
        }
        let (r, c) = empty[rand::gen_range(0, empty.len())];
        self.board[r][c] = if rand::gen_range(0, 10) == 0 { 4 } else { 2 };
        self.born[r][c] = 0.0;
        Some((r, c))
    }

    fn sliding(&self) -> bool {
        self.slide_t < SLIDE
    }

    pub fn play(&mut self, dir: Dir) -> bool {
        let (next, gained) = shift(&self.board, dir);
        if next == self.board {
            return false;
        }
        self.slides = plan(&self.board, dir);
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

pub fn tile_tint(v: u32) -> Color {
    let k = v.max(2).ilog2() as usize;
    const HUES: [f32; 11] = [
        0.58, 0.5, 0.38, 0.25, 0.15, 0.09, 0.0, 0.92, 0.83, 0.75, 0.68,
    ];
    hsv(HUES[(k - 1).min(HUES.len() - 1)], 0.55, 1.0)
}

struct Grid {
    origin: Vec2,
    size: f32,
    gap: f32,
    cell: f32,
}

impl Grid {
    fn new(view: &View) -> Self {
        let size = (view.h - 150.0).min(view.w - 40.0);
        let origin = vec2(
            (view.w - size) / 2.0,
            120.0 + ((view.h - 150.0 - size) / 2.0).max(0.0),
        );
        let gap = size * 0.03;
        Self {
            origin,
            size,
            gap,
            cell: (size - gap * (N as f32 + 1.0)) / N as f32,
        }
    }

    fn pos(&self, r: f32, c: f32) -> Vec2 {
        self.origin
            + vec2(
                self.gap + c * (self.cell + self.gap),
                self.gap + r * (self.cell + self.gap),
            )
    }

    fn center(&self, r: usize, c: usize) -> Vec2 {
        self.pos(r as f32, c as f32) + Vec2::splat(self.cell / 2.0)
    }
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

fn draw_tile(center: Vec2, cs: f32, scale: f32, v: u32, t: f32) {
    let tint = tile_tint(v);
    let s = cs * scale;
    let r = Rect::new(center.x - s / 2.0, center.y - s / 2.0, s, s);
    if v >= 128 {
        let glow = 0.35 + 0.15 * (t * 4.0).sin();
        art::sprite_ex(
            "tile",
            Rect::new(r.x - 6.0, r.y - 6.0, r.w + 12.0, r.h + 12.0),
            crate::art::Draw {
                tint: with_alpha(Color::new(1.0, 0.95, 0.5, 1.0), glow),
                ..Default::default()
            },
        );
    }
    art::sprite_ex(
        "tile",
        Rect::new(r.x, r.y + 5.0 * scale, r.w, r.h),
        crate::art::Draw {
            tint: Color::new(0.0, 0.0, 0.0, 0.22),
            ..Default::default()
        },
    );
    art::sprite_ex(
        "tile",
        r,
        crate::art::Draw {
            tint,
            ..Default::default()
        },
    );
    if v >= 2048 {
        let bob = (t * 3.0).sin() * s * 0.02;
        art::sprite_at(
            "crown",
            vec2(r.center().x, r.y + s * 0.1 + bob),
            s * 0.32,
            Default::default(),
        );
    }
    let label = v.to_string();
    let fs = s * match label.len() {
        0..=2 => 0.5,
        3 => 0.4,
        _ => 0.32,
    };
    text_outline(
        &label,
        center.x,
        center.y + s * 0.03,
        fs,
        WHITE,
        darker(tint, 0.45),
    );
}

fn draw_backdrop(view: &View, t: f32) {
    let f = view.full();
    let side = f.w.max(f.h);
    art::sprite(
        "bg",
        Rect::new(f.x + (f.w - side) / 2.0, f.y + f.h - side, side, side),
    );
    for (i, (x, y, w)) in [(0.15, 0.1, 190.0), (0.62, 0.2, 240.0), (0.9, 0.06, 160.0)]
        .iter()
        .enumerate()
    {
        let drift = (t * (6.0 + i as f32 * 3.0) + i as f32 * 300.0) % (view.w + 400.0) - 200.0;
        let cx = (view.w * x + drift).rem_euclid(view.w + 400.0) - 200.0;
        crate::ui::cloud(
            i * 3 + 1,
            vec2(cx, view.h * y + 90.0),
            *w,
            with_alpha(WHITE, 0.85),
        );
    }
}

impl Game for Merge {
    fn update(&mut self, input: &Input, view: &View, dt: f32) {
        self.t += dt;
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
        self.score_bump.update(dt);
        self.particles.update(dt);
        self.popups.update(dt);
        self.shake.update(dt);
        if self.over || (self.max_tile() >= 2048 && !self.won_shown) {
            if input.tap() {
                if self.over {
                    let best = self.best;
                    *self = Self::new();
                    self.best = best;
                    sfx::say(sfx::Say::Go);
                } else {
                    self.won_shown = true;
                }
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

    fn draw(&self, view: &View) {
        let g = Grid::new(view);
        let (o, size) = (g.origin, g.size);
        draw_backdrop(view, self.t);
        let sh = self.shake.offset();
        text_outline(
            "2048",
            view.w / 2.0,
            o.y - 64.0,
            64.0 * (1.0 + 0.02 * (self.t * 2.0).sin()),
            Color::from_rgba(255, 215, 90, 255),
            Color::from_rgba(119, 90, 60, 255),
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
            crate::ui::chip_box(r);
            text_center(label, r.x + r.w / 2.0, r.y + 22.0, 22.0, crate::ui::BODY);
            if i == 0 {
                score_text(
                    &v.to_string(),
                    r.x + r.w / 2.0,
                    r.y + 54.0,
                    36.0,
                    self.score_bump.t,
                );
            } else {
                text_outline(
                    &v.to_string(),
                    r.x + r.w / 2.0,
                    r.y + 54.0,
                    36.0,
                    Color::from_rgba(255, 200, 40, 255),
                    Color::from_rgba(40, 30, 60, 255),
                );
            }
        }
        let frame = Rect::new(o.x + sh.x, o.y + sh.y, size, size);
        art::sprite_ex(
            "tile",
            Rect::new(frame.x, frame.y + 8.0, frame.w, frame.h),
            crate::art::Draw {
                tint: Color::new(0.0, 0.0, 0.0, 0.25),
                ..Default::default()
            },
        );
        art::nine(
            "tile",
            frame,
            14.0,
            (frame.w / 128.0).max(0.5),
            Color::from_rgba(120, 92, 80, 255),
        );
        for r in 0..N {
            for c in 0..N {
                let p = g.pos(r as f32, c as f32) + sh;
                art::sprite_ex(
                    "tile",
                    Rect::new(p.x, p.y, g.cell, g.cell),
                    crate::art::Draw {
                        tint: Color::from_rgba(80, 60, 55, 120),
                        ..Default::default()
                    },
                );
            }
        }
        if self.sliding() {
            let k = ease_out_cubic(self.slide_t / SLIDE);
            for m in &self.slides {
                let from = g.pos(m.from.0 as f32, m.from.1 as f32);
                let to = g.pos(m.to.0 as f32, m.to.1 as f32);
                let p = from + (to - from) * k + sh + Vec2::splat(g.cell / 2.0);
                draw_tile(p, g.cell, 1.0, m.value, self.t);
            }
        } else {
            for r in 0..N {
                for c in 0..N {
                    let v = self.board[r][c];
                    if v == 0 {
                        continue;
                    }
                    let s =
                        born_scale(self.born[r][c]).max(0.01) * merge_scale(self.merged_at[r][c]);
                    draw_tile(g.center(r, c) + sh, g.cell, s, v, self.t);
                }
            }
        }
        self.particles.draw();
        self.popups.draw();
        if self.over {
            let s = format!("Score {}", self.score);
            overlay(view, "No more moves", &[&s, "Tap to start over"]);
        } else if self.max_tile() >= 2048 && !self.won_shown {
            overlay(view, "2048!", &["You did it!", "Tap to keep going"]);
        }
    }
}

impl Merge {
    fn land(&mut self, view: &View) {
        let g = Grid::new(view);
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
            if v >= 2048 {
                sfx::play(Sfx::Win);
                sfx::say(sfx::Say::Win);
                self.particles.confetti(view);
            }
        }
        if self.over {
            sfx::play(Sfx::Lose);
            sfx::say(if self.record_broken {
                sfx::Say::Best
            } else {
                sfx::Say::GameOver
            });
            self.shake.add(0.5);
        }
    }
}

pub fn icon(r: Rect) {
    art::sprite("bg", r);
    let g = r.w * 0.05;
    let frame = Rect::new(r.x + g, r.y + g * 1.5, r.w - 2.0 * g, r.w - 2.0 * g);
    art::nine(
        "tile",
        frame,
        14.0,
        frame.w / 300.0,
        Color::from_rgba(120, 92, 80, 255),
    );
    let pad = g * 0.7;
    let cs = (frame.w - pad * 3.0) / 2.0;
    for (i, v) in [2u32, 4, 8, 2048].iter().enumerate() {
        let x = frame.x + pad + (i % 2) as f32 * (cs + pad);
        let y = frame.y + pad + (i / 2) as f32 * (cs + pad);
        draw_tile(vec2(x + cs / 2.0, y + cs / 2.0), cs, 1.0, *v, 0.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merge_sprites_all_exist() {
        let have = art::names();
        for n in ["bg", "tile", "crown"] {
            assert!(have.iter().any(|h| h == n), "missing {n}");
        }
    }

    #[test]
    fn every_tile_value_has_its_own_colour() {
        let mut seen: Vec<[u8; 4]> = vec![];
        for k in 1..=11 {
            let c: [u8; 4] = tile_tint(1 << k).into();
            assert!(!seen.contains(&c), "2^{k} repeats a colour");
            seen.push(c);
        }
    }

    #[test]
    fn game_over_loses_and_calls_it() {
        let v = View::ipad_landscape();
        let mut g = Merge::new();
        g.board = [[2, 4, 2, 4], [4, 2, 4, 2], [2, 4, 2, 4], [4, 2, 0, 2]];
        g.best = Best(4);
        g.score = 100;
        g.board[3][2] = 8;
        crate::sfx::take_pending();
        g.over = true;
        g.land(&v);
        assert!(crate::sfx::take_pending().contains(&Sfx::Lose));
        assert_eq!(crate::sfx::take_voice(), Some(sfx::Say::GameOver));
    }

    #[test]
    fn beating_a_standing_best_is_remembered() {
        let mut g = Merge::new();
        g.best = Best(4);
        g.board = [[4, 4, 0, 0], [0; 4], [0; 4], [0; 4]];
        assert!(g.play(Dir::Left));
        assert!(g.record_broken);
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
    fn plan_tracks_every_tile_and_merges() {
        let b = [[2, 2, 4, 0], [0, 0, 0, 0], [0, 2, 0, 2], [8, 0, 0, 0]];
        let moves = plan(&b, Dir::Left);
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
        for d in Dir::ALL {
            let (next, _) = shift(&b, d);
            let mut rebuilt = [[0u32; N]; N];
            for m in plan(&b, d) {
                rebuilt[m.to.0][m.to.1] += m.value;
            }
            assert_eq!(rebuilt, next, "{d:?}");
        }
    }

    #[test]
    fn move_starts_a_slide_that_finishes_and_pops() {
        let v = View::ipad_landscape();
        let mut g = Merge::new();
        g.board = [[2, 2, 0, 0], [0; 4], [0; 4], [0; 4]];
        assert!(g.play(Dir::Left));
        assert!(g.sliding());
        assert_eq!(g.reveal.len(), 1);
        g.update(&Input::default(), &v, SLIDE + 0.01);
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
    fn stuck_board_detected() {
        let b = [[2, 4, 2, 4], [4, 2, 4, 2], [2, 4, 2, 4], [4, 2, 4, 2]];
        assert!(!can_move(&b));
        let mut b2 = b;
        b2[0][0] = 4;
        assert!(can_move(&b2));
    }
}
