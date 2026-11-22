use crate::engine::*;
use crate::feel::{self, Impact};
use crate::fx::*;
use crate::games::garden;
use crate::games::spell::Memory;
use crate::geom::*;
use crate::learn::*;
use crate::mastery;
use crate::profile;
use crate::rng as rand;
use crate::sfx::{self, Sfx};
use glam::{vec2, Vec2};

pub const GAME: &str = "share";
pub const LEVELS: usize = 6;
pub const OPTIONS: usize = 3;
pub const RECENT: usize = 6;
pub const CELEBRATE: f32 = 1.5;
pub const SPEECH_GAP: f32 = 0.9;

fn gap_after(say: sfx::Say) -> f32 {
    match say {
        sfx::Say::ShareIntro => 3.6,
        sfx::Say::ShareEach | sfx::Say::ShareDrag => 2.6,
        _ => SPEECH_GAP,
    }
}
pub const MAX_SPOKEN: i32 = 99;
pub const ANIMALS: [&str; 5] = ["bunny", "bear", "fox", "panda", "frog"];
pub const HUD_TITLE: &str = "Level 6";
pub const HUD_GOAL: &str = "Get 4 right in a row to reach Share with friends";

pub fn level_name(level: usize) -> &'static str {
    [
        "Share with 2",
        "Share with friends",
        "Share and tell",
        "How many each",
        "How many friends",
        "Leftovers",
    ][level.min(LEVELS - 1)]
}

pub fn level_example(level: usize) -> &'static str {
    [
        "8 \u{f7} 2",
        "12 \u{f7} 3",
        "15 \u{f7} 5 = ?",
        "40 \u{f7} 4 = ?",
        "12 \u{f7} ? = 4",
        "13 \u{f7} 4 = 3 r1",
    ][level.min(LEVELS - 1)]
}

pub fn has_drag(level: usize) -> bool {
    level <= 2
}

pub fn has_choice(level: usize) -> bool {
    level >= 2
}

pub fn target_secs(level: usize) -> f32 {
    [8.0, 9.0, 12.0, 8.0, 10.0, 12.0][level.min(LEVELS - 1)]
}

pub fn stars_score(s: i64) -> String {
    format!("{s} stars")
}

pub fn divisors() -> Vec<i32> {
    let d: Vec<i32> = (0..garden::MIXED)
        .filter(|l| garden::bloomed(*l))
        .map(|l| garden::TABLES[l])
        .collect();
    if d.is_empty() {
        vec![2, 5, 10]
    } else {
        d
    }
}

pub fn pool(level: usize) -> Vec<(i32, i32)> {
    let mut out = vec![];
    match level {
        0 => {
            for each in 2..=6 {
                out.push((each * 2, 2));
            }
        }
        1 => {
            for f in 2..=5 {
                for each in 2..=5 {
                    out.push((f * each, f));
                }
            }
        }
        2 => {
            for f in 2..=5 {
                for each in 2..=6 {
                    out.push((f * each, f));
                }
            }
        }
        3 | 4 => {
            for d in divisors() {
                for each in 2..=(MAX_SPOKEN / d).min(10) {
                    out.push((d * each, d));
                }
            }
        }
        _ => {
            for f in 2..=6 {
                for t in 7..=40 {
                    if t % f != 0 {
                        out.push((t, f));
                    }
                }
            }
        }
    }
    out.sort_unstable();
    out.dedup();
    out
}

fn memory_key(level: usize, t: i32, f: i32, field: &str) -> String {
    format!("q{level}.{t}.{f}.{field}")
}

pub fn memory_of(level: usize, t: i32, f: i32) -> Memory {
    Memory {
        at: profile::get(GAME, &memory_key(level, t, f, "at")) as u32,
        due: profile::get(GAME, &memory_key(level, t, f, "due")) as u32,
        rung: profile::get(GAME, &memory_key(level, t, f, "rung")) as u32,
    }
}

fn remember(level: usize, t: i32, f: i32, m: Memory) {
    profile::set(GAME, &memory_key(level, t, f, "at"), m.at as i64);
    profile::set(GAME, &memory_key(level, t, f, "due"), m.due as i64);
    profile::set(GAME, &memory_key(level, t, f, "rung"), m.rung as i64);
}

pub fn round() -> u32 {
    profile::get(GAME, "round") as u32
}

pub fn pick_pair(level: usize, recent: &[(i32, i32)], round: u32) -> (i32, i32) {
    let all = pool(level);
    let window = RECENT.min(all.len() / 2);
    let recent = &recent[recent.len().saturating_sub(window)..];
    let mut open: Vec<(i32, i32)> = all
        .iter()
        .copied()
        .filter(|p| !recent.contains(p))
        .collect();
    if open.is_empty() {
        open = all;
    }
    let mems: Vec<Memory> = open.iter().map(|p| memory_of(level, p.0, p.1)).collect();
    let due = open
        .iter()
        .zip(&mems)
        .filter(|(_, m)| m.seen() && round >= m.due)
        .min_by_key(|(_, m)| m.due);
    if let Some((p, _)) = due {
        return *p;
    }
    let fresh: Vec<(i32, i32)> = open
        .iter()
        .zip(&mems)
        .filter(|(_, m)| !m.seen())
        .map(|(p, _)| *p)
        .collect();
    if !fresh.is_empty() {
        return fresh[rand::gen_range(0, fresh.len())];
    }
    *open
        .iter()
        .zip(&mems)
        .min_by_key(|(_, m)| m.at)
        .map(|(p, _)| p)
        .unwrap_or(&open[0])
}

