use crate::engine::{Input, View};
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
            let bird = glam::vec2(bx, self.y);
            for (x, gap_y, gap) in &mut self.pipes {
                *x -= speed * FLAPPY_DT;
                let top = *gap_y - *gap / 2.0;
                let bottom = *gap_y + *gap / 2.0;
                let roof = -1000.0;
                let upper = crate::geom::Rect::new(*x, roof, flappy::PIPE_W, top - roof);
                let lower = crate::geom::Rect::new(*x, bottom, flappy::PIPE_W, 2000.0);
                if crate::geom::circle_hits_rect(bird, r, upper) {
                    return Some("hits the top pipe");
                }
                if crate::geom::circle_hits_rect(bird, r, lower) {
                    return Some("hits the bottom pipe");
                }
                let over = bx + r > *x && bx - r < *x + flappy::PIPE_W;
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

pub fn input_for(choice: &str) -> Input {
    match choice {
        "flap" => Input {
            action: true,
            ..Default::default()
        },
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

pub fn mode_from(s: Option<&str>) -> StateMode {
    match s {
        Some("raw") => StateMode::Raw,
        _ => StateMode::Described,
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
    pub fn new(key: String) -> Self {
        Self {
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
        self.pending = Some((id, m, crate::clock::now()));
        self.since_ask = 0.0;
    }

    fn take(&mut self) -> Option<Decision> {
        let (id, _, _) = self.pending.as_ref()?;
        let resp = self.transport.poll(*id)?;
        let (_, m, at) = self.pending.take()?;
        let ms = ((crate::clock::now() - at) * 1000.0) as f32;
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

    pub fn drive(&mut self, g: &Flappy, view: &View, dt: f32) -> Input {
        self.since_ask += dt;
        let decided = self.take();
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

    pub fn lines(&self, score: u32) -> Vec<(String, bool)> {
        let mut out = vec![(
            match self.mode {
                StateMode::Described => "JEV 1.13 playing (described)",
                StateMode::Raw => "JEV 1.13 playing (raw state)",
            }
            .to_string(),
            true,
        )];
        let avg = if self.calls > 0 {
            self.ms_total / self.calls as f32
        } else {
            0.0
        };
        out.push((
            format!(
                "calls {}   avg {:.0} ms   speed {:.0}%",
                self.calls,
                avg,
                self.clock() * 100.0
            ),
            false,
        ));
        out.push((
            format!(
                "run {}   score {}   best {}",
                self.runs + 1,
                score,
                self.best
            ),
            false,
        ));
        match self.last.as_ref().and_then(|d| d.state.as_object()) {
            Some(o) => {
                for k in o
                    .keys()
                    .filter(|k| k.starts_with("if_") || *k == "bird")
                    .take(2)
                {
                    let v = &o[k];
                    let shown = v
                        .as_str()
                        .map(String::from)
                        .unwrap_or_else(|| v.to_string());
                    let k = k.trim_start_matches("if_").replace('_', " ");
                    out.push((format!("{k}: {shown}"), false));
                }
            }
            None => out.push(("waiting for the first answer".into(), false)),
        }
        out.extend(self.log.iter().map(|l| (l.clone(), false)));
        out
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
        let v = View::ipad_landscape();
        let mut g = Flappy::new();
        g.state = flappy::State::Playing;
        let m = flappy_moment(&g, &v);
        let b = m.body();
        let c = &b["questions"]["act"]["criteria"];
        for k in ["flap", "wait"] {
            assert!(c[k].is_string(), "{k}");
        }
        assert!(b["state"]["if_flap_now"].is_string());
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
            let mut lowest = u32::MAX;
            for seed in 0..12 {
                quad_rand::srand(seed);
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
                lowest = lowest.min(g.score);
            }
            assert!(lowest >= 8, "latency {latency}: lowest score {lowest}");
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
