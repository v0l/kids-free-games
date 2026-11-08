use super::build::{Anim, Model, Tint, Xf, LAYER};
use crate::gcad::Shelf;
use bevy::math::{Quat, Vec3};
use bevy::prelude::Transform;
use glam::{vec2, vec3, Vec2};
use kg_core::games::hotel::layout::Theme;
use kg_core::geom::{Color, Rect};

pub static PEOPLE: &[&str] = &[
    "hotel_person",
    "hotel_person_up",
    "hotel_person_sit",
    "hotel_person_sit_up",
    "hotel_person_lie",
];

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

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Stance {
    pub sit: bool,
    pub lie: bool,
    pub arms_up: bool,
    pub shoes: Option<Color>,
}

fn side_of(part: &str) -> f32 {
    if part.ends_with("_r") {
        -1.0
    } else {
        1.0
    }
}

fn person_model(st: Stance) -> &'static str {
    match (st.lie, st.sit, st.arms_up) {
        (true, ..) => "hotel_person_lie",
        (false, true, true) => "hotel_person_sit_up",
        (false, true, false) => "hotel_person_sit",
        (false, false, true) => "hotel_person_up",
        (false, false, false) => "hotel_person",
    }
}

pub fn body(s: &mut Model, shelf: &Shelf, look: &Look, st: Stance) {
    let w = look.wide;
    let swings = !st.sit && !st.lie;
    s.model(shelf, person_model(st), Transform::default(), |part| {
        let side = side_of(part);
        let base = part.trim_end_matches("_l").trim_end_matches("_r");
        let arm = |c: Color| Tint {
            color: Some(c),
            anim: (!st.arms_up).then_some(Anim::Arm(side)),
            at: Some(Transform::from_xyz(0.33 * (w - 1.0) * side, 0.0, 0.0)),
            ..Default::default()
        };
        let leg = |c: Color| Tint {
            color: Some(c),
            anim: swings.then_some(Anim::Leg(side)),
            ..Default::default()
        };
        match base {
            "leg" | "thigh" | "shin" => leg(look.legs),
            "shoe" if st.shoes.is_some() => Tint::hidden(),
            "boot" => match st.shoes {
                Some(c) => leg(c),
                None => Tint::hidden(),
            },
            "shoe" => leg(rgb(60, 40, 35)),
            "arm" => arm(look.body),
            "hand" => arm(look.skin),
            "torso" => Tint {
                color: Some(look.body),
                at: Some(Transform::from_scale(vec3(w, 1.0, w))),
                ..Default::default()
            },
            "head" => Tint::color(look.skin),
            "hair" => Tint::color(look.hair),
            "hat" => look.hat.map_or(Tint::hidden(), Tint::color),
            "brim" => look
                .hat
                .map_or(Tint::hidden(), |h| Tint::color(darker(h, 0.85))),
            _ => Tint::default(),
        }
    });
}

pub fn swimmer(s: &mut Model, shelf: &Shelf, look: &Look) {
    s.model(shelf, "hotel_swimmer", Transform::default(), |part| {
        let side = side_of(part);
        match part.trim_end_matches("_l").trim_end_matches("_r") {
            "torso" => Tint::color(look.body),
            "arm" => Tint {
                color: Some(look.skin),
                anim: Some(Anim::Stroke(side)),
                ..Default::default()
            },
            "leg" => Tint {
                color: Some(look.legs),
                anim: Some(Anim::Kick(side)),
                ..Default::default()
            },
            "head" => Tint {
                color: Some(look.skin),
                anim: Some(Anim::Turn),
                ..Default::default()
            },
            _ => Tint {
                anim: Some(Anim::Turn),
                ..Default::default()
            },
        }
    });
}

pub fn stroke(side: f32, t: f32) -> Vec3 {
    let a = t * 5.5
        + if side > 0.0 {
            0.0
        } else {
            std::f32::consts::PI
        };
    vec3(
        0.36 * side,
        0.08 + a.sin().max(0.0) * 0.35,
        0.25 + a.cos() * 0.35,
    )
}

