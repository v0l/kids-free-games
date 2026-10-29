use super::sight;
use super::sim::{level_of, wrap_angle, Event, Spec, World};
use crate::brain::conv::{Obs, Policy};
use crate::brain::state::{look, BrainState, Layer, Lens, Mark, Viewer};
use crate::brain::Effort;
use crate::rng as rand;
use glam::vec2;
use std::f32::consts::PI;

pub const VIEW: usize = 32;
pub const CHANNELS: usize = 2 + 15 + 5;
pub const SCALARS: usize = 19;
pub const ACTIONS: usize = 5;
pub const TURNS: [f32; ACTIONS] = [-1.2, -0.5, 0.0, 0.5, 1.2];
pub const DECIDE: f32 = 0.1;
pub const FRAME: f32 = 1.0 / 30.0;
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rewards {
    pub xp: f32,
    pub level: f32,
    pub kill: f32,
    pub cut: f32,
    pub death: f32,
    pub stall: f32,
    pub wall: f32,
    pub bump: f32,
    pub win: f32,
}

impl Default for Rewards {
    fn default() -> Self {
        Self {
            xp: 0.02,
            level: 1.0,
            kill: 6.0,
            cut: 1.0,
            death: 3.0,
            stall: 0.002,
            wall: 0.3,
            bump: 0.3,
            win: 10.0,
        }
    }
}

static SHIPPED: &[u8] = include_bytes!("../../../../../assets/brains/clash_rl.kgb");

pub fn shipped_arc() -> Option<std::sync::Arc<Policy>> {
    static NET: std::sync::OnceLock<Option<std::sync::Arc<Policy>>> = std::sync::OnceLock::new();
    NET.get_or_init(|| shipped().cloned().map(std::sync::Arc::new))
        .clone()
}

pub fn shipped() -> Option<&'static Policy> {
    static NET: std::sync::OnceLock<Option<Policy>> = std::sync::OnceLock::new();
    NET.get_or_init(|| Policy::from_bytes(SHIPPED).filter(fits))
        .as_ref()
}

pub fn fits(p: &Policy) -> bool {
    p.grid == VIEW && p.channels == CHANNELS && p.scalars == SCALARS && p.actions == ACTIONS
}

pub fn grid_len() -> usize {
    CHANNELS * VIEW * VIEW
}

pub mod layer {
    pub const EGGS: usize = 0;
    pub const BIGGER: usize = 1;
    pub const BIGGER_HEAD: usize = 2;
    pub const SMALLER: usize = 3;
    pub const SMALLER_HEAD: usize = 4;
    pub const EVEN: usize = 5;
    pub const MINE: usize = 6;
    pub const POWERS: usize = 7;
    pub const BOOSTED: usize = 11;
    pub const LEVEL_GAP: usize = 12;
    pub const BOSS: usize = 13;
    pub const DIZZY: usize = 14;
    pub const ARROWS: usize = 15;
}

static LAYERS: [Layer; 16] = [
    Layer::add("eggs"),
    Layer::max("bigger").mapped(),
    Layer::max("bigger head"),
    Layer::max("smaller").mapped(),
    Layer::max("smaller head"),
    Layer::max("even or shielded").mapped(),
    Layer::max("own body"),
    Layer::max("speed"),
    Layer::max("magnet"),
    Layer::max("shield"),
    Layer::max("grow"),
    Layer::max("speed boosted"),
    Layer::set("level gap"),
    Layer::max("boss").mapped(),
    Layer::max("dizzy"),
    Layer::set("arrow level gap").map_only(),
];

static SCALAR_NAMES: [&str; SCALARS] = [
    "level",
    "level progress",
    "shield left",
    "speed left",
    "magnet left",
    "dizzy",
    "centre bearing",
    "centre distance",
    "heading cos",
    "heading sin",
    "share bigger",
    "share smaller",
    "top level gap",
    "leading",
    "crowd",
    "board rank",
    "board last",
    "xp behind next on board",
    "xp ahead of next below",
];

fn level_gap(of: u32, me: u32) -> f32 {
    ((of as f32 - me as f32) / 10.0).clamp(-1.0, 1.0)
}

