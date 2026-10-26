use crate::game3d::{Finish, Paint, Shape, Skin};
use bevy::math::Vec2;
use bevy::math::{Quat, Vec3};
use bevy::prelude::{Component, Transform};
use glam::vec3;
use kg_core::geom::Color;

pub const LAYER: f32 = 0.02;

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

    pub fn transform(&self) -> Transform {
        Transform {
            translation: self.origin,
            rotation: Quat::from_rotation_y(self.yaw),
            scale: self.scale,
        }
    }
}

#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub enum Anim {
    Bob,
    Orbit(Vec2, f32),
    Ripple(f32, f32),
    Leg(f32),
    Arm(f32),
    Stroke(f32),
    Kick(f32),
    Turn,
    Wake(usize),
    Fill(usize),
    Charge(usize),
    Drop(usize, usize),
    Wave(usize),
}

#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct Rest(pub Transform);

#[derive(Clone, Copy, Debug)]
pub struct Piece {
    pub shape: Shape,
    pub paint: Paint,
    pub at: Transform,
    pub anim: Option<Anim>,
}

#[derive(Default)]
pub struct Model {
    pub pieces: Vec<Piece>,
    pub xf: Xf,
}

pub fn box_at(base: Vec3, size: Vec3) -> Transform {
    Transform::from_translation(base + vec3(0.0, size.y / 2.0, 0.0)).with_scale(size)
}

pub fn ball_at(c: Vec3, r: Vec3) -> Transform {
    Transform::from_translation(c).with_scale(r * 2.0)
}

fn sides(n: usize) -> u8 {
    (n.clamp(6, 24) as u8 / 2) * 2
}

impl Model {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn tag(&mut self, anim: Anim) {
        if let Some(p) = self.pieces.last_mut() {
            p.anim = Some(anim);
        }
    }

    fn put(&mut self, shape: Shape, finish: Finish, color: Color, center: Vec3, size: Vec3) {
        let finish = if color.a < 0.99 && matches!(finish, Finish::Matte | Finish::Gloss) {
            Finish::Ghost
        } else {
            finish
        };
        self.pieces.push(Piece {
            shape,
            paint: Paint::new(finish, color),
            at: Transform {
                translation: self.xf.point(center),
                rotation: Quat::from_rotation_y(self.xf.yaw),
                scale: size * self.xf.scale,
            },
            anim: None,
        });
    }

    pub fn cuboid(&mut self, base: Vec3, size: Vec3, color: Color) {
        self.put(
            Shape::Cube,
            Finish::Matte,
            color,
            base + vec3(0.0, size.y / 2.0, 0.0),
            size,
        );
    }

    pub fn cuboid2(&mut self, base: Vec3, size: Vec3, side: Color, top: Color) {
        let cap = (size.y * 0.12).min(0.06);
        self.put(
            Shape::Cube,
            Finish::Matte,
            side,
            base + vec3(0.0, (size.y - cap) / 2.0, 0.0),
            vec3(size.x, size.y - cap, size.z),
        );
        self.put(
            Shape::Cube,
            Finish::Matte,
            top,
            base + vec3(0.0, size.y - cap / 2.0, 0.0),
            vec3(size.x, cap, size.z),
        );
    }

    pub fn cylinder(&mut self, base: Vec3, r: f32, h: f32, n: usize, side: Color) {
        self.put(
            Shape::Cylinder(sides(n)),
            Finish::Matte,
            side,
            base + vec3(0.0, h / 2.0, 0.0),
            vec3(r * 2.0, h, r * 2.0),
        );
    }

    pub fn frustum(&mut self, base: Vec3, r0: f32, r1: f32, h: f32, n: usize, side: Color) {
        let (shape, r) = if r1 < 0.08 {
            (Shape::Cone(sides(n)), r0)
        } else {
            (Shape::Cylinder(sides(n)), (r0 + r1) / 2.0)
        };
        self.put(
            shape,
            Finish::Matte,
            side,
            base + vec3(0.0, h / 2.0, 0.0),
            vec3(r * 2.0, h, r * 2.0),
        );
    }

    pub fn sphere(&mut self, c: Vec3, r: Vec3, color: Color) {
        self.put(Shape::Sphere, Finish::Gloss, color, c, r * 2.0);
    }

