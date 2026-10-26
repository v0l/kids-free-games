pub mod crossy;
pub mod flappy;
pub mod snake;
pub mod blocks;

use crate::geom::Color;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Arcade,
    Learn,
}

impl Kind {
    pub fn mood(self) -> crate::sfx::Mood {
        match self {
            Kind::Arcade => crate::sfx::Mood::Arcade,
            Kind::Learn => crate::sfx::Mood::Learn,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Info {
    pub key: &'static str,
    pub name: &'static str,
    pub kind: Kind,
    pub like: &'static str,
    pub color: Color,
}

const fn info(
    key: &'static str,
    name: &'static str,
    kind: Kind,
    like: &'static str,
    c: (u8, u8, u8),
) -> Info {
    Info {
        key,
        name,
        kind,
        like,
        color: Color::from_rgba(c.0, c.1, c.2, 255),
    }
}

pub const CATALOG: [Info; 16] = [
    info(
        "flappy",
        "Flappy Bird",
        Kind::Arcade,
        "Flappy Bird",
        (90, 190, 240),
    ),
    info(
        "paper",
        "Paper Land",
        Kind::Arcade,
        "Paper.io 2",
        (70, 140, 250),
    ),
    info(
        "hotel",
        "Hotel Hop",
        Kind::Arcade,
        "My Perfect Hotel",
        (180, 70, 80),
    ),
    info(
        "snake",
        "Snake",
        Kind::Arcade,
        "Snake.io, Slither.io",
        (90, 170, 70),
    ),
    info("merge", "2048", Kind::Arcade, "2048", (237, 194, 46)),
    info(
        "fruit",
        "Fruit Slice",
        Kind::Arcade,
        "Fruit Ninja",
        (230, 90, 60),
    ),
    info(
        "stack",
        "Stack Tower",
        Kind::Arcade,
        "Stack",
        (120, 110, 230),
    ),
    info(
        "breakout",
        "Brick Breaker",
        Kind::Arcade,
        "Bricks Breaker Quest",
        (80, 60, 160),
    ),
    info(
        "jumper",
        "Sky Jumper",
        Kind::Arcade,
        "Doodle Jump",
        (110, 190, 90),
    ),
    info(
        "memory",
        "Memory Match",
        Kind::Arcade,
        "Memory card games",
        (60, 170, 200),
    ),
    info(
        "crossy",
        "Road Hopper",
        Kind::Arcade,
        "Crossy Road",
        (100, 190, 80),
    ),
    info(
        "helix",
        "Helix Drop",
        Kind::Arcade,
        "Helix Jump",
        (255, 130, 70),
    ),
    info(
        "blocks",
        "Block Drop",
        Kind::Arcade,
        "Tetris",
        (110, 90, 210),
    ),
    info(
        "mathpop",
        "Math Pop",
        Kind::Learn,
        "Math games for kids",
        (235, 100, 170),
    ),
    info(
        "count",
        "Count It",
        Kind::Learn,
        "Counting games for kids",
        (250, 170, 60),
    ),
    info(
        "spell",
        "Spell It",
        Kind::Learn,
        "Spelling games",
        (250, 150, 80),
    ),
];

pub fn find(key: &str) -> Option<&'static Info> {
    CATALOG.iter().find(|i| i.key == key)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_learning_game_is_listed_with_its_bands() {
        for e in CATALOG {
            assert_eq!(
                crate::mastery::bands_of(e.key).is_some(),
                e.kind == Kind::Learn,
                "{}",
                e.name
            );
        }
    }

    #[test]
    fn play_games_come_before_learn_games_and_keys_are_unique() {
        let first_learn = CATALOG.iter().position(|e| e.kind == Kind::Learn).unwrap();
        assert!(CATALOG[first_learn..].iter().all(|e| e.kind == Kind::Learn));
        for (i, a) in CATALOG.iter().enumerate() {
            assert!(CATALOG[i + 1..].iter().all(|b| b.key != a.key));
        }
        assert_eq!(find("flappy").unwrap().name, "Flappy Bird");
    }
}
