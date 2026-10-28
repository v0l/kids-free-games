pub mod bots;
pub mod powers;
pub mod sim;
pub mod skins;
pub mod tower;

use crate::engine::*;
use crate::feel::{self, Impact};
use crate::fx::*;
use crate::geom::*;
use crate::sfx::{self, Sfx};
use glam::{vec2, Vec2};
use sim::{Event, Spec, World};
use skins::{Stats, SKINS};
use tower::{Floor, Goal};

pub const GAME: &str = "clash";
pub const DEATH_HOLD: f32 = 1.0;
pub const CLEAR_HOLD: f32 = 1.4;
pub const COMBO_WINDOW: f32 = 0.7;
pub const RESULTS_DELAY: f32 = 0.5;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Screen {
    Title,
    Tower,
    Playing,
    Over,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Arena,
    Tower,
}

#[derive(Clone, Debug, Default)]
pub struct Outcome {
    pub level: u32,
    pub eaten: u32,
    pub cleared: bool,
    pub new_skins: Vec<usize>,
    pub new_best: bool,
    pub floor: u32,
}

pub struct Clash {
    pub screen: Screen,
    pub mode: Mode,
    pub world: World,
    pub floor: Floor,
    pub pick_floor: u32,
    pub stats: Stats,
    pub skin: usize,
    pub t: f32,
    pub run_t: f32,
    pub cam: Vec2,
    pub zoom: f32,
    pub fx: Particles,
    pub shake: Shake,
    pub popups: Popups,
    pub bump: Bump,
    pub rings: Vec<(Vec2, f32, Color)>,
    pub flash: f32,
    pub best: Best,
    pub rank: Option<usize>,
    pub outcome: Outcome,
    pub training: bool,
    pub help: u32,
    pub ending: f32,
    pub combo: u32,
    pub combo_t: f32,
    pub steer: f32,
    pub toast: Option<(String, f32)>,
    pub press: Option<Press>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Press {
    Arena,
    Tower,
    Prev,
    Next,
    Play,
    Back,
    Again,
    Menu,
}

pub fn read_stats() -> Stats {
    Stats {
        best_level: crate::profile::get(GAME, "best_level").max(0) as u32,
        eaten: crate::profile::get(GAME, "eaten").max(0) as u32,
        floor: crate::profile::get(GAME, "floor").max(0) as u32,
        games: crate::profile::get(GAME, "games").max(0) as u32,
    }
}

fn write_stats(s: &Stats) {
    crate::profile::set(GAME, "best_level", s.best_level as i64);
    crate::profile::set(GAME, "eaten", s.eaten as i64);
    crate::profile::set(GAME, "floor", s.floor as i64);
    crate::profile::set(GAME, "games", s.games as i64);
}

pub fn title_layout(view: &View) -> TitleLayout {
    let cx = view.w / 2.0;
    let bw = (view.w * 0.8).min(460.0);
    let bh = 84.0;
    let tall = view.h > view.w;
    let preview_y = view.h * if tall { 0.36 } else { 0.4 };
    let arrow = 74.0;
    let reach = (view.w * 0.38).min(250.0);
    TitleLayout {
        preview: vec2(cx, preview_y),
        prev: Rect::new(
            cx - reach - arrow / 2.0,
            preview_y - arrow / 2.0,
            arrow,
            arrow,
        ),
        next: Rect::new(
            cx + reach - arrow / 2.0,
            preview_y - arrow / 2.0,
            arrow,
            arrow,
        ),
        arena: Rect::new(cx - bw / 2.0, view.h * 0.66, bw, bh),
        tower: Rect::new(cx - bw / 2.0, view.h * 0.66 + bh + 18.0, bw, bh),
    }
}

pub struct TitleLayout {
    pub preview: Vec2,
    pub prev: Rect,
    pub next: Rect,
    pub arena: Rect,
    pub tower: Rect,
}

pub fn tower_layout(view: &View) -> TowerLayout {
    let cx = view.w / 2.0;
    let arrow = 74.0;
    let reach = (view.w * 0.38).min(250.0);
    let y = view.h * 0.4;
    let bw = (view.w * 0.7).min(400.0);
    TowerLayout {
        card: Rect::new(cx - reach + 20.0, y - 150.0, reach * 2.0 - 40.0, 300.0),
        prev: Rect::new(
            cx - reach - arrow / 2.0 - 20.0,
            y - arrow / 2.0,
            arrow,
            arrow,
        ),
        next: Rect::new(
            cx + reach - arrow / 2.0 + 20.0,
            y - arrow / 2.0,
            arrow,
            arrow,
        ),
        play: Rect::new(cx - bw / 2.0, view.h * 0.72, bw, 90.0),
        back: menu_back(),
    }
}

pub struct TowerLayout {
    pub card: Rect,
    pub prev: Rect,
    pub next: Rect,
    pub play: Rect,
    pub back: Rect,
}

pub fn over_buttons(view: &View) -> (Rect, Rect) {
    (
        view.rect(),
        Rect::new(
            20.0,
            view.h - 92.0 - view.inset.bottom.min(30.0),
            72.0,
            72.0,
        ),
    )
}

pub fn menu_back() -> Rect {
    Rect::new(14.0, 76.0, 64.0, 64.0)
}

impl Default for Clash {
    fn default() -> Self {
        Self::new()
    }
}

impl Clash {
    pub fn new() -> Self {
        let stats = read_stats();
        let skin = (crate::profile::get(GAME, "skin").max(0) as usize).min(SKINS.len() - 1);
        let skin = if skins::unlocked(skin, &stats) {
            skin
        } else {
            0
        };
        let floor = tower::floor(1);
        let help = crate::profile::get(GAME, "struggle").clamp(0, 3) as u32;
        let mut c = Self {
            screen: Screen::Title,
            mode: Mode::Arena,
            world: World::new(Spec::arena(1, true), "You", skin),
            floor,
            pick_floor: (stats.floor + 1).max(1),
            stats,
            skin,
            t: 0.0,
            run_t: 0.0,
            cam: Vec2::ZERO,
            zoom: 1.0,
            fx: Particles::default(),
            shake: Shake::default(),
            popups: Popups::default(),
            bump: Bump::default(),
            rings: vec![],
            flash: 0.0,
            best: Best::load(GAME),
            rank: None,
            outcome: Outcome::default(),
            training: false,
            help,
            ending: 0.0,
            combo: 0,
            combo_t: 0.0,
            steer: 0.0,
            toast: None,
            press: None,
        };
        c.show_title();
        c
    }

