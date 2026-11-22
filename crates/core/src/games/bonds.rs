use crate::engine::*;
use crate::feel::{self, Impact};
use crate::fx::*;
use crate::games::trains::{Memory, GAPS};
use crate::geom::*;
use crate::learn::*;
use crate::profile;
use crate::rng as rand;
use crate::sfx::{self, Line, Say, Sfx};
use glam::{vec2, Vec2};

pub const GAME: &str = "bonds";
pub const KEY: &str = GAME;
pub const LEVELS: usize = 7;
pub const OPTIONS: usize = 3;
pub const RECENT: usize = 6;
pub const SAMPLES: usize = 40;
pub const MAX_TILT: f32 = 0.35;
pub const TILT_PER_FULL: f32 = 0.7;
pub const BEAD_ROW: usize = 5;
pub const SPEECH_GAP: f32 = 1.0;
pub const HINT_AFTER: f32 = 4.0;
pub const WRONG_HOLD: f32 = 0.9;
pub const CELEBRATE: f32 = 1.5;
pub const BUTTON_COLORS: [&str; 4] = ["w_red", "w_blue", "w_green", "w_orange"];

pub fn level_name(level: usize) -> &'static str {
    [
        "Make 5",
        "Make 10",
        "Make 10, no beads",
        "Make 20 from teens",
        "Make 20",
        "Make 100 in tens",
        "Mixed bonds",
    ][level.min(LEVELS - 1)]
}

pub fn level_example(level: usize) -> &'static str {
    [
        "3 + ? = 5",
        "6 + ? = 10",
        "7 + ? = 10",
        "14 + ? = 20",
        "8 + ? = 20",
        "30 + ? = 100",
        "? + ? = ?",
    ][level.min(LEVELS - 1)]
}

pub fn level_hint(level: usize) -> &'static str {
    [
        "Pick the weight that balances the scale",
        "Count the beads, then pick the weight",
        "No beads now: think it through",
        "Teens plus a bit make 20",
        "Any number can make 20",
        "Tens that make 100",
        "Every target mixed together",
    ][level.min(LEVELS - 1)]
}

pub fn target_secs(level: usize) -> f32 {
    [4.0, 5.0, 5.0, 6.0, 7.0, 7.0, 8.0][level.min(LEVELS - 1)]
}

pub fn beads(level: usize) -> bool {
    level < 2
}

pub fn stars_score(s: i64) -> String {
    format!("{s} stars")
}

pub fn level_pool(level: usize) -> Vec<(i32, i32)> {
    let range = |t: i32, lo: i32, hi: i32, step: i32| -> Vec<(i32, i32)> {
        (lo..=hi).step_by(step as usize).map(|a| (t, a)).collect()
    };
    match level {
        0 => range(5, 1, 4, 1),
        1 | 2 => range(10, 1, 9, 1),
        3 => range(20, 11, 19, 1),
        4 => range(20, 1, 19, 1),
        5 => range(100, 10, 90, 10),
        _ => [
            range(5, 1, 4, 1),
            range(10, 1, 9, 1),
            range(20, 1, 19, 1),
            range(100, 10, 90, 10),
        ]
        .concat(),
    }
}

fn step_of(target: i32) -> i32 {
    if target == 100 {
        10
    } else {
        1
    }
}

fn memory_key(level: usize, t: i32, a: i32) -> String {
    format!("q{level}.{t}.{a}")
}

pub fn memory_of(level: usize, t: i32, a: i32) -> Memory {
    Memory::unpack(profile::get(GAME, &memory_key(level, t, a)))
}

fn remember(level: usize, t: i32, a: i32, m: Memory) {
    profile::set(GAME, &memory_key(level, t, a), m.pack());
}

pub fn round() -> u32 {
    profile::get(GAME, "round") as u32
}

pub fn seen_count(level: usize) -> usize {
    let prefix = crate::save::player_key(&format!("{GAME}.q{level}."));
    crate::save::keys_with_prefix(&prefix).len()
}

fn seen_items(level: usize) -> Vec<((i32, i32), Memory)> {
    let prefix = crate::save::player_key(&format!("{GAME}.q{level}."));
    crate::save::keys_with_prefix(&prefix)
        .into_iter()
        .filter_map(|k| {
            let rest = k.strip_prefix(&prefix)?;
            let (t, a) = rest.split_once('.')?;
            let m = Memory::unpack(crate::save::get(&k));
            Some(((t.parse().ok()?, a.parse().ok()?), m))
        })
        .collect()
}

