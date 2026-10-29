use super::{parse_choice, URL};
use crate::games::clash::sim::World;
use crate::net::transport::{default_transport, HttpRequest, Transport};
use glam::{vec2, Vec2};
use serde_json::{json, Value};
use std::f32::consts::TAU;

pub const HEADINGS: [&str; 8] = [
    "east",
    "south-east",
    "south",
    "south-west",
    "west",
    "north-west",
    "north",
    "north-east",
];

pub const INSTRUCTIONS: &str = "You are a snake in an arena. Grow as big as you can by eating eggs and smaller snakes. Never go where it says DEADLY. Avoid risky bigger snakes when another way has eggs. Pick the heading to steer.";

pub fn heading_angle(k: usize) -> f32 {
    k as f32 / 8.0 * TAU
}

fn key(k: usize) -> String {
    format!("go_{}", HEADINGS[k].replace('-', "_"))
}

pub fn describe(w: &World, k: usize) -> String {
    let me = w.player();
    let head = me.head();
    let lvl = me.level();
    let r = me.radius();
    let d = vec2(heading_angle(k).cos(), heading_angle(k).sin());
    let cone = |p: Vec2, reach: f32| {
        let v = p - head;
        let len = v.length();
        len < reach && len > 1.0 && v.dot(d) / len > 0.75
    };
    let eggs: u32 = w
        .eggs
        .iter()
        .filter(|e| cone(e.pos, 450.0))
        .map(|e| e.value)
        .sum();
    let mut danger: Option<(f32, u32)> = None;
    let mut prey: Option<(f32, u32)> = None;
    let mut same: Option<f32> = None;
    for s in w.snakes.iter().filter(|s| s.alive && !s.player) {
        let near = s
            .segs
            .iter()
            .step_by(2)
            .filter(|p| cone(**p, 500.0))
            .map(|p| (*p - head).length() - r)
            .fold(f32::MAX, f32::min);
        if near == f32::MAX {
            continue;
        }
        let l = s.level();
        if l > lvl && s.shield <= 0.0 {
            if danger.is_none_or(|x| near < x.0) {
                danger = Some((near, l));
            }
        } else if l < lvl && s.shield <= 0.0 {
            let hd = (s.head() - head).length();
            if cone(s.head(), 500.0) && prey.is_none_or(|x| hd < x.0) {
                prey = Some((hd, l));
            }
        } else if same.is_none_or(|x| near < x) {
            same = Some(near);
        }
    }
    let mut wall = 0.0;
    let mut p = head;
    while wall < 600.0 && p.x > r && p.y > r && p.x < w.size - r && p.y < w.size - r {
        p += d * 20.0;
        wall += 20.0;
    }
    let mut parts = vec![];
    match danger {
        Some((dist, l)) if dist < 140.0 => parts.push(format!(
            "DEADLY: a bigger level {l} snake's body is only {:.0} px away, you would be eaten",
            dist.max(0.0)
        )),
        Some((dist, l)) => parts.push(format!("risky: a bigger level {l} snake {dist:.0} px away")),
        None => {}
    }
    if let Some((dist, l)) = prey {
        parts.push(format!(
            "a smaller level {l} snake's head {dist:.0} px away you could eat"
        ));
    }
    if let Some(dist) = same.filter(|d| *d < 150.0) {
        parts.push(format!(
            "a same-size snake {dist:.0} px away (bumps you aside)"
        ));
    }
    parts.push(match eggs {
        0 => "no eggs".to_string(),
        n => format!("{n} egg points"),
    });
    if wall < 200.0 {
        parts.push(format!("wall in {wall:.0} px"));
    }
    parts.join("; ")
}

