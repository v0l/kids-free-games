pub const PLAYER_SPEED: f32 = 6.5;
pub const FULL_SKILL_AT: i64 = 250;
pub const CALM_SECS: f32 = 12.0;

pub const NAMES: [&str; 40] = [
    "Zippy", "Bloop", "Mango", "Pip", "Noodle", "Biscuit", "Pickle", "Waffle", "Bubbles", "Sprout",
    "Gizmo", "Peanut", "Jelly", "Tofu", "Muffin", "Pebble", "Doodle", "Sunny", "Cocoa", "Ziggy",
    "Button", "Nibbles", "Taco", "Popcorn", "Fizz", "Bean", "Maple", "Olive", "Rocket", "Squish",
    "Wiggles", "Pogo", "Kiwi", "Pudding", "Marble", "Comet", "Dumpling", "Tumble", "Banjo", "Coco",
];

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Persona {
    pub speed: f32,
    pub greed: f32,
    pub aggro: f32,
    pub caution: f32,
}

pub const PERSONAS: [Persona; 5] = [
    Persona {
        speed: 0.94,
        greed: 0.45,
        aggro: 0.1,
        caution: 1.0,
    },
    Persona {
        speed: 1.0,
        greed: 1.0,
        aggro: 0.25,
        caution: 0.75,
    },
    Persona {
        speed: 1.0,
        greed: 0.55,
        aggro: 1.0,
        caution: 0.6,
    },
    Persona {
        speed: 0.97,
        greed: 1.5,
        aggro: 0.3,
        caution: 0.45,
    },
    Persona {
        speed: 0.9,
        greed: 0.7,
        aggro: 0.05,
        caution: 0.3,
    },
];

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tuning {
    pub speed: f32,
    pub reach: usize,
    pub effort: f32,
}

pub fn skill(best_tenths: i64) -> f32 {
    (best_tenths.max(0) as f32 / FULL_SKILL_AT as f32)
        .clamp(0.0, 1.0)
        .sqrt()
}

pub fn bots_for(skill: f32) -> usize {
    8 + (skill.clamp(0.0, 1.0) * 3.0).round() as usize
}

pub fn tuning(p: Persona, skill: f32) -> Tuning {
    let s = skill.clamp(0.0, 1.0);
    Tuning {
        speed: (PLAYER_SPEED * (0.64 + 0.32 * s) * p.speed).min(PLAYER_SPEED * 0.97),
        reach: (8.0 + p.greed * (8.0 + 26.0 * s)) as usize,
        effort: 0.1 + 0.9 * s,
    }
}

pub fn rl_effort(skill: f32) -> f32 {
    skill.clamp(0.0, 1.0).sqrt()
}

pub fn hunts_player(skill: f32, player_age: f32) -> bool {
    skill >= 0.2 && player_age >= CALM_SECS
}

pub fn score(share: f32) -> i64 {
    (share * 1000.0).floor().max(0.0) as i64
}

pub fn score_label(s: i64) -> String {
    format!("{}.{}%", s / 10, s % 10)
}

pub fn clock_label(secs: f32) -> String {
    let s = secs as u32;
    format!("{}:{:02}", s / 60, s % 60)
}

pub const MILESTONES: [i64; 11] = [10, 20, 50, 100, 150, 200, 300, 400, 500, 750, 1000];

pub fn next_milestone(tenths: i64) -> Option<i64> {
    MILESTONES.iter().copied().find(|m| *m > tenths)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bots_start_slow_and_ramp_with_your_best_but_never_outrun_you() {
        let steps: Vec<f32> = (0..=12).map(|k| skill(k * 25)).collect();
        for w in steps.windows(2) {
            assert!(w[1] >= w[0]);
        }
        assert_eq!(skill(0), 0.0);
        assert_eq!(skill(10_000), 1.0);
        for p in PERSONAS {
            let easy = tuning(p, 0.0);
            let hard = tuning(p, 1.0);
            assert!(easy.speed < hard.speed);
            assert!(easy.speed <= PLAYER_SPEED * 0.7);
            assert!(hard.speed < PLAYER_SPEED);
            assert!(easy.reach < hard.reach);
            assert!(easy.effort < hard.effort);
        }
    }

    #[test]
    fn the_map_is_at_least_a_hundred_units_square() {
        const { assert!(super::super::world::SIZE >= 100.0) };
    }

    #[test]
    fn more_bots_join_as_you_get_better() {
        assert_eq!(bots_for(0.0), 8);
        assert_eq!(bots_for(1.0), 11);
        assert!(bots_for(0.5) >= 9);
    }

    #[test]
    fn nobody_hunts_a_beginner_or_a_fresh_spawn() {
        assert!(!hunts_player(0.0, 100.0));
        assert!(!hunts_player(1.0, 3.0));
        assert!(hunts_player(0.6, 30.0));
    }

    #[test]
    fn personas_differ() {
        for (i, a) in PERSONAS.iter().enumerate() {
            for b in &PERSONAS[i + 1..] {
                assert_ne!(a, b);
            }
        }
        let hunter = PERSONAS.iter().map(|p| p.aggro).fold(0.0, f32::max);
        let timid = PERSONAS.iter().map(|p| p.aggro).fold(1.0, f32::min);
        assert!(hunter > timid * 5.0);
    }

    #[test]
    fn names_are_unique() {
        let mut n = NAMES.to_vec();
        n.sort();
        n.dedup();
        assert_eq!(n.len(), NAMES.len());
    }

    #[test]
    fn scores_read_as_percent() {
        assert_eq!(score(0.1234), 123);
        assert_eq!(score_label(123), "12.3%");
        assert_eq!(score_label(5), "0.5%");
        assert_eq!(clock_label(65.4), "1:05");
    }

    #[test]
    fn milestones_lead_on() {
        assert_eq!(next_milestone(0), Some(10));
        assert_eq!(next_milestone(10), Some(20));
        assert_eq!(next_milestone(1000), None);
    }
}