    pub fn is_training(&self) -> bool {
        self.stats.games < 2 || self.help >= 2
    }

    fn show_title(&mut self) {
        self.screen = Screen::Title;
        self.stats = read_stats();
        let mut spec = Spec::arena(1, false);
        spec.bots = 7;
        spec.eggs = 200;
        spec.size = 1800.0;
        self.world = World::new(spec, "You", self.skin);
        self.world.snakes[0].shield = 0.0;
        self.cam = self.world.player().head();
        self.zoom = 1.0;
        self.clear_fx();
    }

    fn clear_fx(&mut self) {
        self.fx = Particles::default();
        self.popups = Popups::default();
        self.rings.clear();
        self.shake = Shake::default();
        self.flash = 0.0;
        self.combo = 0;
    }

    pub fn start(&mut self, mode: Mode, floor: u32) {
        self.mode = mode;
        self.stats = read_stats();
        self.clear_fx();
        self.training = self.is_training();
        let name = "You";
        match mode {
            Mode::Arena => {
                let spec = Spec::arena(self.stats.best_level, self.training);
                self.world = World::new(spec, name, self.skin);
            }
            Mode::Tower => {
                self.floor = tower::floor(floor);
                self.world = self.floor.build(name, self.skin);
            }
        }
        self.screen = Screen::Playing;
        self.run_t = 0.0;
        self.ending = 0.0;
        self.rank = None;
        self.cam = self.world.player().head();
        self.zoom = self.target_zoom_for(Vec2::new(1024.0, 768.0));
        self.steer = self.world.player().angle;
        let banner = match (mode, self.floor.goal) {
            (Mode::Tower, Goal::Boss(_)) => Some("BOSS!".to_string()),
            (Mode::Tower, _) => Some(format!("Floor {}", self.floor.n)),
            _ => None,
        };
        self.toast = banner.map(|b| (b, 2.2));
        sfx::play(Sfx::Whoosh);
        sfx::say(sfx::Say::Go);
    }

