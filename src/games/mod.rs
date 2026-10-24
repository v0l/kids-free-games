pub mod breakout;
pub mod crossy;
pub mod flappy;
pub mod fruit;
pub mod hotel;
pub mod jumper;
pub mod memory;
pub mod merge;
pub mod paper;
pub mod snake;
pub mod stack;

use crate::engine::Game;
use macroquad::prelude::*;

pub struct Entry {
    pub name: &'static str,
    pub like: &'static str,
    pub color: Color,
    pub make: fn() -> Box<dyn Game>,
    pub icon: fn(Rect),
}

pub fn all() -> Vec<Entry> {
    vec![
        Entry {
            name: "Flappy Bird",
            like: "Flappy Bird",
            color: Color::from_rgba(90, 190, 240, 255),
            make: || Box::new(flappy::Flappy::new()),
            icon: flappy::icon,
        },
        Entry {
            name: "Paper Land",
            like: "Paper.io 2",
            color: Color::from_rgba(70, 140, 250, 255),
            make: || Box::new(paper::Paper::new()),
            icon: paper::icon,
        },
        Entry {
            name: "Hotel Hop",
            like: "Hotel Empire, Doorman Story",
            color: Color::from_rgba(180, 70, 80, 255),
            make: || Box::new(hotel::Hotel::new()),
            icon: hotel::icon,
        },
        Entry {
            name: "Snake",
            like: "Snake.io, Slither.io",
            color: Color::from_rgba(90, 170, 70, 255),
            make: || Box::new(snake::Snake::new()),
            icon: snake::icon,
        },
        Entry {
            name: "2048",
            like: "2048",
            color: Color::from_rgba(237, 194, 46, 255),
            make: || Box::new(merge::Merge::new()),
            icon: merge::icon,
        },
        Entry {
            name: "Fruit Slice",
            like: "Fruit Ninja",
            color: Color::from_rgba(230, 90, 60, 255),
            make: || Box::new(fruit::Fruit::new()),
            icon: fruit::icon,
        },
        Entry {
            name: "Stack Tower",
            like: "Stack",
            color: Color::from_rgba(120, 110, 230, 255),
            make: || Box::new(stack::Stack::new()),
            icon: stack::icon,
        },
        Entry {
            name: "Brick Breaker",
            like: "Bricks Breaker Quest",
            color: Color::from_rgba(80, 60, 160, 255),
            make: || Box::new(breakout::Breakout::new()),
            icon: breakout::icon,
        },
        Entry {
            name: "Sky Jumper",
            like: "Doodle Jump",
            color: Color::from_rgba(110, 190, 90, 255),
            make: || Box::new(jumper::Jumper::new()),
            icon: jumper::icon,
        },
        Entry {
            name: "Memory Match",
            like: "Memory card games",
            color: Color::from_rgba(60, 170, 200, 255),
            make: || Box::new(memory::Memory::new()),
            icon: memory::icon,
        },
        Entry {
            name: "Road Hopper",
            like: "Crossy Road",
            color: Color::from_rgba(100, 190, 80, 255),
            make: || Box::new(crossy::Crossy::new()),
            icon: crossy::icon,
        },
    ]
}
