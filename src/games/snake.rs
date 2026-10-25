use crate::engine::*;
use crate::feel::{self, Impact};
use crate::fx::*;
use crate::sfx::{self, Sfx};
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

#[derive(Clone, Copy, Debug, PartialEq)]
enum Event {
    Ate(IVec2),
    Crashed(IVec2),
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
    prev: Vec<IVec2>,
    bulges: Vec<u32>,
    events: Vec<Event>,
    particles: Particles,
    popups: Popups,
    shake: Shake,
    bump: Bump,
    clock: f32,
    food_age: f32,
    dead_age: f32,
}

pub fn bulge_scale(index: usize, bulge_at: f32) -> f32 {
    let d = (index as f32 - bulge_at).abs();
    1.0 + 0.4 * (1.0 - d / 1.6).max(0.0)
}

pub fn tongue_len(clock: f32) -> f32 {
    let phase = clock.rem_euclid(1.8) / 0.4;
    if phase >= 1.0 {
        0.0
    } else {
        1.0 - (phase * 2.0 - 1.0).abs()
    }
}

fn wrap_delta(d: f32, span: f32) -> f32 {
    if d > span / 2.0 {
        d - span
    } else if d < -span / 2.0 {
        d + span
    } else {
        d
    }
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
            prev: vec![],
            bulges: vec![],
            events: vec![],
            particles: Particles::default(),
            popups: Popups::default(),
            shake: Shake::default(),
            bump: Bump::default(),
            clock: 0.0,
            food_age: 0.0,
            dead_age: 0.0,
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
        self.prev = self.body.iter().copied().collect();
        for b in &mut self.bulges {
            *b += 1;
        }
        let len = self.body.len() as u32 + 2;
        self.bulges.retain(|b| *b < len);
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
            self.events.push(Event::Crashed(self.body[0]));
            self.die();
            return;
        }
        let eating = head == self.food;
        if !eating {
            self.body.pop_back();
        }
        if self.body.contains(&head) {
            self.events.push(Event::Crashed(head));
            self.die();
            return;
        }
        self.body.push_front(head);
        if eating {
            self.score += 1;
            self.events.push(Event::Ate(head));
            self.bulges.push(0);
            self.place_food();
            self.food_age = 0.0;
        }
    }

    fn die(&mut self) {
        self.state = State::Dead;
        sfx::play(Sfx::Lose);
        if self.score > self.best.0 && self.best.0 > 0 {
            sfx::say(sfx::Say::Best);
        }
        self.best.offer(self.score);
    }

    pub fn size_for(view: &View) -> IVec2 {
        if view.h > view.w {
            ivec2(SHORT, LONG)
        } else {
            ivec2(LONG, SHORT)
        }
    }

    fn step_time(&self) -> f32 {
        STEP * (1.0 - (self.score as f32 * 0.01).min(0.4))
    }

    fn frac(&self) -> f32 {
        match self.state {
            State::Playing => (self.timer / self.step_time()).clamp(0.0, 1.0),
            _ => 1.0,
        }
    }

    pub fn chain(&self, frac: f32) -> Vec<Vec2> {
        let size = self.size.as_vec2();
        let mut out: Vec<Vec2> = Vec::with_capacity(self.body.len());
        for (i, to) in self.body.iter().enumerate() {
            let from = self
                .prev
                .get(i)
                .or(self.prev.last())
                .copied()
                .unwrap_or(*to);
            let d = (*to - from).as_vec2();
            let d = vec2(wrap_delta(d.x, size.x), wrap_delta(d.y, size.y));
            let raw = from.as_vec2() + d * frac;
            let at = match out.last() {
                None => raw,
                Some(last) => {
                    let step = raw - *last;
                    let step = vec2(
                        wrap_delta(step.x.rem_euclid(size.x), size.x),
                        wrap_delta(step.y.rem_euclid(size.y), size.y),
                    );
                    *last + step
                }
            };
            out.push(at);
        }
        out
    }

    fn react(&mut self, view: &View) {
        let (cell, o) = self.layout(view);
        let center = |p: IVec2| o + p.as_vec2() * cell + vec2(cell, cell) / 2.0;
        for e in std::mem::take(&mut self.events) {
            match e {
                Event::Ate(p) => {
                    sfx::play(Sfx::Crunch);
                    let at = center(p);
                    self.particles
                        .sparkle(at, Color::from_rgba(230, 60, 60, 255));
                    self.popups.add(
                        at - vec2(0.0, cell * 0.6),
                        "+1",
                        Color::from_rgba(255, 230, 90, 255),
                        cell * 1.1,
                    );
                    self.bump.kick();
                }
                Event::Crashed(p) => {
                    sfx::play(Sfx::Hit);
                    feel::impact(Impact::Large);
                    let at = center(p);
                    self.shake.add(0.8);
                    self.particles.emit(
                        at,
                        &[
                            Color::from_rgba(70, 115, 230, 255),
                            Color::from_rgba(150, 180, 255, 255),
                            WHITE,
                        ],
                        Burst {
                            count: 26,
                            size: (cell * 0.12, cell * 0.28),
                            shape: Shape::Square,
                            ..Default::default()
                        },
                    );
                    self.dead_age = 0.0;
                }
            }
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
        self.clock += dt;
        self.food_age += dt;
        self.dead_age += dt;
        self.particles.update(dt);
        self.popups.update(dt);
        self.shake.update(dt);
        self.bump.update(dt);
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
                    sfx::say(sfx::Say::Go);
                } else if input.tap() {
                    self.state = State::Playing;
                    sfx::say(sfx::Say::Go);
                }
            }
            State::Playing => {
                if let Some(d) = input.dir() {
                    self.turn(d);
                }
                let speed = self.step_time();
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
        self.react(view);
    }

    fn draw(&self, view: &View) {
        let (cell, o) = self.layout(view);
        let off = self.shake.offset();
        set_view_camera(view, vec2(off.x, off.y));
        let board = Rect::new(
            o.x,
            o.y,
            cell * self.size.x as f32,
            cell * self.size.y as f32,
        );
        draw_meadow(view, board);
        for y in 0..self.size.y {
            for x in 0..self.size.x {
                let c = if (x + y) % 2 == 0 {
                    Color::new(1.0, 1.0, 0.7, 0.07)
                } else {
                    Color::new(0.0, 0.3, 0.0, 0.08)
                };
                draw_rectangle(o.x + x as f32 * cell, o.y + y as f32 * cell, cell, cell, c);
            }
        }
        draw_rim(board);
        self.draw_apple(o, cell);
        self.draw_snake(o, cell);
        self.particles.draw();
        self.popups.draw();
        set_view_camera(view, Vec2::ZERO);
        chip(
            view,
            Side::Left,
            "APPLES",
            &self.score.to_string(),
            self.bump.t,
        );
        chip(view, Side::Right, "BEST", &self.best.0.to_string(), 1.0);
        match self.state {
            State::Ready => overlay(
                view,
                "Snake",
                &["Swipe to steer", "Eat fruit, don't bite yourself"],
            ),
            State::Dead => {
                let s = format!("Apples {}   Best {}", self.score, self.best.0);
                overlay(view, "Ouch!", &[&s, "Tap to play again"]);
            }
            State::Playing => {}
        }
    }
}