pub fn kick(side: f32, t: f32) -> Vec3 {
    vec3(0.13 * side, (t * 9.0 + side).sin() * 0.06, -0.5)
}

pub fn head_turn(t: f32) -> f32 {
    (t * 5.5 * 0.5).sin() * 0.12
}

pub const WAKE: usize = 4;

pub fn wake(s: &mut Model, water: f32) {
    for k in 0..WAKE {
        s.disc(
            vec2(0.0, -(0.35 + k as f32 * 0.32)),
            vec2(0.15, 0.12),
            water + LAYER + k as f32 * 0.002,
            Color::new(1.0, 1.0, 1.0, 0.55 - k as f32 * 0.12),
        );
        s.tag(Anim::Wake(k));
    }
}

pub fn wake_radius(k: usize, t: f32) -> f32 {
    0.12 + k as f32 * 0.07 + (t * 6.0 + k as f32).sin() * 0.02
}

fn at(p: Vec2) -> Vec3 {
    vec3(p.x, 0.0, p.y)
}

fn placed(p: Vec3, yaw: f32) -> Transform {
    Transform::from_translation(p).with_rotation(Quat::from_rotation_y(yaw))
}

pub fn desk(s: &mut Model, shelf: &Shelf, r: Rect, theme: &Theme) {
    let c = vec2(r.x + r.w / 2.0, r.y + r.h / 2.0);
    s.model(shelf, "hotel_desk", placed(at(c), 0.0), |part| match part {
        "top" => Tint::color(theme.trim),
        "panel" => Tint::color(darker(theme.trim, 0.9)),
        _ => Tint::default(),
    });
}

pub fn bed(s: &mut Model, shelf: &Shelf, sheet: Color, wood: Color, fancy: bool) {
    let name = if fancy {
        "hotel_bed_fancy"
    } else {
        "hotel_bed"
    };
    s.model(shelf, name, Transform::default(), |part| match part {
        "frame" | "headboard" => Tint::color(wood),
        "frame_top" => Tint::color(darker(wood, 1.1)),
        "sheet" => Tint::color(sheet),
        "sheet_top" => Tint::color(darker(sheet, 1.05)),
        _ => Tint::default(),
    });
}

pub fn plant(s: &mut Model, shelf: &Shelf, p: Vec2, h: f32) {
    let name = if h > 1.4 {
        "hotel_plant_tall"
    } else if h > 1.1 {
        "hotel_plant_mid"
    } else {
        "hotel_plant"
    };
    s.prop(shelf, name, at(p), 0.0);
}

pub fn sofa(s: &mut Model, shelf: &Shelf, p: Vec2, c: Color, yaw: f32) {
    s.model(shelf, "hotel_sofa", placed(at(p), yaw), |part| match part {
        "seat" => Tint::color(c),
        _ => Tint::color(darker(c, 0.9)),
    });
}

pub fn burger(s: &mut Model, shelf: &Shelf, p: Vec3) {
    s.prop(shelf, "hotel_burger", p, 0.0);
}

pub fn roll(s: &mut Model, shelf: &Shelf, p: Vec3) {
    s.prop(shelf, "hotel_roll", p, 0.0);
}

pub fn towel(s: &mut Model, shelf: &Shelf, p: Vec3) {
    s.prop(shelf, "hotel_towel", p, 0.0);
}

pub fn coin_stack(s: &mut Model, shelf: &Shelf, p: Vec2, amount: u32) {
    let n = (amount as f32).sqrt().ceil().clamp(1.0, 10.0) as usize;
    for i in 0..n {
        let col = i % 3;
        let row = i / 3;
        let o = vec2(col as f32 * 0.32 - 0.32, (row % 2) as f32 * 0.3 - 0.15);
        let h = 0.1 + ((i * 7) % 5) as f32 * 0.04;
        s.model(
            shelf,
            "hotel_coin",
            Transform::from_translation(at(p + o)).with_scale(vec3(1.0, h, 1.0)),
            |_| Tint::default(),
        );
    }
    s.model(
        shelf,
        "hotel_coin_top",
        Transform::from_translation(at(p) + vec3(0.0, 0.6, 0.0)),
        |_| Tint {
            anim: Some(Anim::Bob),
            ..Default::default()
        },
    );
}

