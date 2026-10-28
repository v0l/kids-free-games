use super::*;

pub const OPTIONS: usize = 12;
pub const FEATURES: usize = 15;
const FAR: f32 = 7.0;
const SPAN: f32 = 20.0;
const CONE: f32 = 0.6;
const HOME_SPAN: f32 = 30.0;
const TRAIL_SPAN: f32 = 10.0;

struct Ray {
    clear: f32,
    home: bool,
    cut: Option<f32>,
    gain: f32,
    gentle: bool,
}

impl World {
    fn ray(&self, id: usize, aim: f32) -> Ray {
        let p = &self.players[id];
        let me = Some(id as u8);
        let mut a = p.angle;
        let mut pos = p.pos;
        let mut odo = p.odo;
        let dt = PROBE_STEP / p.speed.max(0.5);
        let steps = (FAR / PROBE_STEP) as usize;
        let mut ray = Ray {
            clear: FAR,
            home: false,
            cut: None,
            gain: 0.0,
            gentle: false,
        };
        let mut open = 0.0;
        let mut fatal = false;
        for k in 1..=steps {
            let limit = p.turn * dt;
            a += wrap_angle(aim - a).clamp(-limit, limit);
            pos += dir_of(a) * PROBE_STEP;
            odo += PROBE_STEP;
            let gone = (k - 1) as f32 * PROBE_STEP;
            if !inside(pos, WALL + 0.3) {
                if !fatal {
                    ray.clear = gone;
                }
                break;
            }
            let i = idx(cell_at(pos));
            if self.owner[i] != me {
                open += 1.0;
            }
            if p.out() && self.owner[i] == me {
                ray.home = true;
                break;
            }
            match self.trail[i] {
                Some(o) if Some(o) == me => {
                    if self.laid[i] < odo - SELF_GAP && !fatal {
                        fatal = true;
                        ray.clear = gone;
                    }
                }
                Some(o) if self.gentle_to(id, o as usize) => {
                    ray.gentle = true;
                    if !fatal {
                        fatal = true;
                        ray.clear = gone;
                    }
                }
                Some(_) if ray.cut.is_none() && !fatal => ray.cut = Some(gone),
                Some(_) => {}
                None => {}
            }
            if k <= 8 && !fatal {
                for (o, q) in self.players.iter().enumerate() {
                    if o == id || !q.alive || q.pos.distance(pos) > 1.4 {
                        continue;
                    }
                    if self.gentle_to(id, o) || q.odo >= p.odo {
                        ray.gentle |= self.gentle_to(id, o);
                        fatal = true;
                        ray.clear = gone;
                        break;
                    }
                }
            }
        }
        ray.gain = open / steps as f32;
        ray
    }

    pub(super) fn hits_gentle(&self, id: usize, aim: f32) -> bool {
        self.ray(id, aim).gentle
    }

    fn head_in_cone(&self, id: usize, aim: f32) -> (f32, f32) {
        let p = &self.players[id];
        let mut best = (SPAN, 0.0);
        for (o, q) in self.players.iter().enumerate() {
            if o == id || !q.alive {
                continue;
            }
            let rel = q.pos - p.pos;
            let d = rel.length();
            if d >= best.0 || wrap_angle(angle_of(rel) - aim).abs() > CONE {
                continue;
            }
            best = (d, if q.odo > p.odo { 1.0 } else { -1.0 });
        }
        (1.0 - best.0 / SPAN, best.1)
    }

    fn trail_threat(&self, id: usize) -> f32 {
        let p = &self.players[id];
        if p.trail.is_empty() {
            return 0.0;
        }
        let mut best = TRAIL_SPAN;
        for (o, q) in self.players.iter().enumerate() {
            if o == id || !q.alive {
                continue;
            }
            for t in p.trail.iter().step_by(4) {
                best = best.min(t.distance(q.pos));
            }
        }
        1.0 - best / TRAIL_SPAN
    }

    pub fn options(&self, id: usize) -> (Vec<f32>, Vec<Vec<f32>>) {
        let p = &self.players[id];
        let out = p.out();
        let reach = p.tune.reach.max(1) as f32;
        let home = if out { self.way_home(id) } else { None };
        let home_dist = match home {
            Some((_, d)) => (d / HOME_SPAN).min(1.0),
            None => {
                if out {
                    1.0
                } else {
                    0.0
                }
            }
        };
        let nearest = (1.0 - self.nearest_head(id, p.pos) / SPAN).max(0.0);
        let threat = self.trail_threat(id);
        let land = (self.share(id) * 10.0).min(1.0);
        let mut aims = Vec::with_capacity(OPTIONS);
        let mut feats = Vec::with_capacity(OPTIONS);
        for k in 0..OPTIONS {
            let off = wrap_angle(k as f32 / OPTIONS as f32 * TAU);
            let aim = wrap_angle(p.angle + off);
            let ray = self.ray(id, aim);
            let (head, longer) = self.head_in_cone(id, aim);
            let toward = home.map_or(0.0, |(a, _)| (aim - a).cos());
            feats.push(vec![
                off / PI,
                off.abs() / PI,
                ray.clear / FAR * 2.0 - 1.0,
                if ray.home { 1.0 } else { 0.0 },
                ray.cut.map_or(0.0, |d| 1.0 - d / FAR),
                ray.gain * 2.0 - 1.0,
                head,
                longer,
                if out { 1.0 } else { 0.0 },
                (p.odo / reach).min(2.0) - 0.5,
                home_dist,
                toward,
                nearest,
                threat,
                land,
            ]);
            aims.push(aim);
        }
        (aims, feats)
    }
}
