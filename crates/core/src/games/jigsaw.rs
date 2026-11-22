use crate::engine::*;
use crate::feel::{self, Impact};
use crate::fx::*;
use crate::geom::*;
use crate::rng as rand;
use crate::sfx::{self, Say, Sfx};
use glam::{vec2, Vec2};

pub const GAME: &str = "jigsaw";
pub const LEVELS: usize = 8;
pub const GRIDS: [(usize, usize); LEVELS] = [
    (2, 2),
    (3, 2),
    (3, 3),
    (4, 3),
    (4, 4),
    (5, 4),
    (5, 5),
    (6, 6),
];
pub const GHOST_LEVELS: usize = 4;
pub const FINISH_SECS: f32 = 1.5;
pub const HUD_GOAL: &str = "Drag each piece to its spot";
pub const HUD_TITLE: &str = "Level 8: 36 pieces";
const FIRST_HINT_SECS: f32 = 6.0;
const HINT_SECS: f32 = 14.0;

pub fn pieces_in(level: usize) -> usize {
    let (c, r) = GRIDS[level.min(LEVELS - 1)];
    c * r
}

pub fn level_name(level: usize) -> String {
    format!("{} pieces", pieces_in(level))
}

pub fn picture_name(level: usize) -> String {
    format!("p{}", level.min(LEVELS - 1))
}

pub fn ghost(level: usize) -> bool {
    level < GHOST_LEVELS
}

pub fn par_secs(level: usize) -> f32 {
    8.0 + pieces_in(level) as f32 * 5.0
}

pub fn stars_for(level: usize, misses: u32, secs: f32) -> u8 {
    let n = pieces_in(level) as u32;
    1 + (misses <= n / 6) as u8 + (secs <= par_secs(level)) as u8
}

pub fn stars_score(s: i64) -> String {
    format!("{s} stars")
}

pub fn hud(view: &View) -> crate::learn::Hud {
    crate::learn::learn_hud(
        view,
        &crate::learn::HudSpec {
            title: HUD_TITLE,
            levels: crate::learn::LevelsAt::Goal,
            stars: LEVELS as u32,
            progress: 0.0,
            goal: HUD_GOAL,
        },
    )
}

pub fn levels_button(view: &View) -> Rect {
    hud(view).levels.unwrap_or_default()
}

pub use super::pattern::{level_tiles, play_button};

#[derive(Clone, Copy, Debug)]
pub struct Layout {
    pub board: Rect,
    pub cell: f32,
    pub area: Rect,
}

pub fn layout(view: &View, level: usize) -> Layout {
    let n = pieces_in(level);
    let mut shrink = 1.0;
    loop {
        let lay = layout_scaled(view, level, shrink);
        if shrink < 0.3 || lay.grid_spots().len() >= n {
            return lay;
        }
        shrink *= 0.95;
    }
}

fn layout_scaled(view: &View, level: usize, shrink: f32) -> Layout {
    let (cols, rows) = GRIDS[level.min(LEVELS - 1)];
    let top = hud(view).bottom + 6.0;
    let m = view.edge().clamp(10.0, 24.0);
    let area = Rect::new(m, top, view.w - 2.0 * m, (view.h - top - m).max(1.0));
    let tall = view.h > view.w;
    let (max_w, max_h) = if tall {
        (area.w, area.h * 0.5)
    } else {
        (area.w * 0.5, area.h)
    };
    let cell = (max_w / cols as f32).min(max_h / rows as f32) * shrink;
    let bw = cell * cols as f32;
    let bh = cell * rows as f32;
    let y = if tall {
        area.y
    } else {
        area.y + (area.h - bh) / 2.0
    };
    Layout {
        board: Rect::new(area.x + (area.w - bw) / 2.0, y, bw, bh),
        cell,
        area,
    }
}

impl Layout {
    pub fn cell_center(&self, col: usize, row: usize) -> Vec2 {
        vec2(
            self.board.x + (col as f32 + 0.5) * self.cell,
            self.board.y + (row as f32 + 0.5) * self.cell,
        )
    }

    pub fn snap_radius(&self, level: usize) -> f32 {
        self.cell * if ghost(level) { 0.7 } else { 0.5 }
    }

