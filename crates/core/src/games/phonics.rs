use crate::engine::*;
use crate::feel::{self, Impact};
use crate::fx::*;
use crate::geom::*;
use crate::learn::PRAISE;
use crate::profile;
use crate::rng as rand;
use crate::sfx::{self, Sfx};
use glam::{vec2, Vec2};

pub use super::mathpop::{
    balloon_radius, balloon_rest, question_rect, question_text_size, Tap, BALLOON_COLORS,
};

#[derive(Clone, Debug)]
pub struct Balloon {
    pub value: &'static str,
    pub color: usize,
    pub wobble: f32,
    pub tried: bool,
    pub popped: bool,
}

pub const KEY: &str = "phonics";
pub const LEVEL_NAMES: [&str; 8] = [
    "First sounds",
    "More sounds",
    "Next sounds",
    "Tricky sounds",
    "Whole alphabet",
    "Sound pairs",
    "Blends",
    "Sound whizz",
];
pub const QUESTIONS: u32 = 6;
pub const TARGET_SECS: [f32; 8] = [4.0, 4.0, 4.5, 5.0, 5.0, 5.0, 5.0, 5.5];
pub const DOWN_SLIPS: u32 = 3;
pub const BALLOONS: usize = 4;
pub const REPEAT_SECS: f32 = 6.0;
const LEAVE_TIME: f32 = 1.0;

pub const FIRST: [&str; 6] = ["s", "a", "t", "p", "i", "n"];
pub const SECOND: [&str; 6] = ["m", "d", "g", "o", "c", "k"];
pub const THIRD: [&str; 7] = ["e", "u", "r", "h", "b", "f", "l"];
pub const FOURTH: [&str; 7] = ["j", "v", "w", "x", "y", "z", "q"];
pub const PAIRS: [&str; 3] = ["sh", "ch", "th"];
pub const BLENDS: [&str; 6] = ["st", "bl", "tr", "cl", "fl", "gr"];

pub const CLIPS: [&str; 34] = [
    "ph_s", "ph_a", "ph_t", "ph_p", "ph_i", "ph_n", "ph_m", "ph_d", "ph_g", "ph_o", "ph_c", "ph_e",
    "ph_u", "ph_r", "ph_h", "ph_b", "ph_f", "ph_l", "ph_j", "ph_v", "ph_w", "ph_x", "ph_y", "ph_z",
    "ph_q", "ph_sh", "ph_ch", "ph_th", "ph_st", "ph_bl", "ph_tr", "ph_cl", "ph_fl", "ph_gr",
];

pub fn clip_of(letters: &str) -> &'static str {
    let letters = if letters == "k" { "c" } else { letters };
    CLIPS
        .iter()
        .copied()
        .find(|c| &c[3..] == letters)
        .unwrap_or("ph_s")
}

pub fn same_sound(a: &str, b: &str) -> bool {
    clip_of(a) == clip_of(b)
}

pub fn level_sounds(level: usize) -> Vec<&'static str> {
    let upto = |n: usize| -> Vec<&'static str> {
        let groups: [&[&str]; 4] = [&FIRST, &SECOND, &THIRD, &FOURTH];
        groups[..n].iter().flat_map(|g| g.iter().copied()).collect()
    };
    match level {
        0 => FIRST.to_vec(),
        1 => SECOND.to_vec(),
        2 => THIRD.to_vec(),
        3 => FOURTH.to_vec(),
        4 => upto(4),
        5 => PAIRS.to_vec(),
        6 => BLENDS.to_vec(),
        _ => {
            let mut all = upto(4);
            all.extend(PAIRS);
            all.extend(BLENDS);
            all
        }
    }
}

pub fn level_pool(level: usize) -> Vec<&'static str> {
    let mut pool = level_sounds(level);
    let help: Vec<&'static str> = match level {
        1 => FIRST.to_vec(),
        2 => SECOND.to_vec(),
        3 => THIRD.to_vec(),
        5 => vec!["s", "c", "t", "h"],
        6 => vec!["s", "b", "t", "c", "f", "g"],
        _ => vec![],
    };
    pool.extend(help);
    pool
}

fn misses(sound: &str) -> i64 {
    profile::get(KEY, &format!("miss.{}", clip_of(sound)))
}

