use crate::engine::draw_circle;
use crate::engine::*;
use crate::feel::{self, Impact};
use crate::fx::*;
use crate::sfx::{self, Sfx};
use macroquad::prelude::*;

pub const BLOCK_H: f32 = 40.0;
pub const START_W: f32 = 320.0;
pub const PERFECT: f32 = 6.0;
pub const NIGHT_AT: f32 = 45.0;

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

struct Ring {
    at: Vec2,
    w: f32,
    age: f32,
}

const RING_LIFE: f32 = 0.6;

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

const SCENERY: [(f32, &str, f32); 14] = [
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
    chips: Vec<Chip>,
    cam: f32,
    center: f32,
    time: f32,
    rings: Vec<Ring>,
    fx: Particles,
    shake: Shake,
    popups: Popups,
    bump: Bump,
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

fn block_top(index: usize) -> f32 {
    -(index as f32 + 1.0) * BLOCK_H
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
        260.0 + self.score as f32 * 8.0
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
        let (res, block) = place(base, self.moving);
        let y = block_top(self.tower.len());
        match res {
            Drop::Missed => {
                self.chips.push(Chip {
                    rect: Rect::new(self.moving.x, 0.0, self.moving.w, BLOCK_H),
                    vy: 0.0,
                    color: block_tint(self.moving.hue),
                });
                self.crumble(self.moving.x, self.moving.w, y, self.moving.hue);
                sfx::play(Sfx::Crunch);
                sfx::play(Sfx::Lose);
                if self.score > self.best.0 && self.best.0 > 0 {
                    sfx::say(sfx::Say::Best);
                }
                feel::impact(Impact::Medium);
                self.shake.add(0.6);
                self.state = State::Over;
                self.best.offer(self.score);
                return;
            }
            Drop::Perfect => {
                sfx::play_step(Sfx::Right, self.streak as usize);
                self.streak += 1;
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
        self.bump.kick();
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
        self.time += dt;
        self.fx.update(dt);
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
                    sfx::say(sfx::Say::Go);
                }
            }
        }
    }

    fn draw(&self, view: &View) {
        let hue = self.tower.last().unwrap().hue;
        let (top, bottom) = sky_palette(hue, self.score);
        sky(view, top, bottom);
        self.draw_backdrop(view);
        let ground = view.h - 80.0;
        let ox = view.w / 2.0 - self.center * 0.5;
        set_view_camera(view, vec2(ox, ground + self.cam) + self.shake.offset());
        self.draw_tower(view, ground);
        for r in &self.rings {
            let k = ease_out_cubic(r.age / RING_LIFE);
            let a = 1.0 - k;
            draw_ellipse_lines(
                r.at.x,
                r.at.y,
                r.w * (0.5 + 0.35 * k),
                BLOCK_H * (0.6 + 1.2 * k),
                0.0,
                5.0 * a + 1.0,
                with_alpha(WHITE, a),
            );
        }
        self.fx.draw();
        self.popups.draw();
        set_view_camera(view, Vec2::ZERO);
        big_score(view, &self.score.to_string(), 90.0, self.bump.t);
        chip(view, Side::Right, "BEST", &self.best.0.to_string(), 1.0);
        if self.score == 0 && self.state == State::Playing {
            let bob = (self.time * 4.0).sin() * 4.0;
            text_shadow(
                "Tap to drop the block",
                view.w / 2.0,
                220.0 + bob,
                40.0,
                WHITE,
            );
        }
        if self.state == State::Over && self.chips.is_empty() {
            let s = format!("Height {}   Best {}", self.score, self.best.0);
            overlay(view, "Tower done!", &[&s, "Tap to build again"]);
        }
    }
}

impl Stack {
    fn draw_backdrop(&self, view: &View) {
        let k = altitude(self.score);
        let dusk = dusk_tint(self.score);
        for (fx, fy, size) in star_field(48) {
            let twinkle = 0.6 + 0.4 * (self.time * 2.5 + fx * 40.0).sin();
            let y = (fy * view.h + self.cam * 0.05).rem_euclid(view.h);
            let a = (k * 1.6 - 0.5).clamp(0.0, 1.0) * twinkle;
            if a > 0.02 {
                if size > 3.0 {
                    star(
                        vec2(fx * view.w, y),
                        size * 1.8,
                        self.time * 0.3,
                        with_alpha(WHITE, a),
                    );
                } else {
                    draw_circle(fx * view.w, y, size, with_alpha(WHITE, a));
                }
            }
        }
        let moon_a = (k * 2.0 - 0.6).clamp(0.0, 1.0);
        if moon_a > 0.0 {
            art::sprite_at(
                "moon",
                vec2(view.w * 0.82, 150.0 + (1.0 - moon_a) * 60.0),
                90.0,
                crate::art::Draw {
                    tint: with_alpha(WHITE, moon_a),
                    ..Default::default()
                },
            );
        }
        let cloud_alpha = 0.85 * (1.0 - k * 0.8);
        for i in 0..7 {
            let depth = 0.12 + (i % 3) as f32 * 0.08;
            let span = view.w + 500.0;
            let x = (i as f32 * 270.0 + self.time * (6.0 + depth * 40.0)).rem_euclid(span) - 250.0;
            let y =
                (60.0 + (i * 97 % 520) as f32 + self.cam * depth).rem_euclid(view.h + 160.0) - 80.0;
            art::sprite_w(
                ["cloud1", "cloud2", "cloud3"][i % 3],
                vec2(x, y),
                (0.8 + depth * 2.5) * 150.0,
                crate::art::Draw {
                    tint: with_alpha(dusk_cloud(self.score), cloud_alpha),
                    ..Default::default()
                },
            );
        }
        let ground = view.h - 80.0;
        let f = view.full();
        let peak_h = 190.0;
        let peak_w = peak_h * art::size("peaks").x / art::size("peaks").y;
        let peak_bottom = ground + 24.0 + self.cam * 0.55;
        let haze = Color::new(0.76 * dusk.r, 0.89 * dusk.g, 0.97 * dusk.b, 1.0);
        draw_rectangle(f.x, peak_bottom - 2.0, f.w, f.h + 400.0, haze);
        let shift = -self.center * 0.12;
        for (x, flip) in tile_spans(f.x - shift, f.x + f.w - shift, peak_w) {
            art::sprite_ex(
                "peaks",
                Rect::new(x + shift, peak_bottom - peak_h, peak_w + 1.0, peak_h),
                crate::art::Draw {
                    tint: Color::new(dusk.r, dusk.g, dusk.b, 1.0),
                    flip_x: flip,
                    ..Default::default()
                },
            );
        }
    }

