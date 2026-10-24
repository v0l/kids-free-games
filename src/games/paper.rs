use crate::engine::*;
use macroquad::prelude::*;
use std::collections::VecDeque;

pub const GW: i32 = 60;
pub const GH: i32 = 44;
pub const STEP: f32 = 0.11;
pub const START_R: i32 = 2;
const RESPAWN: f32 = 3.0;
const VISIBLE_CELLS: f32 = 28.0;

pub const COLORS: [(u8, u8, u8); 5] = [
    (70, 140, 250),
    (250, 90, 90),
    (250, 180, 40),
    (90, 200, 110),
    (190, 110, 230),
];

#[derive(Clone, Debug)]
pub struct Player {
    pub pos: IVec2,
    pub prev: IVec2,
    pub dir: Dir,
    pub next_dir: Option<Dir>,
    pub trail: Vec<IVec2>,
    pub alive: bool,
    pub respawn: f32,
    pub bot: bool,
    bot_steps: i32,
    bot_turns: i32,
}

impl Player {
    fn spawn(pos: IVec2, dir: Dir, bot: bool) -> Self {
        Self {
            pos,
            prev: pos,
            dir,
            next_dir: None,
            trail: vec![],
            alive: true,
            respawn: 0.0,
            bot,
            bot_steps: 0,
            bot_turns: 0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum State {
    Playing,
    Lost,
    Won,
}

pub struct Paper {
    pub owner: Vec<Option<u8>>,
    pub trail: Vec<Option<u8>>,
    pub players: Vec<Player>,
    pub state: State,
    pub best: f32,
    timer: f32,
}

pub fn idx(p: IVec2) -> usize {
    (p.y * GW + p.x) as usize
}

pub fn in_bounds(p: IVec2) -> bool {
    p.x >= 0 && p.y >= 0 && p.x < GW && p.y < GH
}

impl Paper {
    pub fn empty() -> Self {
        Self {
            owner: vec![None; (GW * GH) as usize],
            trail: vec![None; (GW * GH) as usize],
            players: vec![],
            state: State::Playing,
            best: 0.0,
            timer: 0.0,
        }
    }

    pub fn new() -> Self {
        let mut g = Self::empty();
        let spots = [
            ivec2(GW / 2, GH / 2),
            ivec2(10, 9),
            ivec2(GW - 11, 9),
            ivec2(10, GH - 10),
            ivec2(GW - 11, GH - 10),
        ];
        for (i, s) in spots.iter().enumerate() {
            g.add_player(*s, if i == 0 { Dir::Right } else { Dir::Up }, i != 0);
        }
        g
    }

    pub fn add_player(&mut self, at: IVec2, dir: Dir, bot: bool) -> usize {
        let id = self.players.len();
        self.players.push(Player::spawn(at, dir, bot));
        self.claim_square(id, at);
        id
    }

    fn claim_square(&mut self, id: usize, at: IVec2) {
        for dy in -START_R..=START_R {
            for dx in -START_R..=START_R {
                let p = at + ivec2(dx, dy);
                if in_bounds(p) {
                    self.owner[idx(p)] = Some(id as u8);
                }
            }
        }
    }

    pub fn share(&self, id: usize) -> f32 {
        let n = self.owner.iter().filter(|o| **o == Some(id as u8)).count();
        n as f32 / self.owner.len() as f32
    }

    pub fn kill(&mut self, id: usize) {
        let p = &mut self.players[id];
        if !p.alive {
            return;
        }
        p.alive = false;
        p.respawn = RESPAWN;
        p.trail.clear();
        for c in self.trail.iter_mut().chain(self.owner.iter_mut()) {
            if *c == Some(id as u8) {
                *c = None;
            }
        }
    }

    pub fn close_loop(&mut self, id: usize) {
        let me = Some(id as u8);
        let trail = std::mem::take(&mut self.players[id].trail);
        for t in &trail {
            self.trail[idx(*t)] = None;
            self.owner[idx(*t)] = me;
        }
        let mut outside = vec![false; self.owner.len()];
        let mut q = VecDeque::new();
        for x in 0..GW {
            for y in 0..GH {
                if (x == 0 || y == 0 || x == GW - 1 || y == GH - 1)
                    && self.owner[idx(ivec2(x, y))] != me
                {
                    outside[idx(ivec2(x, y))] = true;
                    q.push_back(ivec2(x, y));
                }
            }
        }
        while let Some(p) = q.pop_front() {
            for d in Dir::ALL {
                let n = p + d.delta();
                if in_bounds(n) && !outside[idx(n)] && self.owner[idx(n)] != me {
                    outside[idx(n)] = true;
                    q.push_back(n);
                }
            }
        }
        let mut victims = vec![];
        for ((owner, trail), out) in self.owner.iter_mut().zip(&self.trail).zip(&outside) {
            if !out && *owner != me {
                *owner = me;
                if let Some(other) = trail {
                    victims.push(*other as usize);
                }
            }
        }
        for v in victims {
            self.kill(v);
        }
    }

    pub fn tick(&mut self) {
        for id in 0..self.players.len() {
            if self.players[id].bot && self.players[id].alive {
                self.bot_think(id);
            }
        }
        for id in 0..self.players.len() {
            if !self.players[id].alive {
                continue;
            }
            let p = &mut self.players[id];
            if let Some(nd) = p.next_dir.take() {
                if nd != p.dir.opposite() || p.trail.is_empty() {
                    p.dir = nd;
                }
            }
            let target = p.pos + p.dir.delta();
            p.prev = p.pos;
            if !in_bounds(target) {
                continue;
            }
            p.pos = target;
            let me = Some(id as u8);
            if let Some(owner) = self.trail[idx(target)] {
                self.kill(owner as usize);
                if owner as usize == id {
                    continue;
                }
            }
            if self.owner[idx(target)] == me {
                if !self.players[id].trail.is_empty() {
                    self.close_loop(id);
                }
            } else {
                self.trail[idx(target)] = me;
                self.players[id].trail.push(target);
            }
        }
        for a in 0..self.players.len() {
            for b in (a + 1)..self.players.len() {
                let (pa, pb) = (&self.players[a], &self.players[b]);
                if pa.alive && pb.alive && pa.pos == pb.pos {
                    let a_home = self.owner[idx(pa.pos)] == Some(a as u8);
                    let b_home = self.owner[idx(pb.pos)] == Some(b as u8);
                    if !a_home {
                        self.kill(a);
                    }
                    if !b_home {
                        self.kill(b);
                    }
                }
            }
        }
    }

    fn safe(&self, id: usize, d: Dir) -> bool {
        let p = &self.players[id];
        let n = p.pos + d.delta();
        in_bounds(n)
            && self.trail[idx(n)] != Some(id as u8)
            && !(d == p.dir.opposite() && !p.trail.is_empty())
    }

    fn bot_think(&mut self, id: usize) {
        let me = Some(id as u8);
        let pos = self.players[id].pos;
        let home = self.owner[idx(pos)] == me;
        let trail_len = self.players[id].trail.len() as i32;
        let dir = self.players[id].dir;
        let mut want = dir;
        if home {
            let p = &mut self.players[id];
            p.bot_turns = 0;
            p.bot_steps = rand::gen_range(4, 9);
            if rand::gen_range(0, 4) == 0 {
                want = Dir::ALL[rand::gen_range(0, 4)];
            }
        } else if trail_len > 14 || self.players[id].bot_turns >= 2 {
            want = self.dir_home(id).unwrap_or(dir);
        } else {
            let p = &mut self.players[id];
            p.bot_steps -= 1;
            if p.bot_steps <= 0 {
                p.bot_turns += 1;
                p.bot_steps = rand::gen_range(4, 9);
                want = if rand::gen_range(0, 2) == 0 {
                    turn_left(dir)
                } else {
                    turn_right(dir)
                };
            }
        }
        if !self.safe(id, want) {
            want = [dir, turn_left(dir), turn_right(dir), dir.opposite()]
                .into_iter()
                .find(|d| self.safe(id, *d))
                .unwrap_or(dir);
        }
        self.players[id].next_dir = Some(want);
    }

    fn dir_home(&self, id: usize) -> Option<Dir> {
        let me = Some(id as u8);
        let start = self.players[id].pos;
        let mut seen = vec![false; self.owner.len()];
        let mut first: Vec<Option<Dir>> = vec![None; self.owner.len()];
        let mut q = VecDeque::new();
        seen[idx(start)] = true;
        q.push_back(start);
        while let Some(p) = q.pop_front() {
            if self.owner[idx(p)] == me && p != start {
                return first[idx(p)];
            }
            for d in Dir::ALL {
                let n = p + d.delta();
                if !in_bounds(n) || seen[idx(n)] || self.trail[idx(n)] == me {
                    continue;
                }
                if p == start && !self.safe(id, d) {
                    continue;
                }
                seen[idx(n)] = true;
                first[idx(n)] = first[idx(p)].or(Some(d));
                q.push_back(n);
            }
        }
        None
    }

    fn respawn_bots(&mut self, dt: f32) {
        for id in 1..self.players.len() {
            if !self.players[id].bot || self.players[id].alive {
                continue;
            }
            self.players[id].respawn -= dt;
            if self.players[id].respawn > 0.0 {
                continue;
            }
            for _ in 0..40 {
                let at = ivec2(rand::gen_range(4, GW - 4), rand::gen_range(4, GH - 4));
                let clear = (-START_R - 1..=START_R + 1).all(|dy| {
                    (-START_R - 1..=START_R + 1).all(|dx| {
                        let c = idx(at + ivec2(dx, dy));
                        self.owner[c].is_none() && self.trail[c].is_none()
                    })
                });
                if clear {
                    self.players[id] = Player::spawn(at, Dir::ALL[rand::gen_range(0, 4)], true);
                    self.claim_square(id, at);
                    break;
                }
            }
        }
    }

    fn layout(&self, view: &View) -> (f32, Vec2) {
        let top = 64.0;
        let area = vec2(view.w - 20.0, view.h - top - 10.0);
        let fit = (area.x / GW as f32).min(area.y / GH as f32);
        let cell = fit.max(view.w.min(view.h) / VISIBLE_CELLS).floor();
        let board = vec2(GW as f32, GH as f32) * cell;
        let center = vec2(10.0, top) + area / 2.0;
        let p = &self.players[0];
        let head = (p
            .prev
            .as_vec2()
            .lerp(p.pos.as_vec2(), (self.timer / STEP).clamp(0.0, 1.0))
            + 0.5)
            * cell;
        let axis = |board: f32, area: f32, head: f32, center: f32, lo: f32| {
            if board <= area {
                center - board / 2.0
            } else {
                (center - head).clamp(lo + area - board, lo)
            }
        };
        let origin = vec2(
            axis(board.x, area.x, head.x, center.x, 10.0),
            axis(board.y, area.y, head.y, center.y, top),
        );
        (cell, origin)
    }
}

impl Default for Paper {
    fn default() -> Self {
        Self::new()
    }
}

fn turn_left(d: Dir) -> Dir {
    match d {
        Dir::Up => Dir::Left,
        Dir::Left => Dir::Down,
        Dir::Down => Dir::Right,
        Dir::Right => Dir::Up,
    }
}

fn turn_right(d: Dir) -> Dir {
    turn_left(d).opposite()
}

fn color(id: usize) -> Color {
    let (r, g, b) = COLORS[id % COLORS.len()];
    Color::from_rgba(r, g, b, 255)
}

impl Game for Paper {
    fn update(&mut self, input: &Input, _view: &View, dt: f32) {
        if self.state != State::Playing {
            if input.tap() {
                let best = self.best;
                *self = Self::new();
                self.best = best;
            }
            return;
        }
        if let Some(d) = input.dir() {
            self.players[0].next_dir = Some(d);
        }
        self.timer += dt;
        while self.timer >= STEP {
            self.timer -= STEP;
            self.tick();
            self.respawn_bots(STEP);
        }
        let share = self.share(0);
        self.best = self.best.max(share);
        if !self.players[0].alive {
            self.state = State::Lost;
        } else if share >= 0.5 {
            self.state = State::Won;
        }
    }

    fn draw(&self, view: &View) {
        clear_background(Color::from_rgba(200, 208, 222, 255));
        let (cell, origin) = self.layout(view);
        draw_rectangle(
            origin.x,
            origin.y,
            cell * GW as f32,
            cell * GH as f32,
            WHITE,
        );
        let x0 = ((-origin.x / cell).floor() as i32).max(0);
        let y0 = ((-origin.y / cell).floor() as i32).max(0);
        let x1 = (((view.w - origin.x) / cell).ceil() as i32).min(GW);
        let y1 = (((view.h - origin.y) / cell).ceil() as i32).min(GH);
        for y in y0..y1 {
            for x in x0..x1 {
                let i = idx(ivec2(x, y));
                let px = origin.x + x as f32 * cell;
                let py = origin.y + y as f32 * cell;
                if let Some(o) = self.owner[i] {
                    draw_rectangle(px, py, cell + 0.5, cell + 0.5, color(o as usize));
                } else if (x + y) % 2 == 0 {
                    draw_rectangle(px, py, cell, cell, Color::from_rgba(246, 248, 252, 255));
                }
                if let Some(t) = self.trail[i] {
                    let c = with_alpha(
                        Color::from_vec(
                            color(t as usize).to_vec() * 0.5 + vec4(0.5, 0.5, 0.5, 0.5),
                        ),
                        1.0,
                    );
                    draw_rectangle(
                        px + cell * 0.15,
                        py + cell * 0.15,
                        cell * 0.7,
                        cell * 0.7,
                        c,
                    );
                }
            }
        }
        let frac = (self.timer / STEP).clamp(0.0, 1.0);
        for (id, p) in self.players.iter().enumerate() {
            if !p.alive {
                continue;
            }
            let at = p.prev.as_vec2().lerp(p.pos.as_vec2(), frac);
            let c = origin + at * cell + vec2(cell / 2.0, cell / 2.0);
            let s = cell * 0.85;
            draw_rectangle(
                c.x - s,
                c.y - s + 3.0,
                s * 2.0,
                s * 2.0,
                darker(color(id), 0.6),
            );
            draw_rectangle(c.x - s, c.y - s, s * 2.0, s * 2.0, darker(color(id), 0.85));
            draw_rectangle(
                c.x - s + 3.0,
                c.y - s + 3.0,
                s * 2.0 - 6.0,
                s * 2.0 - 6.0,
                color(id),
            );
            let look = p.dir.delta().as_vec2() * s * 0.25;
            for side in [-1.0, 1.0] {
                let perp = vec2(-look.y, look.x).normalize_or_zero() * s * 0.4 * side;
                draw_circle(
                    c.x + look.x + perp.x,
                    c.y + look.y + perp.y,
                    s * 0.28,
                    WHITE,
                );
                draw_circle(
                    c.x + look.x * 1.6 + perp.x,
                    c.y + look.y * 1.6 + perp.y,
                    s * 0.13,
                    BLACK,
                );
            }
            if id == 0 {
                text_shadow("YOU", c.x, c.y - s - 16.0, 24.0, WHITE);
            }
        }
        let pct = self.share(0) * 100.0;
        draw_rectangle(0.0, 0.0, view.w, 62.0, Color::from_rgba(236, 240, 246, 235));
        round_rect(
            Rect::new(view.w / 2.0 - 150.0, 8.0, 300.0, 48.0),
            24.0,
            color(0),
        );
        text_center(&format!("{pct:.1}%"), view.w / 2.0, 32.0, 34.0, WHITE);
        let mut x = view.w - 20.0;
        for id in (1..self.players.len()).rev() {
            let s = format!("{:.0}%", self.share(id) * 100.0);
            let w = text_size(&s, 26.0).width + 24.0;
            x -= w + 8.0;
            round_rect(
                Rect::new(x, 14.0, w, 36.0),
                18.0,
                if self.players[id].alive {
                    color(id)
                } else {
                    GRAY
                },
            );
            text_center(&s, x + w / 2.0, 32.0, 26.0, WHITE);
        }
        match self.state {
            State::Lost => {
                let s = format!("You had {:.1}%   Best {:.1}%", pct, self.best * 100.0);
                overlay(view, "Oops!", &[&s, "Tap to try again"]);
            }
            State::Won => overlay(
                view,
                "You win!",
                &["Half the map is yours!", "Tap to play again"],
            ),
            State::Playing => {}
        }
    }
}

pub fn icon(r: Rect) {
    draw_rectangle(r.x, r.y, r.w, r.h, WHITE);
    draw_rectangle(
        r.x + r.w * 0.1,
        r.y + r.h * 0.15,
        r.w * 0.45,
        r.h * 0.45,
        color(0),
    );
    draw_rectangle(
        r.x + r.w * 0.5,
        r.y + r.h * 0.55,
        r.w * 0.4,
        r.h * 0.35,
        color(1),
    );
    draw_rectangle(
        r.x + r.w * 0.55,
        r.y + r.h * 0.2,
        r.w * 0.06,
        r.h * 0.3,
        with_alpha(color(0), 0.5),
    );
    draw_rectangle(
        r.x + r.w * 0.55,
        r.y + r.h * 0.2,
        r.w * 0.2,
        r.h * 0.06,
        with_alpha(color(0), 0.5),
    );
    draw_rectangle(
        r.x + r.w * 0.68,
        r.y + r.h * 0.15,
        r.w * 0.14,
        r.h * 0.14,
        darker(color(0), 0.8),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    fn solo() -> Paper {
        let mut g = Paper::empty();
        g.add_player(ivec2(10, 10), Dir::Right, false);
        g
    }

    fn walk(g: &mut Paper, moves: &[(Dir, usize)]) {
        for (d, n) in moves {
            g.players[0].next_dir = Some(*d);
            for _ in 0..*n {
                g.tick();
            }
        }
    }

    #[test]
    fn closing_a_loop_claims_the_inside() {
        let mut g = solo();
        let before = g.owner.iter().filter(|o| o.is_some()).count();
        assert_eq!(before, 25);
        walk(
            &mut g,
            &[
                (Dir::Right, 6),
                (Dir::Down, 6),
                (Dir::Left, 6),
                (Dir::Up, 4),
            ],
        );
        assert!(g.players[0].trail.is_empty());
        assert_eq!(g.owner[idx(ivec2(15, 15))], Some(0));
        assert_eq!(g.owner[idx(ivec2(14, 13))], Some(0));
        assert!(g.owner.iter().filter(|o| o.is_some()).count() > 50);
        assert!(g.trail.iter().all(|t| t.is_none()));
    }

    #[test]
    fn crossing_your_own_trail_kills_you() {
        let mut g = solo();
        walk(
            &mut g,
            &[
                (Dir::Right, 6),
                (Dir::Down, 3),
                (Dir::Left, 2),
                (Dir::Up, 4),
            ],
        );
        assert!(!g.players[0].alive);
        assert!(g.owner.iter().all(|o| o.is_none()));
    }

    #[test]
    fn cutting_an_enemy_trail_kills_the_enemy() {
        let mut g = solo();
        let bot = g.add_player(ivec2(30, 10), Dir::Down, false);
        for _ in 0..5 {
            g.players[bot].next_dir = Some(Dir::Down);
            g.tick();
        }
        assert!(g.trail[idx(ivec2(30, 14))] == Some(bot as u8));
        g.players[0].pos = ivec2(28, 14);
        g.players[0].dir = Dir::Right;
        for _ in 0..2 {
            g.players[bot].next_dir = Some(Dir::Down);
            g.players[0].next_dir = Some(Dir::Right);
            g.tick();
        }
        assert!(!g.players[bot].alive);
        assert!(g.players[0].alive);
        assert!(g.owner.iter().all(|o| *o != Some(bot as u8)));
    }

    #[test]
    fn camera_keeps_player_on_screen_on_phone_and_ipad() {
        for v in [
            View::fit(1080.0, 2400.0),
            View::ipad_landscape(),
            View::ipad_portrait(),
        ] {
            let mut g = Paper::new();
            for at in [
                ivec2(0, 0),
                ivec2(GW - 1, GH - 1),
                ivec2(GW - 1, 0),
                ivec2(30, 20),
            ] {
                g.players[0].pos = at;
                g.players[0].prev = at;
                let (cell, origin) = g.layout(&v);
                assert!(cell >= v.w.min(v.h) / VISIBLE_CELLS - 1.0);
                let head = origin + at.as_vec2() * cell;
                assert!(
                    head.x >= 0.0 && head.x + cell <= v.w && head.y >= 64.0 && head.y + cell <= v.h,
                    "{at} at {head} in {v:?}"
                );
            }
        }
    }

    #[test]
    fn walls_stop_instead_of_kill() {
        let mut g = solo();
        walk(&mut g, &[(Dir::Up, 30)]);
        assert!(g.players[0].alive);
        assert_eq!(g.players[0].pos.y, 0);
    }

    #[test]
    fn bots_survive_and_grow_alone() {
        rand::srand(7);
        let mut g = Paper::empty();
        g.add_player(ivec2(2, 2), Dir::Right, false);
        let bot = g.add_player(ivec2(30, 22), Dir::Up, true);
        let start = g.share(bot);
        let mut deaths = 0;
        for _ in 0..1500 {
            g.players[0].next_dir = Some(Dir::Down);
            g.tick();
            if !g.players[bot].alive {
                deaths += 1;
                g.respawn_bots(10.0);
            }
        }
        assert!(deaths <= 2, "bot died {deaths} times");
        assert!(g.share(bot) > start * 3.0, "bot share {}", g.share(bot));
    }
}