fn note(sound: &str, right_first: bool) {
    let key = format!("miss.{}", clip_of(sound));
    let now = profile::get(KEY, &key);
    let next = if right_first {
        (now - 1).max(0)
    } else {
        (now + 2).min(8)
    };
    profile::set(KEY, &key, next);
}

#[derive(Clone, Debug)]
pub struct Question {
    pub target: &'static str,
    pub options: Vec<&'static str>,
}

impl Question {
    pub fn from_target(level: usize, target: &'static str) -> Self {
        let mut pool: Vec<&'static str> = level_pool(level)
            .into_iter()
            .filter(|s| !same_sound(s, target))
            .collect();
        pool.sort_unstable();
        pool.dedup();
        let mut taken: Vec<&'static str> = vec![];
        while taken.len() < BALLOONS - 1 && !pool.is_empty() {
            let pick = pool.swap_remove(rand::gen_range(0, pool.len()));
            if !taken.iter().any(|t| same_sound(t, pick)) {
                taken.push(pick);
            }
        }
        taken.insert(rand::gen_range(0, taken.len() + 1), target);
        Self {
            target,
            options: taken,
        }
    }

    pub fn generate(level: usize, previous: Option<&str>) -> Self {
        let sounds = level_sounds(level);
        let mut best: Option<&'static str> = None;
        for _ in 0..3 {
            let cand = sounds[rand::gen_range(0, sounds.len())];
            if Some(cand) == previous || best.is_some_and(|b| misses(b) >= misses(cand)) {
                continue;
            }
            best = Some(cand);
        }
        let target = best.unwrap_or_else(|| {
            *sounds
                .iter()
                .find(|s| Some(**s) != previous)
                .unwrap_or(&sounds[0])
        });
        Self::from_target(level, target)
    }

    pub fn starter() -> Self {
        Self::from_target(0, "s")
    }

    pub fn slot_of_answer(&self) -> usize {
        self.options
            .iter()
            .position(|v| *v == self.target)
            .unwrap_or(0)
    }

    pub fn clip(&self) -> &'static str {
        clip_of(self.target)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Screen {
    Play,
    Levels,
    Done(f32),
    Final(f32),
}

pub struct Phonics {
    pub level: usize,
    pub answered: u32,
    pub slips: u32,
    pub slip_run: u32,
    pub wrong: u32,
    pub solved: bool,
    pub rank: Option<usize>,
    pub teach: bool,
    pub screen: Screen,
    pub finished: Option<u8>,
    pub question: Question,
    pub balloons: Vec<Balloon>,
    pub leave: Option<f32>,
    pub asked: f32,
    pub since_said: f32,
    pub streak_bump: Bump,
    pub question_bump: Bump,
    pub fx: Particles,
    pub popups: Popups,
    pub press: f32,
    pub t: f32,
    pub session: crate::mastery::Session,
}

impl Phonics {
    pub fn new() -> Self {
        let teach = profile::get(KEY, "played") == 0;
        let last = LEVEL_NAMES.len() - 1;
        let level = (profile::level(KEY) as usize)
            .min(profile::unlocked(KEY) as usize)
            .min(last);
        let mut g = Self {
            level,
            answered: 0,
            slips: 0,
            slip_run: 0,
            wrong: 0,
            solved: false,
            rank: None,
            teach,
            screen: Screen::Play,
            finished: None,
            question: Question::generate(level, None),
            balloons: vec![],
            leave: None,
            asked: 0.0,
            since_said: 0.0,
            streak_bump: Bump::default(),
            question_bump: Bump::default(),
            fx: Particles::default(),
            popups: Popups::default(),
            press: 0.0,
            t: 0.0,
            session: crate::mastery::Session::default(),
        };
        if teach && level == 0 {
            g.question = Question::starter();
        }
        g.load_balloons();
        g
    }

    pub fn is_final_level(&self) -> bool {
        self.level + 1 == LEVEL_NAMES.len()
    }

    pub fn start_level(&mut self, level: usize) {
        self.level = level.min(LEVEL_NAMES.len() - 1);
        profile::set_level(KEY, self.level as u32);
        self.answered = 0;
        self.slips = 0;
        self.slip_run = 0;
        self.session.reset();
        self.finished = None;
        self.rank = None;
        self.screen = Screen::Play;
        self.next_question();
    }

