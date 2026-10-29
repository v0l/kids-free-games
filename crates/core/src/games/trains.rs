use crate::engine::*;
use crate::feel::{self, Impact};
use crate::fx::*;
use crate::games::hotel::Cam;
use crate::geom::*;
use crate::learn::*;
use crate::profile;
use crate::rng as rand;
use crate::sfx::{self, Line, Say, Sfx};
use glam::{vec2, vec3, Vec3};

pub const GAME: &str = "trains";
pub const KEY: &str = GAME;
pub const LEVELS: usize = 8;
pub const OPTIONS: usize = 3;
pub const MAX_SHOWN: usize = 10;
pub const PER_CARRIAGE: usize = 5;
pub const SEAT_GAP: f32 = 0.5;
pub const CARRIAGE_LEN: f32 = 2.9;
pub const ENGINE_LEN: f32 = 3.2;
pub const ARRIVE_TIME: f32 = 1.9;
pub const DEPART_TIME: f32 = 2.2;
pub const HOP_TIME: f32 = 0.6;
pub const HIDE_AFTER: f32 = 1.0;
pub const HINT_AFTER: f32 = 4.5;
pub const SPEECH_GAP: f32 = 1.0;
pub const RECENT: usize = 8;
pub const SAMPLES: usize = 40;
pub const GAPS: [u32; 5] = [6, 12, 30, 80, 200];
pub const SPRITES: [&str; 3] = ["bg", "grass", "platform"];
pub const STOP_X: f32 = 0.0;
pub const DECOR_RIDERS: usize = 5;
pub const FLOOR_Y: f32 = 0.45;
pub const PLATFORM_Y: f32 = 0.4;

pub fn level_name(level: usize) -> &'static str {
    [
        "Take away within 5",
        "Take away within 10",
        "Remember the riders",
        "Take away within 20",
        "Missing numbers",
        "Tens and ones",
        "Take away tens",
        "Borrowing",
    ][level.min(LEVELS - 1)]
}

pub fn level_example(level: usize) -> &'static str {
    [
        "5 - 2",
        "9 - 4",
        "8 - 3",
        "17 - 9",
        "12 - ? = 7",
        "52 - 7",
        "68 - 24",
        "63 - 28",
    ][level.min(LEVELS - 1)]
}

pub fn level_hint(level: usize) -> &'static str {
    [
        "Tap riders off the train",
        "Tap riders off the train",
        "Riders hide after a second",
        "Numbers only",
        "Find the number taken away",
        "Two digits minus one",
        "Two digits minus two",
        "Borrow from the tens",
    ][level.min(LEVELS - 1)]
}

pub fn target_secs(level: usize) -> f32 {
    [5.0, 6.0, 7.0, 8.0, 9.0, 10.0, 12.0, 14.0][level.min(LEVELS - 1)]
}

pub fn taps_off(level: usize) -> bool {
    level < 2
}

pub fn riders_hide(level: usize) -> bool {
    level >= 2
}

pub fn pictures(level: usize) -> bool {
    level < 3
}

pub fn missing_form(level: usize) -> bool {
    level == 4
}

pub fn stars_score(s: i64) -> String {
    format!("{s} stars")
}

pub fn borrows(a: i32, b: i32) -> bool {
    b % 10 > a % 10
}

pub fn pool(level: usize) -> Vec<(i32, i32)> {
    let mut out = vec![];
    match level {
        0 => {
            for a in 2..=5 {
                for b in 1..a {
                    out.push((a, b));
                }
            }
        }
        1 | 2 => {
            for a in 3..=10 {
                for b in 1..a {
                    out.push((a, b));
                }
            }
        }
        3 => {
            for a in 11..=20 {
                for b in 1..a {
                    out.push((a, b));
                }
            }
        }
        4 => {
            for a in 6..=20 {
                for b in 1..a {
                    out.push((a, b));
                }
            }
        }
        5 => {
            for a in 21..=99 {
                for b in 2..=9 {
                    out.push((a, b));
                }
            }
        }
        6 => {
            for a in 21..=99 {
                for b in 10..a {
                    if !borrows(a, b) {
                        out.push((a, b));
                    }
                }
            }
        }
        _ => {
            for a in 21..=99 {
                for b in 11..a {
                    if borrows(a, b) {
                        out.push((a, b));
                    }
                }
            }
        }
    }
    out
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Memory {
    pub at: u32,
    pub due: u32,
    pub rung: u32,
}

impl Memory {
    pub fn seen(&self) -> bool {
        self.at > 0
    }

    pub fn after(self, round: u32, perfect: bool) -> Memory {
        let rung = if perfect {
            (self.rung + 1).min(GAPS.len() as u32 - 1)
        } else {
            0
        };
        Memory {
            at: round,
            due: round + GAPS[rung as usize],
            rung,
        }
    }

    fn pack(self) -> i64 {
        ((self.rung as i64) << 56)
            | ((self.due as i64 & 0x0FFF_FFFF) << 28)
            | (self.at as i64 & 0x0FFF_FFFF)
    }

    fn unpack(v: i64) -> Memory {
        Memory {
            at: (v & 0x0FFF_FFFF) as u32,
            due: ((v >> 28) & 0x0FFF_FFFF) as u32,
            rung: ((v >> 56) & 0xF) as u32,
        }
    }
}

fn memory_key(level: usize, a: i32, b: i32) -> String {
    format!("q{level}.{a}.{b}")
}

pub fn memory_of(level: usize, a: i32, b: i32) -> Memory {
    Memory::unpack(profile::get(GAME, &memory_key(level, a, b)))
}

fn remember(level: usize, a: i32, b: i32, m: Memory) {
    profile::set(GAME, &memory_key(level, a, b), m.pack());
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
            let (a, b) = rest.split_once('.')?;
            let m = Memory::unpack(crate::save::get(&k));
            Some(((a.parse().ok()?, b.parse().ok()?), m))
        })
        .collect()
}

pub fn pick_pair(
    level: usize,
    recent: &[(i32, i32)],
    round: u32,
    reviewed_lately: bool,
) -> ((i32, i32), bool) {
    let all = pool(level);
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
    pub a: i32,
    pub b: i32,
    pub options: Vec<i32>,
    pub seed: u32,
}

impl Question {
    pub fn result(&self) -> i32 {
        self.a - self.b
    }

    pub fn answer(&self) -> i32 {
        if missing_form(self.level) {
            self.b
        } else {
            self.result()
        }
    }

