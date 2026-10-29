use crate::engine::*;
use crate::feel::{self, Impact};
use crate::fx::*;
use crate::games::hotel::Cam;
use crate::geom::*;
use crate::learn::{hud_bottom, tall, ui_scale, PRAISE};
use crate::mastery::{self, Session};
use crate::profile;
use crate::rng as rand;
use crate::sfx::{self, Say, Sfx, TimesLine};
use glam::{vec2, vec3, Vec2, Vec3};

pub const GAME: &str = "garden";
pub const TABLES: [i32; 11] = [2, 10, 5, 3, 4, 8, 6, 9, 7, 11, 12];
pub const LEVELS: usize = 12;
pub const MIXED: usize = 11;
pub const QUESTIONS: u32 = 8;
pub const OPTIONS: usize = 3;
pub const MASTER_RUN: u32 = 10;
pub const STAGE_UP_RUN: u32 = 4;
pub const SLIPS_DOWN: u32 = 3;
pub const MAX_FACTOR: i32 = 12;
pub const FADE_AFTER: f32 = 3.2;
pub const GROW_TIME: f32 = 0.45;
pub const TARGET_SECS: f32 = 7.0;
pub const MAX_DUE: usize = 2;
pub const SPACING: f32 = 1.0;
pub const PITCH: f32 = 0.8;
pub const FOV: f32 = 0.7;
pub const BED_COLORS: usize = 6;

pub fn level_name(level: usize) -> String {
    match level {
        MIXED => "Mixed garden".to_string(),
        l => format!("Times {}", TABLES[l]),
    }
}

pub fn stage_name(stage: usize) -> &'static str {
    [
        "Count the flowers",
        "Remember the flowers",
        "Just the numbers",
    ][stage.min(2)]
}