    pub fn say_sound(&mut self) {
        self.since_said = 0.0;
        sfx::say(sfx::Say::Word(self.question.clip()));
    }

    fn load_balloons(&mut self) {
        let offset = rand::gen_range(0, BALLOON_COLORS.len());
        self.balloons = self
            .question
            .options
            .iter()
            .enumerate()
            .map(|(i, v)| Balloon {
                value: *v,
                color: (offset + i * 5) % BALLOON_COLORS.len(),
                wobble: 0.0,
                tried: false,
                popped: false,
            })
            .collect();
        self.wrong = 0;
        self.solved = false;
        self.leave = None;
        self.asked = 0.0;
        self.session.start_question();
        self.question_bump.kick();
        sfx::prefetch_words(&level_clips(self.level));
        self.say_sound();
    }

    fn next_question(&mut self) {
        self.question = Question::generate(self.level, Some(self.question.target));
        self.load_balloons();
    }

    pub fn revealed(&self) -> bool {
        self.wrong >= 2
    }

    pub fn finished_stars(&self) -> u8 {
        profile::stars(KEY, self.level as u32).max(self.session.stars(TARGET_SECS[self.level]))
    }

    fn complete_level(&mut self) {
        let stars = self.session.stars(TARGET_SECS[self.level]);
        profile::award_stars(KEY, self.level as u32, stars);
        if !self.is_final_level() {
            profile::unlock(KEY, self.level as u32 + 1);
            profile::set_level(KEY, self.level as u32 + 1);
        } else {
            let total = profile::total_stars(KEY) as i64;
            if profile::personal_best(KEY).is_none_or(|b| total > b) {
                self.rank = profile::submit(KEY, total, profile::Better::Higher);
            }
        }
        self.finished = Some(stars);
    }

    fn advance(&mut self, view: &View) {
        if self.finished.take().is_some() {
            self.screen = if self.is_final_level() {
                Screen::Final(0.0)
            } else {
                Screen::Done(0.0)
            };
            return;
        }
        if self.slip_run >= DOWN_SLIPS && self.level > 0 {
            self.start_level(self.level - 1);
            self.popups.add(
                vec2(view.w / 2.0, view.h * 0.36),
                format!("Let's practise {}", LEVEL_NAMES[self.level]),
                WHITE,
                44.0,
            );
            return;
        }
        self.next_question();
    }

    pub fn pick(&mut self, slot: usize) -> Tap {
        let Some(b) = self.balloons.get(slot) else {
            return Tap::Ignored;
        };
        if b.popped || self.solved {
            return Tap::Ignored;
        }
        if b.value == self.question.target {
            self.solved = true;
            self.balloons[slot].popped = true;
            self.leave = Some(0.0);
            self.answered += 1;
            if !self.teach {
                let answer = self.session.answer(self.wrong == 0, false);
                crate::mastery::record(KEY, self.level, answer, TARGET_SECS[self.level]);
                note(self.question.target, self.wrong == 0);
            }
            if self.wrong > 0 {
                self.slips += 1;
                self.slip_run += 1;
            } else {
                self.slip_run = 0;
            }
            if self.teach {
                self.teach = false;
                profile::set(KEY, "played", 1);
            }
            if self.answered >= QUESTIONS {
                self.complete_level();
            }
            return Tap::Right;
        }
        if self.balloons[slot].tried {
            self.balloons[slot].wobble = 1.0;
            return Tap::Again;
        }
        self.balloons[slot].tried = true;
        self.balloons[slot].wobble = 1.0;
        self.wrong += 1;
        if self.revealed() {
            Tap::Reveal
        } else {
            Tap::Wrong
        }
    }

    pub fn center(&self, view: &View, slot: usize) -> Vec2 {
        let rest = balloon_rest(view, slot);
        let rise = ease_out_cubic((self.asked - slot as f32 * 0.1) / 0.9);
        let bob = (self.t * 1.6 + slot as f32 * 1.7).sin() * 8.0;
        let lift = self.leave.map_or(0.0, |l| l * l * (view.h + 400.0));
        let wob = (self.balloons[slot].wobble * 38.0).sin() * 14.0 * self.balloons[slot].wobble;
        vec2(
            rest.x + wob,
            rest.y + bob + (1.0 - rise) * (view.h - rest.y + 220.0) - lift,
        )
    }