    pub fn target_zoom_for(&self, size: Vec2) -> f32 {
        let level = self.world.player().level().min(40) as f32;
        let span = 460.0 + 15.0 * level + self.world.player().radius() * 3.0;
        size.x.min(size.y).max(300.0) / span
    }

    pub fn to_screen(&self, view: &View, p: Vec2) -> Vec2 {
        (p - self.cam) * self.zoom + self.centre(view)
    }

    pub fn centre(&self, view: &View) -> Vec2 {
        vec2(view.w / 2.0, view.h * 0.5)
    }

    pub fn to_world(&self, view: &View, p: Vec2) -> Vec2 {
        (p - self.centre(view)) / self.zoom + self.cam
    }

    pub fn playing(&self) -> bool {
        self.screen == Screen::Playing
    }

    pub fn shows_results(&self) -> bool {
        self.screen == Screen::Over && self.ending >= RESULTS_DELAY
    }

    pub fn skin_open(&self, i: usize) -> bool {
        skins::unlocked(i, &self.stats)
    }

    pub fn goal_text(goal: Goal) -> (&'static str, u32) {
        match goal {
            Goal::Level(n) => ("level", n),
            Goal::Eat(n) => ("eat", n),
            Goal::Survive(t) => ("time", t as u32),
            Goal::Boss(n) => ("boss", n),
        }
    }

    pub fn update(&mut self, input: &Input, view: &View, dt: f32) {
        self.t += dt;
        self.fx.update(dt);
        self.shake.update(dt);
        self.popups.update(dt);
        self.bump.update(dt);
        self.flash = (self.flash - dt * 3.0).max(0.0);
        for r in &mut self.rings {
            r.1 += dt;
        }
        self.rings.retain(|r| r.1 < 0.6);
        if let Some(t) = &mut self.toast {
            t.1 -= dt;
        }
        if self.toast.as_ref().is_some_and(|t| t.1 <= 0.0) {
            self.toast = None;
        }
        self.combo_t -= dt;
        if self.combo_t <= 0.0 {
            self.combo = 0;
        }
        self.press = None;
        self.follow(view, dt);
        match self.screen {
            Screen::Title => self.update_title(input, view, dt),
            Screen::Tower => self.update_tower(input, view, dt),
            Screen::Playing => self.update_playing(input, view, dt),
            Screen::Over => self.update_over(input, view, dt),
        }
    }

    fn follow(&mut self, view: &View, dt: f32) {
        let head = self.world.player().head();
        let k = (dt * 7.0).min(1.0);
        self.cam += (head - self.cam) * k;
        let want = self.target_zoom_for(vec2(view.w, view.h));
        self.zoom += (want - self.zoom) * (dt * 2.0).min(1.0);
    }

    fn drift(&mut self, dt: f32) {
        let a = self.world.autopilot();
        self.world.step(dt, Some(a));
        self.world.events.clear();
    }

