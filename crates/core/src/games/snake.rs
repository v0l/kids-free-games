use crate::engine::*;
use crate::feel::{self, Impact};
use crate::fx::*;
use crate::geom::*;
use crate::input::SwipeTracker;
use crate::rng as rand;
use crate::sfx::{self, Sfx};
use glam::{ivec2, vec2, IVec2, Vec2};
use std::collections::VecDeque;

pub const LONG: i32 = 24;
pub const SHORT: i32 = 18;
pub const START_STEP: f32 = 0.22;
pub const FASTEST_STEP: f32 = 0.09;
pub const STAGE_DROP: f32 = 0.02;
pub const STAGE_APPLES: u32 = 5;
pub const GRACE_MOVES: u32 = 20;
pub const GENTLE_GRACE_MOVES: u32 = 32;
pub const GENTLE_SLOWDOWN: f32 = 1.25;
pub const GENTLE_LIMIT: u32 = 4;
pub const GENTLE_AFTER: i64 = 2;
pub const FIRST_RUN_SAFE_APPLES: u32 = 3;
pub const RESULTS_DELAY: f32 = 0.6;
pub const GAME: &str = "snake";
pub const FIRST_FOOD_AHEAD: i32 = 6;
pub const RETRO_WINDOW: f32 = 0.6;
pub const SNAKE_SWIPE_MM: f32 = 3.0;
pub const BRINK_START: f32 = 0.2;
pub const BRINK_DROP: f32 = 0.03;
pub const FACE_RATE: f32 = 28.0;
pub const NUDGE_RATE: f32 = 30.0;
pub const MOUTH_CELLS: f32 = 3.0;
pub const MAX_LONG: i32 = 34;
pub const PHONE_SHORT: i32 = 15;
pub const LAYOUT_TOP: f32 = 70.0;

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
    Stage(u32),
    Bonk(IVec2),
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
    pub moves: u32,
    pub gentle: bool,
    pub best_stage: u32,
    pub rank: Option<usize>,
    pub timer: f32,
    prev: Vec<IVec2>,
    pub bulges: Vec<u32>,
    events: Vec<Event>,
    pub particles: Particles,
    pub popups: Popups,
    pub shake: Shake,
    pub bump: Bump,
    pub clock: f32,
    pub food_age: f32,
    pub dead_age: f32,
    pub face: f32,
    pub nudge: Vec2,
    pub turn_kick: f32,
    pub gulp: f32,
    pub brink: f32,
    pub intro_said: bool,
    pub eaten_at: IVec2,
    pub eat_age: f32,
    swipes: SwipeTracker,
    pad_last: Option<Dir>,
    pre_dir: Dir,
    last_tail: Option<IVec2>,
    retro_used: bool,
    tap_armed: bool,
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

pub fn angle_of(d: Dir) -> f32 {
    let v = d.delta().as_vec2();
    v.y.atan2(v.x)
}