pub fn coin_bob(t: f32) -> f32 {
    (t * 4.0).sin() * 0.06
}

pub fn shelf(s: &mut Model, shelf: &Shelf, p: Vec2, item_col: Color) {
    s.model(shelf, "hotel_shelf", placed(at(p), 0.0), |part| {
        if part.starts_with("item") {
            Tint::color(item_col)
        } else {
            Tint::default()
        }
    });
}

pub fn umbrella(s: &mut Model, shelf: &Shelf, p: Vec2, c: Color) {
    s.model(
        shelf,
        "hotel_umbrella",
        placed(at(p), 0.0),
        |part| match part {
            "canopy" => Tint::color(c),
            _ => Tint::default(),
        },
    );
}

pub fn rug(s: &mut Model, shelf: &Shelf, p: Vec2, c: Color) {
    s.model(shelf, "hotel_rug", placed(at(p), 0.0), |part| match part {
        "rug" => Tint::color(c),
        _ => Tint::color(darker(c, 1.15)),
    });
}

pub fn bin(s: &mut Model, shelf: &Shelf, p: Vec2, full: bool) {
    let name = if full { "hotel_bin_full" } else { "hotel_bin" };
    s.prop(shelf, name, at(p), 0.0);
}

pub fn wardrobe(s: &mut Model, shelf: &Shelf, p: Vec2, wood: Color, yaw: f32) {
    s.model(
        shelf,
        "hotel_wardrobe",
        placed(at(p), yaw),
        |part| match part {
            "wardrobe" => Tint::color(wood),
            "wardrobe_top" => Tint::color(darker(wood, 1.1)),
            "seam" => Tint::color(darker(wood, 0.7)),
            _ => Tint::default(),
        },
    );
}

pub fn lamp(s: &mut Model, shelf: &Shelf, p: Vec2, c: Color) {
    s.model(shelf, "hotel_lamp", placed(at(p), 0.0), |part| match part {
        "shade" => Tint::color(c),
        _ => Tint::default(),
    });
}

pub fn dust(s: &mut Model, p: Vec2) {
    for k in 0..3 {
        s.disc(
            p,
            vec2(0.14, 0.11),
            0.015,
            Color::new(0.45, 0.32, 0.22, 0.75),
        );
        s.tag(Anim::Orbit(p, k as f32 * 2.1));
    }
}

pub fn orbit(at: Vec2, phase: f32, t: f32) -> Vec2 {
    let a = t * 1.5 + phase;
    at + vec2(a.cos(), a.sin()) * 0.25
}

pub fn locked_door(s: &mut Model, r: Rect, theme: &Theme) {
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

pub fn car(s: &mut Model, shelf: &Shelf, p: Vec2, c: Color) {
    s.model(shelf, "hotel_car", placed(at(p), 0.0), |part| match part {
        "body" | "roof" => Tint::color(c),
        "body_top" => Tint::color(darker(c, 1.1)),
        _ => Tint::default(),
    });
}

pub fn tv(s: &mut Model, shelf: &Shelf, p: Vec2, yaw: f32) {
    s.prop(shelf, "hotel_tv", at(p), yaw);
}

pub fn toilet(s: &mut Model, shelf: &Shelf, p: Vec2, yaw: f32) {
    s.prop(shelf, "hotel_toilet", at(p), yaw);
}

pub fn entrance(s: &mut Model, shelf: &Shelf, p: Vec3, trim: Color) {
    s.model(shelf, "hotel_entrance", placed(p, 0.0), |part| {
        if part.starts_with("cap") || part == "lintel_cap" {
            Tint::color(darker(trim, 1.1))
        } else {
            Tint::color(trim)
        }
    });
}

pub fn yawed(s: &mut Model, p: Vec2, yaw: f32, scale: f32, f: impl FnOnce(&mut Model)) {
    let outer = s.xf;
    s.xf = outer.then(Xf::at(at(p), yaw, scale));
    f(s);
    s.xf = outer;
}