pub fn tables_of(level: usize) -> Vec<i32> {
    if level == MIXED {
        let mut t = TABLES.to_vec();
        t.sort_unstable();
        t
    } else {
        vec![TABLES[level]]
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Fact {
    pub table: i32,
    pub other: i32,
}

impl Fact {
    pub fn key(&self) -> String {
        format!(
            "{}x{}",
            self.table.min(self.other),
            self.table.max(self.other)
        )
    }

    pub fn product(&self) -> i32 {
        self.table * self.other
    }
}

pub fn facts_of(level: usize) -> Vec<Fact> {
    let mut out: Vec<Fact> = vec![];
    for t in tables_of(level) {
        for o in 2..=MAX_FACTOR {
            let f = Fact { table: t, other: o };
            if !out.iter().any(|g| g.key() == f.key()) {
                out.push(f);
            }
        }
    }
    out
}

pub fn pool_size(level: usize) -> usize {
    facts_of(level).len() * 2
}

pub fn memory_of(f: &Fact) -> crate::games::spell::Memory {
    let k = f.key();
    crate::games::spell::Memory {
        at: profile::get(GAME, &format!("f.{k}.at")) as u32,
        due: profile::get(GAME, &format!("f.{k}.due")) as u32,
        rung: profile::get(GAME, &format!("f.{k}.rung")) as u32,
    }
}

fn remember(f: &Fact, m: crate::games::spell::Memory) {
    let k = f.key();
    profile::set(GAME, &format!("f.{k}.at"), m.at as i64);
    profile::set(GAME, &format!("f.{k}.due"), m.due as i64);
    profile::set(GAME, &format!("f.{k}.rung"), m.rung as i64);
}

pub fn round_clock() -> u32 {
    profile::get(GAME, "round") as u32
}

pub fn choose_fact(
    pool: &[Fact],
    memory: &dyn Fn(&Fact) -> crate::games::spell::Memory,
    round: u32,
    avoid: &[String],
) -> Fact {
    let mut open: Vec<Fact> = pool
        .iter()
        .copied()
        .filter(|f| !avoid.contains(&f.key()))
        .collect();
    if open.is_empty() {
        open = pool.to_vec();
    }
    for i in (1..open.len()).rev() {
        open.swap(i, rand::gen_range(0, i + 1));
    }
    let mut due: Vec<Fact> = open
        .iter()
        .copied()
        .filter(|f| {
            let m = memory(f);
            m.seen() && round >= m.due
        })
        .collect();
    due.sort_by_key(|f| memory(f).due);
    if let Some(f) = due.into_iter().take(MAX_DUE).next() {
        return f;
    }
    if let Some(f) = open.iter().find(|f| !memory(f).seen()) {
        return *f;
    }
    open.sort_by_key(|f| memory(f).at);
    open[0]
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Form {
    Product,
    MissingFactor,
}

#[derive(Clone, Debug)]
pub struct Question {
    pub fact: Fact,
    pub a: i32,
    pub b: i32,
    pub form: Form,
    pub answer: i32,
    pub options: Vec<i32>,
    pub stage: usize,
}

pub fn missing_chance(level: usize, stage: usize) -> f32 {
    match (level, stage) {
        (_, s) if s < 2 => 0.0,
        (l, _) if l < 3 => 0.0,
        (MIXED, _) => 0.4,
        _ => 0.3,
    }
}

pub fn decoys(q_a: i32, q_b: i32, form: Form, stage: usize, level: usize) -> Vec<i32> {
    let answer = match form {
        Form::Product => q_a * q_b,
        Form::MissingFactor => q_b,
    };
    let mut pool: Vec<i32> = vec![];
    match form {
        Form::Product => {
            let near = [
                q_a * (q_b - 1),
                q_a * (q_b + 1),
                (q_a - 1) * q_b,
                (q_a + 1) * q_b,
            ];
            if stage == 0 && level < 3 {
                pool.extend([
                    answer - 3,
                    answer + 3,
                    answer + 2,
                    answer - 2,
                    answer + 1,
                    answer - 1,
                ]);
            } else {
                pool.extend(near);
                pool.extend([answer + 10, answer - 10, answer + q_a + 1]);
            }
        }
        Form::MissingFactor => pool.extend([answer - 1, answer + 1, answer - 2, answer + 2]),
    }
    pool.retain(|v| *v >= 2 && *v != answer);
    pool.sort_unstable();
    pool.dedup();
    pool
}

impl Question {
    pub fn new(fact: Fact, level: usize, stage: usize, flip: bool, missing: bool) -> Self {
        let (a, b) = if flip {
            (fact.other, fact.table)
        } else {
            (fact.table, fact.other)
        };
        let form = if missing {
            Form::MissingFactor
        } else {
            Form::Product
        };
        let answer = match form {
            Form::Product => a * b,
            Form::MissingFactor => b,
        };
        let mut pool = decoys(a, b, form, stage, level);
        let mut picks = vec![];
        while picks.len() < OPTIONS - 1 && !pool.is_empty() {
            picks.push(pool.swap_remove(rand::gen_range(0, pool.len())));
        }
        let mut n = 1;
        while picks.len() < OPTIONS - 1 {
            let v = answer + n;
            if v != answer && !picks.contains(&v) {
                picks.push(v);
            }
            n += 1;
        }
        picks.insert(rand::gen_range(0, picks.len() + 1), answer);
        Self {
            fact,
            a,
            b,
            form,
            answer,
            options: picks,
            stage,
        }
    }

    pub fn product(&self) -> i32 {
        self.a * self.b
    }

    pub fn slot_of_answer(&self) -> usize {
        self.options
            .iter()
            .position(|v| *v == self.answer)
            .unwrap_or(0)
    }

    pub fn prompt(&self) -> String {
        match self.form {
            Form::Product => format!("{} × {} = ?", self.a, self.b),
            Form::MissingFactor => format!("{} × ? = {}", self.a, self.product()),
        }
    }

    pub fn voice(&self) -> Say {
        match self.form {
            Form::Product => Say::Times(TimesLine::Ask, self.a as u8, self.b as u8),
            Form::MissingFactor => {
                Say::Times(TimesLine::Missing, self.a as u8, self.product() as u8)
            }
        }
    }

    pub fn answer_voice(&self) -> Say {
        Say::Times(TimesLine::Answer, self.a as u8, self.b as u8)
    }

    pub fn rows(&self) -> usize {
        self.a as usize
    }

    pub fn cols(&self) -> usize {
        self.b as usize
    }
}

pub fn stage_for(level: usize) -> usize {
    let b = mastery::band(GAME, level);
    let s: usize = match b.answers {
        0..=4 => 0,
        5..=11 => 1,
        _ => 2,
    };
    if b.answers >= 3 && b.accuracy < 0.6 {
        s.saturating_sub(1)
    } else if level == MIXED {
        s.max(1)
    } else {
        s
    }
}

pub fn bloomed(level: usize) -> bool {
    profile::get(GAME, &format!("bed.{level}")) > 0
}

pub fn beds_bloomed() -> usize {
    (0..LEVELS).filter(|l| bloomed(*l)).count()
}

pub fn unlocked_level() -> usize {
    (profile::unlocked(GAME) as usize).min(LEVELS - 1)
}

pub fn bed_color(level: usize) -> usize {
    level % BED_COLORS
}

pub fn stars_score(s: i64) -> String {
    format!("{s} stars")
}

pub fn plot(rows: usize, cols: usize, i: usize, j: usize) -> Vec3 {
    vec3(
        (j as f32 - (cols as f32 - 1.0) / 2.0) * SPACING,
        0.0,
        (i as f32 - (rows as f32 - 1.0) / 2.0) * SPACING,
    )
}

pub fn flower_hue(level: usize, i: usize, j: usize) -> f32 {
    [0.0, 0.14, 0.9, 0.58, 0.76, 0.08][flower_color(level, i, j)]
}

pub fn flower_color(level: usize, i: usize, j: usize) -> usize {
    if level == MIXED {
        (i * 2 + j) % BED_COLORS
    } else {
        bed_color(level)
    }
}

pub fn play_region(view: &View) -> Rect {
    let top = hud_bottom(view) + 130.0 * ui_scale(view);
    let bottom = seed_rects(view)[0].y - 24.0;
    let f = view.full();
    Rect::new(f.x + 40.0, top, f.w - 80.0, (bottom - top).max(120.0))
}

pub fn seed_size(view: &View) -> Vec2 {
    let h = (view.h * 0.25).clamp(170.0, 280.0);
    vec2(h * 0.8, h)
}

pub fn seed_rects(view: &View) -> Vec<Rect> {
    let s = seed_size(view);
    let gap = (view.w * 0.04).clamp(24.0, 50.0);
    let total = s.x * OPTIONS as f32 + gap * (OPTIONS as f32 - 1.0);
    let left = (view.w - total) / 2.0;
    (0..OPTIONS)
        .map(|i| Rect::new(left + i as f32 * (s.x + gap), view.h - s.y - 36.0, s.x, s.y))
        .collect()
}

pub fn prompt_pos(view: &View) -> Vec2 {
    vec2(view.w / 2.0, hud_bottom(view) + 34.0 * ui_scale(view))
}

pub fn prompt_size(view: &View) -> f32 {
    74.0 * ui_scale(view)
}

pub fn prompt_rect(view: &View) -> Rect {
    let c = prompt_pos(view);
    Rect::new(c.x - 260.0, c.y - 50.0, 520.0, 100.0)
}

pub fn beds_button(view: &View) -> Rect {
    let k = ui_scale(view);
    Rect::new(view.w - 30.0 - 160.0 * k, 78.0, 160.0 * k, 46.0 * k)
}

pub fn bed_tiles(view: &View) -> Vec<Rect> {
    let cols = if tall(view) { 3 } else { 4 };
    let rows = LEVELS.div_ceil(cols);
    let gap = 20.0;
    let top = 150.0;
    let bottom = view.h - 170.0;
    let w = ((view.w - 80.0 - gap * (cols as f32 - 1.0)) / cols as f32).min(230.0);
    let h = ((bottom - top - gap * (rows as f32 - 1.0)) / rows as f32).min(220.0);
    let left = (view.w - w * cols as f32 - gap * (cols as f32 - 1.0)) / 2.0;
    let up = top + (bottom - top - h * rows as f32 - gap * (rows as f32 - 1.0)) / 2.0;
    (0..LEVELS)
        .map(|i| {
            Rect::new(
                left + (i % cols) as f32 * (w + gap),
                up + (i / cols) as f32 * (h + gap),
                w,
                h,
            )
        })
        .collect()
}

pub fn play_button(view: &View) -> Rect {
    Rect::new(view.w / 2.0 - 170.0, view.h - 130.0, 340.0, 100.0)
}

pub fn overview_cam(view: &View) -> Cam {
    let f = view.full();
    let tall_view = f.h > f.w * 1.15;
    let fov = if tall_view {
        FOV * (f.h / f.w).min(1.6)
    } else {
        FOV
    };
    let dist = if tall_view { 24.0 } else { 17.0 };
    Cam {
        target: Vec3::ZERO,
        eye: vec3(0.0, 1.05f32.sin(), 1.05f32.cos()) * dist,
        fov,
        aspect: f.w / f.h.max(1.0),
    }
}

pub fn ground_at(cam: &Cam, view: &View, screen: Vec2) -> Vec3 {
    let f = view.full();
    let nx = (screen.x - f.x) / f.w * 2.0 - 1.0;
    let ny = 1.0 - (screen.y - f.y) / f.h * 2.0;
    let half = (cam.fov / 2.0).tan();
    let dir = cam.forward() + cam.right() * nx * half * cam.aspect + cam.up() * ny * half;
    let t = -cam.eye.y / dir.y;
    cam.eye + dir * t
}

pub fn fit_cam(view: &View, rows: f32, cols: f32, push: f32) -> Cam {
    let region = play_region(view);
    let f = view.full();
    let aspect = f.w / f.h.max(1.0);
    let tall_view = f.h > f.w * 1.15;
    let fov = if tall_view {
        FOV * (f.h / f.w).min(1.6)
    } else {
        FOV
    };
    let corners = |rows: f32, cols: f32| {
        let hw = cols * SPACING / 2.0 + 1.0;
        let hd = rows * SPACING / 2.0 + 0.8;
        [
            vec3(-hw, 0.0, -hd),
            vec3(hw, 0.0, -hd),
            vec3(-hw, 1.5, -hd),
            vec3(hw, 1.5, -hd),
            vec3(-hw, 0.0, hd),
            vec3(hw, 0.0, hd),
        ]
    };
    let mut cam = Cam {
        target: Vec3::ZERO,
        eye: vec3(0.0, PITCH.sin(), PITCH.cos()) * 12.0,
        fov,
        aspect,
    };
    let (mut lo, mut hi) = (3.0f32, 120.0f32);
    for _ in 0..24 {
        let mid = (lo + hi) / 2.0;
        cam.eye = cam.target + vec3(0.0, PITCH.sin(), PITCH.cos()) * mid;
        let mut min = Vec2::splat(f32::MAX);
        let mut max = Vec2::splat(f32::MIN);
        for c in corners(rows, cols) {
            if let Some(p) = cam.project(view, c) {
                min = min.min(p);
                max = max.max(p);
            }
        }
        if max.x - min.x > region.w || max.y - min.y > region.h {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    let dist = hi / push.max(0.5);
    for _ in 0..4 {
        cam.eye = cam.target + vec3(0.0, PITCH.sin(), PITCH.cos()) * dist;
        let mut min = Vec2::splat(f32::MAX);
        let mut max = Vec2::splat(f32::MIN);
        for c in corners(rows, cols) {
            if let Some(p) = cam.project(view, c) {
                min = min.min(p);
                max = max.max(p);
            }
        }
        let want = region.center();
        let have = (min + max) / 2.0;
        let d = (have - want) / cam.px_per_unit(view, cam.target);
        cam.target += cam.right() * d.x - cam.up() * d.y;
    }
    cam.eye = cam.target + vec3(0.0, PITCH.sin(), PITCH.cos()) * dist;
    cam
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Mode {
    Play,
    Beds,
    Report(f32),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Report {
    Round { stars: u8 },
    Bloom { level: usize, stars: u8 },
    Cleared { rank: Option<usize> },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pick {
    Right,
    Wrong,
    Reveal,
    Again,
    Ignored,
}

pub struct Garden {
    pub mode: Mode,
    pub level: usize,
    pub stage: usize,
    pub run: u32,
    pub stage_run: u32,
    pub slips: u32,
    pub asked: u32,
    pub q: Question,
    pub tried: [bool; OPTIONS],
    pub wrong: u32,
    pub solved: Option<f32>,
    pub ask_t: f32,
    pub grow_age: f32,
    pub ticked: usize,
    pub shown: f32,
    pub wilt: [f32; OPTIONS],
    pub press: [Bump; OPTIONS],
    pub prompt_bump: Bump,
    pub run_bump: Bump,
    pub cam_rows: f32,
    pub cam_cols: f32,
    pub push: f32,
    pub cam_mix: f32,
    pub used: Vec<String>,
    pub first_time: bool,
    pub report: Option<Report>,
    pub summary: String,
    pub session: Session,
    pub shake: Shake,
    pub fx: Particles,
    pub popups: Popups,
    pub t: f32,
    pub bloomed_now: Option<f32>,
    pub rank: Option<usize>,
}

fn dummy_question() -> Question {
    Question::new(Fact { table: 2, other: 2 }, 0, 0, false, false)
}

impl Garden {
    pub fn new() -> Self {
        let level = (profile::level(GAME) as usize).min(unlocked_level());
        let mut g = Self {
            mode: Mode::Play,
            level,
            stage: 0,
            run: 0,
            stage_run: 0,
            slips: 0,
            asked: 0,
            q: dummy_question(),
            tried: [false; OPTIONS],
            wrong: 0,
            solved: None,
            ask_t: 0.0,
            grow_age: 0.0,
            ticked: 0,
            shown: 0.0,
            wilt: [0.0; OPTIONS],
            press: Default::default(),
            prompt_bump: Bump::default(),
            run_bump: Bump::default(),
            cam_rows: 3.0,
            cam_cols: 3.0,
            push: 1.0,
            cam_mix: 0.0,
            used: vec![],
            first_time: profile::get(GAME, "played") == 0,
            report: None,
            summary: String::new(),
            session: Session::default(),
            shake: Shake::default(),
            fx: Particles::default(),
            popups: Popups::default(),
            t: 0.0,
            bloomed_now: None,
            rank: None,
        };
        g.start_level(level);
        if g.first_time {
            sfx::say(Say::GardenIntro);
        }
        g
    }

    pub fn start_level(&mut self, level: usize) {
        self.level = level.min(LEVELS - 1);
        self.stage = stage_for(self.level);
        self.run = 0;
        self.stage_run = 0;
        self.slips = 0;
        self.asked = 0;
        self.used.clear();
        self.session.reset();
        profile::set_level(GAME, self.level as u32);
        self.mode = Mode::Play;
        self.next_question();
        self.cam_rows = self.q.rows() as f32;
        self.cam_cols = self.q.cols() as f32;
    }

    pub fn next_question(&mut self) {
        let pool = facts_of(self.level);
        let fact = choose_fact(&pool, &memory_of, round_clock(), &self.used);
        self.used.push(fact.key());
        let flip = rand::gen_range(0, 2) == 1;
        let missing = rand::gen_range(0.0f32, 1.0) < missing_chance(self.level, self.stage);
        self.q = Question::new(fact, self.level, self.stage, flip, missing);
        self.tried = [false; OPTIONS];
        self.wrong = 0;
        self.solved = None;
        self.ask_t = 0.0;
        self.grow_age = 0.0;
        self.ticked = 0;
        self.wilt = [0.0; OPTIONS];
        self.prompt_bump.kick();
        if self.stage == 2 {
            self.shown = 0.0;
        }
        self.session.start_question();
        sfx::prefetch_say(&[self.q.voice(), self.q.answer_voice()]);
        sfx::say(self.q.voice());
    }

    pub fn array_target(&self) -> f32 {
        if self.solved.is_some() || self.wrong > 0 {
            return 1.0;
        }
        match self.stage {
            0 => 1.0,
            1 if self.ask_t < FADE_AFTER => 1.0,
            _ => 0.0,
        }
    }

    pub fn grow_step(&self) -> f32 {
        let n = (self.q.rows() * self.q.cols()) as f32;
        (1.1 / n).min(0.05)
    }

    pub fn growth(&self, index: usize) -> f32 {
        ((self.grow_age - index as f32 * self.grow_step()) / GROW_TIME).clamp(0.0, 1.0)
    }

    pub fn bloom(&self, index: usize) -> f32 {
        ease_out_back(self.growth(index)).max(0.0) * self.shown
    }

    pub fn overview(&self) -> bool {
        match self.mode {
            Mode::Beds => true,
            Mode::Report(_) => matches!(self.report, Some(Report::Cleared { .. })),
            Mode::Play => false,
        }
    }

    pub fn bloom_level(&self) -> Option<usize> {
        match self.report {
            Some(Report::Bloom { level, .. }) => Some(level),
            _ => None,
        }
    }

    pub fn celebration_length(&self) -> f32 {
        (self.q.rows() * self.q.cols()) as f32 * self.grow_step() + 1.3
    }

    pub fn pick(&mut self, slot: usize) -> Pick {
        if self.solved.is_some() || slot >= OPTIONS {
            return Pick::Ignored;
        }
        let right = self.q.options[slot] == self.q.answer;
        if right {
            let first = self.wrong == 0;
            let answer = self.session.answer(first, false);
            mastery::record(GAME, self.level, answer, TARGET_SECS);
            self.finish_fact(first);
            self.solved = Some(0.0);
            self.grow_age = 0.0;
            self.ticked = 0;
            self.press[slot].kick();
            return Pick::Right;
        }
        if self.tried[slot] {
            return Pick::Again;
        }
        self.tried[slot] = true;
        self.wrong += 1;
        self.wilt[slot] = 1.0;
        self.press[slot].kick();
        if self.wrong == 1 {
            self.grow_age = 0.0;
        }
        if self.wrong >= 2 {
            Pick::Reveal
        } else {
            Pick::Wrong
        }
    }

    fn finish_fact(&mut self, first: bool) {
        self.first_time = false;
        profile::set(GAME, "played", 1);
        let round = round_clock() + 1;
        profile::set(GAME, "round", round as i64);
        remember(&self.q.fact, memory_of(&self.q.fact).after(round, first));
        self.asked += 1;
        if first {
            self.run += 1;
            self.stage_run += 1;
            self.slips = 0;
            self.run_bump.kick();
        } else {
            self.run = 0;
            self.stage_run = 0;
            self.slips += 1;
        }
    }

    fn stage_move(&mut self) -> StageMove {
        if self.stage == 2 && self.run >= MASTER_RUN {
            return StageMove::Bloom;
        }
        if self.stage < 2 && self.stage_run >= STAGE_UP_RUN {
            self.stage += 1;
            self.stage_run = 0;
            return StageMove::Up;
        }
        if self.slips >= SLIPS_DOWN {
            self.slips = 0;
            if self.stage > 0 {
                self.stage -= 1;
                self.stage_run = 0;
                return StageMove::Down;
            }
            if self.level > 0 {
                return StageMove::Back;
            }
        }
        StageMove::Same
    }

    fn finish_round(&mut self) -> u8 {
        let stars = self.session.stars(TARGET_SECS);
        profile::award_stars(GAME, self.level as u32, stars);
        self.summary = format!(
            "{} of {} right first time",
            self.session.first_try, self.session.answers
        );
        self.session.reset();
        self.asked = 0;
        self.used.clear();
        stars
    }

    fn advance(&mut self) {
        match self.stage_move() {
            StageMove::Bloom => {
                let level = self.level;
                profile::set(GAME, &format!("bed.{level}"), 1);
                let stars = self.finish_round();
                profile::award_stars(GAME, level as u32, 3.min(stars + 1));
                if level + 1 < LEVELS {
                    profile::unlock(GAME, level as u32 + 1);
                }
                self.bloomed_now = Some(0.0);
                if beds_bloomed() == LEVELS && profile::get(GAME, "cleared") == 0 {
                    profile::set(GAME, "cleared", 1);
                    self.rank = profile::submit(
                        GAME,
                        profile::total_stars(GAME) as i64,
                        profile::Better::Higher,
                    );
                    self.report = Some(Report::Cleared { rank: self.rank });
                } else {
                    self.report = Some(Report::Bloom { level, stars });
                }
                let next = (level + 1).min(LEVELS - 1);
                self.start_level(next);
                self.mode = Mode::Report(0.0);
            }
            StageMove::Back => {
                let l = self.level - 1;
                self.start_level(l);
            }
            StageMove::Up | StageMove::Down | StageMove::Same => {
                if self.asked >= QUESTIONS {
                    let stars = self.finish_round();
                    self.report = Some(Report::Round { stars });
                    self.mode = Mode::Report(0.0);
                }
                self.next_question();
            }
        }
    }

    pub fn can_pour(&self) -> f32 {
        match self.solved {
            Some(t) => {
                let end = self.celebration_length();
                let into = (t / 0.5).clamp(0.0, 1.0);
                let out = ((end - t) / 0.45).clamp(0.0, 1.0);
                ease_out_cubic(into.min(out))
            }
            None => 0.0,
        }
    }

    pub fn can_rest(&self) -> Vec3 {
        vec3(
            self.cam_cols * SPACING / 2.0 + 1.3,
            0.75,
            self.cam_rows * SPACING / 2.0 + 0.2,
        )
    }

    pub fn can_pos(&self) -> Vec3 {
        let k = self.can_pour();
        let rest = self.can_rest();
        let over = vec3(self.cam_cols * SPACING * 0.1 + 0.9, 2.3, 0.0);
        rest.lerp(over, k)
    }

    pub fn spout(&self) -> Vec3 {
        self.can_pos() + vec3(-0.75, -0.3, 0.0)
    }

    pub fn view_cam(&self, view: &View) -> Cam {
        let play = self.camera(view);
        let top = overview_cam(view);
        let k = self.cam_mix * self.cam_mix * (3.0 - 2.0 * self.cam_mix);
        Cam {
            target: play.target.lerp(top.target, k),
            eye: play.eye.lerp(top.eye, k),
            fov: play.fov + (top.fov - play.fov) * k,
            aspect: play.aspect,
        }
    }

    pub fn camera(&self, view: &View) -> Cam {
        fit_cam(view, self.cam_rows, self.cam_cols, self.push)
    }

    pub fn flower_world(&self, i: usize, j: usize) -> Vec3 {
        plot(self.q.rows(), self.q.cols(), i, j)
    }

    pub fn react(&mut self, view: &View, slot: usize, p: Pick) {
        let gold = Color::from_rgba(255, 220, 70, 255);
        let orange = Color::from_rgba(255, 170, 90, 255);
        let at = seed_rects(view)[slot.min(OPTIONS - 1)].center();
        match p {
            Pick::Right => {
                sfx::play(Sfx::Right);
                sfx::play_step(Sfx::Coin, self.run.saturating_sub(1) as usize);
                self.fx.sparkle(at, gold);
                self.fx.emit(
                    at,
                    &[gold, WHITE, hsv(0.9, 0.5, 1.0), hsv(0.55, 0.5, 1.0)],
                    Burst {
                        count: 26,
                        speed: (200.0, 520.0),
                        shape: Shape::Star,
                        size: (6.0, 12.0),
                        angle: -std::f32::consts::FRAC_PI_2,
                        spread: 2.4,
                        ..Default::default()
                    },
                );
                let cam = self.view_cam(view);
                let top = cam
                    .project(view, vec3(0.0, 1.2, 0.0))
                    .unwrap_or(view.center());
                self.fx.sparkle(top, gold);
                sfx::say(self.q.answer_voice());
                self.push = 1.05;
                if self.run > 0 && self.run.is_multiple_of(5) {
                    feel::impact_tinted(Impact::Medium, gold);
                    self.shake.add(0.2);
                }
                let word = if self.wrong == 0 {
                    PRAISE[rand::gen_range(0, PRAISE.len())]
                } else {
                    "That's it!"
                };
                self.popups
                    .add(vec2(view.w / 2.0, view.h * 0.5), word, gold, 80.0);
            }
            Pick::Wrong | Pick::Reveal => {
                sfx::play(Sfx::Wrong);
                self.shake.add(0.22);
                self.popups
                    .add(at - vec2(0.0, 150.0), "Count and try again", orange, 40.0);
                if self.slips + 1 >= SLIPS_DOWN || self.wrong >= 2 {
                    sfx::say(Say::Steady);
                }
            }
            Pick::Again | Pick::Ignored => {}
        }
    }

    fn after_celebration(&mut self, view: &View) {
        let before = beds_bloomed();
        self.advance();
        if beds_bloomed() > before {
            sfx::say(Say::Bloom);
            sfx::play(Sfx::Win);
            feel::impact_tinted(Impact::Large, Color::from_rgba(255, 230, 120, 255));
            self.fx.confetti(view);
        }
    }

    fn update_play(&mut self, input: &Input, view: &View, dt: f32) {
        self.ask_t += dt;
        self.grow_age += dt;
        if self.solved.is_none() {
            self.session.tick(dt);
        }
        let target = self.array_target();
        let was = self.shown;
        self.shown = approach(
            self.shown,
            target,
            if target > self.shown { 5.0 } else { 6.0 },
            dt,
        );
        if target > 0.5 && was < 0.3 && self.shown >= 0.3 && self.stage > 0 && self.solved.is_none()
        {
            self.grow_age = 0.0;
        }
        if let Some(t) = self.solved {
            let t = t + dt;
            self.solved = Some(t);
            let n = self.q.rows() * self.q.cols();
            let due = ((t / self.grow_step()) as usize).min(n);
            if self.can_pour() > 0.9 {
                let cam = self.view_cam(view);
                if let Some(at) = cam.project(view, self.spout()) {
                    let blue = [Color::from_rgba(120, 190, 255, 255), WHITE];
                    self.fx.emit(
                        at,
                        &blue,
                        Burst {
                            count: 2,
                            speed: (40.0, 120.0),
                            size: (4.0, 7.0),
                            life: (0.35, 0.6),
                            gravity: 900.0,
                            angle: std::f32::consts::PI,
                            spread: 0.8,
                            ..Default::default()
                        },
                    );
                }
            }
            while self.ticked < due {
                self.ticked += 1;
                let cols = self.q.cols();
                let (i, j) = ((self.ticked - 1) / cols, (self.ticked - 1) % cols);
                if n < 40 || self.ticked.is_multiple_of(3) {
                    let cam = self.view_cam(view);
                    let w = self.flower_world(i, j) + vec3(0.0, 0.9, 0.0);
                    if let Some(at) = cam.project(view, w) {
                        self.fx
                            .sparkle(at, hsv(flower_hue(self.level, i, j), 0.5, 1.0));
                    }
                }
                if self.ticked % 2 == 1 || n < 30 {
                    sfx::play_step(Sfx::Pop, self.ticked * 8 / n.max(1));
                }
            }
            if t >= self.celebration_length() {
                self.after_celebration(view);
            }
            return;
        }
        if input.pressed && beds_button(view).contains(input.pos) && !self.first_time {
            self.mode = Mode::Beds;
        } else if input.pressed && prompt_rect(view).contains(input.pos) {
            sfx::say(self.q.voice());
            self.prompt_bump.kick();
        } else if input.pressed {
            if let Some(slot) = seed_rects(view).iter().position(|r| r.contains(input.pos)) {
                let p = self.pick(slot);
                self.react(view, slot, p);
            }
        }
    }

    fn update_beds(&mut self, input: &Input, view: &View) {
        if !input.pressed {
            return;
        }
        if play_button(view).contains(input.pos) {
            self.mode = Mode::Play;
            return;
        }
        let open = unlocked_level();
        if let Some(i) = bed_tiles(view)
            .iter()
            .position(|r| r.contains(input.pos))
            .filter(|i| *i <= open)
        {
            sfx::play(Sfx::Tick);
            self.start_level(i);
        }
    }

    fn update_report(&mut self, input: &Input, age: f32, dt: f32) {
        self.mode = Mode::Report(age + dt);
        if input.pressed && age > 0.8 {
            self.report = None;
            self.mode = Mode::Play;
            self.session.start_question();
        }
    }

    pub fn goal_text(&self) -> String {
        if self.stage == 2 {
            let left = MASTER_RUN.saturating_sub(self.run);
            format!("{left} more right in a row to bloom a new bed")
        } else {
            format!(
                "{} right in a row to grow harder",
                STAGE_UP_RUN.saturating_sub(self.stage_run)
            )
        }
    }

    pub fn cleared(&self) -> bool {
        matches!(self.report, Some(Report::Cleared { .. }))
    }
}

enum StageMove {
    Same,
    Up,
    Down,
    Back,
    Bloom,
}

impl Default for Garden {
    fn default() -> Self {
        Self::new()
    }
}

impl Game for Garden {
    fn update(&mut self, input: &Input, view: &View, dt: f32) {
        self.t += dt;
        self.prompt_bump.update(dt);
        self.run_bump.update(dt);
        for b in &mut self.press {
            b.update(dt);
        }
        for w in &mut self.wilt {
            *w = (*w - dt * 0.9).max(0.0);
        }
        self.push = approach(self.push, 1.0, 3.0, dt);
        self.cam_mix = approach(self.cam_mix, self.overview() as i32 as f32, 4.5, dt);
        let (r, c) = (self.q.rows() as f32, self.q.cols() as f32);
        self.cam_rows = approach(self.cam_rows, r, 5.0, dt);
        self.cam_cols = approach(self.cam_cols, c, 5.0, dt);
        if let Some(t) = &mut self.bloomed_now {
            *t += dt;
        }
        match self.mode {
            Mode::Beds => self.update_beds(input, view),
            Mode::Report(age) => self.update_report(input, age, dt),
            Mode::Play => self.update_play(input, view, dt),
        }
        self.shake.update(dt);
        self.fx.update(dt);
        self.popups.update(dt);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::View;

    fn fresh() -> Garden {
        crate::save::reset_for_test();
        profile::create("Mia", 0);
        rand::srand(3);
        Garden::new()
    }

    fn answer(g: &mut Garden, right: bool) {
        let v = View::ipad_landscape();
        let slot = if right {
            g.q.slot_of_answer()
        } else {
            (0..OPTIONS)
                .find(|i| *i != g.q.slot_of_answer() && !g.tried[*i])
                .unwrap()
        };
        let p = g.pick(slot);
        g.react(&v, slot, p);
        if right {
            let t = g.celebration_length() + 0.1;
            let mut s = 0.0;
            while s < t + 0.2 && g.solved.is_some() {
                g.update(&Input::default(), &v, 1.0 / 30.0);
                s += 1.0 / 30.0;
            }
        }
    }

    fn dismiss(g: &mut Garden) {
        let v = View::ipad_landscape();
        for _ in 0..60 {
            g.update(&Input::default(), &v, 1.0 / 30.0);
        }
        if matches!(g.mode, Mode::Report(_)) {
            g.update(&Input::press_at(vec2(10.0, 10.0)), &v, 1.0 / 30.0);
        }
    }

    #[test]
    fn tables_follow_the_teaching_order_with_a_mixed_garden_last() {
        assert_eq!(TABLES, [2, 10, 5, 3, 4, 8, 6, 9, 7, 11, 12]);
        assert_eq!(LEVELS, TABLES.len() + 1);
        assert_eq!(tables_of(MIXED).len(), 11);
        assert_eq!(level_name(0), "Times 2");
    }

    #[test]
    fn every_level_has_a_big_pool_and_mixed_is_biggest() {
        for l in 0..MIXED {
            assert!(pool_size(l) >= 20, "{l}");
        }
        assert!(pool_size(MIXED) > 100);
    }

    #[test]
    fn a_round_never_repeats_a_fact() {
        for level in [0, 3, MIXED] {
            for seed in 0..20 {
                crate::save::reset_for_test();
                profile::create("Mia", 0);
                rand::srand(seed);
                let mut g = Garden::new();
                g.start_level(level);
                let mut seen = vec![g.q.fact.key()];
                for _ in 1..QUESTIONS {
                    answer(&mut g, true);
                    if g.asked == 0 {
                        break;
                    }
                    assert!(!seen.contains(&g.q.fact.key()), "level {level} seed {seed}");
                    seen.push(g.q.fact.key());
                }
            }
        }
    }

    #[test]
    fn questions_have_one_right_answer_and_distinct_options() {
        rand::srand(9);
        for level in 0..LEVELS {
            for stage in 0..3 {
                for f in facts_of(level) {
                    for flip in [false, true] {
                        for missing in [false, true] {
                            let q = Question::new(f, level, stage, flip, missing);
                            assert_eq!(q.options.len(), OPTIONS);
                            assert_eq!(q.options.iter().filter(|v| **v == q.answer).count(), 1);
                            let mut s = q.options.clone();
                            s.sort_unstable();
                            s.dedup();
                            assert_eq!(s.len(), OPTIONS, "{q:?}");
                            assert!(q.options.iter().all(|v| *v >= 2));
                            assert_eq!(q.a * q.b, q.product());
                            assert!(q.a <= 12 && q.b <= 12 && q.product() <= 144);
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn the_number_of_options_never_grows_with_difficulty() {
        rand::srand(1);
        for level in 0..LEVELS {
            for stage in 0..3 {
                let q = Question::new(facts_of(level)[0], level, stage, false, false);
                assert_eq!(q.options.len(), 3);
            }
        }
    }

    #[test]
    fn harder_stages_use_near_miss_decoys_and_missing_numbers_come_late() {
        let easy = decoys(7, 8, Form::Product, 0, 0);
        let hard = decoys(7, 8, Form::Product, 2, 6);
        assert!(!easy.contains(&(7 * 9)));
        assert!(hard.contains(&(7 * 9)) && hard.contains(&(6 * 8)));
        assert_eq!(missing_chance(0, 2), 0.0);
        assert_eq!(missing_chance(5, 1), 0.0);
        assert!(missing_chance(5, 2) > 0.0);
        assert!(missing_chance(MIXED, 2) > missing_chance(5, 2));
    }

    #[test]
    fn scaffolding_steps_away_as_a_child_answers_cleanly() {
        let mut g = fresh();
        assert_eq!(g.stage, 0);
        assert_eq!(g.array_target(), 1.0);
        for _ in 0..STAGE_UP_RUN {
            answer(&mut g, true);
        }
        assert_eq!(g.stage, 1);
        g.ask_t = FADE_AFTER + 1.0;
        assert_eq!(g.array_target(), 0.0);
        for _ in 0..STAGE_UP_RUN {
            answer(&mut g, true);
        }
        assert_eq!(g.stage, 2);
        assert_eq!(g.array_target(), 0.0);
    }

    #[test]
    fn a_wrong_pick_shows_the_array_to_count() {
        let mut g = fresh();
        g.stage = 2;
        g.next_question();
        assert_eq!(g.array_target(), 0.0);
        answer(&mut g, false);
        assert_eq!(g.array_target(), 1.0);
        assert_eq!(g.wrong, 1);
    }

    #[test]
    fn a_second_wrong_pick_reveals_and_a_repeat_is_gentle() {
        let mut g = fresh();
        let wrong: Vec<usize> = (0..OPTIONS)
            .filter(|i| *i != g.q.slot_of_answer())
            .collect();
        assert_eq!(g.pick(wrong[0]), Pick::Wrong);
        assert_eq!(g.pick(wrong[0]), Pick::Again);
        assert_eq!(g.pick(wrong[1]), Pick::Reveal);
        assert_eq!(g.wrong, 2);
        assert_eq!(g.pick(g.q.slot_of_answer()), Pick::Right);
    }

    #[test]
    fn ten_clean_answers_at_the_top_stage_bloom_the_bed_and_open_the_next_table() {
        let mut g = fresh();
        let mut guard = 0;
        while !bloomed(0) && guard < 40 {
            answer(&mut g, true);
            if matches!(g.mode, Mode::Report(_)) {
                dismiss(&mut g);
            }
            guard += 1;
        }
        assert!(bloomed(0), "bed 0 should bloom, guard {guard}");
        assert_eq!(profile::unlocked(GAME), 1);
        assert_eq!(g.level, 1);
        assert!(profile::stars(GAME, 0) >= 2);
    }

    #[test]
    fn a_strong_player_is_not_held_in_the_baby_levels() {
        let mut g = fresh();
        for l in 0..4 {
            let mut guard = 0;
            while !bloomed(l) && guard < 60 {
                answer(&mut g, true);
                if matches!(g.mode, Mode::Report(_)) {
                    dismiss(&mut g);
                }
                guard += 1;
            }
            assert!(bloomed(l), "table {l}");
            assert!(guard <= 20, "table {l} took {guard} answers");
        }
        assert_eq!(g.level, 4);
    }

    #[test]
    fn a_returning_child_resumes_at_the_stage_they_earned() {
        let mut g = fresh();
        for _ in 0..14 {
            answer(&mut g, true);
            if matches!(g.mode, Mode::Report(_)) {
                dismiss(&mut g);
            }
        }
        profile::set_level(GAME, 0);
        let g2 = Garden::new();
        assert_eq!(g2.level, 0);
        assert!(g2.stage >= 1);
    }

    #[test]
    fn a_struggling_player_is_eased_back_a_stage_then_a_table_and_never_stuck() {
        let mut g = fresh();
        g.start_level(3);
        profile::unlock(GAME, 3);
        g.stage = 2;
        for _ in 0..SLIPS_DOWN {
            answer(&mut g, false);
            answer(&mut g, true);
        }
        assert_eq!(g.stage, 1);
        for _ in 0..SLIPS_DOWN {
            answer(&mut g, false);
            answer(&mut g, true);
        }
        assert_eq!(g.stage, 0);
        for _ in 0..SLIPS_DOWN {
            answer(&mut g, false);
            answer(&mut g, true);
            if matches!(g.mode, Mode::Report(_)) {
                dismiss(&mut g);
            }
        }
        assert_eq!(g.level, 2);
    }

    #[test]
    fn a_weak_player_who_guesses_half_the_time_still_finishes_rounds_and_earns_stars() {
        let mut g = fresh();
        let mut rounds = 0;
        for i in 0..80 {
            answer(&mut g, i % 2 == 0 && false || i % 3 != 0);
            answer(&mut g, true);
            if matches!(g.mode, Mode::Report(_)) {
                rounds += 1;
                dismiss(&mut g);
            }
        }
        assert!(rounds >= 2, "{rounds}");
        assert!(profile::total_stars(GAME) >= 1);
    }

    #[test]
    fn a_missed_fact_comes_back_before_a_mastered_one() {
        let mut g = fresh();
        g.start_level(0);
        let missed = g.q.fact;
        answer(&mut g, false);
        answer(&mut g, true);
        assert_eq!(memory_of(&missed).rung, 0);
        let pool = facts_of(0);
        let mastered = *pool.iter().find(|f| f.key() != missed.key()).unwrap();
        remember(
            &mastered,
            crate::games::spell::Memory {
                at: 1,
                due: 0,
                rung: 0,
            }
            .after(round_clock(), true),
        );
        let later = round_clock() + crate::games::spell::GAPS[0] + 1;
        for _ in 0..30 {
            let next = choose_fact(&pool, &memory_of, later, &[]);
            assert!(memory_of(&next).seen());
            assert_ne!(next.key(), mastered.key());
        }
    }

    #[test]
    fn thirty_sessions_never_serve_the_same_opening_fact_every_time() {
        let mut openers = std::collections::HashSet::new();
        crate::save::reset_for_test();
        profile::create("Mia", 0);
        rand::srand(11);
        let mut g = Garden::new();
        g.start_level(MIXED);
        for _ in 0..30 {
            openers.insert(g.q.fact.key());
            for _ in 0..QUESTIONS {
                answer(&mut g, true);
                if matches!(g.mode, Mode::Report(_)) {
                    dismiss(&mut g);
                }
            }
            g.start_level(MIXED);
        }
        assert!(openers.len() >= 10, "{}", openers.len());
    }

    #[test]
    fn mixed_garden_asks_every_table_over_time() {
        let mut g = fresh();
        g.start_level(MIXED);
        let mut tables = std::collections::HashSet::new();
        for _ in 0..40 {
            tables.insert(g.q.fact.table.min(g.q.fact.other));
            answer(&mut g, true);
            if matches!(g.mode, Mode::Report(_)) {
                dismiss(&mut g);
            }
        }
        assert!(tables.len() >= 6, "{tables:?}");
    }

    #[test]
    fn the_prompt_and_answer_are_spoken() {
        let mut g = fresh();
        g.q = Question::new(Fact { table: 3, other: 4 }, 0, 0, false, false);
        g.next_question();
        let said = sfx::take_voice();
        assert!(matches!(said, Some(Say::Times(TimesLine::Ask, _, _))));
        let v = View::ipad_landscape();
        let slot = g.q.slot_of_answer();
        let p = g.pick(slot);
        g.react(&v, slot, p);
        assert_eq!(sfx::take_voice(), Some(g.q.answer_voice()));
        assert_eq!(Say::Times(TimesLine::Answer, 3, 4).key(), "word_ta3x4");
        assert_eq!(Say::Times(TimesLine::Missing, 3, 12).key(), "word_tm3x12");
    }

    #[test]
    fn the_camera_frames_every_bed_size_inside_the_play_region() {
        for v in [
            View::ipad_landscape(),
            View::ipad_portrait(),
            View::fit(390.0, 844.0),
        ] {
            let region = play_region(&v);
            for (r, c) in [(2usize, 2usize), (3, 4), (12, 12), (12, 2), (2, 12)] {
                let cam = fit_cam(&v, r as f32, c as f32, 1.0);
                for i in [0, r - 1] {
                    for j in [0, c - 1] {
                        let p = cam.project(&v, plot(r, c, i, j)).unwrap();
                        assert!(
                            p.x > region.x - 8.0 && p.x < region.x + region.w + 8.0,
                            "{p:?} {region:?} {r}x{c}"
                        );
                        assert!(
                            p.y > region.y - 8.0 && p.y < region.y + region.h + 8.0,
                            "{p:?} {region:?} {r}x{c}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn seeds_are_big_enough_to_touch_and_do_not_overlap() {
        for v in [
            View::ipad_landscape(),
            View::ipad_portrait(),
            View::fit(390.0, 844.0),
        ] {
            let r = seed_rects(&v);
            for w in r.windows(2) {
                assert!(w[0].right() < w[1].left());
            }
            assert!(r.iter().all(|x| x.w >= 130.0 && x.bottom() <= v.h));
        }
    }

    #[test]
    fn clearing_every_bed_opens_the_board_report() {
        let mut g = fresh();
        for l in 0..LEVELS {
            profile::unlock(GAME, l as u32);
            if l + 1 < LEVELS {
                profile::set(GAME, &format!("bed.{l}"), 1);
            }
        }
        g.start_level(MIXED);
        g.stage = 2;
        let mut guard = 0;
        while !g.cleared() && guard < 40 {
            answer(&mut g, true);
            if !g.cleared() && matches!(g.mode, Mode::Report(_)) {
                dismiss(&mut g);
            }
            guard += 1;
        }
        assert!(g.cleared());
    }
}

#[cfg(test)]
mod overview_tests {
    use super::*;

    #[test]
    fn ground_at_inverts_the_projection() {
        for v in [View::ipad_landscape(), View::ipad_portrait()] {
            let cam = overview_cam(&v);
            for t in bed_tiles(&v) {
                let w = ground_at(&cam, &v, t.center());
                let back = cam.project(&v, w).unwrap();
                assert!(back.distance(t.center()) < 0.5, "{back:?} {t:?}");
                assert!(w.y.abs() < 1e-3);
            }
        }
    }

    #[test]
    fn the_garden_overview_shows_for_the_bed_picker_and_the_finale() {
        crate::save::reset_for_test();
        let mut g = Garden::new();
        assert!(!g.overview());
        g.mode = Mode::Beds;
        assert!(g.overview());
        g.mode = Mode::Report(0.0);
        g.report = Some(Report::Round { stars: 2 });
        assert!(!g.overview());
        g.report = Some(Report::Cleared { rank: None });
        assert!(g.overview());
        g.report = Some(Report::Bloom { level: 2, stars: 3 });
        assert!(!g.overview());
        assert_eq!(g.bloom_level(), Some(2));
    }
}
