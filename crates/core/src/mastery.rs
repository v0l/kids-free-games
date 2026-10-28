use crate::profile;

pub const EMA: f32 = 0.2;
pub const CONFIDENT_AFTER: i64 = 8;
pub const MASTERED: f32 = 0.85;
pub const MAX_SAMPLE_SECS: f32 = 30.0;
pub const TIERS: [f32; 3] = [0.5, 0.7, MASTERED];
pub const LEARN_GAMES: [(&str, usize); 4] = [
    ("mathpop", 9),
    ("count", 6),
    ("spell", 9),
    ("trains", 8),
];

pub fn bands_of(game: &str) -> Option<usize> {
    LEARN_GAMES.iter().find(|g| g.0 == game).map(|g| g.1)
}

pub fn overall_progress() -> f32 {
    let total: usize = LEARN_GAMES.iter().map(|g| g.1).sum();
    let earned: f32 = LEARN_GAMES
        .iter()
        .map(|(g, n)| game_progress(g, *n) * *n as f32)
        .sum();
    earned / total as f32
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Answer {
    pub first_try: bool,
    pub secs: f32,
    pub helped: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Band {
    pub answers: i64,
    pub accuracy: f32,
    pub secs: f32,
}

impl Band {
    pub fn speed(&self, target_secs: f32) -> f32 {
        if self.secs <= 0.0 {
            return 0.0;
        }
        (1.0 - (self.secs - target_secs) / (target_secs * 2.0)).clamp(0.0, 1.0)
    }

    pub fn mastery(&self, target_secs: f32) -> f32 {
        let confidence = (self.answers as f32 / CONFIDENT_AFTER as f32).min(1.0);
        confidence * self.accuracy * (0.6 + 0.4 * self.speed(target_secs))
    }
}

fn key(band: usize, field: &str) -> String {
    format!("m{band}.{field}")
}

pub fn band(game: &str, band: usize) -> Band {
    Band {
        answers: profile::get(game, &key(band, "n")),
        accuracy: profile::get(game, &key(band, "acc")) as f32 / 1000.0,
        secs: profile::get(game, &key(band, "ms")) as f32 / 1000.0,
    }
}

fn blend(old: f32, new: f32, first: bool) -> f32 {
    if first {
        new
    } else {
        old + (new - old) * EMA
    }
}

pub fn record(game: &str, band_index: usize, answer: Answer, target_secs: f32) -> Band {
    let mut b = band(game, band_index);
    let first = b.answers == 0;
    let right = if answer.first_try && !answer.helped {
        1.0
    } else {
        0.0
    };
    b.accuracy = blend(b.accuracy, right, first);
    if answer.first_try && answer.secs > 0.0 && answer.secs <= MAX_SAMPLE_SECS {
        b.secs = blend(b.secs, answer.secs, b.secs <= 0.0);
    }
    b.answers += 1;
    profile::set(game, &key(band_index, "n"), b.answers);
    profile::set(
        game,
        &key(band_index, "acc"),
        (b.accuracy * 1000.0).round() as i64,
    );
    profile::set(
        game,
        &key(band_index, "ms"),
        (b.secs * 1000.0).round() as i64,
    );
    let tier = TIERS
        .iter()
        .filter(|t| b.mastery(target_secs) >= **t)
        .count() as i64;
    let tier_key = key(band_index, "tier");
    if tier > profile::get(game, &tier_key) {
        let gained = tier - profile::get(game, &tier_key);
        profile::set(game, &tier_key, tier);
        crate::save::player_set("learn.points", points() + gained);
    }
    b
}

pub fn tier(game: &str, band: usize) -> i64 {
    profile::get(game, &key(band, "tier"))
}

pub fn mastered(game: &str, band: usize) -> bool {
    tier(game, band) >= TIERS.len() as i64
}

pub fn points() -> i64 {
    crate::save::player_get("learn.points")
}

pub fn game_progress(game: &str, bands: usize) -> f32 {
    if bands == 0 {
        return 0.0;
    }
    let earned: i64 = (0..bands).map(|b| tier(game, b)).sum();
    earned as f32 / (bands * TIERS.len()) as f32
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Session {
    pub answers: u32,
    pub first_try: u32,
    pub helped: u32,
    times: Vec<f32>,
    clock: f32,
    running: bool,
}

impl Session {
    pub fn start_question(&mut self) {
        self.clock = 0.0;
        self.running = true;
    }

    pub fn tick(&mut self, dt: f32) {
        if self.running {
            self.clock += dt;
        }
    }

    pub fn elapsed(&self) -> f32 {
        self.clock
    }

    pub fn answer(&mut self, first_try: bool, helped: bool) -> Answer {
        self.running = false;
        self.answers += 1;
        let clean = first_try && !helped;
        if clean {
            self.first_try += 1;
        }
        if helped {
            self.helped += 1;
        }
        if first_try && self.clock <= MAX_SAMPLE_SECS {
            self.times.push(self.clock);
        }
        Answer {
            first_try,
            secs: self.clock,
            helped,
        }
    }

    pub fn accuracy(&self) -> f32 {
        if self.answers == 0 {
            return 0.0;
        }
        self.first_try as f32 / self.answers as f32
    }

    pub fn median_secs(&self) -> Option<f32> {
        if self.times.is_empty() {
            return None;
        }
        let mut t = self.times.clone();
        t.sort_by(|a, b| a.partial_cmp(b).unwrap());
        Some(t[t.len() / 2])
    }

    pub fn fast(&self, target_secs: f32) -> bool {
        self.median_secs().is_some_and(|m| m <= target_secs)
    }

    pub fn stars(&self, target_secs: f32) -> u8 {
        let acc = self.accuracy();
        if acc >= 0.9 && self.fast(target_secs) {
            3
        } else if acc >= 0.7 {
            2
        } else {
            1
        }
    }

    pub fn summary(&self) -> String {
        let right = format!("{} of {} right first time", self.first_try, self.answers);
        match self.median_secs() {
            Some(s) => format!("{right}   {:.1} s each", s),
            None => right,
        }
    }

    pub fn reset(&mut self) {
        *self = Self::default();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fresh() {
        crate::save::reset_for_test();
        crate::profile::create("Mia", 0);
    }

    fn answer(secs: f32, first_try: bool) -> Answer {
        Answer {
            first_try,
            secs,
            helped: false,
        }
    }

    #[test]
    fn a_child_who_is_right_and_quick_masters_a_band() {
        fresh();
        for _ in 0..12 {
            record("t", 0, answer(2.0, true), 3.0);
        }
        assert!(band("t", 0).mastery(3.0) >= MASTERED);
        assert!(mastered("t", 0));
    }

    #[test]
    fn right_but_slow_is_progress_but_not_mastery() {
        fresh();
        for _ in 0..12 {
            record("t", 0, answer(12.0, true), 3.0);
        }
        let m = band("t", 0).mastery(3.0);
        assert!((0.5..MASTERED).contains(&m), "{m}");
        assert!(!mastered("t", 0));
        assert!(tier("t", 0) >= 1);
    }

    #[test]
    fn guessing_quickly_does_not_master_anything() {
        fresh();
        for i in 0..20 {
            record("t", 0, answer(0.8, i % 3 == 0), 3.0);
        }
        assert!(band("t", 0).mastery(3.0) < 0.5);
        assert_eq!(tier("t", 0), 0);
    }

    #[test]
    fn a_few_lucky_answers_are_not_enough() {
        fresh();
        for _ in 0..2 {
            record("t", 0, answer(1.0, true), 3.0);
        }
        assert!(band("t", 0).mastery(3.0) < 0.5);
    }

    #[test]
    fn hinted_answers_do_not_count_as_right() {
        fresh();
        for _ in 0..12 {
            record(
                "t",
                0,
                Answer {
                    first_try: true,
                    secs: 1.0,
                    helped: true,
                },
                3.0,
            );
        }
        assert!(band("t", 0).accuracy < 0.01);
    }

    #[test]
    fn learning_points_only_go_up() {
        fresh();
        for _ in 0..12 {
            record("t", 0, answer(2.0, true), 3.0);
        }
        let earned = points();
        assert_eq!(earned, TIERS.len() as i64);
        for _ in 0..12 {
            record("t", 0, answer(2.0, false), 3.0);
        }
        assert_eq!(points(), earned);
        assert!(mastered("t", 0));
    }

    #[test]
    fn points_and_bands_are_per_player() {
        fresh();
        for _ in 0..12 {
            record("t", 0, answer(2.0, true), 3.0);
        }
        crate::profile::create("Leo", 1);
        assert_eq!(points(), 0);
        assert_eq!(band("t", 0).answers, 0);
    }

    #[test]
    fn a_long_pause_does_not_poison_the_speed() {
        fresh();
        for _ in 0..8 {
            record("t", 0, answer(2.0, true), 3.0);
        }
        record("t", 0, answer(300.0, true), 3.0);
        assert!(band("t", 0).secs < 2.1);
    }

    #[test]
    fn session_stars_need_accuracy_and_speed_for_three() {
        let mut s = Session::default();
        for i in 0..10 {
            s.start_question();
            s.tick(2.0);
            s.answer(i != 0, false);
        }
        assert_eq!(s.stars(3.0), 3);
        assert_eq!(s.stars(1.5), 2);
        let mut slow = Session::default();
        for _ in 0..10 {
            slow.start_question();
            slow.tick(9.0);
            slow.answer(true, false);
        }
        assert_eq!(slow.stars(3.0), 2);
        let mut shaky = Session::default();
        for i in 0..10 {
            shaky.start_question();
            shaky.tick(1.0);
            shaky.answer(i % 2 == 0, false);
        }
        assert_eq!(shaky.stars(3.0), 1);
    }

    #[test]
    fn session_summary_reads_naturally() {
        let mut s = Session::default();
        for i in 0..4 {
            s.start_question();
            s.tick(2.0 + i as f32);
            s.answer(i < 3, false);
        }
        assert_eq!(s.summary(), "3 of 4 right first time   3.0 s each");
        assert!((s.accuracy() - 0.75).abs() < 1e-6);
    }

    #[test]
    fn progress_counts_tiers_over_all_bands() {
        fresh();
        for _ in 0..12 {
            record("t", 1, answer(2.0, true), 3.0);
        }
        assert!((game_progress("t", 4) - 0.25).abs() < 1e-6);
    }
}
