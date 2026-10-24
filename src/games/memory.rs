use crate::engine::draw_circle;
use crate::engine::*;
use crate::fx::*;
use crate::sfx::{self, Sfx};
use macroquad::prelude::*;

pub const PAIRS: usize = 8;
pub const PEEK: f32 = 0.8;

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

pub fn flip_shape(progress: f32) -> (f32, f32) {
    let k = progress.clamp(0.0, 1.0);
    let eased = k * k * (3.0 - 2.0 * k);
    let a = eased * std::f32::consts::PI;
    (a.cos().abs(), a.sin())
}

pub fn wobble_offset(wobble: f32) -> f32 {
    (wobble * 36.0).sin() * 12.0 * wobble.clamp(0.0, 1.0)
}

pub struct Memory {
    pub cards: Vec<(u8, Face)>,
    pub moves: u32,
    pub best: Option<u32>,
    pub wait: f32,
    pub events: Vec<Event>,
    flip: Vec<f32>,
    wobble: Vec<f32>,
    pop: Vec<Bump>,
    moves_bump: Bump,
    particles: Particles,
    popups: Popups,
    shake: Shake,
    celebrated: f32,
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
            events: vec![],
            flip: vec![0.0; PAIRS * 2],
            wobble: vec![0.0; PAIRS * 2],
            pop: (0..PAIRS * 2).map(|_| Bump::default()).collect(),
            moves_bump: Bump::default(),
            particles: Particles::default(),
            popups: Popups::default(),
            shake: Shake::default(),
            celebrated: -1.0,
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
                for j in &up {
                    self.cards[*j].1 = Face::Matched;
                }
                self.events.push(Event::Match(up[0], up[1]));
                if self.won() {
                    self.best = Some(self.best.map_or(self.moves, |b| b.min(self.moves)));
                    self.events.push(Event::Win);
                }
            } else {
                self.wait = PEEK;
                self.events.push(Event::Miss(up[0], up[1]));
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

const BACK: Color = Color::new(0.35, 0.43, 0.86, 1.0);
const FRAME: Color = Color::new(0.24, 0.30, 0.68, 1.0);

impl Memory {
    fn react(&mut self, view: &View) {
        let rects = Self::layout(view);
        for e in std::mem::take(&mut self.events) {
            match e {
                Event::Match(a, b) => {
                    sfx::play(Sfx::Right);
                    for i in [a, b] {
                        let c = rects[i].center();
                        let col = hsv(self.cards[i].0 as f32 / PAIRS as f32, 0.6, 1.0);
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
                    sfx::play(Sfx::Win);
                    self.celebrated = 0.0
                }
            }
        }
    }
}

impl Game for Memory {
    fn update(&mut self, input: &Input, view: &View, dt: f32) {
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
        if won {
            if input.tap() {
                let best = self.best;
                *self = Self::new();
                self.best = best;
            }
        } else if input.pressed {
            if let Some(i) = Self::layout(view)
                .iter()
                .position(|r| r.contains(input.pos))
            {
                self.pick(i);
            }
        }
        if self.moves != moves {
            self.moves_bump.kick();
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
        sky(
            view,
            Color::from_rgba(120, 200, 230, 255),
            Color::from_rgba(180, 230, 210, 255),
        );
        for k in 0..9 {
            let f = k as f32;
            let x = (f * 137.0 + self.t * (6.0 + f)) % (view.w + 80.0) - 40.0;
            let y = view.h - ((f * 91.0 + self.t * (14.0 + f * 3.0)) % (view.h + 60.0)) + 30.0;
            draw_circle(x, y, 10.0 + f * 3.0, Color::new(1.0, 1.0, 1.0, 0.12));
            draw_circle_lines(x, y, 10.0 + f * 3.0, 2.0, Color::new(1.0, 1.0, 1.0, 0.2));
        }
        text("Moves", HUD_LEFT, 50.0, 30.0, with_alpha(WHITE, 0.85));
        score_text(
            &self.moves.to_string(),
            HUD_LEFT + 118.0,
            38.0,
            44.0,
            self.moves_bump.t,
        );
        if let Some(b) = self.best {
            text_shadow(&format!("Best {b}"), view.w - 100.0, 40.0, 34.0, WHITE);
        }
        let sh = self.shake.offset();
        for (i, r) in Self::layout(view).iter().enumerate() {
            self.draw_card(i, *r, sh);
        }
        self.particles.draw();
        self.popups.draw();
        if self.won() {
            let s = format!("Done in {} moves", self.moves);
            overlay(view, "All matched!", &[&s, "Tap to shuffle again"]);
        }
    }
}

impl Memory {
    fn draw_card(&self, i: usize, r: Rect, shake: Vec2) {
        let (w, lift) = flip_shape(self.flip[i]);
        let matched = self.cards[i].1 == Face::Matched;
        let pop = self.pop[i].scale(0.18);
        let pop = if matched { pop } else { 1.0 };
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
            r.w * 0.42 * (0.4 + 0.6 * w),
            r.h * 0.07,
            0.6 - lift * 0.3,
        );
        let face_up = self.flip[i] >= 0.5;
        if !face_up {
            round_rect(
                Rect::new(rr.x, rr.y + 5.0, rr.w, rr.h),
                14.0,
                darker(FRAME, 0.7),
            );
            round_rect(rr, 14.0, FRAME);
            let inner = Rect::new(
                rr.x + 7.0 * w,
                rr.y + 7.0,
                (rr.w - 14.0 * w).max(0.0),
                rr.h - 16.0,
            );
            round_rect(inner, 10.0, BACK);
            if w > 0.25 {
                let cols = 4;
                for gy in 0..4 {
                    for gx in 0..cols {
                        let off = if gy % 2 == 0 { 0.0 } else { 0.5 };
                        let px =
                            inner.x + inner.w * (gx as f32 + 0.5 + off * 0.5 - 0.25) / cols as f32;
                        let py = inner.y + inner.h * (gy as f32 + 0.5) / 4.0;
                        if px > inner.x + 4.0 && px < inner.x + inner.w - 4.0 {
                            draw_poly(px, py, 4, 5.0 * w, 0.0, with_alpha(WHITE, 0.16));
                        }
                    }
                }
                text_center(
                    "?",
                    rr.x + rr.w / 2.0,
                    rr.y + rr.h / 2.0,
                    r.h * 0.4 * w.max(0.5),
                    with_alpha(WHITE, 0.85),
                );
                gloss(inner, 10.0);
                if w > 0.9 {
                    shine_sweep(inner, self.t + i as f32 * 0.37);
                }
            }
        } else {
            let tint = hsv(self.cards[i].0 as f32 / PAIRS as f32, 0.45, 1.0);
            let base = if matched {
                Color::from_rgba(224, 255, 226, 255)
            } else {
                WHITE
            };
            round_rect(
                Rect::new(rr.x, rr.y + 5.0, rr.w, rr.h),
                14.0,
                darker(tint, 0.6),
            );
            round_rect(rr, 14.0, tint);
            round_rect(
                Rect::new(
                    rr.x + 5.0 * w,
                    rr.y + 5.0,
                    (rr.w - 10.0 * w).max(0.0),
                    rr.h - 10.0,
                ),
                11.0,
                base,
            );
            if w > 0.2 {
                let s = r.w * 0.28 * w;
                let bounce = if matched {
                    (self.t * 4.0 + i as f32).sin() * 3.0
                } else {
                    0.0
                };
                let sc = vec2(rr.x + rr.w / 2.0, rr.y + rr.h / 2.0 + bounce);
                draw_circle(sc.x, sc.y + s * 0.15, s * 1.25, with_alpha(tint, 0.25));
                draw_symbol(self.cards[i].0, sc, s);
                if matched {
                    shine_sweep(rr, self.t * 1.5 + i as f32 * 0.2);
                }
            }
            gloss(rr, 14.0);
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
        let mut m = Memory::new();
        let (a, b) = pair_positions(&m, 0);
        let (c, _) = pair_positions(&m, 1);
        m.pick(a);
        m.pick(c);
        assert_eq!(m.events, vec![Event::Miss(a.min(c), a.max(c))]);
        m.tick(PEEK + 0.01);
        m.events.clear();
        m.pick(a);
        m.pick(b);
        assert_eq!(m.events, vec![Event::Match(a.min(b), a.max(b))]);
        m.events.clear();
        for id in 1..PAIRS as u8 {
            let (x, y) = pair_positions(&m, id);
            m.pick(x);
            m.pick(y);
        }
        assert_eq!(m.events.last(), Some(&Event::Win));
    }

    #[test]
    fn mismatch_shakes_and_match_sparkles() {
        let v = View::ipad_landscape();
        let mut m = Memory::new();
        let (a, b) = pair_positions(&m, 0);
        let (c, _) = pair_positions(&m, 1);
        m.pick(a);
        m.pick(c);
        m.update(&Input::default(), &v, 1.0 / 60.0);
        assert!(m.shake.active());
        assert_eq!(m.wobble[a], 1.0);
        m.tick(PEEK + 0.01);
        m.pick(a);
        m.pick(b);
        m.update(&Input::default(), &v, 1.0 / 60.0);
        assert!(!m.particles.list.is_empty());
        assert_eq!(m.popups.list.len(), 1);
    }

    #[test]
    fn winning_rains_confetti_once_per_wave() {
        let v = View::ipad_landscape();
        let mut m = Memory::new();
        for id in 0..PAIRS as u8 {
            let (a, b) = pair_positions(&m, id);
            m.pick(a);
            m.pick(b);
        }
        m.update(&Input::default(), &v, 1.0 / 60.0);
        assert!(m.particles.list.iter().any(|p| p.shape == Shape::Confetti));
        assert!(m.celebrated >= 0.0);
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
