use crate::engine::*;
use macroquad::prelude::*;
use std::collections::VecDeque;

pub const LONG: i32 = 24;
pub const SHORT: i32 = 18;
pub const STEP: f32 = 0.12;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum State {
    Ready,
    Playing,
    Dead,
}

pub struct Snake {
    pub body: VecDeque<IVec2>,
    pub dir: Dir,
    pub queued: VecDeque<Dir>,
    pub food: IVec2,
    pub state: State,
    pub score: u32,
    pub best: Best,
    pub wrap: bool,
    pub size: IVec2,
    timer: f32,
}

impl Snake {
    pub fn new() -> Self {
        Self::sized(ivec2(LONG, SHORT))
    }

    pub fn sized(size: IVec2) -> Self {
        let body: VecDeque<IVec2> = (0..4).map(|i| ivec2(size.x / 3 - i, size.y / 2)).collect();
        let mut s = Self {
            body,
            dir: Dir::Right,
            queued: VecDeque::new(),
            food: IVec2::ZERO,
            state: State::Ready,
            score: 0,
            best: Best::default(),
            wrap: true,
            size,
            timer: 0.0,
        };
        s.place_food();
        s
    }

    pub fn place_food(&mut self) {
        let size = self.size;
        let free: Vec<IVec2> = (0..size.y)
            .flat_map(|y| (0..size.x).map(move |x| ivec2(x, y)))
            .filter(|p| !self.body.contains(p))
            .collect();
        if !free.is_empty() {
            self.food = free[rand::gen_range(0, free.len())];
        }
    }

    pub fn turn(&mut self, d: Dir) {
        let last = *self.queued.back().unwrap_or(&self.dir);
        if d != last && d != last.opposite() && self.queued.len() < 3 {
            self.queued.push_back(d);
        }
    }

    pub fn tick(&mut self) {
        if let Some(d) = self.queued.pop_front() {
            self.dir = d;
        }
        let mut head = self.body[0] + self.dir.delta();
        if self.wrap {
            head = ivec2(
                head.x.rem_euclid(self.size.x),
                head.y.rem_euclid(self.size.y),
            );
        } else if head.x < 0 || head.y < 0 || head.x >= self.size.x || head.y >= self.size.y {
            self.die();
            return;
        }
        let eating = head == self.food;
        if !eating {
            self.body.pop_back();
        }
        if self.body.contains(&head) {
            self.die();
            return;
        }
        self.body.push_front(head);
        if eating {
            self.score += 1;
            self.place_food();
        }
    }

    fn die(&mut self) {
        self.state = State::Dead;
        self.best.offer(self.score);
    }

    pub fn size_for(view: &View) -> IVec2 {
        if view.h > view.w {
            ivec2(SHORT, LONG)
        } else {
            ivec2(LONG, SHORT)
        }
    }

    fn layout(&self, view: &View) -> (f32, Vec2) {
        let top = 70.0;
        let (gw, gh) = (self.size.x as f32, self.size.y as f32);
        let cell = ((view.w - 40.0) / gw)
            .min((view.h - top - 20.0) / gh)
            .floor();
        (
            cell,
            vec2(
                (view.w - cell * gw) / 2.0,
                top + (view.h - top - 20.0 - cell * gh) / 2.0,
            ),
        )
    }
}

impl Default for Snake {
    fn default() -> Self {
        Self::new()
    }
}

impl Game for Snake {
    fn update(&mut self, input: &Input, view: &View, dt: f32) {
        match self.state {
            State::Ready => {
                if Self::size_for(view) != self.size {
                    let best = self.best;
                    *self = Self::sized(Self::size_for(view));
                    self.best = best;
                }
                if let Some(d) = input.dir() {
                    self.turn(d);
                    self.state = State::Playing;
                } else if input.tap() {
                    self.state = State::Playing;
                }
            }
            State::Playing => {
                if let Some(d) = input.dir() {
                    self.turn(d);
                }
                let speed = STEP * (1.0 - (self.score as f32 * 0.01).min(0.4));
                self.timer += dt;
                while self.timer >= speed && self.state == State::Playing {
                    self.timer -= speed;
                    self.tick();
                }
            }
            State::Dead => {
                if input.tap() {
                    let best = self.best;
                    *self = Self::sized(Self::size_for(view));
                    self.best = best;
                }
            }
        }
    }

