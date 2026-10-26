use crate::engine::{Dir, Input, View};
use crate::games::crossy::{self, Crossy, Lane};
use crate::games::flappy::{self, Flappy};
use serde_json::{json, Value};

pub const MODEL: &str = "openrouter/typesafe/jev-1.13";
pub const URL: &str = "https://yalr.v0l.io/v1/systemone";
pub const MAX_LEAD: f32 = 0.32;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StateMode {
    Described,
    Raw,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GameKind {
    Flappy,
    Crossy,
}

impl GameKind {
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "flappy" => Some(Self::Flappy),
            "crossy" => Some(Self::Crossy),
            _ => None,
        }
    }

    pub fn key(self) -> &'static str {
        match self {
            Self::Flappy => flappy::GAME,
            Self::Crossy => crossy::GAME,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Moment {
    pub state: Value,
    pub instructions: &'static str,
    pub options: Vec<(&'static str, &'static str)>,
}

impl Moment {
    pub fn body(&self) -> Value {
        let criteria: serde_json::Map<String, Value> = self
            .options
            .iter()
            .map(|(k, v)| (k.to_string(), json!(v)))
            .collect();
        json!({
            "model": MODEL,
            "state": self.state,
            "questions": {"act": {"type": "choice", "instructions": self.instructions, "criteria": criteria}},
        })
    }
}

pub fn parse_choice(body: &str) -> Option<(String, f32)> {
    let v: Value = serde_json::from_str(body).ok()?;
    let a = &v["answers"]["act"];
    let choice = a["choice"].as_str()?.to_string();
    let conf = a["confidence"].as_f64().unwrap_or(0.0) as f32;
    Some((choice, conf))
}

const FLAPPY_HORIZON: f32 = 1.3;
const FLAPPY_DT: f32 = 1.0 / 120.0;

#[derive(Clone)]
struct Sim {
    y: f32,
    vy: f32,
    t: f32,
    pipes: Vec<(f32, f32, f32)>,
    passed: Option<f32>,
}

enum End {
    Dead(&'static str, f32),
    Alive(Option<f32>),
}

impl Sim {
    fn run(&mut self, secs: f32, speed: f32, bx: f32, ground: f32) -> Option<&'static str> {
        let stop = self.t + secs;
        while self.t < stop {
            self.vy += flappy::GRAVITY * FLAPPY_DT;
            self.y += self.vy * FLAPPY_DT;
            if self.y < flappy::BIRD_R {
                self.y = flappy::BIRD_R;
                self.vy = 0.0;
            }
            self.t += FLAPPY_DT;
            if self.y + flappy::BIRD_R >= ground {
                return Some("hits the ground");
            }
            let r = flappy::BIRD_R - 5.0;
            for (x, gap_y, gap) in &mut self.pipes {
                *x -= speed * FLAPPY_DT;
                let over = bx + r > *x && bx - r < *x + flappy::PIPE_W;
                if over && self.y - r < *gap_y - *gap / 2.0 {
                    return Some("hits the top pipe");
                }
                if over && self.y + r > *gap_y + *gap / 2.0 {
                    return Some("hits the bottom pipe");
                }
                if over && self.passed.is_none() {
                    self.passed = Some((self.y - *gap_y) / (*gap / 2.0));
                }
            }
        }
        None
    }
}

fn best_future(sim: Sim, period: f32, speed: f32, bx: f32, ground: f32) -> End {
    if sim.t >= FLAPPY_HORIZON {
        return End::Alive(sim.passed);
    }
    let mut worst: Option<(&'static str, f32)> = None;
    let mut best_alive: Option<Option<f32>> = None;
    for flap in [false, true] {
        let mut s = sim.clone();
        if flap {
            s.vy = flappy::FLAP;
        }
        match s.run(period, speed, bx, ground) {
            Some(why) => {
                if worst.is_none_or(|(_, t)| s.t > t) {
                    worst = Some((why, s.t));
                }
            }
            None => match best_future(s, period, speed, bx, ground) {
                End::Alive(p) => {
                    let better = match (best_alive, p) {
                        (None, _) => true,
                        (Some(Some(a)), Some(b)) => b.abs() < a.abs(),
                        (Some(None), Some(_)) => true,
                        _ => false,
                    };
                    if better {
                        best_alive = Some(p);
                    }
                }
                End::Dead(why, t) => {
                    if worst.is_none_or(|(_, w)| t > w) {
                        worst = Some((why, t));
                    }
                }
            },
        }
    }
    match (best_alive, worst) {
        (Some(p), _) => End::Alive(p),
        (None, Some((why, t))) => End::Dead(why, t),
        (None, None) => End::Alive(None),
    }
}

pub fn flappy_outcome(g: &Flappy, view: &View, flap: bool) -> String {
    flappy_outcome_after(g, view, flap, 0.0)
}

pub fn flappy_outcome_after(g: &Flappy, view: &View, flap: bool, lead: f32) -> String {
    let ground = view.h - flappy::GROUND;
    let bx = view.w * 0.3;
    let mut sim = Sim {
        y: g.y,
        vy: g.vy,
        t: 0.0,
        pipes: g.pipes.iter().map(|p| (p.x, p.gap_y, p.gap)).collect(),
        passed: None,
    };
    if let Some(why) = sim.run(lead, g.speed, bx, ground) {
        return why.into();
    }
    if flap {
        sim.vy = flappy::FLAP;
    }
    let period = (lead + 0.05).max(0.2);
    if let Some(why) = sim.run(period, g.speed, bx, ground) {
        return why.into();
    }
    let target = sim
        .pipes
        .iter()
        .find(|(x, _, _)| *x + flappy::PIPE_W > bx - flappy::BIRD_R)
        .map_or(ground * 0.5, |p| p.1);
    let height = match sim.y - target {
        d if d < -110.0 => "far above the next gap",
        d if d < -40.0 => "a little above the next gap",
        d if d > 110.0 => "far below the next gap",
        d if d > 40.0 => "a little below the next gap",
        _ => "level with the next gap",
    };
    match best_future(sim, period, g.speed, bx, ground) {
        End::Dead(why, _) => format!("{why} soon"),
        End::Alive(_) => format!("safe, and the bird ends up {height}"),
    }
}

pub fn flappy_moment(g: &Flappy, view: &View) -> Moment {
    flappy_moment_after(g, view, 0.0)
}

pub fn flappy_moment_after(g: &Flappy, view: &View, lead: f32) -> Moment {
    Moment {
        state: json!({
            "if_flap_now": flappy_outcome_after(g, view, true, lead),
            "if_wait": flappy_outcome_after(g, view, false, lead),
        }),
        instructions: "Keep the bird alive. Never pick an action that hits a pipe or the ground. If both are safe, pick the one that leaves the bird closest to level with the next gap.",
        options: vec![("flap", "flap now"), ("wait", "do not flap")],
    }
}

fn crossy_hit_if(g: &Crossy, col: f32, row: i32, lead: f32) -> bool {
    let Some(r) = g.rows.get(row.max(0) as usize) else {
        return false;
    };
    let mut r = r.clone();
    let mut t = 0.0;
    while t < lead + crossy::HOP_TIME + 0.45 {
        r.step(1.0 / 60.0);
        if t >= lead && r.hits(col) {
            return true;
        }
        t += 1.0 / 60.0;
    }
    match r.lane {
        Lane::River { .. } => r.log_under(col).is_none(),
        _ => false,
    }
}

fn crossy_option(g: &Crossy, d: Dir, lead: f32) -> String {
    let delta = d.delta();
    let col = (g.x.round() as i32 + delta.x).clamp(0, crossy::COLS - 1);
    let row = g.row - delta.y;
    if g.blocked(col, row) {
        return "blocked by a tree".into();
    }
    let lane = g.rows.get(row.max(0) as usize).map(|r| r.lane);
    let what = match lane {
        Some(Lane::Road { .. }) => "a road",
        Some(Lane::River { .. }) => "the river",
        Some(Lane::Rail { .. }) => "train tracks",
        _ => "grass",
    };
    if crossy_hit_if(g, col as f32, row, lead) {
        match lane {
            Some(Lane::River { .. }) => format!("lands in {what} and falls in the water"),
            _ => format!("lands on {what} and gets hit"),
        }
    } else {
        format!("lands safely on {what}")
    }
}

pub fn crossy_moment(g: &Crossy) -> Moment {
    crossy_moment_after(g, 0.0)
}

pub fn crossy_moment_after(g: &Crossy, lead: f32) -> Moment {
    let here_safe = !crossy_hit_if(g, g.x, g.row, 0.0);
    Moment {
        state: json!({
            "if_hop_forward": crossy_option(g, Dir::Up, lead),
            "if_hop_left": crossy_option(g, Dir::Left, lead),
            "if_hop_right": crossy_option(g, Dir::Right, lead),
            "if_wait": if here_safe { "stays safe where it is" } else { "gets hit where it stands" },
        }),
        instructions: "The goal is to cross as many rows as possible without getting hit. Hop forward whenever it lands safely. If forward is not safe, wait or step sideways to a safe spot.",
        options: vec![
            ("up", "hop forward one row"),
            ("left", "hop left"),
            ("right", "hop right"),
            ("wait", "stay still"),
        ],
    }
}

fn r1(v: f32) -> f64 {
    (v as f64 * 10.0).round() / 10.0
}

pub fn flappy_raw(g: &Flappy, view: &View, lead: f32) -> Moment {
    let ground = view.h - flappy::GROUND;
    let bx = view.w * 0.3;
    let pipes: Vec<Value> = g
        .pipes
        .iter()
        .filter(|p| p.x + flappy::PIPE_W > bx - flappy::BIRD_R - 10.0)
        .take(2)
        .map(|p| {
            json!({
                "distance_ahead": (p.x - bx).round(),
                "gap_top_y": (p.gap_y - p.gap / 2.0).round(),
                "gap_bottom_y": (p.gap_y + p.gap / 2.0).round(),
                "width": flappy::PIPE_W,
            })
        })
        .collect();
    Moment {
        state: json!({
            "units": "pixels, y grows downward, time in seconds",
            "bird": {"y": g.y.round(), "velocity_y": g.vy.round(), "radius": flappy::BIRD_R},
            "ground_y": ground.round(),
            "ceiling_y": 0,
            "physics": {"gravity": flappy::GRAVITY, "flap_sets_velocity_y_to": flappy::FLAP, "scroll_speed": g.speed.round()},
            "pipes_ahead": pipes,
            "answer_arrives_in": r1(lead),
        }),
        instructions: "Flappy bird. Keep the bird alive: it must pass between gap_top_y and gap_bottom_y of each pipe and never touch the ground. Flapping sets the vertical velocity to the flap value; otherwise gravity pulls it down. Your answer takes effect after answer_arrives_in seconds.",
        options: vec![("flap", "flap now"), ("wait", "do not flap")],
    }
}

pub fn crossy_raw(g: &Crossy, lead: f32) -> Moment {
    let col = g.x.round() as i32;
    let rows: Vec<Value> = (0..4)
        .filter_map(|ahead| {
            let r = g.rows.get((g.row + ahead) as usize)?;
            let (kind, speed) = match r.lane {
                Lane::Grass => ("grass", 0.0),
                Lane::Road { speed, .. } => ("road", speed),
                Lane::River { speed, .. } => ("river", speed),
                Lane::Rail { .. } => ("rail", 34.0),
            };
            let things: Vec<Value> = r
                .things
                .iter()
                .filter(|x| **x > -4.0 && **x < crossy::COLS as f32 + 4.0)
                .map(|x| json!({"from": r1(*x), "to": r1(*x + r.len)}))
                .collect();
            let mut row = json!({"rows_ahead": ahead, "kind": kind});
            if kind != "grass" {
                row["speed_columns_per_second"] = json!(r1(speed));
                row[if kind == "river" { "logs" } else { "vehicles" }] = json!(things);
            }
            if kind == "rail" {
                row["train_warning_light"] = json!(r.warning());
            }
            if !r.trees.is_empty() {
                row["trees_at_columns"] = json!(r.trees);
            }
            Some(row)
        })
        .collect();
    Moment {
        state: json!({
            "units": "columns 0 to 8 left to right, rows counted forward from the chicken",
            "chicken": {"column": col, "x": r1(g.x)},
            "hop_takes_seconds": r1(crossy::HOP_TIME),
            "answer_arrives_in": r1(lead),
            "rows": rows,
        }),
        instructions: "Road crossing. Cross as many rows as possible. A vehicle or train covering the chicken's column when it lands kills it; landing in the river off a log drowns it; trees block hops. Positive speed moves right. Your answer takes effect after answer_arrives_in seconds.",
        options: vec![
            ("up", "hop forward one row"),
            ("left", "hop left"),
            ("right", "hop right"),
            ("wait", "stay still"),
        ],
    }
}

pub fn input_for(choice: &str) -> Input {
    match choice {
        "flap" => Input {
            action: true,
            ..Default::default()
        },
        "up" => Input::key(Dir::Up),
        "left" => Input::key(Dir::Left),
        "right" => Input::key(Dir::Right),
        _ => Input::default(),
    }
}

#[derive(Clone, Debug, Default)]
pub struct Decision {
    pub choice: String,
    pub confidence: f32,
    pub ms: f32,
    pub state: Value,
    pub error: Option<String>,
}

pub fn decision_from(m: &Moment, ms: f32, status: i32, body: &str) -> Decision {
    match (status, parse_choice(body)) {
        (200, Some((choice, confidence))) => Decision {
            choice,
            confidence,
            ms,
            state: m.state.clone(),
            error: None,
        },
        _ => Decision {
            ms,
            state: m.state.clone(),
            error: Some(format!(
                "{status} {}",
                body.chars().take(100).collect::<String>()
            )),
            ..Default::default()
        },
    }
}

pub fn yalr_key() -> Option<String> {
    if cfg!(target_arch = "wasm32") {
        return None;
    }
    if let Ok(k) = std::env::var("YALR_KEY") {
        return Some(k);
    }
    let home = std::env::var_os("HOME")?;
    let path = std::path::Path::new(&home).join(".pi/agent/models.json");
    let v: Value = serde_json::from_str(&std::fs::read_to_string(path).ok()?).ok()?;
    v["providers"]["yalr"]["apiKey"].as_str().map(String::from)
}

pub struct Player {
    pub kind: GameKind,
    pub mode: StateMode,
    key: String,
    transport: Box<dyn crate::net::transport::Transport>,
    pending: Option<(u32, Moment, f64)>,
    pub last: Option<Decision>,
    pub calls: u32,
    pub ms_total: f32,
    pub runs: u32,
    pub best: u32,
    pub log: Vec<String>,
    since_ask: f32,
    dead_for: f32,
}

impl Player {
    pub fn new(kind: GameKind, key: String) -> Self {
        Self {
            kind,
            mode: StateMode::Described,
            key,
            transport: crate::net::transport::default_transport(),
            pending: None,
            last: None,
            calls: 0,
            ms_total: 0.0,
            runs: 0,
            best: 0,
            log: vec![],
            since_ask: 0.0,
            dead_for: 0.0,
        }
    }

    pub fn clock(&self) -> f32 {
        let avg = if self.calls == 0 {
            0.3
        } else {
            self.ms_total / self.calls as f32 / 1000.0
        };
        (MAX_LEAD / avg).clamp(0.25, 1.0)
    }

    fn lead(&self) -> f32 {
        let avg = if self.calls == 0 {
            0.3
        } else {
            self.ms_total / self.calls as f32 / 1000.0
        };
        avg * self.clock() + 1.0 / 60.0
    }

    fn launch(&mut self, m: Moment) {
        let id = self.transport.start(crate::net::transport::HttpRequest {
            method: "POST",
            path: URL.to_string(),
            token: Some(self.key.clone()),
            body: Some(m.body().to_string()),
        });
        self.pending = Some((id, m, macroquad::miniquad::date::now()));
        self.since_ask = 0.0;
    }

    fn take(&mut self) -> Option<Decision> {
        let (id, _, _) = self.pending.as_ref()?;
        let resp = self.transport.poll(*id)?;
        let (_, m, at) = self.pending.take()?;
        let ms = ((macroquad::miniquad::date::now() - at) * 1000.0) as f32;
        let d = decision_from(&m, ms, resp.status, &resp.body);
        self.calls += 1;
        self.ms_total += d.ms;
        let line = match &d.error {
            Some(e) => format!("error: {e}"),
            None => format!(
                "{} ({:.0}%, {:.0} ms)",
                d.choice,
                d.confidence * 100.0,
                d.ms
            ),
        };
        self.log.push(line);
        if self.log.len() > 6 {
            self.log.remove(0);
        }
        self.last = Some(d.clone());
        Some(d)
    }

    pub fn drive(&mut self, game: &mut dyn std::any::Any, view: &View, dt: f32) -> Input {
        self.since_ask += dt;
        let decided = self.take();
        match self.kind {
            GameKind::Flappy => {
                let g = game.downcast_mut::<Flappy>().unwrap();
                match g.state {
                    flappy::State::Ready => {
                        self.dead_for = 0.0;
                        return input_for("flap");
                    }
                    flappy::State::Dead => {
                        self.end_run(g.score);
                        return self.restart_after(dt);
                    }
                    flappy::State::Playing => {}
                }
                if self.pending.is_none() && decided.is_none() {
                    let lead = self.lead();
                    let m = match self.mode {
                        StateMode::Described => flappy_moment_after(g, view, lead),
                        StateMode::Raw => flappy_raw(g, view, lead),
                    };
                    self.launch(m);
                }
                decided.map_or_else(Input::default, |d| input_for(&d.choice))
            }
            GameKind::Crossy => {
                let g = game.downcast_mut::<Crossy>().unwrap();
                match g.state {
                    crossy::State::Ready => {
                        self.dead_for = 0.0;
                        return input_for("up");
                    }
                    crossy::State::Dead => {
                        self.end_run(g.score);
                        return self.restart_after(dt);
                    }
                    crossy::State::Playing => {}
                }
                if self.pending.is_none() && decided.is_none() && g.hop >= 1.0 {
                    let lead = self.lead();
                    let m = match self.mode {
                        StateMode::Described => crossy_moment_after(g, lead),
                        StateMode::Raw => crossy_raw(g, lead),
                    };
                    self.launch(m);
                }
                decided.map_or_else(Input::default, |d| input_for(&d.choice))
            }
        }
    }

    fn end_run(&mut self, score: u32) {
        if self.dead_for == 0.0 {
            self.runs += 1;
            self.best = self.best.max(score);
            self.pending = None;
        }
    }

    fn restart_after(&mut self, dt: f32) -> Input {
        self.dead_for += dt;
        if self.dead_for > 2.0 {
            self.dead_for = 0.0;
            input_for("flap")
        } else {
            Input::default()
        }
    }
}

impl Player {
    pub fn draw(&self, view: &View, score: u32) {
        use crate::engine::round_rect;
        let w = 380.0f32.min(view.w * 0.45);
        let lines = 6 + self.log.len();
        let r =
            macroquad::prelude::Rect::new(view.w - w - 16.0, 70.0, w, 34.0 + lines as f32 * 26.0);
        round_rect(
            r,
            18.0,
            macroquad::prelude::Color::new(0.06, 0.03, 0.15, 0.78),
        );
        let white = macroquad::prelude::WHITE;
        let dim = macroquad::prelude::Color::new(0.75, 0.78, 0.9, 1.0);
        let gold = macroquad::prelude::Color::from_rgba(255, 210, 70, 255);
        let mut y = r.y + 34.0;
        let x0 = r.x + 16.0;
        let mut line = |s: &str, c: macroquad::prelude::Color| {
            let w = crate::outline::measure(crate::engine::FONT_BYTES, s, 22.0);
            crate::fx::text_outline(
                s,
                x0 + w / 2.0,
                y - 7.0,
                22.0,
                c,
                macroquad::prelude::Color::new(0.06, 0.03, 0.15, 1.0),
            );
            y += 26.0;
        };
        line(
            match self.mode {
                StateMode::Described => "JEV 1.13 playing (described)",
                StateMode::Raw => "JEV 1.13 playing (raw state)",
            },
            gold,
        );
        let avg = if self.calls > 0 {
            self.ms_total / self.calls as f32
        } else {
            0.0
        };
        line(
            &format!(
                "calls {}   avg {:.0} ms   speed {:.0}%",
                self.calls,
                avg,
                self.clock() * 100.0
            ),
            dim,
        );
        line(
            &format!(
                "run {}   score {}   best {}",
                self.runs + 1,
                score,
                self.best
            ),
            dim,
        );
        if let Some(d) = &self.last {
            if let Some(o) = d.state.as_object() {
                let keys: Vec<&String> = o.keys().filter(|k| k.starts_with("if_") || *k == "bird" || *k == "chicken").take(2).collect();
                for k in keys {
                    let v = &o[k];
                    let shown = v.as_str().map(String::from).unwrap_or_else(|| v.to_string());
                    let k = k.trim_start_matches("if_").replace('_', " ");
                    line(&format!("{k}: {shown}"), white);
                }
            }
        } else {
            line("waiting for the first answer", white);
            line("", white);
        }
        line("", white);
        for l in &self.log {
            line(l, dim);
        }
    }
}

pub fn game_for(kind: GameKind) -> Box<dyn std::any::Any> {
    match kind {
        GameKind::Flappy => Box::new(Flappy::new()),
        GameKind::Crossy => Box::new(Crossy::new()),
    }
}

pub fn score_of(kind: GameKind, game: &dyn std::any::Any) -> u32 {
    match kind {
        GameKind::Flappy => game.downcast_ref::<Flappy>().map_or(0, |g| g.score),
        GameKind::Crossy => game.downcast_ref::<Crossy>().map_or(0, |g| g.score),
    }
}

pub fn as_game(kind: GameKind, game: &mut dyn std::any::Any) -> &mut dyn crate::engine::Game {
    match kind {
        GameKind::Flappy => game.downcast_mut::<Flappy>().unwrap(),
        GameKind::Crossy => game.downcast_mut::<Crossy>().unwrap(),
    }
}

#[cfg(target_arch = "wasm32")]
extern "C" {
    fn kg_jev_len() -> u32;
    fn kg_jev_read(ptr: *mut u8, len: u32);
}

#[cfg(target_arch = "wasm32")]
fn web_param() -> String {
    unsafe {
        let n = kg_jev_len();
        let mut b = vec![0u8; n as usize];
        if n > 0 {
            kg_jev_read(b.as_mut_ptr(), n);
        }
        String::from_utf8(b).unwrap_or_default()
    }
}

#[cfg(target_arch = "wasm32")]
pub fn arg() -> Option<(GameKind, Option<String>)> {
    let p = web_param();
    let mut parts = p.splitn(2, ',');
    let kind = GameKind::parse(parts.next()?)?;
    Some((
        kind,
        parts.next().filter(|k| !k.is_empty()).map(String::from),
    ))
}

#[cfg(not(target_arch = "wasm32"))]
pub fn arg() -> Option<(GameKind, Option<String>)> {
    native_arg().map(|k| (k, None))
}

pub fn mode_arg() -> StateMode {
    let raw = if cfg!(target_arch = "wasm32") {
        false
    } else {
        std::env::args().any(|a| a == "--raw")
    };
    if raw {
        StateMode::Raw
    } else {
        StateMode::Described
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn native_arg() -> Option<GameKind> {
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        if a == "--jev" {
            return args.next().as_deref().and_then(GameKind::parse);
        }
        if let Some(g) = a.strip_prefix("--jev=") {
            return GameKind::parse(g);
        }
    }
    None
}

pub async fn run(kind: GameKind, key: Option<String>) {
    use macroquad::prelude::*;
    crate::sfx::load().await;
    let Some(key) = key.or_else(yalr_key) else {
        loop {
            crate::begin_frame();
            clear_background(BLACK);
            crate::engine::text("No Yalr key: set YALR_KEY", 40.0, 100.0, 32.0, WHITE);
            next_frame().await;
        }
    };
    if crate::profile::current().is_none() {
        crate::profile::create("Jev", 4);
    }
    crate::profile::set(flappy::GAME, "passed", 1);
    for _ in 0..2 {
        crate::begin_frame();
        clear_background(BLACK);
        for size in [18.0, 22.0, 26.0, 32.0] {
            crate::engine::text_center(
                "BEST 0123456789 abcdefghijklmnopqrstuvwxyz",
                300.0,
                300.0,
                size,
                BLACK,
            );
        }
        next_frame().await;
    }
    let mut player = Player::new(kind, key);
    player.mode = mode_arg();
    let mut game = game_for(kind);
    loop {
        let view = crate::begin_frame();
        let dt = get_frame_time().min(1.0 / 20.0) * player.clock();
        let input = player.drive(game.as_mut(), &view, dt);
        let g = as_game(kind, game.as_mut());
        g.update(&input, &view, dt);
        g.draw(&view);
        crate::engine::set_view_camera(&view, Vec2::ZERO);
        player.draw(&view, score_of(kind, game.as_ref()));
        crate::sfx::flush(get_time(), get_frame_time().min(0.1));
        next_frame().await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::Game;

    fn fresh() {
        crate::save::reset_for_test();
        crate::profile::create("Jev", 0);
        crate::profile::set(flappy::GAME, "passed", 1);
    }

    #[test]
    fn flappy_falling_onto_the_ground_says_so() {
        fresh();
        let v = View::ipad_landscape();
        let mut g = Flappy::new();
        g.state = flappy::State::Playing;
        g.y = v.h - flappy::GROUND - 60.0;
        g.vy = 300.0;
        assert_eq!(flappy_outcome(&g, &v, false), "hits the ground");
        assert_ne!(flappy_outcome(&g, &v, true), "hits the ground");
    }

    #[test]
    fn flappy_outcomes_say_where_the_bird_ends_up() {
        fresh();
        let v = View::ipad_landscape();
        let mut g = Flappy::new();
        g.state = flappy::State::Playing;
        g.pipes.push(flappy::Pipe::new(v.w * 0.3 + 200.0, 400.0));
        g.y = 150.0;
        g.vy = 0.0;
        assert!(flappy_outcome(&g, &v, true).contains("far above"));
        g.y = 400.0;
        assert!(flappy_outcome(&g, &v, false).starts_with("safe"));
    }

    #[test]
    fn raw_states_carry_the_real_numbers() {
        fresh();
        let v = View::ipad_landscape();
        let mut g = Flappy::new();
        g.state = flappy::State::Playing;
        g.y = 321.0;
        g.vy = -150.0;
        g.pipes.push(flappy::Pipe::new(v.w * 0.3 + 200.0, 400.0));
        let m = flappy_raw(&g, &v, 0.3);
        assert_eq!(m.state["bird"]["y"], 321.0);
        assert_eq!(m.state["bird"]["velocity_y"], -150.0);
        assert_eq!(m.state["pipes_ahead"][0]["distance_ahead"], 200.0);
        assert_eq!(m.state["pipes_ahead"][0]["gap_top_y"], 400.0 - 115.0);
        let c = crossy_raw(&Crossy::new(), 0.3);
        assert_eq!(c.state["chicken"]["column"], crossy::COLS / 2);
        assert_eq!(c.state["rows"].as_array().unwrap().len(), 4);
        assert_eq!(c.state["rows"][0]["kind"], "grass");
    }

    #[test]
    fn a_choice_reply_parses() {
        let body = r#"{"answers":{"act":{"type":"choice","choice":"flap","confidence":0.9}}}"#;
        assert_eq!(parse_choice(body), Some(("flap".into(), 0.9)));
        assert_eq!(parse_choice("nope"), None);
    }

    #[test]
    fn the_request_lists_every_option() {
        fresh();
        let m = crossy_moment(&Crossy::new());
        let b = m.body();
        let c = &b["questions"]["act"]["criteria"];
        for k in ["up", "left", "right", "wait"] {
            assert!(c[k].is_string(), "{k}");
        }
        assert!(b["state"]["if_hop_forward"].is_string());
    }

    fn pick_flappy(m: &Moment) -> &'static str {
        let f = m.state["if_flap_now"].as_str().unwrap();
        let w = m.state["if_wait"].as_str().unwrap();
        let rank = |o: &str| {
            if o.contains("hits") {
                9
            } else if o.contains("level") {
                0
            } else if o.contains("a little") {
                1
            } else {
                2
            }
        };
        if rank(f) < rank(w) {
            "flap"
        } else {
            "wait"
        }
    }

    #[test]
    fn a_perfect_chooser_survives_real_time_latency() {
        fresh();
        let v = View::ipad_landscape();
        let dt = 1.0 / 60.0;
        for latency in [0.0f32, 0.15, 0.3, 0.35] {
            let mut g = Flappy::new();
            g.update(&input_for("flap"), &v, dt);
            let mut inflight: Option<(f32, &str)> = None;
            let mut t = 0.0;
            while t < 40.0 && g.state == flappy::State::Playing {
                let mut input = Input::default();
                let mut applied = false;
                if let Some((due, pick)) = inflight {
                    if t >= due {
                        input = input_for(pick);
                        inflight = None;
                        applied = true;
                    }
                }
                if inflight.is_none() && !applied {
                    let m = flappy_moment_after(&g, &v, latency);
                    inflight = Some((t + latency, pick_flappy(&m)));
                }
                g.update(&input, &v, dt);
                t += dt;
            }
            assert!(g.score >= 8, "latency {latency}: score {}", g.score);
        }
    }

    #[test]
    fn a_perfect_describer_can_play_flappy() {
        fresh();
        let v = View::ipad_landscape();
        let mut g = Flappy::new();
        g.update(&input_for("flap"), &v, 1.0 / 60.0);
        for _ in 0..60 * 40 {
            let m = flappy_moment(&g, &v);
            let pick = pick_flappy(&m);
            g.update(&input_for(pick), &v, 1.0 / 60.0);
            if g.state == flappy::State::Dead {
                break;
            }
        }
        assert!(g.score >= 8, "score {}", g.score);
    }
}
