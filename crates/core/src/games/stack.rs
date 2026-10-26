use crate::engine::*;
use crate::feel::{self, Impact};
use crate::fx::*;
use crate::geom::*;
use crate::sfx::{self, Sfx};
use glam::{vec2, Vec2};
pub const BLOCK_H: f32 = 40.0;
pub const START_W: f32 = 320.0;
pub const PERFECT: f32 = 6.0;
pub const NIGHT_AT: f32 = 45.0;
pub const KEY: &str = "stack";
pub const MAX_LEVEL: u32 = 20;
const MAX_STRUGGLE: u32 = 3;

pub fn goal_for(level: u32) -> u32 {
    (3 + 2 * level).min(30)
}

pub fn base_speed(level: u32) -> f32 {
    (130.0 + 16.0 * level.saturating_sub(1) as f32).min(300.0)
}

pub fn climb(level: u32) -> f32 {
    (1.0 + 0.2 * level.saturating_sub(1) as f32).min(4.0)
}

pub fn tolerance_for(level: u32, struggle: u32) -> f32 {
    (44.0 - 4.0 * level as f32).max(PERFECT) + 3.0 * struggle as f32
}

pub fn stars_for(perfects: u32, goal: u32) -> u8 {
    let ratio = perfects as f32 / goal.max(1) as f32;
    if ratio >= 0.6 {
        3
    } else if ratio >= 0.3 {
        2
    } else {
        1
    }
}

pub fn continue_level() -> u32 {
    let open = crate::profile::unlocked(KEY).clamp(1, MAX_LEVEL);
    match crate::profile::level(KEY) {
        0 => open,
        l => l.min(open),
    }
}

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
    Won,
}

pub struct Chip {
    pub rect: Rect,
    pub vy: f32,
    pub color: Color,
}

pub struct Ring {
    pub at: Vec2,
    pub w: f32,
    pub age: f32,
}

pub const RING_LIFE: f32 = 0.6;

pub fn altitude(score: u32) -> f32 {
    (score as f32 / NIGHT_AT).min(1.0)
}

pub fn sky_palette(hue: f32, score: u32) -> (Color, Color) {
    let k = ease_out_cubic(altitude(score));
    let drift = (hue * 6.0).sin() * 0.03;
    let day_top = hsv(0.56 + drift, 0.45, 0.98);
    let day_bottom = hsv(0.54 + drift, 0.2, 0.97);
    let night_top = Color::from_rgba(12, 14, 46, 255);
    let night_bottom = Color::from_rgba(52, 38, 96, 255);
    let mix = |a: Color, b: Color| {
        Color::new(
            a.r + (b.r - a.r) * k,
            a.g + (b.g - a.g) * k,
            a.b + (b.b - a.b) * k,
            1.0,
        )
    };
    (mix(day_top, night_top), mix(day_bottom, night_bottom))
}

pub fn block_tint(hue: f32) -> Color {
    hsv(hue, 0.6, 1.0)
}

pub fn dusk_tint(score: u32) -> Color {
    let k = ease_out_cubic(altitude(score)) * 0.65;
    let night = Color::new(0.4, 0.42, 0.7, 1.0);
    Color::new(
        1.0 + (night.r - 1.0) * k,
        1.0 + (night.g - 1.0) * k,
        1.0 + (night.b - 1.0) * k,
        1.0,
    )
}

pub fn dusk_cloud(score: u32) -> Color {
    let d = dusk_tint(score);
    Color::new(d.r, d.g, d.b, 1.0)
}

pub fn tile_spans(from: f32, to: f32, width: f32) -> Vec<(f32, bool)> {
    let first = (from / width).floor() as i32;
    let last = (to / width).ceil() as i32;
    (first..last)
        .map(|i| (i as f32 * width, i.rem_euclid(2) == 1))
        .collect()
}

