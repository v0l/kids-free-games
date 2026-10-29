use super::sim::{radius_for, World};
use glam::Vec2;

pub const ARROW_REACH: f32 = 1400.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Dot {
    Bigger,
    Smaller,
    Even,
    Boss,
}

pub struct Arrow {
    pub at: Vec2,
    pub dir: Vec2,
    pub level: u32,
}

pub struct Board {
    pub rank: usize,
    pub size: usize,
    pub behind: u32,
    pub ahead: u32,
}

pub fn board(w: &World, i: usize) -> Board {
    let rows = w.leaderboard_for(i);
    let rank = rows.iter().position(|r| r.3).unwrap_or(rows.len());
    let mine = rows.get(rank).map_or(0, |r| r.2);
    Board {
        rank,
        size: rows.len(),
        behind: rank
            .checked_sub(1)
            .map_or(0, |k| rows[k].2.saturating_sub(mine)),
        ahead: rows.get(rank + 1).map_or(0, |r| mine.saturating_sub(r.2)),
    }
}

pub fn span(level: u32, scale: f32) -> f32 {
    let level = level.min(40);
    460.0 + 15.0 * level as f32 + radius_for(level) * scale * 3.0
}

pub fn span_of(w: &World, i: usize) -> f32 {
    let s = &w.snakes[i];
    span(s.level(), s.scale)
}

pub fn reach(w: &World, i: usize) -> f32 {
    span_of(w, i) * 0.5
}

pub fn in_sight(w: &World, i: usize, p: Vec2) -> bool {
    (p - w.snakes[i].head()).length() <= reach(w, i)
}

pub fn minimap(w: &World, i: usize) -> Vec<(Vec2, Dot)> {
    let me = w.snakes[i].level();
    w.snakes
        .iter()
        .enumerate()
        .filter(|(j, s)| *j != i && s.alive)
        .map(|(_, s)| {
            let dot = if s.boss {
                Dot::Boss
            } else if s.level() > me {
                Dot::Bigger
            } else if s.level() < me {
                Dot::Smaller
            } else {
                Dot::Even
            };
            (s.head(), dot)
        })
        .collect()
}

pub fn arrows(w: &World, i: usize) -> Vec<Arrow> {
    let me = &w.snakes[i];
    let far = ARROW_REACH + span_of(w, i);
    w.snakes
        .iter()
        .enumerate()
        .filter(|(j, s)| *j != i && s.alive && (s.boss || s.level() > me.level()))
        .filter_map(|(_, s)| {
            let d = s.head() - me.head();
            let dist = d.length();
            (dist > reach(w, i) && dist <= far).then(|| Arrow {
                at: s.head(),
                dir: d / dist,
                level: s.level(),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rng as rand;
    use glam::vec2;

    fn world() -> World {
        rand::srand(4);
        let (mut spec, _) = super::super::rl::training_spec(0);
        spec.bots = 0;
        let mut w = World::new(spec, "Me", 0);
        w.snakes.truncate(1);
        let mid = vec2(w.size, w.size) * 0.5;
        w.snakes[0].segs = vec![mid; w.snakes[0].segs.len()];
        w
    }

    #[test]
    fn a_bigger_snake_off_screen_gets_an_arrow_and_a_red_dot() {
        let mut w = world();
        let lvl = w.snakes[0].level();
        let at = w.snakes[0].head() + vec2(reach(&w, 0) + 300.0, 0.0);
        w.add_snake(at, lvl + 4, "Big".into(), 0);
        let a = arrows(&w, 0);
        assert_eq!(a.len(), 1);
        assert_eq!(a[0].level, lvl + 4);
        assert!((a[0].dir - vec2(1.0, 0.0)).length() < 1e-4);
        assert_eq!(minimap(&w, 0), vec![(at, Dot::Bigger)]);
    }

    #[test]
    fn smaller_or_close_snakes_get_no_arrow() {
        let mut w = world();
        let lvl = w.snakes[0].level() + 5;
        w.snakes[0].xp = super::super::sim::xp_for_level(lvl);
        let head = w.snakes[0].head();
        w.add_snake(
            head + vec2(reach(&w, 0) + 300.0, 0.0),
            lvl - 2,
            "Small".into(),
            0,
        );
        w.add_snake(head + vec2(50.0, 0.0), lvl + 2, "Close".into(), 0);
        assert!(arrows(&w, 0).is_empty());
        assert!(in_sight(&w, 0, head + vec2(50.0, 0.0)));
    }

    #[test]
    fn the_board_says_last_place_and_how_far_behind() {
        let mut w = world();
        let me = w.snakes[0].xp;
        let a = w.add_snake(vec2(100.0, 100.0), 9, "A".into(), 0);
        let b = w.add_snake(vec2(200.0, 100.0), 6, "B".into(), 0);
        w.snakes[a].xp = me + 80;
        w.snakes[b].xp = me + 30;
        let r = board(&w, 0);
        assert_eq!((r.rank, r.size, r.behind, r.ahead), (2, 3, 30, 0));
        let top = board(&w, a);
        assert_eq!((top.rank, top.ahead), (0, 50));
    }

    #[test]
    fn the_view_widens_as_the_snake_grows() {
        assert!(span(20, 1.0) > span(1, 1.0));
    }
}
