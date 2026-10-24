use crate::engine::draw_circle;
use crate::engine::draw_ellipse;
use crate::engine::*;
use crate::fx::*;
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
}

type Rgb = (u8, u8, u8);

const FRUITS: [(Rgb, Rgb); 5] = [
    ((220, 40, 50), (255, 230, 220)),
    ((250, 150, 30), (255, 210, 120)),
    ((60, 170, 60), (240, 70, 70)),
    ((250, 220, 50), (255, 250, 200)),
    ((130, 60, 170), (220, 190, 240)),
];

const SEEDS: [Option<Rgb>; 5] = [
    Some((90, 50, 30)),
    None,
    Some((30, 25, 25)),
    Some((235, 215, 140)),
    Some((70, 40, 90)),
];

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
        let flesh = rgb(FRUITS[fruit].1);
        let skin = rgb(FRUITS[fruit].0);
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
        if let Some(seed) = SEEDS[fruit] {
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
            r: rand::gen_range(26.0, 40.0),
            color: flesh,
            age: 0.0,
            drops: [0, 1, 2].map(|_| {
                vec2(rand::gen_range(-1.0, 1.0), rand::gen_range(-1.0, 1.0))
                    * rand::gen_range(38.0, 80.0)
            }),
        });
    }

    fn explode(&mut self, at: Vec2) {
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
                    combo += 1;
                    centre += t.pos;
                    self.score += 1;
                    self.score_bump.kick();
                    let side = vec2(-dir.y, dir.x) * 120.0;
                    let angle = dir.y.atan2(dir.x);
                    for s in [-1.0, 1.0] {
                        self.halves.push(Half {
                            pos: t.pos,
                            vel: t.vel * 0.4 + side * s,
                            r: t.r,
                            fruit: f,
                            angle,
                            spin: s * 4.0,
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
                let c = rgb(FRUITS[h.fruit].1);
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
                    self.best.offer(self.score);
                }
            }
            State::Over => {
                if input.tap() && self.flash == 0.0 {
                    let best = self.best;
                    *self = Self::new();
                    self.best = best;
                    self.state = State::Playing;
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
        vignette(view, 0.8);
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
    let r = st.r * grow;
    draw_circle(st.pos.x, st.pos.y, r, c);
    for (i, d) in st.drops.iter().enumerate() {
        let dr = r * (0.28 + 0.1 * i as f32);
        draw_circle(st.pos.x + d.x * grow, st.pos.y + d.y * grow, dr, c);
    }
    let drip = st.r * 2.2 * ease_out_cubic(st.age / 2.5);
    draw_rectangle(st.pos.x - 4.0, st.pos.y, 8.0, drip, c);
    draw_circle(st.pos.x, st.pos.y + drip, 5.5, c);
    draw_circle(
        st.pos.x - r * 0.3,
        st.pos.y - r * 0.3,
        r * 0.2,
        with_alpha(WHITE, a * 0.35),
    );
}

fn draw_half(h: &Half) {
    let (skin, flesh) = FRUITS[h.fruit];
    let n = vec2(h.angle.cos(), h.angle.sin());
    let off = vec2(-n.y, n.x) * if h.spin > 0.0 { 1.0 } else { -1.0 };
    let base = h.pos;
    let steps = 12;
    let start = off.y.atan2(off.x) - std::f32::consts::FRAC_PI_2;
    let core = Color::from_rgba(flesh.0, flesh.1, flesh.2, 255);
    for i in 0..steps {
        let a0 = start + std::f32::consts::PI * i as f32 / steps as f32;
        let a1 = start + std::f32::consts::PI * (i + 1) as f32 / steps as f32;
        let p0 = vec2(a0.cos(), a0.sin());
        let p1 = vec2(a1.cos(), a1.sin());
        draw_triangle(
            base,
            base + p0 * h.r,
            base + p1 * h.r,
            darker(rgb(skin), 0.8),
        );
        draw_triangle(
            base,
            base + p0 * (h.r - 3.0),
            base + p1 * (h.r - 3.0),
            rgb(skin),
        );
        draw_triangle(
            base,
            base + p0 * (h.r - 7.0),
            base + p1 * (h.r - 7.0),
            rgb(flesh),
        );
        draw_triangle(
            base,
            base + p0 * h.r * 0.3,
            base + p1 * h.r * 0.3,
            with_alpha(darker(core, 0.9), 0.7),
        );
    }
    if let Some(seed) = SEEDS[h.fruit] {
        for k in -2..=2 {
            let kf = k as f32;
            let at = base + off * h.r * (0.45 - kf.abs() * 0.06) + n * kf * h.r * 0.2;
            draw_ellipse(
                at.x,
                at.y,
                h.r * 0.07,
                h.r * 0.12,
                off.y.atan2(off.x).to_degrees(),
                rgb(seed),
            );
        }
    } else {
        for k in 0..6 {
            let a = start + std::f32::consts::PI * (k as f32 + 0.5) / 6.0;
            let d = vec2(a.cos(), a.sin());
            draw_line(
                base.x + d.x * h.r * 0.2,
                base.y + d.y * h.r * 0.2,
                base.x + d.x * (h.r - 9.0),
                base.y + d.y * (h.r - 9.0),
                2.0,
                Color::new(1.0, 1.0, 1.0, 0.5),
            );
        }
    }
    draw_line(
        base.x - n.x * h.r,
        base.y - n.y * h.r,
        base.x + n.x * h.r,
        base.y + n.y * h.r,
        2.0,
        with_alpha(WHITE, 0.6),
    );
}

impl Fruit {
    fn draw_wall(&self, view: &View) {
        sky(
            view,
            Color::from_rgba(110, 70, 40, 255),
            Color::from_rgba(70, 40, 25, 255),
        );
        let plank = 120.0;
        for i in 0..((view.w / plank) as i32 + 1) {
            let x = i as f32 * plank;
            let tone = 0.04 * ((i * 7 % 5) as f32 - 2.0);
            draw_rectangle(
                x,
                0.0,
                plank,
                view.h,
                Color::new(1.0, 0.8, 0.5, 0.05 + tone.abs()),
            );
            for g in 0..5 {
                let gx = x + 14.0 + g as f32 * 21.0 + ((i * 13 + g * 5) % 9) as f32;
                let mut prev = vec2(gx, 0.0);
                let mut y = 0.0;
                while y < view.h {
                    y += 40.0;
                    let wob = ((y * 0.02 + (i * 3 + g) as f32).sin()) * 3.0;
                    let next = vec2(gx + wob, y);
                    draw_line(
                        prev.x,
                        prev.y,
                        next.x,
                        next.y,
                        1.5,
                        Color::new(0.2, 0.1, 0.03, 0.25),
                    );
                    prev = next;
                }
            }
            let ky = 80.0 + ((i * 197) % 420) as f32;
            draw_ellipse(
                x + 60.0,
                ky,
                13.0,
                9.0,
                0.0,
                Color::new(0.2, 0.1, 0.03, 0.4),
            );
            draw_ellipse(
                x + 60.0,
                ky,
                7.0,
                4.5,
                0.0,
                Color::new(0.1, 0.05, 0.02, 0.4),
            );
            draw_line(x, 0.0, x, view.h, 4.0, Color::from_rgba(60, 35, 20, 255));
            draw_line(
                x + 3.0,
                0.0,
                x + 3.0,
                view.h,
                2.0,
                Color::new(1.0, 0.8, 0.5, 0.12),
            );
        }
    }

    fn draw_thing(&self, t: &Thing) {
        match t.kind {
            Kind::Bomb => {
                let pulse = 0.5 + 0.5 * (self.time * 12.0).sin();
                draw_circle(
                    t.pos.x,
                    t.pos.y,
                    t.r * 1.25,
                    Color::new(1.0, 0.3, 0.2, 0.08 + 0.08 * pulse),
                );
                draw_circle(t.pos.x, t.pos.y, t.r, Color::from_rgba(25, 25, 32, 255));
                draw_circle(
                    t.pos.x,
                    t.pos.y,
                    t.r - 3.0,
                    Color::from_rgba(45, 45, 56, 255),
                );
                draw_circle(
                    t.pos.x + t.r * 0.15,
                    t.pos.y + t.r * 0.2,
                    t.r * 0.75,
                    Color::new(0.0, 0.0, 0.0, 0.25),
                );
                draw_circle(
                    t.pos.x - t.r * 0.3,
                    t.pos.y - t.r * 0.3,
                    t.r * 0.25,
                    Color::from_rgba(110, 110, 125, 255),
                );
                draw_line(
                    t.pos.x,
                    t.pos.y - t.r,
                    t.pos.x + 10.0,
                    t.pos.y - t.r - 16.0,
                    4.0,
                    Color::from_rgba(160, 120, 80, 255),
                );
                let tip = vec2(t.pos.x + 10.0, t.pos.y - t.r - 16.0);
                let spark = (self.time * 30.0).sin() * 3.0 + 7.0;
                draw_circle(tip.x, tip.y, spark + 4.0, Color::new(1.0, 0.7, 0.2, 0.3));
                star(
                    tip,
                    spark + 3.0,
                    self.time * 6.0,
                    Color::from_rgba(255, 220, 80, 255),
                );
                text_center("X", t.pos.x, t.pos.y + 2.0, t.r, RED);
            }
            Kind::Fruit(f) => {
                let (skin, _) = FRUITS[f];
                let skin = rgb(skin);
                let spin = t.spin * self.time;
                soft_shadow(
                    t.pos + vec2(t.r * 0.12, t.r * 0.16),
                    t.r * 0.9,
                    t.r * 0.9,
                    0.4,
                );
                draw_circle(t.pos.x, t.pos.y, t.r, darker(skin, 0.7));
                draw_circle(t.pos.x, t.pos.y, t.r - 3.0, skin);
                draw_circle(
                    t.pos.x + t.r * 0.18,
                    t.pos.y + t.r * 0.2,
                    t.r * 0.72,
                    Color::new(0.0, 0.0, 0.0, 0.12),
                );
                draw_circle(
                    t.pos.x - t.r * 0.08,
                    t.pos.y - t.r * 0.08,
                    t.r * 0.72,
                    with_alpha(skin, 1.0),
                );
                draw_fruit_detail(t, f, spin);
                draw_ellipse(
                    t.pos.x - t.r * 0.35,
                    t.pos.y - t.r * 0.4,
                    t.r * 0.2,
                    t.r * 0.12,
                    -40.0,
                    with_alpha(WHITE, 0.55),
                );
                draw_circle(
                    t.pos.x - t.r * 0.12,
                    t.pos.y - t.r * 0.6,
                    t.r * 0.05,
                    with_alpha(WHITE, 0.5),
                );
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
            let x = view.w - 50.0 - i as f32 * 50.0;
            let alive = i < self.lives;
            let c = if alive {
                Color::from_rgba(240, 60, 70, 255)
            } else {
                Color::from_rgba(90, 60, 50, 255)
            };
            let k = if i == self.lives { pulse } else { 1.0 };
            let (cx, cy) = (x, 48.0);
            draw_circle(cx - 8.0 * k, cy - 8.0 * k, 11.0 * k, c);
            draw_circle(cx + 8.0 * k, cy - 8.0 * k, 11.0 * k, c);
            draw_triangle(
                vec2(cx - 19.0 * k, cy - 4.0 * k),
                vec2(cx + 19.0 * k, cy - 4.0 * k),
                vec2(cx, cy + 16.0 * k),
                c,
            );
            if alive {
                draw_circle(cx - 11.0, cy - 12.0, 3.5, with_alpha(WHITE, 0.55));
            }
        }
    }
}

fn draw_fruit_detail(t: &Thing, f: usize, spin: f32) {
    let c = t.pos;
    let r = t.r;
    let dark = darker(rgb(FRUITS[f].0), 0.6);
    match f {
        0 => {
            draw_circle(c.x, c.y - r * 0.82, r * 0.14, darker(rgb(FRUITS[0].0), 0.5));
            draw_line(
                c.x,
                c.y - r * 0.85,
                c.x + 3.0,
                c.y - r - 8.0,
                3.0,
                Color::from_rgba(90, 60, 30, 255),
            );
            draw_ellipse(
                c.x + r * 0.25,
                c.y - r * 0.95,
                r * 0.3,
                r * 0.14,
                spin.to_degrees() * 0.2 - 25.0,
                Color::from_rgba(60, 170, 50, 255),
            );
        }
        1 => {
            for k in 0..9 {
                let a = spin + k as f32 * 0.7;
                let d = r * (0.2 + 0.07 * (k % 4) as f32 * 2.0);
                draw_circle(
                    c.x + a.cos() * d,
                    c.y + a.sin() * d,
                    1.6,
                    Color::new(0.7, 0.3, 0.0, 0.5),
                );
            }
            draw_circle(
                c.x,
                c.y - r * 0.82,
                r * 0.09,
                Color::from_rgba(70, 140, 40, 255),
            );
        }
        2 => {
            for k in -2..=2 {
                let a = spin + k as f32 * 0.35;
                let d = vec2(a.cos(), a.sin());
                let n = vec2(-d.y, d.x);
                let half = (r - 5.0) * (1.0 - (k as f32 * 0.3).powi(2)).sqrt();
                draw_line(
                    c.x - n.x * k as f32 * 6.0 - d.x * half,
                    c.y - n.y * k as f32 * 6.0 - d.y * half,
                    c.x - n.x * k as f32 * 6.0 + d.x * half,
                    c.y - n.y * k as f32 * 6.0 + d.y * half,
                    5.0,
                    with_alpha(dark, 0.55),
                );
            }
        }
        3 => {
            draw_circle(c.x + r * 0.95, c.y, r * 0.12, rgb(FRUITS[3].0));
            draw_circle(c.x - r * 0.95, c.y, r * 0.12, rgb(FRUITS[3].0));
            for k in 0..8 {
                let a = spin + k as f32 * 0.8;
                draw_circle(
                    c.x + a.cos() * r * 0.5,
                    c.y + a.sin() * r * 0.5,
                    1.4,
                    Color::new(0.6, 0.5, 0.0, 0.35),
                );
            }
        }
        _ => {
            for row in 0..3 {
                for col in 0..3 {
                    let at = c + vec2((col as f32 - 1.0) * r * 0.42, (row as f32 - 1.0) * r * 0.42);
                    if at.distance(c) < r * 0.8 {
                        draw_circle(at.x, at.y, r * 0.22, with_alpha(dark, 0.25));
                        draw_circle(
                            at.x - r * 0.06,
                            at.y - r * 0.06,
                            r * 0.06,
                            with_alpha(WHITE, 0.3),
                        );
                    }
                }
            }
            draw_line(
                c.x,
                c.y - r * 0.85,
                c.x + 5.0,
                c.y - r - 10.0,
                3.0,
                Color::from_rgba(70, 120, 40, 255),
            );
        }
    }
}

pub fn icon(r: Rect) {
    draw_rectangle(r.x, r.y, r.w, r.h, Color::from_rgba(110, 70, 40, 255));
    let c = vec2(r.x + r.w * 0.5, r.y + r.h * 0.55);
    draw_circle(c.x, c.y, r.w * 0.3, Color::from_rgba(60, 170, 60, 255));
    draw_circle(c.x, c.y, r.w * 0.25, Color::from_rgba(240, 70, 70, 255));
    for i in 0..5 {
        let a = i as f32 * 1.25;
        draw_circle(
            c.x + a.cos() * r.w * 0.13,
            c.y + a.sin() * r.w * 0.13,
            3.0,
            BLACK,
        );
    }
    draw_line(
        r.x + r.w * 0.1,
        r.y + r.h * 0.85,
        r.x + r.w * 0.9,
        r.y + r.h * 0.15,
        5.0,
        WHITE,
    );
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
