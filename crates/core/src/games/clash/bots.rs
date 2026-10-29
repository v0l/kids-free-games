use super::sim::{turn_for, World};
use crate::rng as rand;
use glam::{vec2, Vec2};

#[derive(Clone, Debug)]
pub struct Brain {
    pub skill: f32,
    pub aggression: f32,
    pub timer: f32,
    pub target_egg: Option<Vec2>,
    pub retarget: f32,
    pub hunting: bool,
    pub roam: Option<Vec2>,
    pub roam_t: f32,
}

impl Default for Brain {
    fn default() -> Self {
        Self::new(0.5, 0.5)
    }
}

impl Brain {
    pub fn new(skill: f32, aggression: f32) -> Self {
        Self {
            skill,
            aggression,
            timer: rand::gen_range(0.0, 0.2),
            target_egg: None,
            retarget: 0.0,
            hunting: rand::gen_range(0.0, 1.0) < aggression,
            roam: None,
            roam_t: 0.0,
        }
    }
}

fn angle_of(v: Vec2) -> f32 {
    v.y.atan2(v.x)
}

impl World {
    pub fn think(&self, i: usize, brain: &mut Brain) -> f32 {
        let me = &self.snakes[i];
        let head = me.head();
        let level = me.level();
        let radius = me.radius();
        let heading = me.dir();
        let mut steer = Vec2::ZERO;

        let sight = 150.0 + 260.0 * brain.skill + radius * 3.0;
        let margin = 170.0 + radius * 2.0;
        let inside = |p: Vec2| {
            p.x > margin && p.y > margin && p.x < self.size - margin && p.y < self.size - margin
        };
        let mut danger = false;
        for other in self.snakes.iter().filter(|o| o.alive && o.id != me.id) {
            let lvl = other.level();
            if lvl > level {
                let close = other
                    .segs
                    .iter()
                    .enumerate()
                    .step_by(2)
                    .map(|(k, p)| ((*p - head).length() - other.seg_radius(k), *p))
                    .min_by(|a, b| a.0.total_cmp(&b.0));
                if let Some((d, p)) = close {
                    if d < sight {
                        danger = true;
                        let away = (head - p).normalize_or_zero();
                        let w = ((sight - d) / sight).powi(2) * 5.0;
                        steer += away * w;
                        let ahead = head + heading * (radius * 2.0 + 90.0 * brain.skill);
                        if (ahead - p).length() < other.radius() + radius * 2.0 {
                            steer += away * 3.0;
                        }
                        let toward = (other.head() - head).normalize_or_zero();
                        let side = vec2(-toward.y, toward.x);
                        let slide = if side.dot(heading) >= 0.0 {
                            side
                        } else {
                            -side
                        };
                        steer += slide * w * 0.6;
                    }
                }
            } else if lvl == level {
                let d = (other.head() - head).length();
                if d < radius * 5.0 {
                    steer += (head - other.head()).normalize_or_zero() * 1.5;
                }
            }
        }

        if danger {
            steer += heading * 1.2;
        } else {
            let mut chasing = false;
            if brain.hunting {
                let prey = self
                    .snakes
                    .iter()
                    .filter(|o| o.alive && o.id != me.id && o.level() < level && o.shield <= 0.0)
                    .map(|o| (o, (o.head() - head).length()))
                    .filter(|(_, d)| me.player || *d < 520.0 + 500.0 * brain.skill)
                    .min_by(|a, b| a.1.total_cmp(&b.1));
                if let Some((o, d)) = prey {
                    let lead = o.dir() * (d * 0.45 * brain.skill) * (o.speed() / me.speed());
                    let cut = (o.segs[(o.segs.len() / 4).min(o.segs.len() - 1)] - head)
                        .normalize_or_zero()
                        * (1.0 - brain.skill);
                    let aim = (o.head() + lead - head).normalize_or_zero() + cut;
                    steer += aim.normalize_or_zero() * 2.6;
                    chasing = true;
                }
            }
            brain.retarget -= 0.15;
            brain.roam_t -= 0.15;
            let edge = 260.0 + radius * 2.0;
            let roam_done = brain
                .roam
                .is_none_or(|p| (p - head).length() < 200.0 || brain.roam_t <= 0.0);
            if roam_done {
                brain.roam_t = rand::gen_range(6.0, 12.0);
                let pick = || {
                    vec2(
                        rand::gen_range(edge, self.size - edge),
                        rand::gen_range(edge, self.size - edge),
                    )
                };
                let mut best = pick();
                for _ in 0..4 {
                    let p = pick();
                    if (p - head).normalize_or_zero().dot(heading)
                        > (best - head).normalize_or_zero().dot(heading)
                    {
                        best = p;
                    }
                }
                brain.roam = Some(best);
            }
            let roam = brain.roam.unwrap_or(head);
            let to_roam = (roam - head).normalize_or_zero();
            let turn_r = me.speed() / turn_for(level);
            let left = vec2(-heading.y, heading.x);
            let grab = radius + 8.0 + 55.0 + radius * 1.6;
            let reachable = |p: Vec2| {
                (p - (head + left * turn_r)).length() > turn_r * 1.15
                    && (p - (head - left * turn_r)).length() > turn_r * 1.15
            };
            let stale = brain.target_egg.is_none_or(|p| {
                (p - head).length() < grab || !reachable(p) || !self.eggs.iter().any(|e| e.pos == p)
            });
            if stale || brain.retarget <= 0.0 {
                brain.retarget = 1.5;
                brain.target_egg = self
                    .eggs
                    .iter()
                    .filter(|e| {
                        let v = e.pos - head;
                        let len = v.length();
                        len > grab
                            && len < 420.0
                            && v.normalize_or_zero().dot(heading) > 0.35
                            && reachable(e.pos)
                            && inside(e.pos)
                    })
                    .map(|e| {
                        let v = e.pos - head;
                        let off = 1.6 - v.normalize_or_zero().dot(to_roam);
                        (e.pos, v.length() * off / e.value as f32)
                    })
                    .min_by(|a, b| a.1.total_cmp(&b.1))
                    .map(|(p, _)| p);
            }
            let notice = 180.0 + 320.0 * brain.skill;
            let power = self
                .powers
                .iter()
                .map(|p| (p.pos, (p.pos - head).length()))
                .filter(|(p, d)| *d < notice && reachable(*p))
                .min_by(|a, b| a.1.total_cmp(&b.1));
            if let Some((p, _)) = power.filter(|_| !chasing) {
                steer += (p - head).normalize_or_zero() * 2.4;
            } else if !chasing {
                steer += to_roam * 0.7;
                if let Some(p) = brain.target_egg {
                    steer += (p - head).normalize_or_zero() * 1.6;
                }
            }
        }

        let (lo, hi) = (margin, self.size - margin);
        if head.x < lo {
            steer.x += (lo - head.x) / margin * 6.0;
        }
        if head.x > hi {
            steer.x -= (head.x - hi) / margin * 6.0;
        }
        if head.y < lo {
            steer.y += (lo - head.y) / margin * 6.0;
        }
        if head.y > hi {
            steer.y -= (head.y - hi) / margin * 6.0;
        }
        for r in &self.rocks {
            let d = head - r.pos;
            let reach = r.r + radius + 160.0;
            if d.length() < reach {
                let side = vec2(-d.y, d.x).normalize_or_zero();
                let side = if side.dot(heading) >= 0.0 {
                    side
                } else {
                    -side
                };
                steer += (d.normalize_or_zero() * 2.0 + side * 3.0)
                    * (1.0 - d.length() / reach).max(0.1)
                    * 3.0;
            }
        }
        if steer.length_squared() < 1e-4 {
            return me.angle;
        }
        let noise = (1.0 - brain.skill) * 0.5;
        angle_of(steer) + rand::gen_range(-noise, noise)
    }
}
