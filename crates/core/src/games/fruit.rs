use crate::engine::*;
use crate::feel::{self, Impact};
use crate::fx::*;
use crate::geom::*;
use crate::sfx::{self, Sfx};
use glam::{vec2, Vec2};
use quad_rand as rand;
pub const GRAVITY: f32 = 900.0;
pub const LIVES: u32 = 3;
pub const BLADE_LIFE: f32 = 0.15;
pub const MIN_SLICE_SPEED: f32 = 4.0;
pub const STAIN_LIFE: f32 = 6.0;
const MAX_STAINS: usize = 36;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Kind {
    Fruit(usize),
    Bomb,
}

pub struct Thing {
    pub pos: Vec2,
    pub vel: Vec2,
    pub r: f32,
    pub kind: Kind,
    pub spin: f32,
}

pub struct Half {
    pub pos: Vec2,
    pub vel: Vec2,
    pub r: f32,
    pub fruit: usize,
    pub angle: f32,
    pub spin: f32,
}

pub struct Stain {
    pub pos: Vec2,
    pub r: f32,
    pub color: Color,
    pub age: f32,
    pub drops: [Vec2; 3],
    pub turn: f32,
}

pub type Rgb = (u8, u8, u8);

pub struct Species {
    pub name: &'static str,
    pub skin: Rgb,
    pub flesh: Rgb,
    pub seed: Option<Rgb>,
}

pub const FRUITS: [Species; 6] = [
    Species {
        name: "apple",
        skin: (220, 50, 50),
        flesh: (255, 232, 205),
        seed: Some((90, 50, 30)),
    },
    Species {
        name: "orange",
        skin: (250, 150, 30),
        flesh: (255, 190, 70),
        seed: None,
    },
    Species {
        name: "watermelon",
        skin: (60, 150, 80),
        flesh: (236, 70, 84),
        seed: Some((30, 25, 25)),
    },
    Species {
        name: "pineapple",
        skin: (240, 190, 60),
        flesh: (252, 220, 90),
        seed: None,
    },
    Species {
        name: "lemon",
        skin: (250, 220, 60),
        flesh: (255, 240, 150),
        seed: Some((235, 215, 140)),
    },
    Species {
        name: "strawberry",
        skin: (220, 60, 70),
        flesh: (240, 92, 104),
        seed: Some((250, 230, 120)),
    },
];

pub const GAME: &str = "fruit";
pub const FRUIT_PER_LEVEL: u32 = 10;
pub const FIRST_BOMB_LEVEL: u32 = 4;
pub const MAX_START_LEVEL: u32 = 3;
const _: () = assert!(MAX_START_LEVEL < FIRST_BOMB_LEVEL);
const CALM_WAVES: u32 = 3;
pub const FRUIT_SPAN: f32 = 2.3;
pub const BOMB_SPAN: f32 = 2.5;

pub fn start_level(saved: u32) -> u32 {
    (saved / 2).clamp(1, MAX_START_LEVEL)
}

pub fn bomb_chance(level: u32) -> f32 {
    if level < FIRST_BOMB_LEVEL {
        0.0
    } else {
        (0.06 + 0.02 * (level - FIRST_BOMB_LEVEL) as f32).min(0.25)
    }
}

pub fn wave_size(level: u32) -> (usize, usize) {
    let d = level.saturating_sub(1) as usize;
    let lo = 1 + d / 5;
    let hi = (1 + d / 2).max(lo);
    (lo.min(5), hi.min(6))
}

pub fn wave_gap(level: u32) -> f32 {
    (1.7 - 0.08 * level.saturating_sub(1) as f32).max(0.6)
}

pub fn fruit_radius_bonus(level: u32) -> f32 {
    (12.0 - 4.0 * level.saturating_sub(1) as f32).max(0.0)
}

pub fn misses_cost_lives(level: u32) -> bool {
    level > 1
}

pub fn combo_bonus(n: u32) -> u32 {
    if n >= 3 {
        n
    } else {
        0
    }
}