    pub fn pair(&self) -> (i32, i32) {
        (self.a, self.b)
    }

    pub fn from_pair(level: usize, a: i32, b: i32) -> Self {
        let mut q = Self {
            level,
            a,
            b,
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

    pub fn riders(&self) -> usize {
        if pictures(self.level) {
            self.a as usize
        } else {
            DECOR_RIDERS
        }
    }

    pub fn hops(&self) -> usize {
        if pictures(self.level) {
            self.b as usize
        } else {
            0
        }
    }

    pub fn sum_text(&self) -> String {
        if missing_form(self.level) {
            format!("{} - ? = {}", self.a, self.result())
        } else {
            format!("{} - {}", self.a, self.b)
        }
    }

    pub fn solved_text(&self) -> String {
        format!("{} - {} = {}", self.a, self.b, self.result())
    }
}

fn decoys(q: &Question) -> Vec<i32> {
    let answer = q.answer();
    let (a, b) = (q.a, q.b);
    let mut near: Vec<i32> = vec![answer - 1, answer + 1, answer - 2, answer + 2];
    if q.level >= 3 {
        near.push(answer - 10);
        near.push(answer + 10);
    }
    let mut smart: Vec<i32> = vec![];
    if missing_form(q.level) {
        smart.push(a + q.result());
        smart.push(q.result());
    } else {
        smart.push(a + b);
        smart.push(b);
        if borrows(a, b) && q.level >= 5 {
            let slip = (a / 10 * 10 - b / 10 * 10) + (b % 10 - a % 10);
            smart.push(slip);
        }
    }
    let max = if q.level < 3 {
        10
    } else if q.level < 5 {
        20
    } else {
        99
    };
    let ok = |v: &i32| *v >= 1 && *v <= max && *v != answer;
    let mut pool: Vec<i32> = vec![];
    for v in smart.into_iter().filter(ok) {
        if !pool.contains(&v) {
            pool.push(v);
        }
    }
    let mut rest: Vec<i32> = near.into_iter().filter(ok).collect();
    rest.sort_unstable();
    rest.dedup();
    let want = OPTIONS - 1;
    let mut picks: Vec<i32> = vec![];
    if q.level >= 2 && !pool.is_empty() {
        picks.push(pool[rand::gen_range(0, pool.len())]);
    }
    while picks.len() < want && !rest.is_empty() {
        let v = rest.swap_remove(rand::gen_range(0, rest.len()));
        if !picks.contains(&v) {
            picks.push(v);
        }
    }
    let mut v = 1;
    while picks.len() < want {
        if ok(&v) && !picks.contains(&v) {
            picks.push(v);
        }
        v += 1;
    }
    picks.insert(rand::gen_range(0, picks.len() + 1), answer);
    picks
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Phase {
    Arrive,
    Remove,
    Choose,
    Resolve,
    Depart,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Rider {
    Aboard,
    Hopping(f32),
    Off(f32),
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

pub fn carriages(riders: usize) -> usize {
    riders.div_ceil(PER_CARRIAGE).max(1)
}

pub fn train_len(riders: usize) -> f32 {
    ENGINE_LEN + carriages(riders) as f32 * CARRIAGE_LEN
}

pub fn seat(i: usize, riders: usize) -> Vec3 {
    let c = i / PER_CARRIAGE;
    let k = i % PER_CARRIAGE;
    let in_carriage = (riders - c * PER_CARRIAGE).min(PER_CARRIAGE);
    let front = train_len(riders) / 2.0 - ENGINE_LEN;
    let mid = front - CARRIAGE_LEN * (c as f32 + 0.5);
    let off = (k as f32 - (in_carriage as f32 - 1.0) / 2.0) * SEAT_GAP;
    vec3(mid + off, FLOOR_Y, 0.0)
}

pub fn platform_spot(k: usize) -> Vec3 {
    let col = k % 5;
    let row = k / 5;
    vec3(
        -2.0 + col as f32 * 1.05 + (row % 2) as f32 * 0.5,
        PLATFORM_Y,
        3.2 + row as f32 * 0.9,
    )
}

pub fn rider_look(seed: u32, i: usize) -> u8 {
    let mut x = seed ^ (i as u32).wrapping_mul(0x9E37_79B1);
    x ^= x >> 15;
    x = x.wrapping_mul(0x85EB_CA6B);
    x ^= x >> 13;
    (x % 40) as u8
}

pub fn hop_pos(from: Vec3, to: Vec3, u: f32) -> Vec3 {
    let u = u.clamp(0.0, 1.0);
    let flat = from.lerp(to, ease_out_cubic(u));
    vec3(
        flat.x,
        flat.y + (u * std::f32::consts::PI).sin() * 1.1,
        flat.z,
    )
}

pub fn cam(view: &View) -> Cam {
    let f = view.full();
    let aspect = f.w / f.h.max(1.0);
    let portrait = aspect < 1.0;
    let fov: f32 = if portrait { 0.8 } else { 0.62 };
    let span = if portrait { 10.6 } else { 12.5 };
    let dist = (span / 2.0 / ((fov / 2.0).tan() * aspect)).max(12.0);
    let pitch: f32 = if portrait { 0.42 } else { 0.36 };
    let target = if portrait {
        vec3(0.0, 0.6, -2.4)
    } else {
        vec3(0.0, 1.6, -0.2)
    };
    let eye = target + vec3(0.0, pitch.sin(), pitch.cos()) * dist;
    Cam {
        target,
        eye,
        fov,
        aspect,
    }
}

pub fn goal_row_y(view: &View) -> f32 {
    hud_bottom(view) + 34.0
}

pub fn prompt_rect(view: &View) -> Rect {
    let k = ui_scale(view).min(1.25);
    let w = (view.w - 60.0).min(560.0);
    Rect::new(
        view.w / 2.0 - w / 2.0,
        goal_row_y(view) + 34.0 * k,
        w,
        92.0 * k,
    )
}

pub fn levels_button(view: &View) -> Rect {
    let k = ui_scale(view);
    let w = 170.0 * k;
    let h = 46.0 * k;
    Rect::new(view.w - 30.0 - w, goal_row_y(view) - h / 2.0, w, h)
}

pub fn button_height(view: &View) -> f32 {
    124.0 * ui_scale(view)
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
    Rect::new(view.w / 2.0 - 170.0, view.h - 150.0, 340.0, 100.0)
}

pub struct Trains {
    pub mode: Mode,
    pub phase: Phase,
    pub phase_t: f32,
    pub choose_t: f32,
    pub idle: f32,
    pub misses: u32,
    pub first_time: bool,
    pub report: Option<Report>,
    pub ladder: Ladder,
    pub question: Question,
    pub attempt: Attempt,
    pub riders: Vec<Rider>,
    pub looks: Vec<u8>,
    pub spots: Vec<usize>,
    pub recent: Vec<(i32, i32)>,
    pub since_review: u32,
    pub wobble: Vec<f32>,
    pub press: Vec<Bump>,
    pub streak_bump: Bump,
    pub prompt_bump: Bump,
    pub shake: Shake,
    pub particles: Particles,
    pub popups: Popups,
    pub bursts: Vec<(Vec3, Color)>,
    pub speech: Vec<(f32, Say)>,
    pub t: f32,
    pub session: crate::mastery::Session,
    pub summary: String,
    pub pending_step: Option<Step>,
    pub puff_kick: f32,
    pub punch: f32,
    pub chuff: f32,
}

impl Trains {
    pub fn new() -> Self {
        let top = LEVELS - 1;
        let unlocked = (profile::unlocked(GAME) as usize).min(top);
        let mut level = (profile::level(GAME) as usize).min(unlocked);
        while level < unlocked && crate::mastery::mastered(GAME, level) {
            level += 1;
        }
        let cleared = profile::get(GAME, "cleared") > 0;
        let mut ladder = Ladder::new(LEVELS);
        ladder.level = level;
        ladder.stars = if cleared { LEVELS } else { unlocked };
        let mut g = Self {
            mode: Mode::Play,
            phase: Phase::Arrive,
            phase_t: 0.0,
            choose_t: 0.0,
            idle: 0.0,
            misses: 0,
            first_time: profile::get(GAME, "played") == 0,
            report: None,
            ladder,
            question: Question::from_pair(0, 2, 1),
            attempt: Attempt::new(OPTIONS),
            riders: vec![],
            looks: vec![],
            spots: vec![],
            recent: vec![],
            since_review: 9,
            wobble: vec![0.0; OPTIONS],
            press: (0..OPTIONS).map(|_| Bump::default()).collect(),
            streak_bump: Bump::default(),
            prompt_bump: Bump::default(),
            shake: Shake::default(),
            particles: Particles::default(),
            popups: Popups::default(),
            bursts: vec![],
            speech: vec![],
            t: 0.0,
            session: crate::mastery::Session::default(),
            summary: String::new(),
            pending_step: None,
            puff_kick: 0.0,
            punch: 0.0,
            chuff: 0.0,
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

    fn next_question(&mut self) {
        let level = self.ladder.level;
        let (pair, review) = pick_pair(level, &self.recent, round(), self.since_review < 2);
        self.since_review = if review { 0 } else { self.since_review + 1 };
        self.recent.push(pair);
        if self.recent.len() > RECENT * 2 {
            self.recent.remove(0);
        }
        self.question = Question::from_pair(level, pair.0, pair.1);
        self.load();
    }

    fn load(&mut self) {
        let n = self.question.riders();
        self.riders = vec![Rider::Aboard; n];
        self.spots = vec![0; n];
        self.looks = (0..n).map(|i| rider_look(self.question.seed, i)).collect();
        self.attempt = Attempt::new(OPTIONS);
        self.wobble = vec![0.0; OPTIONS];
        self.press = (0..OPTIONS).map(|_| Bump::default()).collect();
        self.phase = Phase::Arrive;
        self.phase_t = 0.0;
        self.choose_t = 0.0;
        self.idle = 0.0;
        self.speech.clear();
        self.prompt_bump.kick();
        self.pending_step = None;
        sfx::play(Sfx::Whoosh);
    }

    pub fn removed(&self) -> usize {
        self.riders
            .iter()
            .filter(|r| !matches!(r, Rider::Aboard))
            .count()
    }

    pub fn aboard(&self) -> usize {
        self.riders.len() - self.removed()
    }

    pub fn train_front(&self) -> f32 {
        let half = train_len(self.riders.len()) / 2.0;
        let away = train_len(self.riders.len()) + 24.0;
        match self.phase {
            Phase::Arrive => {
                let k = ease_out_cubic((self.phase_t / ARRIVE_TIME).min(1.0));
                half + STOP_X - away * (1.0 - k)
            }
            Phase::Depart => {
                let k = (self.phase_t / DEPART_TIME).clamp(0.0, 1.0);
                half + STOP_X + away * k * k
            }
            _ => half + STOP_X,
        }
    }

    pub fn train_speed(&self) -> f32 {
        let away = train_len(self.riders.len()) + 24.0;
        match self.phase {
            Phase::Arrive => {
                let k = (self.phase_t / ARRIVE_TIME).min(1.0);
                3.0 * away / ARRIVE_TIME * (1.0 - k) * (1.0 - k)
            }
            Phase::Depart => {
                let k = (self.phase_t / DEPART_TIME).clamp(0.0, 1.0);
                2.0 * away / DEPART_TIME * k
            }
            _ => 0.0,
        }
    }

    pub fn rider_world(&self, i: usize) -> Vec3 {
        let front = self.train_front() - train_len(self.riders.len()) / 2.0;
        let seat = seat(i, self.riders.len());
        let at = vec3(seat.x + front, seat.y, seat.z);
        let spot = platform_spot(self.spots[i]);
        match self.riders[i] {
            Rider::Aboard => at,
            Rider::Hopping(t) => {
                let from = vec3(seat.x + STOP_X, seat.y, seat.z);
                hop_pos(from, spot, t / HOP_TIME)
            }
            Rider::Off(_) => spot,
        }
    }

    fn send_off(&mut self, i: usize) {
        self.spots[i] = self.removed();
        self.riders[i] = Rider::Hopping(0.0);
    }

    fn hop_order(&self, rank: usize) -> Option<usize> {
        self.riders
            .iter()
            .enumerate()
            .rev()
            .filter(|(_, r)| matches!(r, Rider::Aboard))
            .map(|(i, _)| i)
            .nth(rank)
    }

    pub fn hide_k(&self) -> f32 {
        let level = self.ladder.level;
        if !pictures(level) {
            return 1.0;
        }
        if !riders_hide(level) {
            return 0.0;
        }
        match self.phase {
            Phase::Choose => ((self.choose_t - HIDE_AFTER) / 0.4).clamp(0.0, 1.0),
            Phase::Resolve => 1.0 - (self.phase_t / 0.3).clamp(0.0, 1.0),
            _ => 0.0,
        }
    }

    pub fn tap_rider(&mut self, i: usize) -> bool {
        if self.phase != Phase::Remove || !matches!(self.riders.get(i), Some(Rider::Aboard)) {
            return false;
        }
        if self.removed() >= self.question.hops() {
            return false;
        }
        self.send_off(i);
        self.idle = 0.0;
        let n = self.removed();
        sfx::play(Sfx::Pop);
        sfx::play_step(Sfx::Tick, n);
        sfx::say(sfx::number(n as i32));
        let c = hsv(i as f32 * 0.17, 0.5, 1.0);
        self.bursts.push((self.rider_world(i), c));
        if n >= self.question.hops() {
            self.phase = Phase::Choose;
            self.phase_t = 0.0;
            self.choose_t = 0.0;
            self.say_left();
        }
        true
    }

    fn say_left(&mut self) {
        self.speech.push((0.9, Say::Line(Line::Left)));
    }

    pub fn speak_question(&mut self) {
        let q = &self.question;
        let mut lines = vec![sfx::number(q.a)];
        if missing_form(q.level) {
            lines.push(Say::Line(Line::Missing));
            lines.push(sfx::number(q.result()));
        } else {
            lines.push(Say::Line(Line::TakeAway));
            lines.push(sfx::number(q.b));
        }
        let start = self.speech.first().map_or(0.0, |s| s.0);
        let mut at = start.max(0.0);
        let mut queued = vec![];
        for l in lines {
            queued.push((at, l));
            at += SPEECH_GAP;
        }
        if taps_off(q.level) && self.first_time {
            queued.push((at, Say::Line(Line::Tap)));
        } else if !taps_off(q.level) {
            queued.push((at, Say::Line(Line::Left)));
        }
        self.speech = queued;
    }

    pub fn pick(&mut self, slot: usize) -> Tap {
        if self.phase != Phase::Choose || slot >= OPTIONS {
            return Tap::Ignored;
        }
        let correct = self.question.options[slot] == self.question.answer();
        let level = self.ladder.level;
        let result = self.attempt.choose(slot, correct, &mut self.ladder);
        if let Tap::Right(step) = result {
            let answer = self.session.answer(self.attempt.wrong == 0, false);
            crate::mastery::record(GAME, level, answer, target_secs(level));
            let (a, b) = self.question.pair();
            let r = round() + 1;
            profile::set(GAME, "round", r as i64);
            remember(
                level,
                a,
                b,
                memory_of(level, a, b).after(r, self.attempt.wrong == 0),
            );
            self.record(step);
            self.pending_step = Some(step);
            self.phase = Phase::Resolve;
            self.phase_t = 0.0;
        }
        match result {
            Tap::Wrong | Tap::Reveal | Tap::Again => self.wobble[slot] = 1.0,
            _ => {}
        }
        if result != Tap::Ignored {
            self.press[slot].kick();
        }
        self.idle = 0.0;
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

    fn resolve_length(&self) -> f32 {
        0.4 + self.auto_hops() as f32 * 0.2 + HOP_TIME + 0.9
    }

    fn auto_hops(&self) -> usize {
        self.question.hops().saturating_sub(self.removed())
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
                self.punch = 1.0;
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
                            .add(mid + vec2(0.0, 90.0), "Subtraction star!", WHITE, 50.0);
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

    fn tap_target(&self, view: &View, input: &Input) -> Option<usize> {
        let c = self.cam_of(view);
        let reach = (input.mm() * 5.0).max(34.0);
        self.riders
            .iter()
            .enumerate()
            .filter(|(_, r)| matches!(r, Rider::Aboard))
            .filter_map(|(i, _)| {
                let p = self.rider_world(i) + vec3(0.0, 0.35, 0.0);
                let s = c.project(view, p)?;
                let r = reach.max(c.px_per_unit(view, p) * 0.5);
                let d = s.distance(input.pos);
                (d <= r).then_some((i, d))
            })
            .min_by(|a, b| a.1.partial_cmp(&b.1).unwrap())
            .map(|(i, _)| i)
    }

    fn begin_choose(&mut self) {
        self.speak_question();
        if taps_off(self.ladder.level) {
            self.phase = Phase::Remove;
        } else {
            self.phase = Phase::Choose;
        }
        self.phase_t = 0.0;
        self.choose_t = 0.0;
        self.idle = 0.0;
        self.session.start_question();
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

    pub fn hint_rider(&self) -> Option<usize> {
        if self.phase != Phase::Remove || self.idle < HINT_AFTER && !self.first_time {
            return None;
        }
        if self.first_time && self.idle < 1.5 {
            return None;
        }
        self.hop_order(0)
    }

    pub fn glow_slot(&self) -> Option<usize> {
        let late = self.choose_t > 8.0 && self.ladder.level < 2;
        (self.attempt.revealed() || late).then(|| self.question.slot_of_answer())
    }

    fn update_play(&mut self, input: &Input, view: &View, dt: f32) {
        self.phase_t += dt;
        self.chuff -= dt;
        if self.train_speed() > 2.0 && self.chuff <= 0.0 {
            self.chuff = (0.34 - self.train_speed() * 0.006).max(0.12);
            sfx::play(Sfx::Click);
            self.puff_kick = self.puff_kick.max(0.5);
        }
        match self.phase {
            Phase::Arrive => {
                if self.phase_t >= ARRIVE_TIME {
                    sfx::play(Sfx::Bounce);
                    self.shake.add(0.12);
                    self.puff_kick = 1.0;
                    self.begin_choose();
                }
            }
            Phase::Remove | Phase::Choose => {
                self.session.tick(dt);
                self.idle += dt;
                if self.phase == Phase::Choose {
                    self.choose_t += dt;
                }
            }
            Phase::Resolve => {
                let n = self.auto_hops();
                let before = self.phase_t - dt;
                for k in 0..n {
                    let at = 0.4 + k as f32 * 0.2;
                    if before < at && self.phase_t >= at {
                        if let Some(i) = self.hop_order(0) {
                            self.send_off(i);
                            sfx::play(Sfx::Pop);
                            self.bursts
                                .push((self.rider_world(i), hsv(k as f32 * 0.2, 0.5, 1.0)));
                        }
                    }
                }
                if self.phase_t >= self.resolve_length() {
                    self.phase = Phase::Depart;
                    self.phase_t = 0.0;
                    sfx::play(Sfx::Whoosh);
                    sfx::say(Say::Line(Line::Choo));
                    self.puff_kick = 1.5;
                }
            }
            Phase::Depart => {
                if self.phase_t >= DEPART_TIME {
                    let step = self.pending_step.take();
                    if step.is_some() && self.report.is_some() {
                        self.mode = Mode::Report(0.0);
                    }
                    self.next_question();
                    return;
                }
            }
        }
        self.update_speech(dt);
        if input.pressed {
            if levels_button(view).contains(input.pos)
                && !self.first_time
                && matches!(self.phase, Phase::Remove | Phase::Choose)
            {
                self.mode = Mode::Levels;
                return;
            }
            if matches!(self.phase, Phase::Remove | Phase::Choose)
                && prompt_rect(view).contains(input.pos)
            {
                self.speak_question();
                return;
            }
            if self.phase == Phase::Choose {
                if let Some(slot) = button_rects(view)
                    .iter()
                    .position(|r| r.contains(input.pos))
                {
                    let tap = self.pick(slot);
                    self.react(view, slot, tap);
                    return;
                }
            }
            if self.phase == Phase::Remove {
                if let Some(i) = self.tap_target(view, input) {
                    self.tap_rider(i);
                    self.react_remove(view);
                } else if self.removed() >= self.question.hops() {
                    self.idle = 0.0;
                }
            }
        }
    }

    fn react_remove(&mut self, view: &View) {
        let _ = view;
        self.streak_bump.kick();
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

    pub fn train_center(&self) -> f32 {
        self.train_front() - train_len(self.riders.len()) / 2.0
    }

    pub fn cam_of(&self, view: &View) -> Cam {
        let mut c = cam(view);
        let pan = vec3((self.train_center() * 0.16).clamp(-4.0, 4.0), 0.0, 0.0);
        c.eye += pan;
        c.target += pan;
        c.fov *= 1.0 - 0.06 * ease_out_cubic(self.punch.min(1.0));
        c
    }

    pub fn left_in_train(&self) -> i32 {
        let shown = matches!(self.phase, Phase::Resolve | Phase::Depart)
            || self.attempt.solved
            || (self.phase == Phase::Choose && taps_off(self.ladder.level));
        if shown {
            self.question.result()
        } else {
            self.question.a
        }
    }
}

impl Default for Trains {
    fn default() -> Self {
        Self::new()
    }
}

impl Game for Trains {
    fn update(&mut self, input: &Input, view: &View, dt: f32) {
        self.t += dt;
        self.streak_bump.update(dt);
        self.prompt_bump.update(dt);
        self.puff_kick = (self.puff_kick - dt * 1.4).max(0.0);
        self.punch = (self.punch - dt * 2.2).max(0.0);
        for b in &mut self.press {
            b.update(dt);
        }
        for w in &mut self.wobble {
            *w = (*w - dt * 1.8).max(0.0);
        }
        let mut landed = 0;
        for r in &mut self.riders {
            match *r {
                Rider::Hopping(t) => {
                    *r = if t + dt >= HOP_TIME {
                        landed += 1;
                        Rider::Off(0.0)
                    } else {
                        Rider::Hopping(t + dt)
                    }
                }
                Rider::Off(t) => *r = Rider::Off(t + dt),
                Rider::Aboard => {}
            }
        }
        if landed > 0 {
            sfx::play(Sfx::Bounce);
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

    const DT: f32 = 1.0 / 60.0;

    fn fresh() -> Trains {
        crate::save::reset_for_test();
        profile::create("Mia", 0);
        rand::srand(11);
        Trains::new()
    }

    fn views() -> [View; 2] {
        [View::ipad_landscape(), View::ipad_portrait()]
    }

    fn run(g: &mut Trains, v: &View, secs: f32) {
        for _ in 0..(secs / DT) as usize {
            g.update(&Input::default(), v, DT);
        }
    }

    fn ready(g: &mut Trains, v: &View) {
        let mut guard = 0;
        while !matches!(g.phase, Phase::Remove | Phase::Choose) && guard < 2000 {
            g.update(&Input::default(), v, DT);
            guard += 1;
        }
        assert!(matches!(g.phase, Phase::Remove | Phase::Choose));
    }

    fn solve(g: &mut Trains, v: &View, clean: bool) -> Tap {
        ready(g, v);
        if g.phase == Phase::Remove {
            for i in (0..g.riders.len()).rev().take(g.question.hops()) {
                g.tap_rider(i);
            }
        }
        if !clean {
            let slot = (0..OPTIONS)
                .find(|s| *s != g.question.slot_of_answer())
                .unwrap();
            let t = g.pick(slot);
            g.react(v, slot, t);
        }
        let slot = g.question.slot_of_answer();
        let tap = g.pick(slot);
        g.react(v, slot, tap);
        loop {
            let before = g.question.pair();
            let level = g.ladder.level;
            let serial = g.question.seed;
            run(g, v, 0.1);
            if g.question.seed != serial || g.question.pair() != before || g.ladder.level != level {
                break;
            }
            if g.mode != Mode::Play {
                break;
            }
        }
        if let Mode::Report(_) = g.mode {
            g.report = None;
            g.mode = Mode::Play;
        }
        tap
    }

    #[test]
    fn every_level_has_a_pool_of_valid_sums() {
        for level in 0..LEVELS {
            let p = pool(level);
            assert!(!p.is_empty());
            for (a, b) in p {
                assert!(b >= 1 && b < a, "level {level}: {a} - {b}");
                match level {
                    0 => assert!(a <= 5),
                    1 | 2 => assert!(a <= 10),
                    3 | 4 => assert!(a <= 20),
                    _ => assert!(a <= 99),
                }
                match level {
                    5 => assert!(b <= 9),
                    6 => assert!(b >= 10 && !borrows(a, b), "{a} - {b}"),
                    7 => assert!(b >= 11 && borrows(a, b), "{a} - {b}"),
                    _ => {}
                }
            }
        }
    }

    #[test]
    fn pools_grow_and_difficulty_rises_level_by_level() {
        let avg = |l: usize| {
            let p = pool(l);
            p.iter().map(|(a, _)| *a as f32).sum::<f32>() / p.len() as f32
        };
        assert!(avg(0) < avg(1) && avg(1) < avg(3) && avg(3) < avg(5));
        for level in 1..LEVELS {
            assert!(pool(level).len() >= 30, "level {level}");
        }
        for level in 5..LEVELS {
            assert!(pool(level).len() >= 200, "level {level}");
        }
        assert!((0..LEVELS).all(|l| target_secs(l) <= target_secs((l + 1).min(LEVELS - 1))));
        assert!(taps_off(0) && taps_off(1) && !taps_off(2));
        assert!(pictures(2) && !pictures(3));
        assert!(riders_hide(2) && !riders_hide(1));
    }

    #[test]
    fn the_same_sum_never_comes_twice_close_together() {
        let mut g = fresh();
        let v = View::ipad_landscape();
        for level in 0..LEVELS {
            g.start_level(level);
            let mut last: Vec<(i32, i32)> = vec![];
            for n in 0..120 {
                let pair = g.question.pair();
                let window = RECENT.min(pool(level).len() / 2);
                let tail = &last[last.len().saturating_sub(window)..];
                assert!(
                    !tail.contains(&pair),
                    "level {level} question {n}: {pair:?}"
                );
                last.push(pair);
                solve(&mut g, &v, true);
                if g.ladder.level != level {
                    g.start_level(level);
                    last.clear();
                }
            }
        }
    }

    #[test]
    fn new_sums_come_before_old_ones_until_a_review_is_due() {
        let mut g = fresh();
        g.start_level(5);
        let mut seen = std::collections::HashSet::new();
        for _ in 0..5 {
            let p = pick_pair(5, &[], round(), false).0;
            remember(5, p.0, p.1, Memory::default().after(round() + 1, true));
            profile::set(GAME, "round", round() as i64 + 1);
            assert!(seen.insert(p), "repeated {p:?} before its gap");
        }
    }

    #[test]
    fn a_missed_sum_comes_back_after_its_gap_and_a_mastered_one_waits_longer() {
        let mut g = fresh();
        g.start_level(5);
        let missed = (47, 8);
        let solid = (63, 5);
        profile::set(GAME, "round", 100);
        remember(5, missed.0, missed.1, Memory::default().after(100, false));
        let mut m = Memory::default();
        for _ in 0..3 {
            m = m.after(100, true);
        }
        remember(5, solid.0, solid.1, m);
        assert_eq!(memory_of(5, 47, 8).due, 100 + GAPS[0]);
        assert!(memory_of(5, 63, 5).due >= 100 + GAPS[2]);
        let before = pick_pair(5, &[], 100 + GAPS[0] - 1, false);
        assert!(!before.1);
        let at = pick_pair(5, &[], 100 + GAPS[0], false);
        assert_eq!((at.0, at.1), (missed, true));
        let later = pick_pair(5, &[missed], 100 + GAPS[0], false);
        assert!(!later.1);
        assert!(GAPS.windows(2).all(|w| w[0] < w[1]));
    }

    #[test]
    fn memory_survives_the_save_round_trip_per_player() {
        let mut g = fresh();
        g.start_level(3);
        let m = Memory {
            at: 77,
            due: 107,
            rung: 3,
        };
        remember(3, 15, 6, m);
        assert_eq!(memory_of(3, 15, 6), m);
        let other = profile::create("Leo", 1).unwrap();
        profile::select(&other.id);
        assert!(!memory_of(3, 15, 6).seen());
        assert_eq!(seen_count(3), 0);
    }

    #[test]
    fn every_question_has_three_distinct_options_with_one_answer() {
        rand::srand(5);
        for level in 0..LEVELS {
            for (a, b) in pool(level) {
                let q = Question::from_pair(level, a, b);
                assert_eq!(q.options.len(), OPTIONS);
                assert_eq!(q.options.iter().filter(|v| **v == q.answer()).count(), 1);
                let mut s = q.options.clone();
                s.sort_unstable();
                s.dedup();
                assert_eq!(s.len(), OPTIONS, "{q:?}");
                assert!(q.options.iter().all(|v| *v >= 1), "{q:?}");
                assert_eq!(q.options[q.slot_of_answer()], q.answer());
            }
        }
    }

    #[test]
    fn options_never_grow_with_difficulty() {
        for level in 0..LEVELS {
            assert_eq!(Question::from_pair(level, 9, 3).options.len(), 3);
        }
    }

    #[test]
    fn later_levels_use_the_childs_likely_slips_as_wrong_answers() {
        rand::srand(21);
        let mut hit = 0;
        for _ in 0..200 {
            let q = Question::from_pair(5, 52, 7);
            if q.options.contains(&59) || q.options.contains(&7) {
                hit += 1;
            }
        }
        assert!(hit > 120, "{hit}");
        let early = (0..200)
            .filter(|_| {
                let q = Question::from_pair(0, 5, 2);
                q.options.contains(&7)
            })
            .count();
        assert!(early < hit);
    }

    #[test]
    fn the_missing_number_level_asks_for_the_amount_taken_away() {
        let q = Question::from_pair(4, 12, 5);
        assert_eq!(q.answer(), 5);
        assert_eq!(q.sum_text(), "12 - ? = 7");
        let q = Question::from_pair(3, 12, 5);
        assert_eq!(q.answer(), 7);
        assert_eq!(q.sum_text(), "12 - 5");
        assert_eq!(q.solved_text(), "12 - 5 = 7");
    }

    #[test]
    fn picture_levels_seat_exactly_the_riders_the_sum_names() {
        let q = Question::from_pair(1, 8, 3);
        assert_eq!((q.riders(), q.hops()), (8, 3));
        let q = Question::from_pair(5, 52, 7);
        assert_eq!(q.hops(), 0);
        assert!(q.riders() <= MAX_SHOWN);
    }

    #[test]
    fn seats_stay_on_their_carriage_and_do_not_overlap() {
        for riders in 1..=10 {
            let mut xs: Vec<f32> = (0..riders).map(|i| seat(i, riders).x).collect();
            xs.sort_by(|a, b| a.partial_cmp(b).unwrap());
            for w in xs.windows(2) {
                assert!(w[1] - w[0] >= SEAT_GAP - 1e-4, "{riders}: {xs:?}");
            }
            let half = train_len(riders) / 2.0;
            assert!(xs.iter().all(|x| x.abs() < half));
        }
        assert_eq!(carriages(5), 1);
        assert_eq!(carriages(6), 2);
    }

    #[test]
    fn a_hop_lands_on_the_platform_and_peaks_in_between() {
        let from = vec3(1.0, 0.6, 0.0);
        let to = platform_spot(0);
        assert!((hop_pos(from, to, 1.0) - to).length() < 1e-4);
        assert!(hop_pos(from, to, 0.5).y > 0.8);
        assert!((hop_pos(from, to, 0.0) - from).length() < 1e-4);
    }

    #[test]
    fn the_camera_frames_the_train_in_both_orientations() {
        for v in views() {
            let c = cam(&v);
            let f = v.full();
            for riders in [3usize, 10] {
                for x in [-train_len(riders) / 2.0, train_len(riders) / 2.0] {
                    let p = c.project(&v, vec3(x, 0.5, 0.0)).expect("on screen");
                    if riders <= 10 {
                        assert!(
                            p.x > f.x - 5.0 && p.x < f.x + f.w + 5.0,
                            "{riders} {p:?} {f:?}"
                        );
                    }
                }
            }
            let p = c.project(&v, platform_spot(9)).unwrap();
            assert!(p.y < f.y + f.h && p.x > f.x && p.x < f.x + f.w, "{p:?}");
        }
    }

    #[test]
    fn the_train_pulls_in_stops_and_leaves() {
        let mut g = fresh();
        let v = View::ipad_landscape();
        let start = g.train_front();
        assert!(start < -10.0);
        ready(&mut g, &v);
        let stop = g.train_front();
        assert!((stop - train_len(g.riders.len()) / 2.0 - STOP_X).abs() < 1e-3);
        assert_eq!(g.train_speed(), 0.0);
        solve(&mut g, &v, true);
        assert!(g.question.seed != 0);
    }

    #[test]
    fn tapping_riders_off_counts_them_aloud_and_opens_the_answer() {
        let mut g = fresh();
        let v = View::ipad_landscape();
        g.question = Question::from_pair(0, 5, 2);
        g.load();
        ready(&mut g, &v);
        assert_eq!(g.phase, Phase::Remove);
        crate::sfx::take_voice();
        assert!(g.tap_rider(4));
        assert_eq!(crate::sfx::take_voice(), Some(Say::Number(1)));
        assert!(!g.tap_rider(4));
        assert_eq!(g.phase, Phase::Remove);
        assert!(g.tap_rider(3));
        assert_eq!(crate::sfx::take_voice(), Some(Say::Number(2)));
        assert_eq!(g.phase, Phase::Choose);
        assert!(!g.tap_rider(2));
        assert_eq!(g.removed(), 2);
        assert_eq!(g.aboard(), 3);
    }

    #[test]
    fn answering_before_the_riders_are_off_is_ignored() {
        let mut g = fresh();
        let v = View::ipad_landscape();
        ready(&mut g, &v);
        assert_eq!(g.pick(0), Tap::Ignored);
    }

    #[test]
    fn the_sum_is_spoken_in_order_and_can_be_replayed_by_tapping_it() {
        let mut g = fresh();
        let v = View::ipad_landscape();
        g.start_level(3);
        g.question = Question::from_pair(3, 14, 6);
        g.load();
        ready(&mut g, &v);
        let mut said = vec![];
        for _ in 0..(6.0 / DT) as usize {
            g.update(&Input::default(), &v, DT);
            if let Some(s) = crate::sfx::take_voice() {
                said.push(s);
            }
        }
        assert_eq!(said[0], sfx::number(14));
        assert_eq!(said[1], Say::Line(Line::TakeAway));
        assert_eq!(said[2], sfx::number(6));
        assert_eq!(said[3], Say::Line(Line::Left));
        g.update(&Input::press_at(prompt_rect(&v).center()), &v, DT);
        run(&mut g, &v, 0.2);
        assert_eq!(crate::sfx::take_voice(), Some(sfx::number(14)));
        let missing = {
            g.start_level(4);
            g.question = Question::from_pair(4, 12, 5);
            g.load();
            ready(&mut g, &v);
            g.speak_question();
            g.speech.iter().map(|s| s.1).collect::<Vec<_>>()
        };
        assert_eq!(
            &missing[..3],
            &[sfx::number(12), Say::Line(Line::Missing), sfx::number(7)]
        );
    }

    #[test]
    fn every_spoken_number_has_a_clip_name() {
        for n in 1..=99 {
            let key = sfx::number(n).key();
            assert!(key.starts_with("say_"), "{key}");
        }
        assert_eq!(sfx::number(10).key(), "say_10");
        assert_eq!(sfx::number(11).key(), "say_n11");
        assert_eq!(sfx::number(99).key(), "say_n99");
    }

    #[test]
    fn riders_hide_after_a_second_on_the_remember_level_and_reappear_on_the_answer() {
        let mut g = fresh();
        let v = View::ipad_landscape();
        g.start_level(2);
        ready(&mut g, &v);
        assert_eq!(g.phase, Phase::Choose);
        assert_eq!(g.hide_k(), 0.0);
        run(&mut g, &v, HIDE_AFTER + 0.6);
        assert_eq!(g.hide_k(), 1.0);
        let slot = g.question.slot_of_answer();
        g.pick(slot);
        assert_eq!(g.phase, Phase::Resolve);
        run(&mut g, &v, 0.4);
        assert_eq!(g.hide_k(), 0.0);
    }

    #[test]
    fn number_only_levels_never_show_the_riders() {
        let mut g = fresh();
        let v = View::ipad_landscape();
        g.start_level(3);
        ready(&mut g, &v);
        assert_eq!(g.hide_k(), 1.0);
        assert_eq!(g.question.hops(), 0);
    }

    #[test]
    fn the_answer_glows_only_on_early_levels_after_a_pause_or_two_misses() {
        let mut g = fresh();
        let v = View::ipad_landscape();
        g.start_level(1);
        g.question = Question::from_pair(1, 6, 2);
        g.load();
        ready(&mut g, &v);
        for i in (0..6).rev().take(2) {
            g.tap_rider(i);
        }
        assert_eq!(g.glow_slot(), None);
        run(&mut g, &v, 9.0);
        assert_eq!(g.glow_slot(), Some(g.question.slot_of_answer()));
        g.start_level(5);
        ready(&mut g, &v);
        run(&mut g, &v, 12.0);
        assert_eq!(g.glow_slot(), None);
        let wrong: Vec<usize> = (0..OPTIONS)
            .filter(|s| *s != g.question.slot_of_answer())
            .collect();
        g.pick(wrong[0]);
        g.pick(wrong[1]);
        assert!(g.glow_slot().is_some());
    }

    #[test]
    fn a_wrong_answer_never_ends_the_question_and_costs_no_level() {
        let mut g = fresh();
        let v = View::ipad_landscape();
        ready(&mut g, &v);
        for i in (0..g.riders.len()).rev().take(g.question.hops()) {
            g.tap_rider(i);
        }
        let wrong = (0..OPTIONS)
            .find(|s| *s != g.question.slot_of_answer())
            .unwrap();
        assert_eq!(g.pick(wrong), Tap::Wrong);
        assert_eq!(g.phase, Phase::Choose);
        assert_eq!(g.ladder.level, 0);
    }

    #[test]
    fn a_strong_player_clears_every_level_in_the_fewest_questions() {
        let mut g = fresh();
        let v = View::ipad_landscape();
        let mut questions = 0;
        while g
            .report
            .as_ref()
            .is_none_or(|r| !matches!(r, Report::Cleared { .. }))
        {
            let level = g.ladder.level;
            let tap = {
                ready(&mut g, &v);
                if g.phase == Phase::Remove {
                    for i in (0..g.riders.len()).rev().take(g.question.hops()) {
                        g.tap_rider(i);
                    }
                }
                let slot = g.question.slot_of_answer();
                let t = g.pick(slot);
                g.react(&v, slot, t);
                t
            };
            questions += 1;
            assert!(matches!(tap, Tap::Right(_)), "level {level}");
            if matches!(tap, Tap::Right(Step::Cleared)) {
                break;
            }
            run(&mut g, &v, 6.0);
            if matches!(g.mode, Mode::Report(_)) {
                g.report = None;
                g.mode = Mode::Play;
            }
            assert!(questions < 500);
        }
        assert_eq!(questions, LEVELS * UP_STREAK as usize);
        assert_eq!(profile::get(GAME, "cleared"), 1);
        assert!(profile::stars(GAME, 0) >= 1);
        assert_eq!(profile::unlocked(GAME) as usize, LEVELS - 1);
    }

    #[test]
    fn a_weak_player_who_slips_often_still_moves_up_but_slowly() {
        let mut g = fresh();
        let v = View::ipad_landscape();
        rand::srand(3);
        let mut highest = 0;
        let mut after_strong_time = 0;
        for n in 0..260 {
            let clean = n % 5 != 0;
            solve(&mut g, &v, clean);
            highest = highest.max(g.ladder.level);
            if n == LEVELS * UP_STREAK as usize {
                after_strong_time = g.ladder.level;
            }
        }
        assert!(highest >= 2, "stuck at {highest}");
        assert!(after_strong_time < LEVELS - 1);
    }

    #[test]
    fn a_guessing_player_is_never_pushed_past_what_they_know() {
        let mut g = fresh();
        let v = View::ipad_landscape();
        g.start_level(0);
        profile::unlock(GAME, 4);
        g.start_level(4);
        for _ in 0..30 {
            solve(&mut g, &v, false);
        }
        assert!(g.ladder.level <= 2, "{}", g.ladder.level);
    }

    #[test]
    fn every_answer_feeds_the_mastery_record() {
        let mut g = fresh();
        let v = View::ipad_landscape();
        for _ in 0..3 {
            solve(&mut g, &v, true);
        }
        assert_eq!(crate::mastery::band(GAME, 0).answers, 3);
        assert!(crate::mastery::bands_of(GAME) == Some(LEVELS));
    }

    #[test]
    fn a_returning_child_resumes_where_they_were_and_skips_mastered_levels() {
        let _ = fresh();
        profile::unlock(GAME, 3);
        profile::set_level(GAME, 1);
        for _ in 0..12 {
            crate::mastery::record(
                GAME,
                1,
                crate::mastery::Answer {
                    first_try: true,
                    secs: 2.0,
                    helped: false,
                },
                target_secs(1),
            );
        }
        let g = Trains::new();
        assert!(g.ladder.level >= 2);
        let other = profile::create("Leo", 3).unwrap();
        profile::select(&other.id);
        assert_eq!(Trains::new().ladder.level, 0);
    }

    #[test]
    fn a_saved_level_past_what_is_unlocked_is_clamped() {
        let _ = fresh();
        profile::set_level(GAME, 6);
        assert_eq!(Trains::new().ladder.level, 0);
    }

    #[test]
    fn level_picker_only_opens_unlocked_levels() {
        let mut g = fresh();
        let v = View::ipad_landscape();
        profile::unlock(GAME, 2);
        g.mode = Mode::Levels;
        let tiles = level_tiles(&v);
        g.update(&Input::press_at(tiles[5].center()), &v, DT);
        assert_eq!(g.mode, Mode::Levels);
        g.update(&Input::press_at(tiles[2].center()), &v, DT);
        assert_eq!(g.mode, Mode::Play);
        assert_eq!(g.ladder.level, 2);
        assert_eq!(profile::level(GAME), 2);
    }

    #[test]
    fn tiles_and_buttons_are_big_touch_targets_in_both_orientations() {
        for v in views() {
            for r in button_rects(&v) {
                assert!(r.w >= 120.0 && r.h >= 90.0, "{r:?}");
                assert!(r.x >= 0.0 && r.x + r.w <= v.w && r.y + r.h <= v.h);
            }
            for r in level_tiles(&v) {
                assert!(r.w >= 100.0 && r.h >= 90.0);
                assert!(r.x >= 0.0 && r.x + r.w <= v.w);
            }
            let p = prompt_rect(&v);
            assert!(p.x >= 0.0 && p.x + p.w <= v.w);
        }
    }

    #[test]
    fn clearing_a_level_reports_stars_and_the_last_one_files_a_score() {
        let mut g = fresh();
        let v = View::ipad_landscape();
        for _ in 0..UP_STREAK {
            solve(&mut g, &v, true);
        }
        assert_eq!(g.ladder.level, 1);
        assert!(profile::stars(GAME, 0) >= 1);
        assert_eq!(profile::unlocked(GAME), 1);
    }

    #[test]
    fn random_taps_for_a_minute_never_break_the_game() {
        let mut g = fresh();
        for v in views() {
            for f in 0..3600 {
                let pos = vec2(rand::gen_range(0.0, v.w), rand::gen_range(0.0, v.h));
                let mut input = Input::press_at(pos);
                input.pressed = f % 7 == 0;
                g.update(&input, &v, DT);
            }
        }
    }
}
