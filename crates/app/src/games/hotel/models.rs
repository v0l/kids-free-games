use super::build::{Scene, Xf};
use bevy::math::Vec3;
use glam::{vec2, vec3, Vec2};
use kg_core::games::hotel::layout::Theme;
use kg_core::geom::{Color, Rect, WHITE};

pub const SKIN: [Color; 4] = [
    Color::new(1.0, 0.85, 0.72, 1.0),
    Color::new(0.93, 0.72, 0.55, 1.0),
    Color::new(0.72, 0.5, 0.35, 1.0),
    Color::new(0.5, 0.33, 0.22, 1.0),
];

#[derive(Clone, Copy, Debug)]
pub struct Look {
    pub body: Color,
    pub legs: Color,
    pub skin: Color,
    pub hair: Color,
    pub hat: Option<Color>,
    pub wide: f32,
}

pub fn guest_look(look: u8, vip: bool) -> Look {
    let shirts = [
        rgb(255, 120, 90),
        rgb(90, 170, 255),
        rgb(120, 210, 110),
        rgb(255, 200, 60),
        rgb(200, 120, 230),
        rgb(255, 150, 200),
        rgb(80, 200, 200),
        rgb(250, 140, 40),
        rgb(160, 160, 250),
        rgb(240, 90, 120),
    ];
    let hair = [
        rgb(60, 40, 30),
        rgb(230, 190, 90),
        rgb(150, 70, 40),
        rgb(30, 30, 40),
        rgb(200, 100, 50),
    ];
    let i = look as usize;
    Look {
        body: if vip { rgb(50, 40, 70) } else { shirts[i % 10] },
        legs: [rgb(70, 90, 160), rgb(90, 70, 60), rgb(60, 60, 80)][i % 3],
        skin: SKIN[(i * 7 / 3) % 4],
        hair: hair[(i * 3) % 5],
        hat: if vip {
            Some(rgb(255, 205, 50))
        } else if i % 3 == 1 {
            Some(shirts[(i + 4) % 10])
        } else {
            None
        },
        wide: 1.0 + (i % 4) as f32 * 0.06,
    }
}

pub fn player_look() -> Look {
    Look {
        body: rgb(225, 45, 50),
        legs: rgb(40, 40, 70),
        skin: SKIN[0],
        hair: rgb(90, 55, 30),
        hat: Some(rgb(225, 45, 50)),
        wide: 1.05,
    }
}

pub fn staff_look(kind: u8) -> Look {
    let (body, hat) = match kind {
        0 => (rgb(40, 60, 120), Some(rgb(40, 60, 120))),
        1 => (rgb(110, 190, 240), Some(rgb(255, 255, 255))),
        2 => (rgb(90, 150, 80), Some(rgb(250, 200, 60))),
        3 => (rgb(250, 250, 250), Some(rgb(250, 250, 250))),
        _ => (rgb(240, 70, 60), Some(rgb(250, 220, 60))),
    };
    Look {
        body,
        legs: rgb(50, 50, 70),
        skin: SKIN[(kind as usize + 1) % 4],
        hair: rgb(70, 45, 30),
        hat,
        wide: 1.0,
    }
}

pub const fn rgb(r: u8, g: u8, b: u8) -> Color {
    Color::new(r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0, 1.0)
}