#[derive(Clone, Debug)]
pub struct Question {
    pub level: usize,
    pub total: i32,
    pub friends: i32,
    pub options: Vec<i32>,
    pub seed: u32,
}

impl Question {
    pub fn from_pair(level: usize, total: i32, friends: i32) -> Self {
        let mut q = Self {
            level,
            total,
            friends,
            options: vec![],
            seed: rand::gen_range(0u32, u32::MAX),
        };
        if has_choice(level) {
            let a = q.answer();
            let pool: Vec<i32> = (1..=4)
                .flat_map(|d| [a - d, a + d])
                .filter(|v| *v >= 1)
                .collect();
            q.options = choices(a, pool, OPTIONS);
        }
        q
    }

    pub fn each(&self) -> i32 {
        self.total / self.friends
    }

    pub fn left(&self) -> i32 {
        self.total % self.friends
    }

    pub fn asks_friends(&self) -> bool {
        self.level == 4
    }

    pub fn answer(&self) -> i32 {
        if self.asks_friends() {
            self.friends
        } else {
            self.each()
        }
    }

    pub fn slot_of_answer(&self) -> usize {
        self.options
            .iter()
            .position(|v| *v == self.answer())
            .unwrap_or(0)
    }

    pub fn animal(&self, i: usize) -> usize {
        (self.seed as usize / 7 + i) % ANIMALS.len()
    }

    pub fn sharing_text(&self) -> String {
        format!("{} cookies, {} friends", self.total, self.friends)
    }

    pub fn sum_text(&self) -> String {
        if self.asks_friends() {
            format!("{} \u{f7} ? = {}", self.total, self.each())
        } else {
            format!("{} \u{f7} {} = ?", self.total, self.friends)
        }
    }

