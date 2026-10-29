use super::world::{dir_of, wrap_angle, World, BLOCK, BLOCKS, SIZE, SUB};
use crate::brain::conv::Obs;
use crate::rng as rand;
use glam::{vec2, Vec2};
use std::f32::consts::PI;

pub const VIEW: usize = 32;
pub const VIEW_SPAN: f32 = 32.0;
pub const MAP: usize = BLOCKS;
pub const LOCAL: usize = 6;
pub const CHANNELS: usize = 12;
pub const SCALARS: usize = 8;
pub const ACTIONS: usize = 5;
pub const TURNS: [f32; ACTIONS] = [-1.1, -0.45, 0.0, 0.45, 1.1];
pub const DECIDE: f32 = 0.16;
pub const BOTS: usize = 10;
const LAND_WEIGHT: f32 = 400.0;
const KILL_REWARD: f32 = 1.0;
const DEATH_PENALTY: f32 = 2.0;
const IDLE_COST: f32 = 0.01;
const HOME_GRACE: f32 = 1.5;
const LINGER_COST: f32 = 0.05;
const RESPAWN: f32 = 1.0;

static SHIPPED: &[u8] = include_bytes!("../../../../../assets/brains/paper_rl.kgb");

pub fn shipped() -> Option<&'static crate::brain::conv::Policy> {
    static NET: std::sync::OnceLock<Option<crate::brain::conv::Policy>> =
        std::sync::OnceLock::new();
    NET.get_or_init(|| {
        crate::brain::conv::Policy::from_bytes(SHIPPED).filter(|p| {
            p.grid == VIEW && p.channels == CHANNELS && p.scalars == SCALARS && p.actions == ACTIONS
        })
    })
    .as_ref()
}

pub fn grid_len() -> usize {
    CHANNELS * VIEW * VIEW
}

pub struct Coarse;

impl Coarse {
    pub fn of(_: &World) -> Self {
        Self
    }
}

pub fn observe(w: &World, id: usize, _: &Coarse) -> Obs {
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
        let y = (VIEW as f32 / 2.0 - rel.dot(fwd) / cell).floor();
        let x = (rel.dot(right) / cell + VIEW as f32 / 2.0).floor();
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
    for my in 0..MAP {
        for mx in 0..MAP {
            let k = my * MAP + mx;
            let per = (BLOCK * BLOCK) as f32;
            let all: u16 = w.block_owner[k].iter().sum();
            let mine = w.block_owner[k][id] as f32 / per;
            let theirs = (all - w.block_owner[k][id]) as f32 / per;
            let trails = (w.block_trail[k] as f32 / per * 8.0).min(1.0);
            paint(0, my, mx, mine, &mut grid);
            paint(2, my, mx, theirs, &mut grid);
            paint(3, my, mx, trails, &mut grid);
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
            paint(4, y, x, 1.0, &mut grid);
        }
    }
    let home = if p.out() { w.nearest_own(id) } else { None };
    let (home_turn, home_dist) = match home {
        Some(at) => {
            let v = at - p.pos;
            (
                wrap_angle(super::world::angle_of(v) - p.angle) / PI,
                (v.length() / 30.0).min(1.0),
            )
        }
        None => (0.0, if p.out() { 1.0 } else { 0.0 }),
    };
    let nearest = (1.0 - w.nearest_head(id, p.pos) / 20.0).max(0.0);
    let scalars = vec![
        if p.out() { 1.0 } else { 0.0 },
        (p.odo / 40.0).min(2.0),
        home_turn,
        home_dist,
        (w.share(id) * 10.0).min(2.0),
        nearest,
        p.angle.cos(),
        p.angle.sin(),
    ];
    Obs { grid, scalars }
}

pub struct Arena {
    pub w: World,
    pub obs: Vec<Obs>,
    land: Vec<f32>,
    home_for: Vec<f32>,
    pub home_turn: Vec<f32>,
    pub home_secs: f32,
    pub circling_secs: f32,
    dead_for: Vec<f32>,
    pub returns: Vec<f32>,
    pub finished: Vec<f32>,
    pub peaks: Vec<f32>,
    pub steps: u32,
    max_steps: u32,
}

pub struct Step {
    pub reward: Vec<f32>,
    pub done: Vec<bool>,
}

impl Arena {
    pub fn new(seed: u64, skill: f32, secs: f32) -> Self {
        rand::srand(seed);
        let mut w = World::new(skill);
        w.brains = false;
        for s in super::world::spread_spots(BOTS) {
            w.add_player(s, rand::gen_range(-PI, PI), true);
        }
        for p in w.players.iter_mut() {
            p.manual = true;
        }
        let land = (0..BOTS).map(|i| w.share(i)).collect();
        let mut a = Self {
            w,
            obs: vec![],
            land,
            home_for: vec![0.0; BOTS],
            home_turn: vec![0.0; BOTS],
            home_secs: 0.0,
            circling_secs: 0.0,
            dead_for: vec![0.0; BOTS],
            returns: vec![0.0; BOTS],
            finished: vec![],
            peaks: vec![],
            steps: 0,
            max_steps: (secs / DECIDE) as u32,
        };
        a.refresh();
        a
    }