pub const SCENERY: [(f32, &str, f32); 14] = [
    (-980.0, "tree_pine", 190.0),
    (-830.0, "bush", 60.0),
    (-700.0, "castle", 170.0),
    (-540.0, "tree_round", 150.0),
    (-410.0, "bush", 70.0),
    (-300.0, "tree_pine", 130.0),
    (-190.0, "tree_tall", 190.0),
    (230.0, "tree_pine", 170.0),
    (360.0, "bush", 64.0),
    (480.0, "house", 120.0),
    (610.0, "tree_round", 170.0),
    (730.0, "bush", 74.0),
    (850.0, "tree_tall", 200.0),
    (990.0, "tree_pine", 190.0),
];

pub fn star_field(n: usize) -> Vec<(f32, f32, f32)> {
    (0..n)
        .map(|i| {
            let f = i as f32;
            (
                (f * 0.618_034 + 0.13).fract(),
                (f * 0.381_966 * 7.0 + 0.31).fract(),
                1.5 + (i * 7 % 5) as f32 * 0.7,
            )
        })
        .collect()
}

pub struct Stack {
    pub tower: Vec<Block>,
    pub moving: Block,
    pub dir: f32,
    pub state: State,
    pub score: u32,
    pub best: Best,
    pub streak: u32,
    pub level: u32,
    pub goal: u32,
    pub points: u32,
    pub perfects: u32,
    pub struggle: u32,
    pub stars: u8,
    pub rank: Option<usize>,
    pub picking: bool,
    pub taught: bool,
    pub age: f32,
    confetti_due: bool,
    pub celebrate: Particles,
    pub chips: Vec<Chip>,
    pub cam: f32,
    pub center: f32,
    pub time: f32,
    pub rings: Vec<Ring>,
    pub fx: Particles,
    pub shake: Shake,
    pub popups: Popups,
    pub bump: Bump,
}

pub enum Drop {
    Perfect,
    Trimmed(f32),
    Missed,
}

pub fn place(base: Block, mov: Block) -> (Drop, Option<Block>) {
    place_within(base, mov, PERFECT)
}