    fn update_title(&mut self, input: &Input, view: &View, dt: f32) {
        self.drift(dt);
        let l = title_layout(view);
        let dir = if input.pressed && l.prev.contains(input.pos) {
            Some(-1)
        } else if input.pressed && l.next.contains(input.pos) {
            Some(1)
        } else if input.key_dir == Some(Dir::Left) {
            Some(-1)
        } else if input.key_dir == Some(Dir::Right) {
            Some(1)
        } else {
            None
        };
        if let Some(d) = dir {
            self.cycle_skin(d);
        }
        if input.pressed {
            if l.arena.contains(input.pos) {
                self.press = Some(Press::Arena);
            } else if l.tower.contains(input.pos) {
                self.press = Some(Press::Tower);
            }
        }
        if input.action {
            self.press = Some(Press::Arena);
        }
        match self.press {
            Some(Press::Arena) => {
                sfx::play(Sfx::Click);
                self.start(Mode::Arena, 1);
            }
            Some(Press::Tower) => {
                sfx::play(Sfx::Click);
                self.pick_floor = (self.stats.floor + 1).max(1);
                self.screen = Screen::Tower;
            }
            _ => {}
        }
    }

    pub fn cycle_skin(&mut self, d: i32) {
        let n = SKINS.len() as i32;
        self.skin = (self.skin as i32 + d).rem_euclid(n) as usize;
        self.world.snakes[0].skin = self.skin;
        self.bump.kick();
        if self.skin_open(self.skin) {
            sfx::play(Sfx::Tick);
            crate::profile::set(GAME, "skin", self.skin as i64);
        } else {
            sfx::play(Sfx::Tap);
        }
    }

    fn update_tower(&mut self, input: &Input, view: &View, dt: f32) {
        self.drift(dt);
        let l = tower_layout(view);
        let top = self.stats.floor + 1;
        let mut d = 0;
        if input.pressed && l.prev.contains(input.pos) || input.key_dir == Some(Dir::Left) {
            d = -1;
        }
        if input.pressed && l.next.contains(input.pos) || input.key_dir == Some(Dir::Right) {
            d = 1;
        }
        if d != 0 {
            let to = (self.pick_floor as i32 + d).clamp(1, top as i32) as u32;
            if to != self.pick_floor {
                self.pick_floor = to;
                self.bump.kick();
                sfx::play(Sfx::Tick);
            }
        }
        if input.pressed && l.back.contains(input.pos) || input.back {
            sfx::play(Sfx::Click);
            self.screen = Screen::Title;
            return;
        }
        if input.tapped_in(l.play) || input.action {
            sfx::play(Sfx::Click);
            self.press = Some(Press::Play);
            self.start(Mode::Tower, self.pick_floor);
        }
    }

    fn update_playing(&mut self, input: &Input, view: &View, dt: f32) {
        if input.back {
            self.show_title();
            return;
        }
        if self.ending > 0.0 {
            self.ending += dt;
            let hold = if self.world.player_alive() {
                CLEAR_HOLD
            } else {
                DEATH_HOLD
            };
            if self.ending >= hold {
                self.finish();
                return;
            }
        }
        self.run_t += dt;
        if self.ending == 0.0 {
            if input.down {
                let d = input.pos - self.centre(view);
                if d.length() > 12.0 {
                    self.steer = d.y.atan2(d.x);
                }
            } else {
                let turn = sim::turn_for(self.world.player().level()) * dt * 0.8;
                if input.left {
                    self.steer = sim::wrap_angle(self.steer - turn);
                }
                if input.right {
                    self.steer = sim::wrap_angle(self.steer + turn);
                }
            }
        }
        let steer = if self.world.player_alive() {
            Some(self.steer)
        } else {
            None
        };
        let dt_w = if self.ending > 0.0 && !self.world.player_alive() {
            dt * 0.4
        } else {
            dt
        };
        self.world.step(dt_w, steer);
        let events = std::mem::take(&mut self.world.events);
        for e in events {
            self.react(e);
        }
        if self.ending == 0.0 && self.mode == Mode::Tower && self.floor.done(&self.world) {
            self.ending = 0.001;
            self.celebrate();
        }
    }