    fn refresh(&mut self) {
        let coarse = Coarse::of(&self.w);
        self.obs = (0..BOTS).map(|i| observe(&self.w, i, &coarse)).collect();
    }

    pub fn alive(&self, id: usize) -> bool {
        self.w.players[id].alive
    }

    pub fn write_obs(&self, grid: &mut [u8], scalars: &mut [f32]) {
        let g = grid_len();
        for (i, o) in self.obs.iter().enumerate() {
            for (d, v) in grid[i * g..(i + 1) * g].iter_mut().zip(&o.grid) {
                *d = (v * 255.0 + 0.5) as u8;
            }
            scalars[i * SCALARS..(i + 1) * SCALARS].copy_from_slice(&o.scalars);
        }
    }

    pub fn step(&mut self, actions: &[usize]) -> Step {
        for (id, a) in actions.iter().enumerate() {
            let p = &mut self.w.players[id];
            if p.alive {
                p.want = wrap_angle(p.angle + TURNS[(*a).min(ACTIONS - 1)]);
            }
        }
        let mut reward = vec![0.0; BOTS];
        let mut done = vec![false; BOTS];
        let ticks = (DECIDE / SUB).round() as usize;
        for _ in 0..ticks {
            self.w.step_with(&mut |_, _| None);
            for d in std::mem::take(&mut self.w.deaths) {
                if d.victim < BOTS && !done[d.victim] {
                    reward[d.victim] -= DEATH_PENALTY + self.land[d.victim] * LAND_WEIGHT;
                    self.land[d.victim] = 0.0;
                    done[d.victim] = true;
                }
                if let Some(k) = d.killer.filter(|k| *k < BOTS) {
                    reward[k] += KILL_REWARD;
                }
            }
            self.w.claims.clear();
        }
        for id in 0..BOTS {
            let p = &self.w.players[id];
            if p.alive {
                let now = self.w.share(id);
                reward[id] += (now - self.land[id]) * LAND_WEIGHT;
                self.land[id] = now;
                if !p.out() && self.w.owner_at(p.pos) == Some(id as u8) {
                    self.home_for[id] += DECIDE;
                    self.home_turn[id] += TURNS[actions[id].min(ACTIONS - 1)].abs();
                    self.home_secs += DECIDE;
                    reward[id] -= IDLE_COST;
                    if self.home_for[id] > HOME_GRACE {
                        reward[id] -= LINGER_COST * (self.home_for[id] - HOME_GRACE).min(4.0);
                    }
                    if self.home_turn[id] > std::f32::consts::TAU * 0.75 {
                        self.circling_secs += DECIDE;
                    }
                } else {
                    self.home_for[id] = 0.0;
                    self.home_turn[id] = 0.0;
                }
            }
            self.returns[id] += reward[id];
        }
        self.steps += 1;
        let timeout = self.steps >= self.max_steps;
        for (id, finished) in done.iter().enumerate() {
            if *finished {
                self.close(id);
            }
            if !self.w.players[id].alive {
                self.dead_for[id] += DECIDE;
                if self.dead_for[id] >= RESPAWN {
                    if let Some(at) = self.w.free_spot(40) {
                        self.w.respawn(id, at);
                        self.w.players[id].manual = true;
                        self.land[id] = self.w.share(id);
                        self.dead_for[id] = 0.0;
                    }
                }
            }
        }
        if timeout {
            for (id, d) in done.iter_mut().enumerate() {
                if self.w.players[id].alive {
                    self.close(id);
                }
                *d = true;
            }
            let seed = rand::gen_range(0u64, u64::MAX);
            let skill = self.w.skill;
            let secs = self.max_steps as f32 * DECIDE;
            let finished = std::mem::take(&mut self.finished);
            let peaks = std::mem::take(&mut self.peaks);
            let (hs, cs) = (self.home_secs, self.circling_secs);
            *self = Self::new(seed, skill, secs);
            self.finished = finished;
            self.peaks = peaks;
            self.home_secs = hs;
            self.circling_secs = cs;
        }
        self.refresh();
        Step { reward, done }
    }

    fn close(&mut self, id: usize) {
        self.finished.push(self.returns[id]);
        self.peaks.push(self.w.players[id].peak);
        self.returns[id] = 0.0;
    }
}

type Run = (f32, f32, f32, f32, f32, f32);

