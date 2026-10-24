use crate::engine::*;
use macroquad::prelude::*;

pub const PAIRS: usize = 8;
pub const PEEK: f32 = 0.8;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Face {
    Down,
    Up,
    Matched,
}

pub struct Memory {
    pub cards: Vec<(u8, Face)>,
    pub moves: u32,
    pub best: Option<u32>,
    pub wait: f32,
    flip: Vec<f32>,
    t: f32,
}

impl Memory {
    pub fn new() -> Self {
        let mut cards: Vec<(u8, Face)> = (0..PAIRS as u8)
            .flat_map(|i| [(i, Face::Down), (i, Face::Down)])
            .collect();
        for i in (1..cards.len()).rev() {
            cards.swap(i, rand::gen_range(0, i + 1));
        }
        Self {
            cards,
            moves: 0,
            best: None,
            wait: 0.0,
            flip: vec![0.0; PAIRS * 2],
            t: 0.0,
        }
    }

    fn up(&self) -> Vec<usize> {
        (0..self.cards.len())
            .filter(|i| self.cards[*i].1 == Face::Up)
            .collect()
    }

    pub fn won(&self) -> bool {
        self.cards.iter().all(|c| c.1 == Face::Matched)
    }

    pub fn pick(&mut self, i: usize) {
        if self.wait > 0.0 || self.cards[i].1 != Face::Down {
            return;
        }
        self.cards[i].1 = Face::Up;
        let up = self.up();
        if up.len() == 2 {
            self.moves += 1;
            if self.cards[up[0]].0 == self.cards[up[1]].0 {
                for j in up {
                    self.cards[j].1 = Face::Matched;
                }
                if self.won() {
                    self.best = Some(self.best.map_or(self.moves, |b| b.min(self.moves)));
                }
            } else {
                self.wait = PEEK;
            }
        }
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

    fn layout(view: &View) -> Vec<Rect> {
        let n = PAIRS * 2;
        let cols = if view.w >= view.h { n / 2 } else { 4 };
        let rows = n.div_ceil(cols);
        let top = 80.0;
        let size = ((view.w - 40.0) / cols as f32).min((view.h - top - 20.0) / rows as f32);
        let ox = (view.w - size * cols as f32) / 2.0;
        let oy = top + (view.h - top - 20.0 - size * rows as f32) / 2.0;
        (0..PAIRS * 2)
            .map(|i| {
                Rect::new(
                    ox + (i % cols) as f32 * size + 6.0,
                    oy + (i / cols) as f32 * size + 6.0,
                    size - 12.0,
                    size - 12.0,
                )
            })
            .collect()
    }
}

impl Default for Memory {
    fn default() -> Self {
        Self::new()
    }
}

fn draw_symbol(id: u8, c: Vec2, s: f32) {
    let col = hsv(id as f32 / PAIRS as f32, 0.7, 0.9);
    match id % 8 {
        0 => draw_circle(c.x, c.y, s, col),
        1 => draw_rectangle(c.x - s * 0.85, c.y - s * 0.85, s * 1.7, s * 1.7, col),
        2 => draw_triangle(
            vec2(c.x, c.y - s),
            vec2(c.x - s, c.y + s * 0.8),
            vec2(c.x + s, c.y + s * 0.8),
            col,
        ),
        3 => {
            draw_poly(c.x, c.y, 5, s, -90.0, col);
            draw_poly(c.x, c.y, 5, s * 0.5, -54.0, col);
            for k in 0..5 {
                let a = (-90.0 + k as f32 * 72.0f32).to_radians();
                let b0 = (-90.0 + k as f32 * 72.0 - 36.0f32).to_radians();
                let b1 = (-90.0 + k as f32 * 72.0 + 36.0f32).to_radians();
                draw_triangle(
                    c + vec2(a.cos(), a.sin()) * s * 1.1,
                    c + vec2(b0.cos(), b0.sin()) * s * 0.45,
                    c + vec2(b1.cos(), b1.sin()) * s * 0.45,
                    col,
                );
            }
        }
        4 => {
            draw_circle(c.x - s * 0.45, c.y - s * 0.2, s * 0.55, col);
            draw_circle(c.x + s * 0.45, c.y - s * 0.2, s * 0.55, col);
            draw_triangle(
                vec2(c.x - s, c.y),
                vec2(c.x + s, c.y),
                vec2(c.x, c.y + s),
                col,
            );
        }
        5 => draw_poly(c.x, c.y, 6, s, 0.0, col),
        6 => {
            draw_rectangle(c.x - s * 0.3, c.y - s, s * 0.6, s * 2.0, col);
            draw_rectangle(c.x - s, c.y - s * 0.3, s * 2.0, s * 0.6, col);
        }
        _ => {
            draw_circle(c.x, c.y, s, col);
            draw_circle(
                c.x + s * 0.4,
                c.y - s * 0.3,
                s * 0.8,
                Color::from_rgba(250, 250, 255, 255),
            );
        }
    }
}

impl Game for Memory {
    fn update(&mut self, input: &Input, view: &View, dt: f32) {
        self.t += dt;
        self.tick(dt);
        let won = self.won();
        for (i, f) in self.flip.iter_mut().enumerate() {
            let target = if self.cards[i].1 == Face::Down {
                0.0
            } else {
                1.0
            };
            *f += (target - *f).clamp(-dt * 6.0, dt * 6.0);
        }
        if won {
            if input.tap() {
                let best = self.best;
                *self = Self::new();
                self.best = best;
            }
            return;
        }
        if input.pressed {
            if let Some(i) = Self::layout(view)
                .iter()
                .position(|r| r.contains(input.pos))
            {
                self.pick(i);
            }
        }
    }

