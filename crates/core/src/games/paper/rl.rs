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
pub const MAP: usize = BLOCKS;
pub const LOCAL: usize = 6;
pub const CHANNELS: usize = LOCAL + 4;
pub const SCALARS: usize = 14;
pub const ACTIONS: usize = 5;
pub const TURNS: [f32; ACTIONS] = [-1.1, -0.45, 0.0, 0.45, 1.1];
pub const DECIDE: f32 = 0.16;
pub const GAME_CAP: f32 = 300.0;
const HOME_GRACE: f32 = 1.5;
const STALL_GRACE: f32 = 6.0;
const GREW: f32 = 0.0005;

pub fn flipped(was: f32, now: f32) -> bool {
    was * now < 0.0
}

pub fn stall_cost(stalled: f32, per_sec: f32) -> f32 {
    per_sec * (stalled - STALL_GRACE).clamp(0.0, 30.0)
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rewards {
    pub land: f32,
    pub kill: f32,
    pub death: f32,
    pub idle: f32,
    pub linger: f32,
    pub win: f32,
    pub outlive: f32,
    pub stall: f32,
    pub trip: f32,
    pub wobble: f32,
    pub steal: f32,
}

impl Default for Rewards {
    fn default() -> Self {
        Self {
            land: 400.0,
            kill: 5.0,
            death: 4.0,
            idle: 0.01,
            linger: 0.05,
            win: 12.0,
            outlive: 0.3,
            stall: 0.01,
            trip: 0.6,
            wobble: 0.15,
            steal: 600.0,
        }
    }
}

static SHIPPED: &[u8] = include_bytes!("../../../../../assets/brains/paper_rl.kgb");

pub fn shipped() -> Option<&'static Policy> {
    static NET: std::sync::OnceLock<Option<Policy>> = std::sync::OnceLock::new();
    NET.get_or_init(|| Policy::from_bytes(SHIPPED).filter(fits))
        .as_ref()
}

static ACTIVE: std::sync::RwLock<Option<Arc<Policy>>> = std::sync::RwLock::new(None);

pub fn use_policy(p: Policy) {
    *ACTIVE.write().unwrap() = Some(Arc::new(p));
}

pub fn active() -> Option<Arc<Policy>> {
    static SHIPPED_ARC: std::sync::OnceLock<Option<Arc<Policy>>> = std::sync::OnceLock::new();
    ACTIVE.read().unwrap().clone().or_else(|| {
        SHIPPED_ARC
            .get_or_init(|| shipped().cloned().map(Arc::new))
            .clone()
    })
}

pub fn fits(p: &Policy) -> bool {
    p.grid == VIEW && p.channels == CHANNELS && p.scalars == SCALARS && p.actions == ACTIONS
}

pub fn grid_len() -> usize {
    CHANNELS * VIEW * VIEW
}

pub fn sight(w: &World, id: usize) -> f32 {
    super::camera::zoom_for(w.share(id))
}

pub fn observe(w: &World, id: usize) -> Obs {
    let mut grid = vec![0.0f32; grid_len()];
    let p = &w.players[id];
    let me = Some(id as u8);
    let span = sight(w, id);
    let reach = span * 0.5;
    let f = dir_of(p.angle);
    let (fwd, right) = (f, vec2(-f.y, f.x));
    let cell = span / VIEW as f32;
    let at = |c: usize, y: usize, x: usize| (c * VIEW + y) * VIEW + x;
    let res = super::world::RES as f32;
    let n = super::world::N;
    let half = VIEW as f32 / 2.0;
    let inside = |y: usize, x: usize| {
        let dy = half - y as f32 - 0.5;
        let dx = x as f32 + 0.5 - half;
        (dx * dx + dy * dy).sqrt() * cell <= reach
    };
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
            if !inside(y, x) {
                continue;
            }
            if let Some(o) = w.owner[(fy * n + fx) as usize] {
                grid[at(if Some(o) == me { 0 } else { 2 }, y, x)] = 1.0;
            }
        }
    }
    let place = |q: Vec2| {
        let rel = q - p.pos;
        if rel.length() > reach {
            return None;
        }
        let y = (half - rel.dot(fwd) / cell).floor();
        let x = (rel.dot(right) / cell + half).floor();
        ((0.0..VIEW as f32).contains(&y) && (0.0..VIEW as f32).contains(&x))
            .then_some((y as usize, x as usize))
    };
    for (o, q) in w.players.iter().enumerate() {
        if !q.alive {
            continue;
        }
        let layer = if o == id { 1 } else { 3 };
        for t in q.trail_points() {
            if let Some((y, x)) = place(t) {
                grid[at(layer, y, x)] = 1.0;
            }
        }
        if o != id {
            if let Some((y, x)) = place(q.pos) {
                grid[at(4, y, x)] = if q.odo >= p.odo { 1.0 } else { 0.5 };
            }
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
        sight(w, id) / super::camera::MAX_SPAN,
    ]
}