impl BrainState for World {
    fn layers() -> &'static [Layer] {
        &LAYERS
    }

    fn scalar_names() -> &'static [&'static str] {
        &SCALAR_NAMES
    }

    fn lens(&self) -> Lens {
        Lens {
            side: VIEW,
            world: vec2(self.size, self.size),
        }
    }

    fn viewer(&self, i: usize) -> Viewer {
        let me = &self.snakes[i];
        Viewer {
            pos: me.head(),
            angle: me.angle,
            span: sight::span_of(self, i),
        }
    }

    fn marks(&self, i: usize, out: &mut Vec<Mark>) {
        use layer::*;
        let lvl = self.snakes[i].level();
        for e in &self.eggs {
            out.push(Mark::near(EGGS, e.pos, e.value as f32 * 0.25));
        }
        for (j, o) in self.snakes.iter().enumerate() {
            if j == i || !o.alive {
                continue;
            }
            let ol = o.level();
            let open = o.shield <= 0.0;
            let (body, head) = if ol > lvl && open {
                (BIGGER, BIGGER_HEAD)
            } else if ol < lvl && open {
                (SMALLER, SMALLER_HEAD)
            } else {
                (EVEN, EVEN)
            };
            let signed = level_gap(ol, lvl);
            let gap = (signed.abs() + 0.2).min(1.0);
            for (k, p) in o.segs.iter().enumerate().step_by(2) {
                out.push(Mark::near(if k == 0 { head } else { body }, *p, gap));
                out.push(Mark::near(LEVEL_GAP, *p, signed));
                if o.boss {
                    out.push(Mark::near(BOSS, *p, 1.0));
                }
                if o.stun > 0.0 {
                    out.push(Mark::near(DIZZY, *p, 1.0));
                }
                if o.boosts.speed > 0.0 {
                    out.push(Mark::near(BOOSTED, *p, 1.0));
                }
            }
            let ahead = o.head() + o.dir() * o.speed() * 0.6;
            out.push(Mark::near(head, ahead, 0.5));
        }
        for (pos, dot) in sight::minimap(self, i) {
            let layer = match dot {
                sight::Dot::Bigger => BIGGER,
                sight::Dot::Smaller => SMALLER,
                sight::Dot::Even => EVEN,
                sight::Dot::Boss => BOSS,
            };
            out.push(Mark::map(layer, pos, 1.0));
        }
        for a in sight::arrows(self, i) {
            out.push(Mark::map(ARROWS, a.at, level_gap(a.level, lvl)));
        }
        for p in &self.powers {
            out.push(Mark::near(POWERS + p.kind.index(), p.pos, 1.0));
        }
        for p in self.snakes[i].segs.iter().step_by(2).skip(1) {
            out.push(Mark::near(MINE, *p, 1.0));
        }
    }

    fn scalars(&self, i: usize, out: &mut Vec<f32>) {
        let me = &self.snakes[i];
        let lvl = me.level();
        let others: Vec<u32> = self
            .snakes
            .iter()
            .enumerate()
            .filter(|(j, o)| *j != i && o.alive)
            .map(|(_, o)| o.level())
            .collect();
        let crowd = others.len().max(1) as f32;
        let bigger = others.iter().filter(|l| **l > lvl).count() as f32;
        let smaller = others.iter().filter(|l| **l < lvl).count() as f32;
        let top = others.iter().copied().max().unwrap_or(lvl);
        let centre = vec2(self.size, self.size) * 0.5 - me.head();
        out.extend([
            (lvl as f32 / 20.0).min(2.0),
            super::sim::level_progress(me.xp),
            if me.shield > 0.0 {
                (me.shield / 5.0).min(1.0)
            } else {
                0.0
            },
            (me.boosts.speed / super::powers::SPEED_SECS).min(1.0),
            (me.boosts.magnet / super::powers::MAGNET_SECS).min(1.0),
            if me.stun > 0.0 { 1.0 } else { 0.0 },
            wrap_angle(centre.y.atan2(centre.x) - me.angle) / PI,
            (centre.length() / (self.size * 0.5)).min(1.5),
            me.angle.cos(),
            me.angle.sin(),
            bigger / crowd,
            smaller / crowd,
            ((top as f32 - lvl as f32) / 10.0).clamp(-1.0, 1.0),
            if bigger == 0.0 { 1.0 } else { 0.0 },
            (crowd / 16.0).min(1.0),
        ]);
        let board = sight::board(self, i);
        out.extend([
            board.rank as f32 / board.size.max(1) as f32,
            if board.rank + 1 >= board.size {
                1.0
            } else {
                0.0
            },
            (board.behind as f32 / 50.0).min(1.0),
            (board.ahead as f32 / 50.0).min(1.0),
        ]);
    }
}