mod art {
    crate::sprites!("snake");
}

const FRUIT: [&str; 6] = [
    "apple",
    "orange",
    "pineapple",
    "strawberry",
    "lemon",
    "watermelon",
];

fn draw_meadow(view: &View, board: Rect) {
    let f = view.full();
    let side = f.w.max(f.h);
    art::sprite(
        "bg",
        Rect::new(
            f.x + (f.w - side) / 2.0,
            f.y + (f.h - side) / 2.0,
            side,
            side,
        ),
    );
    let pad = 14.0;
    for (grow, alpha) in [(26.0, 0.06), (20.0, 0.08), (14.0, 0.1)] {
        round_rect(
            Rect::new(
                board.x - grow,
                board.y - grow + 6.0,
                board.w + grow * 2.0,
                board.h + grow * 2.0,
            ),
            22.0,
            Color::new(0.0, 0.12, 0.0, alpha),
        );
    }
    round_rect(
        Rect::new(
            board.x - pad,
            board.y - pad,
            board.w + pad * 2.0,
            board.h + pad * 2.0,
        ),
        pad + 6.0,
        Color::from_rgba(52, 120, 34, 255),
    );
    round_rect(
        Rect::new(
            board.x - pad + 3.0,
            board.y - pad + 3.0,
            board.w + pad * 2.0 - 6.0,
            board.h + pad * 2.0 - 8.0,
        ),
        pad + 3.0,
        Color::from_rgba(120, 200, 70, 255),
    );
    round_rect(
        Rect::new(board.x - 4.0, board.y - 4.0, board.w + 8.0, board.h + 8.0),
        10.0,
        Color::from_rgba(150, 214, 76, 255),
    );
}