    fn flat(&mut self, shape: Shape, center: Vec2, size: Vec2, y: f32, color: Color) {
        let finish = if color.a < 0.99 {
            Finish::Ghost
        } else {
            Finish::Matte
        };
        self.put(
            shape,
            finish,
            color,
            vec3(center.x, y, center.y),
            vec3(size.x, 1.0, size.y),
        );
    }

    pub fn disc(&mut self, center: Vec2, r: Vec2, y: f32, color: Color) {
        self.flat(Shape::Disc, center, r * 2.0, y, color);
    }

    pub fn flat_rect(&mut self, center: Vec2, size: Vec2, y: f32, color: Color) {
        self.flat(Shape::Plane, center, size, y, color);
    }

    pub fn floor_at(
        &mut self,
        area: kg_core::geom::Rect,
        y: f32,
        tile: f32,
        sheet: &'static str,
        name: &'static str,
        tint: Color,
    ) {
        let repeat = Vec2::new(area.w / tile, area.h / tile);
        self.pieces.push(Piece {
            shape: Shape::Plane,
            paint: Paint::skinned(Finish::Floor, tint, Skin::Tile(sheet, name, repeat)),
            at: Transform::from_translation(self.xf.point(vec3(
                area.x + area.w / 2.0,
                y,
                area.y + area.h / 2.0,
            )))
            .with_scale(vec3(area.w, 1.0, area.h)),
            anim: None,
        });
    }

    pub fn ring(&mut self, center: Vec2, r: f32, y: f32, color: Color) {
        let finish = if color.a < 0.99 {
            Finish::Ghost
        } else {
            Finish::Glow
        };
        self.put(
            Shape::Ring,
            finish,
            color,
            vec3(center.x, y, center.y),
            vec3(r * 2.0, 1.0, r * 2.0),
        );
    }
}

pub const ARC_DOTS: usize = 24;

pub fn arc_dot(center: Vec2, r: f32, i: usize, y: f32) -> Transform {
    let a =
        std::f32::consts::FRAC_PI_2 - (i as f32 + 0.5) / ARC_DOTS as f32 * std::f32::consts::TAU;
    let p = center + Vec2::new(a.cos(), -a.sin()) * (r * 0.9);
    Transform::from_xyz(p.x, y, p.y).with_scale(vec3(r * 0.24, 1.0, r * 0.24))
}

pub fn arc_shown(k: f32) -> usize {
    (ARC_DOTS as f32 * k.clamp(0.0, 1.0)).ceil() as usize
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn yaw_turns_local_forward_to_face_the_heading() {
        let xf = Xf::at(Vec3::ZERO, std::f32::consts::FRAC_PI_2, 1.0);
        assert!((xf.point(vec3(0.0, 0.0, 1.0)) - vec3(1.0, 0.0, 0.0)).length() < 1e-5);
        let t = xf.transform();
        assert!((t.rotation * vec3(0.0, 0.0, 1.0) - vec3(1.0, 0.0, 0.0)).length() < 1e-5);
    }

    #[test]
    fn a_box_sits_on_its_base_not_its_middle() {
        let mut s = Model::new();
        s.cuboid(Vec3::ZERO, vec3(1.0, 2.0, 1.0), kg_core::geom::WHITE);
        let it = s.pieces[0];
        assert_eq!(it.at.translation, vec3(0.0, 1.0, 0.0));
        assert_eq!(it.at.scale, vec3(1.0, 2.0, 1.0));
        assert_eq!(box_at(Vec3::ZERO, vec3(1.0, 2.0, 1.0)), it.at);
    }

    #[test]
    fn see_through_solids_turn_ghostly() {
        let mut s = Model::new();
        s.sphere(Vec3::ZERO, Vec3::ONE, kg_core::geom::WHITE.with_alpha(0.5));
        assert_eq!(s.pieces[0].paint.finish, Finish::Ghost);
    }

    #[test]
    fn a_progress_arc_shows_one_dot_per_slice_from_the_top() {
        assert_eq!(arc_shown(0.0), 0);
        assert_eq!(arc_shown(0.01), 1);
        assert_eq!(arc_shown(0.5), ARC_DOTS / 2);
        let first = arc_dot(Vec2::ZERO, 1.0, 0, 0.0).translation;
        assert!(first.z < -0.8 && first.x > 0.0);
    }
}
