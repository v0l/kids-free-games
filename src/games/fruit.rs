use crate::engine::draw_circle;
use crate::engine::*;
use crate::feel::{self, Impact};
use crate::fx::*;
use crate::sfx::{self, Sfx};
use macroquad::prelude::*;

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

struct Half {
    pos: Vec2,
    vel: Vec2,
    r: f32,
    fruit: usize,
    angle: f32,
    spin: f32,
}

struct Stain {
    pos: Vec2,
    r: f32,
    color: Color,
    age: f32,
    drops: [Vec2; 3],
    turn: f32,
}

type Rgb = (u8, u8, u8);

struct Species {
    name: &'static str,
    skin: Rgb,
    flesh: Rgb,
    seed: Option<Rgb>,
}

const FRUITS: [Species; 6] = [
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

const FRUIT_SPAN: f32 = 2.3;
const BOMB_SPAN: f32 = 2.5;

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
    halves: Vec<Half>,
    stains: Vec<Stain>,
    pub blade: Vec<(Vec2, f32)>,
    pub score: u32,
    pub best: Best,
    pub lives: u32,
    pub state: State,
    wave_in: f32,
    time: f32,
    flash: f32,
    fx: Particles,
    shake: Shake,
    popups: Popups,
    score_bump: Bump,
    heart_bump: Bump,
}

fn mix(a: Color, b: Color, k: f32) -> Color {
    Color::from_vec(a.to_vec() * (1.0 - k) + b.to_vec() * k)
}

fn rgb(c: Rgb) -> Color {
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
            best: Best::default(),
            lives: LIVES,
            state: State::Ready,
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

    fn launch_wave(&mut self, view: &View) {
        let count = 1 + (self.time / 20.0) as usize + rand::gen_range(0, 3);
        let bomb_chance = (0.08 + self.time * 0.003).min(0.25);
        for _ in 0..count.min(6) {
            let x = rand::gen_range(view.w * 0.15, view.w * 0.85);
            let peak = rand::gen_range(view.h * 0.12, view.h * 0.45);
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
                    rand::gen_range(34.0, 46.0)
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
                if missed > 0 {
                    sfx::play(Sfx::Wrong);
                    self.heart_bump.kick();
                    self.shake.add(0.2);
                }
                self.lives = self.lives.saturating_sub(missed);
                self.wave_in -= dt;
                if self.wave_in <= 0.0 && self.things.is_empty() {
                    self.launch_wave(view);
                    self.wave_in = (1.2 - self.time * 0.005).max(0.5);
                }
                if self.lives == 0 {
                    self.state = State::Over;
                    sfx::play(Sfx::Lose);
                    if self.score > self.best.0 && self.best.0 > 0 {
                        sfx::say(sfx::Say::Best);
                    }
                    self.best.offer(self.score);
                }
            }
            State::Over => {
                if input.tap() && self.flash == 0.0 {
                    let best = self.best;
                    *self = Self::new();
                    self.best = best;
                    self.state = State::Playing;
                    sfx::say(sfx::Say::Go);
                }
            }
        }
    }

    fn draw(&self, view: &View) {
        set_view_camera(view, self.shake.offset());
        self.draw_wall(view);
        for st in &self.stains {
            draw_stain(st);
        }
        for t in &self.things {
            self.draw_thing(t);
        }
        for h in &self.halves {
            draw_half(h);
        }
        self.fx.draw();
        self.draw_blade();
        self.popups.draw();
        set_view_camera(view, Vec2::ZERO);
        vignette(view, 0.6);
        self.draw_hud(view);
        if self.flash > 0.0 {
            fill_screen(view, with_alpha(WHITE, self.flash));
        }
        match self.state {
            State::Ready => overlay(
                view,
                "Fruit Slice",
                &[
                    "Swipe across fruit to slice it",
                    "Don't touch the bombs!",
                    "Tap to start",
                ],
            ),
            State::Over => {
                let s = format!("Score {}   Best {}", self.score, self.best.0);
                overlay(view, "Game over", &[&s, "Tap to play again"]);
            }
            State::Playing => {}
        }
    }
}

fn draw_stain(st: &Stain) {
    let a = stain_alpha(st.age);
    let c = with_alpha(st.color, a);
    let grow = ease_out_back(st.age / 0.25);
    let tint = crate::art::Draw {
        tint: c,
        rotation: st.turn,
        ..Default::default()
    };
    art::sprite_at(splat_name(), st.pos, st.r * 2.2 * grow, tint);
    for (i, d) in st.drops.iter().enumerate() {
        let size = st.r * (0.4 + 0.12 * i as f32) * grow;
        art::sprite_at(
            splat_name(),
            st.pos + *d * grow,
            size,
            crate::art::Draw {
                tint: c,
                rotation: st.turn + i as f32,
                ..Default::default()
            },
        );
    }
}

fn splat_name() -> &'static str {
    "splat0"
}

fn fit_in(s: Vec2, side: f32) -> (f32, f32) {
    let k = side / s.x.max(s.y).max(1.0);
    (s.x * k, s.y * k)
}