pub fn place_within(base: Block, mov: Block, tolerance: f32) -> (Drop, Option<Block>) {
    let off = mov.x - base.x;
    if off.abs() <= tolerance {
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

pub fn block_top(index: usize) -> f32 {
    -(index as f32 + 1.0) * BLOCK_H
}

impl Stack {
    pub fn new() -> Self {
        Self::for_level(continue_level())
    }

    pub fn for_level(level: u32) -> Self {
        let level = level.clamp(1, MAX_LEVEL);
        crate::profile::set_level(KEY, level);
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
            best: Best::load(KEY),
            streak: 0,
            level,
            goal: goal_for(level),
            points: 0,
            perfects: 0,
            struggle: (crate::profile::get(KEY, "fails").max(0) as u32).min(MAX_STRUGGLE),
            stars: 0,
            rank: None,
            picking: false,
            taught: crate::profile::get(KEY, "taught") > 0,
            age: 0.0,
            confetti_due: false,
            celebrate: Particles::default(),
            chips: vec![],
            cam: 0.0,
            center: 0.0,
            time: 0.0,
            rings: vec![],
            fx: Particles::default(),
            shake: Shake::default(),
            popups: Popups::default(),
            bump: Bump::default(),
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
        let eased = 1.0 - 0.1 * self.struggle as f32;
        (base_speed(self.level) + self.score as f32 * climb(self.level)) * eased
    }

    pub fn tolerance(&self) -> f32 {
        tolerance_for(self.level, self.struggle)
    }

    pub fn forgiving(&self) -> bool {
        !self.taught || self.level <= 2
    }

    pub fn sky_score(&self) -> u32 {
        let k = (self.score as f32 / self.goal.max(1) as f32).min(1.0);
        (k * k * NIGHT_AT) as u32
    }

    fn start_level(&mut self, level: u32) {
        if level != self.level {
            crate::profile::set(KEY, "fails", 0);
        }
        *self = Self::for_level(level);
        sfx::say(sfx::Say::Go);
    }

    fn end_run(&mut self) {
        let had_best = self.best.0 > 0;
        self.rank = self.best.submit(KEY, self.points);
        if self.rank == Some(0) && had_best {
            sfx::say(sfx::Say::Best);
        }
    }

    fn finish_level(&mut self) {
        self.stars = stars_for(self.perfects, self.goal);
        let first_time = crate::profile::stars(KEY, self.level) == 0;
        crate::profile::award_stars(KEY, self.level, self.stars);
        let next = (self.level + 1).min(MAX_LEVEL);
        crate::profile::unlock(KEY, next);
        crate::profile::set_level(KEY, next);
        crate::profile::set(KEY, "fails", 0);
        self.struggle = 0;
        self.end_run();
        self.state = State::Won;
        self.age = 0.0;
        self.confetti_due = true;
        sfx::play(Sfx::Win);
        sfx::say(if first_time {
            sfx::Say::LevelUp
        } else {
            sfx::Say::Great
        });
        feel::impact_tinted(Impact::Large, Color::from_rgba(255, 210, 80, 255));
    }

    fn forgive_miss(&mut self, y: f32) {
        self.chips.push(Chip {
            rect: Rect::new(self.moving.x, 0.0, self.moving.w, BLOCK_H),
            vy: 0.0,
            color: block_tint(self.moving.hue),
        });
        self.crumble(self.moving.x, self.moving.w, y, self.moving.hue);
        sfx::play(Sfx::Tap);
        self.shake.add(0.25);
        self.popups.add(
            vec2(
                self.tower.last().unwrap().x + self.tower.last().unwrap().w / 2.0,
                y - 90.0,
            ),
            "Oops! Try again",
            Color::from_rgba(255, 230, 90, 255),
            40.0,
        );
        self.next_block();
    }

    fn crumble(&mut self, x: f32, w: f32, y: f32, hue: f32) {
        let c = block_tint(hue);
        self.fx.emit(
            vec2(x + w / 2.0, y + BLOCK_H / 2.0),
            &[c, darker(c, 0.8), WHITE],
            Burst {
                count: 12,
                speed: (80.0, 320.0),
                size: (3.0, 8.0),
                life: (0.4, 0.8),
                gravity: 900.0,
                shape: Shape::Square,
                spread: 2.6,
                angle: -std::f32::consts::FRAC_PI_2,
                ..Default::default()
            },
        );
    }

    pub fn drop_block(&mut self) {
        let base = *self.tower.last().unwrap();
        let (res, block) = place_within(base, self.moving, self.tolerance());
        let y = block_top(self.tower.len());
        match res {
            Drop::Missed if self.forgiving() => {
                self.forgive_miss(y);
                return;
            }
            Drop::Missed => {
                self.chips.push(Chip {
                    rect: Rect::new(self.moving.x, 0.0, self.moving.w, BLOCK_H),
                    vy: 0.0,
                    color: block_tint(self.moving.hue),
                });
                self.crumble(self.moving.x, self.moving.w, y, self.moving.hue);
                sfx::play(Sfx::Crunch);
                sfx::play(Sfx::Lose);
                feel::impact(Impact::Medium);
                self.shake.add(0.6);
                self.struggle = (self.struggle + 1).min(MAX_STRUGGLE);
                crate::profile::set(KEY, "fails", self.struggle as i64);
                self.state = State::Over;
                self.age = 0.0;
                self.end_run();
                return;
            }
            Drop::Perfect => {
                sfx::play_step(Sfx::Right, self.streak as usize);
                self.streak += 1;
                self.perfects += 1;
                self.points += 1;
                let b = block.unwrap();
                let at = vec2(b.x + b.w / 2.0, y + BLOCK_H / 2.0);
                self.rings.push(Ring {
                    at,
                    w: b.w,
                    age: 0.0,
                });
                self.fx
                    .sparkle(vec2(b.x + b.w * 0.5, y), hsv(b.hue, 0.3, 1.0));
                self.fx.sparkle(vec2(b.x, y), WHITE);
                self.fx.sparkle(vec2(b.x + b.w, y), WHITE);
                let label = if self.streak >= 2 {
                    format!("Perfect x{}", self.streak)
                } else {
                    "Perfect!".to_string()
                };
                self.popups.add(
                    vec2(at.x, y - 30.0),
                    label,
                    Color::from_rgba(255, 230, 90, 255),
                    40.0,
                );
            }
            Drop::Trimmed(cut) => {
                sfx::play(Sfx::Tap);
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
                    color: block_tint(self.moving.hue),
                });
                self.crumble(cx, cut, y, self.moving.hue);
                self.shake.add((0.25 + cut / START_W).min(0.7));
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
        self.points += self.level;
        self.bump.kick();
        if !self.taught {
            self.taught = true;
            crate::profile::set(KEY, "taught", 1);
        }
        if self.score >= self.goal {
            self.finish_level();
            return;
        }
        self.milestone(b, y);
        self.next_block();
    }

    fn milestone(&mut self, b: Block, y: f32) {
        if self.goal < 6 {
            return;
        }
        let label = if self.score == self.goal / 2 {
            "Halfway!"
        } else if self.score + 1 == self.goal {
            "One more!"
        } else {
            return;
        };
        sfx::play(Sfx::Score);
        self.popups.add(
            vec2(b.x + b.w / 2.0, y - 100.0),
            label,
            Color::from_rgba(120, 230, 255, 255),
            46.0,
        );
    }
}

impl Default for Stack {
    fn default() -> Self {
        Self::new()
    }
}

impl Game for Stack {
    fn update(&mut self, input: &Input, view: &View, dt: f32) {
        self.time += dt;
        self.age += dt;
        self.fx.update(dt);
        self.celebrate.update(dt);
        self.shake.update(dt);
        self.popups.update(dt);
        self.bump.update(dt);
        for r in &mut self.rings {
            r.age += dt;
        }
        self.rings.retain(|r| r.age < RING_LIFE);
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
        if self.confetti_due {
            self.confetti_due = false;
            self.celebrate.confetti(view);
        }
        if self.picking {
            self.update_picker(input, view);
            return;
        }
        if self.shows_levels_button() && input.tapped_in(levels_button(view)) {
            self.picking = true;
            sfx::play(Sfx::Click);
            return;
        }
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
                if input.tap() && self.chips.is_empty() && self.age > 0.6 {
                    self.start_level(self.level);
                }
            }
            State::Won => {
                if input.tap() && self.age > 0.8 {
                    self.start_level((self.level + 1).min(MAX_LEVEL));
                }
            }
        }
    }
}

