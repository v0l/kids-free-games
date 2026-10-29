use super::arena;
use super::world::{angle_of, dir_of, spread_spots, wrap_angle, World, BLOCK, BLOCKS, SIZE, SUB};
use crate::brain::conv::{Obs, Policy};
use crate::brain::learn::{Seat, Spec, Tick};
use crate::brain::Effort;
use crate::rng as rand;
use glam::{vec2, Vec2};
use std::f32::consts::{PI, TAU};
use std::sync::Arc;

pub const VIEW: usize = 32;
pub const VIEW_SPAN: f32 = 32.0;
pub const MAP: usize = BLOCKS;
pub const LOCAL: usize = 6;
pub const CHANNELS: usize = LOCAL + 4;
pub const SCALARS: usize = 13;
pub const ACTIONS: usize = 5;
pub const TURNS: [f32; ACTIONS] = [-1.1, -0.45, 0.0, 0.45, 1.1];
pub const DECIDE: f32 = 0.16;
pub const GAME_CAP: f32 = 300.0;
const HOME_GRACE: f32 = 1.5;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rewards {
    pub land: f32,
    pub kill: f32,
    pub death: f32,
    pub idle: f32,
    pub linger: f32,
    pub win: f32,
    pub outlive: f32,
}

impl Default for Rewards {
    fn default() -> Self {
        Self {
            land: 400.0,
            kill: 2.0,
            death: 4.0,
            idle: 0.01,
            linger: 0.05,
            win: 12.0,
            outlive: 0.3,
        }
    }
}

static SHIPPED: &[u8] = include_bytes!("../../../../../assets/brains/paper_rl.kgb");

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

pub fn observe(w: &World, id: usize) -> Obs {
    let mut grid = vec![0.0f32; grid_len()];
    let p = &w.players[id];
    let me = Some(id as u8);
    let f = dir_of(p.angle);
    let (fwd, right) = (f, vec2(-f.y, f.x));
    let cell = VIEW_SPAN / VIEW as f32;
    let at = |c: usize, y: usize, x: usize| (c * VIEW + y) * VIEW + x;
    let res = super::world::RES as f32;
    let n = super::world::N;
    let half = VIEW as f32 / 2.0;
    let step_x = right * cell * res;
    for y in 0..VIEW {
        let ahead = (half - y as f32 - 0.5) * cell;
        let mut q = (p.pos + fwd * ahead + right * ((0.5 - half) * cell)) * res;
        for x in 0..VIEW {
            let (fx, fy) = (q.x.floor() as i32, q.y.floor() as i32);
            q += step_x;
            if fx < 0 || fy < 0 || fx >= n || fy >= n {
                grid[at(5, y, x)] = 1.0;
                continue;
            }
            let i = (fy * n + fx) as usize;
            if let Some(o) = w.owner[i] {
                grid[at(if Some(o) == me { 0 } else { 2 }, y, x)] = 1.0;
            }
            if let Some(o) = w.trail[i] {
                grid[at(if Some(o) == me { 1 } else { 3 }, y, x)] = 1.0;
            }
        }
    }
    for (o, q) in w.players.iter().enumerate() {
        if o == id || !q.alive {
            continue;
        }
        let rel = q.pos - p.pos;
        if rel.length() > VIEW_SPAN * 0.5 {
            continue;
        }
        let y = (half - rel.dot(fwd) / cell).floor();
        let x = (rel.dot(right) / cell + half).floor();
        if (0.0..VIEW as f32).contains(&y) && (0.0..VIEW as f32).contains(&x) {
            grid[at(4, y as usize, x as usize)] = if q.odo >= p.odo { 1.0 } else { 0.5 };
        }
    }
    let up = VIEW / MAP;
    let paint = |c: usize, my: usize, mx: usize, v: f32, grid: &mut Vec<f32>| {
        for dy in 0..up {
            for dx in 0..up {
                grid[at(LOCAL + c, my * up + dy, mx * up + dx)] = v;
            }
        }
    };
    let per = (BLOCK * BLOCK) as f32;
    for my in 0..MAP {
        for mx in 0..MAP {
            let k = my * MAP + mx;
            let all: u16 = w.block_owner[k].iter().sum();
            let mine = w.block_owner[k][id];
            paint(0, my, mx, mine as f32 / per, &mut grid);
            paint(2, my, mx, (all - mine) as f32 / per, &mut grid);
        }
    }
    let cellm = SIZE / MAP as f32;
    let mark = |pos: Vec2| {
        let c = (pos / cellm).floor();
        (
            c.y.clamp(0.0, MAP as f32 - 1.0) as usize,
            c.x.clamp(0.0, MAP as f32 - 1.0) as usize,
        )
    };
    let (sy, sx) = mark(p.pos);
    paint(1, sy, sx, 1.0, &mut grid);
    for (o, q) in w.players.iter().enumerate() {
        if o != id && q.alive {
            let (y, x) = mark(q.pos);
            paint(3, y, x, 1.0, &mut grid);
        }
    }
    Obs {
        grid,
        scalars: scalars(w, id),
    }
}