    fn draw(&self, view: &View) {
        clear_background(Color::from_rgba(70, 120, 60, 255));
        let (cell, o) = self.layout(view);
        for y in 0..self.size.y {
            for x in 0..self.size.x {
                let c = if (x + y) % 2 == 0 {
                    Color::from_rgba(170, 215, 80, 255)
                } else {
                    Color::from_rgba(160, 205, 72, 255)
                };
                draw_rectangle(o.x + x as f32 * cell, o.y + y as f32 * cell, cell, cell, c);
            }
        }
        let f = o + self.food.as_vec2() * cell + vec2(cell, cell) / 2.0;
        draw_circle(
            f.x,
            f.y + 2.0,
            cell * 0.4,
            Color::from_rgba(200, 40, 40, 255),
        );
        draw_circle(
            f.x - cell * 0.12,
            f.y - cell * 0.05,
            cell * 0.1,
            with_alpha(WHITE, 0.6),
        );
        draw_rectangle(
            f.x - 1.5,
            f.y - cell * 0.5,
            3.0,
            cell * 0.2,
            Color::from_rgba(100, 60, 30, 255),
        );
        draw_ellipse(
            f.x + cell * 0.12,
            f.y - cell * 0.38,
            cell * 0.14,
            cell * 0.07,
            -20.0,
            Color::from_rgba(60, 160, 50, 255),
        );
        let n = self.body.len();
        for (i, p) in self.body.iter().enumerate().rev() {
            let k = 1.0 - i as f32 / n as f32 * 0.4;
            let c = o + p.as_vec2() * cell + vec2(cell, cell) / 2.0;
            let col = Color::from_rgba(70, 115, 230, 255);
            draw_circle(
                c.x,
                c.y,
                cell * 0.48 * k.max(0.75),
                darker(col, 0.9 + 0.1 * k),
            );
            if i + 1 < n {
                let nxt = self.body[i + 1];
                if (nxt - *p).abs().max_element() == 1 {
                    let m = o + (p.as_vec2() + nxt.as_vec2()) / 2.0 * cell + vec2(cell, cell) / 2.0;
                    draw_circle(
                        m.x,
                        m.y,
                        cell * 0.44 * k.max(0.75),
                        darker(col, 0.9 + 0.1 * k),
                    );
                }
            }
        }
        let h = o + self.body[0].as_vec2() * cell + vec2(cell, cell) / 2.0;
        let fwd = self.dir.delta().as_vec2();
        let side = vec2(-fwd.y, fwd.x);
        for s in [-1.0, 1.0] {
            let e = h + fwd * cell * 0.12 + side * s * cell * 0.22;
            draw_circle(e.x, e.y, cell * 0.16, WHITE);
            draw_circle(e.x + fwd.x * 2.0, e.y + fwd.y * 2.0, cell * 0.08, BLACK);
        }
        text(
            &format!("Apples {}", self.score),
            HUD_LEFT,
            48.0,
            40.0,
            WHITE,
        );
        text(
            &format!("Best {}", self.best.0),
            view.w - 180.0,
            48.0,
            36.0,
            WHITE,
        );
        match self.state {
            State::Ready => overlay(
                view,
                "Snake",
                &["Swipe to steer", "Eat apples, don't bite yourself"],
            ),
            State::Dead => {
                let s = format!("Apples {}   Best {}", self.score, self.best.0);
                overlay(view, "Ouch!", &[&s, "Tap to play again"]);
            }
            State::Playing => {}
        }
    }
}

pub fn icon(r: Rect) {
    draw_rectangle(r.x, r.y, r.w, r.h, Color::from_rgba(170, 215, 80, 255));
    let c = Color::from_rgba(70, 115, 230, 255);
    for i in 0..5 {
        draw_circle(
            r.x + r.w * (0.2 + i as f32 * 0.12),
            r.y + r.h * 0.6,
            r.w * 0.09,
            c,
        );
    }
    draw_circle(r.x + r.w * 0.72, r.y + r.h * 0.6, r.w * 0.11, c);
    draw_circle(r.x + r.w * 0.74, r.y + r.h * 0.55, r.w * 0.04, WHITE);
    draw_circle(
        r.x + r.w * 0.75,
        r.y + r.h * 0.3,
        r.w * 0.09,
        Color::from_rgba(200, 40, 40, 255),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn eating_grows_and_scores() {
        let mut s = Snake::new();
        s.food = s.body[0] + ivec2(1, 0);
        let len = s.body.len();
        s.tick();
        assert_eq!(s.body.len(), len + 1);
        assert_eq!(s.score, 1);
        assert!(!s.body.contains(&s.food));
    }

    #[test]
    fn reverse_turn_is_ignored() {
        let mut s = Snake::new();
        s.turn(Dir::Left);
        assert!(s.queued.is_empty());
    }

    #[test]
    fn two_quick_turns_both_apply() {
        let mut s = Snake::new();
        s.food = ivec2(0, 0);
        s.turn(Dir::Up);
        s.turn(Dir::Left);
        let h = s.body[0];
        s.tick();
        s.tick();
        assert_eq!(s.body[0], h + ivec2(-1, -1));
    }

    #[test]
    fn biting_yourself_ends_the_game() {
        let mut s = Snake::new();
        s.body = (0..6).map(|i| ivec2(10 - i, 5)).collect();
        s.food = ivec2(0, 0);
        for d in [Dir::Down, Dir::Left, Dir::Up] {
            s.turn(d);
            s.tick();
        }
        assert_eq!(s.state, State::Dead);
    }

    #[test]
    fn walls_wrap_around() {
        let mut s = Snake::new();
        s.body = [ivec2(LONG - 1, 3), ivec2(LONG - 2, 3)]
            .into_iter()
            .collect();
        s.food = ivec2(5, 5);
        s.tick();
        assert_eq!(s.body[0], ivec2(0, 3));
        assert_ne!(s.state, State::Dead);
    }

    #[test]
    fn portrait_screen_gets_a_tall_board() {
        let mut s = Snake::new();
        s.update(&Input::default(), &View::fit(1080.0, 2400.0), 0.01);
        assert_eq!(s.size, ivec2(SHORT, LONG));
        let v = View::fit(1080.0, 2400.0);
        let (tall, _) = s.layout(&v);
        let (wide, _) = Snake::new().layout(&v);
        assert!(tall > wide * 1.3, "tall {tall} wide {wide}");
    }

    #[test]
    fn moving_into_the_tail_cell_is_safe() {
        let mut s = Snake::new();
        s.body = [ivec2(5, 5), ivec2(5, 6), ivec2(6, 6), ivec2(6, 5)]
            .into_iter()
            .collect();
        s.dir = Dir::Up;
        s.food = ivec2(0, 0);
        s.turn(Dir::Right);
        s.tick();
        assert_ne!(s.state, State::Dead);
    }
}