fn fruit_sprite(f: usize, suffix: &str, c: Vec2, r: f32, rotation: f32, flip_x: bool) {
    let side = r * FRUIT_SPAN;
    let name = format!("{}{}", FRUITS[f].name, suffix);
    let (w, h) = fit_in(art::size(&name), side);
    art::sprite_ex(
        &name,
        Rect::new(c.x - w / 2.0, c.y - h / 2.0, w, h),
        crate::art::Draw {
            rotation,
            flip_x,
            ..Default::default()
        },
    );
}

fn draw_half(h: &Half) {
    soft_shadow(
        h.pos + vec2(h.r * 0.12, h.r * 0.8),
        h.r * 0.6,
        h.r * 0.14,
        0.3,
    );
    fruit_sprite(h.fruit, "_half", h.pos, h.r, h.angle, h.spin > 0.0);
}

mod art {
    crate::sprites!("fruit");
}

impl Fruit {
    fn draw_wall(&self, view: &View) {
        let f = view.full();
        let side = f.w.max(f.h);
        art::sprite(
            "wall",
            Rect::new(
                f.x + (f.w - side) / 2.0,
                f.y + (f.h - side) / 2.0,
                side,
                side,
            ),
        );
    }

    fn draw_thing(&self, t: &Thing) {
        match t.kind {
            Kind::Bomb => {
                let pulse = 0.5 + 0.5 * (self.time * 12.0).sin();
                draw_circle(
                    t.pos.x,
                    t.pos.y,
                    t.r * 1.3,
                    Color::new(1.0, 0.3, 0.2, 0.08 + 0.1 * pulse),
                );
                let (bw, bh) = fit_in(art::size("bomb"), t.r * BOMB_SPAN);
                let at = Rect::new(t.pos.x - bw / 2.0, t.pos.y - bh / 2.0, bw, bh);
                let turn = crate::art::Draw {
                    rotation: (t.spin * self.time).sin() * 0.4,
                    tint: Color::new(1.0, 1.0 - 0.25 * pulse, 1.0 - 0.3 * pulse, 1.0),
                    ..Default::default()
                };
                art::sprite_ex("bomb", at, turn);
            }
            Kind::Fruit(f) => {
                soft_shadow(
                    t.pos + vec2(t.r * 0.12, t.r * 0.9),
                    t.r * 0.8,
                    t.r * 0.18,
                    0.4,
                );
                fruit_sprite(f, "", t.pos, t.r, t.spin * self.time, false);
            }
        }
    }

    fn draw_blade(&self) {
        let glow = Color::from_rgba(140, 200, 255, 255);
        for w in self.blade.windows(2) {
            let k = w[1].1 / BLADE_LIFE;
            let (a, b) = (w[0].0, w[1].0);
            for (width, alpha) in [(26.0, 0.12), (16.0, 0.22), (8.0, 0.5)] {
                draw_line(
                    a.x,
                    a.y,
                    b.x,
                    b.y,
                    width * k + 2.0,
                    with_alpha(glow, alpha * k),
                );
            }
            draw_line(a.x, a.y, b.x, b.y, 2.0 + 5.0 * k, with_alpha(WHITE, k));
        }
        if let Some((tip, k)) = self.blade.last() {
            let k = k / BLADE_LIFE;
            draw_circle(tip.x, tip.y, 14.0 * k, with_alpha(glow, 0.35 * k));
            draw_circle(tip.x, tip.y, 6.0 * k, with_alpha(WHITE, 0.9 * k));
        }
    }

    fn draw_hud(&self, view: &View) {
        let s = self.score.to_string();
        text_outline(
            &s,
            HUD_LEFT + 50.0,
            50.0,
            60.0 * self.score_bump.scale(0.35),
            Color::from_rgba(255, 220, 60, 255),
            Color::from_rgba(40, 30, 60, 255),
        );
        let pulse = self.heart_bump.scale(0.5);
        for i in 0..LIVES {
            let x = view.w - 50.0 - i as f32 * 54.0;
            let alive = i < self.lives;
            let k = if i == self.lives { pulse } else { 1.0 };
            art::sprite_at(
                "heart",
                vec2(x, 48.0),
                54.0 * k,
                crate::art::Draw {
                    tint: if alive {
                        WHITE
                    } else {
                        Color::new(0.25, 0.25, 0.3, 0.45)
                    },
                    ..Default::default()
                },
            );
        }
    }
}

pub fn icon(r: Rect) {
    crate::ui::icon("fruit", r);
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
        let have = art::names();
        let mut want: Vec<String> = ["wall", "bomb", "heart"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        for f in FRUITS.iter() {
            for suffix in ["", "_half"] {
                want.push(format!("{}{}", f.name, suffix));
            }
        }
        want.push(splat_name().to_string());
        for n in want {
            assert!(have.contains(&n), "missing {n}");
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
        g.things[0].vel = vec2(0.0, 10.0);
        g.update(&Input::default(), &v, 0.01);
        assert!(g.heart_bump.t < 1.0);
    }
}