pub struct Report {
    pub peak: f32,
    pub deaths: f32,
    pub kills: f32,
    pub inside: f32,
    pub circling: f32,
}

pub fn evaluate(
    policy: &crate::brain::conv::Policy,
    seeds: u64,
    secs: f32,
    effort: crate::brain::Effort,
    self_play: bool,
) -> Report {
    let runs: Vec<Run> = std::thread::scope(|s| {
        let handles: Vec<_> = (0..seeds)
            .map(|seed| s.spawn(move || evaluate_one(policy, seed, secs, effort, self_play)))
            .collect();
        handles.into_iter().map(|h| h.join().unwrap()).collect()
    });
    let n = seeds.max(1) as f32;
    let sum = |f: fn(&Run) -> f32| runs.iter().map(f).sum::<f32>();
    let alive = sum(|r| r.4).max(1.0);
    let inside = sum(|r| r.3);
    Report {
        peak: sum(|r| r.0) / n,
        deaths: sum(|r| r.1) / n,
        kills: sum(|r| r.2) / n,
        inside: inside / alive,
        circling: sum(|r| r.5) / inside.max(1.0),
    }
}

fn evaluate_one(
    policy: &crate::brain::conv::Policy,
    seed: u64,
    secs: f32,
    effort: crate::brain::Effort,
    self_play: bool,
) -> Run {
    let (mut deaths, mut kills, mut inside, mut alive) = (0.0f32, 0.0f32, 0.0f32, 0.0f32);
    let (mut turned, mut circling) = (0.0f32, 0.0f32);
    rand::srand(9_000_000 + seed);
    let mut w = World::new(1.0);
    w.brains = false;
    for s in super::world::spread_spots(BOTS) {
        w.add_player(s, rand::gen_range(-PI, PI), true);
    }
    let mine: Vec<usize> = if self_play {
        (0..BOTS).collect()
    } else {
        vec![0]
    };
    for &id in &mine {
        w.players[id].manual = true;
    }
    let mut dead_for = [0.0f32; BOTS];
    let mut best = 0.0f32;
    let mut last = w.players[0].angle;
    let ticks = (DECIDE / SUB).round() as usize;
    for _ in 0..(secs / DECIDE) as usize {
        let coarse = Coarse::of(&w);
        for &id in &mine {
            if w.players[id].alive {
                let a = policy.choose(&observe(&w, id, &coarse), effort);
                let p = &mut w.players[id];
                p.want = wrap_angle(p.angle + TURNS[a]);
            }
        }
        for _ in 0..ticks {
            w.step_with(&mut |_, _| None);
            for d in std::mem::take(&mut w.deaths) {
                if d.victim == 0 {
                    deaths += 1.0;
                }
                if d.killer == Some(0) {
                    kills += 1.0;
                }
            }
            w.claims.clear();
        }
        w.respawn_bots(DECIDE);
        for &id in &mine {
            if !w.players[id].alive {
                dead_for[id] += DECIDE;
                if dead_for[id] >= RESPAWN {
                    if let Some(at) = w.free_spot(40) {
                        w.respawn(id, at);
                        w.players[id].manual = true;
                        dead_for[id] = 0.0;
                    }
                }
            }
        }
        let p = &w.players[0];
        if p.alive {
            alive += 1.0;
            best = best.max(w.share(0));
            if !p.out() && w.owner_at(p.pos) == Some(0) {
                inside += 1.0;
                turned += wrap_angle(p.angle - last).abs();
                if turned > std::f32::consts::TAU * 0.75 {
                    circling += 1.0;
                }
            } else {
                turned = 0.0;
            }
            last = p.angle;
        }
    }
    (best, deaths, kills, inside, alive, circling)
}

pub fn baseline(seeds: u64, secs: f32) -> (f32, f32, f32, f32) {
    let runs: Vec<(f32, f32, f32, f32)> = std::thread::scope(|s| {
        let handles: Vec<_> = (0..seeds)
            .map(|seed| {
                s.spawn(move || {
                    let (mut deaths, mut kills, mut inside, mut alive) =
                        (0.0f32, 0.0f32, 0.0f32, 0.0f32);
                    rand::srand(9_000_000 + seed);
                    let mut w = World::new(1.0);
                    w.brains = false;
                    for s in super::world::spread_spots(BOTS) {
                        w.add_player(s, rand::gen_range(-PI, PI), true);
                    }
                    let mut best = 0.0f32;
                    let ticks = (DECIDE / SUB).round() as usize;
                    for _ in 0..(secs / DECIDE) as usize {
                        for _ in 0..ticks {
                            w.step_with(&mut |_, _| None);
                            for d in std::mem::take(&mut w.deaths) {
                                if d.victim == 0 {
                                    deaths += 1.0;
                                }
                                if d.killer == Some(0) {
                                    kills += 1.0;
                                }
                            }
                            w.claims.clear();
                        }
                        w.respawn_bots(DECIDE);
                        let p = &w.players[0];
                        if p.alive {
                            alive += 1.0;
                            best = best.max(w.share(0));
                            if !p.out() && w.owner_at(p.pos) == Some(0) {
                                inside += 1.0;
                            }
                        }
                    }
                    (best, deaths, kills, inside / alive.max(1.0))
                })
            })
            .collect();
        handles.into_iter().map(|h| h.join().unwrap()).collect()
    });
    let n = seeds.max(1) as f32;
    (
        runs.iter().map(|r| r.0).sum::<f32>() / n,
        runs.iter().map(|r| r.1).sum::<f32>() / n,
        runs.iter().map(|r| r.2).sum::<f32>() / n,
        runs.iter().map(|r| r.3).sum::<f32>() / n,
    )
}

