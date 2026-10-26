pub mod layout;
pub mod nav;
pub mod sim;

use crate::engine::*;
use crate::feel::{self, Impact};
use crate::fx::*;
use crate::geom::*;
use crate::sfx::{self, Say, Sfx};
use glam::{vec2, vec3, Vec2, Vec3};
use layout::*;
use quad_rand as rand;
use sim::*;

pub const GAME: &str = "hotel";
pub const STICK_MM: f32 = 9.0;
pub const SAVE_EVERY: f32 = 4.0;
pub const FLY_TIME: f32 = 0.55;
pub const CAM_PITCH: f32 = 0.95;
pub const CAM_PITCH_TALL: f32 = 1.05;
pub const CAM_FOV: f32 = 0.82;

#[derive(Clone, Copy, Debug, Default)]
pub struct Stick {
    pub origin: Vec2,
    pub knob: Vec2,
    pub active: bool,
}

impl Stick {
    pub fn update(&mut self, input: &Input) -> Option<Vec2> {
        let radius = STICK_MM * input.mm();
        if input.pressed {
            self.origin = input.pos;
            self.active = true;
        }
        if !input.down {
            self.active = false;
            return None;
        }
        if !self.active {
            self.origin = input.pos;
            self.active = true;
        }
        let d = input.pos - self.origin;
        let len = d.length();
        if len > radius {
            self.origin += d / len * (len - radius);
        }
        self.knob = input.pos - self.origin;
        let dead = radius * 0.12;
        (self.knob.length() > dead).then(|| self.knob / radius)
    }

    pub fn radius_px(input_mm: f32) -> f32 {
        STICK_MM * input_mm
    }
}