    pub fn grid_spots(&self) -> Vec<Vec2> {
        let step = self.cell * 1.06;
        let pad = Rect::new(
            self.board.x - self.cell * 0.3,
            self.board.y - self.cell * 0.3,
            self.board.w + self.cell * 0.6,
            self.board.h + self.cell * 0.6,
        );
        let half = self.cell * 0.5;
        let mut spots = vec![];
        let mut y = self.area.y + half;
        while y <= self.area.bottom() - half {
            let mut x = self.area.x + half;
            while x <= self.area.right() - half {
                let piece = Rect::new(x - half, y - half, self.cell, self.cell);
                if !piece.overlaps(&pad) {
                    spots.push(vec2(x, y));
                }
                x += step;
            }
            y += step;
        }
        spots
    }

    pub fn loose_spots(&self, n: usize) -> Vec<Vec2> {
        let half = self.cell * 0.5;
        let mut spots = self.grid_spots();
        if spots.is_empty() {
            spots.push(vec2(self.area.center().x, self.area.bottom() - half));
        }
        for i in (1..spots.len()).rev() {
            spots.swap(i, rand::gen_range(0, i + 1));
        }
        (0..n)
            .map(|i| {
                let s = spots[i % spots.len()];
                if i < spots.len() {
                    s
                } else {
                    let j = self.cell * 0.3;
                    let p = s + vec2(rand::gen_range(-j, j), rand::gen_range(-j, j));
                    vec2(
                        p.x.clamp(self.area.x + half, self.area.right() - half),
                        p.y.clamp(self.area.y + half, self.area.bottom() - half),
                    )
                }
            })
            .collect()
    }
}