pub fn wrap_angle(a: f32) -> f32 {
    (a + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI
}

pub fn left_of(d: Dir) -> Dir {
    match d {
        Dir::Right => Dir::Up,
        Dir::Up => Dir::Left,
        Dir::Left => Dir::Down,
        Dir::Down => Dir::Right,
    }
}

pub fn right_of(d: Dir) -> Dir {
    left_of(d).opposite()
}

pub fn blink(clock: f32) -> f32 {
    let t = clock.rem_euclid(3.7);
    let lid = |at: f32| (1.0 - ((t - at).abs() / 0.07)).max(0.0);
    lid(3.2).max(lid(3.45) * 0.9)
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
            best: Best::load(GAME),
            wrap: true,
            size,
            moves: 0,
            gentle: crate::profile::get(GAME, "weak") >= GENTLE_AFTER,
            best_stage: crate::profile::get(GAME, "stage").max(1) as u32,
            rank: None,
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
            face: 0.0,
            nudge: Vec2::ZERO,
            turn_kick: 0.0,
            gulp: 0.0,
            brink: 0.0,
            intro_said: false,
            eaten_at: IVec2::ZERO,
            eat_age: 9.0,
            swipes: SwipeTracker::default(),
            pad_last: None,
            pre_dir: Dir::Right,
            last_tail: None,
            retro_used: false,
            tap_armed: false,
        };
        s.food = s.body[0] + ivec2(FIRST_FOOD_AHEAD, 0);
        s
    }

    pub fn stage(&self) -> u32 {
        self.score / STAGE_APPLES + 1
    }

    pub fn forgiving(&self) -> bool {
        let grace = if self.gentle {
            GENTLE_GRACE_MOVES
        } else {
            GRACE_MOVES
        };
        self.moves < grace || (self.best.0 == 0 && self.score < FIRST_RUN_SAFE_APPLES)
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

    pub fn heading(&self) -> Dir {
        *self.queued.front().unwrap_or(&self.dir)
    }

    pub fn drawn_head(&self) -> Vec2 {
        self.chain(self.frac())[0] + self.nudge
    }

    fn can_retro(&self, d: Dir) -> bool {
        self.state == State::Playing
            && self.queued.is_empty()
            && !self.retro_used
            && self.last_tail.is_some()
            && self.frac() < RETRO_WINDOW
            && d != self.dir
            && d != self.pre_dir.opposite()
            && self.body.len() > 1
            && self.retro_is_safe(d)
    }

    fn retro_is_safe(&self, d: Dir) -> bool {
        let neck = self.body[1];
        let mut at = neck + d.delta();
        if self.wrap {
            at = ivec2(at.x.rem_euclid(self.size.x), at.y.rem_euclid(self.size.y));
        } else if at.x < 0 || at.y < 0 || at.x >= self.size.x || at.y >= self.size.y {
            return false;
        }
        let n = self.body.len();
        !self.body.iter().skip(1).take(n - 1).any(|p| *p == at)
    }

    pub fn steer(&mut self, d: Dir) {
        if self.can_retro(d) {
            let frac = self.frac();
            let size = self.size.as_vec2();
            let from = self.chain(frac)[0];
            let tail = self.last_tail.take().unwrap_or_default();
            self.body.pop_front();
            self.body.push_back(tail);
            for b in &mut self.bulges {
                *b = b.saturating_sub(1);
            }
            self.moves = self.moves.saturating_sub(1);
            self.dir = self.pre_dir;
            self.queued.push_back(d);
            self.tick();
            self.retro_used = true;
            let to = self.chain(frac)[0];
            let gap = from - to;
            self.nudge += vec2(wrap_delta(gap.x, size.x), wrap_delta(gap.y, size.y));
        } else {
            let before = self.queued.len();
            self.turn(d);
            if self.queued.len() == before {
                return;
            }
        }
        self.turn_kick = 1.0;
        self.face += wrap_angle(angle_of(self.heading()) - self.face) * 0.5;
        sfx::play(Sfx::Tick);
    }

    fn next_cell(&self) -> IVec2 {
        let head = self.body[0] + self.heading().delta();
        if self.wrap {
            ivec2(
                head.x.rem_euclid(self.size.x),
                head.y.rem_euclid(self.size.y),
            )
        } else {
            head
        }
    }

    pub fn would_crash(&self) -> bool {
        if self.forgiving() {
            return false;
        }
        let head = self.next_cell();
        if head.x < 0 || head.y < 0 || head.x >= self.size.x || head.y >= self.size.y {
            return true;
        }
        let solid = self.body.len() - usize::from(head != self.food);
        self.body.iter().take(solid).any(|p| *p == head)
    }

    pub fn brink_time(&self) -> f32 {
        (BRINK_START - (self.stage() - 1) as f32 * BRINK_DROP).max(0.0)
    }

    pub fn mouth(&self) -> f32 {
        if self.state == State::Dead {
            return 0.0;
        }
        let size = self.size.as_vec2();
        let head = self.chain(self.frac())[0];
        let d = self.food.as_vec2() - head;
        let d = vec2(wrap_delta(d.x, size.x), wrap_delta(d.y, size.y));
        let near = (1.0 - (d.length() - 0.6) / (MOUTH_CELLS - 0.6)).clamp(0.0, 1.0);
        let ahead = d.normalize_or_zero().dot(Vec2::from_angle(self.face)).max(0.0);
        (near * (0.4 + 0.6 * ahead)).max(self.gulp)
    }

    pub fn look(&self) -> Vec2 {
        let size = self.size.as_vec2();
        let head = self.chain(self.frac())[0];
        let d = self.food.as_vec2() - head;
        vec2(wrap_delta(d.x, size.x), wrap_delta(d.y, size.y)).normalize_or_zero()
    }

    pub fn tick(&mut self) {
        self.prev = self.body.iter().copied().collect();
        self.pre_dir = self.dir;
        self.last_tail = None;
        self.retro_used = false;
        self.brink = 0.0;
        for b in &mut self.bulges {
            *b += 1;
        }
        let len = self.body.len() as u32 + 2;
        self.bulges.retain(|b| *b < len);
        let before = self.dir;
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
        let solid = self.body.len() - usize::from(!eating);
        if self.body.iter().take(solid).any(|p| *p == head) {
            if self.forgiving() {
                self.events.push(Event::Bonk(head));
                self.dir = before;
                self.moves += 1;
                return;
            }
            self.events.push(Event::Crashed(head));
            self.die();
            return;
        }
        self.moves += 1;
        if !eating {
            self.last_tail = self.body.pop_back();
        }
        self.body.push_front(head);
        if eating {
            self.score += 1;
            self.gulp = 1.0;
            self.eaten_at = head;
            self.eat_age = 0.0;
            self.events.push(Event::Ate(head));
            self.bulges.push(0);
            self.place_food();
            self.food_age = 0.0;
            if self.score.is_multiple_of(STAGE_APPLES) {
                let stage = self.stage();
                self.events.push(Event::Stage(stage));
                if stage > self.best_stage {
                    self.best_stage = stage;
                    crate::profile::set(GAME, "stage", stage as i64);
                }
            }
        }
    }

    fn die(&mut self) {
        self.state = State::Dead;
        sfx::play(Sfx::Lose);
        if self.score > self.best.0 && self.best.0 > 0 {
            sfx::say(sfx::Say::Best);
        }
        self.rank = self.best.submit(GAME, self.score);
        let weak = crate::profile::get(GAME, "weak");
        let weak = if self.score < GENTLE_LIMIT {
            weak + 1
        } else {
            0
        };
        crate::profile::set(GAME, "weak", weak);
    }

    pub fn size_for(view: &View) -> IVec2 {
        let (avail_w, avail_h) = (view.w - 40.0, view.h - LAYOUT_TOP - 20.0);
        let (short, long) = (avail_w.min(avail_h), avail_w.max(avail_h));
        let across = if long / short > 1.7 { PHONE_SHORT } else { SHORT };
        let along = ((long / (short / across as f32)).floor() as i32).clamp(LONG, MAX_LONG);
        if view.h > view.w {
            ivec2(across, along)
        } else {
            ivec2(along, across)
        }
    }

    pub fn step_time(&self) -> f32 {
        let base = (START_STEP - (self.stage() - 1) as f32 * STAGE_DROP).max(FASTEST_STEP);
        if self.gentle {
            base * GENTLE_SLOWDOWN
        } else {
            base
        }
    }

    pub fn shows_results(&self) -> bool {
        self.state == State::Dead && self.dead_age > RESULTS_DELAY
    }

    pub fn results_age(&self) -> f32 {
        (self.dead_age - RESULTS_DELAY).max(0.0)
    }

    pub fn fruit(&self) -> &'static str {
        FRUIT[(self.stage() as usize - 1) % FRUIT.len()]
    }

    pub fn frac(&self) -> f32 {
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
                    sfx::play_step(Sfx::Pop, (self.score.saturating_sub(1) % STAGE_APPLES) as usize * 2);
                    let at = center(p);
                    self.particles
                        .sparkle(at, Color::from_rgba(230, 60, 60, 255));
                    self.particles.emit(
                        at,
                        &[
                            Color::from_rgba(255, 220, 70, 255),
                            Color::from_rgba(255, 255, 255, 255),
                            Color::from_rgba(120, 220, 90, 255),
                        ],
                        Burst {
                            count: 14,
                            speed: (cell * 3.0, cell * 7.0),
                            size: (cell * 0.08, cell * 0.16),
                            life: (0.3, 0.55),
                            gravity: 0.0,
                            drag: 4.0,
                            ..Default::default()
                        },
                    );
                    self.popups.add(
                        at - vec2(0.0, cell * 0.6),
                        "+1",
                        Color::from_rgba(255, 230, 90, 255),
                        cell * 1.1,
                    );
                    self.bump.kick();
                }
                Event::Stage(n) => {
                    sfx::play(Sfx::LevelUp);
                    sfx::say(if n.is_multiple_of(2) {
                        sfx::Say::Great
                    } else {
                        sfx::Say::LevelUp
                    });
                    feel::impact_tinted(Impact::Large, Color::from_rgba(255, 210, 80, 255));
                    self.particles.confetti(view);
                    self.popups.add(
                        view.center() - vec2(0.0, view.h * 0.12),
                        format!("Stage {n}!"),
                        Color::from_rgba(255, 230, 90, 255),
                        cell * 2.4,
                    );
                }
                Event::Bonk(p) => {
                    sfx::play(Sfx::Wrong);
                    self.shake.add(0.25);
                    self.particles.puff(center(p), WHITE);
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

    pub fn layout(&self, view: &View) -> (f32, Vec2) {
        let top = LAYOUT_TOP;
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
        self.eat_age += dt;
        self.particles.update(dt);
        self.popups.update(dt);
        self.shake.update(dt);
        self.bump.update(dt);
        self.turn_kick = (self.turn_kick - dt * 5.0).max(0.0);
        self.gulp = (self.gulp - dt * 3.5).max(0.0);
        if self.state == State::Ready && Self::size_for(view) != self.size {
            let best = self.best;
            *self = Self::sized(Self::size_for(view));
            self.best = best;
        }
        let steer = self.read_steer(input, view);
        match self.state {
            State::Ready => {
                if !self.intro_said && self.clock > 0.4 {
                    self.intro_said = true;
                    sfx::say(sfx::Say::SnakeIntro);
                }
                if let Some(d) = steer.or(input.key_dir) {
                    self.turn(d);
                    self.start();
                } else if input.pressed || input.action {
                    self.start();
                }
            }
            State::Playing => {
                if let Some(d) = steer {
                    self.steer(d);
                }
                self.advance(dt);
            }
            State::Dead => {
                if input.tap() && self.dead_age > RESULTS_DELAY {
                    let best = self.best;
                    *self = Self::sized(Self::size_for(view));
                    self.best = best;
                    self.intro_said = true;
                }
            }
        }
        if self.state != State::Dead {
            let want = angle_of(self.heading());
            let gap = wrap_angle(want - self.face);
            self.face = wrap_angle(self.face + gap * (1.0 - (-FACE_RATE * dt).exp()));
        }
        self.nudge *= (-NUDGE_RATE * dt).exp();
        self.react(view);
    }
}