pub fn observe(w: &World, i: usize) -> Obs {
    look(w, i)
}

pub fn training_spec(seed: u64) -> (Spec, u32) {
    let band = (seed % 5) as u32;
    let start = 1 + band * 4;
    let mut spec = Spec::survival(start, band == 0 && seed.is_multiple_of(10));
    spec.start = start;
    (spec, start)
}

type Rivals = (Vec<std::sync::Arc<Policy>>, f32);

static RIVALS: std::sync::Mutex<Rivals> = std::sync::Mutex::new((Vec::new(), 0.0));
static SEATS: std::sync::Mutex<(usize, f32)> = std::sync::Mutex::new((0, 0.0));

pub fn set_rivals(pool: Vec<Policy>, share: f32) {
    *RIVALS.lock().unwrap() = (pool.into_iter().map(std::sync::Arc::new).collect(), share);
}

pub fn set_seats(policies: usize, share: f32) {
    *SEATS.lock().unwrap() = (policies, share);
}

fn rival_spec(seed: u64) -> Spec {
    let (mut spec, _) = training_spec(seed);
    let (pool, share) = RIVALS.lock().unwrap().clone();
    if !pool.is_empty() {
        spec.rival = Some(pool[(seed / 5) as usize % pool.len()].clone());
        spec.rival_share = share;
    }
    let (seats, share) = *SEATS.lock().unwrap();
    if seats > 0 {
        spec.seat = Some((seed / 5) as usize % seats);
        spec.rival_share = share;
    }
    spec
}

pub struct Arena {
    pub w: World,
    pub obs: Obs,
    xp: u32,
    since_growth: f32,
    pub ret: f32,
    pub finished: Vec<f32>,
    pub levels: Vec<u32>,
    pub steps: u32,
    max_steps: u32,
    seed: u64,
    pub rewards: Rewards,
    seats: Vec<crate::brain::learn::Seat>,
    seat_of: Vec<usize>,
}

pub struct Step {
    pub reward: f32,
    pub done: bool,
}

impl Arena {
    pub fn new(seed: u64, secs: f32) -> Self {
        Self::with(seed, secs, Rewards::default())
    }

    pub fn with(seed: u64, secs: f32, rewards: Rewards) -> Self {
        rand::srand(seed);
        let spec = rival_spec(seed);
        let mut w = World::new(spec, "Learner", 0);
        w.snakes[0].shield = 1.0;
        let obs = observe(&w, 0);
        let xp = w.snakes[0].xp;
        Self {
            w,
            obs,
            xp,
            since_growth: 0.0,
            ret: 0.0,
            finished: vec![],
            levels: vec![],
            steps: 0,
            max_steps: (secs / DECIDE) as u32,
            seed,
            rewards,
            seats: vec![],
            seat_of: vec![],
        }
        .seated()
    }

    fn seated(mut self) -> Self {
        self.refresh_seats();
        self
    }

    fn refresh_seats(&mut self) {
        self.seats.clear();
        self.seat_of.clear();
        for (i, s) in self.w.snakes.iter().enumerate() {
            if let (true, Some(policy)) = (s.alive, s.seat) {
                self.seats.push(crate::brain::learn::Seat {
                    policy,
                    obs: observe(&self.w, i),
                });
                self.seat_of.push(i);
            }
        }
    }

    pub fn seat_actions(&mut self, actions: &[usize]) {
        for (&i, &a) in self.seat_of.iter().zip(actions) {
            if let Some(s) = self.w.snakes.get_mut(i) {
                if s.alive && s.seat.is_some() {
                    s.want = wrap_angle(s.angle + TURNS[a.min(ACTIONS - 1)]);
                }
            }
        }
    }