pub struct Piece {
    pub col: usize,
    pub row: usize,
    pub pos: Vec2,
    pub goal: Vec2,
    pub placed: bool,
    pub pop: Bump,
    pub wobble: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Mode {
    Play,
    Levels,
    Done(f32),
    Report(f32),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Report {
    Level { level: usize, stars: u8 },
    Cleared { rank: Option<usize> },
}

pub struct Jigsaw {
    pub mode: Mode,
    pub level: usize,
    pub pieces: Vec<Piece>,
    pub order: Vec<usize>,
    pub held: Option<(usize, Vec2, Vec2)>,
    pub misses: u32,
    pub secs: f32,
    pub idle: f32,
    pub first_time: bool,
    pub report: Option<Report>,
    pub stars: u8,
    pub laid: (f32, f32),
    pub shake: Shake,
    pub particles: Particles,
    pub popups: Popups,
    pub t: f32,
}

pub fn unlocked_level() -> usize {
    (crate::profile::unlocked(GAME) as usize).min(LEVELS - 1)
}

impl Default for Jigsaw {
    fn default() -> Self {
        Self::new()
    }
}

impl Jigsaw {
    pub fn new() -> Self {
        let level = (crate::profile::level(GAME) as usize).min(unlocked_level());
        let mut g = Self {
            mode: Mode::Play,
            level,
            pieces: vec![],
            order: vec![],
            held: None,
            misses: 0,
            secs: 0.0,
            idle: 0.0,
            first_time: crate::profile::get(GAME, "played") == 0,
            report: None,
            stars: 0,
            laid: (0.0, 0.0),
            shake: Shake::default(),
            particles: Particles::default(),
            popups: Popups::default(),
            t: 0.0,
        };
        g.start_level(level);
        g
    }

    pub fn start_level(&mut self, level: usize) {
        self.level = level.min(LEVELS - 1);
        crate::profile::set_level(GAME, self.level as u32);
        let (cols, rows) = GRIDS[self.level];
        self.pieces = (0..cols * rows)
            .map(|i| Piece {
                col: i % cols,
                row: i / cols,
                pos: Vec2::ZERO,
                goal: Vec2::ZERO,
                placed: false,
                pop: Bump::default(),
                wobble: 0.0,
            })
            .collect();
        self.order = (0..self.pieces.len()).collect();
        self.held = None;
        self.misses = 0;
        self.secs = 0.0;
        self.idle = 0.0;
        self.stars = 0;
        self.mode = Mode::Play;
        self.laid = (0.0, 0.0);
    }

    pub fn placed_count(&self) -> usize {
        self.pieces.iter().filter(|p| p.placed).count()
    }

    pub fn solved(&self) -> bool {
        self.pieces.iter().all(|p| p.placed)
    }

    fn scatter(&mut self, view: &View) {
        let lay = layout(view, self.level);
        let loose: Vec<usize> = (0..self.pieces.len())
            .filter(|i| !self.pieces[*i].placed)
            .collect();
        let spots = lay.loose_spots(loose.len());
        for (k, i) in loose.into_iter().enumerate() {
            let p = &mut self.pieces[i];
            p.pos = spots[k];
            p.goal = spots[k];
        }
        for p in self.pieces.iter_mut().filter(|p| p.placed) {
            let c = lay.cell_center(p.col, p.row);
            p.pos = c;
            p.goal = c;
        }
        self.laid = (view.w, view.h);
    }

    pub fn piece_at(&self, view: &View, at: Vec2) -> Option<usize> {
        let half = layout(view, self.level).cell / 2.0;
        self.order.iter().rev().copied().find(|i| {
            let p = &self.pieces[*i];
            !p.placed && (at.x - p.pos.x).abs() <= half && (at.y - p.pos.y).abs() <= half
        })
    }

    pub fn hint_piece(&self) -> Option<usize> {
        let wait = if self.first_time || ghost(self.level) {
            FIRST_HINT_SECS
        } else {
            HINT_SECS
        };
        if self.idle < wait || self.held.is_some() || self.mode != Mode::Play {
            return None;
        }
        self.pieces.iter().position(|p| !p.placed)
    }

    fn pick(&mut self, view: &View, at: Vec2) {
        if let Some(i) = self.piece_at(view, at) {
            self.order.retain(|o| *o != i);
            self.order.push(i);
            let p = &self.pieces[i];
            self.held = Some((i, p.pos - at, p.pos));
            self.first_time = false;
            crate::profile::set(GAME, "played", 1);
            sfx::play(Sfx::Tap);
        }
    }

    fn drag(&mut self, view: &View, at: Vec2) {
        if let Some((i, off, _)) = self.held {
            let lay = layout(view, self.level);
            let half = lay.cell / 2.0;
            let c = at + off;
            let c = vec2(
                c.x.clamp(half, view.w - half),
                c.y.clamp(half, view.h - half),
            );
            self.pieces[i].pos = c;
            self.pieces[i].goal = c;
        }
    }

    pub fn drop_piece(&mut self, view: &View) {
        let Some((i, _, from)) = self.held.take() else {
            return;
        };
        let lay = layout(view, self.level);
        let (col, row) = (self.pieces[i].col, self.pieces[i].row);
        let target = lay.cell_center(col, row);
        let here = self.pieces[i].pos;
        if here.distance(target) <= lay.snap_radius(self.level) {
            self.snap(view, i, target);
        } else if lay.board.scaled_about_center(1.1).contains(here) {
            self.misses += 1;
            let p = &mut self.pieces[i];
            p.goal = from;
            p.wobble = 1.0;
            sfx::play(Sfx::Wrong);
            self.shake.add(0.15);
            self.popups
                .add(here - vec2(0.0, lay.cell * 0.6), "Not here", Color::from_rgba(255, 170, 90, 255), 44.0);
        }
    }

    fn snap(&mut self, view: &View, i: usize, target: Vec2) {
        let gold = Color::from_rgba(255, 220, 70, 255);
        let p = &mut self.pieces[i];
        p.placed = true;
        p.goal = target;
        p.pop.kick();
        let streak = self.pieces.iter().filter(|p| p.placed).count();
        sfx::play_step(Sfx::Pop, streak.min(8));
        self.particles.sparkle(target, gold);
        if self.solved() {
            self.finish(view);
        }
    }

    fn finish(&mut self, view: &View) {
        let level = self.level;
        let stars = stars_for(level, self.misses, self.secs);
        self.stars = stars;
        crate::profile::award_stars(GAME, level as u32, stars);
        crate::profile::set(GAME, "played", 1);
        if level + 1 < LEVELS {
            crate::profile::unlock(GAME, level as u32 + 1);
            crate::profile::set_level(GAME, level as u32 + 1);
            self.report = Some(Report::Level { level, stars });
        } else if crate::profile::get(GAME, "cleared") == 0 {
            crate::profile::set(GAME, "cleared", 1);
            let total = crate::profile::total_stars(GAME) as i64;
            let rank = crate::profile::submit(GAME, total, crate::profile::Better::Higher);
            self.report = Some(Report::Cleared { rank });
        } else {
            self.report = Some(Report::Level { level, stars });
        }
        sfx::play(Sfx::Win);
        sfx::say(if stars == 3 { Say::Congrats } else { Say::Win });
        feel::impact_tinted(Impact::Large, Color::from_rgba(255, 230, 120, 255));
        self.particles.confetti(view);
        self.shake.add(0.4);
        self.mode = Mode::Done(0.0);
    }

    fn after_report(&mut self) {
        let next = match self.report.take() {
            Some(Report::Level { level, .. }) if level + 1 < LEVELS => level + 1,
            _ => self.level,
        };
        self.start_level(next);
    }

    fn update_play(&mut self, input: &Input, view: &View, dt: f32) {
        self.secs += dt;
        if input.active() {
            self.idle = 0.0;
        } else {
            self.idle += dt;
        }
        if input.pressed && levels_button(view).contains(input.pos) && !self.first_time {
            self.held = None;
            self.mode = Mode::Levels;
            return;
        }
        if input.pressed {
            self.pick(view, input.pos);
        }
        if input.down {
            self.drag(view, input.pos);
        }
        if input.released || (self.held.is_some() && !input.down && !input.pressed) {
            self.drop_piece(view);
        }
    }

    fn update_levels(&mut self, input: &Input, view: &View) {
        if !input.pressed {
            return;
        }
        if play_button(view).contains(input.pos) {
            self.mode = Mode::Play;
            return;
        }
        let open = unlocked_level();
        let hit = level_tiles(view).iter().position(|r| r.contains(input.pos));
        if let Some(i) = hit.filter(|i| *i <= open) {
            sfx::play(Sfx::Tick);
            self.start_level(i);
        }
    }
}

impl Game for Jigsaw {
    fn update(&mut self, input: &Input, view: &View, dt: f32) {
        self.t += dt;
        if self.laid != (view.w, view.h) {
            self.scatter(view);
        }
        let held = self.held.map(|h| h.0);
        let k = (dt * 16.0).min(1.0);
        for (i, p) in self.pieces.iter_mut().enumerate() {
            p.pop.update(dt);
            p.wobble = (p.wobble - dt * 1.8).max(0.0);
            if Some(i) != held {
                p.pos += (p.goal - p.pos) * k;
            }
        }
        match self.mode {
            Mode::Play => self.update_play(input, view, dt),
            Mode::Levels => self.update_levels(input, view),
            Mode::Done(t) => {
                self.mode = if t + dt >= FINISH_SECS {
                    Mode::Report(0.0)
                } else {
                    Mode::Done(t + dt)
                };
            }
            Mode::Report(age) => {
                self.mode = Mode::Report(age + dt);
                if input.pressed && age > 0.8 {
                    self.after_report();
                }
            }
        }
        self.shake.update(dt);
        self.particles.update(dt);
        self.popups.update(dt);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn views() -> [View; 4] {
        [
            View::ipad_landscape(),
            View::ipad_portrait(),
            View::fit(390.0, 844.0),
            View::fit(844.0, 390.0),
        ]
    }

    fn fresh() -> Jigsaw {
        crate::save::reset_for_test();
        crate::profile::create("Mia", 0);
        Jigsaw::new()
    }

    fn settle(g: &mut Jigsaw, v: &View) {
        for _ in 0..40 {
            g.update(&Input::default(), v, 1.0 / 60.0);
        }
    }

    fn drag_to(g: &mut Jigsaw, v: &View, piece: usize, to: Vec2) {
        let from = g.pieces[piece].pos;
        g.update(&Input::press_at(from), v, 1.0 / 60.0);
        let mut mid = Input::press_at(to);
        mid.pressed = false;
        g.update(&mid, v, 1.0 / 60.0);
        let mut up = Input::default();
        up.pos = to;
        up.released = true;
        g.update(&up, v, 1.0 / 60.0);
        settle(g, v);
    }

    fn place_all(g: &mut Jigsaw, v: &View) {
        for i in 0..g.pieces.len() {
            let lay = layout(v, g.level);
            let t = lay.cell_center(g.pieces[i].col, g.pieces[i].row);
            drag_to(g, v, i, t);
        }
    }

    #[test]
    fn piece_counts_rise_level_by_level_from_four() {
        assert_eq!(pieces_in(0), 4);
        assert_eq!(pieces_in(1), 6);
        assert_eq!(pieces_in(2), 9);
        for l in 1..LEVELS {
            assert!(pieces_in(l) > pieces_in(l - 1));
        }
        assert_eq!(pieces_in(LEVELS - 1), 36);
    }

    #[test]
    fn the_ghost_helps_only_on_the_early_levels() {
        assert!(ghost(0) && ghost(GHOST_LEVELS - 1));
        assert!(!ghost(GHOST_LEVELS) && !ghost(LEVELS - 1));
    }

    #[test]
    fn stars_reward_few_misses_and_speed() {
        assert_eq!(stars_for(2, 0, 10.0), 3);
        assert_eq!(stars_for(2, 0, 9999.0), 2);
        assert_eq!(stars_for(2, 99, 10.0), 2);
        assert_eq!(stars_for(2, 99, 9999.0), 1);
        assert_eq!(stars_for(4, 2, par_secs(4)), 3);
        assert_eq!(stars_for(4, 3, par_secs(4)), 2);
    }

    #[test]
    fn level_picker_matches_the_level_count() {
        assert_eq!(level_tiles(&View::ipad_landscape()).len(), LEVELS);
        assert_eq!(super::super::pattern::LEVELS, LEVELS);
    }

    #[test]
    fn the_board_and_every_loose_piece_fit_on_every_screen() {
        for v in views() {
            for level in 0..LEVELS {
                let lay = layout(&v, level);
                assert!(lay.board.x >= 0.0 && lay.board.right() <= v.w + 0.5, "{level}");
                assert!(lay.board.y >= hud(&v).bottom && lay.board.bottom() <= v.h + 0.5, "{level} {v:?}");
                for s in lay.loose_spots(pieces_in(level)) {
                    assert!(s.x - lay.cell / 2.0 >= -0.5 && s.x + lay.cell / 2.0 <= v.w + 0.5);
                    assert!(s.y - lay.cell / 2.0 >= hud(&v).bottom - 0.5 && s.y + lay.cell / 2.0 <= v.h + 0.5);
                }
            }
        }
    }

    #[test]
    fn pieces_start_loose_and_spread_outside_the_board() {
        for v in [View::ipad_landscape(), View::ipad_portrait()] {
            let mut g = fresh();
            g.start_level(2);
            settle(&mut g, &v);
            let lay = layout(&v, 2);
            let inside = g.pieces.iter().filter(|p| lay.board.contains(p.pos)).count();
            assert_eq!(inside, 0);
            assert_eq!(g.placed_count(), 0);
        }
    }

    #[test]
    fn dropping_a_piece_near_its_spot_snaps_it_in() {
        let v = View::ipad_landscape();
        let mut g = fresh();
        settle(&mut g, &v);
        let lay = layout(&v, 0);
        let near = lay.cell_center(g.pieces[0].col, g.pieces[0].row) + vec2(lay.cell * 0.3, 0.0);
        drag_to(&mut g, &v, 0, near);
        assert!(g.pieces[0].placed);
        assert_eq!(g.misses, 0);
        let target = lay.cell_center(g.pieces[0].col, g.pieces[0].row);
        assert!(g.pieces[0].pos.distance(target) < 1.0);
    }

    #[test]
    fn a_piece_dropped_on_the_wrong_cell_goes_back_and_counts_a_miss() {
        let v = View::ipad_landscape();
        let mut g = fresh();
        settle(&mut g, &v);
        let lay = layout(&v, 0);
        let wrong = lay.cell_center(1 - g.pieces[0].col, 1 - g.pieces[0].row);
        let before = g.pieces[0].pos;
        drag_to(&mut g, &v, 0, wrong);
        assert!(!g.pieces[0].placed);
        assert_eq!(g.misses, 1);
        assert!(g.pieces[0].pos.distance(before) < 2.0);
    }

    #[test]
    fn a_piece_dropped_far_from_the_board_just_stays_there() {
        let v = View::ipad_landscape();
        let mut g = fresh();
        settle(&mut g, &v);
        let lay = layout(&v, 0);
        let away = vec2(lay.area.x + lay.cell, lay.area.bottom() - lay.cell);
        drag_to(&mut g, &v, 0, away);
        assert!(!g.pieces[0].placed);
        assert_eq!(g.misses, 0);
        assert!(g.pieces[0].pos.distance(away) < 2.0);
    }

    #[test]
    fn placed_pieces_cannot_be_picked_up_again() {
        let v = View::ipad_landscape();
        let mut g = fresh();
        settle(&mut g, &v);
        let lay = layout(&v, 0);
        let t = lay.cell_center(g.pieces[0].col, g.pieces[0].row);
        drag_to(&mut g, &v, 0, t);
        assert!(g.piece_at(&v, t).is_none());
    }

    #[test]
    fn finishing_a_level_saves_stars_unlocks_the_next_and_opens_the_report() {
        let v = View::ipad_landscape();
        let mut g = fresh();
        settle(&mut g, &v);
        place_all(&mut g, &v);
        assert!(g.solved());
        assert!(matches!(g.mode, Mode::Done(_)));
        assert_eq!(crate::profile::stars(GAME, 0), 3);
        assert_eq!(crate::profile::unlocked(GAME), 1);
        assert_eq!(crate::profile::level(GAME), 1);
        for _ in 0..120 {
            g.update(&Input::default(), &v, 1.0 / 60.0);
        }
        assert!(matches!(g.mode, Mode::Report(_)));
        assert_eq!(g.report, Some(Report::Level { level: 0, stars: 3 }));
        g.update(&Input::press_at(vec2(500.0, 400.0)), &v, 1.0 / 60.0);
        assert_eq!(g.level, 1);
        assert_eq!(g.mode, Mode::Play);
        assert_eq!(g.placed_count(), 0);
    }

    #[test]
    fn progress_is_saved_per_player() {
        let v = View::ipad_landscape();
        let mut g = fresh();
        settle(&mut g, &v);
        place_all(&mut g, &v);
        let other = crate::profile::create("Leo", 1).unwrap();
        crate::profile::select(&other.id);
        assert_eq!(crate::profile::stars(GAME, 0), 0);
        assert_eq!(Jigsaw::new().level, 0);
    }

    #[test]
    fn a_new_game_resumes_at_the_saved_level() {
        let mut g = fresh();
        crate::profile::unlock(GAME, 5);
        g.start_level(3);
        assert_eq!(Jigsaw::new().level, 3);
    }

    #[test]
    fn locked_levels_cannot_be_opened_from_the_picker() {
        let v = View::ipad_landscape();
        let mut g = fresh();
        g.mode = Mode::Levels;
        let tiles = level_tiles(&v);
        g.update(&Input::press_at(tiles[4].center()), &v, 1.0 / 60.0);
        assert_eq!(g.mode, Mode::Levels);
        g.update(&Input::press_at(tiles[0].center()), &v, 1.0 / 60.0);
        assert_eq!(g.mode, Mode::Play);
    }

    #[test]
    fn clearing_the_top_level_submits_total_stars_once() {
        let v = View::ipad_landscape();
        let mut g = fresh();
        for l in 0..LEVELS - 1 {
            crate::profile::award_stars(GAME, l as u32, 2);
        }
        crate::profile::unlock(GAME, LEVELS as u32 - 1);
        g.start_level(LEVELS - 1);
        settle(&mut g, &v);
        place_all(&mut g, &v);
        assert!(matches!(g.report, Some(Report::Cleared { rank: Some(0) })));
        assert_eq!(crate::profile::board(GAME).len(), 1);
        assert!(crate::profile::board(GAME)[0].score >= 14);
        g.start_level(LEVELS - 1);
        settle(&mut g, &v);
        place_all(&mut g, &v);
        assert!(matches!(g.report, Some(Report::Level { .. })));
        assert_eq!(crate::profile::board(GAME).len(), 1);
    }

    #[test]
    fn an_idle_child_gets_a_hint_on_a_loose_piece() {
        let v = View::ipad_landscape();
        let mut g = fresh();
        assert!(g.hint_piece().is_none());
        for _ in 0..(FIRST_HINT_SECS * 60.0) as usize + 5 {
            g.update(&Input::default(), &v, 1.0 / 60.0);
        }
        assert!(g.hint_piece().is_some());
    }

    #[test]
    fn rotating_the_screen_keeps_placed_pieces_in_place() {
        let mut g = fresh();
        let a = View::ipad_landscape();
        settle(&mut g, &a);
        let lay = layout(&a, 0);
        let t = lay.cell_center(g.pieces[0].col, g.pieces[0].row);
        drag_to(&mut g, &a, 0, t);
        let b = View::ipad_portrait();
        settle(&mut g, &b);
        let lb = layout(&b, 0);
        let tb = lb.cell_center(g.pieces[0].col, g.pieces[0].row);
        assert!(g.pieces[0].pos.distance(tb) < 1.0);
    }

    #[test]
    fn the_top_bar_fits_on_every_screen() {
        for v in views() {
            let spec = crate::learn::HudSpec {
                title: HUD_TITLE,
                levels: crate::learn::LevelsAt::Goal,
                stars: LEVELS as u32,
                progress: 0.0,
                goal: HUD_GOAL,
            };
            let bad = crate::learn::hud_problems(&v, "jigsaw", &spec, &[]);
            assert!(bad.is_empty(), "{bad:?}");
        }
    }
}