fn draw_rim(board: Rect) {
    let edge = Color::new(0.0, 0.2, 0.0, 0.22);
    let clear = Color::new(0.0, 0.2, 0.0, 0.0);
    let d = 16.0;
    let (x0, y0, x1, y1) = (board.x, board.y, board.x + board.w, board.y + board.h);
    let quad = |a: Vec2, b: Vec2, c: Vec2, e: Vec2, ca: Color, cb: Color| {
        let v = |p: Vec2, col: Color| macroquad::models::Vertex::new(p.x, p.y, 0.0, 0.0, 0.0, col);
        let verts = [v(a, ca), v(b, ca), v(c, cb), v(e, cb)];
        draw_mesh(&macroquad::models::Mesh {
            vertices: verts.to_vec(),
            indices: vec![0, 1, 2, 0, 2, 3],
            texture: None,
        });
    };
    quad(
        vec2(x0, y0),
        vec2(x1, y0),
        vec2(x1, y0 + d),
        vec2(x0, y0 + d),
        edge,
        clear,
    );
    quad(
        vec2(x0, y1),
        vec2(x1, y1),
        vec2(x1, y1 - d),
        vec2(x0, y1 - d),
        edge,
        clear,
    );
    quad(
        vec2(x0, y0),
        vec2(x0, y1),
        vec2(x0 + d, y1),
        vec2(x0 + d, y0),
        edge,
        clear,
    );
    quad(
        vec2(x1, y0),
        vec2(x1, y1),
        vec2(x1 - d, y1),
        vec2(x1 - d, y0),
        edge,
        clear,
    );
}

impl Snake {
    fn draw_apple(&self, o: Vec2, cell: f32) {
        let f = o + self.food.as_vec2() * cell + vec2(cell, cell) / 2.0;
        let pop = ease_out_elastic(self.food_age / 0.7).max(0.01);
        let bob = (self.clock * 4.0).sin() * cell * 0.03;
        let drop = (1.0 - ease_out_cubic(self.food_age / 0.35)) * cell * 0.8;
        let squash = 1.0 + (self.clock * 4.0).sin() * 0.03;
        let h = cell * 1.7 * pop;
        soft_shadow(f + vec2(0.0, cell * 0.38), h * 0.4, h * 0.13, 0.6);
        let c = f + vec2(0.0, bob - drop);
        let name = FRUIT[self.score as usize % FRUIT.len()];
        let s = art::size(name);
        let w = h * s.x / s.y.max(1.0);
        art::sprite(
            name,
            Rect::new(
                c.x - w * squash / 2.0,
                c.y - h / squash / 2.0,
                w * squash,
                h / squash,
            ),
        );
    }

    fn draw_snake(&self, o: Vec2, cell: f32) {
        let chain = self.chain(self.frac());
        let n = chain.len();
        let size = self.size.as_vec2();
        let dead = self.state == State::Dead;
        let bulge_pos: Vec<f32> = self
            .bulges
            .iter()
            .map(|b| *b as f32 + self.frac())
            .collect();
        let radius = |i: usize| {
            let k = 1.0 - i as f32 / n as f32 * 0.3;
            let swell = bulge_pos
                .iter()
                .map(|p| bulge_scale(i, *p))
                .fold(1.0, f32::max);
            cell * 0.56 * k * swell
        };
        let visible =
            |p: Vec2| p.x > -1.0 && p.y > -1.0 && p.x < size.x + 1.0 && p.y < size.y + 1.0;
        let shifts = [-1.0f32, 0.0, 1.0];
        for sx in shifts {
            for sy in shifts {
                let shift = vec2(sx * size.x, sy * size.y);
                for i in (1..n).rev() {
                    let p = chain[i] + shift;
                    if !visible(p) {
                        continue;
                    }
                    let to = o + (p + vec2(0.5, 0.5)) * cell;
                    let from = o + (chain[i - 1] + shift + vec2(0.5, 0.5)) * cell;
                    let r = radius(i);
                    soft_shadow(to + vec2(r * 0.18, r * 0.62), r * 0.9, r * 0.32, 0.5);
                    let shade = if i % 2 == 0 { 1.0 } else { 0.93 };
                    let tint = if dead {
                        Color::from_rgba(170, 180, 205, 255)
                    } else {
                        darker(WHITE, shade)
                    };
                    let steps = 2;
                    for k in 0..steps {
                        let t = k as f32 / steps as f32;
                        let c = to + (from - to) * t * 0.5;
                        art::sprite_at(
                            "ball",
                            c,
                            r * 2.0 * (1.0 - 0.04 * t),
                            crate::art::Draw {
                                tint,
                                ..Default::default()
                            },
                        );
                    }
                }
            }
        }
        let head = o + (chain[0] + vec2(0.5, 0.5)) * cell;
        let fwd = self.dir.delta().as_vec2();
        let r = radius(0);
        if !dead {
            let len = tongue_len(self.clock) * cell * 0.75;
            if len > 1.0 {
                let side = vec2(-fwd.y, fwd.x);
                let red = Color::from_rgba(230, 50, 80, 255);
                let base_p = head + fwd * r * 0.8;
                let tip = base_p + fwd * len;
                draw_line(base_p.x, base_p.y, tip.x, tip.y, cell * 0.07, red);
                for s in [-1.0, 1.0] {
                    let fork = tip + (fwd + side * s * 0.8) * cell * 0.14;
                    draw_line(tip.x, tip.y, fork.x, fork.y, cell * 0.06, red);
                }
            }
        }
        let wob = if dead {
            (self.dead_age * 20.0).sin() * 0.12
        } else {
            0.0
        };
        let lean = fwd.x * 0.16 + wob;
        let at = head + vec2(fwd.x * r * 0.25, -r * 0.18 + fwd.y * r * 0.2);
        soft_shadow(head + vec2(r * 0.15, r * 0.7), r * 1.2, r * 0.4, 0.55);
        art::sprite_at(
            "head",
            at,
            r * 3.4,
            crate::art::Draw {
                rotation: lean,
                tint: if dead {
                    Color::from_rgba(190, 200, 235, 255)
                } else {
                    WHITE
                },
                ..Default::default()
            },
        );
    }
}