    fn react(&mut self, view: &View, slot: usize, tap: Tap) {
        let c = self.center(view, slot);
        let color = BALLOON_COLORS[self.balloons[slot].color].1;
        let gold = Color::from_rgba(255, 220, 70, 255);
        match tap {
            Tap::Right => {
                let cleared = self.finished.is_some();
                sfx::play(Sfx::Pop);
                sfx::play(if cleared { Sfx::Win } else { Sfx::Right });
                if !cleared {
                    sfx::play_step(Sfx::Right, self.answered as usize * 2);
                }
                if cleared {
                    feel::impact_tinted(Impact::Large, Color::from_rgba(255, 230, 120, 255));
                }
                self.fx.emit(
                    c,
                    &[color, WHITE, gold],
                    Burst {
                        count: 26,
                        shape: Shape::Circle,
                        size: (5.0, 11.0),
                        ..Default::default()
                    },
                );
                self.fx.sparkle(c, gold);
                self.streak_bump.kick();
                sfx::say(if cleared && self.is_final_level() {
                    sfx::Say::Congrats
                } else if cleared {
                    sfx::Say::LevelUp
                } else if self.wrong == 0 {
                    sfx::Say::Great
                } else {
                    sfx::Say::Correct
                });
                let word = if self.wrong == 0 {
                    PRAISE[rand::gen_range(0, PRAISE.len())]
                } else {
                    "That's it!"
                };
                self.popups.add(c, word, gold, 60.0);
                if cleared {
                    let mid = vec2(view.w / 2.0, view.h * 0.36);
                    self.fx.confetti(view);
                    let title = if self.is_final_level() {
                        "You did it!"
                    } else {
                        "Level done!"
                    };
                    self.popups.add(mid, title, gold, 88.0);
                }
            }
            Tap::Wrong => {
                sfx::play(Sfx::Wrong);
                self.popups
                    .add(c, "Try again", Color::from_rgba(255, 170, 90, 255), 48.0);
                self.say_sound();
            }
            Tap::Reveal => {
                sfx::play(Sfx::Wrong);
                let text = format!("It's {}", self.question.target);
                self.popups
                    .add(c, text, Color::from_rgba(255, 170, 90, 255), 48.0);
                self.say_sound();
            }
            Tap::Again | Tap::Ignored => {}
        }
    }

    fn play_input(&mut self, input: &Input, view: &View) {
        if input.tapped_in(levels_button(view)) {
            sfx::play(Sfx::Click);
            self.screen = Screen::Levels;
            return;
        }
        if self.leave.is_some() || !input.pressed {
            return;
        }
        if input.tapped_in(question_rect(view)) {
            sfx::play(Sfx::Click);
            self.say_sound();
            return;
        }
        let r = balloon_radius(view) * 1.1;
        let hit = (0..self.balloons.len()).find(|i| {
            !self.balloons[*i].popped && (input.pos - self.center(view, *i)).length() <= r.max(50.0)
        });
        if let Some(slot) = hit {
            let tap = self.pick(slot);
            self.react(view, slot, tap);
        }
    }

    fn levels_input(&mut self, input: &Input, view: &View) {
        if !input.pressed {
            return;
        }
        let l = levels_layout(view);
        if input.tapped_in(l.back) {
            sfx::play(Sfx::Click);
            if self.answered >= QUESTIONS {
                self.start_level(self.level);
            }
            self.screen = Screen::Play;
            return;
        }
        let open = profile::unlocked(KEY) as usize;
        for (i, tile) in l.tiles.iter().enumerate() {
            if tile.contains(input.pos) && i <= open {
                sfx::play(Sfx::Click);
                self.start_level(i);
                return;
            }
        }
    }

    fn done_input(&mut self, input: &Input, view: &View, age: f32) {
        if !input.pressed || age < 0.45 {
            return;
        }
        let l = done_layout(view);
        if input.tapped_in(l.next) {
            sfx::play(Sfx::Click);
            self.start_level(self.level + 1);
        } else if input.tapped_in(l.again) {
            sfx::play(Sfx::Click);
            self.start_level(self.level);
        } else if input.tapped_in(l.levels) {
            sfx::play(Sfx::Click);
            self.screen = Screen::Levels;
        }
    }