pub fn combo_label(n: u32) -> Option<String> {
    (n >= 3).then(|| format!("{}x Combo!", n))
}

pub fn stain_alpha(age: f32) -> f32 {
    let fade = ((age - STAIN_LIFE * 0.5) / (STAIN_LIFE * 0.5)).clamp(0.0, 1.0);
    0.62 * (1.0 - fade) * ease_out_cubic(age / 0.12)
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum State {
    Ready,
    Playing,
    Over,
}

pub struct Fruit {
    pub things: Vec<Thing>,
    pub halves: Vec<Half>,
    pub stains: Vec<Stain>,
    pub blade: Vec<(Vec2, f32)>,
    pub score: u32,
    pub best: Best,
    pub rank: Option<usize>,
    pub lives: u32,
    pub state: State,
    pub sliced: u32,
    pub start_level: u32,
    pub level_up_pending: bool,
    pub calm: u32,
    pub bomb_hint_pending: bool,
    pub bomb_hint_time: f32,
    pub wave_in: f32,
    pub time: f32,
    pub flash: f32,
    pub fx: Particles,
    pub shake: Shake,
    pub popups: Popups,
    pub score_bump: Bump,
    pub heart_bump: Bump,
}

pub fn mix(a: Color, b: Color, k: f32) -> Color {
    a.lerp(b, k)
}

pub fn rgb(c: Rgb) -> Color {
    Color::from_rgba(c.0, c.1, c.2, 255)
}

impl Fruit {
    pub fn new() -> Self {
        Self {
            things: vec![],
            halves: vec![],
            stains: vec![],
            blade: vec![],
            score: 0,
            best: Best::load(GAME),
            rank: None,
            lives: LIVES,
            state: State::Ready,
            sliced: 0,
            start_level: start_level(crate::profile::level(GAME)),
            level_up_pending: false,
            calm: 0,
            bomb_hint_pending: crate::profile::get(GAME, "bomb_hint") == 0,
            bomb_hint_time: 0.0,
            wave_in: 0.8,
            time: 0.0,
            flash: 0.0,
            fx: Particles::default(),
            shake: Shake::default(),
            popups: Popups::default(),
            score_bump: Bump::default(),
            heart_bump: Bump::default(),
        }
    }

    pub fn level(&self) -> u32 {
        self.start_level + self.sliced / FRUIT_PER_LEVEL
    }

    fn launch_wave(&mut self, view: &View) {
        let level = self.level();
        let (lo, hi) = wave_size(level);
        let calm = self.calm > 0;
        self.calm = self.calm.saturating_sub(1);
        let count = if calm { 1 } else { rand::gen_range(lo, hi + 1) };
        let bomb_chance = if calm { 0.0 } else { bomb_chance(level) };
        let bonus = fruit_radius_bonus(level);
        for _ in 0..count {
            let x = rand::gen_range(view.w * 0.15, view.w * 0.85);
            let peak = rand::gen_range(view.h * 0.12 + bonus * 4.0, view.h * 0.45);
            let vy = -(2.0 * GRAVITY * (view.h + 40.0 - peak)).sqrt();
            let vx = (view.w / 2.0 - x) * rand::gen_range(0.2, 0.6);
            let kind = if rand::gen_range(0.0, 1.0) < bomb_chance {
                Kind::Bomb
            } else {
                Kind::Fruit(rand::gen_range(0, FRUITS.len()))
            };
            self.things.push(Thing {
                pos: vec2(x, view.h + 40.0),
                vel: vec2(vx, vy),
                r: if kind == Kind::Bomb {
                    34.0
                } else {
                    rand::gen_range(34.0, 46.0) + bonus
                },
                kind,
                spin: rand::gen_range(-3.0, 3.0),
            });
        }
    }

    fn splash(&mut self, at: Vec2, fruit: usize, dir: Vec2) {
        let flesh = rgb(FRUITS[fruit].flesh);
        let skin = rgb(FRUITS[fruit].skin);
        let angle = dir.y.atan2(dir.x);
        for side in [0.0, std::f32::consts::PI] {
            self.fx.emit(
                at,
                &[flesh, flesh, skin],
                Burst {
                    count: 9,
                    speed: (160.0, 420.0),
                    size: (3.0, 7.0),
                    life: (0.4, 0.8),
                    gravity: GRAVITY,
                    drag: 1.2,
                    spread: 1.4,
                    angle: angle + std::f32::consts::FRAC_PI_2 + side,
                    ..Default::default()
                },
            );
        }
        if let Some(seed) = FRUITS[fruit].seed {
            self.fx.emit(
                at,
                &[rgb(seed)],
                Burst {
                    count: 4,
                    speed: (100.0, 300.0),
                    size: (2.5, 4.0),
                    life: (0.6, 1.0),
                    gravity: GRAVITY,
                    drag: 0.8,
                    ..Default::default()
                },
            );
        }
        if self.stains.len() >= MAX_STAINS {
            self.stains.remove(0);
        }
        self.stains.push(Stain {
            pos: at,
            r: rand::gen_range(34.0, 52.0),
            color: mix(flesh, skin, 0.45),
            age: 0.0,
            turn: rand::gen_range(0.0, std::f32::consts::TAU),
            drops: [0, 1, 2].map(|_| {
                vec2(rand::gen_range(-1.0, 1.0), rand::gen_range(-1.0, 1.0))
                    * rand::gen_range(38.0, 80.0)
            }),
        });
    }

    fn explode(&mut self, at: Vec2) {
        sfx::play(Sfx::Boom);
        feel::impact(Impact::Large);
        self.shake.add(1.0);
        self.flash = 1.0;
        self.fx.emit(
            at,
            &[
                Color::from_rgba(255, 200, 60, 255),
                Color::from_rgba(255, 120, 40, 255),
                Color::from_rgba(70, 70, 80, 255),
                WHITE,
            ],
            Burst {
                count: 34,
                speed: (150.0, 560.0),
                size: (6.0, 14.0),
                life: (0.5, 1.0),
                gravity: 300.0,
                drag: 2.0,
                shape: Shape::Star,
                ..Default::default()
            },
        );
        self.fx.puff(at, Color::from_rgba(60, 60, 70, 255));
    }

    fn level_up(&mut self, view: &View) {
        let level = self.level();
        if level > crate::profile::level(GAME) {
            crate::profile::set_level(GAME, level);
        }
        sfx::play(Sfx::LevelUp);
        sfx::say(sfx::Say::LevelUp);
        let gold = Color::from_rgba(255, 210, 60, 255);
        feel::impact_tinted(Impact::Large, gold);
        self.fx.confetti(view);
        self.popups.add(
            vec2(view.w / 2.0, view.h * 0.4),
            format!("Level {}!", level),
            gold,
            76.0,
        );
        if self.lives < LIVES {
            self.lives += 1;
            self.heart_bump.kick();
        }
    }

    fn bomb_on_screen(&self) -> bool {
        self.things.iter().any(|t| t.kind == Kind::Bomb)
    }

    pub fn slice(&mut self, a: Vec2, b: Vec2) {
        if a.distance(b) < MIN_SLICE_SPEED {
            return;
        }
        let dir = (b - a).normalize_or_zero();
        let mut i = 0;
        let mut combo = 0;
        let mut centre = Vec2::ZERO;
        while i < self.things.len() {
            let t = &self.things[i];
            if !segment_hits_circle(a, b, t.pos, t.r) {
                i += 1;
                continue;
            }
            let t = self.things.swap_remove(i);
            match t.kind {
                Kind::Bomb => {
                    self.lives = 0;
                    self.explode(t.pos);
                }
                Kind::Fruit(f) => {
                    sfx::play(Sfx::Slice);
                    combo += 1;
                    centre += t.pos;
                    self.score += 1;
                    self.sliced += 1;
                    if self.sliced.is_multiple_of(FRUIT_PER_LEVEL) {
                        self.level_up_pending = true;
                    }
                    self.score_bump.kick();
                    let side = vec2(-dir.y, dir.x) * 120.0;
                    let angle = dir.y.atan2(dir.x) - std::f32::consts::FRAC_PI_2;
                    for s in [-1.0, 1.0] {
                        self.halves.push(Half {
                            pos: t.pos,
                            vel: t.vel * 0.4 + side * s,
                            r: t.r,
                            fruit: f,
                            angle,
                            spin: s * 2.5,
                        });
                    }
                    self.splash(t.pos, f, dir);
                    self.popups.add(
                        t.pos + vec2(0.0, -t.r),
                        "+1",
                        Color::from_rgba(255, 220, 60, 255),
                        34.0,
                    );
                }
            }
        }
        self.score += combo_bonus(combo);
        if let Some(label) = combo_label(combo) {
            centre /= combo as f32;
            self.popups.add(
                centre + vec2(0.0, -90.0),
                label,
                Color::from_rgba(255, 140, 60, 255),
                54.0,
            );
            sfx::play_step(Sfx::Right, combo.saturating_sub(3) as usize);
            feel::impact(Impact::Medium);
            self.fx.sparkle(centre, Color::from_rgba(255, 220, 60, 255));
            self.shake.add(0.25);
        }
    }
}

impl Default for Fruit {
    fn default() -> Self {
        Self::new()
    }
}

impl Game for Fruit {
    fn update(&mut self, input: &Input, view: &View, dt: f32) {
        self.flash = (self.flash - dt * 2.0).max(0.0);
        self.fx.update(dt);
        self.shake.update(dt);
        self.popups.update(dt);
        self.score_bump.update(dt);
        self.heart_bump.update(dt);
        for st in &mut self.stains {
            st.age += dt;
        }
        self.stains.retain(|st| st.age < STAIN_LIFE);
        for b in &mut self.blade {
            b.1 -= dt;
        }
        self.blade.retain(|b| b.1 > 0.0);
        if input.down {
            if input.pressed {
                self.blade.clear();
            }
            self.blade.push((input.pos, BLADE_LIFE));
        }
        for h in &mut self.halves {
            h.vel.y += GRAVITY * dt;
            h.pos += h.vel * dt;
            h.angle += h.spin * dt;
        }
        for h in &self.halves {
            if rand::gen_range(0.0, 1.0) < dt * 14.0 {
                let c = rgb(FRUITS[h.fruit].flesh);
                self.fx.emit(
                    h.pos,
                    &[c],
                    Burst {
                        count: 1,
                        speed: (10.0, 60.0),
                        size: (2.5, 4.5),
                        life: (0.3, 0.6),
                        gravity: GRAVITY,
                        ..Default::default()
                    },
                );
            }
        }
        self.halves.retain(|h| h.pos.y < view.h + 100.0);
        match self.state {
            State::Ready => {
                if input.tap() {
                    self.state = State::Playing;
                    sfx::say(sfx::Say::Go);
                }
            }
            State::Playing => {
                self.time += dt;
                if input.down && !input.pressed {
                    self.slice(input.prev_pos, input.pos);
                }
                for t in &mut self.things {
                    t.vel.y += GRAVITY * dt;
                    t.pos += t.vel * dt;
                }
                let mut missed = 0;
                self.things.retain(|t| {
                    let gone = t.vel.y > 0.0 && t.pos.y > view.h + t.r + 10.0;
                    if gone && matches!(t.kind, Kind::Fruit(_)) {
                        missed += 1;
                    }
                    !gone
                });
                if missed > 0 && misses_cost_lives(self.level()) {
                    sfx::play(Sfx::Wrong);
                    self.heart_bump.kick();
                    self.shake.add(0.2);
                    self.calm = CALM_WAVES;
                    self.lives = self.lives.saturating_sub(missed);
                }
                if self.level_up_pending {
                    self.level_up_pending = false;
                    self.level_up(view);
                }
                if self.bomb_hint_pending && self.bomb_on_screen() {
                    self.bomb_hint_time += dt;
                    if self.bomb_hint_time > 4.0 {
                        self.bomb_hint_pending = false;
                        crate::profile::set(GAME, "bomb_hint", 1);
                    }
                }
                self.wave_in -= dt;
                if self.wave_in <= 0.0 && self.things.is_empty() {
                    self.launch_wave(view);
                    self.wave_in = wave_gap(self.level()) * 0.6;
                }
                if self.lives == 0 {
                    self.state = State::Over;
                    sfx::play(Sfx::Lose);
                    if self.score > self.best.0 && self.best.0 > 0 {
                        sfx::say(sfx::Say::Best);
                    }
                    self.rank = self.best.submit(GAME, self.score);
                }
            }
            State::Over => {
                if input.tap() && self.flash == 0.0 {
                    *self = Self::new();
                    self.state = State::Playing;
                    sfx::say(sfx::Say::Go);
                }
            }
        }
    }
}

pub fn fit_in(s: Vec2, side: f32) -> (f32, f32) {
    let k = side / s.x.max(s.y).max(1.0);
    (s.x * k, s.y * k)
}

pub const SPLAT: &str = "splat0";

pub fn sprite_names() -> Vec<String> {
    let mut want: Vec<String> = ["wall", "bomb", "heart", SPLAT]
        .iter()
        .map(|s| s.to_string())
        .collect();
    for f in FRUITS.iter() {
        for suffix in ["", "_half"] {
            want.push(format!("{}{}", f.name, suffix));
        }
    }
    want
}

impl Fruit {
    pub fn show_swipe_hint(&self) -> bool {
        self.best.0 == 0 && self.sliced < 3
    }

    pub fn show_bomb_hint(&self) -> bool {
        !self.show_swipe_hint() && self.bomb_hint_pending && self.bomb_on_screen()
    }

    pub fn level_progress(&self) -> f32 {
        (self.sliced % FRUIT_PER_LEVEL) as f32 / FRUIT_PER_LEVEL as f32
    }

    pub fn shows_results(&self) -> bool {
        self.state == State::Over
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn with(kind: Kind, at: Vec2) -> Fruit {
        let mut g = Fruit::new();
        g.state = State::Playing;
        g.things.push(Thing {
            pos: at,
            vel: Vec2::ZERO,
            r: 40.0,
            kind,
            spin: 0.0,
        });
        g
    }

    #[test]
    fn fruit_sprites_all_exist() {
        let b = std::fs::read(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../assets/art/fruit.kga"
        ))
        .unwrap();
        let idx = crate::art::parse(&b).unwrap();
        for n in sprite_names() {
            assert!(idx.frames.contains_key(&n), "missing {n}");
        }
    }

    #[test]
    fn slicing_splits_into_two_halves_that_part_ways() {
        let mut g = with(Kind::Fruit(2), vec2(300.0, 300.0));
        g.slice(vec2(200.0, 300.0), vec2(400.0, 300.0));
        assert_eq!(g.halves.len(), 2);
        assert!(g.halves[0].spin * g.halves[1].spin < 0.0);
        assert!(g.halves[0].vel.y * g.halves[1].vel.y < 0.0);
    }

    #[test]
    fn losing_plays_the_lose_jingle() {
        let v = View::ipad_landscape();
        let mut g = with(Kind::Fruit(1), vec2(300.0, v.h + 100.0));
        g.start_level = 2;
        g.lives = 1;
        g.things[0].vel = vec2(0.0, 10.0);
        crate::sfx::take_pending();
        g.update(&Input::default(), &v, 0.01);
        assert!(crate::sfx::take_pending().contains(&Sfx::Lose));
    }

    #[test]
    fn swipe_through_fruit_scores() {
        let mut g = with(Kind::Fruit(0), vec2(300.0, 300.0));
        g.slice(vec2(200.0, 290.0), vec2(400.0, 310.0));
        assert_eq!(g.score, 1);
        assert!(g.things.is_empty());
    }

    #[test]
    fn swipe_that_misses_does_nothing() {
        let mut g = with(Kind::Fruit(0), vec2(300.0, 300.0));
        g.slice(vec2(200.0, 400.0), vec2(400.0, 400.0));
        assert_eq!(g.score, 0);
        assert_eq!(g.things.len(), 1);
    }

    #[test]
    fn resting_finger_does_not_slice() {
        let mut g = with(Kind::Fruit(0), vec2(300.0, 300.0));
        g.slice(vec2(300.0, 300.0), vec2(301.0, 300.0));
        assert_eq!(g.score, 0);
    }

    #[test]
    fn slicing_a_bomb_ends_the_game() {
        let mut g = with(Kind::Bomb, vec2(300.0, 300.0));
        g.slice(vec2(200.0, 300.0), vec2(400.0, 300.0));
        g.update(&Input::default(), &View::ipad_landscape(), 0.01);
        assert_eq!(g.state, State::Over);
    }

    #[test]
    fn dropped_fruit_costs_a_life() {
        let v = View::ipad_landscape();
        let mut g = with(Kind::Fruit(1), vec2(300.0, v.h + 100.0));
        g.start_level = 2;
        g.things[0].vel = vec2(0.0, 10.0);
        g.update(&Input::default(), &v, 0.01);
        assert_eq!(g.lives, LIVES - 1);
    }

    #[test]
    fn combo_of_three_pays_bonus_and_announces() {
        assert_eq!(combo_bonus(2), 0);
        assert_eq!(combo_bonus(3), 3);
        assert_eq!(combo_label(2), None);
        assert_eq!(combo_label(4).as_deref(), Some("4x Combo!"));
        let mut g = Fruit::new();
        g.state = State::Playing;
        for x in [200.0, 300.0, 400.0] {
            g.things.push(Thing {
                pos: vec2(x, 300.0),
                vel: Vec2::ZERO,
                r: 40.0,
                kind: Kind::Fruit(0),
                spin: 0.0,
            });
        }
        g.slice(vec2(100.0, 300.0), vec2(500.0, 300.0));
        assert_eq!(g.score, 6);
        assert!(g.popups.list.iter().any(|p| p.text == "3x Combo!"));
    }

    #[test]
    fn slicing_leaves_a_stain_that_fades_away() {
        let mut g = with(Kind::Fruit(2), vec2(300.0, 300.0));
        g.slice(vec2(200.0, 300.0), vec2(400.0, 300.0));
        assert_eq!(g.stains.len(), 1);
        assert!(!g.fx.list.is_empty());
        let v = View::ipad_landscape();
        for _ in 0..((STAIN_LIFE + 1.0) * 60.0) as usize {
            g.update(&Input::default(), &v, 1.0 / 60.0);
        }
        assert!(g.stains.is_empty());
    }

    #[test]
    fn stain_alpha_fades_in_then_out() {
        assert_eq!(stain_alpha(0.0), 0.0);
        let mid = stain_alpha(STAIN_LIFE * 0.4);
        assert!(mid > 0.5);
        assert!(stain_alpha(STAIN_LIFE * 0.8) < mid);
        assert!(stain_alpha(STAIN_LIFE).abs() < 1e-4);
    }

    #[test]
    fn stains_are_capped() {
        let mut g = Fruit::new();
        for _ in 0..(MAX_STAINS * 2) {
            g.splash(vec2(300.0, 300.0), 0, vec2(1.0, 0.0));
        }
        assert_eq!(g.stains.len(), MAX_STAINS);
    }

    #[test]
    fn bomb_shakes_and_flashes() {
        let mut g = with(Kind::Bomb, vec2(300.0, 300.0));
        g.slice(vec2(200.0, 300.0), vec2(400.0, 300.0));
        assert!(g.shake.active());
        assert_eq!(g.flash, 1.0);
        assert!(!g.fx.list.is_empty());
    }

    #[test]
    fn missed_fruit_pulses_the_heart() {
        let v = View::ipad_landscape();
        let mut g = with(Kind::Fruit(1), vec2(300.0, v.h + 100.0));
        g.start_level = 2;
        g.things[0].vel = vec2(0.0, 10.0);
        g.update(&Input::default(), &v, 0.01);
        assert!(g.heart_bump.t < 1.0);
    }

    fn fresh_player() -> Fruit {
        crate::save::reset_for_test();
        crate::profile::create("Mia", 0);
        Fruit::new()
    }

    fn start(g: &mut Fruit, v: &View) {
        g.update(&Input::press_at(vec2(100.0, 100.0)), v, 0.01);
        assert_eq!(g.state, State::Playing);
    }

    #[test]
    fn a_child_who_never_swipes_survives_the_first_stretch() {
        let v = View::ipad_landscape();
        let mut g = fresh_player();
        start(&mut g, &v);
        for _ in 0..(40 * 60) {
            g.update(&Input::default(), &v, 1.0 / 60.0);
            assert!(!g.things.iter().any(|t| t.kind == Kind::Bomb));
        }
        assert_eq!(g.state, State::Playing);
        assert_eq!(g.lives, LIVES);
        assert_eq!(g.level(), 1);
    }

    #[test]
    fn no_bombs_before_level_four() {
        let v = View::ipad_landscape();
        for level in 1..FIRST_BOMB_LEVEL {
            assert_eq!(bomb_chance(level), 0.0);
            let mut g = fresh_player();
            g.start_level = level;
            g.state = State::Playing;
            for _ in 0..200 {
                g.things.clear();
                g.launch_wave(&v);
                assert!(!g.things.iter().any(|t| t.kind == Kind::Bomb));
            }
        }
        assert!(bomb_chance(FIRST_BOMB_LEVEL) > 0.0);
    }

    #[test]
    fn difficulty_rises_with_level() {
        assert!(bomb_chance(8) > bomb_chance(4));
        assert!(bomb_chance(99) <= 0.25);
        assert!(wave_size(10).1 > wave_size(1).1);
        assert!(wave_gap(10) < wave_gap(1));
        assert!(wave_gap(99) >= 0.6);
        assert!(fruit_radius_bonus(1) > fruit_radius_bonus(3));
        assert_eq!(fruit_radius_bonus(5), 0.0);
        assert!(!misses_cost_lives(1));
        assert!(misses_cost_lives(2));
    }

    #[test]
    fn first_level_waves_are_one_big_fruit() {
        let v = View::ipad_landscape();
        let mut g = fresh_player();
        for _ in 0..100 {
            g.things.clear();
            g.launch_wave(&v);
            assert_eq!(g.things.len(), 1);
            assert!(g.things[0].r >= 44.0);
        }
    }

    #[test]
    fn ten_fruit_reach_the_next_level_with_a_cheer() {
        let v = View::ipad_landscape();
        let mut g = fresh_player();
        g.start_level = 2;
        start(&mut g, &v);
        g.lives = 1;
        crate::sfx::take_pending();
        for _ in 0..FRUIT_PER_LEVEL {
            g.things.push(Thing {
                pos: vec2(300.0, 300.0),
                vel: Vec2::ZERO,
                r: 40.0,
                kind: Kind::Fruit(0),
                spin: 0.0,
            });
            g.slice(vec2(200.0, 300.0), vec2(400.0, 300.0));
        }
        g.update(&Input::default(), &v, 0.01);
        assert_eq!(g.level(), 3);
        assert!(crate::sfx::take_pending().contains(&Sfx::LevelUp));
        assert!(g.popups.list.iter().any(|p| p.text == "Level 3!"));
        assert_eq!(g.lives, 2);
        assert_eq!(crate::profile::level(GAME), 3);
    }

    #[test]
    fn a_player_who_slices_every_fruit_and_dodges_bombs_climbs_levels() {
        let v = View::ipad_landscape();
        let mut g = fresh_player();
        start(&mut g, &v);
        for _ in 0..(120 * 60) {
            let cuts: Vec<(Vec2, Vec2)> = g
                .things
                .iter()
                .filter(|t| matches!(t.kind, Kind::Fruit(_)) && t.pos.y < v.h - 40.0)
                .map(|t| (t.pos - vec2(30.0, 0.0), t.pos + vec2(30.0, 0.0)))
                .collect();
            for (a, b) in cuts {
                let safe = !g
                    .things
                    .iter()
                    .any(|t| t.kind == Kind::Bomb && segment_hits_circle(a, b, t.pos, t.r));
                if safe {
                    g.slice(a, b);
                }
            }
            g.update(&Input::default(), &v, 1.0 / 60.0);
            if g.state == State::Over {
                break;
            }
        }
        assert_eq!(g.state, State::Playing);
        assert!(g.level() >= 4, "level {}", g.level());
        assert!(g.lives >= 1);
    }

    #[test]
    fn a_missed_fruit_calms_the_next_waves() {
        let v = View::ipad_landscape();
        let mut g = fresh_player();
        g.start_level = 8;
        g.state = State::Playing;
        g.things.push(Thing {
            pos: vec2(300.0, v.h + 100.0),
            vel: vec2(0.0, 10.0),
            r: 40.0,
            kind: Kind::Fruit(1),
            spin: 0.0,
        });
        g.update(&Input::default(), &v, 0.01);
        assert_eq!(g.calm, CALM_WAVES);
        for _ in 0..CALM_WAVES {
            g.things.clear();
            g.launch_wave(&v);
            assert_eq!(g.things.len(), 1);
            assert_ne!(g.things[0].kind, Kind::Bomb);
        }
    }

    #[test]
    fn a_new_run_starts_gently_but_not_at_the_bottom() {
        assert_eq!(start_level(0), 1);
        assert_eq!(start_level(1), 1);
        assert_eq!(start_level(4), 2);
        assert_eq!(start_level(6), 3);
        assert_eq!(start_level(30), MAX_START_LEVEL);
    }

    #[test]
    fn progress_saves_per_player_and_resumes() {
        let v = View::ipad_landscape();
        crate::save::reset_for_test();
        let mia = crate::profile::create("Mia", 0).unwrap();
        crate::profile::set_level(GAME, 8);
        let leo = crate::profile::create("Leo", 1).unwrap();
        assert_eq!(Fruit::new().start_level, 1);
        crate::profile::select(&mia.id);
        let g = Fruit::new();
        assert_eq!(g.start_level, 3);
        assert_eq!(g.level(), 3);
        crate::profile::select(&leo.id);
        assert_eq!(Fruit::new().start_level, 1);
        let _ = v;
    }

    #[test]
    fn game_over_posts_the_score_and_keeps_the_rank() {
        let v = View::ipad_landscape();
        let mut g = fresh_player();
        g.start_level = 2;
        start(&mut g, &v);
        g.score = 14;
        g.lives = 1;
        g.things.push(Thing {
            pos: vec2(300.0, v.h + 100.0),
            vel: vec2(0.0, 10.0),
            r: 40.0,
            kind: Kind::Fruit(1),
            spin: 0.0,
        });
        g.update(&Input::default(), &v, 0.01);
        assert_eq!(g.state, State::Over);
        assert_eq!(g.rank, Some(0));
        assert_eq!(Best::load(GAME).0, 14);
        assert_eq!(Fruit::new().best.0, 14);
    }

    #[test]
    fn the_bomb_warning_shows_once() {
        let v = View::ipad_landscape();
        let mut g = fresh_player();
        g.start_level = 5;
        start(&mut g, &v);
        g.things.push(Thing {
            pos: vec2(300.0, 100.0),
            vel: Vec2::ZERO,
            r: 34.0,
            kind: Kind::Bomb,
            spin: 0.0,
        });
        for _ in 0..(5 * 60) {
            g.things[0].pos = vec2(300.0, 100.0);
            g.things[0].vel = Vec2::ZERO;
            g.update(&Input::default(), &v, 1.0 / 60.0);
        }
        assert!(!g.bomb_hint_pending);
        assert_eq!(crate::profile::get(GAME, "bomb_hint"), 1);
        assert!(!Fruit::new().bomb_hint_pending);
    }
}
