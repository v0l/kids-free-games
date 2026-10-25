pub mod blocks;
pub mod breakout;
pub mod count;
pub mod crossy;
pub mod flappy;
pub mod fruit;
pub mod helix;
pub mod hotel;
pub mod jumper;
pub mod mathpop;
pub mod memory;
pub mod merge;
pub mod paper;
pub mod snake;
pub mod spell;
pub mod stack;

use crate::engine::Game;
use macroquad::prelude::*;

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

pub struct Entry {
    pub icon_key: &'static str,
    pub name: &'static str,
    pub kind: Kind,
    pub like: &'static str,
    pub color: Color,
    pub make: fn() -> Box<dyn Game>,
    pub icon: fn(Rect),
}

pub fn all() -> Vec<Entry> {
    vec![
        Entry {
            icon_key: "flappy",
            name: "Flappy Bird",
            kind: Kind::Arcade,
            like: "Flappy Bird",
            color: Color::from_rgba(90, 190, 240, 255),
            make: || Box::new(flappy::Flappy::new()),
            icon: flappy::icon,
        },
        Entry {
            icon_key: "paper",
            name: "Paper Land",
            kind: Kind::Arcade,
            like: "Paper.io 2",
            color: Color::from_rgba(70, 140, 250, 255),
            make: || Box::new(paper::Paper::new()),
            icon: paper::icon,
        },
        Entry {
            icon_key: "hotel",
            name: "Hotel Hop",
            kind: Kind::Arcade,
            like: "Hotel Empire, Doorman Story",
            color: Color::from_rgba(180, 70, 80, 255),
            make: || Box::new(hotel::Hotel::new()),
            icon: hotel::icon,
        },
        Entry {
            icon_key: "snake",
            name: "Snake",
            kind: Kind::Arcade,
            like: "Snake.io, Slither.io",
            color: Color::from_rgba(90, 170, 70, 255),
            make: || Box::new(snake::Snake::new()),
            icon: snake::icon,
        },
        Entry {
            icon_key: "merge",
            name: "2048",
            kind: Kind::Arcade,
            like: "2048",
            color: Color::from_rgba(237, 194, 46, 255),
            make: || Box::new(merge::Merge::new()),
            icon: merge::icon,
        },
        Entry {
            icon_key: "fruit",
            name: "Fruit Slice",
            kind: Kind::Arcade,
            like: "Fruit Ninja",
            color: Color::from_rgba(230, 90, 60, 255),
            make: || Box::new(fruit::Fruit::new()),
            icon: fruit::icon,
        },
        Entry {
            icon_key: "stack",
            name: "Stack Tower",
            kind: Kind::Arcade,
            like: "Stack",
            color: Color::from_rgba(120, 110, 230, 255),
            make: || Box::new(stack::Stack::new()),
            icon: stack::icon,
        },
        Entry {
            icon_key: "breakout",
            name: "Brick Breaker",
            kind: Kind::Arcade,
            like: "Bricks Breaker Quest",
            color: Color::from_rgba(80, 60, 160, 255),
            make: || Box::new(breakout::Breakout::new()),
            icon: breakout::icon,
        },
        Entry {
            icon_key: "jumper",
            name: "Sky Jumper",
            kind: Kind::Arcade,
            like: "Doodle Jump",
            color: Color::from_rgba(110, 190, 90, 255),
            make: || Box::new(jumper::Jumper::new()),
            icon: jumper::icon,
        },
        Entry {
            icon_key: "memory",
            name: "Memory Match",
            kind: Kind::Arcade,
            like: "Memory card games",
            color: Color::from_rgba(60, 170, 200, 255),
            make: || Box::new(memory::Memory::new()),
            icon: memory::icon,
        },
        Entry {
            icon_key: "crossy",
            name: "Road Hopper",
            kind: Kind::Arcade,
            like: "Crossy Road",
            color: Color::from_rgba(100, 190, 80, 255),
            make: || Box::new(crossy::Crossy::new()),
            icon: crossy::icon,
        },
        Entry {
            icon_key: "helix",
            name: "Helix Drop",
            kind: Kind::Arcade,
            like: "Helix Jump",
            color: Color::from_rgba(255, 130, 70, 255),
            make: || Box::new(helix::Helix::new()),
            icon: helix::icon,
        },
        Entry {
            icon_key: "blocks",
            name: "Block Drop",
            kind: Kind::Arcade,
            like: "Tetris",
            color: Color::from_rgba(110, 90, 210, 255),
            make: || Box::new(blocks::Blocks::new()),
            icon: blocks::icon,
        },
        Entry {
            icon_key: "mathpop",
            name: "Math Pop",
            kind: Kind::Learn,
            like: "Math games for kids",
            color: Color::from_rgba(235, 100, 170, 255),
            make: || Box::new(mathpop::MathPop::new()),
            icon: mathpop::icon,
        },
        Entry {
            icon_key: "count",
            name: "Count It",
            kind: Kind::Learn,
            like: "Counting games for kids",
            color: Color::from_rgba(250, 170, 60, 255),
            make: || Box::new(count::Count::new()),
            icon: count::icon,
        },
        Entry {
            icon_key: "spell",
            name: "Spell It",
            kind: Kind::Learn,
            like: "Spelling games",
            color: Color::from_rgba(250, 150, 80, 255),
            make: || Box::new(spell::Spell::new()),
            icon: spell::icon,
        },
    ]
}
