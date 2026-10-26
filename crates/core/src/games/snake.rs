use crate::engine::*;
use crate::feel::{self, Impact};
use crate::fx::*;
use crate::geom::*;
use crate::sfx::{self, Sfx};
use glam::{ivec2, vec2, IVec2, Vec2};
use quad_rand as rand;
use std::collections::VecDeque;

pub const LONG: i32 = 24;
pub const SHORT: i32 = 18;
pub const START_STEP: f32 = 0.30;
pub const FASTEST_STEP: f32 = 0.09;
pub const STAGE_DROP: f32 = 0.025;
pub const STAGE_APPLES: u32 = 5;
pub const GRACE_MOVES: u32 = 16;
pub const GENTLE_GRACE_MOVES: u32 = 28;
pub const GENTLE_SLOWDOWN: f32 = 1.25;
pub const GENTLE_LIMIT: u32 = 4;
pub const GENTLE_AFTER: i64 = 2;
pub const FIRST_RUN_SAFE_APPLES: u32 = 3;
pub const RESULTS_DELAY: f32 = 0.6;
pub const GAME: &str = "snake";
pub const FIRST_FOOD_AHEAD: i32 = 6;

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

    pub fn tick(&mut self) {
        self.prev = self.body.iter().copied().collect();
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
            self.body.pop_back();
        }
        self.body.push_front(head);
        if eating {
            self.score += 1;
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
        if view.h > view.w {
            ivec2(SHORT, LONG)
        } else {
            ivec2(LONG, SHORT)
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
                if input.tap() && self.dead_age > RESULTS_DELAY {
                    let best = self.best;
                    *self = Self::sized(Self::size_for(view));
                    self.best = best;
                }
            }
        }
        self.react(view);
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
        let b = std::fs::read(concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets/art/snake.kga")).unwrap();
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
        assert!(s.step_time() >= 0.28);
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
}
