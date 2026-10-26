pub mod build;
pub mod hud;
pub mod models;
pub mod world;

use crate::game3d::World3d;
use crate::paint::Pen;
use hud::Hud;
use kg_core::engine::{Game, Input, View};
use kg_core::fx::Particles;
use kg_core::games::hotel::{Hotel, Mode};

pub const KEY: &str = kg_core::games::hotel::GAME;
pub const SHEET: &str = "hotel";

pub struct HotelPlay {
    pub h: Hotel,
    pub sparks: Particles,
    mm: f32,
}

impl HotelPlay {
    pub fn new() -> Self {
        Self::with(Hotel::new())
    }

    pub fn with(h: Hotel) -> Self {
        Self {
            h,
            sparks: Particles::default(),
            mm: kg_core::engine::FALLBACK_MM,
        }
    }

    fn hud(&self, view: &View) -> Hud<'_> {
        Hud {
            h: &self.h,
            cam: self.h.cam(view),
            view: *view,
        }
    }
}

impl Default for HotelPlay {
    fn default() -> Self {
        Self::new()
    }
}

impl crate::games::Play for HotelPlay {
    fn update(&mut self, input: &Input, view: &View, dt: f32) {
        self.mm = input.mm();
        self.h.update(input, view, dt);
        self.sparks.update(dt);
        let cam = self.h.cam(view);
        for (p, c) in std::mem::take(&mut self.h.bursts) {
            if let Some(at) = cam.project(view, p) {
                self.sparks.sparkle(at, c);
            }
        }
    }

    fn draw(&self, pen: &mut Pen, view: &View) {
        let hud = self.hud(view);
        hud.draw_overhead(pen);
        pen.particles(&self.sparks);
        hud.draw_flyers(pen);
        hud.draw_hud(pen);
        hud.draw_stick(pen, self.mm);
        hud.draw_overlay(pen);
    }

    fn score(&self) -> u32 {
        self.h.score().max(0) as u32
    }

    fn over(&self) -> bool {
        matches!(self.h.mode, Mode::Moving(_) | Mode::Done(_))
    }

    fn world(&self) -> World3d {
        World3d::Hotel
    }

    fn as_any(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::games::Play;
    use kg_core::games::hotel::sim::Sim;
    use kg_core::games::hotel::SPRITES;

    fn fresh(number: u32, built: &[bool]) -> HotelPlay {
        kg_core::save::reset_for_test();
        kg_core::profile::create("Mia", 0);
        HotelPlay::with(Hotel::with(Sim::new(number, built, 50, 1)))
    }

    fn drawn(p: &HotelPlay, v: &View) -> Pen {
        let mut pen = Pen::default();
        pen.begin(*v, 1.0);
        p.draw(&mut pen, v);
        pen
    }

    #[test]
    fn hotel_sprites_all_exist() {
        let have = crate::art::names(SHEET);
        for n in SPRITES {
            assert!(have.iter().any(|h| h == n), "hotel sheet lacks {n}");
        }
        for n in ["p_star", "p_coin", "icon_lock"] {
            assert!(
                crate::art::names("ui").iter().any(|h| h == n),
                "ui sheet lacks {n}"
            );
        }
    }

    #[test]
    fn the_hud_draws_over_the_3d_world() {
        let v = View::ipad_landscape();
        let p = fresh(1, &[]);
        let pen = drawn(&p, &v);
        assert!(pen.vertex_count() > 100);
        assert_eq!(p.world(), World3d::Hotel);
    }

    #[test]
    fn the_game_over_hook_fires_only_between_hotels() {
        let mut p = fresh(1, &[]);
        assert!(!p.over());
        p.h.mode = Mode::Moving(0.5);
        assert!(p.over());
        p.h.mode = Mode::LevelUp(2, 0.1);
        assert!(!p.over());
    }

    #[test]
    fn juice_bursts_turn_into_sparks_on_screen() {
        let v = View::ipad_landscape();
        let mut p = fresh(1, &[]);
        p.h.bursts
            .push((glam::vec3(0.0, 1.0, 1.2), kg_core::geom::GOLD));
        p.update(&Input::default(), &v, 1.0 / 60.0);
        assert!(p.h.bursts.is_empty());
        assert!(!p.sparks.list.is_empty());
    }
}