pub fn levels_button(view: &View) -> Rect {
    Rect::new(20.0, view.h - 84.0, 170.0, 64.0)
}

pub fn close_button(view: &View) -> Rect {
    Rect::new(view.w - 84.0, 12.0, 64.0, 64.0)
}

pub fn picker_layout(view: &View) -> Vec<Rect> {
    let cols = if view.w >= view.h { 5 } else { 4 };
    let rows = (MAX_LEVEL as usize).div_ceil(cols);
    let top = 140.0;
    let gap = 14.0;
    let area_w = view.w - 60.0;
    let area_h = view.h - top - 30.0;
    let cw = (area_w - gap * (cols as f32 - 1.0)) / cols as f32;
    let ch = (area_h - gap * (rows as f32 - 1.0)) / rows as f32;
    let size = cw.min(ch).min(190.0);
    let ox = (view.w - (size * cols as f32 + gap * (cols as f32 - 1.0))) / 2.0;
    let oy = top + (area_h - (size * rows as f32 + gap * (rows as f32 - 1.0))) / 2.0;
    (0..MAX_LEVEL as usize)
        .map(|i| {
            Rect::new(
                ox + (i % cols) as f32 * (size + gap),
                oy + (i / cols) as f32 * (size + gap),
                size,
                size,
            )
        })
        .collect()
}