pub fn key_dir(input: &Input) -> Option<Vec2> {
    let x = input.right as i32 as f32 - input.left as i32 as f32;
    let y = input.down_key as i32 as f32 - input.up as i32 as f32;
    let v = vec2(x, y);
    (v != Vec2::ZERO).then(|| v.normalize())
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Cam {
    pub target: Vec3,
    pub eye: Vec3,
    pub fov: f32,
    pub aspect: f32,
}

impl Cam {
    pub fn for_view(view: &View, focus: Vec2, zoom: f32) -> Self {
        let f = view.full();
        let portrait = f.h > f.w * 1.15;
        let (dist, pitch) = if portrait {
            (19.0, CAM_PITCH_TALL)
        } else {
            (16.5, CAM_PITCH)
        };
        let dist = dist / zoom.max(0.05);
        let target = vec3(focus.x, 0.0, focus.y);
        let eye = target + vec3(0.0, pitch.sin(), pitch.cos()) * dist;
        let fov = if portrait {
            CAM_FOV * (f.h / f.w).min(1.6)
        } else {
            CAM_FOV
        };
        Self {
            target,
            eye,
            fov,
            aspect: f.w / f.h.max(1.0),
        }
    }

    pub fn forward(&self) -> Vec3 {
        (self.target - self.eye).normalize()
    }

    pub fn right(&self) -> Vec3 {
        self.forward().cross(Vec3::Y).normalize()
    }

    pub fn up(&self) -> Vec3 {
        self.right().cross(self.forward())
    }

    pub fn project(&self, view: &View, p: Vec3) -> Option<Vec2> {
        let d = p - self.eye;
        let z = d.dot(self.forward());
        if z < 0.1 {
            return None;
        }
        let f = view.full();
        let half_h = (self.fov / 2.0).tan();
        let x = d.dot(self.right()) / z / (half_h * self.aspect);
        let y = d.dot(self.up()) / z / half_h;
        Some(vec2(
            f.x + (x + 1.0) / 2.0 * f.w,
            f.y + (1.0 - y) / 2.0 * f.h,
        ))
    }

    pub fn px_per_unit(&self, view: &View, p: Vec3) -> f32 {
        let z = (p - self.eye).dot(self.forward()).max(0.1);
        view.full().h / (2.0 * (self.fov / 2.0).tan() * z)
    }

    pub fn ground_dir(&self, screen_dir: Vec2) -> Vec2 {
        let r = vec2(self.right().x, self.right().z).normalize_or_zero();
        let u = vec2(self.forward().x, self.forward().z).normalize_or_zero();
        r * screen_dir.x - u * screen_dir.y
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Flyer {
    pub from: Vec3,
    pub delay: f32,
    pub t: f32,
    pub xp: bool,
}

impl Flyer {
    pub fn k(&self) -> f32 {
        ((self.t - self.delay) / FLY_TIME).clamp(0.0, 1.0)
    }

    pub fn started(&self) -> bool {
        self.t >= self.delay
    }
}

pub fn fly_path(from: Vec2, to: Vec2, k: f32) -> Vec2 {
    let e = ease_out_cubic(k);
    let mid = from.lerp(to, 0.5) + vec2(0.0, -110.0);
    from.lerp(mid, e).lerp(mid.lerp(to, e), e)
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PopKind {
    Cash(u32),
    Money,
    Heart,
    Angry,
    Sparkle,
}

#[derive(Clone, Copy, Debug)]
pub struct Pop {
    pub at: Vec3,
    pub kind: PopKind,
    pub age: f32,
}

#[derive(Clone, Copy, Debug)]
pub struct Splash {
    pub at: Vec2,
    pub age: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Mode {
    Play,
    LevelUp(u32, f32),
    Moving(f32),
    Done(f32),
}

pub struct Hotel {
    pub sim: Sim,
    pub mode: Mode,
    pub stick: Stick,
    pub focus: Vec2,
    pub zoom: f32,
    pub t: f32,
    pub shown_wallet: f32,
    pub shown_xp: f32,
    pub wallet_bump: Bump,
    pub xp_bump: Bump,
    pub flyers: Vec<Flyer>,
    pub pops: Vec<Pop>,
    pub splashes: Vec<Splash>,
    pub built_age: Vec<(Build, f32)>,
    pub shake: Shake,
    pub bursts: Vec<(Vec3, Color)>,
    save_in: f32,
    cash_streak: usize,
    cash_streak_t: f32,
    spend_tick: f32,
    pub learned_move: bool,
    pub fixed_focus: Option<Vec2>,
}

pub fn encode(number: u32, built: &[bool], wallet: u32, level: u32, xp: u32) -> String {
    let bits: String = built.iter().map(|b| if *b { '1' } else { '0' }).collect();
    format!("{number}:{wallet}:{bits}:{level}:{xp}")
}

pub fn decode(s: &str) -> Option<(u32, u32, Vec<bool>, u32, u32)> {
    let mut f = s.split(':');
    let number: u32 = f.next()?.parse().ok()?;
    let wallet: u32 = f.next()?.parse().ok()?;
    let built = f.next()?.chars().map(|c| c == '1').collect();
    let level = f.next().and_then(|v| v.parse().ok()).unwrap_or(1);
    let xp = f.next().and_then(|v| v.parse().ok()).unwrap_or(0);
    Some((number.clamp(1, MAX_HOTEL), wallet, built, level, xp))
}

fn state_key() -> String {
    crate::save::player_key(&format!("{GAME}.state2"))
}

pub fn load_sim(seed: u64) -> Sim {
    let saved = crate::save::get_str(&state_key()).and_then(|s| decode(&s));
    match saved {
        Some((number, wallet, built, level, xp)) => {
            let mut sim = Sim::new(number, &built, wallet, seed);
            sim.level = sim.level.max(level).min(sim.lay.max_level);
            sim.xp = xp;
            sim
        }
        None => Sim::new(1, &[], 0, seed),
    }
}

pub fn store_state(sim: &Sim) {
    crate::save::set_str(
        &state_key(),
        &encode(sim.number(), &sim.built, sim.wallet, sim.level, sim.xp),
    );
}

pub fn build_icon(b: Build) -> &'static str {
    match b {
        Build::Room(_) | Build::Decor(..) => "bed",
        Build::Toilets | Build::Janitor => "paper",
        Build::Cafe | Build::Waiter => "burger",
        Build::Pool | Build::Lifeguard => "ring",
        Build::Clerk => "heart",
        Build::Maid | Build::Mop => "broom",
        Build::Shoes(_) => "shoe",
        Build::Stack(_) => "towels",
        Build::Gate(_) => "key",
        Build::Next => "cash",
    }
}

pub fn build_label(b: Build) -> String {
    match b {
        Build::Room(_) => "New room".into(),
        Build::Decor(_, l) => format!("Room level {l}"),
        Build::Gate(_) => "Open hallway".into(),
        Build::Toilets => "Toilets".into(),
        Build::Cafe => "Cafe".into(),
        Build::Pool => "Pool".into(),
        Build::Clerk => "Hire clerk".into(),
        Build::Maid => "Hire maid".into(),
        Build::Janitor => "Hire janitor".into(),
        Build::Waiter => "Hire waiter".into(),
        Build::Lifeguard => "Hire lifeguard".into(),
        Build::Mop => "Fast mops".into(),
        Build::Shoes(l) => format!("Shoes {}", l + 1),
        Build::Stack(l) => format!("Carry {}", STACKS[l as usize]),
        Build::Next => "Next hotel".into(),
    }
}

pub fn want_icon(st: Stage) -> Option<&'static str> {
    match st {
        Stage::Queue => Some("bed"),
        Stage::WaitStall => Some("paper"),
        Stage::WaitSnack(_) => Some("burger"),
        Stage::WaitTowel(_) => Some("ring"),
        Stage::Sleep(..) => Some("moon"),
        _ => None,
    }
}

pub fn item_icon(item: Item) -> &'static str {
    match item {
        Item::Paper => "paper",
        Item::Snack => "burger",
        Item::Towel => "towels",
    }
}

pub const SPRITES: [&str; 13] = [
    "parquet",
    "roomfloor",
    "cash",
    "heart",
    "moon",
    "broom",
    "paper",
    "burger",
    "ring",
    "bed",
    "towels",
    "shoe",
    "key",
];

impl Default for Hotel {
    fn default() -> Self {
        Self::new()
    }
}

impl Hotel {
    pub fn new() -> Self {
        Self::with(load_sim(rand::rand() as u64))
    }

    pub fn with(sim: Sim) -> Self {
        let focus = sim.player.pos;
        if crate::profile::unlocked(GAME) == 0 {
            crate::profile::unlock(GAME, 1);
        }
        let wallet = sim.wallet as f32;
        Self {
            sim,
            mode: Mode::Play,
            stick: Stick::default(),
            focus,
            zoom: 1.0,
            t: 0.0,
            shown_wallet: wallet,
            shown_xp: 0.0,
            wallet_bump: Bump::default(),
            xp_bump: Bump::default(),
            flyers: vec![],
            pops: vec![],
            splashes: vec![],
            built_age: vec![],
            shake: Shake::default(),
            bursts: vec![],
            save_in: SAVE_EVERY,
            cash_streak: 0,
            cash_streak_t: 0.0,
            spend_tick: 0.0,
            learned_move: crate::profile::get(GAME, "moved") > 0,
            fixed_focus: None,
        }
    }

    pub fn fresh_for_test(number: u32) -> Self {
        Self::with(Sim::new(number, &[], 0, 7))
    }

    pub fn look_at(&mut self, at: Vec2, zoom: f32) {
        self.zoom = zoom;
        self.fixed_focus = Some(at);
        self.focus = at;
    }

    pub fn overview(&mut self) {
        let mid = vec2(0.0, -(self.sim.lay.wings as f32) * WING_D / 2.0 + 2.0);
        self.look_at(mid, 0.45);
    }

    pub fn cam(&self, view: &View) -> Cam {
        Cam::for_view(view, self.focus, self.zoom)
    }

    pub fn pop_of(&self, b: Build) -> f32 {
        self.built_age
            .iter()
            .find(|(x, _)| *x == b)
            .map(|(_, t)| ease_out_back((t / 0.5).min(1.0)).max(0.01))
            .unwrap_or(1.0)
    }

    pub fn score(&self) -> i64 {
        let n = self.sim.number() as i64;
        let built = self.sim.built.iter().filter(|b| **b).count() as i64;
        (n - 1) * 100 + built * 3 + self.sim.level as i64 * 5
    }

    pub fn guide(&self) -> Option<Task> {
        if self.mode != Mode::Play {
            return None;
        }
        let early = self.sim.number() == 1 && self.sim.level <= 2;
        let guided = early || self.sim.idle > 4.0 + self.sim.lay.guide_after;
        guided.then(|| self.sim.next_task()).flatten()
    }

    pub fn wallet_rect(view: &View) -> Rect {
        Rect::new(view.w - 196.0, 12.0, 182.0, 52.0)
    }

    pub fn level_rect(view: &View) -> Rect {
        Rect::new(view.w / 2.0 - 150.0, 16.0, 300.0, 26.0)
    }

    pub fn carry_rect(view: &View) -> Rect {
        Rect::new(14.0, view.h - 74.0, 150.0, 58.0)
    }

    pub fn coin_target(view: &View) -> Vec2 {
        let w = Self::wallet_rect(view);
        vec2(w.x + 30.0, w.y + w.h / 2.0)
    }

    pub fn xp_target(view: &View) -> Vec2 {
        let b = Self::level_rect(view);
        vec2(b.x + 4.0, b.y + b.h / 2.0)
    }

    pub fn next_theme(&self) -> Theme {
        THEMES[(self.sim.number() as usize).min(THEMES.len() - 1)]
    }

    fn finish_hotel(&mut self) {
        let n = self.sim.number();
        crate::profile::award_stars(GAME, n, 3);
        crate::profile::unlock(GAME, (n + 1).min(MAX_HOTEL));
        crate::profile::submit(GAME, self.score(), crate::profile::Better::Higher);
        self.mode = if n >= MAX_HOTEL {
            Mode::Done(0.0)
        } else {
            Mode::Moving(0.0)
        };
        sfx::play(Sfx::Win);
        sfx::say(Say::Congrats);
        feel::impact_tinted(Impact::Large, GOLD);
    }

    fn move_on(&mut self) {
        let n = self.sim.number();
        let wallet = self.sim.wallet;
        self.sim = Sim::new(n + 1, &[], wallet, rand::rand() as u64);
        self.focus = self.sim.player.pos;
        self.shown_xp = 0.0;
        store_state(&self.sim);
        crate::profile::set_level(GAME, n + 1);
        self.mode = Mode::Play;
        self.flyers.clear();
        self.pops.clear();
        self.built_age.clear();
        sfx::say(Say::Go);
    }

    fn pop(&mut self, at: Vec3, kind: PopKind) {
        self.pops.push(Pop { at, kind, age: 0.0 });
    }

    fn burst_at(&mut self, at: Vec2, y: f32, color: Color) {
        self.bursts.push((vec3(at.x, y, at.y), color));
    }

    pub fn react(&mut self) {
        let events = std::mem::take(&mut self.sim.events);
        for e in events {
            match e {
                Event::Cash { at, amount } => {
                    let n = (amount as usize / 3).clamp(3, 10);
                    for k in 0..n {
                        self.flyers.push(Flyer {
                            from: vec3(at.x, 0.4, at.y),
                            delay: k as f32 * 0.035,
                            t: 0.0,
                            xp: false,
                        });
                    }
                    self.pop(vec3(at.x, 1.2, at.y), PopKind::Cash(amount));
                    self.cash_streak = if self.cash_streak_t > 0.0 {
                        self.cash_streak + 1
                    } else {
                        0
                    };
                    self.cash_streak_t = 1.2;
                    sfx::play_step(Sfx::Coin, self.cash_streak);
                    feel::impact(Impact::Small);
                }
                Event::Paid { at } => self.pop(vec3(at.x, 1.0, at.y), PopKind::Money),
                Event::CheckIn { at } => {
                    sfx::play(Sfx::Right);
                    self.burst_at(at, 1.6, Color::from_rgba(255, 120, 140, 255));
                    self.pop(vec3(at.x, 2.0, at.y), PopKind::Heart);
                }
                Event::SpotClean { at } => {
                    sfx::play(Sfx::Pop);
                    self.burst_at(at, 0.6, Color::from_rgba(150, 230, 255, 255));
                }
                Event::RoomClean { at } => {
                    sfx::play(Sfx::Magic);
                    self.pop(vec3(at.x, 1.4, at.y), PopKind::Sparkle);
                    self.burst_at(at, 1.0, Color::from_rgba(255, 240, 120, 255));
                }
                Event::Served { at } => {
                    sfx::play(Sfx::Pop);
                    self.burst_at(at, 1.2, Color::from_rgba(255, 200, 80, 255));
                }
                Event::Angry { at } => {
                    sfx::play(Sfx::Wrong);
                    self.pop(vec3(at.x, 2.0, at.y), PopKind::Angry);
                }
                Event::Grab { .. } => sfx::play(Sfx::Card),
                Event::Xp { at } => self.flyers.push(Flyer {
                    from: vec3(at.x, 1.0, at.y),
                    delay: 0.0,
                    t: 0.0,
                    xp: true,
                }),
                Event::LevelUp { level } => {
                    sfx::play(Sfx::LevelUp);
                    sfx::say(Say::LevelUp);
                    feel::impact_tinted(Impact::Medium, GOLD);
                    self.shown_xp = 0.0;
                    if self.mode == Mode::Play {
                        self.mode = Mode::LevelUp(level, 0.0);
                    }
                }
                Event::Splash { at } => {
                    self.splashes.push(Splash { at, age: 0.0 });
                    sfx::play(Sfx::Splash);
                }
                Event::Built { build, at } => {
                    self.built_age.push((build, 0.0));
                    if self.sim.complete() {
                        self.finish_hotel();
                    } else {
                        sfx::play(Sfx::Score);
                        sfx::say(Say::Great);
                        self.shake.add(0.3);
                        feel::impact(Impact::Medium);
                        self.burst_at(at, 0.3, GOLD);
                    }
                    store_state(&self.sim);
                }
                Event::Spend => {
                    if self.spend_tick <= 0.0 {
                        sfx::play(Sfx::Tick);
                        self.spend_tick = 0.07;
                    }
                }
            }
        }
    }

    pub fn tick(&mut self, dt: f32) {
        self.spend_tick -= dt;
        self.cash_streak_t -= dt;
        self.sim.step(dt);
        self.react();
        self.save_in -= dt;
        if self.save_in <= 0.0 {
            self.save_in = SAVE_EVERY;
            store_state(&self.sim);
        }
    }

    fn age_effects(&mut self, dt: f32) {
        self.wallet_bump.update(dt);
        self.xp_bump.update(dt);
        self.shake.update(dt);
        for p in &mut self.pops {
            p.age += dt;
        }
        self.pops.retain(|p| p.age < 1.1);
        for s in &mut self.splashes {
            s.age += dt;
        }
        self.splashes.retain(|s| s.age < 0.8);
        for b in &mut self.built_age {
            b.1 += dt;
        }
        self.built_age.retain(|b| b.1 < 1.0);
        let (mut coins, mut xps) = (0, 0);
        for f in &mut self.flyers {
            f.t += dt;
            if f.t - f.delay >= FLY_TIME {
                if f.xp {
                    xps += 1;
                } else {
                    coins += 1;
                }
            }
        }
        self.flyers.retain(|f| f.t - f.delay < FLY_TIME);
        if coins > 0 {
            self.wallet_bump.kick();
        }
        if xps > 0 {
            self.xp_bump.kick();
        }
        self.shown_wallet = approach(self.shown_wallet, self.sim.wallet as f32, 14.0, dt);
        if (self.shown_wallet - self.sim.wallet as f32).abs() < 0.5 {
            self.shown_wallet = self.sim.wallet as f32;
        }
        let xp_k = self.sim.xp as f32 / self.sim.xp_needed().max(1) as f32;
        self.shown_xp = approach(self.shown_xp, xp_k, 8.0, dt);
    }
}

impl Game for Hotel {
    fn update(&mut self, input: &Input, view: &View, dt: f32) {
        self.t += dt;
        self.age_effects(dt);
        match self.mode {
            Mode::Play => {
                let dir = key_dir(input).or_else(|| self.stick.update(input));
                let world = dir.map(|d| self.cam(view).ground_dir(d));
                if world.is_some() && !self.learned_move {
                    self.learned_move = true;
                    crate::profile::set(GAME, "moved", 1);
                }
                self.sim.steer(world, dt);
                self.tick(dt);
            }
            Mode::LevelUp(l, t) => {
                self.sim.steer(None, dt);
                self.mode = Mode::LevelUp(l, t + dt);
                if t > 2.2 || (t > 0.6 && input.pressed) {
                    self.mode = Mode::Play;
                    self.stick.active = false;
                }
            }
            Mode::Moving(t) => {
                self.sim.steer(None, dt);
                self.mode = Mode::Moving(t + dt);
                if t + dt > 3.2 || (t > 1.2 && input.pressed) {
                    self.move_on();
                }
            }
            Mode::Done(t) => {
                self.sim.steer(None, dt);
                self.tick(dt);
                self.mode = Mode::Done(t + dt);
                if t > 2.5 && input.pressed {
                    self.mode = Mode::Play;
                }
            }
        }
        let target = self
            .fixed_focus
            .unwrap_or(self.sim.player.pos + vec2(0.0, 0.6));
        self.focus = self.focus.lerp(target, 1.0 - (-6.0 * dt).exp());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fresh() {
        crate::save::reset_for_test();
        crate::profile::create("Mia", 0);
    }

    fn play(h: &mut Hotel, frames: usize) {
        let view = View::ipad_landscape();
        for _ in 0..frames {
            h.update(&Input::default(), &view, 1.0 / 60.0);
        }
    }

    #[test]
    fn state_round_trips() {
        let s = encode(3, &[true, false, true], 42, 5, 7);
        assert_eq!(decode(&s), Some((3, 42, vec![true, false, true], 5, 7)));
        assert_eq!(decode("junk"), None);
        assert_eq!(decode("9:1:1").unwrap().0, MAX_HOTEL);
    }

    #[test]
    fn progress_level_and_cash_survive_leaving_the_game() {
        fresh();
        let mut h = Hotel::fresh_for_test(1);
        h.sim.wallet = 200;
        h.sim.player.pos = h.sim.lay.pads[0].pos;
        for _ in 0..120 {
            h.tick(1.0 / 60.0);
        }
        assert!(h.sim.built[0]);
        let need = h.sim.xp_needed();
        h.sim.gain_xp(need + 1, Vec2::ZERO);
        let (wallet, level, xp) = (h.sim.wallet, h.sim.level, h.sim.xp);
        store_state(&h.sim);
        let back = Hotel::new();
        assert!(back.sim.built[0]);
        assert_eq!(back.sim.wallet, wallet);
        assert_eq!(back.sim.level, level);
        assert_eq!(back.sim.xp, xp);
        assert_eq!(back.sim.open_rooms(), 2);
    }

    #[test]
    fn a_level_up_pauses_for_a_moment_then_play_resumes() {
        fresh();
        let mut h = Hotel::fresh_for_test(1);
        let need = h.sim.xp_needed();
        h.sim.gain_xp(need, Vec2::ZERO);
        h.react();
        assert!(matches!(h.mode, Mode::LevelUp(2, _)));
        play(&mut h, 200);
        assert_eq!(h.mode, Mode::Play);
    }

    #[test]
    fn buying_the_next_hotel_moves_you_there_with_your_cash() {
        fresh();
        let n = Layout::new(1).pads.len();
        let mut built = vec![true; n];
        built[n - 1] = false;
        let mut h = Hotel::with(Sim::new(1, &built, 900, 1));
        h.sim.player.pos = h.sim.lay.pads[n - 1].pos;
        let cost = Layout::new(1).pads[n - 1].cost;
        play(&mut h, 200);
        assert!(matches!(h.mode, Mode::Moving(_)), "{:?}", h.mode);
        play(&mut h, 240);
        assert_eq!(h.mode, Mode::Play);
        assert_eq!(h.sim.number(), 2);
        assert_eq!(h.sim.level, 1);
        assert_eq!(h.sim.wallet, 900 - cost);
        assert_eq!(crate::profile::unlocked(GAME), 2);
        assert_eq!(Hotel::new().sim.number(), 2);
    }

    #[test]
    fn a_finished_last_hotel_celebrates_once_then_lets_you_play() {
        fresh();
        let n = Layout::new(MAX_HOTEL).pads.len();
        store_state(&Sim::new(MAX_HOTEL, &vec![true; n], 50, 1));
        let mut h = Hotel::new();
        let view = View::ipad_landscape();
        let tap = Input::press_at(view.center());
        for f in 0..600 {
            let i = if f % 60 == 59 {
                tap.clone()
            } else {
                Input::default()
            };
            h.update(&i, &view, 1.0 / 60.0);
        }
        assert_eq!(h.mode, Mode::Play);
        assert_eq!(crate::profile::board(GAME).len(), 0);
    }

    #[test]
    fn dragging_up_the_screen_walks_north() {
        fresh();
        let mut h = Hotel::fresh_for_test(1);
        let view = View::ipad_landscape();
        let start = h.sim.player.pos;
        let c = view.center() + vec2(200.0, 200.0);
        h.update(&Input::press_at(c), &view, 1.0 / 60.0);
        for _ in 0..20 {
            let p = c + vec2(0.0, -80.0);
            let i = Input {
                pos: p,
                prev_pos: p,
                down: true,
                ..Default::default()
            };
            h.update(&i, &view, 1.0 / 60.0);
        }
        let moved = h.sim.player.pos - start;
        assert!(moved.y < -0.5 && moved.x.abs() < 0.1, "{moved}");
        assert!(h.learned_move);
    }

    #[test]
    fn arrow_keys_walk_too() {
        fresh();
        let mut h = Hotel::fresh_for_test(1);
        let view = View::ipad_landscape();
        let start = h.sim.player.pos;
        for _ in 0..20 {
            let i = Input {
                left: true,
                ..Default::default()
            };
            h.update(&i, &view, 1.0 / 60.0);
        }
        assert!(h.sim.player.pos.x < start.x - 0.5);
    }

    #[test]
    fn the_guide_arrow_shows_early_and_later_only_when_idle() {
        fresh();
        let h = Hotel::fresh_for_test(1);
        assert!(h.guide().is_some() || h.sim.next_task().is_none());
        let mut late = Hotel::fresh_for_test(4);
        late.sim.idle = 1.0;
        assert!(late.guide().is_none());
        late.sim.idle = 100.0;
        late.sim.wallet = 999;
        assert!(late.guide().is_some());
    }

    #[test]
    fn every_build_has_a_label_and_a_sprite() {
        for n in 1..=MAX_HOTEL {
            for p in &Layout::new(n).pads {
                assert!(!build_label(p.build).is_empty());
                assert!(SPRITES.contains(&build_icon(p.build)));
            }
        }
        for st in [
            Stage::Queue,
            Stage::WaitStall,
            Stage::WaitSnack(0),
            Stage::WaitTowel(0),
            Stage::Sleep(0, 1.0),
        ] {
            assert!(SPRITES.contains(&want_icon(st).unwrap()));
        }
        for i in [Item::Paper, Item::Snack, Item::Towel] {
            assert!(SPRITES.contains(&item_icon(i)));
        }
    }

    #[test]
    fn the_level_bar_stays_centred_and_clear_of_the_wallet() {
        for view in [View::ipad_landscape(), View::ipad_portrait()] {
            let bar = Hotel::level_rect(&view);
            assert!((bar.center().x - view.w / 2.0).abs() < 0.5);
            assert!(bar.x + bar.w < Hotel::wallet_rect(&view).x);
        }
    }

    #[test]
    fn the_camera_frames_the_player_in_both_orientations() {
        for view in [View::ipad_landscape(), View::ipad_portrait()] {
            let c = Cam::for_view(&view, vec2(0.0, 1.2), 1.0);
            let p = c.project(&view, vec3(0.0, 0.0, 1.2)).unwrap();
            assert!((p - view.full().center()).length() < 1.0, "{p}");
            let desk = c.project(&view, vec3(0.0, 0.0, 2.5)).unwrap();
            assert!(view.full().contains(desk));
            let north = c.project(&view, vec3(0.0, 0.0, -2.0)).unwrap();
            assert!(north.y < p.y);
            let east = c.project(&view, vec3(2.0, 0.0, 1.2)).unwrap();
            assert!(east.x > p.x);
        }
    }

    #[test]
    fn the_coin_trail_ends_on_the_wallet() {
        let to = Hotel::coin_target(&View::ipad_landscape());
        assert!((fly_path(vec2(100.0, 500.0), to, 1.0) - to).length() < 0.01);
        assert!(fly_path(vec2(100.0, 500.0), to, 0.5).y < 500.0);
    }
}
