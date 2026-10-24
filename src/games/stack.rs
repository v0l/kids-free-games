use crate::engine::*;
use macroquad::prelude::*;

pub const BLOCK_H: f32 = 40.0;
pub const START_W: f32 = 320.0;
pub const PERFECT: f32 = 6.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Block {
    pub x: f32,
    pub w: f32,
    pub hue: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum State {
    Playing,
    Over,
}

struct Chip {
    rect: Rect,
    vy: f32,
    color: Color,
}

pub struct Stack {
    pub tower: Vec<Block>,
    pub moving: Block,
    pub dir: f32,
    pub state: State,
    pub score: u32,
    pub best: Best,
    pub streak: u32,
    chips: Vec<Chip>,
    cam: f32,
    center: f32,
    flash: f32,
}

pub enum Drop {
    Perfect,
    Trimmed(f32),
    Missed,
}

pub fn place(base: Block, mov: Block) -> (Drop, Option<Block>) {
    let off = mov.x - base.x;
    if off.abs() <= PERFECT {
        return (
            Drop::Perfect,
            Some(Block {
                x: base.x,
                w: base.w,
                hue: mov.hue,
            }),
        );
    }
    let left = mov.x.max(base.x);
    let right = (mov.x + mov.w).min(base.x + base.w);
    if right <= left {
        return (Drop::Missed, None);
    }
    (
        Drop::Trimmed(mov.w - (right - left)),
        Some(Block {
            x: left,
            w: right - left,
            hue: mov.hue,
        }),
    )
}

impl Stack {
    pub fn new() -> Self {
        let base = Block {
            x: -START_W / 2.0,
            w: START_W,
            hue: 0.55,
        };
        let mut s = Self {
            tower: vec![base],
            moving: base,
            dir: 1.0,
            state: State::Playing,
            score: 0,
            best: Best::default(),
            streak: 0,
            chips: vec![],
            cam: 0.0,
            center: 0.0,
            flash: 0.0,
        };
        s.next_block();
        s
    }

    fn next_block(&mut self) {
        let top = *self.tower.last().unwrap();
        self.dir = if self.tower.len().is_multiple_of(2) {
            1.0
        } else {
            -1.0
        };
        let start = if self.dir > 0.0 {
            -420.0
        } else {
            420.0 - top.w
        };
        self.moving = Block {
            x: start,
            w: top.w,
            hue: top.hue + 0.035,
        };
    }

    pub fn speed(&self) -> f32 {
        260.0 + self.score as f32 * 8.0
    }

    pub fn drop_block(&mut self) {
        let base = *self.tower.last().unwrap();
        let (res, block) = place(base, self.moving);
        match res {
            Drop::Missed => {
                self.chips.push(Chip {
                    rect: Rect::new(self.moving.x, 0.0, self.moving.w, BLOCK_H),
                    vy: 0.0,
                    color: hsv(self.moving.hue, 0.5, 0.95),
                });
                self.state = State::Over;
                self.best.offer(self.score);
                return;
            }
            Drop::Perfect => {
                self.streak += 1;
                self.flash = 1.0;
            }
            Drop::Trimmed(cut) => {
                self.streak = 0;
                let b = block.unwrap();
                let cx = if self.moving.x < b.x {
                    self.moving.x
                } else {
                    b.x + b.w
                };
                self.chips.push(Chip {
                    rect: Rect::new(cx, 0.0, cut, BLOCK_H),
                    vy: 0.0,
                    color: hsv(self.moving.hue, 0.5, 0.95),
                });
            }
        }
        let mut b = block.unwrap();
        if self.streak >= 3 {
            let grow = (b.w + 16.0).min(START_W) - b.w;
            b.x -= grow / 2.0;
            b.w += grow;
        }
        self.tower.push(b);
        self.score += 1;
        self.next_block();
    }
}

impl Default for Stack {
    fn default() -> Self {
        Self::new()
    }
}

impl Game for Stack {
    fn update(&mut self, input: &Input, _view: &View, dt: f32) {
        self.flash = (self.flash - dt * 3.0).max(0.0);
        let level = self.tower.len() as f32 * BLOCK_H;
        let target = (level - 200.0).max(0.0);
        self.cam += (target - self.cam) * (dt * 5.0).min(1.0);
        let top = self.tower.last().unwrap();
        self.center += (top.x + top.w / 2.0 - self.center) * (dt * 3.0).min(1.0);
        for c in &mut self.chips {
            c.vy += 1400.0 * dt;
            c.rect.y += c.vy * dt;
        }
        self.chips.retain(|c| c.rect.y < 1500.0);
        match self.state {
            State::Playing => {
                let speed = self.speed();
                self.moving.x += self.dir * speed * dt;
                if self.moving.x > 420.0 - self.moving.w {
                    self.dir = -1.0;
                }
                if self.moving.x < -420.0 {
                    self.dir = 1.0;
                }
                if input.tap() {
                    self.drop_block();
                }
            }
            State::Over => {
                if input.tap() && self.chips.is_empty() {
                    let best = self.best;
                    *self = Self::new();
                    self.best = best;
                }
            }
        }
    }

    fn draw(&self, view: &View) {
        let hue = self.tower.last().unwrap().hue;
        sky(view, hsv(hue + 0.5, 0.35, 0.95), hsv(hue + 0.45, 0.5, 0.7));
        let ground = view.h - 80.0;
        let ox = view.w / 2.0 - self.center * 0.5;
        let y_of = |i: usize| ground - (i as f32 + 1.0) * BLOCK_H + self.cam;
        let draw_block = |b: Block, y: f32| {
            let c = hsv(b.hue, 0.5, 0.95);
            draw_rectangle(ox + b.x, y, b.w, BLOCK_H, c);
            draw_rectangle(ox + b.x, y, b.w, 6.0, with_alpha(WHITE, 0.35));
            draw_rectangle(ox + b.x, y + BLOCK_H - 6.0, b.w, 6.0, darker(c, 0.8));
        };
        for (i, b) in self.tower.iter().enumerate() {
            let y = y_of(i);
            if y > view.h + BLOCK_H || y < -BLOCK_H {
                continue;
            }
            draw_block(*b, y);
        }
        let my = y_of(self.tower.len());
        if self.state == State::Playing {
            draw_block(self.moving, my);
        }
        for c in &self.chips {
            draw_rectangle(ox + c.rect.x, my + c.rect.y, c.rect.w, c.rect.h, c.color);
        }
        if self.flash > 0.0 {
            let t = self.tower.last().unwrap();
            let y = y_of(self.tower.len() - 1);
            let g = 12.0 * (1.0 - self.flash);
            draw_rectangle_lines(
                ox + t.x - g,
                y - g,
                t.w + g * 2.0,
                BLOCK_H + g * 2.0,
                4.0,
                with_alpha(WHITE, self.flash),
            );
        }
        text_shadow(&self.score.to_string(), view.w / 2.0, 90.0, 96.0, WHITE);
        if self.streak >= 2 {
            text_shadow(
                &format!("Perfect x{}", self.streak),
                view.w / 2.0,
                160.0,
                36.0,
                WHITE,
            );
        }
        if self.score == 0 && self.state == State::Playing {
            text_shadow("Tap to drop the block", view.w / 2.0, 220.0, 40.0, WHITE);
        }
        if self.state == State::Over && self.chips.is_empty() {
            let s = format!("Height {}   Best {}", self.score, self.best.0);
            overlay(view, "Tower done!", &[&s, "Tap to build again"]);
        }
    }
}

pub fn icon(r: Rect) {
    draw_rectangle(r.x, r.y, r.w, r.h, hsv(0.05, 0.3, 0.95));
    for i in 0..6 {
        let w = r.w * (0.7 - i as f32 * 0.05);
        let x = r.x + (r.w - w) / 2.0 + (i % 2) as f32 * 3.0;
        draw_rectangle(
            x,
            r.y + r.h - (i + 1) as f32 * r.h * 0.13,
            w,
            r.h * 0.13,
            hsv(0.55 + i as f32 * 0.035, 0.5, 0.95),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn overhang_is_cut_off() {
        let base = Block {
            x: 0.0,
            w: 100.0,
            hue: 0.0,
        };
        let (res, b) = place(
            base,
            Block {
                x: 30.0,
                w: 100.0,
                hue: 0.0,
            },
        );
        assert!(matches!(res, Drop::Trimmed(c) if (c - 30.0).abs() < 0.01));
        assert_eq!(b.unwrap().x, 30.0);
        assert_eq!(b.unwrap().w, 70.0);
    }

    #[test]
    fn near_miss_snaps_to_perfect() {
        let base = Block {
            x: 0.0,
            w: 100.0,
            hue: 0.0,
        };
        let (res, b) = place(
            base,
            Block {
                x: -4.0,
                w: 100.0,
                hue: 0.0,
            },
        );
        assert!(matches!(res, Drop::Perfect));
        assert_eq!(b.unwrap().x, 0.0);
    }

    #[test]
    fn total_miss_ends_game() {
        let mut s = Stack::new();
        s.moving.x = 400.0;
        s.drop_block();
        assert_eq!(s.state, State::Over);
        assert_eq!(s.score, 0);
    }

    #[test]
    fn perfect_streak_grows_block() {
        let mut s = Stack::new();
        s.tower.last_mut().unwrap().w = 200.0;
        s.tower.last_mut().unwrap().x = -100.0;
        for _ in 0..3 {
            s.moving = Block {
                x: s.tower.last().unwrap().x,
                w: s.tower.last().unwrap().w,
                hue: 0.0,
            };
            s.drop_block();
        }
        assert_eq!(s.score, 3);
        assert!(s.tower.last().unwrap().w > 200.0);
    }
}