    fn draw(&self, view: &View) {
        sky(
            view,
            Color::from_rgba(120, 200, 230, 255),
            Color::from_rgba(180, 230, 210, 255),
        );
        text(
            &format!("Moves {}", self.moves),
            HUD_LEFT,
            50.0,
            40.0,
            WHITE,
        );
        if let Some(b) = self.best {
            text(&format!("Best {b}"), view.w - 160.0, 50.0, 36.0, WHITE);
        }
        for (i, r) in Self::layout(view).iter().enumerate() {
            let f = self.flip[i];
            let w = (f * 2.0 - 1.0).abs();
            let rr = Rect::new(r.x + r.w * (1.0 - w) / 2.0, r.y, r.w * w, r.h);
            round_rect(
                Rect::new(rr.x, rr.y + 5.0, rr.w, rr.h),
                14.0,
                Color::from_rgba(0, 0, 0, 40),
            );
            if f < 0.5 {
                round_rect(rr, 14.0, Color::from_rgba(90, 110, 220, 255));
                round_rect(
                    Rect::new(
                        rr.x + 8.0 * w,
                        rr.y + 8.0,
                        (rr.w - 16.0 * w).max(0.0),
                        rr.h - 16.0,
                    ),
                    10.0,
                    Color::from_rgba(110, 135, 240, 255),
                );
                if w > 0.3 {
                    text_center(
                        "?",
                        rr.x + rr.w / 2.0,
                        rr.y + rr.h / 2.0,
                        r.h * 0.4,
                        with_alpha(WHITE, 0.7),
                    );
                }
            } else {
                let matched = self.cards[i].1 == Face::Matched;
                round_rect(
                    rr,
                    14.0,
                    if matched {
                        Color::from_rgba(230, 255, 230, 255)
                    } else {
                        WHITE
                    },
                );
                if w > 0.2 {
                    let s = r.w * 0.28 * w;
                    let bounce = if matched {
                        (self.t * 4.0 + i as f32).sin() * 3.0
                    } else {
                        0.0
                    };
                    draw_symbol(
                        self.cards[i].0,
                        vec2(rr.x + rr.w / 2.0, rr.y + rr.h / 2.0 + bounce),
                        s,
                    );
                }
            }
        }
        if self.won() {
            let s = format!("Done in {} moves", self.moves);
            overlay(view, "All matched!", &[&s, "Tap to shuffle again"]);
        }
    }
}

pub fn icon(r: Rect) {
    draw_rectangle(r.x, r.y, r.w, r.h, Color::from_rgba(150, 215, 220, 255));
    let w = r.w * 0.4;
    draw_rectangle(
        r.x + r.w * 0.07,
        r.y + r.h * 0.15,
        w,
        r.h * 0.7,
        Color::from_rgba(90, 110, 220, 255),
    );
    draw_rectangle(r.x + r.w * 0.53, r.y + r.h * 0.15, w, r.h * 0.7, WHITE);
    draw_symbol(4, vec2(r.x + r.w * 0.73, r.y + r.h * 0.48), r.w * 0.12);
    text_center("?", r.x + r.w * 0.27, r.y + r.h * 0.5, r.h * 0.35, WHITE);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pair_positions(m: &Memory, id: u8) -> (usize, usize) {
        let v: Vec<usize> = (0..m.cards.len()).filter(|i| m.cards[*i].0 == id).collect();
        (v[0], v[1])
    }

    #[test]
    fn deck_has_two_of_each() {
        let m = Memory::new();
        for id in 0..PAIRS as u8 {
            assert_eq!(m.cards.iter().filter(|c| c.0 == id).count(), 2);
        }
    }

    #[test]
    fn matching_pair_stays_up() {
        let mut m = Memory::new();
        let (a, b) = pair_positions(&m, 3);
        m.pick(a);
        m.pick(b);
        assert_eq!(m.cards[a].1, Face::Matched);
        assert_eq!(m.moves, 1);
    }

    #[test]
    fn mismatch_flips_back_and_blocks_third_pick() {
        let mut m = Memory::new();
        let (a, _) = pair_positions(&m, 0);
        let (b, c) = pair_positions(&m, 1);
        m.pick(a);
        m.pick(b);
        m.pick(c);
        assert_eq!(m.cards[c].1, Face::Down);
        m.tick(PEEK + 0.01);
        assert_eq!(m.cards[a].1, Face::Down);
        assert_eq!(m.cards[b].1, Face::Down);
    }

    #[test]
    fn clearing_all_pairs_wins_and_records_best() {
        let mut m = Memory::new();
        for id in 0..PAIRS as u8 {
            let (a, b) = pair_positions(&m, id);
            m.pick(a);
            m.pick(b);
        }
        assert!(m.won());
        assert_eq!(m.best, Some(PAIRS as u32));
    }

    #[test]
    fn every_card_fits_on_screen() {
        for v in [
            View::ipad_landscape(),
            View::ipad_portrait(),
            View::fit(1920.0, 1080.0),
        ] {
            let rects = Memory::layout(&v);
            assert_eq!(rects.len(), PAIRS * 2);
            for r in rects {
                assert!(
                    r.x >= 0.0 && r.y >= 0.0 && r.x + r.w <= v.w && r.y + r.h <= v.h,
                    "{r:?} outside {v:?}"
                );
            }
        }
    }
}