    fn celebrate(&mut self) {
        let head = self.world.player().head();
        sfx::play(Sfx::Win);
        sfx::say(sfx::Say::Win);
        feel::impact(Impact::Large);
        self.flash = 0.8;
        self.fx.emit(
            head,
            &[GOLD, WHITE, PINK, Color::from_rgba(120, 220, 255, 255)],
            Burst {
                count: 60,
                speed: (200.0, 700.0),
                size: (6.0, 12.0),
                life: (0.6, 1.2),
                shape: Shape::Confetti,
                ..Default::default()
            },
        );
        self.popups.add(
            head + vec2(0.0, -90.0),
            "CLEAR!",
            Color::from_rgba(255, 230, 90, 255),
            72.0,
        );
    }

    fn skin_color(&self, skin: usize) -> Color {
        SKINS[skin.min(SKINS.len() - 1)].a
    }

    fn react(&mut self, e: Event) {
        let my_id = self.world.player().id;
        match e {
            Event::Egg { by, at, value } => {
                if by != my_id {
                    return;
                }
                self.combo = (self.combo + 1).min(11);
                self.combo_t = COMBO_WINDOW;
                sfx::play_step(Sfx::Pop, (self.combo as usize).saturating_sub(1) / 2);
                let col = if value > 1 {
                    GOLD
                } else {
                    Color::from_rgba(255, 235, 140, 255)
                };
                self.fx.emit(
                    at,
                    &[col, WHITE],
                    Burst {
                        count: if value > 1 { 8 } else { 4 },
                        speed: (50.0, 160.0),
                        size: (2.5, 5.0),
                        life: (0.25, 0.5),
                        gravity: 0.0,
                        drag: 3.5,
                        shape: Shape::Star,
                        ..Default::default()
                    },
                );
                self.bump.kick();
            }
            Event::Level { id, level, at } => {
                if id != my_id {
                    return;
                }
                sfx::play(Sfx::LevelUp);
                self.rings.push((at, 0.0, GOLD));
                self.popups.add(
                    at + vec2(0.0, -70.0),
                    format!("LEVEL {level}"),
                    Color::from_rgba(255, 230, 90, 255),
                    58.0,
                );
                self.fx.sparkle(at, GOLD);
                feel::impact(Impact::Small);
            }
            Event::Power {
                kind, at, player, ..
            } => {
                let col = powers::color(kind);
                self.fx.emit(
                    at,
                    &[col, WHITE],
                    Burst {
                        count: if player { 18 } else { 6 },
                        speed: (90.0, 260.0),
                        size: (4.0, 8.0),
                        life: (0.3, 0.6),
                        gravity: 0.0,
                        drag: 3.5,
                        ..Default::default()
                    },
                );
                if player {
                    sfx::play(Sfx::Magic);
                    feel::impact(Impact::Small);
                    self.bump.kick();
                    self.popups.add(at, powers::label(kind), col, 46.0);
                }
            }
            Event::Bump { at, player } => {
                self.fx.emit(
                    at,
                    &[WHITE, Color::from_rgba(255, 245, 200, 255)],
                    Burst {
                        count: 6,
                        speed: (80.0, 200.0),
                        size: (3.0, 6.0),
                        life: (0.2, 0.4),
                        gravity: 0.0,
                        drag: 4.0,
                        ..Default::default()
                    },
                );
                if player {
                    sfx::play(Sfx::Bounce);
                    self.shake.add(0.25);
                    feel::impact(Impact::Small);
                }
            }
            Event::Eaten {
                at,
                level,
                skin,
                player,
                by_player,
                boss,
                ..
            } => {
                let col = self.skin_color(skin);
                let big =
                    by_player && (level >= self.world.player().level().saturating_sub(1) || boss);
                self.fx.emit(
                    at,
                    &[col, WHITE, col.lerp(WHITE, 0.5)],
                    Burst {
                        count: if boss { 60 } else { 26 },
                        speed: (120.0, 420.0),
                        size: (4.0, 9.0),
                        life: (0.4, 0.8),
                        gravity: 0.0,
                        drag: 2.6,
                        ..Default::default()
                    },
                );
                if player {
                    sfx::play(Sfx::Boom);
                    sfx::play(Sfx::Lose);
                    sfx::say(sfx::Say::GameOver);
                    feel::impact(Impact::Large);
                    self.shake.add(0.9);
                    self.flash = 1.0;
                    self.ending = 0.001;
                    return;
                }
                sfx::play(Sfx::Crunch);
                if by_player {
                    self.popups.add(
                        at + vec2(0.0, -40.0),
                        format!("+{}", level * 2 + 2),
                        Color::from_rgba(255, 150, 190, 255),
                        46.0,
                    );
                    self.shake.add(if boss {
                        1.0
                    } else if big {
                        0.5
                    } else {
                        0.28
                    });
                    feel::impact(if boss || big {
                        Impact::Large
                    } else {
                        Impact::Medium
                    });
                    if boss {
                        sfx::play(Sfx::Boom);
                    }
                }
            }
        }
    }