fn scalars(w: &World, id: usize) -> Vec<f32> {
    let p = &w.players[id];
    let home = if p.out() { w.nearest_own(id) } else { None };
    let (home_turn, home_dist) = match home {
        Some(at) => {
            let v = at - p.pos;
            (
                wrap_angle(angle_of(v) - p.angle) / PI,
                (v.length() / 30.0).min(1.0),
            )
        }
        None => (0.0, if p.out() { 1.0 } else { 0.0 }),
    };
    let nearest = (1.0 - w.nearest_head(id, p.pos) / 20.0).max(0.0);
    let field = w.players.len().saturating_sub(1).max(1) as f32;
    let left = w
        .players
        .iter()
        .enumerate()
        .filter(|(o, q)| *o != id && q.alive)
        .count() as f32;
    let ranks = w.ranking();
    let rank = ranks.iter().position(|r| *r == id).unwrap_or(ranks.len()) as f32;
    let top = ranks
        .iter()
        .filter(|r| **r != id)
        .map(|r| w.share(*r))
        .fold(0.0, f32::max);
    vec![
        if p.out() { 1.0 } else { 0.0 },
        (p.odo / 40.0).min(2.0),
        home_turn,
        home_dist,
        (w.share(id) * 10.0).min(2.0),
        nearest,
        p.angle.cos(),
        p.angle.sin(),
        left / field,
        rank / ranks.len().max(1) as f32,
        ((top - w.share(id)) * 10.0).clamp(-1.0, 1.0),
        p.speed / arena::PLAYER_SPEED,
        p.turn / super::world::PLAYER_TURN,
    ]
}

pub fn skill_of(seed: u64) -> f32 {
    (seed % 5) as f32 / 4.0
}

fn field(seed: u64, skill: f32, as_bot: bool, brains: bool) -> World {
    rand::srand(seed);
    let mut w = World::new(skill);
    w.brains = brains;
    let spots = spread_spots(arena::bots_for(skill) + 1);
    w.add_player(spots[0], rand::gen_range(-PI, PI), as_bot);
    w.players[0].manual = true;
    for s in &spots[1..] {
        w.add_player(*s, rand::gen_range(-PI, PI), true);
    }
    w
}

fn seat_rivals(w: &mut World, share: f32) -> Vec<usize> {
    let mut seated = vec![];
    for id in 1..w.players.len() {
        if rand::gen_range(0.0, 1.0) < share {
            w.players[id].manual = true;
            seated.push(id);
        }
    }
    seated
}

static SEATS: std::sync::Mutex<(usize, f32)> = std::sync::Mutex::new((0, 0.0));

pub fn set_seats(policies: usize, share: f32) {
    *SEATS.lock().unwrap() = (policies, share);
}

#[derive(Default)]
struct Watch {
    home_for: f32,
    turned: f32,
}

impl Watch {
    fn tick(&mut self, w: &World, id: usize, turn: f32) -> (bool, bool) {
        let p = &w.players[id];
        if !p.out() && w.owner_at(p.pos) == Some(id as u8) {
            self.home_for += DECIDE;
            self.turned += turn.abs();
            (true, self.turned > TAU * 0.75)
        } else {
            self.home_for = 0.0;
            self.turned = 0.0;
            (false, false)
        }
    }
}