    fn draw_scenery(&self, view: &View) {
        let dusk = dusk_tint(self.score);
        let reach = 1500.0;
        let hill_h = 230.0;
        let hill_w = hill_h * art::size("hills").x / art::size("hills").y;
        for (x, flip) in tile_spans(-reach, reach, hill_w) {
            art::sprite_ex(
                "hills",
                Rect::new(x - 120.0, -hill_h + 30.0, hill_w + 1.0, hill_h),
                crate::art::Draw {
                    tint: Color::new(dusk.r, dusk.g, dusk.b, 1.0),
                    flip_x: flip,
                    ..Default::default()
                },
            );
        }
        for (x, name, h) in SCENERY {
            if x < -view.w || x > view.w {
                continue;
            }
            let tint = if name == "castle" || name == "house" {
                Color::new(dusk.r * 0.95, dusk.g * 0.95, dusk.b * 0.95, 1.0)
            } else {
                Color::new(dusk.r, dusk.g, dusk.b, 1.0)
            };
            art::sprite_at(
                name,
                vec2(x, 6.0 - h / 2.0),
                h,
                crate::art::Draw {
                    tint,
                    ..Default::default()
                },
            );
        }
    }

    fn draw_block_sprite(r: Rect, tint: Color) {
        let src_h = art::size("blk").y;
        art::nine("blk", r, src_h * 0.3, BLOCK_H / src_h, tint);
    }

    fn draw_tower(&self, view: &View, ground: f32) {
        let screen_y = |wy: f32| ground + self.cam + wy;
        let dusk = dusk_tint(self.score);
        self.draw_scenery(view);
        draw_rectangle(
            -3000.0,
            0.0,
            6000.0,
            700.0,
            Color::new(0.45 * dusk.r, 0.3 * dusk.g, 0.26 * dusk.b, 1.0),
        );
        let gs = art::size("ground");
        let gh = 150.0;
        let gw = gh * gs.x / gs.y;
        for (x, flip) in tile_spans(-1800.0, 1800.0, gw) {
            art::sprite_ex(
                "ground",
                Rect::new(x, -gh * 0.12, gw + 1.0, gh),
                crate::art::Draw {
                    tint: Color::new(dusk.r, dusk.g, dusk.b, 1.0),
                    flip_x: flip,
                    ..Default::default()
                },
            );
        }
        let base = self.tower[0];
        soft_shadow(vec2(base.x + base.w / 2.0, 10.0), base.w * 0.6, 10.0, 0.8);
        for (i, b) in self.tower.iter().enumerate() {
            let y = block_top(i);
            let sy = screen_y(y);
            if sy > view.h + BLOCK_H || sy < -BLOCK_H {
                continue;
            }
            Self::draw_block_sprite(Rect::new(b.x, y, b.w, BLOCK_H), block_tint(b.hue));
        }
        let my = block_top(self.tower.len());
        if self.state == State::Playing {
            let b = self.moving;
            let c = block_tint(b.hue);
            Self::draw_block_sprite(
                Rect::new(b.x - 5.0, my - 5.0, b.w + 10.0, BLOCK_H + 10.0),
                with_alpha(c, 0.25),
            );
            Self::draw_block_sprite(Rect::new(b.x, my, b.w, BLOCK_H), c);
            shine_sweep(Rect::new(b.x, my, b.w, BLOCK_H), self.time);
        }
        for c in &self.chips {
            Self::draw_block_sprite(
                Rect::new(c.rect.x, my + c.rect.y, c.rect.w, c.rect.h),
                c.color,
            );
        }
    }
}

pub fn icon(r: Rect) {
    crate::ui::icon("stack", r);
}

mod art {
    crate::sprites!("stack");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stack_sprites_all_exist() {
        let have = art::names();
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
        let mut s = Stack::new();
        s.best = Best(2);
        s.score = 5;
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