impl Snake {
    fn start(&mut self) {
        self.state = State::Playing;
        self.timer = 0.0;
        self.tick();
        sfx::say(sfx::Say::Go);
    }

    fn advance(&mut self, dt: f32) {
        let speed = self.step_time();
        if self.timer + dt >= speed && self.brink < self.brink_time() && self.would_crash() {
            self.brink += dt;
            self.timer = speed;
            return;
        }
        self.timer += dt;
        while self.timer >= speed && self.state == State::Playing {
            self.timer -= speed;
            self.tick();
        }
    }

    fn read_steer(&mut self, input: &Input, view: &View) -> Option<Dir> {
        let threshold = SNAKE_SWIPE_MM * input.mm();
        let swipe = self
            .swipes
            .track(input.pos, input.down && !input.cancelled, input.pressed, threshold);
        let pad = if input.axis.length() > 0.6 {
            Some(Dir::from_vec(input.axis))
        } else {
            None
        };
        let pad_new = pad.filter(|d| self.pad_last != Some(*d));
        self.pad_last = pad;
        if input.pressed {
            self.tap_armed = self.state == State::Playing;
        }
        let tap = (input.released
            && self.tap_armed
            && !input.cancelled
            && !self.swipes.swiped
            && self.swipes.travel < crate::input::TAP_SLOP_MM * input.mm()
            && self.state == State::Playing)
            .then(|| {
                let h = self.heading();
                if input.pos.x < view.w / 2.0 {
                    left_of(h)
                } else {
                    right_of(h)
                }
            });
        input.key_dir.or(swipe).or(pad_new).or(tap)
    }
}