pub fn pick_item(
    level: usize,
    recent: &[(i32, i32)],
    round: u32,
    reviewed_lately: bool,
) -> ((i32, i32), bool) {
    let all = level_pool(level);
    let window = RECENT.min(all.len() / 2);
    let recent = &recent[recent.len().saturating_sub(window)..];
    let seen = seen_items(level);
    if !reviewed_lately {
        let mut due: Vec<_> = seen
            .iter()
            .filter(|(p, m)| m.seen() && round >= m.due && !recent.contains(p))
            .collect();
        due.sort_by_key(|(_, m)| m.due);
        if let Some((p, _)) = due.first() {
            return (*p, true);
        }
    }
    for _ in 0..SAMPLES {
        let p = all[rand::gen_range(0, all.len())];
        if !recent.contains(&p) && !seen.iter().any(|(q, _)| *q == p) {
            return (p, false);
        }
    }
    let mut best: Option<((i32, i32), u32)> = None;
    for _ in 0..SAMPLES {
        let p = all[rand::gen_range(0, all.len())];
        if recent.contains(&p) {
            continue;
        }
        let at = seen.iter().find(|(q, _)| *q == p).map_or(0, |(_, m)| m.at);
        if best.is_none_or(|(_, b)| at < b) {
            best = Some((p, at));
        }
    }
    match best {
        Some((p, _)) => (p, false),
        None => (
            all.iter()
                .copied()
                .find(|p| !recent.contains(p))
                .unwrap_or(all[0]),
            false,
        ),
    }
}

#[derive(Clone, Debug)]
pub struct Question {
    pub level: usize,
    pub target: i32,
    pub a: i32,
    pub options: Vec<i32>,
    pub seed: u32,
}

impl Question {
    pub fn answer(&self) -> i32 {
        self.target - self.a
    }

    pub fn pair(&self) -> (i32, i32) {
        (self.target, self.a)
    }

    pub fn from_pair(level: usize, target: i32, a: i32) -> Self {
        let mut q = Self {
            level,
            target,
            a,
            options: vec![],
            seed: rand::gen_range(0u32, u32::MAX),
        };
        q.options = decoys(&q);
        q
    }

    pub fn slot_of_answer(&self) -> usize {
        self.options
            .iter()
            .position(|v| *v == self.answer())
            .unwrap_or(0)
    }
}

fn decoys(q: &Question) -> Vec<i32> {
    let answer = q.answer();
    let step = step_of(q.target);
    let ok = |v: &i32| *v >= step && *v < q.target && *v != answer;
    let mut near: Vec<i32> = [1, 2, 3]
        .iter()
        .flat_map(|d| [answer - d * step, answer + d * step])
        .filter(ok)
        .collect();
    near.sort_unstable();
    near.dedup();
    let mut picks: Vec<i32> = vec![];
    if q.level >= 2 && ok(&q.a) && rand::gen_range(0, 2) == 0 {
        picks.push(q.a);
    }
    while picks.len() < OPTIONS - 1 && !near.is_empty() {
        let v = near.swap_remove(rand::gen_range(0, near.len()));
        if !picks.contains(&v) {
            picks.push(v);
        }
    }
    let mut far: Vec<i32> = (1..q.target / step).map(|k| k * step).filter(ok).collect();
    while picks.len() < OPTIONS - 1 && !far.is_empty() {
        let v = far.swap_remove(rand::gen_range(0, far.len()));
        if !picks.contains(&v) {
            picks.push(v);
        }
    }
    picks.insert(rand::gen_range(0, picks.len() + 1), answer);
    picks
}

pub fn tilt_for(left: i32, right: i32, target: i32) -> f32 {
    let diff = (left - right) as f32 / target.max(1) as f32;
    (diff * TILT_PER_FULL).clamp(-MAX_TILT, MAX_TILT)
}

pub const HUD_TITLE: &str = "Level 7";
pub const HUD_GOAL: &str = "Get 4 right in a row to reach Make 100 in tens";

pub fn hud(view: &View) -> crate::learn::Hud {
    crate::learn::learn_hud(
        view,
        &crate::learn::HudSpec {
            title: HUD_TITLE,
            levels: crate::learn::LevelsAt::Goal,
            stars: LEVELS as u32,
            progress: 18.0,
            goal: HUD_GOAL,
        },
    )
}

pub fn prompt_size(view: &View) -> f32 {
    56.0 * ui_scale(view)
}

pub fn prompt_y(view: &View) -> f32 {
    hud(view).bottom + prompt_size(view) * 0.7
}

pub fn levels_button(view: &View) -> Rect {
    hud(view).levels.unwrap_or_default()
}