    fn finish(&mut self) {
        let before = self.stats;
        let mut s = read_stats();
        let level = self.world.player().level();
        let cleared = self.world.player_alive() && self.mode == Mode::Tower;
        s.games += 1;
        s.eaten += self.world.player_eats;
        s.best_level = s.best_level.max(level);
        if cleared {
            s.floor = s.floor.max(self.floor.n);
            crate::profile::unlock(GAME, self.floor.n + 1);
        }
        write_stats(&s);
        let was_best = self.best.0;
        self.rank = self.best.submit(GAME, level);
        let help = if level < 3 && self.mode == Mode::Arena {
            (self.help + 1).min(3)
        } else if level >= 6 {
            0
        } else {
            self.help.saturating_sub(1)
        };
        self.help = help;
        crate::profile::set(GAME, "struggle", help as i64);
        self.stats = s;
        self.outcome = Outcome {
            level,
            eaten: self.world.player_eats,
            cleared,
            new_skins: skins::newly_unlocked(&before, &s),
            new_best: self.mode == Mode::Arena && level > was_best && was_best > 0,
            floor: self.floor.n,
        };
        if self.outcome.new_best {
            sfx::say(sfx::Say::Best);
        }
        if !self.outcome.new_skins.is_empty() {
            sfx::play(Sfx::Magic);
        }
        self.screen = Screen::Over;
        self.ending = 0.0;
        self.press = None;
        crate::net::request_board(GAME);
    }

