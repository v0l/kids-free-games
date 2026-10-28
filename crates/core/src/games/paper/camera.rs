use crate::engine::View;
use glam::{vec2, vec3, Vec2, Vec3};

pub const FOV: f32 = 0.62;
pub const PITCH: f32 = 1.12;
pub const SLAB: f32 = 0.5;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Cam {
    pub eye: Vec3,
    pub target: Vec3,
    pub aspect: f32,
}

impl Cam {
    pub fn new(view: &View, focus: Vec2, zoom: f32) -> Self {
        let f = view.full();
        let aspect = f.w / f.h.max(1.0);
        let t = (FOV / 2.0).tan();
        let dist = if aspect < 1.0 {
            zoom / (2.0 * t * aspect)
        } else {
            zoom * PITCH.sin() / (2.0 * t)
        };
        let target = vec3(focus.x, 0.0, focus.y);
        let eye = target + vec3(0.0, PITCH.sin(), PITCH.cos()) * dist;
        Self {
            eye,
            target,
            aspect,
        }
    }

    pub fn distance(&self) -> f32 {
        (self.eye - self.target).length()
    }

    fn basis(&self) -> (Vec3, Vec3, Vec3) {
        let fwd = (self.target - self.eye).normalize();
        let right = fwd.cross(Vec3::Y).normalize();
        let up = right.cross(fwd);
        (fwd, right, up)
    }

    pub fn project(&self, view: &View, p: Vec3) -> Vec2 {
        let f = view.full();
        let (fwd, right, up) = self.basis();
        let d = p - self.eye;
        let depth = d.dot(fwd).max(0.01);
        let k = 1.0 / (FOV / 2.0).tan();
        let n = vec2(
            d.dot(right) * k / (depth * self.aspect),
            d.dot(up) * k / depth,
        );
        vec2(f.x + (n.x * 0.5 + 0.5) * f.w, f.y + (0.5 - n.y * 0.5) * f.h)
    }

    pub fn ground(&self, view: &View, s: Vec2, height: f32) -> Option<Vec2> {
        let f = view.full();
        let (fwd, right, up) = self.basis();
        let t = (FOV / 2.0).tan();
        let n = vec2((s.x - f.x) / f.w * 2.0 - 1.0, 1.0 - (s.y - f.y) / f.h * 2.0);
        let ray = fwd + right * (n.x * t * self.aspect) + up * (n.y * t);
        if ray.y >= -1e-4 {
            return None;
        }
        let k = (height - self.eye.y) / ray.y;
        let hit = self.eye + ray * k;
        Some(vec2(hit.x, hit.z))
    }
}