pub fn skill_of(seed: u64) -> f32 {
    (seed % 5) as f32 / 4.0
}

pub fn field(seed: u64, skill: f32, as_bot: bool) -> World {
    rand::srand(seed);
    let mut w = World::new(skill);
    w.brains = false;
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
static EVAL_THREADS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

pub fn set_eval_threads(n: usize) {
    EVAL_THREADS.store(n, std::sync::atomic::Ordering::Relaxed);
}

fn eval_threads() -> usize {
    match EVAL_THREADS.load(std::sync::atomic::Ordering::Relaxed) {
        0 => std::thread::available_parallelism().map_or(8, |n| n.get()),
        n => n,
    }
}

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
    record: f32,
    stalled: f32,
    pub trips: u32,
    last_turn: f32,
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
        let mut w = field(seed, skill_of(seed), as_bot);
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
            record: land,
            stalled: 0.0,
            trips: 0,
            last_turn: 0.0,
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
        if flipped(self.last_turn, turn) {
            reward -= r.wobble;
        }
        if turn != 0.0 {
            self.last_turn = turn;
        }
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
            let total = self.w.total() as f32;
            for c in self.w.claims.drain(..) {
                if c.id == 0 {
                    reward -= r.trip;
                    reward += c.taken as f32 / total * r.steal;
                    self.trips += 1;
                }
            }
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
            if now > self.record + GREW {
                self.record = now;
                self.stalled = 0.0;
            } else {
                self.stalled += DECIDE;
                reward -= stall_cost(self.stalled, r.stall);
            }
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
    pub stall: f32,
    pub claim: f32,
    pub wobble: f32,
    pub stolen: f32,
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
    let next = std::sync::atomic::AtomicU64::new(0);
    let runs: Vec<Report> = std::thread::scope(|s| {
        let hs: Vec<_> = (0..eval_threads().min(seeds as usize).max(1))
            .map(|_| {
                let (next, pool) = (&next, &pool);
                s.spawn(move || {
                    let mut out = vec![];
                    loop {
                        let seed = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        if seed >= seeds {
                            return out;
                        }
                        let rival =
                            (!pool.is_empty()).then(|| pool[seed as usize % pool.len()].clone());
                        let skill = skill.unwrap_or(skill_of(seed));
                        out.push(play_one(pilot, 77_000 + seed, skill, rival, share));
                    }
                })
            })
            .collect();
        hs.into_iter().flat_map(|h| h.join().unwrap()).collect()
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
        claim: avg(|r| r.claim),
        wobble: avg(|r| r.wobble),
        stolen: avg(|r| r.stolen),
        stall: runs.iter().map(|r| r.stall).sum::<f32>()
            / (runs.iter().map(|r| r.secs).sum::<f32>() / DECIDE).max(1.0),
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
    let mut w = field(seed, skill, bot);
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
    let (mut record, mut stalled) = (w.share(0), 0.0f32);
    let (mut trips, mut claimed) = (0.0f32, 0.0f32);
    let (mut last_turn, mut flips, mut decisions) = (0.0f32, 0.0f32, 0.0f32);
    let mut last = w.players[0].angle;
    let mut think = 0.0f32;
    for _ in 0..(GAME_CAP / DECIDE) as usize {
        if let Pilot::Policy(p, e) = pilot {
            think -= DECIDE;
            if think <= 0.0 {
                think += e.think_every(DECIDE);
                let before = w.players[0].angle;
                w.rl_steer(0, p, *e);
                let turn = wrap_angle(w.players[0].want - before);
                let turn = if turn.abs() < 0.05 { 0.0 } else { turn };
                decisions += 1.0;
                if flipped(last_turn, turn) {
                    flips += 1.0;
                }
                if turn != 0.0 {
                    last_turn = turn;
                }
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
            let total = w.total() as f32;
            for c in w.claims.drain(..) {
                if c.id == 0 {
                    trips += 1.0;
                    claimed += c.cells as f32 / total;
                    r.stolen += c.taken as f32 / total;
                }
            }
        }
        if r.deaths > 0.0 {
            break;
        }
        r.secs += DECIDE;
        r.peak = r.peak.max(w.share(0));
        if w.share(0) > record + GREW {
            record = w.share(0);
            stalled = 0.0;
        } else {
            stalled += DECIDE;
            if stalled > STALL_GRACE {
                r.stall += 1.0;
            }
        }
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
    r.claim = claimed / trips.max(1.0);
    r.wobble = flips / decisions.max(1.0);
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
    use super::*;

    #[test]
    fn the_way_home_points_back_at_the_players_own_land() {
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
    fn the_near_view_shows_only_what_the_players_screen_shows() {
        let mut a = Arena::new(11, 600.0);
        for k in 0..200 {
            a.step(k % ACTIONS);
        }
        let w = &a.w;
        let reach = sight(w, 0) * 0.5;
        let o = observe(w, 0);
        let half = VIEW as f32 / 2.0;
        let cell = sight(w, 0) / VIEW as f32;
        for c in [0, 1, 2, 3, 4] {
            for y in 0..VIEW {
                for x in 0..VIEW {
                    if o.grid[(c * VIEW + y) * VIEW + x] == 0.0 {
                        continue;
                    }
                    let d = vec2(x as f32 + 0.5 - half, half - y as f32 - 0.5).length() * cell;
                    assert!(d <= reach + cell * 1.5, "layer {c} at {d} beyond {reach}");
                }
            }
        }
    }

    #[test]
    fn more_land_zooms_the_view_out_like_the_camera() {
        let mut w = field(4, 1.0, false);
        let small = sight(&w, 0);
        let me = Some(0u8);
        for i in 0..w.owner.len() / 5 {
            if w.owner[i].is_none() {
                w.claim(i, me);
            }
        }
        assert!(sight(&w, 0) > small * 1.3);
        assert_eq!(sight(&w, 0), super::super::camera::zoom_for(w.share(0)));
    }

    #[test]
    fn the_view_turns_with_the_player_and_shows_its_own_land_behind() {
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
        let mut w = field(4, 1.0, false);
        let far = w.players[0].pos + vec2(sight(&w, 0) * 0.6, 0.0);
        w.players[1].pos = far.clamp(Vec2::splat(1.0), Vec2::splat(SIZE - 1.0));
        let o = observe(&w, 0);
        let near = &o.grid[4 * VIEW * VIEW..5 * VIEW * VIEW];
        let mini = &o.grid[(LOCAL + 3) * VIEW * VIEW..(LOCAL + 4) * VIEW * VIEW];
        let p1 = w.players[1].pos - w.players[0].pos;
        assert!(p1.length() > sight(&w, 0) * 0.5);
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
    fn being_the_last_one_standing_pays_the_win_bonus_and_ends_the_episode() {
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
    fn going_nowhere_drains_the_reward_after_a_grace() {
        assert_eq!(stall_cost(STALL_GRACE * 0.5, 0.01), 0.0);
        assert!(stall_cost(STALL_GRACE + 10.0, 0.01) > stall_cost(STALL_GRACE + 1.0, 0.01));
        let mut a = Arena::new(9, 120.0);
        a.w.players[0].speed = 0.0;
        let mut late = 0.0;
        for k in 0..((STALL_GRACE + 5.0) / DECIDE) as usize {
            let s = a.step(2);
            if k as f32 * DECIDE > STALL_GRACE + 1.0 {
                late += s.reward;
            }
            if s.done {
                break;
            }
        }
        assert!(late < -0.1, "{late}");
    }

    #[test]
    fn every_trip_home_costs_a_fixed_toll() {
        let mut a = Arena::new(9, 120.0);
        let mut tolls = 0;
        for k in 0..400 {
            let before = a.trips;
            let s = a.step(if k % 14 < 11 { 2 } else { 4 });
            if s.done {
                break;
            }
            if a.trips > before {
                tolls += 1;
            }
        }
        assert!(tolls > 3, "{tolls}");
        assert!(a.trips >= tolls);
    }

    #[test]
    fn zig_zagging_costs_and_holding_a_turn_does_not() {
        let mut a = Arena::new(9, 120.0);
        let mut b = Arena::new(9, 120.0);
        let (mut zig, mut arc) = (0.0, 0.0);
        for k in 0..6 {
            zig += a.step(if k % 2 == 0 { 1 } else { 3 }).reward;
            arc += b.step(3).reward;
        }
        assert!(arc - zig > Rewards::default().wobble * 4.0, "{zig} {arc}");
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