pub const FRUIT: [&str; 6] = [
    "apple",
    "orange",
    "pineapple",
    "strawberry",
    "lemon",
    "watermelon",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snake_sprites_all_exist() {
        let b = std::fs::read(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../assets/art/snake.kga"
        ))
        .unwrap();
        let idx = crate::art::parse(&b).unwrap();
        let have: Vec<String> = idx.frames.keys().cloned().collect();
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
        s.moves = GRACE_MOVES;
        s.best = Best(9);
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
        assert_eq!(s.size.x, PHONE_SHORT);
        assert!(s.size.y >= LONG && s.size.y <= MAX_LONG);
        let v = View::fit(1080.0, 2400.0);
        let (tall, o) = s.layout(&v);
        let (wide, _) = Snake::new().layout(&v);
        assert!(tall > wide * 1.3, "tall {tall} wide {wide}");
        let unused = v.h - LAYOUT_TOP - tall * s.size.y as f32;
        assert!(unused < tall * 3.0, "{unused} left over, top {o:?}");
        assert_eq!(Snake::size_for(&View::ipad_portrait()), ivec2(SHORT, LONG));
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

    fn bitten(s: &mut Snake) {
        s.body = (0..6).map(|i| ivec2(10 - i, 5)).collect();
        s.dir = Dir::Right;
        s.food = ivec2(0, 0);
        for d in [Dir::Down, Dir::Left, Dir::Up] {
            s.turn(d);
            s.tick();
        }
    }

    fn fresh_player() {
        crate::save::reset_for_test();
        crate::profile::create("Mia", 0);
    }

    #[test]
    fn the_first_apple_is_straight_ahead() {
        fresh_player();
        let s = Snake::new();
        assert_eq!(s.food.y, s.body[0].y);
        assert_eq!(s.food.x - s.body[0].x, FIRST_FOOD_AHEAD);
    }

    #[test]
    fn a_first_timer_who_only_goes_straight_eats_the_first_apple() {
        fresh_player();
        let mut s = Snake::new();
        for _ in 0..FIRST_FOOD_AHEAD {
            s.tick();
        }
        assert_eq!(s.score, 1);
        assert_ne!(s.state, State::Dead);
    }

    #[test]
    fn a_first_timer_is_not_killed_by_an_early_self_bite() {
        fresh_player();
        let mut s = Snake::new();
        bitten(&mut s);
        assert_ne!(s.state, State::Dead);
        assert!(s.events.iter().any(|e| matches!(e, Event::Bonk(_))));
    }

    #[test]
    fn an_early_bite_bonks_instead_of_moving() {
        fresh_player();
        let mut s = Snake::new();
        s.body = (0..6).map(|i| ivec2(10 - i, 5)).collect();
        s.food = ivec2(0, 0);
        s.turn(Dir::Down);
        s.tick();
        s.turn(Dir::Left);
        s.tick();
        s.turn(Dir::Up);
        let before = s.body.clone();
        s.tick();
        assert_eq!(s.body, before);
        assert_eq!(s.dir, Dir::Left);
        assert!(s.events.iter().any(|e| matches!(e, Event::Bonk(_))));
        assert_eq!(s.state, State::Ready);
    }

    #[test]
    fn a_veteran_still_dies_from_a_bite_after_the_grace_period() {
        fresh_player();
        crate::profile::submit(GAME, 20, crate::profile::Better::Higher);
        let mut s = Snake::new();
        s.moves = GRACE_MOVES;
        bitten(&mut s);
        assert_eq!(s.state, State::Dead);
    }

    #[test]
    fn the_first_stretch_is_slow_and_speed_rises_by_stage() {
        fresh_player();
        let mut s = Snake::new();
        assert!(s.step_time() >= START_STEP - 1e-6);
        let mut last = s.step_time();
        for score in (0..60).step_by(STAGE_APPLES as usize) {
            s.score = score;
            let t = s.step_time();
            assert!(t <= last + 1e-6, "score {score}: {t} > {last}");
            last = t;
        }
        assert!(last < s.step_time() + 1e-6);
        s.score = 0;
        let start = s.step_time();
        s.score = 40;
        assert!(s.step_time() < start * 0.5);
        s.score = 500;
        assert!((s.step_time() - FASTEST_STEP).abs() < 1e-6);
    }

    #[test]
    fn every_stage_is_a_few_apples_and_cheers() {
        fresh_player();
        let mut s = Snake::new();
        s.score = STAGE_APPLES - 1;
        s.food = s.body[0] + ivec2(1, 0);
        s.tick();
        assert_eq!(s.stage(), 2);
        assert!(s.events.contains(&Event::Stage(2)));
        assert_eq!(s.fruit(), FRUIT[1]);
        assert_eq!(crate::profile::get(GAME, "stage"), 2);
    }

    #[test]
    fn the_furthest_stage_is_saved_per_player_and_resumes() {
        fresh_player();
        let mut s = Snake::new();
        s.score = STAGE_APPLES * 2 - 1;
        s.food = s.body[0] + ivec2(1, 0);
        s.tick();
        assert_eq!(Snake::new().best_stage, 3);
        crate::profile::create("Leo", 1);
        assert_eq!(Snake::new().best_stage, 1);
    }

    #[test]
    fn dying_submits_the_score_and_remembers_the_rank() {
        fresh_player();
        let mut s = Snake::new();
        s.wrap = false;
        s.score = 7;
        s.body = [ivec2(LONG - 1, 3), ivec2(LONG - 2, 3)]
            .into_iter()
            .collect();
        s.tick();
        assert_eq!(s.rank, Some(0));
        assert_eq!(crate::profile::personal_best(GAME), Some(7));
        assert_eq!(Snake::new().best.0, 7);
    }

    #[test]
    fn two_weak_runs_in_a_row_ease_the_next_start_and_a_good_run_restores_it() {
        fresh_player();
        let normal = Snake::new().step_time();
        for _ in 0..GENTLE_AFTER {
            let mut s = Snake::new();
            s.wrap = false;
            s.body = [ivec2(LONG - 1, 3), ivec2(LONG - 2, 3)]
                .into_iter()
                .collect();
            s.tick();
        }
        let easy = Snake::new();
        assert!(easy.gentle);
        assert!(easy.step_time() > normal);
        let mut s = Snake::new();
        s.wrap = false;
        s.score = GENTLE_LIMIT + 3;
        s.body = [ivec2(LONG - 1, 3), ivec2(LONG - 2, 3)]
            .into_iter()
            .collect();
        s.tick();
        assert!(!Snake::new().gentle);
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

    const DT: f32 = 1.0 / 60.0;

    fn phone() -> View {
        View::fit(1080.0, 2400.0)
    }

    fn playing_at(frac: f32) -> Snake {
        fresh_player();
        let v = phone();
        let mut s = Snake::new();
        s.update(&Input::default(), &v, DT);
        s.state = State::Playing;
        s.moves = 100;
        s.best = Best(30);
        s.food = ivec2(0, 0);
        while s.body[0].x < s.size.x / 3 + 2 {
            s.update(&Input::default(), &v, DT);
        }
        while !(frac..frac + 0.1).contains(&s.frac()) {
            s.update(&Input::default(), &v, DT);
        }
        s
    }

    fn up_by(s: &Snake, from: Vec2) -> f32 {
        from.y - s.drawn_head().y
    }

    #[test]
    fn an_early_turn_moves_the_drawn_head_the_same_frame() {
        let mut s = playing_at(0.2);
        let before = s.drawn_head();
        s.update(&Input::key(Dir::Up), &phone(), DT);
        assert_eq!(s.dir, Dir::Up);
        assert!(up_by(&s, before) > 0.02, "head {:?} -> {:?}", before, s.drawn_head());
        assert!((s.drawn_head() - before).length() < 0.3);
    }

    #[test]
    fn a_late_turn_swings_the_face_the_same_frame_and_lands_next_cell() {
        let mut s = playing_at(0.7);
        let face = s.face;
        s.update(&Input::key(Dir::Up), &phone(), DT);
        assert_eq!(s.heading(), Dir::Up);
        let up = angle_of(Dir::Up);
        assert!(wrap_angle(up - s.face).abs() < wrap_angle(up - face).abs() * 0.5);
        let start = s.body[0];
        for _ in 0..30 {
            s.update(&Input::default(), &phone(), DT);
        }
        assert!(s.body.contains(&(start + ivec2(0, -1))));
    }

    #[test]
    fn turns_queue_in_order_and_cap_at_three() {
        let mut s = playing_at(0.7);
        s.steer(Dir::Up);
        s.steer(Dir::Left);
        s.steer(Dir::Down);
        s.steer(Dir::Right);
        assert_eq!(s.queued, [Dir::Up, Dir::Left, Dir::Down]);
        let h = s.body[0];
        s.tick();
        s.tick();
        assert_eq!(s.body[0], h + ivec2(-1, -1));
    }

    #[test]
    fn no_turn_ever_reverses_into_the_neck() {
        for frac in [0.0, 0.1, 0.3, 0.6, 0.95] {
            for first in Dir::ALL {
                for second in Dir::ALL {
                    let mut s = playing_at(frac);
                    s.update(&Input::key(first), &phone(), DT);
                    s.update(&Input::key(second), &phone(), DT);
                    for _ in 0..40 {
                        s.update(&Input::default(), &phone(), DT);
                        let n = s.body.len();
                        let cells: std::collections::HashSet<_> = s.body.iter().collect();
                        assert_eq!(cells.len(), n, "overlap after {first:?} {second:?} at {frac}");
                        assert_ne!(s.state, State::Dead);
                    }
                }
            }
        }
    }

    #[test]
    fn the_reverse_of_the_way_in_is_ignored_even_just_after_a_turn() {
        let mut s = playing_at(0.7);
        s.steer(Dir::Up);
        s.tick();
        s.steer(Dir::Left);
        assert_eq!(s.dir, Dir::Up);
        assert_eq!(s.queued, [Dir::Left]);
        let neck = s.body[1];
        s.tick();
        assert_ne!(s.body[0], neck);
    }

    #[test]
    fn a_swipe_fires_while_the_finger_is_still_moving() {
        let mut s = playing_at(0.7);
        let v = phone();
        let mut sc = Script::new(v);
        let from = vec2(500.0, 400.0);
        s.update(&sc.press(from), &v, DT);
        let step = SNAKE_SWIPE_MM * 1.2 * sc_mm(&v);
        s.update(&sc.drag_to(from + vec2(0.0, -step)), &v, DT);
        assert_eq!(s.heading(), Dir::Up);
    }

    fn sc_mm(v: &View) -> f32 {
        6.0 / v.scale
    }

    #[test]
    fn a_tap_on_a_half_turns_relative_to_the_heading() {
        let v = phone();
        for (x, want) in [(v.w * 0.2, Dir::Up), (v.w * 0.8, Dir::Down)] {
            let mut s = playing_at(0.7);
            let mut sc = Script::new(v);
            for f in sc.tap(vec2(x, v.h * 0.5)) {
                s.update(&f, &v, DT);
            }
            assert_eq!(s.heading(), want);
        }
        let mut s = playing_at(0.7);
        s.dir = Dir::Up;
        let mut sc = Script::new(v);
        for f in sc.tap(vec2(v.w * 0.2, v.h * 0.5)) {
            s.update(&f, &v, DT);
        }
        assert_eq!(s.heading(), Dir::Left);
    }

    #[test]
    fn a_gamepad_stick_turns_once_per_push() {
        let mut s = playing_at(0.7);
        let v = phone();
        s.update(&Input::held(vec2(0.0, -1.0)), &v, DT);
        s.update(&Input::held(vec2(0.0, -1.0)), &v, DT);
        assert_eq!(s.queued, [Dir::Up]);
    }

    #[test]
    fn the_starting_swipe_moves_the_snake_within_a_few_frames() {
        fresh_player();
        let v = phone();
        let mut s = Snake::new();
        s.update(&Input::default(), &v, DT);
        let before = s.drawn_head();
        s.update(&Input::key(Dir::Up), &v, DT);
        let mut frames = 1;
        while (s.drawn_head() - before).length() < 0.05 && frames < 60 {
            s.update(&Input::default(), &v, DT);
            frames += 1;
        }
        assert!(frames <= 6, "{frames} frames");
    }

    #[test]
    fn a_head_about_to_crash_waits_a_moment_so_a_late_turn_still_saves_it() {
        fresh_player();
        crate::profile::submit(GAME, 20, crate::profile::Better::Higher);
        let v = phone();
        let mut s = Snake::sized(Snake::size_for(&v));
        s.state = State::Playing;
        s.wrap = false;
        s.moves = GRACE_MOVES;
        let w = s.size.x;
        s.body = (0..4).map(|i| ivec2(w - 1 - i, 5)).collect();
        s.food = ivec2(0, 0);
        let step = s.step_time();
        for _ in 0..((step / DT) as usize + 3) {
            s.update(&Input::default(), &v, DT);
        }
        assert_eq!(s.state, State::Playing);
        s.update(&Input::key(Dir::Down), &v, DT);
        for _ in 0..30 {
            s.update(&Input::default(), &v, DT);
        }
        assert_eq!(s.state, State::Playing);
        assert_eq!(s.body[0].x, w - 1);
    }

    #[test]
    fn the_crash_pause_shrinks_as_stages_rise() {
        let mut s = Snake::new();
        let first = s.brink_time();
        s.score = STAGE_APPLES * 6;
        assert!(s.brink_time() < first);
        assert!(s.brink_time() >= 0.0);
    }

    #[test]
    fn the_mouth_opens_as_the_fruit_comes_close_ahead() {
        fresh_player();
        let mut s = Snake::new();
        s.food = s.body[0] + ivec2(6, 0);
        let far = s.mouth();
        s.food = s.body[0] + ivec2(1, 0);
        assert!(s.mouth() > far + 0.5);
        assert!(s.look().x > 0.9);
    }

    #[test]
    fn blinks_are_short_and_rare() {
        let shut = (0..370).filter(|i| blink(*i as f32 / 100.0) > 0.5).count();
        assert!(shut > 0 && shut < 20, "{shut}");
    }

    struct Bot {
        swipe: Option<(Dir, u8)>,
    }

    fn bot_input(bot: &mut Bot, s: &Snake, sc: &mut Script, v: &View, slow: bool, frame: usize) -> Input {
        let mid = vec2(v.w / 2.0, v.h / 2.0);
        if let Some((d, k)) = bot.swipe {
            bot.swipe = (k < 2).then_some((d, k + 1));
            return match k {
                0 => sc.drag_to(mid + d.delta().as_vec2() * 2.0 * sc_mm(v)),
                1 => sc.drag_to(mid + d.delta().as_vec2() * 5.0 * sc_mm(v)),
                _ => sc.release(),
            };
        }
        let head = s.body[0];
        let size = s.size;
        let wrapped = |p: IVec2| ivec2(p.x.rem_euclid(size.x), p.y.rem_euclid(size.y));
        let free = |d: Dir| {
            let at = wrapped(head + d.delta());
            d != s.dir.opposite() && !s.body.iter().take(s.body.len() - 1).any(|b| *b == at)
        };
        let gap = |d: Dir| {
            let at = wrapped(head + d.delta());
            let dx = (s.food.x - at.x).abs();
            let dy = (s.food.y - at.y).abs();
            dx.min(size.x - dx) + dy.min(size.y - dy)
        };
        let room = |d: Dir| {
            let start = wrapped(head + d.delta());
            let blocked: std::collections::HashSet<IVec2> =
                s.body.iter().take(s.body.len() - 1).copied().collect();
            let mut seen = std::collections::HashSet::from([start]);
            let mut todo = vec![start];
            while let Some(p) = todo.pop() {
                if seen.len() > s.body.len() * 2 {
                    break;
                }
                for n in Dir::ALL {
                    let q = wrapped(p + n.delta());
                    if !blocked.contains(&q) && seen.insert(q) {
                        todo.push(q);
                    }
                }
            }
            seen.len() > s.body.len() * 2
        };
        let want = if slow && frame % 30 != 0 {
            s.heading()
        } else if slow && (frame / 30) % 5 == 4 {
            Dir::ALL
                .into_iter()
                .filter(|d| free(*d))
                .max_by_key(|d| gap(*d))
                .unwrap_or(s.dir)
        } else {
            Dir::ALL
                .into_iter()
                .filter(|d| free(*d))
                .min_by_key(|d| (!room(*d), gap(*d)))
                .unwrap_or(s.dir)
        };
        if want != s.heading() && s.queued.is_empty() {
            bot.swipe = Some((want, 0));
            return sc.press(mid);
        }
        sc.idle()
    }

    fn play(slow: bool, frames: usize) -> Snake {
        let v = phone();
        let mut s = Snake::new();
        let mut sc = Script::new(v);
        let mut bot = Bot { swipe: None };
        s.update(&Input::key(Dir::Right), &v, DT);
        for f in 0..frames {
            let i = bot_input(&mut bot, &s, &mut sc, &v, slow, f);
            s.update(&i, &v, DT);
            if s.state == State::Dead {
                break;
            }
        }
        s
    }

    #[test]
    fn a_strong_swiping_player_climbs_stages_and_the_game_speeds_up() {
        fresh_player();
        let s = play(false, 60 * 240);
        assert!(s.stage() >= 5, "stage {} score {}", s.stage(), s.score);
        assert!(s.step_time() < START_STEP * 0.75);
    }

    #[test]
    fn a_weak_player_still_eats_and_gets_an_easier_start_after_two_poor_runs() {
        fresh_player();
        let first = play(true, 60 * 120);
        assert!(first.score >= 3, "score {} {:?}", first.score, first.state);
        for _ in 0..GENTLE_AFTER {
            let mut s = Snake::new();
            s.wrap = false;
            s.best = Best(9);
            s.moves = 99;
            s.body = [ivec2(LONG - 1, 3), ivec2(LONG - 2, 3)].into_iter().collect();
            s.tick();
        }
        let s = Snake::new();
        assert!(s.gentle && s.step_time() > START_STEP);
    }
}
