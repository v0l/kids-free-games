use crate::paint::Pen;
use glam::Vec2;
use kg_core::engine::DirControl;
use kg_core::geom::Color;

const MIN_RADIUS: f32 = 30.0;

pub fn draw_stick(pen: &mut Pen, stick: &DirControl) {
    if !stick.active {
        return;
    }
    let r = stick.radius.max(MIN_RADIUS);
    pen.circle(stick.origin, r, Color::new(1.0, 1.0, 1.0, 0.18));
    pen.ring(stick.origin, r, 3.0, Color::new(1.0, 1.0, 1.0, 0.35));
    let knob: Vec2 = stick.knob.clamp_length_max(r);
    pen.circle(
        stick.origin + knob,
        r * 0.45,
        Color::new(1.0, 1.0, 1.0, 0.55),
    );
}