fn others_alive(w: &World) -> usize {
    w.players.iter().skip(1).filter(|p| p.alive).count()
}

pub struct Arena {
    pub w: World,
    pub obs: Obs,
    pub rewards: Rewards,
    land: f32,
    watch: Watch,
    pub ret: f32,
    pub finished: Vec<f32>,
    pub peaks: Vec<f32>,
    pub steps: u32,
    max_steps: u32,
    seed: u64,
    seats: Vec<Seat>,
    seat_of: Vec<usize>,
    seat_policy: usize,
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
        let as_bot = (seed / 5) % 2 == 1;
        let brains = !(seed / 10).is_multiple_of(4);
        let mut w = field(seed, skill_of(seed), as_bot, brains);
        let (policies, share) = *SEATS.lock().unwrap();
        let seat_of = if policies > 0 {
            seat_rivals(&mut w, share)
        } else {
            vec![]
        };
        let land = w.share(0);
        let mut a = Self {
            obs: observe(&w, 0),
            w,
            rewards,
            land,
            watch: Watch::default(),
            ret: 0.0,
            finished: vec![],
            peaks: vec![],
            steps: 0,
            max_steps: (secs / DECIDE) as u32,
            seed,
            seats: vec![],
            seat_of,
            seat_policy: if policies > 0 {
                (seed / 20) as usize % policies
            } else {
                0
            },
        };
        a.refresh_seats();
        a
    }

    fn refresh_seats(&mut self) {
        self.seats.clear();
        for &id in &self.seat_of {
            if self.w.players[id].alive {
                self.seats.push(Seat {
                    policy: self.seat_policy,
                    obs: observe(&self.w, id),
                });
            }
        }
    }

    pub fn seat_actions(&mut self, actions: &[usize]) {
        let alive: Vec<usize> = self
            .seat_of
            .iter()
            .copied()
            .filter(|id| self.w.players[*id].alive)
            .collect();
        for (id, a) in alive.into_iter().zip(actions) {
            let p = &mut self.w.players[id];
            p.want = wrap_angle(p.angle + TURNS[(*a).min(ACTIONS - 1)]);
        }
    }

    pub fn step(&mut self, action: usize) -> Step {
        let r = self.rewards;
        let turn = TURNS[action.min(ACTIONS - 1)];
        {
            let p = &mut self.w.players[0];
            p.want = wrap_angle(p.angle + turn);
        }
        let mut reward = 0.0;
        let mut dead = false;
        for _ in 0..(DECIDE / SUB).round() as usize {
            self.w.step_with(&mut |_, _| None);
            for d in std::mem::take(&mut self.w.deaths) {
                if d.victim == 0 {
                    dead = true;
                } else if !dead {
                    reward += r.outlive;
                }
                if d.killer == Some(0) {
                    reward += r.kill;
                }
            }
            self.w.claims.clear();
            if dead {
                break;
            }
        }
        if dead {
            reward -= r.death + self.land * r.land;
        } else {
            let now = self.w.share(0);
            reward += (now - self.land) * r.land;
            self.land = now;
            let (home, _) = self.watch.tick(&self.w, 0, turn);
            if home {
                reward -= r.idle;
                if self.watch.home_for > HOME_GRACE {
                    reward -= r.linger * (self.watch.home_for - HOME_GRACE).min(4.0);
                }
            }
        }
        let won = !dead && others_alive(&self.w) == 0;
        if won {
            reward += r.win;
        }
        self.ret += reward;
        self.steps += 1;
        let done = dead || won || self.steps >= self.max_steps;
        if done {
            self.finished.push(self.ret);
            self.peaks.push(self.w.players[0].peak);
            let finished = std::mem::take(&mut self.finished);
            let peaks = std::mem::take(&mut self.peaks);
            let secs = self.max_steps as f32 * DECIDE;
            self.seed = self
                .seed
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            *self = Self::with(self.seed >> 1, secs, self.rewards);
            self.finished = finished;
            self.peaks = peaks;
        } else {
            self.obs = observe(&self.w, 0);
            self.refresh_seats();
        }
        Step { reward, done }
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Report {
    pub wins: f32,
    pub peak: f32,
    pub secs: f32,
    pub deaths: f32,
    pub kills: f32,
    pub outlived: f32,
    pub inside: f32,
    pub circling: f32,
}

pub enum Pilot<'a> {
    Policy(&'a Policy, Effort),
    Bot,
}

pub fn evaluate(pilot: &Pilot, seeds: u64, skill: Option<f32>) -> Report {
    evaluate_against(pilot, seeds, skill, &[], 0.0)
}

pub fn evaluate_against(
    pilot: &Pilot,
    seeds: u64,
    skill: Option<f32>,
    rivals: &[Policy],
    share: f32,
) -> Report {
    let pool: Vec<Arc<Policy>> = rivals.iter().cloned().map(Arc::new).collect();
    let runs: Vec<Report> = std::thread::scope(|s| {
        let hs: Vec<_> = (0..seeds)
            .map(|seed| {
                let rival = (!pool.is_empty()).then(|| pool[seed as usize % pool.len()].clone());
                let skill = skill.unwrap_or(skill_of(seed));
                s.spawn(move || play_one(pilot, 77_000 + seed, skill, rival, share))
            })
            .collect();
        hs.into_iter().map(|h| h.join().unwrap()).collect()
    });
    let n = seeds.max(1) as f32;
    let avg = |f: fn(&Report) -> f32| runs.iter().map(f).sum::<f32>() / n;
    let inside = runs.iter().map(|r| r.inside).sum::<f32>();
    Report {
        wins: avg(|r| r.wins),
        peak: avg(|r| r.peak),
        secs: avg(|r| r.secs),
        deaths: avg(|r| r.deaths),
        kills: avg(|r| r.kills),
        outlived: avg(|r| r.outlived),
        inside: inside / runs.iter().map(|r| r.secs).sum::<f32>().max(1.0) * DECIDE,
        circling: runs.iter().map(|r| r.circling).sum::<f32>() / inside.max(1.0),
    }
}

fn play_one(
    pilot: &Pilot,
    seed: u64,
    skill: f32,
    rival: Option<Arc<Policy>>,
    share: f32,
) -> Report {
    let bot = matches!(pilot, Pilot::Bot);
    let mut w = field(seed, skill, bot, true);
    if bot {
        let p = &mut w.players[0];
        p.manual = false;
        p.tune = arena::tuning(arena::PERSONAS[1], 1.0);
        p.speed = arena::PLAYER_SPEED;
        p.turn = super::world::PLAYER_TURN;
    }
    let seated = match &rival {
        Some(_) => seat_rivals(&mut w, share),
        None => vec![],
    };
    let field = others_alive(&w) as f32;
    let mut r = Report::default();
    let mut watch = Watch::default();
    let mut last = w.players[0].angle;
    let mut think = 0.0f32;
    for _ in 0..(GAME_CAP / DECIDE) as usize {
        if let Pilot::Policy(p, e) = pilot {
            think -= DECIDE;
            if think <= 0.0 {
                think += e.think_every(DECIDE);
                w.rl_steer(0, p, *e);
            }
        }
        if let Some(net) = &rival {
            for &id in &seated {
                if w.players[id].alive {
                    let a = net.choose(&observe(&w, id), Effort::BEST);
                    let q = &mut w.players[id];
                    q.want = wrap_angle(q.angle + TURNS[a]);
                }
            }
        }
        for _ in 0..(DECIDE / SUB).round() as usize {
            w.step_with(&mut |_, _| None);
            for d in std::mem::take(&mut w.deaths) {
                if d.victim == 0 {
                    r.deaths = 1.0;
                }
                if d.killer == Some(0) {
                    r.kills += 1.0;
                }
            }
            w.claims.clear();
        }
        if r.deaths > 0.0 {
            break;
        }
        r.secs += DECIDE;
        r.peak = r.peak.max(w.share(0));
        let a = w.players[0].angle;
        let (home, circling) = watch.tick(&w, 0, wrap_angle(a - last));
        last = a;
        r.inside += f32::from(home);
        r.circling += f32::from(circling);
        if others_alive(&w) == 0 {
            r.wins = 1.0;
            break;
        }
    }
    r.outlived = (field - others_alive(&w) as f32) / field.max(1.0);
    r
}

impl crate::brain::learn::Learner for Arena {
    fn spec() -> Spec {
        Spec {
            channels: CHANNELS,
            side: VIEW,
            scalars: SCALARS,
            actions: ACTIONS,
        }
    }

    fn agents(&self) -> usize {
        1
    }

    fn obs(&self, _: usize) -> &Obs {
        &self.obs
    }

    fn step(&mut self, actions: &[usize], out: &mut [Tick]) {
        let s = Arena::step(self, actions[0]);
        out[0] = Tick {
            reward: s.reward,
            done: s.done,
        };
    }

    fn drain_episodes(&mut self) -> Vec<(f32, f32)> {
        let r = std::mem::take(&mut self.finished);
        let p = std::mem::take(&mut self.peaks);
        r.into_iter().zip(p).map(|(a, b)| (a, b * 100.0)).collect()
    }

    fn seats(&self) -> &[Seat] {
        &self.seats
    }

    fn seat_actions(&mut self, actions: &[usize]) {
        Arena::seat_actions(self, actions);
    }
}

#[cfg(test)]
mod tests {
    use super::super::world::{cell_at, idx};
    use super::*;

    fn observe_reference(w: &World, id: usize) -> Vec<f32> {
        let mut grid = vec![0.0f32; CHANNELS * VIEW * VIEW];
        let p = &w.players[id];
        let me = Some(id as u8);
        let f = dir_of(p.angle);
        let (fwd, right) = (f, vec2(-f.y, f.x));
        let cell = VIEW_SPAN / VIEW as f32;
        let at = |c: usize, y: usize, x: usize| (c * VIEW + y) * VIEW + x;
        for y in 0..VIEW {
            for x in 0..VIEW {
                let ahead = (VIEW as f32 / 2.0 - y as f32 - 0.5) * cell;
                let side = (x as f32 + 0.5 - VIEW as f32 / 2.0) * cell;
                let q = p.pos + fwd * ahead + right * side;
                if q.x < 0.0 || q.y < 0.0 || q.x >= SIZE || q.y >= SIZE {
                    grid[at(5, y, x)] = 1.0;
                    continue;
                }
                let i = idx(cell_at(q));
                match w.owner[i] {
                    Some(o) if Some(o) == me => grid[at(0, y, x)] = 1.0,
                    Some(_) => grid[at(2, y, x)] = 1.0,
                    None => {}
                }
                match w.trail[i] {
                    Some(o) if Some(o) == me => grid[at(1, y, x)] = 1.0,
                    Some(_) => grid[at(3, y, x)] = 1.0,
                    None => {}
                }
            }
        }
        grid
    }

    #[test]
    fn the_way_home_points_back_at_the_snakes_own_land() {
        let mut w = World::new(0.5);
        let id = w.add_player(vec2(30.0, 30.0), 0.0, false);
        w.players[id].pos = vec2(45.0, 30.0);
        let home = w.nearest_own(id).unwrap();
        assert!(home.x < 35.0 && (home.y - 30.0).abs() < 3.5, "{home}");
        assert!(home.distance(vec2(45.0, 30.0)) < 13.0);
        let id2 = w.add_player(vec2(80.0, 80.0), 0.0, false);
        let near = w.nearest_own(id2).unwrap();
        assert!(near.distance(vec2(80.0, 80.0)) < 1.0, "{near}");
    }

    #[test]
    fn the_fast_view_matches_the_straightforward_one() {
        let mut a = Arena::new(11, 600.0);
        let mut worst = 0usize;
        for k in 0..120 {
            a.step(k % ACTIONS);
            if k % 10 != 9 {
                continue;
            }
            for id in 0..a.w.players.len() {
                if !a.w.players[id].alive {
                    continue;
                }
                let fast = observe(&a.w, id);
                let slow = observe_reference(&a.w, id);
                let diff = (0..LOCAL * VIEW * VIEW)
                    .filter(|i| i / (VIEW * VIEW) != 4)
                    .filter(|i| fast.grid[*i] != slow[*i])
                    .count();
                worst = worst.max(diff);
            }
        }
        assert!(worst <= 4, "{worst} local cells differ");
    }

    #[test]
    fn the_view_turns_with_the_snake_and_shows_its_own_land_behind() {
        let a = Arena::new(3, 30.0);
        let o = &a.obs;
        assert_eq!(o.grid.len(), grid_len());
        assert_eq!(o.scalars.len(), SCALARS);
        let own: f32 = o.grid[..VIEW * VIEW].iter().sum();
        let map_own: f32 = o.grid[LOCAL * VIEW * VIEW..(LOCAL + 1) * VIEW * VIEW]
            .iter()
            .sum();
        assert!(map_own > 0.0, "own land on the map summary");
        assert!(own > 10.0, "own land near the start");
        let centre = (VIEW / 2) * VIEW + VIEW / 2;
        assert_eq!(o.grid[centre], 1.0, "standing on own land");
    }

    #[test]
    fn heads_off_screen_only_show_on_the_minimap() {
        let mut w = field(4, 1.0, false, false);
        let far = w.players[0].pos + vec2(VIEW_SPAN * 0.6, 0.0);
        w.players[1].pos = far.clamp(Vec2::splat(1.0), Vec2::splat(SIZE - 1.0));
        let o = observe(&w, 0);
        let near = &o.grid[4 * VIEW * VIEW..5 * VIEW * VIEW];
        let mini = &o.grid[(LOCAL + 3) * VIEW * VIEW..(LOCAL + 4) * VIEW * VIEW];
        let p1 = w.players[1].pos - w.players[0].pos;
        assert!(p1.length() > VIEW_SPAN * 0.5);
        assert!(near.iter().filter(|v| **v > 0.0).count() < w.players.len() - 1);
        assert!(mini.iter().sum::<f32>() > 0.0);
    }

    #[test]
    fn the_field_matches_the_real_game_for_each_skill_band() {
        for seed in 0..5 {
            let a = Arena::new(seed, 30.0);
            let skill = skill_of(seed);
            assert_eq!(a.w.players.len(), arena::bots_for(skill) + 1);
            assert!(a.w.players[0].manual);
        }
        assert_eq!(skill_of(0), 0.0);
        assert_eq!(skill_of(4), 1.0);
    }

    #[test]
    fn being_the_last_snake_pays_the_win_bonus_and_ends_the_episode() {
        let mut a = Arena::new(2, 60.0);
        for id in 1..a.w.players.len() {
            a.w.kill(id, None, super::super::world::Cause::Trail);
        }
        a.w.deaths.clear();
        let s = a.step(2);
        assert!(s.done);
        assert!(s.reward >= Rewards::default().win * 0.9, "{}", s.reward);
        assert_eq!(a.finished.len(), 1);
    }

    #[test]
    fn dying_ends_the_episode_and_costs_the_land() {
        let mut a = Arena::new(5, 60.0);
        let land = a.w.share(0);
        a.w.kill(0, Some(1), super::super::world::Cause::Trail);
        let s = a.step(2);
        assert!(s.done);
        assert!(s.reward <= -(Rewards::default().death + land * 300.0));
    }

    #[test]
    fn seated_rivals_are_steered_by_the_actions_handed_in() {
        set_seats(1, 1.0);
        let mut a = Arena::new(7, 60.0);
        set_seats(0, 0.0);
        assert_eq!(a.seats.len(), a.w.players.len() - 1);
        let id = a.seat_of[0];
        let before = a.w.players[id].angle;
        a.seat_actions(&vec![0; a.seats.len()]);
        assert!((wrap_angle(a.w.players[id].want - before) - TURNS[0]).abs() < 1e-4);
        assert!(a.w.players[id].manual);
    }

    #[test]
    fn an_episode_resets_after_its_time_runs_out() {
        let mut a = Arena::new(7, 1.0);
        let mut done = false;
        for _ in 0..20 {
            if a.step(2).done {
                done = true;
                break;
            }
        }
        assert!(done);
        assert_eq!(a.steps, 0);
        assert_eq!(a.finished.len(), 1);
    }
}
