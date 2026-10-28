use super::world::{dir_of, wrap_angle};
use super::Paper;
use glam::Vec2;
use std::f32::consts::FRAC_PI_2;

pub struct Pilot {
    leg: f32,
    wary: bool,
    turns: u8,
    from: Vec2,
    heading: f32,
    was_out: bool,
}

impl Pilot {
    pub fn new(leg: usize, wary: bool) -> Self {
        Self {
            leg: leg as f32,
            wary,
            turns: 0,
            from: Vec2::ZERO,
            heading: 0.0,
            was_out: false,
        }
    }

    pub fn decide(&mut self, g: &Paper) -> Option<f32> {
        let p = g.me();
        if !p.alive {
            return None;
        }
        if !p.out() {
            self.was_out = false;
            self.turns = 0;
            let best = (-3..=3)
                .map(|k| wrap_angle(p.angle + k as f32 * 0.45))
                .max_by_key(|a| g.w.open_ahead(0, *a))
                .unwrap_or(p.angle);
            return Some(best);
        }
        if !self.was_out {
            self.was_out = true;
            self.from = p.pos;
            self.heading = p.angle;
        }
        let home = g.w.way_home(0).map(|(a, _)| a);
        let danger = self.wary && g.w.nearest_head(0, p.pos) < 7.0;
        if self.turns >= 3 || danger {
            return home.or(Some(p.want));
        }
        let leg = if self.turns == 1 {
            self.leg * 2.0 / 3.0
        } else {
            self.leg
        };
        if (p.pos - self.from).dot(dir_of(self.heading)) >= leg {
            self.turns += 1;
            self.from = p.pos;
            self.heading = wrap_angle(self.heading + FRAC_PI_2);
        }
        Some(self.heading)
    }
}