pub fn darker(c: Color, k: f32) -> Color {
    Color::new(c.r * k, c.g * k, c.b * k, c.a)
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Pose {
    pub walk: f32,
    pub sit: bool,
    pub lie: bool,
    pub arms_up: bool,
    pub hop: f32,
    pub squash: f32,
    pub shoes: Option<Color>,
    pub sweep: f32,
}

pub fn person(s: &mut Scene, at: Vec2, face: f32, size: f32, look: &Look, pose: Pose) {
    let swing = if pose.walk > 0.0 {
        pose.walk.sin()
    } else {
        0.0
    };
    let bob = if pose.walk > 0.0 {
        (pose.walk * 2.0).sin().abs() * 0.06
    } else {
        0.0
    };
    let sq = 1.0 + pose.squash;
    let base = vec3(at.x, bob + pose.hop, at.y);
    let outer = s.xf;
    s.xf = Xf {
        origin: base,
        yaw: face,
        scale: vec3(size / sq.sqrt(), size * sq, size / sq.sqrt()),
    };
    if pose.lie {
        s.xf.scale = Vec3::splat(size);
        s.xf.origin.y += 0.55;
        lying(s, look);
        s.xf = outer;
        return;
    }
    let leg_h = if pose.sit { 0.22 } else { 0.42 };
    let hip = if pose.sit { 0.45 } else { leg_h };
    let w = look.wide;
    if pose.sit {
        for side in [-1.0, 1.0] {
            s.cuboid(
                vec3(0.11 * side, 0.32, 0.12),
                vec3(0.15, 0.13, 0.4),
                look.legs,
            );
            s.cuboid(
                vec3(0.11 * side, 0.0, 0.3),
                vec3(0.14, 0.32, 0.14),
                look.legs,
            );
        }
    } else {
        for side in [-1.0f32, 1.0] {
            let k = swing * side * 0.12;
            s.cuboid(
                vec3(0.11 * side, 0.0, k),
                vec3(0.15, leg_h, 0.16),
                look.legs,
            );
            s.cuboid(
                vec3(0.11 * side, 0.0, k + 0.04),
                vec3(0.17, if pose.shoes.is_some() { 0.12 } else { 0.08 }, 0.24),
                pose.shoes.unwrap_or(rgb(60, 40, 35)),
            );
        }
    }
    s.sphere(
        vec3(0.0, hip + 0.3, 0.0),
        vec3(0.3 * w, 0.34, 0.25 * w),
        5,
        look.body,
    );
    for side in [-1.0f32, 1.0] {
        let k = if pose.arms_up {
            0.0
        } else if pose.sweep > 0.0 {
            (pose.sweep * 14.0).sin() * 0.18 * side
        } else {
            -swing * side * 0.14
        };
        let (y, z, h) = if pose.arms_up {
            (hip + 0.32, 0.22, 0.32)
        } else {
            (hip + 0.14, k, 0.36)
        };
        s.cuboid(vec3(0.33 * w * side, y, z), vec3(0.12, h, 0.12), look.body);
        s.sphere(
            vec3(0.33 * w * side, y - 0.02, z),
            vec3(0.07, 0.07, 0.07),
            3,
            look.skin,
        );
    }
    let head = vec3(0.0, hip + 0.86, 0.0);
    s.sphere(head, vec3(0.3, 0.29, 0.28), 6, look.skin);
    s.sphere(
        head + vec3(0.0, 0.08, -0.05),
        vec3(0.31, 0.24, 0.27),
        5,
        look.hair,
    );
    for side in [-1.0f32, 1.0] {
        s.sphere(
            head + vec3(0.1 * side, 0.02, 0.255),
            vec3(0.055, 0.07, 0.04),
            3,
            rgb(40, 30, 40),
        );
        s.sphere(
            head + vec3(0.1 * side + 0.015, 0.045, 0.285),
            vec3(0.018, 0.022, 0.015),
            2,
            WHITE,
        );
    }
    s.sphere(
        head + vec3(0.0, -0.1, 0.25),
        vec3(0.07, 0.03, 0.04),
        2,
        rgb(200, 70, 80),
    );
    if let Some(h) = look.hat {
        s.cylinder(
            head + vec3(0.0, 0.18, 0.0),
            0.22,
            0.16,
            10,
            h,
            darker(h, 1.1),
        );
        s.cylinder(
            head + vec3(0.0, 0.18, 0.06),
            0.3,
            0.03,
            10,
            darker(h, 0.85),
            darker(h, 0.95),
        );
    }
    s.xf = outer;
}

fn lying(s: &mut Scene, look: &Look) {
    s.sphere(vec3(0.0, 0.1, 0.55), vec3(0.27, 0.25, 0.25), 5, look.skin);
    s.sphere(vec3(0.0, 0.17, 0.5), vec3(0.28, 0.2, 0.24), 4, look.hair);
}

pub fn shadow(s: &mut Scene, at: Vec2, r: f32) {
    s.disc(
        at,
        vec2(r, r * 0.9),
        0.012,
        Color::new(0.1, 0.05, 0.15, 0.22),
    );
}

pub fn desk(s: &mut Scene, r: Rect, theme: &Theme) {
    let c = vec2(r.x + r.w / 2.0, r.y + r.h / 2.0);
    s.cuboid2(
        vec3(c.x, 0.0, c.y),
        vec3(r.w, 1.05, r.h),
        rgb(150, 90, 55),
        theme.trim,
    );
    s.cuboid(
        vec3(c.x, 0.25, c.y - r.h / 2.0 - 0.01),
        vec3(r.w * 0.8, 0.45, 0.02),
        darker(theme.trim, 0.9),
    );
    s.cylinder(
        vec3(c.x + 0.9, 1.05, c.y),
        0.12,
        0.05,
        8,
        rgb(240, 200, 60),
        rgb(255, 230, 120),
    );
    s.sphere(
        vec3(c.x + 0.9, 1.12, c.y),
        vec3(0.06, 0.06, 0.06),
        3,
        rgb(255, 220, 80),
    );
    s.cuboid(
        vec3(c.x - 0.8, 1.05, c.y),
        vec3(0.45, 0.32, 0.06),
        rgb(60, 60, 80),
    );
}

pub fn bed(s: &mut Scene, at: Vec2, sheet: Color, wood: Color, fancy: bool) {
    s.cuboid2(
        vec3(at.x, 0.0, at.y),
        vec3(1.5, 0.4, 2.1),
        wood,
        darker(wood, 1.1),
    );
    s.cuboid2(
        vec3(at.x, 0.4, at.y + 0.1),
        vec3(1.45, 0.2, 1.85),
        sheet,
        darker(sheet, 1.05),
    );
    s.cuboid(
        vec3(at.x, 0.0, at.y - 1.0),
        vec3(1.6, if fancy { 1.3 } else { 1.0 }, 0.15),
        wood,
    );
    for k in [-0.35, 0.35] {
        s.sphere(
            vec3(at.x + k, 0.67, at.y - 0.65),
            vec3(0.3, 0.1, 0.2),
            4,
            WHITE,
        );
    }
    if fancy {
        s.sphere(
            vec3(at.x, 1.3, at.y - 1.0),
            vec3(0.18, 0.18, 0.1),
            4,
            rgb(255, 205, 60),
        );
    }
}

pub fn mess(s: &mut Scene, at: Vec2, k: f32) {
    let a = Color::new(0.55, 0.38, 0.25, 1.0);
    let spots = [
        vec2(-0.4, 0.4),
        vec2(0.3, 0.8),
        vec2(0.6, -0.2),
        vec2(-0.2, 1.3),
    ];
    for (i, o) in spots.iter().enumerate() {
        let r = (0.32 - i as f32 * 0.03) * k;
        if r > 0.02 {
            s.disc(at + *o, vec2(r, r * 0.75), 0.02, a);
        }
    }
    if k > 0.3 {
        s.cuboid(
            vec3(at.x + 0.9, 0.0, at.y + 0.9),
            vec3(0.35 * k, 0.25 * k, 0.3 * k),
            rgb(240, 240, 250),
        );
    }
}

pub fn plant(s: &mut Scene, at: Vec2, h: f32) {
    s.frustum(
        vec3(at.x, 0.0, at.y),
        0.24,
        0.3,
        0.4,
        8,
        rgb(210, 110, 70),
        rgb(110, 70, 40),
    );
    for (i, o) in [vec2(0.0, 0.0), vec2(0.12, 0.08), vec2(-0.1, 0.1)]
        .iter()
        .enumerate()
    {
        s.sphere(
            vec3(at.x + o.x, 0.55 + h * 0.3 + i as f32 * 0.1, at.y + o.y),
            vec3(0.3, 0.32 * h, 0.3),
            4,
            rgb(70, 170 + i as u8 * 15, 80),
        );
    }
}

pub fn sofa(s: &mut Scene, at: Vec2, c: Color, yaw: f32) {
    let outer = s.xf;
    s.xf = outer.then(Xf::at(vec3(at.x, 0.0, at.y), yaw, 1.0));
    s.cuboid(vec3(0.0, 0.0, 0.0), vec3(1.8, 0.45, 0.8), c);
    s.cuboid(vec3(0.0, 0.0, -0.32), vec3(1.8, 0.95, 0.2), darker(c, 0.9));
    for x in [-0.95, 0.95] {
        s.cuboid(vec3(x, 0.0, 0.0), vec3(0.2, 0.65, 0.8), darker(c, 0.9));
    }
    s.xf = outer;
}

pub fn toilet(s: &mut Scene, at: Vec2) {
    s.cuboid(vec3(at.x + 0.3, 0.0, at.y), vec3(0.3, 0.85, 0.5), WHITE);
    s.frustum(
        vec3(at.x - 0.05, 0.0, at.y),
        0.18,
        0.26,
        0.45,
        10,
        rgb(245, 245, 250),
        rgb(200, 220, 240),
    );
}

pub fn roll(s: &mut Scene, at: Vec3) {
    s.cylinder(at, 0.12, 0.2, 8, rgb(250, 250, 250), rgb(225, 225, 235));
}

pub fn burger(s: &mut Scene, at: Vec3) {
    s.cylinder(at, 0.16, 0.05, 8, rgb(220, 150, 70), rgb(230, 170, 90));
    s.cylinder(
        at + vec3(0.0, 0.05, 0.0),
        0.17,
        0.05,
        8,
        rgb(110, 60, 40),
        rgb(120, 70, 45),
    );
    s.cylinder(
        at + vec3(0.0, 0.1, 0.0),
        0.17,
        0.02,
        8,
        rgb(110, 200, 80),
        rgb(120, 210, 90),
    );
    s.sphere(
        at + vec3(0.0, 0.14, 0.0),
        vec3(0.17, 0.09, 0.17),
        4,
        rgb(240, 170, 80),
    );
}

pub fn towel(s: &mut Scene, at: Vec3) {
    s.cuboid2(
        at,
        vec3(0.36, 0.1, 0.26),
        rgb(120, 190, 250),
        rgb(160, 210, 255),
    );
}

pub fn coin_stack(s: &mut Scene, at: Vec2, amount: u32, t: f32) {
    let n = (amount as f32).sqrt().ceil().clamp(1.0, 10.0) as usize;
    for i in 0..n {
        let col = i % 3;
        let row = i / 3;
        let o = vec2(col as f32 * 0.32 - 0.32, (row % 2) as f32 * 0.3 - 0.15);
        let h = 0.1 + ((i * 7) % 5) as f32 * 0.04;
        s.cylinder(
            vec3(at.x + o.x, row as f32 / 2.0 * 0.0, at.y + o.y),
            0.15,
            h,
            10,
            rgb(240, 180, 30),
            rgb(255, 220, 70),
        );
    }
    let bob = (t * 4.0).sin() * 0.06;
    s.cylinder(
        vec3(at.x, 0.6 + bob, at.y),
        0.12,
        0.04,
        10,
        rgb(255, 210, 60),
        rgb(255, 235, 120),
    );
}

pub fn shelf(s: &mut Scene, at: Vec2, item_col: Color) {
    s.cuboid2(
        vec3(at.x, 0.0, at.y),
        vec3(0.7, 0.75, 0.6),
        rgb(170, 120, 80),
        rgb(200, 150, 100),
    );
    for k in [-0.15, 0.15] {
        s.cuboid(vec3(at.x + k, 0.75, at.y), vec3(0.25, 0.2, 0.25), item_col);
    }
}

pub fn umbrella(s: &mut Scene, at: Vec2, c: Color) {
    s.cylinder(vec3(at.x, 0.0, at.y), 0.04, 1.6, 6, WHITE, WHITE);
    s.frustum(vec3(at.x, 1.5, at.y), 0.75, 0.05, 0.3, 10, c, c);
}

pub fn swimmer(s: &mut Scene, at: Vec2, water: f32, face: f32, t: f32, look: &Look) {
    let outer = s.xf;
    s.xf = Xf {
        origin: vec3(at.x, water, at.y),
        yaw: face,
        scale: Vec3::splat(0.85),
    };
    let stroke = t * 5.5;
    s.sphere(vec3(0.0, 0.02, 0.05), vec3(0.28, 0.14, 0.42), 4, look.body);
    for side in [-1.0f32, 1.0] {
        let a = stroke
            + if side > 0.0 {
                0.0
            } else {
                std::f32::consts::PI
            };
        let up = a.sin();
        let reach = a.cos();
        s.sphere(
            vec3(0.36 * side, 0.08 + up.max(0.0) * 0.35, 0.25 + reach * 0.35),
            vec3(0.08, 0.08, 0.2),
            3,
            look.skin,
        );
        let kick = (t * 9.0 + side).sin() * 0.06;
        s.sphere(
            vec3(0.13 * side, kick, -0.5),
            vec3(0.08, 0.06, 0.2),
            3,
            look.legs,
        );
    }
    let head = vec3(0.0, 0.18, 0.5);
    let turn = (stroke * 0.5).sin() * 0.12;
    s.sphere(
        head + vec3(turn, 0.0, 0.0),
        vec3(0.26, 0.25, 0.25),
        5,
        look.skin,
    );
    s.sphere(
        head + vec3(turn, 0.1, -0.04),
        vec3(0.27, 0.18, 0.24),
        4,
        rgb(240, 70, 80),
    );
    for side in [-1.0f32, 1.0] {
        s.sphere(
            head + vec3(turn + 0.09 * side, 0.03, 0.21),
            vec3(0.07, 0.05, 0.03),
            3,
            rgb(60, 180, 230),
        );
    }
    s.xf = outer;
}

pub fn wake(s: &mut Scene, at: Vec2, water: f32, face: f32, t: f32) {
    let back = vec2(-face.sin(), -face.cos());
    for k in 0..4 {
        let d = 0.35 + k as f32 * 0.32;
        let r = 0.12 + k as f32 * 0.07 + ((t * 6.0 + k as f32).sin() * 0.02);
        let p = at + back * d;
        s.disc(
            p,
            vec2(r, r * 0.8),
            water + 0.004,
            Color::new(1.0, 1.0, 1.0, 0.55 - k as f32 * 0.12),
        );
    }
}

pub fn rug(s: &mut Scene, at: Vec2, c: Color) {
    s.disc(at, vec2(0.85, 0.6), 0.008, c);
    s.disc(at, vec2(0.6, 0.4), 0.01, darker(c, 1.15));
}

pub fn bin(s: &mut Scene, at: Vec2, full: bool) {
    s.frustum(
        vec3(at.x, 0.0, at.y),
        0.18,
        0.22,
        0.45,
        10,
        rgb(120, 130, 150),
        rgb(70, 75, 90),
    );
    if full {
        for (i, o) in [vec2(0.0, 0.0), vec2(0.08, 0.06), vec2(-0.07, 0.05)]
            .iter()
            .enumerate()
        {
            s.sphere(
                vec3(at.x + o.x, 0.5 + i as f32 * 0.05, at.y + o.y),
                vec3(0.1, 0.09, 0.1),
                3,
                rgb(245, 245, 235),
            );
        }
    }
}

pub fn wardrobe(s: &mut Scene, at: Vec2, wood: Color, yaw: f32) {
    let outer = s.xf;
    s.xf = outer.then(Xf::at(vec3(at.x, 0.0, at.y), yaw, 1.0));
    s.cuboid2(
        vec3(0.0, 0.0, 0.0),
        vec3(1.1, 1.7, 0.55),
        wood,
        darker(wood, 1.1),
    );
    s.cuboid(
        vec3(0.0, 0.15, 0.28),
        vec3(0.02, 1.4, 0.02),
        darker(wood, 0.7),
    );
    for x in [-0.12, 0.12] {
        s.sphere(
            vec3(x, 0.9, 0.3),
            vec3(0.04, 0.04, 0.04),
            2,
            rgb(250, 210, 80),
        );
    }
    s.xf = outer;
}

pub fn tv(s: &mut Scene, at: Vec2, yaw: f32) {
    let outer = s.xf;
    s.xf = outer.then(Xf::at(vec3(at.x, 0.0, at.y), yaw, 1.0));
    s.cuboid2(
        vec3(0.0, 0.0, 0.0),
        vec3(1.0, 0.45, 0.45),
        rgb(150, 100, 70),
        rgb(180, 125, 85),
    );
    s.cuboid(
        vec3(0.0, 0.45, -0.05),
        vec3(0.9, 0.55, 0.06),
        rgb(40, 40, 55),
    );
    s.cuboid(
        vec3(0.0, 0.5, -0.01),
        vec3(0.8, 0.45, 0.02),
        rgb(80, 200, 240),
    );
    s.xf = outer;
}

pub fn lamp(s: &mut Scene, at: Vec2, c: Color) {
    s.cylinder(
        vec3(at.x, 0.0, at.y),
        0.18,
        0.5,
        8,
        rgb(150, 100, 70),
        rgb(180, 125, 85),
    );
    s.cylinder(vec3(at.x, 0.5, at.y), 0.03, 0.3, 5, WHITE, WHITE);
    s.frustum(
        vec3(at.x, 0.75, at.y),
        0.22,
        0.13,
        0.25,
        10,
        c,
        darker(c, 1.1),
    );
}

pub fn dust(s: &mut Scene, at: Vec2, t: f32) {
    for k in 0..3 {
        let a = t * 1.5 + k as f32 * 2.1;
        let p = at + vec2(a.cos(), a.sin()) * 0.25;
        s.disc(
            p,
            vec2(0.14, 0.11),
            0.015,
            Color::new(0.45, 0.32, 0.22, 0.75),
        );
    }
}

pub fn locked_door(s: &mut Scene, r: Rect, theme: &Theme) {
    let c = vec3(r.x + r.w / 2.0, 0.0, r.y + r.h / 2.0);
    let w = r.w.max(0.24);
    let d = r.h.max(0.24);
    s.cuboid2(
        c,
        vec3(w, 1.15, d),
        darker(theme.trim, 0.75),
        darker(theme.trim, 0.9),
    );
}

pub fn car(s: &mut Scene, at: Vec2, c: Color) {
    s.cuboid2(
        vec3(at.x, 0.15, at.y),
        vec3(1.4, 0.45, 2.4),
        c,
        darker(c, 1.1),
    );
    s.cuboid2(
        vec3(at.x, 0.6, at.y + 0.1),
        vec3(1.2, 0.4, 1.3),
        rgb(180, 220, 250),
        c,
    );
    for (x, z) in [(-0.6, -0.8), (0.6, -0.8), (-0.6, 0.8), (0.6, 0.8)] {
        s.sphere(
            vec3(at.x + x, 0.18, at.y + z),
            vec3(0.12, 0.18, 0.18),
            3,
            rgb(40, 40, 50),
        );
    }
}