    pub fn solved_text(&self) -> String {
        match (self.asks_friends(), self.left()) {
            (true, _) => format!("{} \u{f7} {} = {}", self.total, self.friends, self.each()),
            (false, 0) => format!("{} \u{f7} {} = {}", self.total, self.friends, self.each()),
            (false, l) => format!("{} each, {} left over", self.each(), l),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Drag,
    Choose,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Mode {
    Play,
    Levels,
    Report(f32),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Report {
    Level { level: usize, stars: u8 },
    Cleared { rank: Option<usize> },
}

pub fn hud(view: &View) -> Hud {
    learn_hud(
        view,
        &HudSpec {
            title: HUD_TITLE,
            levels: LevelsAt::Goal,
            stars: LEVELS as u32,
            progress: 18.0,
            goal: HUD_GOAL,
        },
    )
}

pub fn prompt_rect(view: &View) -> Rect {
    let k = ui_scale(view).min(1.25);
    let w = (view.w - 60.0).min(560.0);
    Rect::new(view.w / 2.0 - w / 2.0, hud(view).bottom, w, 92.0 * k)
}

pub fn levels_button(view: &View) -> Rect {
    hud(view).levels.unwrap_or_default()
}

pub fn button_height(view: &View) -> f32 {
    view.kid_tap(124.0 * ui_scale(view))
}

pub fn button_rects(view: &View) -> Vec<Rect> {
    let n = OPTIONS;
    let gap = 24.0;
    let h = button_height(view);
    let w = ((view.w - 80.0 - gap * (n as f32 - 1.0)) / n as f32).min(250.0 * ui_scale(view));
    let left = (view.w - w * n as f32 - gap * (n as f32 - 1.0)) / 2.0;
    (0..n)
        .map(|i| Rect::new(left + i as f32 * (w + gap), view.h - h - 40.0, w, h))
        .collect()
}

pub fn wide(view: &View) -> bool {
    view.w > view.h * 1.2
}

pub fn pile_rect(view: &View) -> Rect {
    let e = view.edge();
    if wide(view) {
        let top = prompt_rect(view).bottom() + view.gap();
        let bottom = view.h - e;
        return Rect::new(e, top, (view.w * 0.2).max(160.0), (bottom - top).max(40.0));
    }
    let h = view.kid_tap(80.0 * ui_scale(view));
    Rect::new(e, view.h - h - e, view.w - 2.0 * e, h)
}

pub fn field(view: &View) -> Rect {
    let top = prompt_rect(view).bottom() + view.gap();
    let e = view.edge();
    if wide(view) {
        let p = pile_rect(view);
        let x = p.right() + view.gap();
        return Rect::new(x, top, view.w - e - x, p.h);
    }
    let bottom = pile_rect(view).y - view.gap();
    Rect::new(e, top, view.w - 2.0 * e, (bottom - top).max(40.0))
}

pub fn cells(view: &View, n: usize) -> Vec<Rect> {
    let f = field(view);
    let cols = if n <= 5 { n } else { n.div_ceil(2) }.max(1);
    let rows = n.div_ceil(cols).max(1);
    let w = (f.w / cols as f32).min(220.0);
    let h = f.h / rows as f32;
    (0..n)
        .map(|i| {
            let (r, c) = (i / cols, i % cols);
            let in_row = if r == rows - 1 { n - r * cols } else { cols };
            let left = f.x + (f.w - w * in_row as f32) / 2.0;
            Rect::new(left + c as f32 * w, f.y + r as f32 * h, w, h)
        })
        .collect()
}

pub fn plate_rect(cell: Rect) -> Rect {
    let s = (cell.w * 0.84).min(cell.h * 0.52);
    Rect::new(cell.center().x - s / 2.0, cell.bottom() - s - 2.0, s, s)
}

pub fn animal_height(cell: Rect) -> f32 {
    let p = plate_rect(cell);
    (cell.h - p.h * 0.6).min(cell.w * 0.95).max(10.0)
}

pub fn pile_slots(pile: Rect, count: usize) -> (Vec<Vec2>, f32) {
    let n = count.max(1);
    let mut best = (1usize, 0.0f32);
    for rows in 1..=8usize {
        let cols = n.div_ceil(rows);
        let c = (pile.w / cols as f32).min(pile.h / rows as f32);
        if c > best.1 {
            best = (rows, c);
        }
    }
    let (rows, size) = best;
    let size = size.min(pile.h * 0.6);
    let cols = n.div_ceil(rows);
    let left = pile.x + (pile.w - size * cols as f32) / 2.0;
    let top = pile.y + (pile.h - size * rows as f32) / 2.0;
    let slots = (0..count)
        .map(|i| {
            vec2(
                left + (i % cols) as f32 * size + size / 2.0,
                top + (i / cols) as f32 * size + size / 2.0,
            )
        })
        .collect();
    (slots, size)
}

pub fn plate_spots(plate: Rect, count: usize) -> (Vec<Vec2>, f32) {
    let size = plate.w * if count > 4 { 0.3 } else { 0.36 };
    let c = plate.center();
    let spots = (0..count)
        .map(|k| {
            if count == 1 {
                c
            } else {
                let a = std::f32::consts::TAU * k as f32 / count as f32 - 1.2;
                c + vec2(a.cos(), a.sin()) * plate.w * 0.24
            }
        })
        .collect();
    (spots, size)
}

pub fn level_tiles(view: &View) -> Vec<Rect> {
    let n = LEVELS;
    let cols = if tall(view) { 2 } else { 3 };
    let rows = n.div_ceil(cols);
    let gap = 24.0;
    let top = 150.0;
    let bottom = view.h - 190.0;
    let w = ((view.w - 80.0 - gap * (cols as f32 - 1.0)) / cols as f32).min(300.0);
    let h = ((bottom - top - gap * (rows as f32 - 1.0)) / rows as f32).min(230.0);
    let left = (view.w - w * cols as f32 - gap * (cols as f32 - 1.0)) / 2.0;
    let up = top + (bottom - top - h * rows as f32 - gap * (rows as f32 - 1.0)) / 2.0;
    (0..n)
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
    crate::sizing::bottom_button(view, "Play", 340.0, 100.0)
}

pub struct Share {
    pub mode: Mode,
    pub phase: Phase,
    pub misses: u32,
    pub first_time: bool,
    pub report: Option<Report>,
    pub ladder: Ladder,
    pub question: Question,
    pub attempt: Attempt,
    pub placed: Vec<i32>,
    pub held: bool,
    pub held_pos: Vec2,
    pub bounces: u32,
    pub plate_bump: Vec<Bump>,
    pub plate_wobble: Vec<f32>,
    pub wobble: Vec<f32>,
    pub press: Vec<Bump>,
    pub celebrate: Option<f32>,
    pub streak_bump: Bump,
    pub prompt_bump: Bump,
    pub shake: Shake,
    pub particles: Particles,
    pub popups: Popups,
    pub speech: Vec<(f32, sfx::Say)>,
    pub recent: Vec<(i32, i32)>,
    pub t: f32,
    pub session: mastery::Session,
    pub summary: String,
}

impl Share {
    pub fn new() -> Self {
        let top = LEVELS - 1;
        let unlocked = (profile::unlocked(GAME) as usize).min(top);
        let mut level = (profile::level(GAME) as usize).min(unlocked);
        while level < unlocked && mastery::mastered(GAME, level) {
            level += 1;
        }
        let cleared = profile::get(GAME, "cleared") > 0;
        let mut ladder = Ladder::new(LEVELS);
        ladder.level = level;
        ladder.stars = if cleared { LEVELS } else { unlocked };
        let mut g = Self {
            mode: Mode::Play,
            phase: Phase::Drag,
            misses: 0,
            first_time: profile::get(GAME, "played") == 0,
            report: None,
            ladder,
            question: Question::from_pair(0, 4, 2),
            attempt: Attempt::new(OPTIONS),
            placed: vec![],
            held: false,
            held_pos: Vec2::ZERO,
            bounces: 0,
            plate_bump: vec![],
            plate_wobble: vec![],
            wobble: vec![0.0; OPTIONS],
            press: (0..OPTIONS).map(|_| Bump::default()).collect(),
            celebrate: None,
            streak_bump: Bump::default(),
            prompt_bump: Bump::default(),
            shake: Shake::default(),
            particles: Particles::default(),
            popups: Popups::default(),
            speech: vec![],
            recent: vec![],
            t: 0.0,
            session: mastery::Session::default(),
            summary: String::new(),
        };
        g.next_question();
        g
    }

    pub fn unlocked_level(&self) -> usize {
        (profile::unlocked(GAME) as usize).min(LEVELS - 1)
    }

    pub fn start_level(&mut self, level: usize) {
        self.ladder.level = level.min(LEVELS - 1);
        self.ladder.streak = 0;
        self.ladder.slips = 0;
        self.misses = 0;
        self.session.reset();
        profile::set_level(GAME, self.ladder.level as u32);
        self.mode = Mode::Play;
        self.next_question();
    }

    pub fn next_question(&mut self) {
        let level = self.ladder.level;
        let (t, f) = pick_pair(level, &self.recent, round());
        self.recent.push((t, f));
        self.question = Question::from_pair(level, t, f);
        self.load();
    }

    pub fn load(&mut self) {
        let level = self.question.level;
        let f = self.question.friends;
        self.phase = if has_drag(level) {
            Phase::Drag
        } else {
            Phase::Choose
        };
        self.placed = vec![0; f as usize];
        self.plate_bump = (0..f).map(|_| Bump::default()).collect();
        self.plate_wobble = vec![0.0; f as usize];
        self.held = false;
        self.bounces = 0;
        self.attempt = Attempt::new(OPTIONS);
        self.wobble = vec![0.0; OPTIONS];
        self.celebrate = None;
        self.prompt_bump.kick();
        self.session.start_question();
        self.speak_question();
    }

    pub fn remaining(&self) -> i32 {
        self.question.total - self.placed.iter().sum::<i32>()
    }

    pub fn finish_level(&mut self, level: usize) -> u8 {
        let stars = self.session.stars(target_secs(level));
        profile::award_stars(GAME, level as u32, stars);
        self.summary = self.session.summary();
        self.session.reset();
        self.misses = 0;
        stars
    }

    fn record(&mut self, step: Step) {
        if self.attempt.wrong > 0 || self.bounces > 0 {
            self.misses += 1;
        }
        self.first_time = false;
        profile::set(GAME, "played", 1);
        match step {
            Step::Up => {
                let level = self.ladder.level - 1;
                let stars = self.finish_level(level);
                profile::unlock(GAME, self.ladder.level as u32);
                profile::set_level(GAME, self.ladder.level as u32);
                self.report = Some(Report::Level { level, stars });
            }
            Step::Cleared => {
                let level = self.ladder.level;
                let stars = self.finish_level(level);
                if profile::get(GAME, "cleared") == 0 {
                    profile::set(GAME, "cleared", 1);
                    let total = profile::total_stars(GAME) as i64;
                    let rank = profile::submit(GAME, total, profile::Better::Higher);
                    self.report = Some(Report::Cleared { rank });
                } else {
                    self.report = Some(Report::Level { level, stars });
                }
            }
            Step::Down => {
                self.misses = 0;
                self.session.reset();
                profile::set_level(GAME, self.ladder.level as u32);
            }
            Step::Same => {}
        }
    }

    fn lines_for_question(&self) -> Vec<sfx::Say> {
        let q = &self.question;
        let mut lines = vec![];
        if self.first_time {
            lines.push(sfx::Say::ShareIntro);
        }
        if q.asks_friends() {
            lines.extend([sfx::number(q.total), sfx::number(q.each())]);
        } else {
            lines.extend([sfx::number(q.total), sfx::number(q.friends)]);
        }
        if self.phase == Phase::Drag && self.first_time {
            lines.push(sfx::Say::ShareDrag);
        } else if self.phase == Phase::Choose && !q.asks_friends() {
            lines.push(sfx::Say::ShareEach);
        }
        lines
    }

    fn spaced(lines: Vec<sfx::Say>, scale: f32) -> Vec<(f32, sfx::Say)> {
        let mut t = 0.0;
        lines
            .into_iter()
            .map(|l| {
                let item = (t, l);
                t += gap_after(l) * scale;
                item
            })
            .collect()
    }

    pub fn speak_question(&mut self) {
        let lines = self.lines_for_question();
        sfx::prefetch_say(&lines);
        self.speech = Self::spaced(lines, 1.0);
    }

    fn update_speech(&mut self, dt: f32) {
        for s in &mut self.speech {
            s.0 -= dt;
        }
        if let Some(i) = self.speech.iter().position(|s| s.0 <= 0.0) {
            let (_, say) = self.speech.remove(i);
            sfx::say(say);
        }
    }

    fn on_right(&mut self, view: &View, first: bool, step: Step, at: Vec2) {
        let level = self.ladder.level;
        let answer = self.session.answer(first, false);
        mastery::record(GAME, level, answer, target_secs(level));
        let (t, f) = (self.question.total, self.question.friends);
        let r = round() + 1;
        profile::set(GAME, "round", r as i64);
        remember(level, t, f, memory_of(level, t, f).after(r, first));
        self.record(step);
        self.celebrate = Some(0.0);
        self.react_right(view, step, at);
    }

    fn react_right(&mut self, view: &View, step: Step, at: Vec2) {
        let gold = Color::from_rgba(255, 220, 70, 255);
        sfx::play(match step {
            Step::Up | Step::Cleared => Sfx::Win,
            _ => Sfx::Right,
        });
        if step == Step::Same {
            sfx::play_step(Sfx::Right, self.ladder.streak as usize * 2);
        }
        if matches!(step, Step::Up | Step::Cleared) {
            feel::impact_tinted(Impact::Large, Color::from_rgba(255, 230, 120, 255));
        } else {
            feel::impact(Impact::Small);
        }
        self.particles.sparkle(at, gold);
        self.particles.emit(
            at,
            &[gold, WHITE, hsv(0.08, 0.6, 1.0), hsv(0.9, 0.5, 1.0)],
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
        self.streak_bump.kick();
        self.speech.clear();
        let q = &self.question;
        let mut said = vec![sfx::number(q.answer())];
        if q.left() > 0 && !q.asks_friends() {
            said = vec![
                sfx::number(q.each()),
                sfx::number(q.left()),
                sfx::Say::ShareLeft,
            ];
        }
        self.speech = Self::spaced(said, 0.8);
        let word = if self.attempt.wrong == 0 && self.bounces == 0 {
            PRAISE[rand::gen_range(0, PRAISE.len())]
        } else {
            "That's it!"
        };
        let mid = vec2(view.w / 2.0, view.h * 0.4);
        self.popups.add(mid, word, gold, 90.0);
        match step {
            Step::Up => {
                self.particles.confetti(view);
                self.popups.add(
                    mid + vec2(0.0, 90.0),
                    format!("Level up: {}", level_name(self.ladder.level)),
                    WHITE,
                    46.0,
                );
            }
            Step::Cleared => {
                self.particles.confetti(view);
                self.popups
                    .add(mid + vec2(0.0, 90.0), "Fair sharing star!", WHITE, 50.0);
            }
            Step::Down => self.popups.add(
                mid + vec2(0.0, 90.0),
                format!("Let's practise: {}", level_name(self.ladder.level)),
                WHITE,
                42.0,
            ),
            Step::Same => {}
        }
    }

    pub fn pick(&mut self, view: &View, slot: usize) -> Tap {
        if self.phase != Phase::Choose || self.celebrate.is_some() || slot >= OPTIONS {
            return Tap::Ignored;
        }
        let correct = self.question.options[slot] == self.question.answer();
        let result = self.attempt.choose(slot, correct, &mut self.ladder);
        let at = button_rects(view)[slot].center();
        let orange = Color::from_rgba(255, 170, 90, 255);
        match result {
            Tap::Right(step) => {
                let first = self.attempt.wrong == 0;
                self.press[slot].kick();
                self.on_right(view, first, step, at);
            }
            Tap::Wrong | Tap::Reveal | Tap::Again => {
                self.wobble[slot] = 1.0;
                self.press[slot].kick();
                if result != Tap::Again {
                    sfx::play(Sfx::Wrong);
                    self.shake.add(0.2);
                    let text = if result == Tap::Reveal {
                        format!("It's {}", self.question.answer())
                    } else {
                        "Try again".to_string()
                    };
                    self.popups.add(at - vec2(0.0, 90.0), text, orange, 48.0);
                }
            }
            Tap::Ignored => {}
        }
        result
    }

    pub fn place(&mut self, view: &View, i: usize) -> bool {
        if self.phase != Phase::Drag || self.celebrate.is_some() || i >= self.placed.len() {
            return false;
        }
        let cell = cells(view, self.placed.len())[i];
        let plate = plate_rect(cell);
        if self.placed[i] >= self.question.each() || self.remaining() <= 0 {
            self.plate_wobble[i] = 1.0;
            self.bounces += 1;
            sfx::play(Sfx::Wrong);
            self.popups.add(
                plate.center() - vec2(0.0, plate.h * 0.9),
                "Same for everyone",
                Color::from_rgba(255, 170, 90, 255),
                40.0,
            );
            return false;
        }
        self.placed[i] += 1;
        self.plate_bump[i].kick();
        self.first_time = false;
        let done = (self.question.total - self.remaining()) as usize;
        sfx::play(Sfx::Pop);
        sfx::play_step(Sfx::Tick, done.saturating_sub(1));
        self.particles
            .sparkle(plate.center(), hsv(0.09 + i as f32 * 0.11, 0.55, 1.0));
        if self.remaining() == 0 {
            self.finish_drag(view);
        }
        true
    }

    fn finish_drag(&mut self, view: &View) {
        if has_choice(self.ladder.level) {
            self.phase = Phase::Choose;
            self.attempt = Attempt::new(OPTIONS);
            self.speak_question();
            sfx::play(Sfx::Magic);
        } else {
            let first = self.bounces == 0;
            let step = if first {
                self.ladder.right()
            } else {
                self.ladder.slip()
            };
            self.on_right(view, first, step, view.center());
        }
    }

    fn drop_held(&mut self, view: &View, at: Vec2) {
        self.held = false;
        let cells = cells(view, self.placed.len());
        if let Some(i) = cells.iter().position(|c| c.contains(at)) {
            self.place(view, i);
        }
    }

    fn update_drag(&mut self, input: &Input, view: &View) {
        if self.held {
            self.held_pos = input.pos;
            if input.released || !input.down {
                self.drop_held(view, input.pos);
            }
            return;
        }
        if !input.pressed {
            return;
        }
        let cells = cells(view, self.placed.len());
        if let Some(i) = cells.iter().position(|c| c.contains(input.pos)) {
            self.place(view, i);
        } else if pile_rect(view).contains(input.pos) && self.remaining() > 0 {
            self.held = true;
            self.held_pos = input.pos;
            sfx::play(Sfx::Tap);
        }
    }

    fn update_play(&mut self, input: &Input, view: &View, dt: f32) {
        self.update_speech(dt);
        if let Some(t) = self.celebrate {
            let t = t + dt;
            self.celebrate = Some(t);
            if t >= CELEBRATE {
                self.next_question();
                if self.report.is_some() {
                    self.mode = Mode::Report(0.0);
                }
            }
            return;
        }
        self.session.tick(dt);
        if input.pressed && levels_button(view).contains(input.pos) && !self.first_time {
            self.mode = Mode::Levels;
            return;
        }
        if input.pressed && prompt_rect(view).contains(input.pos) {
            self.speak_question();
            return;
        }
        match self.phase {
            Phase::Drag => self.update_drag(input, view),
            Phase::Choose => {
                if input.pressed {
                    if let Some(slot) = button_rects(view)
                        .iter()
                        .position(|r| r.contains(input.pos))
                    {
                        self.pick(view, slot);
                    }
                }
            }
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
        let unlocked = self.unlocked_level();
        let hit = level_tiles(view).iter().position(|r| r.contains(input.pos));
        if let Some(i) = hit.filter(|i| *i <= unlocked) {
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
        if self.ladder.level + 1 < LEVELS {
            format!(
                "Get {UP_STREAK} right in a row to reach {}",
                level_name(self.ladder.level + 1)
            )
        } else {
            format!("Get {UP_STREAK} right in a row to finish")
        }
    }

    pub fn prompt_text(&self) -> String {
        let q = &self.question;
        if self.celebrate.is_some() {
            q.solved_text()
        } else if self.phase == Phase::Drag {
            q.sharing_text()
        } else {
            q.sum_text()
        }
    }

    pub fn glow_slot(&self) -> Option<usize> {
        (self.attempt.revealed() && !self.attempt.solved).then(|| self.question.slot_of_answer())
    }
}

impl Default for Share {
    fn default() -> Self {
        Self::new()
    }
}

impl Game for Share {
    fn update(&mut self, input: &Input, view: &View, dt: f32) {
        self.t += dt;
        self.streak_bump.update(dt);
        self.prompt_bump.update(dt);
        for b in self.press.iter_mut().chain(self.plate_bump.iter_mut()) {
            b.update(dt);
        }
        for w in self.wobble.iter_mut().chain(self.plate_wobble.iter_mut()) {
            *w = (*w - dt * 1.8).max(0.0);
        }
        match self.mode {
            Mode::Levels => self.update_levels(input, view),
            Mode::Report(age) => self.update_report(input, age, dt),
            Mode::Play => self.update_play(input, view, dt),
        }
        self.shake.update(dt);
        self.particles.update(dt);
        self.popups.update(dt);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::games::spell::GAPS;

    const DT: f32 = 1.0 / 60.0;

    fn fresh() -> Share {
        crate::save::reset_for_test();
        profile::create("Mia", 0);
        rand::srand(11);
        Share::new()
    }

    fn views() -> [View; 2] {
        [View::ipad_landscape(), View::ipad_portrait()]
    }

    fn run(g: &mut Share, v: &View, secs: f32) {
        for _ in 0..(secs / DT) as usize {
            g.update(&Input::default(), v, DT);
        }
    }

    fn solve(g: &mut Share, v: &View, clean: bool) {
        if g.phase == Phase::Drag {
            if !clean {
                g.place(v, 0);
                let each = g.question.each();
                for _ in 0..each {
                    g.place(v, 0);
                }
            }
            let n = g.placed.len();
            let mut i = 0;
            while g.phase == Phase::Drag && g.remaining() > 0 && g.celebrate.is_none() {
                g.place(v, i % n);
                i += 1;
            }
        }
        if g.phase == Phase::Choose && g.celebrate.is_none() {
            if !clean {
                let wrong = (0..OPTIONS)
                    .find(|s| *s != g.question.slot_of_answer())
                    .unwrap();
                g.pick(v, wrong);
            }
            g.pick(v, g.question.slot_of_answer());
        }
        run(g, v, CELEBRATE + 0.1);
        while let Mode::Report(_) = g.mode {
            g.update(&Input::press_at(Vec2::ZERO), v, 1.0);
            g.update(&Input::press_at(Vec2::ZERO), v, 1.0);
        }
    }

    #[test]
    fn every_level_has_a_named_example_and_pool() {
        for l in 0..LEVELS {
            assert!(!level_name(l).is_empty() && !level_example(l).is_empty());
            assert!(pool(l).len() >= 5, "level {l}");
        }
        assert!(level_name(1).len() >= level_name(3).len().min(18));
        assert!(HUD_GOAL.len() >= goal_len());
    }

    fn goal_len() -> usize {
        (0..LEVELS)
            .map(|l| format!("Get {UP_STREAK} right in a row to reach {}", level_name(l)).len())
            .max()
            .unwrap()
    }

    #[test]
    fn the_level_plan_steps_up_as_written() {
        assert!(pool(0)
            .iter()
            .all(|p| p.1 == 2 && (6..=12).contains(&p.0) || p.0 == 4));
        assert!(pool(0).iter().all(|p| p.0 % 2 == 0));
        assert!(pool(1)
            .iter()
            .all(|p| (2..=5).contains(&p.1) && p.0 % p.1 == 0));
        assert!(pool(1).iter().any(|p| p.1 == 5));
        assert!(has_drag(0) && has_drag(1) && has_drag(2));
        assert!(!has_choice(1) && has_choice(2));
        assert!(!has_drag(3) && !has_drag(4) && !has_drag(5));
        assert!(pool(5).iter().all(|p| p.0 % p.1 != 0));
        assert!(pool(2).iter().all(|p| p.0 % p.1 == 0));
        assert!(pool(2).len() > pool(0).len());
        let q = Question::from_pair(4, 12, 3);
        assert_eq!(q.sum_text(), "12 \u{f7} ? = 4");
        assert_eq!(q.answer(), 3);
        let r = Question::from_pair(5, 13, 4);
        assert_eq!((r.each(), r.left()), (3, 1));
        assert_eq!(r.solved_text(), "3 each, 1 left over");
    }

    #[test]
    fn answer_only_levels_use_the_tables_mastered_in_the_garden() {
        fresh();
        assert_eq!(divisors(), vec![2, 5, 10]);
        profile::set(garden::GAME, "bed.4", 1);
        profile::set(garden::GAME, "bed.0", 1);
        let mut d = divisors();
        d.sort_unstable();
        assert_eq!(d, vec![2, 4]);
        assert!(pool(3).iter().all(|p| p.1 == 2 || p.1 == 4));
        assert!(pool(4).iter().all(|p| p.0 <= MAX_SPOKEN));
    }

    #[test]
    fn choices_hold_the_one_right_answer_and_are_distinct() {
        rand::srand(3);
        for l in 2..LEVELS {
            for (t, f) in pool(l) {
                let q = Question::from_pair(l, t, f);
                assert_eq!(q.options.len(), OPTIONS);
                assert_eq!(q.options.iter().filter(|v| **v == q.answer()).count(), 1);
                let mut s = q.options.clone();
                s.sort_unstable();
                s.dedup();
                assert_eq!(s.len(), OPTIONS);
                assert!(s[0] >= 1);
            }
        }
    }

    #[test]
    fn questions_do_not_repeat_inside_the_recent_window() {
        for level in 0..LEVELS {
            let mut g = fresh();
            g.start_level(level);
            let mut seen: Vec<(i32, i32)> = vec![];
            for _ in 0..60 {
                let p = (g.question.total, g.question.friends);
                let window = &seen[seen.len().saturating_sub(RECENT.min(pool(level).len() / 2))..];
                assert!(!window.contains(&p), "level {level} repeated {p:?}");
                seen.push(p);
                solve(&mut g, &views()[0], true);
                g.start_level(level);
                g.recent = seen.clone();
            }
        }
    }

    #[test]
    fn a_missed_question_comes_back_after_its_gap_and_new_ones_come_first() {
        let mut g = fresh();
        let p = pool(1)[0];
        remember(1, p.0, p.1, Memory::default().after(5, false));
        let due = memory_of(1, p.0, p.1).due;
        assert_eq!(due, 5 + GAPS[0]);
        rand::srand(5);
        for _ in 0..20 {
            assert_ne!(pick_pair(1, &[], due - 1), p);
        }
        assert_eq!(pick_pair(1, &[], due), p);
        let _ = &mut g;
    }

    #[test]
    fn dragging_cookies_onto_plates_shares_them_evenly() {
        for v in views() {
            let mut g = fresh();
            g.start_level(1);
            g.question = Question::from_pair(1, 12, 3);
            g.load();
            g.placed = vec![0; 3];
            g.celebrate = None;
            g.first_time = false;
            let pile = pile_rect(&v).center();
            let cells = cells(&v, 3);
            for k in 0..12 {
                let cell = cells[k % 3];
                g.update(&Input::press_at(pile), &v, DT);
                assert!(g.held, "cookie {k}");
                let mut drop = Input::press_at(cell.center());
                drop.pressed = false;
                drop.down = false;
                drop.released = true;
                g.update(&drop, &v, DT);
                assert!(!g.held);
            }
            assert_eq!(g.remaining(), 0);
            assert!(g.celebrate.is_some() || g.report.is_some() || g.placed == vec![4, 4, 4]);
        }
    }

    #[test]
    fn a_full_plate_refuses_another_cookie() {
        let v = views()[0];
        let mut g = fresh();
        g.start_level(0);
        g.question = Question::from_pair(0, 8, 2);
        g.load();
        g.placed = vec![0; 2];
        for _ in 0..4 {
            assert!(g.place(&v, 0));
        }
        assert!(!g.place(&v, 0));
        assert_eq!(g.placed, vec![4, 0]);
        assert_eq!(g.bounces, 1);
        assert!(g.place(&v, 1));
    }

    #[test]
    fn tapping_a_plate_places_one_cookie() {
        let v = views()[0];
        let mut g = fresh();
        g.question = Question::from_pair(0, 8, 2);
        g.load();
        g.placed = vec![0; 2];
        let c = cells(&v, 2)[1].center();
        g.update(&Input::press_at(c), &v, DT);
        assert_eq!(g.placed, vec![0, 1]);
    }

    #[test]
    fn the_drag_then_answer_level_asks_after_the_last_cookie() {
        let v = views()[0];
        let mut g = fresh();
        g.start_level(2);
        g.question = Question::from_pair(2, 6, 2);
        g.load();
        g.placed = vec![0; 2];
        for i in 0..6 {
            g.place(&v, i % 2);
        }
        assert_eq!(g.phase, Phase::Choose);
        assert_eq!(g.prompt_text(), "6 \u{f7} 2 = ?");
        let slot = g.question.slot_of_answer();
        assert_eq!(g.pick(&v, slot), Tap::Right(Step::Same));
    }

    #[test]
    fn a_strong_player_climbs_every_level_and_reaches_the_board() {
        let v = views()[0];
        let mut g = fresh();
        for _ in 0..(UP_STREAK as usize * LEVELS + 2) {
            solve(&mut g, &v, true);
        }
        assert_eq!(profile::get(GAME, "cleared"), 1);
        assert!(profile::personal_best(GAME).is_some());
        for l in 0..LEVELS {
            assert!(profile::stars(GAME, l as u32) >= 1, "level {l}");
        }
        assert_eq!(profile::unlocked(GAME) as usize, LEVELS - 1);
    }

    #[test]
    fn a_weak_player_steps_back_but_keeps_playing() {
        let v = views()[0];
        let mut g = fresh();
        profile::unlock(GAME, 3);
        g.start_level(3);
        for _ in 0..3 {
            solve(&mut g, &v, false);
        }
        assert_eq!(g.ladder.level, 2);
        for _ in 0..30 {
            solve(&mut g, &v, false);
        }
        assert!(g.ladder.level <= 2);
        assert_eq!(profile::get(GAME, "cleared"), 0);
    }

    #[test]
    fn a_reveal_after_two_wrong_picks_glows_the_answer() {
        let v = views()[0];
        let mut g = fresh();
        g.start_level(3);
        let wrongs: Vec<usize> = (0..OPTIONS)
            .filter(|s| *s != g.question.slot_of_answer())
            .collect();
        g.pick(&v, wrongs[0]);
        assert_eq!(g.glow_slot(), None);
        g.pick(&v, wrongs[1]);
        assert_eq!(g.glow_slot(), Some(g.question.slot_of_answer()));
    }

    #[test]
    fn the_first_minute_is_spoken_and_easy() {
        let g = fresh();
        assert!(g.first_time);
        assert_eq!(g.ladder.level, 0);
        assert_eq!(g.phase, Phase::Drag);
        let said: Vec<_> = g.speech.iter().map(|s| s.1).collect();
        assert_eq!(
            said,
            vec![
                sfx::Say::ShareIntro,
                sfx::number(g.question.total),
                sfx::number(g.question.friends),
                sfx::Say::ShareDrag,
            ]
        );
        let times: Vec<f32> = g.speech.iter().map(|s| s.0).collect();
        assert!(times.windows(2).all(|w| w[1] - w[0] >= SPEECH_GAP));
    }

    #[test]
    fn level_progress_is_saved_per_player() {
        let v = views()[0];
        let mut g = fresh();
        for _ in 0..UP_STREAK {
            solve(&mut g, &v, true);
        }
        let mia = profile::current_id().unwrap();
        assert_eq!(profile::unlocked(GAME), 1);
        let leo = profile::create("Leo", 1).unwrap();
        profile::select(&leo.id);
        assert_eq!(profile::unlocked(GAME), 0);
        profile::select(&mia);
        assert_eq!(profile::unlocked(GAME), 1);
        assert_eq!(Share::new().ladder.level, 1);
    }

    #[test]
    fn layouts_keep_cells_inside_the_field_in_both_orientations() {
        for (name, v) in crate::sizing::test_views() {
            let f = field(&v);
            for n in 2..=12usize {
                for c in cells(&v, n) {
                    assert!(
                        c.x >= f.x - 0.5 && c.right() <= f.right() + 0.5,
                        "{name} {n}"
                    );
                    assert!(
                        c.y >= f.y - 0.5 && c.bottom() <= f.bottom() + 0.5,
                        "{name} {n}"
                    );
                    let p = plate_rect(c);
                    assert!(p.x >= c.x && p.right() <= c.right() && p.bottom() <= c.bottom());
                }
            }
            assert!(f.h > 60.0, "{name}: field {f:?}");
            let (slots, size) = pile_slots(pile_rect(&v), 30);
            assert_eq!(slots.len(), 30);
            assert!(size > 10.0, "{name}");
        }
    }
}
