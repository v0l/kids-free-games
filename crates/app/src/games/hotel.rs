pub mod build;
pub mod hud;
pub mod models;
pub mod world;

use crate::paint::Pen;
use build::Scene;
use hud::Hud;
use kg_core::engine::{Game, Input, View};
use kg_core::fx::Particles;
use kg_core::games::hotel::{Hotel, Mode};
use world::World;

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

    pub fn scene(&self, view: &View) -> crate::world3d::Scene3d {
        let mut s = Scene::new();
        World { h: &self.h }.draw_world(&mut s);
        let cam = self.h.cam(view);
        let shake = self.h.shake.offset();
        let right = cam.right() * shake.x * 0.02;
        let up = cam.up() * -shake.y * 0.02;
        let mut out = s.out;
        out.eye = to_bevy(cam.eye + right + up);
        out.target = to_bevy(cam.target + right + up);
        out.fov = cam.fov;
        out.sky = self.h.sim.lay.theme.outside;
        out.shadow_reach = 40.0 / self.h.zoom.max(0.3);
        out
    }

    fn hud(&self, view: &View) -> Hud<'_> {
        Hud {
            h: &self.h,
            cam: self.h.cam(view),
            view: *view,
        }
    }
}

fn to_bevy(v: glam::Vec3) -> bevy::math::Vec3 {
    bevy::math::Vec3::new(v.x, v.y, v.z)
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
        crate::world3d::submit(self.scene(view));
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

    fn as_any(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::games::Play;
    use kg_core::games::hotel::layout::{Build, Layout};
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
        let _ = crate::world3d::take();
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
    fn the_hotel_is_built_in_3d_and_the_hud_draws_on_top() {
        let v = View::ipad_landscape();
        let p = fresh(1, &[]);
        let scene = p.scene(&v);
        assert!(scene.items.len() > 150, "only {} parts", scene.items.len());
        assert!(scene.eye.y > 5.0 && scene.eye.z > scene.target.z);
        let pen = drawn(&p, &v);
        assert!(pen.vertex_count() > 100);
    }

    #[test]
    fn drawing_hands_the_3d_scene_to_bevy_each_frame() {
        let v = View::ipad_portrait();
        let p = fresh(2, &[]);
        let mut pen = Pen::default();
        pen.begin(v, 1.0);
        p.draw(&mut pen, &v);
        assert!(crate::world3d::take().is_some());
        assert!(crate::world3d::take().is_none());
    }

    #[test]
    fn every_hotel_fully_built_draws_without_panicking() {
        for n in 1..=kg_core::games::hotel::layout::MAX_HOTEL {
            let pads = Layout::new(n).pads.len();
            let mut p = fresh(n, &vec![true; pads - 1]);
            p.h.sim.add_swimmers(3);
            for v in [View::ipad_landscape(), View::ipad_portrait()] {
                let scene = p.scene(&v);
                assert!(
                    scene.items.len() > 300,
                    "hotel {n}: {} parts",
                    scene.items.len()
                );
                drawn(&p, &v);
            }
        }
    }

    #[test]
    fn upgrades_you_buy_show_in_the_3d_scene() {
        let v = View::ipad_landscape();
        let lay = Layout::new(2);
        let at = |b: Build| lay.pads.iter().position(|p| p.build == b).unwrap();
        let mut built = vec![false; lay.pads.len()];
        let before = fresh(2, &built).scene(&v).items.len();
        built[at(Build::Room(1))] = true;
        built[at(Build::Decor(0, 2))] = true;
        built[at(Build::Cafe)] = true;
        let after = fresh(2, &built).scene(&v).items.len();
        assert!(after > before + 30, "{before} -> {after}");
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