    pub fn step(&mut self, action: usize) -> Step {
        let me = &self.w.snakes[0];
        let want = wrap_angle(me.angle + TURNS[action.min(ACTIONS - 1)]);
        let mut reward = 0.0;
        let mut dead = false;
        for _ in 0..(DECIDE / FRAME).round() as usize {
            self.w.step(FRAME, Some(want));
            for e in std::mem::take(&mut self.w.events) {
                match e {
                    Event::Eaten {
                        by_player, player, ..
                    } => {
                        dead |= player;
                        if by_player {
                            reward += self.rewards.kill;
                        }
                    }
                    Event::Cut {
                        by_player: true, ..
                    } => reward += self.rewards.cut,
                    Event::Bump { player: true, at } => {
                        reward -= if self.w.near_wall(at) {
                            self.rewards.wall
                        } else {
                            self.rewards.bump
                        }
                    }
                    _ => {}
                }
            }
            if dead {
                break;
            }
        }
        let xp = self.w.snakes[0].xp;
        reward += (xp as f32 - self.xp as f32) * self.rewards.xp;
        let (now, was) = (level_of(xp) as f32, level_of(self.xp) as f32);
        reward += (now - was) * self.rewards.level;
        if xp > self.xp {
            self.since_growth = 0.0;
        } else {
            self.since_growth += DECIDE;
            reward -= self.rewards.stall * self.since_growth.min(10.0);
        }
        self.xp = xp;
        if dead {
            reward -= self.rewards.death;
        }
        let won = self.w.won();
        if won {
            reward += self.rewards.win;
        }
        self.ret += reward;
        self.steps += 1;
        let done = dead || won || self.steps >= self.max_steps;
        if done {
            self.finished.push(self.ret);
            self.levels.push(self.w.snakes[0].level());
            let finished = std::mem::take(&mut self.finished);
            let levels = std::mem::take(&mut self.levels);
            let secs = self.max_steps as f32 * DECIDE;
            self.seed = self
                .seed
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            *self = Self::with(self.seed >> 1, secs, self.rewards);
            self.finished = finished;
            self.levels = levels;
        } else {
            self.obs = observe(&self.w, 0);
            self.refresh_seats();
        }
        Step { reward, done }
    }
}

pub const GAME_CAP: f32 = 600.0;

pub struct Report {
    pub level: f32,
    pub secs: f32,
    pub wins: f32,
    pub deaths: f32,
    pub kills: f32,
    pub stall: f32,
    pub circling: f32,
}

pub enum Pilot<'a> {
    Policy(&'a Policy, Effort),
    Bot,
}

pub fn evaluate(pilot: &Pilot, seeds: u64, secs: f32) -> Report {
    evaluate_against(pilot, seeds, secs, &[], 0.0)
}

pub fn evaluate_against(
    pilot: &Pilot,
    seeds: u64,
    secs: f32,
    rivals: &[Policy],
    share: f32,
) -> Report {
    let pool: Vec<_> = rivals.iter().cloned().map(std::sync::Arc::new).collect();
    let runs: Vec<[f32; 7]> = std::thread::scope(|s| {
        let hs: Vec<_> = (0..seeds)
            .map(|seed| {
                let rival = (!pool.is_empty()).then(|| pool[seed as usize % pool.len()].clone());
                s.spawn(move || play_one(pilot, 77_000 + seed, secs, rival, share))
            })
            .collect();
        hs.into_iter().map(|h| h.join().unwrap()).collect()
    });
    let n = seeds.max(1) as f32;
    let avg = |k: usize| runs.iter().map(|r| r[k]).sum::<f32>() / n;
    Report {
        level: avg(0),
        deaths: avg(1),
        kills: avg(2),
        stall: avg(3),
        circling: avg(4),
        secs: avg(5),
        wins: avg(6),
    }
}