pub fn button_height(view: &View) -> f32 {
    view.kid_tap(124.0 * ui_scale(view))
}

pub fn side_layout(view: &View) -> bool {
    view.w > view.h * 1.7
}

pub fn button_rects(view: &View) -> Vec<Rect> {
    let n = OPTIONS;
    let gap = 16.0;
    if side_layout(view) {
        let top = hud(view).bottom + 8.0;
        let avail = view.h - top - 16.0;
        let h = ((avail - gap * (n as f32 - 1.0)) / n as f32).clamp(60.0, 110.0);
        let w = (view.w * 0.2).clamp(110.0, 200.0);
        let x = view.w - w - 24.0;
        return (0..n)
            .map(|i| Rect::new(x, top + i as f32 * (h + gap), w, h))
            .collect();
    }
    let gap = 24.0;
    let h = button_height(view);
    let w = ((view.w - 80.0 - gap * (n as f32 - 1.0)) / n as f32).min(240.0 * ui_scale(view));
    let left = (view.w - w * n as f32 - gap * (n as f32 - 1.0)) / 2.0;
    (0..n)
        .map(|i| Rect::new(left + i as f32 * (w + gap), view.h - h - 40.0, w, h))
        .collect()
}

pub fn hint_pos(view: &View) -> Vec2 {
    let top = button_rects(view)[0].y;
    let half = 32.0 * view.text_k() * 1.05;
    vec2(view.w / 2.0, top - view.gap() - half)
}

pub fn hint_room(view: &View) -> bool {
    if side_layout(view) {
        return false;
    }
    let top = prompt_y(view) + prompt_size(view) * 0.7;
    hint_pos(view).y - 32.0 * view.text_k() * 1.1 - top >= 150.0
}

pub fn scale_area(view: &View) -> Rect {
    let top = prompt_y(view) + prompt_size(view) * 0.7;
    if side_layout(view) {
        let right = button_rects(view)[0].x - 16.0;
        return Rect::new(20.0, top, right - 20.0, (view.h - 12.0 - top).max(60.0));
    }
    let hint_top = hint_pos(view).y - 32.0 * view.text_k() * 1.1;
    let buttons_top = button_rects(view)[0].y - 8.0;
    let bottom = if hint_room(view) {
        hint_top
    } else {
        buttons_top
    };
    Rect::new(30.0, top, view.w - 60.0, (bottom - top).max(60.0))
}

#[derive(Clone, Copy, Debug)]
pub struct ScaleGeo {
    pub pivot: Vec2,
    pub arm: f32,
    pub left_end: Vec2,
    pub right_end: Vec2,
    pub pan_w: f32,
    pub pan_h: f32,
}

impl ScaleGeo {
    pub fn left_dish(&self) -> Vec2 {
        self.left_end + vec2(0.0, self.pan_h * 0.84)
    }

    pub fn right_dish(&self) -> Vec2 {
        self.right_end + vec2(0.0, self.pan_h * 0.84)
    }

    pub fn bottom(&self) -> f32 {
        self.left_end.y.max(self.right_end.y) + self.pan_h
    }
}

pub fn scale_geo(view: &View, tilt: f32) -> ScaleGeo {
    let area = scale_area(view);
    let reach = 1.0 + 0.85 * 1.44;
    let arm = (area.w * 0.34).min(area.h / (reach + 0.4));
    let pivot = vec2(area.x + area.w / 2.0, area.y + arm * 0.22);
    let (s, c) = tilt.sin_cos();
    let pan_w = arm * 0.85;
    ScaleGeo {
        pivot,
        arm,
        left_end: pivot + vec2(-c, s) * arm,
        right_end: pivot + vec2(c, -s) * arm,
        pan_w,
        pan_h: pan_w * 1.44,
    }
}

pub fn bead_spots(count: usize, dish: Vec2, pan_w: f32) -> Vec<Vec2> {
    let r = pan_w * 0.085;
    (0..count)
        .map(|i| {
            let row = i / BEAD_ROW;
            let in_row = (count - row * BEAD_ROW).min(BEAD_ROW);
            let col = i % BEAD_ROW;
            let x = (col as f32 - (in_row as f32 - 1.0) / 2.0) * r * 2.1;
            vec2(dish.x + x, dish.y - r * 1.2 - row as f32 * r * 1.9)
        })
        .collect()
}