    fn final_input(&mut self, input: &Input, age: f32) {
        if input.pressed && age > 0.8 {
            self.start_level(self.level);
        }
    }

    pub fn mood(&self) -> &'static str {
        if self.leave.is_some() {
            "up"
        } else if self.balloons.iter().any(|b| b.wobble > 0.3) {
            "sad"
        } else {
            "stand"
        }
    }
}

pub fn level_clips(level: usize) -> Vec<&'static str> {
    let mut clips: Vec<&'static str> = level_pool(level).into_iter().map(clip_of).collect();
    clips.sort_unstable();
    clips.dedup();
    clips
}

impl Default for Phonics {
    fn default() -> Self {
        Self::new()
    }
}

pub type LevelsLayout = crate::sizing::Picker;

pub const LEVELS_TITLE: &str = "Pick a level";

pub fn levels_layout(view: &View) -> LevelsLayout {
    crate::sizing::picker_layout(view, LEVELS_TITLE, LEVEL_NAMES.len(), 4, 0.95, "Back")
}

pub struct DoneLayout {
    pub panel: Rect,
    pub next: Rect,
    pub again: Rect,
    pub levels: Rect,
    pub done: crate::sizing::Done,
}

pub const DONE_TITLE: &str = "Level done!";
pub const DONE_LABELS: [&str; 3] = ["Next level", "Again", "Levels"];

pub fn done_layout(view: &View) -> DoneLayout {
    let done = crate::sizing::done_layout(view, DONE_TITLE, &DONE_LABELS);
    DoneLayout {
        panel: done.panel(),
        next: done.button(0),
        again: done.button(1),
        levels: done.button(2),
        done,
    }
}

pub const HUD_TITLE: &str = "Level 8: Sound whizz";

pub fn hud(view: &View) -> crate::learn::Hud {
    crate::learn::learn_hud(
        view,
        &crate::learn::HudSpec {
            title: HUD_TITLE,
            levels: crate::learn::LevelsAt::Top,
            stars: 0,
            progress: 18.0,
            goal: "",
        },
    )
}

pub fn levels_button(view: &View) -> Rect {
    hud(view).levels.unwrap_or_default()
}

