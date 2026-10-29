#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Better {
    Higher,
    Lower,
}

impl Better {
    pub fn as_str(self) -> &'static str {
        match self {
            Better::Higher => "higher",
            Better::Lower => "lower",
        }
    }

    pub fn beats(self, a: i64, b: i64) -> bool {
        match self {
            Better::Higher => a > b,
            Better::Lower => a < b,
        }
    }
}

pub const GAMES: &[(&str, Better)] = &[
    ("blocks", Better::Higher),
    ("ballsort", Better::Higher),
    ("breakout", Better::Higher),
    ("clash", Better::Higher),
    ("count", Better::Higher),
    ("crossy", Better::Higher),
    ("flappy", Better::Higher),
    ("fruit", Better::Higher),
    ("helix", Better::Higher),
    ("hotel", Better::Higher),
    ("jumper", Better::Higher),
    ("mathpop", Better::Higher),
    ("memory", Better::Lower),
    ("merge", Better::Higher),
    ("paper", Better::Higher),
    ("runner", Better::Higher),
    ("snake", Better::Higher),
    ("spell", Better::Higher),
    ("stack", Better::Higher),
    ("trains", Better::Higher),
];

pub const PROFILE_SAVE: &str = "profile";

pub fn direction(game: &str) -> Option<Better> {
    GAMES.iter().find(|(g, _)| *g == game).map(|(_, b)| *b)
}

pub fn is_game(game: &str) -> bool {
    direction(game).is_some()
}

pub fn is_save_slot(slot: &str) -> bool {
    slot == PROFILE_SAVE || is_game(slot)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn client_game_keys() -> Vec<String> {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../crates/core/src/games");
        let mut keys = vec![];
        let mut files = vec![];
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                files.push(path.join("mod.rs"));
            } else {
                files.push(path);
            }
        }
        for path in files {
            if path.extension().and_then(|e| e.to_str()) != Some("rs") || !path.exists() {
                continue;
            }
            let text = std::fs::read_to_string(&path).unwrap();
            for line in text.lines() {
                let line = line.trim();
                for prefix in ["pub const GAME: &str = \"", "pub const KEY: &str = \""] {
                    if let Some(rest) = line.strip_prefix(prefix) {
                        keys.push(rest.trim_end_matches("\";").to_string());
                    }
                }
            }
        }
        keys
    }

    #[test]
    fn every_game_in_the_app_is_known_to_the_server() {
        let keys = client_game_keys();
        assert!(keys.len() >= 16, "found {keys:?}");
        for key in keys {
            assert!(is_game(&key), "{key} is missing from GAMES");
        }
    }

    #[test]
    fn games_are_unique_and_url_safe() {
        for (i, (g, _)) in GAMES.iter().enumerate() {
            assert!(g
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit()));
            assert!(GAMES[i + 1..].iter().all(|(o, _)| o != g));
        }
    }

    #[test]
    fn memory_is_the_only_lower_is_better_game() {
        let lower: Vec<_> = GAMES.iter().filter(|(_, b)| *b == Better::Lower).collect();
        assert_eq!(lower.len(), 1);
        assert_eq!(direction("memory"), Some(Better::Lower));
        assert!(Better::Lower.beats(3, 5));
        assert!(Better::Higher.beats(5, 3));
        assert!(!Better::Higher.beats(5, 5));
    }

    #[test]
    fn save_slots_include_profile_but_scores_do_not() {
        assert!(is_save_slot("profile"));
        assert!(is_save_slot("flappy"));
        assert!(!is_save_slot("nope"));
        assert!(!is_game("profile"));
    }
}
