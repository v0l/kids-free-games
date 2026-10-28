use super::world::{idx, in_bounds, turn_left, turn_right};
use super::Paper;
use crate::engine::Dir;
use glam::{ivec2, IVec2};

pub struct Pilot {
    leg: usize,
    wary: bool,
    turns: u8,
    steps: usize,
    last: IVec2,
}

impl Pilot {
    pub fn new(leg: usize, wary: bool) -> Self {
        Self {
            leg,
            wary,
            turns: 0,
            steps: 0,
            last: ivec2(-1, -1),
        }
    }

    pub fn decide(&mut self, g: &Paper) -> Option<Dir> {
        let p = g.me();
        if p.cell == self.last {
            return None;
        }
        self.last = p.cell;
        let pick = self.plan(g);
        let d = pick.unwrap_or(p.dir);
        let n = p.cell + d.delta();
        let blocked = !in_bounds(n) || g.w.trail[idx(n)] == Some(0);
        if blocked {
            return g.w.way_home(0).map(|(d, _)| d);
        }
        pick
    }

    fn plan(&mut self, g: &Paper) -> Option<Dir> {
        let p = g.me();
        let dir = p.dir;
        if p.trail.is_empty() {
            self.turns = 0;
            self.steps = 0;
            return [dir, turn_left(dir), turn_right(dir)]
                .into_iter()
                .max_by_key(|d| g.w.open_ahead(0, *d));
        }
        self.steps += 1;
        let home = g.w.way_home(0).map(|(d, _)| d);
        let danger = self.wary && g.w.nearest_head(0, p.pos()) < 7.0;
        if self.turns >= 3 || danger {
            return home;
        }
        let leg = if self.turns == 1 {
            self.leg * 2 / 3
        } else {
            self.leg
        };
        if self.steps >= leg {
            self.steps = 0;
            self.turns += 1;
            return Some(turn_right(dir));
        }
        None
    }
}