    fn update_over(&mut self, input: &Input, view: &View, dt: f32) {
        self.drift(dt);
        self.ending += dt;
        if self.ending < RESULTS_DELAY {
            return;
        }
        let (again, menu) = over_buttons(view);
        if input.tapped_in(menu) || input.back {
            self.press = Some(Press::Menu);
        } else if input.tapped_in(again) || input.action {
            self.press = Some(Press::Again);
        }
        match self.press {
            Some(Press::Again) => {
                sfx::play(Sfx::Click);
                if self.mode == Mode::Tower {
                    let n = if self.outcome.cleared {
                        self.floor.n + 1
                    } else {
                        self.floor.n
                    };
                    self.pick_floor = n;
                    self.start(Mode::Tower, n);
                } else {
                    self.start(Mode::Arena, 1);
                }
            }
            Some(Press::Menu) => {
                sfx::play(Sfx::Click);
                self.show_title();
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fresh() -> Clash {
        crate::save::reset_for_test();
        crate::profile::create("Mia", 0);
        crate::rng::srand(4);
        Clash::new()
    }

    fn run(c: &mut Clash, secs: f32) {
        let v = View::ipad_landscape();
        for _ in 0..(secs * 60.0) as usize {
            c.update(&Input::default(), &v, 1.0 / 60.0);
        }
    }

    #[test]
    fn the_title_has_a_live_arena_behind_it() {
        let mut c = fresh();
        assert_eq!(c.screen, Screen::Title);
        let before = c.world.player().head();
        run(&mut c, 2.0);
        assert!((c.world.player().head() - before).length() > 50.0);
    }

    #[test]
    fn tapping_arena_starts_a_small_snake_and_dying_shows_results() {
        let mut c = fresh();
        let v = View::ipad_landscape();
        let l = title_layout(&v);
        c.update(&Input::press_at(l.arena.center()), &v, 1.0 / 60.0);
        assert_eq!(c.screen, Screen::Playing);
        assert_eq!(c.world.player().level(), 1);
        c.world.snakes[0].shield = 0.0;
        let head = c.world.player().head();
        let i = c.world.add_snake(head + vec2(5.0, 0.0), 9, "Big".into(), 2);
        c.world.snakes[i].xp = sim::xp_for_level(9);
        run(&mut c, 3.0);
        assert_eq!(c.screen, Screen::Over);
        assert!(c.shows_results());
        assert_eq!(read_stats().games, 1);
    }

    #[test]
    fn steering_follows_the_finger_relative_to_the_snake() {
        let mut c = fresh();
        let v = View::ipad_landscape();
        c.start(Mode::Arena, 1);
        let target = c.centre(&v) + vec2(0.0, 200.0);
        for _ in 0..60 {
            let mut i = Input::press_at(target);
            i.pressed = false;
            c.update(&i, &v, 1.0 / 60.0);
        }
        let a = c.world.player().angle;
        assert!((a - std::f32::consts::FRAC_PI_2).abs() < 0.3, "{a}");
    }

    #[test]
    fn clearing_a_tower_floor_opens_the_next_and_unlocks_skins() {
        let mut c = fresh();
        c.start(Mode::Tower, 1);
        c.world.snakes[0].xp = sim::xp_for_level(20);
        run(&mut c, 3.0);
        assert_eq!(c.screen, Screen::Over);
        assert!(c.outcome.cleared);
        assert_eq!(read_stats().floor, 1);
        assert!(c.outcome.new_skins.contains(&4));
        assert!(c.skin_open(4));
    }

    #[test]
    fn the_tower_picker_cannot_go_past_the_next_floor() {
        let mut c = fresh();
        let v = View::ipad_landscape();
        c.screen = Screen::Tower;
        let l = tower_layout(&v);
        for _ in 0..5 {
            c.update(&Input::press_at(l.next.center()), &v, 1.0 / 60.0);
        }
        assert_eq!(c.pick_floor, 1);
        crate::profile::set(GAME, "floor", 3);
        c.stats = read_stats();
        for _ in 0..8 {
            c.update(&Input::press_at(l.next.center()), &v, 1.0 / 60.0);
        }
        assert_eq!(c.pick_floor, 4);
    }

    #[test]
    fn a_new_best_level_is_recorded_on_the_board() {
        let mut c = fresh();
        c.start(Mode::Arena, 1);
        c.world.snakes[0].xp = sim::xp_for_level(7);
        let i = c
            .world
            .add_snake(c.world.player().head(), 30, "Big".into(), 1);
        c.world.snakes[i].xp = sim::xp_for_level(30);
        c.world.snakes[0].shield = 0.0;
        run(&mut c, 3.0);
        assert_eq!(c.best.0, 7);
        assert_eq!(c.rank, Some(0));
    }

    #[test]
    fn a_locked_skin_can_be_previewed_but_not_kept() {
        let mut c = fresh();
        c.cycle_skin(1);
        c.cycle_skin(1);
        c.cycle_skin(1);
        c.cycle_skin(1);
        assert!(!c.skin_open(c.skin));
        assert_eq!(crate::profile::get(GAME, "skin"), 3);
        let c2 = Clash::new();
        assert!(c2.skin_open(c2.skin));
    }
}