fn play_one(
    pilot: &Pilot,
    seed: u64,
    secs: f32,
    rival: Option<std::sync::Arc<Policy>>,
    share: f32,
) -> [f32; 7] {
    rand::srand(seed);
    let spec_for = |seed: u64| {
        let (mut spec, _) = training_spec(seed * 5);
        if let Some(r) = &rival {
            spec.rival = Some(r.clone());
            spec.rival_share = share;
        }
        spec
    };
    let mut w = World::new(spec_for(seed), "Me", 0);
    let (mut deaths, mut kills, mut peak) = (0.0f32, 0.0f32, 0u32);
    let mut wins = 0.0f32;
    let (mut since, mut stall, mut steps) = (0.0f32, 0.0f32, 0.0f32);
    let (mut turned, mut window, mut loops, mut windows) = (0.0f32, 0usize, 0.0f32, 0.0f32);
    let mut xp = w.snakes[0].xp;
    let mut last = w.snakes[0].angle;
    let mut want = last;
    for _ in 0..(secs / DECIDE) as usize {
        want = match pilot {
            Pilot::Policy(p, e) => {
                let a = p.choose(&observe(&w, 0), *e);
                wrap_angle(w.snakes[0].angle + TURNS[a])
            }
            Pilot::Bot => {
                let _ = want;
                w.autopilot()
            }
        };
        for _ in 0..(DECIDE / FRAME).round() as usize {
            w.step(FRAME, Some(want));
            for e in std::mem::take(&mut w.events) {
                if let Event::Eaten {
                    by_player, player, ..
                } = e
                {
                    if player {
                        deaths += 1.0;
                    }
                    if by_player {
                        kills += 1.0;
                    }
                }
            }
        }
        if !w.player_alive() || w.won() {
            if w.won() {
                wins += 1.0;
            }
            break;
        }
        peak = peak.max(w.snakes[0].level());
        steps += 1.0;
        if w.snakes[0].xp > xp {
            since = 0.0;
        } else {
            since += DECIDE;
            if since > 5.0 {
                stall += 1.0;
            }
        }
        xp = w.snakes[0].xp;
        let a = w.snakes[0].angle;
        turned += wrap_angle(a - last);
        last = a;
        window += 1;
        if window as f32 * DECIDE >= 3.0 {
            windows += 1.0;
            if turned.abs() >= std::f32::consts::TAU {
                loops += 1.0;
            }
            turned = 0.0;
            window = 0;
        }
    }
    [
        peak as f32,
        deaths,
        kills,
        stall / steps.max(1.0),
        loops / windows.max(1.0),
        steps * DECIDE,
        wins,
    ]
}

impl crate::brain::learn::Learner for Arena {
    fn spec() -> crate::brain::learn::Spec {
        crate::brain::state::spec::<World>(VIEW, ACTIONS)
    }

    fn agents(&self) -> usize {
        1
    }

    fn obs(&self, _: usize) -> &Obs {
        &self.obs
    }

    fn step(&mut self, actions: &[usize], out: &mut [crate::brain::learn::Tick]) {
        let s = Arena::step(self, actions[0]);
        out[0] = crate::brain::learn::Tick {
            reward: s.reward,
            done: s.done,
        };
    }

    fn drain_episodes(&mut self) -> Vec<(f32, f32)> {
        let r = std::mem::take(&mut self.finished);
        let l = std::mem::take(&mut self.levels);
        r.into_iter().zip(l).map(|(a, b)| (a, b as f32)).collect()
    }

    fn seats(&self) -> &[crate::brain::learn::Seat] {
        &self.seats
    }

