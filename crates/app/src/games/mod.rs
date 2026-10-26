use crate::paint::Pen;
use kg_core::engine::{Input, View};
use kg_core::games::{Info, CATALOG};

pub mod blocks;
pub mod breakout;
pub mod crossy;
pub mod flappy;
pub mod fruit;
pub mod jumper;
pub mod merge;
pub mod snake;
pub mod stack;

pub trait IntoAny {
    fn into_any(self: Box<Self>) -> Box<dyn std::any::Any>;
}

impl<T: 'static> IntoAny for T {
    fn into_any(self: Box<Self>) -> Box<dyn std::any::Any> {
        self
    }
}

pub trait Play: IntoAny {
    fn update(&mut self, input: &Input, view: &View, dt: f32);
    fn draw(&self, pen: &mut Pen, view: &View);
    fn score(&self) -> u32 {
        0
    }
    fn over(&self) -> bool {
        false
    }
    fn as_any(&mut self) -> &mut dyn std::any::Any;
}

pub struct Entry {
    pub info: &'static Info,
    pub make: Option<fn() -> Box<dyn Play>>,
}

impl Entry {
    pub fn ready(&self) -> bool {
        self.make.is_some()
    }
}

fn maker(key: &str) -> Option<fn() -> Box<dyn Play>> {
    match key {
        flappy::KEY => Some(|| Box::new(flappy::FlappyPlay::new())),
        crossy::KEY => Some(|| Box::new(crossy::CrossyPlay::new())),
        snake::KEY => Some(|| Box::new(snake::SnakePlay::new())),
        merge::KEY => Some(|| Box::new(merge::MergePlay::new())),
        fruit::KEY => Some(|| Box::new(fruit::FruitPlay::new())),
        stack::KEY => Some(|| Box::new(stack::StackPlay::new())),
        breakout::KEY => Some(|| Box::new(breakout::BreakoutPlay::new())),
        jumper::KEY => Some(|| Box::new(jumper::JumperPlay::new())),
        blocks::KEY => Some(|| Box::new(blocks::BlocksPlay::new())),
        _ => None,
    }
}

pub fn all() -> Vec<Entry> {
    CATALOG
        .iter()
        .map(|info| Entry {
            info,
            make: maker(info.key),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_ported_game_survives_a_minute_of_random_input() {
        quad_rand::srand(1);
        for v in [View::ipad_landscape(), View::ipad_portrait()] {
            for e in all().into_iter().filter(Entry::ready) {
                kg_core::save::reset_for_test();
                let mut g = (e.make.unwrap())();
                let mut pen = Pen::default();
                for f in 0..3600 {
                    let pos = glam::vec2(
                        quad_rand::gen_range(0.0, v.w),
                        quad_rand::gen_range(0.0, v.h),
                    );
                    let mut input = Input {
                        pos,
                        prev_pos: pos - glam::vec2(30.0, 10.0),
                        down: f % 7 < 4,
                        ..Default::default()
                    };
                    input.pressed = f % 7 == 0;
                    input.released = f % 7 == 4;
                    if f % 11 == 0 {
                        input.swipe = Some(kg_core::engine::Dir::ALL[quad_rand::gen_range(0, 4)]);
                    }
                    g.update(&input, &v, 1.0 / 60.0);
                    if f % 60 == 0 {
                        pen.begin(v, 1.0);
                        g.draw(&mut pen, &v);
                    }
                }
            }
        }
    }

    #[test]
    fn flappy_is_ported_and_every_catalog_game_has_a_tile() {
        let all = all();
        assert_eq!(all.len(), CATALOG.len());
        assert!(all[0].ready());
        assert_eq!(all[0].info.key, "flappy");
    }
}