pub fn moment(w: &World) -> Value {
    let me = w.player();
    let facing = ((me.angle.rem_euclid(TAU) / TAU * 8.0).round() as usize) % 8;
    let mut state = serde_json::Map::new();
    state.insert(
        "you".into(),
        json!(format!(
            "level {} snake, facing {}",
            me.level(),
            HEADINGS[facing]
        )),
    );
    for k in 0..HEADINGS.len() {
        state.insert(key(k), json!(describe(w, k)));
    }
    let criteria: serde_json::Map<String, Value> = HEADINGS
        .iter()
        .map(|h| (h.to_string(), json!(format!("head {h}"))))
        .collect();
    json!({
        "model": super::MODEL,
        "state": state,
        "questions": {"act": {"type": "choice", "instructions": INSTRUCTIONS, "criteria": criteria}},
    })
}

pub struct ClashPlayer {
    key: String,
    transport: Box<dyn Transport>,
    pending: Option<(u32, Value, f64)>,
    pub steer: Option<f32>,
    pub calls: u32,
    pub ms_total: f32,
    pub runs: u32,
    pub best: u32,
    pub log: Vec<String>,
    pub last_seen: String,
}

impl ClashPlayer {
    pub fn new(key: String) -> Self {
        Self {
            key,
            transport: default_transport(),
            pending: None,
            steer: None,
            calls: 0,
            ms_total: 0.0,
            runs: 0,
            best: 0,
            log: vec![],
            last_seen: String::new(),
        }
    }

    pub fn avg_ms(&self) -> f32 {
        if self.calls == 0 {
            0.0
        } else {
            self.ms_total / self.calls as f32
        }
    }

    pub fn drive(&mut self, w: &World) -> Option<f32> {
        if let Some((id, body, at)) = self.pending.take() {
            match self.transport.poll(id) {
                None => self.pending = Some((id, body, at)),
                Some(resp) => {
                    let ms = ((crate::clock::now() - at) * 1000.0) as f32;
                    self.calls += 1;
                    self.ms_total += ms;
                    let pick = (resp.status == 200)
                        .then(|| parse_choice(&resp.body))
                        .flatten()
                        .and_then(|(c, _)| HEADINGS.iter().position(|h| *h == c));
                    let line = match pick {
                        Some(k) => {
                            self.steer = Some(heading_angle(k));
                            self.last_seen = body["state"][key(k)]
                                .as_str()
                                .unwrap_or_default()
                                .to_string();
                            format!("{} ({ms:.0} ms)", HEADINGS[k])
                        }
                        None => format!("error {}", resp.status),
                    };
                    self.log.push(line);
                    if self.log.len() > 5 {
                        self.log.remove(0);
                    }
                }
            }
        }
        if self.pending.is_none() && w.player_alive() {
            let body = moment(w);
            let id = self.transport.start(HttpRequest {
                method: "POST",
                path: URL.to_string(),
                token: Some(self.key.clone()),
                body: Some(body.to_string()),
            });
            self.pending = Some((id, body, crate::clock::now()));
        }
        self.steer
    }

    pub fn end_run(&mut self, level: u32) {
        self.runs += 1;
        self.best = self.best.max(level);
        self.pending = None;
        self.steer = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::games::clash::sim::{xp_for_level, Spec};

    #[test]
    fn a_bigger_body_ahead_reads_as_deadly_and_eggs_are_counted() {
        crate::rng::srand(3);
        let mut w = World::new(
            Spec {
                bots: 0,
                eggs: 0,
                ..Spec::arena(1, true)
            },
            "Jev",
            0,
        );
        w.snakes[0].shield = 0.0;
        let head = w.player().head();
        let i = w.add_snake(head + vec2(110.0, 0.0), 9, "Big".into(), 1);
        w.snakes[i].xp = xp_for_level(9);
        w.snakes[i].shield = 0.0;
        w.snakes[i].angle = std::f32::consts::FRAC_PI_2;
        w.layout_tail(i);
        let east = describe(&w, 0);
        assert!(east.starts_with("DEADLY"), "{east}");
        let west = describe(&w, 4);
        assert!(!west.contains("DEADLY"), "{west}");
        let m = moment(&w);
        assert_eq!(
            m["questions"]["act"]["criteria"].as_object().unwrap().len(),
            8
        );
    }
}