    fn seat_actions(&mut self, actions: &[usize]) {
        Arena::seat_actions(self, actions);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::brain::state::{channel_names, map_channel, near_channel};
    use glam::Vec2;

    fn lone_world() -> World {
        rand::srand(11);
        let (mut spec, _) = training_spec(0);
        spec.bots = 0;
        spec.powerups = false;
        let mut w = World::new(spec, "Me", 0);
        w.snakes.truncate(1);
        w.eggs.clear();
        w.powers.clear();
        let mid = vec2(w.size, w.size) * 0.5;
        w.snakes[0].segs = vec![mid; w.snakes[0].segs.len()];
        w.snakes[0].angle = 0.0;
        w
    }

    fn channel(o: &Obs, c: usize) -> &[f32] {
        &o.grid[c * VIEW * VIEW..(c + 1) * VIEW * VIEW]
    }

    #[test]
    fn the_layout_matches_the_policy_shape() {
        let a = Arena::new(3, 60.0);
        assert_eq!(a.obs.grid.len(), grid_len());
        assert_eq!(a.obs.scalars.len(), SCALARS);
        assert_eq!(channel_names::<World>().len(), CHANNELS);
        let eggs: f32 = channel(&a.obs, near_channel(&LAYERS, layer::EGGS).unwrap())
            .iter()
            .sum();
        assert!(eggs > 1.0, "eggs in view: {eggs}");
    }

    #[test]
    fn a_far_bigger_snake_shows_on_the_minimap_with_no_near_view() {
        let mut w = lone_world();
        let far = w.snakes[0].head() + vec2(sight::reach(&w, 0) + 200.0, 0.0);
        let j = w.add_snake(far, w.snakes[0].level() + 6, "Big".into(), 0);
        w.snakes[j].shield = 0.0;
        let o = observe(&w, 0);
        let near: f32 = channel(&o, near_channel(&LAYERS, layer::BIGGER).unwrap())
            .iter()
            .sum();
        let map: f32 = channel(&o, map_channel(&LAYERS, layer::BIGGER).unwrap())
            .iter()
            .sum();
        assert_eq!(near, 0.0);
        assert!(map > 0.0);
        assert_eq!(o.scalars[13], 0.0, "not leading with a bigger snake alive");
        let arrows = channel(&o, map_channel(&LAYERS, layer::ARROWS).unwrap());
        assert!(
            arrows.iter().any(|v| (*v - 0.6).abs() < 1e-5),
            "arrow carries the +6 level gap"
        );
    }

    #[test]
    fn eggs_and_powerups_off_screen_are_invisible() {
        let mut w = lone_world();
        assert!(w.eggs.is_empty() && w.powers.is_empty());
        let head = w.snakes[0].head();
        let off = head + vec2(sight::reach(&w, 0) + 50.0, 0.0);
        w.eggs.push(super::super::sim::Egg {
            pos: off,
            vel: Vec2::ZERO,
            value: 4,
            kind: 0,
            age: 0.0,
        });
        w.powers.push(super::super::powers::Power {
            pos: off,
            kind: super::super::powers::Kind::Grow,
            age: 0.0,
        });
        assert_eq!(observe(&w, 0).grid, observe(&lone_world(), 0).grid);
    }

    #[test]
    fn a_near_smaller_snake_carries_its_level_gap() {
        let mut w = lone_world();
        let lvl = w.snakes[0].level().max(5);
        w.snakes[0].xp = super::super::sim::xp_for_level(lvl);
        let near = w.snakes[0].head() + vec2(120.0, 0.0);
        let j = w.add_snake(near, lvl - 3, "Small".into(), 0);
        w.snakes[j].shield = 0.0;
        let o = observe(&w, 0);
        let gaps = channel(&o, near_channel(&LAYERS, layer::LEVEL_GAP).unwrap());
        assert!(
            gaps.iter().any(|v| (*v + 0.3).abs() < 1e-5),
            "level gap -3 shown as -0.3"
        );
        assert!(channel(&o, near_channel(&LAYERS, layer::SMALLER).unwrap())
            .iter()
            .any(|v| *v > 0.0));
        assert_eq!(o.scalars[13], 1.0);
    }

    #[test]
    fn every_powerup_kind_has_its_own_layer() {
        let mut w = lone_world();
        let head = w.snakes[0].head();
        for (k, kind) in super::super::powers::Kind::ALL.iter().enumerate() {
            w.powers.push(super::super::powers::Power {
                pos: head + vec2(100.0, -90.0 + 60.0 * k as f32),
                kind: *kind,
                age: 0.0,
            });
        }
        let o = observe(&w, 0);
        for k in 0..4 {
            let sum: f32 = channel(&o, near_channel(&LAYERS, layer::POWERS + k).unwrap())
                .iter()
                .sum();
            assert_eq!(sum, 1.0, "power kind {k}");
        }
    }

    #[test]
    fn seated_rivals_are_steered_by_the_actions_handed_in() {
        use crate::brain::learn::Learner;
        set_seats(2, 1.0);
        let mut a = Arena::new(15, 600.0);
        set_seats(0, 0.0);
        let seats = Learner::seats(&a);
        assert!(!seats.is_empty(), "some bots sit in a seat");
        assert!(seats
            .iter()
            .all(|s| s.policy < 2 && s.obs.grid.len() == grid_len()));
        let i = a.seat_of[0];
        let before = a.w.snakes[i].angle;
        let left = vec![0; seats.len()];
        Learner::seat_actions(&mut a, &left);
        assert!((wrap_angle(a.w.snakes[i].want - before) - TURNS[0]).abs() < 1e-4);
        let mut ticks = vec![crate::brain::learn::Tick {
            reward: 0.0,
            done: false,
        }];
        Learner::step(&mut a, &[2], &mut ticks);
        assert!(
            wrap_angle(a.w.snakes[i].angle - before) < -0.05,
            "seat turned left"
        );
    }

    #[test]
    fn rival_pool_spreads_policies_across_arenas() {
        let tiny = |tag: usize| Policy {
            grid: VIEW,
            channels: CHANNELS,
            scalars: SCALARS,
            actions: tag,
            trunk: vec![],
            head: vec![],
        };
        let (a, b) = (tiny(1), tiny(2));
        set_rivals(vec![a.clone(), b.clone()], 1.0);
        let picked: Vec<bool> = (0..10u64)
            .map(|s| rival_spec(s * 5).rival.unwrap().actions == 1)
            .collect();
        set_rivals(vec![], 0.0);
        assert!(picked.contains(&true) && picked.contains(&false));
        assert!(rival_spec(0).rival.is_none());
    }

    #[test]
    fn bouncing_off_another_snake_costs_the_learner() {
        let mut a = Arena::new(5, 600.0);
        a.w.snakes.truncate(1);
        a.w.eggs.clear();
        a.w.powers.clear();
        a.w.snakes[0].shield = 0.0;
        a.w.snakes[0].segs[0] = vec2(1500.0, 1500.0);
        a.w.snakes[0].angle = 0.0;
        a.w.layout_tail(0);
        let lvl = a.w.snakes[0].level();
        let j = a.w.add_snake(vec2(1520.0, 1500.0), lvl, "Twin".into(), 0);
        a.w.snakes[j].xp = a.w.snakes[0].xp;
        a.w.snakes[j].angle = std::f32::consts::PI;
        a.w.layout_tail(j);
        a.xp = a.w.snakes[0].xp;
        let r = a.step(2).reward;
        assert!(r <= -Rewards::default().bump + 0.05, "reward {r}");
    }

    #[test]
    fn outliving_every_other_snake_ends_the_game_with_a_win_bonus() {
        let mut a = Arena::new(5, 600.0);
        assert!(!a.w.spec.respawn, "training plays survival rules");
        a.w.snakes[0].shield = 1e9;
        for i in 1..a.w.snakes.len() {
            a.w.kill(i, None);
        }
        a.w.events.clear();
        let s = a.step(2);
        assert!(s.done);
        assert!(
            s.reward >= Rewards::default().win - 1.0,
            "reward {}",
            s.reward
        );
    }

    #[test]
    fn levelling_up_pays_more_than_the_xp_alone() {
        let mut a = Arena::new(5, 600.0);
        let lvl = a.w.snakes[0].level();
        let need = super::super::sim::xp_for_level(lvl + 1) - a.w.snakes[0].xp;
        a.w.snakes[0].xp += need;
        let r = a.step(2).reward;
        let rw = Rewards::default();
        assert!(r >= need as f32 * rw.xp + rw.level - 0.1, "reward {r}");
    }

    #[test]
    fn growing_pays_and_being_eaten_ends_the_episode() {
        let mut a = Arena::new(5, 600.0);
        let mut total = 0.0;
        let mut ended = false;
        for k in 0..3000 {
            let s = a.step(if k % 40 < 30 { 2 } else { 3 });
            total += s.reward;
            if s.done {
                ended = true;
                break;
            }
        }
        assert!(total.abs() > 0.01 || ended);
    }
}