impl Game for Phonics {
    fn update(&mut self, input: &Input, view: &View, dt: f32) {
        self.t += dt;
        self.streak_bump.update(dt);
        self.question_bump.update(dt);
        self.press = approach(
            self.press,
            if input.down && levels_button(view).contains(input.pos) {
                1.0
            } else {
                0.0
            },
            20.0,
            dt,
        );
        self.fx.update(dt);
        self.popups.update(dt);
        match self.screen {
            Screen::Play => {
                self.asked += dt;
                self.since_said += dt;
                self.session.tick(dt);
                for b in &mut self.balloons {
                    b.wobble = (b.wobble - dt * 1.6).max(0.0);
                }
                if let Some(l) = &mut self.leave {
                    *l += dt / LEAVE_TIME;
                    if *l >= 1.0 {
                        self.advance(view);
                    }
                } else {
                    if self.since_said >= REPEAT_SECS {
                        self.say_sound();
                    }
                    self.play_input(input, view);
                }
            }
            Screen::Levels => self.levels_input(input, view),
            Screen::Done(age) => {
                self.screen = Screen::Done(age + dt);
                self.done_input(input, view, age);
            }
            Screen::Final(age) => {
                self.screen = Screen::Final(age + dt);
                self.final_input(input, age);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fresh_player() {
        crate::save::reset_for_test();
        crate::profile::create("Mia", 0);
    }

    fn pop_right(g: &mut Phonics, v: &View) {
        let slot = g.question.slot_of_answer();
        let tap = g.pick(slot);
        g.react(v, slot, tap);
    }

    fn wrong_slot(g: &Phonics) -> usize {
        (0..BALLOONS)
            .find(|i| *i != g.question.slot_of_answer() && !g.balloons[*i].tried)
            .unwrap()
    }

    fn pop_wrong(g: &mut Phonics, v: &View) {
        let slot = wrong_slot(g);
        let tap = g.pick(slot);
        g.react(v, slot, tap);
    }

    fn settle(g: &mut Phonics, v: &View, secs: f32) {
        for _ in 0..(secs * 60.0) as usize {
            g.update(&Input::default(), v, 1.0 / 60.0);
        }
    }

    fn clear_level(g: &mut Phonics, v: &View) {
        for _ in 0..QUESTIONS {
            pop_right(g, v);
            settle(g, v, 1.2);
            if g.screen != Screen::Play {
                return;
            }
        }
    }

    #[test]
    fn every_clip_the_game_asks_for_is_in_the_word_pack_and_decodes() {
        let pack = crate::sfx::words();
        assert!(!pack.is_empty());
        for level in 0..LEVEL_NAMES.len() {
            for clip in level_clips(level) {
                let key = sfx::Say::Word(clip).key();
                let data = crate::sfx::find(pack, &format!("{key}.0"))
                    .or_else(|| crate::sfx::find(pack, &key))
                    .unwrap_or_else(|| panic!("{key} is not packed"));
                let (_, samples) = crate::sfx::decode(data).unwrap();
                assert!(samples.len() > 2000, "{key} is too short");
            }
        }
        for clip in CLIPS {
            let key = sfx::Say::Word(clip).key();
            assert!(
                crate::sfx::find(pack, &key).is_some()
                    || crate::sfx::find(pack, &format!("{key}.0")).is_some(),
                "{key}"
            );
        }
    }

    #[test]
    fn every_sound_has_a_clip_and_every_clip_is_a_sound() {
        for level in 0..LEVEL_NAMES.len() {
            for s in level_pool(level) {
                assert!(CLIPS.contains(&clip_of(s)), "{s}");
                assert_eq!(&clip_of(s)[3..], if s == "k" { "c" } else { s });
            }
        }
        assert_eq!(CLIPS.len(), 34);
    }

    #[test]
    fn level_steps_grow_from_six_sounds_to_digraphs_and_blends() {
        assert_eq!(level_sounds(0), ["s", "a", "t", "p", "i", "n"]);
        assert_eq!(level_sounds(4).len(), 26);
        assert_eq!(level_sounds(5), ["sh", "ch", "th"]);
        assert_eq!(level_sounds(6).len(), 6);
        assert_eq!(level_sounds(7).len(), 26 + 3 + 6);
        let mut all: Vec<&str> = (0..4).flat_map(level_sounds).collect();
        all.sort_unstable();
        all.dedup();
        assert_eq!(all.len(), 26);
    }

    #[test]
    fn every_question_has_one_right_balloon_and_distinct_sounds() {
        rand::srand(7);
        for level in 0..LEVEL_NAMES.len() {
            for _ in 0..300 {
                let q = Question::generate(level, None);
                assert_eq!(q.options.len(), BALLOONS, "{level} {q:?}");
                assert_eq!(q.options.iter().filter(|o| **o == q.target).count(), 1);
                for (i, a) in q.options.iter().enumerate() {
                    for b in &q.options[i + 1..] {
                        assert!(!same_sound(a, b), "{q:?}");
                    }
                }
                assert!(level_sounds(level).contains(&q.target));
            }
        }
    }

    #[test]
    fn no_question_repeats_the_last_target() {
        rand::srand(3);
        fresh_player();
        let mut prev = None;
        for _ in 0..300 {
            let q = Question::generate(0, prev);
            assert_ne!(Some(q.target), prev);
            prev = Some(q.target);
        }
    }

    #[test]
    fn first_minute_starts_on_s_with_a_hint_and_the_sound_spoken() {
        fresh_player();
        let g = Phonics::new();
        assert!(g.teach);
        assert_eq!(g.question.target, "s");
        assert_eq!(crate::sfx::take_voice(), Some(sfx::Say::Word("ph_s")));
    }

    #[test]
    fn the_sound_repeats_while_the_child_thinks_and_after_a_miss() {
        fresh_player();
        let v = View::ipad_landscape();
        let mut g = Phonics::new();
        crate::sfx::take_voice();
        settle(&mut g, &v, REPEAT_SECS + 0.5);
        assert_eq!(crate::sfx::take_voice(), Some(sfx::Say::Word("ph_s")));
        pop_wrong(&mut g, &v);
        assert_eq!(crate::sfx::take_voice(), Some(sfx::Say::Word("ph_s")));
    }

    #[test]
    fn tapping_the_question_says_the_sound_again() {
        fresh_player();
        let v = View::ipad_landscape();
        let mut g = Phonics::new();
        g.asked = 5.0;
        crate::sfx::take_voice();
        g.update(&Input::press_at(question_rect(&v).center()), &v, 1.0 / 60.0);
        assert_eq!(crate::sfx::take_voice(), Some(sfx::Say::Word("ph_s")));
    }

    #[test]
    fn a_wrong_tap_wobbles_and_a_second_one_shows_the_answer() {
        fresh_player();
        let mut g = Phonics::new();
        let first = wrong_slot(&g);
        assert_eq!(g.pick(first), Tap::Wrong);
        let second = wrong_slot(&g);
        assert_eq!(g.pick(second), Tap::Reveal);
        assert!(g.revealed());
        assert!(g.leave.is_none());
    }

    #[test]
    fn six_pops_clear_a_level_award_stars_and_unlock_the_next() {
        fresh_player();
        let v = View::ipad_landscape();
        let mut g = Phonics::new();
        clear_level(&mut g, &v);
        assert!(matches!(g.screen, Screen::Done(_)));
        assert_eq!(profile::stars(KEY, 0), 3);
        assert_eq!(profile::unlocked(KEY), 1);
        assert_eq!(profile::level(KEY), 1);
    }

    #[test]
    fn a_returning_child_resumes_where_they_left_off() {
        fresh_player();
        let v = View::ipad_landscape();
        let mut g = Phonics::new();
        clear_level(&mut g, &v);
        let g = Phonics::new();
        assert_eq!(g.level, 1);
        assert!(!g.teach);
        assert!(level_sounds(1).contains(&g.question.target));
    }

    #[test]
    fn progress_is_kept_per_player() {
        fresh_player();
        let v = View::ipad_landscape();
        let mut g = Phonics::new();
        clear_level(&mut g, &v);
        crate::profile::create("Leo", 3);
        assert_eq!(Phonics::new().level, 0);
        assert_eq!(profile::unlocked(KEY), 0);
    }

    #[test]
    fn three_shaky_answers_in_a_row_drop_back_one_level() {
        fresh_player();
        let v = View::ipad_landscape();
        profile::unlock(KEY, 3);
        profile::set(KEY, "played", 1);
        let mut g = Phonics::new();
        g.start_level(3);
        for _ in 0..DOWN_SLIPS {
            pop_wrong(&mut g, &v);
            pop_right(&mut g, &v);
            settle(&mut g, &v, 1.2);
        }
        assert_eq!(g.level, 2);
        assert_eq!(profile::unlocked(KEY), 3);
    }

    #[test]
    fn missed_sounds_come_back_more_often() {
        fresh_player();
        profile::set(KEY, "played", 1);
        note("a", false);
        note("a", false);
        rand::srand(9);
        let mut a = 0;
        let mut prev = None;
        for _ in 0..600 {
            let q = Question::generate(0, prev);
            if q.target == "a" {
                a += 1;
            }
            prev = Some(q.target);
        }
        assert!(a > 600 / 6 + 40, "{a}");
    }

    #[test]
    fn a_right_answer_cools_a_missed_sound() {
        fresh_player();
        note("sh", false);
        assert_eq!(misses("sh"), 2);
        note("sh", true);
        assert_eq!(misses("sh"), 1);
    }

    #[test]
    fn clearing_the_last_level_submits_stars_to_the_board() {
        fresh_player();
        let v = View::ipad_landscape();
        let last = LEVEL_NAMES.len() - 1;
        profile::unlock(KEY, last as u32);
        let mut g = Phonics::new();
        g.start_level(last);
        clear_level(&mut g, &v);
        assert!(matches!(g.screen, Screen::Final(_)));
        assert_eq!(g.rank, Some(0));
        assert_eq!(profile::board(KEY)[0].score, 3);
    }

    #[test]
    fn the_level_picker_only_opens_unlocked_levels() {
        fresh_player();
        let v = View::ipad_landscape();
        profile::unlock(KEY, 2);
        let mut g = Phonics::new();
        g.screen = Screen::Levels;
        let l = levels_layout(&v);
        g.update(&Input::press_at(l.tiles[5].center()), &v, 1.0 / 60.0);
        assert_eq!(g.screen, Screen::Levels);
        g.update(&Input::press_at(l.tiles[2].center()), &v, 1.0 / 60.0);
        assert_eq!((g.screen, g.level), (Screen::Play, 2));
    }
}