impl Stack {
    pub fn shows_results(&self) -> bool {
        self.state == State::Over && self.chips.is_empty()
    }

    pub fn shows_levels_button(&self) -> bool {
        match self.state {
            State::Playing => self.score == 0,
            State::Over => self.chips.is_empty() && self.age > 0.6,
            State::Won => self.age > 0.8,
        }
    }

    fn update_picker(&mut self, input: &Input, view: &View) {
        if input.tapped_in(close_button(view)) {
            self.picking = false;
            sfx::play(Sfx::Click);
            return;
        }
        let open = crate::profile::unlocked(KEY).max(1);
        for (i, r) in picker_layout(view).iter().enumerate() {
            if input.tapped_in(*r) {
                let level = i as u32 + 1;
                if level <= open {
                    sfx::play(Sfx::Click);
                    self.start_level(level);
                } else {
                    sfx::play(Sfx::Tick);
                }
                return;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stack_sprites_all_exist() {
        let b = std::fs::read(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../assets/art/stack.kga"
        ))
        .unwrap();
        let idx = crate::art::parse(&b).unwrap();
        let have: Vec<String> = idx.frames.keys().cloned().collect();
        for n in [
            "blk", "hills", "peaks", "ground", "cloud1", "cloud2", "cloud3", "moon", "bush",
        ] {
            assert!(have.iter().any(|h| h == n), "missing {n}");
        }
        for (_, name, _) in SCENERY {
            assert!(have.iter().any(|h| h == name), "missing {name}");
        }
    }

    #[test]
    fn results_wait_for_the_falling_chips() {
        practised_player();
        let mut s = Stack::for_level(3);
        s.moving.x = 400.0;
        s.drop_block();
        assert_eq!(s.state, State::Over);
        assert!(!s.shows_results());
        let v = View::ipad_landscape();
        for _ in 0..120 {
            s.update(&Input::default(), &v, 1.0 / 60.0);
        }
        assert!(s.shows_results());
    }

    #[test]
    fn dusk_darkens_with_altitude() {
        let day = dusk_tint(0);
        let night = dusk_tint(80);
        assert_eq!(day, WHITE);
        assert!(night.r < 0.7 && night.g < 0.7);
    }

    #[test]
    fn tile_spans_cover_the_range_and_alternate() {
        let t = tile_spans(-250.0, 1700.0, 1000.0);
        assert_eq!(t.first().unwrap().0, -1000.0);
        assert!(t.last().unwrap().0 + 1000.0 >= 1700.0);
        assert!(t.windows(2).all(|w| w[0].1 != w[1].1));
    }

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

    fn fresh_player() {
        crate::save::reset_for_test();
        crate::profile::create("Mia", 0);
    }

    fn practised_player() {
        fresh_player();
        crate::profile::set(KEY, "taught", 1);
    }

    fn play_with_bot(s: &mut Stack, slack: f32, max_secs: f32) {
        let v = View::ipad_landscape();
        let tap = Input::press_at(v.center());
        let idle = Input::default();
        let mut t = 0.0;
        while s.state == State::Playing && t < max_secs {
            let near = (s.moving.x - s.tower.last().unwrap().x).abs() < slack;
            s.update(if near { &tap } else { &idle }, &v, 1.0 / 60.0);
            t += 1.0 / 60.0;
        }
    }

    fn sloppy_tap_at(s: &mut Stack, offset: f32) {
        let top = *s.tower.last().unwrap();
        s.moving.x = top.x + offset;
        s.drop_block();
    }

    #[test]
    fn total_miss_ends_game_once_the_kid_has_the_idea() {
        practised_player();
        let mut s = Stack::for_level(3);
        s.moving.x = 400.0;
        s.drop_block();
        assert_eq!(s.state, State::Over);
        assert_eq!(s.score, 0);
    }

    #[test]
    fn levels_one_and_two_never_end_in_a_fall() {
        fresh_player();
        for level in [1, 2] {
            let mut s = Stack::for_level(level);
            for _ in 0..6 {
                s.moving.x = 400.0;
                s.drop_block();
            }
            assert_eq!(s.state, State::Playing);
        }
    }

    #[test]
    fn the_first_misses_are_forgiven_and_the_tower_keeps_going() {
        fresh_player();
        let mut s = Stack::new();
        assert_eq!(s.level, 1);
        s.moving.x = 400.0;
        s.drop_block();
        assert_eq!(s.state, State::Playing);
        assert_eq!(s.tower.len(), 1);
        assert_eq!(s.moving.x, 420.0 - s.moving.w);
    }

    #[test]
    fn first_level_is_slow_wide_and_short() {
        fresh_player();
        let s = Stack::new();
        assert!(s.speed() <= 140.0);
        assert!(s.goal <= 5);
        assert!(s.tolerance() >= 30.0);
        assert!(!s.taught);
    }

    #[test]
    fn a_four_year_old_who_taps_near_the_middle_clears_level_one() {
        fresh_player();
        let mut s = Stack::new();
        play_with_bot(&mut s, 35.0, 60.0);
        assert_eq!(s.state, State::Won);
        assert_eq!(s.stars, 3);
        assert_eq!(crate::profile::stars(KEY, 1), 3);
        assert_eq!(crate::profile::unlocked(KEY), 2);
    }

    #[test]
    fn a_player_tapping_at_random_never_loses_in_the_first_ten_seconds() {
        fresh_player();
        let mut s = Stack::new();
        let v = View::ipad_landscape();
        let tap = Input::press_at(v.center());
        let idle = Input::default();
        for f in 0..600 {
            let i = if f % 70 == 69 { &tap } else { &idle };
            s.update(i, &v, 1.0 / 60.0);
            assert_ne!(s.state, State::Over);
        }
    }

    #[test]
    fn speed_and_tolerance_get_harder_with_level() {
        fresh_player();
        let mut last = Stack::for_level(1);
        for l in 2..=MAX_LEVEL {
            let next = Stack::for_level(l);
            assert!(next.speed() >= last.speed());
            assert!(next.tolerance() <= last.tolerance());
            assert!(next.goal >= last.goal);
            last = next;
        }
        assert!(Stack::for_level(MAX_LEVEL).speed() > Stack::for_level(1).speed() * 2.0);
        assert!(Stack::for_level(MAX_LEVEL).tolerance() >= PERFECT);
    }

    #[test]
    fn speed_climbs_inside_a_run() {
        fresh_player();
        let mut s = Stack::for_level(3);
        let before = s.speed();
        s.score = 6;
        assert!(s.speed() > before);
    }

    #[test]
    fn reaching_the_goal_awards_stars_unlocks_and_celebrates() {
        fresh_player();
        let mut s = Stack::for_level(2);
        crate::sfx::take_pending();
        crate::feel::take();
        for _ in 0..s.goal {
            sloppy_tap_at(&mut s, 0.0);
        }
        assert_eq!(s.state, State::Won);
        assert_eq!(s.stars, 3);
        assert_eq!(crate::profile::stars(KEY, 2), 3);
        assert_eq!(crate::profile::unlocked(KEY), 3);
        assert!(crate::sfx::take_pending().contains(&Sfx::Win));
        assert!(crate::feel::take().iter().any(|(i, _)| *i == Impact::Large));
        assert!(s.confetti_due);
    }

    #[test]
    fn stars_depend_on_how_many_drops_were_perfect() {
        assert_eq!(stars_for(10, 10), 3);
        assert_eq!(stars_for(6, 10), 3);
        assert_eq!(stars_for(4, 10), 2);
        assert_eq!(stars_for(0, 10), 1);
    }

    #[test]
    fn winning_never_lowers_earned_stars() {
        fresh_player();
        crate::profile::award_stars(KEY, 1, 3);
        let mut s = Stack::for_level(1);
        for _ in 0..s.goal {
            sloppy_tap_at(&mut s, 60.0);
        }
        assert_eq!(s.state, State::Won);
        assert_eq!(crate::profile::stars(KEY, 1), 3);
    }

    #[test]
    fn progress_resumes_per_player() {
        fresh_player();
        let mut s = Stack::for_level(1);
        for _ in 0..s.goal {
            sloppy_tap_at(&mut s, 0.0);
        }
        assert_eq!(Stack::new().level, 2);
        crate::profile::create("Leo", 1);
        assert_eq!(Stack::new().level, 1);
        assert_eq!(crate::profile::stars(KEY, 1), 0);
    }

    #[test]
    fn replaying_an_earlier_level_does_not_lose_the_frontier() {
        fresh_player();
        crate::profile::unlock(KEY, 5);
        let s = Stack::for_level(2);
        assert_eq!(s.level, 2);
        assert_eq!(crate::profile::unlocked(KEY), 5);
    }

    #[test]
    fn falling_eases_the_next_try_and_winning_resets_it() {
        practised_player();
        let mut s = Stack::for_level(3);
        let hard = (s.speed(), s.tolerance());
        s.moving.x = 400.0;
        s.drop_block();
        assert_eq!(s.state, State::Over);
        let s2 = Stack::for_level(3);
        assert!(s2.speed() < hard.0);
        assert!(s2.tolerance() > hard.1);
        let mut s3 = Stack::for_level(3);
        for _ in 0..s3.goal {
            sloppy_tap_at(&mut s3, 0.0);
        }
        assert_eq!(Stack::for_level(4).struggle, 0);
    }

    #[test]
    fn struggle_is_capped_so_the_game_never_goes_slow_motion() {
        practised_player();
        for _ in 0..10 {
            let mut s = Stack::for_level(4);
            s.moving.x = 400.0;
            s.drop_block();
        }
        let s = Stack::for_level(4);
        assert_eq!(s.struggle, MAX_STRUGGLE);
        assert!(s.speed() > base_speed(4) * 0.6);
    }

    #[test]
    fn game_over_submits_the_score_and_remembers_the_best() {
        practised_player();
        let mut s = Stack::for_level(3);
        sloppy_tap_at(&mut s, 0.0);
        sloppy_tap_at(&mut s, 0.0);
        s.moving.x = 400.0;
        s.drop_block();
        assert_eq!(s.state, State::Over);
        assert_eq!(s.rank, Some(0));
        assert!(crate::profile::personal_best(KEY).unwrap() > 0);
        assert_eq!(
            Stack::for_level(3).best.0 as i64,
            crate::profile::personal_best(KEY).unwrap()
        );
    }

    #[test]
    fn tapping_after_game_over_restarts_the_same_level() {
        practised_player();
        let mut s = Stack::for_level(3);
        s.moving.x = 400.0;
        s.drop_block();
        let v = View::ipad_landscape();
        for _ in 0..120 {
            s.update(&Input::default(), &v, 1.0 / 60.0);
        }
        s.update(&Input::press_at(vec2(v.w / 2.0, 100.0)), &v, 1.0 / 60.0);
        assert_eq!(s.state, State::Playing);
        assert_eq!(s.level, 3);
    }

    #[test]
    fn winning_tap_moves_to_the_next_level() {
        fresh_player();
        let mut s = Stack::for_level(1);
        for _ in 0..s.goal {
            sloppy_tap_at(&mut s, 0.0);
        }
        let v = View::ipad_landscape();
        for _ in 0..90 {
            s.update(&Input::default(), &v, 1.0 / 60.0);
        }
        s.update(&Input::press_at(vec2(v.w / 2.0, 100.0)), &v, 1.0 / 60.0);
        assert_eq!(s.level, 2);
        assert_eq!(s.state, State::Playing);
    }

    #[test]
    fn level_picker_opens_only_unlocked_levels() {
        fresh_player();
        crate::profile::unlock(KEY, 3);
        let v = View::ipad_landscape();
        let mut s = Stack::for_level(1);
        s.picking = true;
        let rects = picker_layout(&v);
        assert_eq!(rects.len(), MAX_LEVEL as usize);
        s.update(&Input::press_at(rects[5].center()), &v, 1.0 / 60.0);
        assert!(s.picking);
        assert_eq!(s.level, 1);
        s.update(&Input::press_at(rects[2].center()), &v, 1.0 / 60.0);
        assert!(!s.picking);
        assert_eq!(s.level, 3);
    }

    #[test]
    fn picker_tiles_fit_the_screen_in_both_orientations() {
        for v in [View::ipad_landscape(), View::ipad_portrait()] {
            for r in picker_layout(&v) {
                assert!(r.x >= 0.0 && r.y >= 0.0 && r.x + r.w <= v.w && r.y + r.h <= v.h);
            }
        }
    }

    #[test]
    fn level_goals_grow_to_a_cap() {
        assert_eq!(goal_for(1), 5);
        assert!(goal_for(10) > goal_for(5));
        assert_eq!(goal_for(MAX_LEVEL), 30);
    }

    #[test]
    fn the_sky_reaches_night_exactly_at_the_goal() {
        fresh_player();
        let mut s = Stack::for_level(4);
        s.score = s.goal;
        assert_eq!(altitude(s.sky_score()), 1.0);
        s.score = 0;
        assert_eq!(s.sky_score(), 0);
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

    #[test]
    fn altitude_rises_with_score_and_saturates() {
        assert_eq!(altitude(0), 0.0);
        assert!(altitude(10) > altitude(5));
        assert_eq!(altitude(500), 1.0);
    }

    #[test]
    fn sky_darkens_as_the_tower_climbs() {
        let (low_top, _) = sky_palette(0.5, 0);
        let (high_top, _) = sky_palette(0.5, 60);
        let lum = |c: Color| c.r + c.g + c.b;
        assert!(lum(high_top) < lum(low_top) * 0.6);
    }

    #[test]
    fn trimming_shakes_and_throws_chips() {
        let mut s = Stack::new();
        s.moving.x = s.tower[0].x + 80.0;
        s.drop_block();
        assert!(s.shake.active());
        assert!(!s.fx.list.is_empty());
        assert!(s.rings.is_empty());
        assert_eq!(s.score, 1);
    }

    #[test]
    fn perfect_drop_rings_and_sparkles() {
        let mut s = Stack::new();
        s.moving.x = s.tower[0].x;
        s.drop_block();
        assert_eq!(s.rings.len(), 1);
        assert!(s.fx.list.iter().any(|p| p.shape == Shape::Star));
        assert!(s.bump.t < 1.0);
        assert_eq!(s.popups.list.len(), 1);
        let v = View::ipad_landscape();
        for _ in 0..120 {
            s.update(&Input::default(), &v, 1.0 / 60.0);
        }
        assert!(s.rings.is_empty());
    }

    #[test]
    fn losing_plays_the_sad_jingle_and_a_new_best_is_announced() {
        practised_player();
        let mut s = Stack::for_level(3);
        crate::profile::submit(KEY, 2, crate::profile::Better::Higher);
        s.best = Best(2);
        s.score = 5;
        s.points = 10;
        crate::sfx::take_pending();
        crate::sfx::take_voice();
        s.moving.x = 400.0;
        s.drop_block();
        assert!(crate::sfx::take_pending().contains(&Sfx::Lose));
        assert_eq!(crate::sfx::take_voice(), Some(sfx::Say::Best));
    }

    #[test]
    fn missing_entirely_shakes() {
        let mut s = Stack::new();
        s.moving.x = 400.0;
        s.drop_block();
        assert!(s.shake.active());
    }

    #[test]
    fn star_field_is_stable_and_inside_the_unit_square() {
        let a = star_field(40);
        assert_eq!(a, star_field(40));
        assert!(a
            .iter()
            .all(|s| (0.0..1.0).contains(&s.0) && (0.0..1.0).contains(&s.1)));
    }
}
