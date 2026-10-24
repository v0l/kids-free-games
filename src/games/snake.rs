use crate::engine::draw_circle;
use crate::engine::draw_ellipse;
use crate::engine::*;
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
                } else if input.tap() {
                    self.state = State::Playing;
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
        clear_background(Color::from_rgba(70, 120, 60, 255));
        let (cell, o) = self.layout(view);
        let off = self.shake.offset();
        set_view_camera(view, vec2(off.x, off.y));
        let board = Rect::new(
            o.x,
            o.y,
            cell * self.size.x as f32,
            cell * self.size.y as f32,
        );
        round_rect(
            Rect::new(board.x - 6.0, board.y - 2.0, board.w + 12.0, board.h + 18.0),
            18.0,
            Color::from_rgba(40, 80, 40, 255),
        );
        round_rect(
            Rect::new(board.x - 6.0, board.y - 6.0, board.w + 12.0, board.h + 12.0),
            18.0,
            Color::from_rgba(95, 150, 70, 255),
        );
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
        self.draw_apple(o, cell);
        self.draw_snake(o, cell);
        self.particles.draw();
        self.popups.draw();
        set_view_camera(view, Vec2::ZERO);
        vignette(view, 0.5);
        let apples = format!("Apples {}", self.score);
        let aw = text_size(&apples, 40.0).width;
        score_text(&apples, HUD_LEFT + aw / 2.0, 42.0, 40.0, self.bump.t);
        let best = format!("Best {}", self.best.0);
        let bw = text_size(&best, 36.0).width;
        score_text(&best, view.w - 30.0 - bw / 2.0, 42.0, 36.0, 1.0);
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

impl Snake {
    fn draw_apple(&self, o: Vec2, cell: f32) {
        let f = o + self.food.as_vec2() * cell + vec2(cell, cell) / 2.0;
        let pop = ease_out_elastic(self.food_age / 0.7).max(0.01);
        let bob = (self.clock * 4.0).sin() * cell * 0.03;
        let drop = (1.0 - ease_out_cubic(self.food_age / 0.35)) * cell * 0.8;
        let squash = 1.0 + (self.clock * 4.0).sin() * 0.03;
        let r = cell * 0.4 * pop;
        soft_shadow(f + vec2(0.0, cell * 0.38), r * 0.9, r * 0.3, 0.6);
        let c = f + vec2(0.0, bob - drop);
        draw_ellipse(
            c.x,
            c.y + 2.0,
            r * squash,
            r / squash,
            0.0,
            Color::from_rgba(200, 40, 40, 255),
        );
        draw_ellipse(
            c.x,
            c.y + 2.0 + r * 0.25,
            r * 0.85,
            r * 0.55,
            0.0,
            Color::from_rgba(170, 25, 40, 255),
        );
        draw_circle(
            c.x - r * 0.3,
            c.y - r * 0.15,
            r * 0.25,
            with_alpha(WHITE, 0.65),
        );
        draw_line(
            c.x,
            c.y - r * 0.8,
            c.x + r * 0.1,
            c.y - r * 1.3,
            3.0,
            Color::from_rgba(100, 60, 30, 255),
        );
        draw_ellipse(
            c.x + r * 0.35,
            c.y - r * 1.05,
            r * 0.38,
            r * 0.2,
            -25.0,
            Color::from_rgba(60, 160, 50, 255),
        );
    }

    fn draw_snake(&self, o: Vec2, cell: f32) {
        let chain = self.chain(self.frac());
        let n = chain.len();
        let size = self.size.as_vec2();
        let base = if self.state == State::Dead {
            Color::from_rgba(110, 130, 190, 255)
        } else {
            Color::from_rgba(70, 115, 230, 255)
        };
        let bulge_pos: Vec<f32> = self
            .bulges
            .iter()
            .map(|b| *b as f32 + self.frac())
            .collect();
        let radius = |i: usize| {
            let k = 1.0 - i as f32 / n as f32 * 0.35;
            let swell = bulge_pos
                .iter()
                .map(|p| bulge_scale(i, *p))
                .fold(1.0, f32::max);
            cell * 0.46 * k * swell
        };
        let visible =
            |p: Vec2| p.x > -1.0 && p.y > -1.0 && p.x < size.x + 1.0 && p.y < size.y + 1.0;
        let shifts = [-1.0f32, 0.0, 1.0];
        for pass in 0..3 {
            for sx in shifts {
                for sy in shifts {
                    let shift = vec2(sx * size.x, sy * size.y);
                    for i in (0..n).rev() {
                        let p = chain[i] + shift;
                        if !visible(p) {
                            continue;
                        }
                        let r = radius(i);
                        let c = o + (p + vec2(0.5, 0.5)) * cell;
                        let stripe = if i % 2 == 0 { 1.0 } else { 0.93 };
                        let (grow, col) = match pass {
                            0 => (1.14, darker(base, 0.6)),
                            1 => (1.0, darker(base, stripe)),
                            _ => (
                                0.0,
                                Color::from_vec(base.to_vec() * 0.5 + vec4(0.5, 0.5, 0.5, 0.5)),
                            ),
                        };
                        if pass < 2 {
                            if i + 1 < n {
                                let q = o + (chain[i + 1] + shift + vec2(0.5, 0.5)) * cell;
                                draw_line(
                                    c.x,
                                    c.y,
                                    q.x,
                                    q.y,
                                    2.0 * r.min(radius(i + 1)) * grow,
                                    col,
                                );
                            }
                            draw_circle(c.x, c.y, r * grow, col);
                        } else if i % 2 == 0 {
                            draw_circle(
                                c.x - r * 0.25,
                                c.y - r * 0.35,
                                r * 0.28,
                                with_alpha(col, 0.28),
                            );
                        }
                    }
                }
            }
        }
        let head = o + (chain[0] + vec2(0.5, 0.5)) * cell;
        let fwd = self.dir.delta().as_vec2();
        let side = vec2(-fwd.y, fwd.x);
        let r = radius(0);
        if self.state != State::Dead {
            let len = tongue_len(self.clock) * cell * 0.75;
            if len > 1.0 {
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
        draw_circle(head.x, head.y, r * 1.08, darker(base, 0.6));
        draw_circle(head.x, head.y, r * 0.98, base);
        draw_circle(
            head.x - r * 0.25,
            head.y - r * 0.4,
            r * 0.3,
            with_alpha(WHITE, 0.25),
        );
        for s in [-1.0, 1.0] {
            let e = head + fwd * cell * 0.12 + side * s * cell * 0.22;
            draw_circle(e.x, e.y, cell * 0.17, WHITE);
            if self.state == State::Dead {
                let k = cell * 0.08;
                draw_line(e.x - k, e.y - k, e.x + k, e.y + k, 2.5, BLACK);
                draw_line(e.x - k, e.y + k, e.x + k, e.y - k, 2.5, BLACK);
            } else {
                draw_circle(e.x + fwd.x * 2.5, e.y + fwd.y * 2.5, cell * 0.085, BLACK);
                draw_circle(
                    e.x + fwd.x * 2.5 - 1.0,
                    e.y + fwd.y * 2.5 - 1.5,
                    cell * 0.03,
                    WHITE,
                );
            }
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