pub fn icon(r: Rect) {
    crate::ui::icon("snake", r);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snake_sprites_all_exist() {
        let have = art::names();
        let mut want = vec!["bg", "head", "ball"];
        want.extend(FRUIT);
        for n in want {
            assert!(have.iter().any(|h| h == n), "missing {n}");
        }
    }

    #[test]
    fn crashing_plays_the_lose_jingle() {
        let mut s = Snake::new();
        s.wrap = false;
        s.body = [ivec2(LONG - 1, 3), ivec2(LONG - 2, 3)]
            .into_iter()
            .collect();
        crate::sfx::take_pending();
        s.tick();
        assert!(crate::sfx::take_pending().contains(&Sfx::Lose));
    }

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

    #[test]
    fn head_glides_between_cells() {
        let mut s = Snake::new();
        s.food = ivec2(0, 0);
        let before = s.body[0];
        s.tick();
        let half = s.chain(0.5);
        assert!((half[0] - (before.as_vec2() + vec2(0.5, 0.0))).length() < 1e-4);
        let done = s.chain(1.0);
        assert_eq!(done[0], s.body[0].as_vec2());
        assert_eq!(done[3], s.body[3].as_vec2());
    }

    #[test]
    fn glide_across_the_wrap_does_not_jump() {
        let mut s = Snake::new();
        s.body = [ivec2(LONG - 1, 3), ivec2(LONG - 2, 3), ivec2(LONG - 3, 3)]
            .into_iter()
            .collect();
        s.food = ivec2(5, 5);
        s.tick();
        for k in 0..=10 {
            let c = s.chain(k as f32 / 10.0);
            for w in c.windows(2) {
                assert!((w[0] - w[1]).length() < 1.6, "gap {:?}", w);
            }
        }
        let c = s.chain(0.5);
        assert!((c[0].x - (LONG as f32 - 0.5)).abs() < 1e-4);
    }

    #[test]
    fn tail_stays_put_while_growing() {
        let mut s = Snake::new();
        let tail = *s.body.back().unwrap();
        s.food = s.body[0] + ivec2(1, 0);
        s.tick();
        let c = s.chain(0.5);
        assert_eq!(*c.last().unwrap(), tail.as_vec2());
    }

    #[test]
    fn eating_queues_an_event_and_a_bulge() {
        let mut s = Snake::new();
        s.food = s.body[0] + ivec2(1, 0);
        s.tick();
        assert_eq!(s.events, vec![Event::Ate(s.body[0])]);
        assert_eq!(s.bulges, vec![0]);
        s.food = ivec2(0, 0);
        s.tick();
        assert_eq!(s.bulges, vec![1]);
        for _ in 0..20 {
            s.tick();
        }
        assert!(s.bulges.is_empty());
    }

    #[test]
    fn crashing_queues_an_event() {
        let mut s = Snake::new();
        s.wrap = false;
        s.body = [ivec2(LONG - 1, 3), ivec2(LONG - 2, 3)]
            .into_iter()
            .collect();
        s.tick();
        assert_eq!(s.state, State::Dead);
        assert_eq!(s.events, vec![Event::Crashed(ivec2(LONG - 1, 3))]);
    }

    #[test]
    fn bulge_peaks_at_its_position_and_fades() {
        assert!(bulge_scale(4, 4.0) > bulge_scale(4, 4.8));
        assert_eq!(bulge_scale(4, 9.0), 1.0);
        assert!((bulge_scale(2, 2.0) - 1.4).abs() < 1e-5);
    }

    #[test]
    fn tongue_flicks_then_rests() {
        assert_eq!(tongue_len(1.0), 0.0);
        assert!(tongue_len(0.2) > 0.99);
        assert_eq!(tongue_len(0.0), 0.0);
        assert_eq!(tongue_len(1.8 + 1.0), 0.0);
    }
}