pub fn level_tiles(view: &View) -> Vec<Rect> {
    let n = LEVELS;
    let cols = if tall(view) { 2 } else { 4 };
    let rows = n.div_ceil(cols);
    let gap = 22.0;
    let top = 150.0;
    let bottom = view.h - 190.0;
    let w = ((view.w - 80.0 - gap * (cols as f32 - 1.0)) / cols as f32).min(260.0);
    let h = ((bottom - top - gap * (rows as f32 - 1.0)) / rows as f32).min(210.0);
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

pub struct Bonds {
    pub mode: Mode,
    pub misses: u32,
    pub first_time: bool,
    pub report: Option<Report>,
    pub ladder: Ladder,
    pub question: Question,
    pub attempt: Attempt,
    pub recent: Vec<(i32, i32)>,
    pub since_review: u32,
    pub on_pan: Option<i32>,
    pub pan_age: f32,
    pub tilt: f32,
    pub wobble: Vec<f32>,
    pub press: Vec<Bump>,
    pub celebrate: Option<f32>,
    pub idle: f32,
    pub streak_bump: Bump,
    pub prompt_bump: Bump,
    pub shake: Shake,
    pub particles: Particles,
    pub popups: Popups,
    pub speech: Vec<(f32, Say)>,
    pub t: f32,
    pub session: crate::mastery::Session,
    pub summary: String,
}

impl Bonds {
    pub fn new() -> Self {
        let top = LEVELS - 1;
        let unlocked = (profile::unlocked(GAME) as usize).min(top);
        let level = (profile::level(GAME) as usize).min(unlocked);
        let cleared = profile::get(GAME, "cleared") > 0;
        let mut ladder = Ladder::new(LEVELS);
        ladder.level = level;
        ladder.stars = if cleared { LEVELS } else { unlocked };
        let mut g = Self {
            mode: Mode::Play,
            misses: 0,
            first_time: profile::get(GAME, "played") == 0,
            report: None,
            ladder,
            question: Question::from_pair(0, 5, 1),
            attempt: Attempt::new(OPTIONS),
            recent: vec![],
            since_review: 9,
            on_pan: None,
            pan_age: 0.0,
            tilt: 0.0,
            wobble: vec![0.0; OPTIONS],
            press: (0..OPTIONS).map(|_| Bump::default()).collect(),
            celebrate: None,
            idle: 0.0,
            streak_bump: Bump::default(),
            prompt_bump: Bump::default(),
            shake: Shake::default(),
            particles: Particles::default(),
            popups: Popups::default(),
            speech: vec![],
            t: 0.0,
            session: crate::mastery::Session::default(),
            summary: String::new(),
        };
        g.next_question();
        g
    }

    pub fn level(&self) -> usize {
        self.ladder.level
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
        self.recent.clear();
        profile::set_level(GAME, self.ladder.level as u32);
        self.mode = Mode::Play;
        self.next_question();
    }

    pub fn next_question(&mut self) {
        let level = self.ladder.level;
        let (pair, review) = pick_item(level, &self.recent, round(), self.since_review < 2);
        self.since_review = if review { 0 } else { self.since_review + 1 };
        self.recent.push(pair);
        if self.recent.len() > RECENT * 2 {
            self.recent.remove(0);
        }
        self.question = Question::from_pair(level, pair.0, pair.1);
        self.load();
    }

    pub fn load(&mut self) {
        self.attempt = Attempt::new(OPTIONS);
        self.wobble = vec![0.0; OPTIONS];
        self.press = (0..OPTIONS).map(|_| Bump::default()).collect();
        self.on_pan = None;
        self.pan_age = 0.0;
        self.celebrate = None;
        self.idle = 0.0;
        self.prompt_bump.kick();
        self.session.start_question();
        self.queue_speech(0.6);
    }

    pub fn queue_speech(&mut self, start: f32) {
        let q = &self.question;
        let mut at = start;
        let mut queued = vec![];
        for l in [
            sfx::number(q.a),
            Say::Line(Line::Makes),
            sfx::number(q.target),
        ] {
            queued.push((at, l));
            at += SPEECH_GAP;
        }
        self.speech = queued;
    }

    pub fn speak_question(&mut self) {
        self.queue_speech(0.0);
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

    pub fn tilt_target(&self) -> f32 {
        let q = &self.question;
        tilt_for(q.a + self.on_pan.unwrap_or(0), q.target, q.target)
    }

    pub fn hint_slot(&self) -> Option<usize> {
        if self.celebrate.is_some() || self.attempt.solved {
            return None;
        }
        let wait = if self.first_time {
            2.0
        } else {
            HINT_AFTER * 2.0
        };
        if self.attempt.revealed() || self.idle >= wait && self.first_time {
            Some(self.question.slot_of_answer())
        } else {
            None
        }
    }

    pub fn pick(&mut self, slot: usize) -> Tap {
        if self.celebrate.is_some() || slot >= self.question.options.len() {
            return Tap::Ignored;
        }
        let value = self.question.options[slot];
        let correct = value == self.question.answer();
        let level = self.ladder.level;
        let result = self.attempt.choose(slot, correct, &mut self.ladder);
        if result != Tap::Ignored {
            self.on_pan = Some(value);
            self.pan_age = 0.0;
            self.idle = 0.0;
        }
        if let Tap::Right(step) = result {
            let answer = self.session.answer(self.attempt.wrong == 0, false);
            crate::mastery::record(GAME, level, answer, target_secs(level));
            let (t, a) = self.question.pair();
            let r = round() + 1;
            profile::set(GAME, "round", r as i64);
            remember(
                level,
                t,
                a,
                memory_of(level, t, a).after(r, self.attempt.wrong == 0),
            );
            self.record(step);
            self.celebrate = Some(0.0);
        }
        match result {
            Tap::Wrong | Tap::Reveal | Tap::Again => self.wobble[slot] = 1.0,
            _ => {}
        }
        if result != Tap::Ignored {
            self.press[slot].kick();
        }
        result
    }

    fn finish_level(&mut self, level: usize) -> u8 {
        let stars = self.session.stars(target_secs(level));
        profile::award_stars(GAME, level as u32, stars);
        self.summary = self.session.summary();
        self.session.reset();
        self.misses = 0;
        stars
    }

    fn record(&mut self, step: Step) {
        if self.attempt.wrong > 0 {
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

    pub fn react(&mut self, view: &View, slot: usize, tap: Tap) {
        let gold = Color::from_rgba(255, 220, 70, 255);
        let orange = Color::from_rgba(255, 170, 90, 255);
        let at = button_rects(view)[slot].center();
        match tap {
            Tap::Right(step) => {
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
                    &[gold, WHITE, hsv(0.55, 0.5, 1.0), hsv(0.9, 0.5, 1.0)],
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
                sfx::say(sfx::number(self.question.answer()));
                let word = if self.attempt.wrong == 0 {
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
                            .add(mid + vec2(0.0, 90.0), "Bonds star!", WHITE, 50.0);
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
            Tap::Wrong => {
                sfx::play(Sfx::Wrong);
                self.shake.add(0.2);
                self.popups
                    .add(at - vec2(0.0, 90.0), "Try again", orange, 48.0);
            }
            Tap::Reveal => {
                sfx::play(Sfx::Wrong);
                self.shake.add(0.2);
                let text = format!("It's {}", self.question.answer());
                self.popups.add(at - vec2(0.0, 90.0), text, orange, 48.0);
            }
            Tap::Again | Tap::Ignored => {}
        }
    }

    fn update_play(&mut self, input: &Input, view: &View, dt: f32) {
        if self.celebrate.is_none() {
            self.session.tick(dt);
            self.idle += dt;
        }
        self.pan_age += dt;
        if self.celebrate.is_none() && self.on_pan.is_some() && self.pan_age >= WRONG_HOLD {
            self.on_pan = None;
        }
        self.update_speech(dt);
        if let Some(t) = &mut self.celebrate {
            *t += dt;
            if *t >= CELEBRATE {
                self.next_question();
                if self.report.is_some() {
                    self.mode = Mode::Report(0.0);
                }
            }
            return;
        }
        if !input.pressed {
            return;
        }
        if levels_button(view).contains(input.pos) && !self.first_time {
            self.mode = Mode::Levels;
        } else if let Some(slot) = button_rects(view)
            .iter()
            .position(|r| r.contains(input.pos))
        {
            let tap = self.pick(slot);
            self.react(view, slot, tap);
        } else if input.pos.y < scale_area(view).y {
            self.speak_question();
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
}

impl Default for Bonds {
    fn default() -> Self {
        Self::new()
    }
}

impl Game for Bonds {
    fn update(&mut self, input: &Input, view: &View, dt: f32) {
        self.t += dt;
        self.streak_bump.update(dt);
        self.prompt_bump.update(dt);
        for b in &mut self.press {
            b.update(dt);
        }
        for w in &mut self.wobble {
            *w = (*w - dt * 1.8).max(0.0);
        }
        self.tilt = approach(self.tilt, self.tilt_target(), 7.0, dt);
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

    const DT: f32 = 1.0 / 60.0;

    fn fresh() -> Bonds {
        crate::save::reset_for_test();
        profile::create("Mia", 0);
        rand::srand(11);
        Bonds::new()
    }

    fn views() -> [View; 4] {
        [
            View::ipad_landscape(),
            View::ipad_portrait(),
            View::fit(390.0, 844.0),
            View::fit(844.0, 390.0),
        ]
    }

    fn run(g: &mut Bonds, v: &View, secs: f32) {
        for _ in 0..(secs / DT) as usize {
            g.update(&Input::default(), v, DT);
        }
    }

    fn answer_right(g: &mut Bonds, v: &View) {
        let slot = g.question.slot_of_answer();
        let tap = g.pick(slot);
        g.react(v, slot, tap);
        run(g, v, CELEBRATE + 0.3);
        if matches!(g.mode, Mode::Report(_)) {
            g.update(&Input::press_at(vec2(1.0, 1.0)), v, DT);
            run(g, v, 1.0);
            g.update(&Input::press_at(vec2(1.0, 1.0)), v, DT);
        }
    }

    fn answer_wrong_once(g: &mut Bonds) {
        let wrong = (0..OPTIONS)
            .find(|i| *i != g.question.slot_of_answer())
            .unwrap();
        g.pick(wrong);
    }

    #[test]
    fn every_level_pool_makes_its_target_and_the_pools_rise() {
        for level in 0..LEVELS {
            let pool = level_pool(level);
            assert!(!pool.is_empty());
            for (t, a) in &pool {
                assert!([5, 10, 20, 100].contains(t));
                assert!(*a >= step_of(*t) && *a < *t, "{t} {a}");
                assert_eq!(a % step_of(*t), 0);
            }
        }
        let tops: Vec<i32> = (0..LEVELS)
            .map(|l| level_pool(l).iter().map(|p| p.0).max().unwrap())
            .collect();
        assert_eq!(tops, vec![5, 10, 10, 20, 20, 100, 100]);
        assert!(level_pool(6).len() > level_pool(4).len());
        assert!(level_pool(3).iter().all(|p| p.1 >= 11));
        assert!(level_pool(5).iter().all(|p| p.1 % 10 == 0));
    }

    #[test]
    fn the_first_levels_use_beads_and_later_ones_do_not() {
        assert!(beads(0) && beads(1));
        assert!((2..LEVELS).all(|l| !beads(l)));
    }

    #[test]
    fn options_hold_the_answer_once_with_no_duplicates() {
        rand::srand(5);
        for level in 0..LEVELS {
            for (t, a) in level_pool(level) {
                for _ in 0..20 {
                    let q = Question::from_pair(level, t, a);
                    assert_eq!(q.options.len(), OPTIONS, "{t} {a}");
                    assert_eq!(q.options.iter().filter(|v| **v == t - a).count(), 1);
                    let mut s = q.options.clone();
                    s.sort_unstable();
                    s.dedup();
                    assert_eq!(s.len(), OPTIONS);
                    assert!(q.options.iter().all(|v| *v >= 1 && *v < t));
                    assert!(q.options.iter().all(|v| v % step_of(t) == 0));
                    assert_eq!(q.options[q.slot_of_answer()], t - a);
                }
            }
        }
    }

    #[test]
    fn the_tilt_follows_which_pan_is_heavier() {
        assert!(tilt_for(7, 10, 10) < 0.0);
        assert_eq!(tilt_for(10, 10, 10), 0.0);
        assert!(tilt_for(14, 10, 10) > 0.0);
        assert!(tilt_for(19, 10, 10) <= MAX_TILT && tilt_for(0, 10, 10) >= -MAX_TILT);
    }

    #[test]
    fn the_right_weight_balances_and_a_wrong_one_tips_the_scale() {
        let v = View::ipad_landscape();
        let mut g = fresh();
        g.question = Question::from_pair(1, 10, 6);
        g.load();
        answer_wrong_once(&mut g);
        run(&mut g, &v, 0.5);
        assert!(g.tilt.abs() > 0.02);
        run(&mut g, &v, 1.0);
        assert!(g.on_pan.is_none());
        let slot = g.question.slot_of_answer();
        g.pick(slot);
        run(&mut g, &v, 0.6);
        assert!(g.tilt.abs() < 0.02, "{}", g.tilt);
        assert_eq!(g.on_pan, Some(4));
    }

    #[test]
    fn the_question_is_spoken_as_number_and_what_makes_target() {
        let mut g = fresh();
        g.question = Question::from_pair(1, 10, 6);
        g.load();
        let said: Vec<Say> = g.speech.iter().map(|s| s.1).collect();
        assert_eq!(
            said,
            vec![Say::Number(6), Say::Line(Line::Makes), Say::Number(10)]
        );
        g.question = Question::from_pair(5, 100, 30);
        g.load();
        assert_eq!(g.speech[2].1, sfx::number(100));
    }

    #[test]
    fn new_items_come_before_old_ones_and_nothing_repeats_in_a_window() {
        let mut g = fresh();
        g.start_level(4);
        let mut seen = vec![];
        for _ in 0..level_pool(4).len() {
            seen.push(g.question.pair());
            answer_right(&mut g, &View::ipad_landscape());
        }
        let mut s = seen.clone();
        s.sort_unstable();
        s.dedup();
        assert!(s.len() >= seen.len() - 3, "too many repeats: {seen:?}");
        for (i, p) in seen.iter().enumerate() {
            let near = &seen[i.saturating_sub(RECENT / 2)..i];
            assert!(!near.contains(p), "{p:?} came back at once");
        }
    }

    #[test]
    fn a_missed_bond_comes_back_after_its_gap() {
        let mut g = fresh();
        g.start_level(4);
        let missed = (20, 8);
        remember(4, missed.0, missed.1, Memory::default().after(100, false));
        for p in level_pool(4) {
            if p != missed {
                remember(4, p.0, p.1, Memory::default().after(100, true));
            }
        }
        let (p, review) = pick_item(4, &[], 100 + GAPS[0], false);
        assert_eq!((p, review), (missed, true));
        let (p, _) = pick_item(4, &[], 100 + 1, false);
        assert_ne!(p, missed);
    }

    #[test]
    fn four_right_in_a_row_level_up_and_save_stars() {
        let v = View::ipad_landscape();
        let mut g = fresh();
        for _ in 0..UP_STREAK {
            answer_right(&mut g, &v);
        }
        assert_eq!(g.ladder.level, 1);
        assert_eq!(profile::unlocked(GAME), 1);
        assert_eq!(profile::level(GAME), 1);
        assert_eq!(profile::stars(GAME, 0), 3);
        assert_eq!(Bonds::new().ladder.level, 1);
    }

    #[test]
    fn a_slip_costs_a_star_and_three_slips_ease_back_a_level() {
        let v = View::ipad_landscape();
        let mut g = fresh();
        answer_wrong_once(&mut g);
        for _ in 0..UP_STREAK + 1 {
            answer_right(&mut g, &v);
        }
        assert_eq!(profile::stars(GAME, 0), 2);
        assert_eq!(g.ladder.level, 1);
        for _ in 0..DOWN_SLIPS {
            answer_wrong_once(&mut g);
            let slot = g.question.slot_of_answer();
            g.pick(slot);
            g.celebrate = None;
            g.next_question();
        }
        assert_eq!(g.ladder.level, 0);
        assert_eq!(profile::level(GAME), 0);
        assert_eq!(profile::unlocked(GAME), 1);
    }

    #[test]
    fn a_new_player_starts_at_level_one_with_the_hint_and_each_player_resumes() {
        let v = View::ipad_landscape();
        let mut g = fresh();
        assert!(g.first_time);
        assert_eq!(g.ladder.level, 0);
        assert!(g.question.target == 5);
        for _ in 0..UP_STREAK * 2 {
            answer_right(&mut g, &v);
        }
        assert_eq!(Bonds::new().ladder.level, 2);
        profile::create("Leo", 3);
        let other = Bonds::new();
        assert_eq!(other.ladder.level, 0);
        assert!(other.first_time);
    }

    #[test]
    fn the_level_picker_only_opens_unlocked_levels() {
        let v = View::ipad_landscape();
        let mut g = fresh();
        for _ in 0..UP_STREAK {
            answer_right(&mut g, &v);
        }
        g.mode = Mode::Levels;
        let tiles = level_tiles(&v);
        g.update(&Input::press_at(tiles[4].center()), &v, DT);
        assert_eq!(g.mode, Mode::Levels);
        g.update(&Input::press_at(tiles[0].center()), &v, DT);
        assert_eq!(g.mode, Mode::Play);
        assert_eq!(g.ladder.level, 0);
    }

    #[test]
    fn the_levels_button_appears_after_the_first_success() {
        let v = View::ipad_landscape();
        let mut g = fresh();
        let at = levels_button(&v).center();
        g.update(&Input::press_at(at), &v, DT);
        assert_eq!(g.mode, Mode::Play);
        answer_right(&mut g, &v);
        g.update(&Input::press_at(at), &v, DT);
        assert_eq!(g.mode, Mode::Levels);
    }

    #[test]
    fn clearing_the_top_level_posts_the_star_total_once() {
        let mut g = fresh();
        let v = View::ipad_landscape();
        let last = LEVELS - 1;
        profile::unlock(GAME, last as u32);
        g.start_level(last);
        for _ in 0..UP_STREAK {
            let slot = g.question.slot_of_answer();
            let tap = g.pick(slot);
            g.react(&v, slot, tap);
            g.celebrate = None;
            g.next_question();
        }
        assert_eq!(profile::get(GAME, "cleared"), 1);
        assert!(matches!(g.report, Some(Report::Cleared { rank: Some(0) })));
        assert_eq!(profile::board(GAME).len(), 1);
        assert_eq!(Bonds::new().ladder.stars, LEVELS);
    }

    #[test]
    fn a_scripted_weak_player_still_progresses_and_a_strong_one_is_not_held_back() {
        let v = View::ipad_landscape();
        let mut weak = fresh();
        let mut steps = 0;
        while weak.ladder.level == 0 && steps < 60 {
            if steps % 5 == 0 {
                answer_wrong_once(&mut weak);
            }
            answer_right(&mut weak, &v);
            steps += 1;
        }
        assert_eq!(weak.ladder.level, 1, "weak player stuck after {steps}");
        let mut strong = fresh();
        for _ in 0..UP_STREAK as usize * (LEVELS - 1) {
            answer_right(&mut strong, &v);
        }
        assert_eq!(strong.ladder.level, LEVELS - 1);
    }

    #[test]
    fn scale_layout_fits_the_play_area_on_every_screen() {
        for v in views() {
            let area = scale_area(&v);
            assert!(area.h >= 90.0, "{area:?} in {v:?}");
            for tilt in [-MAX_TILT, 0.0, MAX_TILT] {
                let g = scale_geo(&v, tilt);
                for end in [g.left_end, g.right_end] {
                    assert!(end.x - g.pan_w / 2.0 >= area.x - 0.5, "{v:?}");
                    assert!(end.x + g.pan_w / 2.0 <= area.x + area.w + 0.5, "{v:?}");
                }
                assert!(g.bottom() <= area.y + area.h + 0.5, "{tilt} {g:?} {area:?}");
                assert!(g.left_end.x + g.pan_w / 2.0 < g.right_end.x - g.pan_w / 2.0);
            }
        }
    }

    #[test]
    fn answer_buttons_are_big_and_clear_the_scale() {
        for v in views() {
            let rects = button_rects(&v);
            for (i, r) in rects.iter().enumerate() {
                assert!(r.w >= 90.0 && r.h >= 60.0, "{r:?}");
                assert!(r.x >= 0.0 && r.x + r.w <= v.w && r.y + r.h <= v.h);
                if i > 0 && side_layout(&v) {
                    assert!(r.y >= rects[i - 1].y + rects[i - 1].h);
                } else if i > 0 {
                    assert!(r.x >= rects[i - 1].x + rects[i - 1].w);
                }
            }
            let a = scale_area(&v);
            assert!(side_layout(&v) || rects[0].y >= a.y + a.h);
            if side_layout(&v) {
                assert!(rects.iter().all(|r| r.x >= a.x + a.w));
            }
            assert!(!hint_room(&v) || hint_pos(&v).y > a.y + a.h - 1.0);
        }
    }

    #[test]
    fn beads_stay_inside_their_pan() {
        for count in 1..=10 {
            let dish = vec2(300.0, 400.0);
            let spots = bead_spots(count, dish, 120.0);
            assert_eq!(spots.len(), count);
            for p in spots {
                assert!((p.x - dish.x).abs() <= 60.0 * 0.85);
                assert!(p.y < dish.y);
            }
        }
    }

    #[test]
    fn level_tiles_fit_on_every_screen_and_clear_the_play_button() {
        for v in views() {
            let tiles = level_tiles(&v);
            assert_eq!(tiles.len(), LEVELS);
            for r in &tiles {
                assert!(r.x >= 0.0 && r.x + r.w <= v.w && r.y >= 100.0 && r.y + r.h <= v.h);
                assert!(r.w >= 120.0 && r.h >= 80.0, "{r:?} in {v:?}");
                assert!(r.y + r.h <= play_button(&v).y);
            }
        }
    }

    #[test]
    fn the_catalog_lists_bonds_as_a_learning_game_with_its_levels() {
        assert_eq!(crate::mastery::bands_of(GAME), Some(LEVELS));
        assert!(crate::games::find(GAME).is_some());
    }

    #[test]
    fn tapping_the_prompt_repeats_the_question() {
        let v = View::ipad_landscape();
        let mut g = fresh();
        g.speech.clear();
        g.update(
            &Input::press_at(vec2(v.w / 2.0, scale_area(&v).y - 10.0)),
            &v,
            DT,
        );
        assert!(!g.speech.is_empty());
    }
}
