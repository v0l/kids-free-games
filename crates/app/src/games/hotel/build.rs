use crate::world3d::{Scene3d, Shape, Surface};
use bevy::math::{Quat, Vec3};
use glam::{vec3, Vec2};
use kg_core::geom::Color;

#[derive(Clone, Copy, Debug)]
pub struct Xf {
    pub origin: Vec3,
    pub yaw: f32,
    pub scale: Vec3,
}

impl Default for Xf {
    fn default() -> Self {
        Self {
            origin: Vec3::ZERO,
            yaw: 0.0,
            scale: Vec3::ONE,
        }
    }
}

impl Xf {
    pub fn at(origin: Vec3, yaw: f32, scale: f32) -> Self {
        Self {
            origin,
            yaw,
            scale: Vec3::splat(scale),
        }
    }

    pub fn point(&self, p: Vec3) -> Vec3 {
        let p = p * self.scale;
        let (s, c) = self.yaw.sin_cos();
        self.origin + vec3(p.x * c + p.z * s, p.y, -p.x * s + p.z * c)
    }

    pub fn then(&self, inner: Xf) -> Xf {
        Xf {
            origin: self.point(inner.origin),
            yaw: self.yaw + inner.yaw,
            scale: self.scale * inner.scale,
        }
    }
}

pub struct Scene {
    pub out: Scene3d,
    pub xf: Xf,
    pub alpha: f32,
}

impl Scene {
    pub fn new() -> Self {
        Self {
            out: Scene3d::default(),
            xf: Xf::default(),
            alpha: 1.0,
        }
    }

    fn rot(&self) -> Quat {
        Quat::from_rotation_y(self.xf.yaw)
    }

    fn put(&mut self, shape: Shape, surface: Surface, color: Color, center: Vec3, size: Vec3) {
        let at = self.xf.point(center);
        let mut c = color;
        c.a *= self.alpha;
        let surface = if c.a < 0.99 && matches!(surface, Surface::Matte | Surface::Gloss) {
            Surface::Ghost
        } else {
            surface
        };
        let rot = self.rot();
        self.out.push(shape, surface, c, at, rot, size * self.xf.scale);
    }

    pub fn cuboid(&mut self, base: Vec3, size: Vec3, color: Color) {
        self.put(Shape::Cube, Surface::Matte, color, base + vec3(0.0, size.y / 2.0, 0.0), size);
    }

    pub fn cuboid2(&mut self, base: Vec3, size: Vec3, side: Color, top: Color) {
        let cap = (size.y * 0.12).min(0.06);
        self.put(
            Shape::Cube,
            Surface::Matte,
            side,
            base + vec3(0.0, (size.y - cap) / 2.0, 0.0),
            vec3(size.x, size.y - cap, size.z),
        );
        self.put(
            Shape::Cube,
            Surface::Matte,
            top,
            base + vec3(0.0, size.y - cap / 2.0, 0.0),
            vec3(size.x, cap, size.z),
        );
    }

    pub fn cylinder(&mut self, base: Vec3, r: f32, h: f32, sides: usize, side: Color, _top: Color) {
        let n = (sides.clamp(6, 24) as u8 / 2) * 2;
        self.put(Shape::Cylinder(n), Surface::Matte, side, base + vec3(0.0, h / 2.0, 0.0), vec3(r * 2.0, h, r * 2.0));
    }

    #[allow(clippy::too_many_arguments)]
    pub fn frustum(&mut self, base: Vec3, r0: f32, r1: f32, h: f32, sides: usize, side: Color, _top: Color) {
        let n = (sides.clamp(6, 24) as u8 / 2) * 2;
        if r1 < 0.08 {
            self.put(Shape::Cone(n), Surface::Matte, side, base + vec3(0.0, h / 2.0, 0.0), vec3(r0 * 2.0, h, r0 * 2.0));
        } else {
            let r = (r0 + r1) / 2.0;
            self.put(Shape::Cylinder(n), Surface::Matte, side, base + vec3(0.0, h / 2.0, 0.0), vec3(r * 2.0, h, r * 2.0));
        }
    }

    pub fn sphere(&mut self, c: Vec3, r: Vec3, _detail: usize, color: Color) {
        self.put(Shape::Sphere, Surface::Gloss, color, c, r * 2.0);
    }

    pub fn disc(&mut self, center: Vec2, r: Vec2, y: f32, color: Color) {
        let surface = if color.a < 0.99 { Surface::Ghost } else { Surface::Matte };
        self.put(Shape::Disc, surface, color, vec3(center.x, y, center.y), vec3(r.x * 2.0, 1.0, r.y * 2.0));
    }

    pub fn flat_rect(&mut self, center: Vec2, size: Vec2, y: f32, color: Color) {
        let surface = if color.a < 0.99 { Surface::Ghost } else { Surface::Matte };
        self.put(Shape::Plane, surface, color, vec3(center.x, y, center.y), vec3(size.x, 1.0, size.y));
    }

    pub fn floor(&mut self, area: kg_core::geom::Rect, tile: f32, sheet: &'static str, name: &'static str, tint: Color) {
        let c = vec3(area.x + area.w / 2.0, 0.0, area.y + area.h / 2.0);
        let at = self.xf.point(c);
        self.out.push(Shape::Plane, Surface::Floor(sheet, name), tint, at, Quat::IDENTITY, vec3(area.w, 1.0, area.h));
        if let Some(last) = self.out.items.last_mut() {
            last.uv_repeat = bevy::math::Vec2::new(area.w / tile, area.h / tile);
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn ring_flat(&mut self, center: Vec2, r: f32, _width: f32, y: f32, k: f32, color: Color) {
        if k >= 0.999 {
            let surface = if color.a < 0.99 { Surface::Ghost } else { Surface::Glow };
            self.put(Shape::Ring, surface, color, vec3(center.x, y, center.y), vec3(r * 2.0, 1.0, r * 2.0));
            return;
        }
        let n = (24.0 * k.clamp(0.0, 1.0)).ceil() as usize;
        for i in 0..n {
            let a = std::f32::consts::FRAC_PI_2 - (i as f32 + 0.5) / 24.0 * std::f32::consts::TAU;
            let p = center + glam::vec2(a.cos(), -a.sin()) * (r * 0.9);
            self.put(Shape::Disc, Surface::Glow, color, vec3(p.x, y, p.y), vec3(r * 0.24, 1.0, r * 0.24));
        }
    }
}

impl Default for Scene {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn yaw_turns_local_forward_to_face_the_heading() {
        let xf = Xf::at(Vec3::ZERO, std::f32::consts::FRAC_PI_2, 1.0);
        assert!((xf.point(vec3(0.0, 0.0, 1.0)) - vec3(1.0, 0.0, 0.0)).length() < 1e-5);
    }

    #[test]
    fn a_box_sits_on_its_base_not_its_middle() {
        let mut s = Scene::new();
        s.cuboid(Vec3::ZERO, vec3(1.0, 2.0, 1.0), kg_core::geom::WHITE);
        let it = s.out.items[0];
        assert_eq!(it.at, vec3(0.0, 1.0, 0.0));
        assert_eq!(it.scale, vec3(1.0, 2.0, 1.0));
    }

    #[test]
    fn faded_scenes_turn_solid_parts_see_through() {
        let mut s = Scene::new();
        s.alpha = 0.5;
        s.sphere(Vec3::ZERO, Vec3::ONE, 4, kg_core::geom::WHITE);
        assert_eq!(s.out.items[0].surface, Surface::Ghost);
        assert!(!s.out.items[0].shadow);
    }
}