impl crate::brain::learn::Learner for Arena {
    fn spec() -> crate::brain::learn::Spec {
        crate::brain::learn::Spec {
            channels: CHANNELS,
            side: VIEW,
            scalars: SCALARS,
            actions: ACTIONS,
        }
    }

    fn agents(&self) -> usize {
        BOTS
    }

    fn obs(&self, agent: usize) -> &Obs {
        &self.obs[agent]
    }

    fn step(&mut self, actions: &[usize], out: &mut [crate::brain::learn::Tick]) {
        let s = Arena::step(self, actions);
        for (o, (r, d)) in out.iter_mut().zip(s.reward.iter().zip(&s.done)) {
            *o = crate::brain::learn::Tick {
                reward: *r,
                done: *d,
            };
        }
    }

    fn drain_episodes(&mut self) -> Vec<(f32, f32)> {
        let r = std::mem::take(&mut self.finished);
        let p = std::mem::take(&mut self.peaks);
        r.into_iter().zip(p).collect()
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
        let mut a = Arena::new(11, 1.0, 600.0);
        let mut worst = 0usize;
        for k in 0..120 {
            let acts: Vec<usize> = (0..BOTS).map(|i| (k + i * 3) % ACTIONS).collect();
            a.step(&acts);
            if k % 10 != 9 {
                continue;
            }
            for id in 0..BOTS {
                let fast = observe(&a.w, id, &Coarse::of(&a.w));
                let slow = observe_reference(&a.w, id);
                let local = LOCAL.min(6) * VIEW * VIEW;
                let diff = (0..local)
                    .filter(|i| i / (VIEW * VIEW) != 4)
                    .filter(|i| fast.grid[*i] != slow[*i])
                    .count();
                worst = worst.max(diff);
            }
        }
        assert!(
            worst <= 4,
            "{worst} of {} local cells differ",
            5 * VIEW * VIEW
        );
    }

    #[test]
    fn the_view_turns_with_the_snake_and_shows_its_own_land_behind() {
        let a = Arena::new(3, 0.5, 30.0);
        let o = &a.obs[0];
        assert_eq!(o.grid.len(), grid_len());
        assert_eq!(o.scalars.len(), SCALARS);
        let own = |o: &Obs| -> f32 { o.grid[..VIEW * VIEW].iter().sum() };
        let map_own: f32 = o.grid[LOCAL * VIEW * VIEW..(LOCAL + 1) * VIEW * VIEW]
            .iter()
            .sum();
        assert!(map_own > 0.0, "own land on the map summary");
        assert!(own(o) > 10.0, "own land near the start");
        let centre = (VIEW / 2) * VIEW + VIEW / 2;
        assert_eq!(o.grid[centre], 1.0, "standing on own land");
    }

    #[test]
    fn driving_straight_out_earns_land_and_dying_ends_the_episode() {
        let mut a = Arena::new(5, 0.5, 60.0);
        let mut total = 0.0;
        let mut ended = false;
        for k in 0..300 {
            let act: Vec<usize> = (0..BOTS)
                .map(|i| if i == 0 && (k / 25) % 2 == 1 { 0 } else { 2 })
                .collect();
            let s = a.step(&act);
            total += s.reward[0];
            ended |= s.done[0];
        }
        assert!(total.abs() > 0.01, "rewards flow: {total}");
        assert!(ended || a.finished.is_empty());
    }

    #[test]
    fn an_episode_resets_after_its_time_runs_out() {
        let mut a = Arena::new(7, 0.5, 2.0);
        let mut done = false;
        for _ in 0..20 {
            let s = a.step(&[2; BOTS]);
            if s.done.iter().all(|d| *d) {
                done = true;
                break;
            }
        }
        assert!(done);
        assert_eq!(a.steps, 0);
        assert!(a.finished.len() >= BOTS);
    }
}
