use super::{TrainsPlay, SHEET};
use crate::game3d::*;
use crate::games::hotel::build::Model;
use crate::games::hotel::models::{body, guest_look, rgb, Stance};
use bevy::ecs::hierarchy::ChildSpawnerCommands;
use bevy::prelude::*;
use kg_core::engine::View;
use kg_core::games::trains::*;
use kg_core::geom::{Color as Col, WHITE};
use std::f32::consts::{FRAC_PI_2, TAU};

const SKY: Col = Col::new(0.62, 0.84, 0.98, 1.0);
const SKY_DEPTH: f32 = 220.0;
const SMOKE: usize = 5;
const ALPHA_STEPS: f32 = 8.0;
const RAIL_Y: f32 = 0.1;
const CHIMNEY_X: f32 = 0.95;
const CHIMNEY_Y: f32 = 1.95;

pub struct TrainsWorld;

impl Plugin for TrainsWorld {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                spawn.run_if(opened(World3d::Trains)),
                sync.run_if(is_open(World3d::Trains)),
            )
                .chain()
                .in_set(Sync3d),
        );
    }
}

#[derive(Component)]
pub struct Root {
    carriages: usize,
    seed: u32,
    riders: usize,
}

#[derive(Component, Clone, Copy, PartialEq, Debug)]
pub enum Role {
    Sky,
    Train,
    Wheel(f32),
    Smoke(usize),
    Rider(usize),
}

#[derive(Component, Clone, Copy, PartialEq, Eq, Debug)]
pub struct Stand {
    pub look: u8,
    pub arms_up: bool,
}

fn view_of(pen: &crate::paint::Pen) -> View {
    pen.view.unwrap_or_else(View::ipad_landscape)
}

fn at(x: f32, y: f32, z: f32) -> Vec3 {
    Vec3::new(x, y, z)
}

fn cube(center: Vec3, size: Vec3) -> Transform {
    Transform::from_translation(center).with_scale(size)
}

fn along_x(center: Vec3, len: f32, dia: f32) -> Transform {
    Transform::from_translation(center)
        .with_rotation(Quat::from_rotation_z(FRAC_PI_2))
        .with_scale(Vec3::new(dia, len, dia))
}

fn put(
    out: &mut ChildSpawnerCommands,
    kit: &mut Kit,
    shape: Shape,
    finish: Finish,
    color: Col,
    t: Transform,
) -> Entity {
    out.spawn(kit.part(shape, Paint::new(finish, color), t)).id()
}

fn tiled(
    out: &mut ChildSpawnerCommands,
    kit: &mut Kit,
    name: &'static str,
    size: Vec2,
    tile: f32,
    t: Transform,
) {
    let repeat = Vec2::new(size.x / tile, size.y / tile);
    let tint = Col::new(0.72, 0.72, 0.72, 1.0);
    let paint = Paint::skinned(Finish::Floor, tint, Skin::Tile(SHEET, name, repeat));
    out.spawn(kit.part(Shape::Plane, paint, t.with_scale(Vec3::new(size.x, 1.0, size.y))));
}

fn spawn(mut commands: Commands, mut kit: Kit) {
    let world = spawn_world(
        &mut commands,
        Root {
            carriages: 0,
            seed: 0,
            riders: 0,
        },
        World3d::Trains,
        Lighting {
            ambient: 650.0,
            illuminance: 9500.0,
            color: Col::new(1.0, 0.97, 0.9, 1.0),
            toward: Vec3::new(-0.5, -1.0, -0.65),
        },
    );
    commands.entity(world.eye).with_children(|c| {
        let paint = Paint::skinned(Finish::Backdrop, WHITE, Skin::Sprite(SHEET, "bg", [0.0, 0.0, 1.0, 1.0]));
        c.spawn((
            kit.part(Shape::Quad, paint, Transform::default()),
            Role::Sky,
        ));
    });
    commands.entity(world.root).with_children(|w| {
        scenery(w, &mut kit);
        w.spawn((Role::Train, Transform::default(), Visibility::default()));
        for k in 0..SMOKE {
            w.spawn((
                kit.part(
                    Shape::Sphere,
                    Paint::new(Finish::Ghost, Col::new(0.97, 0.97, 1.0, 0.5)),
                    Transform::default(),
                ),
                Role::Smoke(k),
            ));
        }
    });
}

fn scenery(w: &mut ChildSpawnerCommands, kit: &mut Kit) {
    let grass = Col::new(0.45, 0.78, 0.3, 1.0);
    put(w, kit, Shape::Plane, Finish::Floor, grass, cube(at(0.0, -0.02, -20.0), at(300.0, 1.0, 120.0)));
    tiled(w, kit, "grass", Vec2::new(160.0, 70.0), 3.5, Transform::from_xyz(0.0, 0.0, -22.0));
    put(w, kit, Shape::Cube, Finish::Matte, rgb(150, 135, 120), cube(at(0.0, 0.05, 0.0), at(160.0, 0.1, 2.8)));
    tiled(w, kit, "gravel", Vec2::new(160.0, 2.8), 1.4, Transform::from_xyz(0.0, 0.102, 0.0));
    for side in [-1.0f32, 1.0] {
        put(w, kit, Shape::Cube, Finish::Gloss, rgb(185, 190, 200), cube(at(0.0, RAIL_Y + 0.07, 0.55 * side), at(160.0, 0.1, 0.09)));
    }
    let mut x = -44.0;
    while x < 44.0 {
        put(w, kit, Shape::Cube, Finish::Matte, rgb(135, 90, 55), cube(at(x, RAIL_Y + 0.02, 0.0), at(0.28, 0.07, 1.5)));
        x += 0.8;
    }
    platform(w, kit);
    station(w, kit);
    trees(w, kit);
    hills(w, kit);
}

fn platform(w: &mut ChildSpawnerCommands, kit: &mut Kit) {
    let (z0, z1) = (2.2, 8.5);
    put(w, kit, Shape::Cube, Finish::Matte, rgb(225, 205, 170), cube(at(0.0, PLATFORM_Y / 2.0, (z0 + z1) / 2.0), at(60.0, PLATFORM_Y, z1 - z0)));
    tiled(w, kit, "platform", Vec2::new(60.0, z1 - z0), 2.0, Transform::from_xyz(0.0, PLATFORM_Y + 0.003, (z0 + z1) / 2.0));
    put(w, kit, Shape::Cube, Finish::Matte, rgb(255, 205, 40), cube(at(0.0, PLATFORM_Y + 0.012, z0 + 0.18), at(60.0, 0.025, 0.22)));
    for x in [-7.0f32, 7.5] {
        lamp(w, kit, at(x, PLATFORM_Y, 7.2));
    }
    bench(w, kit, at(-3.2, PLATFORM_Y, 7.6));
    sign(w, kit, at(4.4, PLATFORM_Y, 7.4));
}

fn lamp(w: &mut ChildSpawnerCommands, kit: &mut Kit, base: Vec3) {
    put(w, kit, Shape::Cylinder(10), Finish::Gloss, rgb(50, 70, 110), cube(base + at(0.0, 1.5, 0.0), at(0.14, 3.0, 0.14)));
    put(w, kit, Shape::Sphere, Finish::Glow, Col::new(1.0, 0.93, 0.6, 0.95), cube(base + at(0.0, 3.1, 0.0), at(0.5, 0.5, 0.5)));
}

fn bench(w: &mut ChildSpawnerCommands, kit: &mut Kit, base: Vec3) {
    put(w, kit, Shape::Cube, Finish::Matte, rgb(165, 105, 60), cube(base + at(0.0, 0.5, 0.0), at(1.8, 0.1, 0.5)));
    put(w, kit, Shape::Cube, Finish::Matte, rgb(165, 105, 60), cube(base + at(0.0, 0.85, 0.22), at(1.8, 0.35, 0.08)));
    for s in [-0.75f32, 0.75] {
        put(w, kit, Shape::Cube, Finish::Matte, rgb(70, 80, 100), cube(base + at(s, 0.25, 0.0), at(0.1, 0.5, 0.45)));
    }
}

fn sign(w: &mut ChildSpawnerCommands, kit: &mut Kit, base: Vec3) {
    put(w, kit, Shape::Cylinder(8), Finish::Matte, rgb(80, 85, 100), cube(base + at(0.0, 1.0, 0.0), at(0.12, 2.0, 0.12)));
    put(w, kit, Shape::Cube, Finish::Gloss, rgb(40, 110, 200), cube(base + at(0.0, 2.1, 0.0), at(1.6, 0.7, 0.1)));
    put(w, kit, Shape::Cube, Finish::Matte, WHITE, cube(base + at(0.0, 2.1, 0.06), at(1.4, 0.5, 0.03)));
}

fn station(w: &mut ChildSpawnerCommands, kit: &mut Kit) {
    let base = at(-8.0, 0.0, -5.5);
    put(w, kit, Shape::Cube, Finish::Matte, rgb(250, 232, 190), cube(base + at(0.0, 1.3, 0.0), at(6.0, 2.6, 3.4)));
    put(w, kit, Shape::Cube, Finish::Matte, rgb(215, 90, 70), cube(base + at(0.0, 2.7, 0.0), at(6.6, 0.2, 4.0)));
    put(
        w,
        kit,
        Shape::Cone(4),
        Finish::Matte,
        rgb(205, 70, 60),
        Transform::from_translation(base + at(0.0, 3.5, 0.0))
            .with_rotation(Quat::from_rotation_y(FRAC_PI_2 / 2.0))
            .with_scale(Vec3::new(9.2, 1.5, 5.6)),
    );
    for dx in [-1.8f32, 0.0, 1.8] {
        put(w, kit, Shape::Cube, Finish::Gloss, rgb(130, 200, 240), cube(base + at(dx, 1.5, 1.72), at(0.8, 0.9, 0.06)));
        put(w, kit, Shape::Cube, Finish::Matte, WHITE, cube(base + at(dx, 1.5, 1.70), at(1.0, 1.1, 0.04)));
    }
    put(w, kit, Shape::Cube, Finish::Matte, rgb(150, 95, 60), cube(base + at(2.5, 0.8, 1.72), at(0.9, 1.6, 0.06)));
    put(w, kit, Shape::Cylinder(12), Finish::Gloss, rgb(255, 210, 60), cube(base + at(0.0, 5.1, 0.0), at(1.0, 0.18, 1.0)));
    put(w, kit, Shape::Cube, Finish::Matte, rgb(120, 125, 140), cube(base + at(2.3, 3.8, -0.6), at(0.7, 1.6, 0.7)));
}

fn trees(w: &mut ChildSpawnerCommands, kit: &mut Kit) {
    let greens = [
        rgb(70, 170, 70),
        rgb(95, 190, 80),
        rgb(55, 150, 85),
        rgb(120, 200, 80),
    ];
    for i in 0..26 {
        let k = i as f32;
        let x = -30.0 + k * 2.4 + (k * 1.7).sin() * 1.1;
        let z = -9.0 - ((k * 2.3).sin() * 0.5 + 0.5) * 8.0 - (i % 3) as f32 * 3.0;
        if (x + 8.0).abs() < 4.5 && z > -9.0 {
            continue;
        }
        let s = 1.0 + ((k * 0.9).cos() * 0.5 + 0.5) * 0.9;
        let c = greens[i % greens.len()];
        put(w, kit, Shape::Cylinder(8), Finish::Matte, rgb(130, 90, 55), cube(at(x, 0.7 * s, z), at(0.4 * s, 1.4 * s, 0.4 * s)));
        if i % 2 == 0 {
            put(w, kit, Shape::Sphere, Finish::Gloss, c, cube(at(x, 2.3 * s, z), at(2.2 * s, 2.2 * s, 2.2 * s)));
        } else {
            put(w, kit, Shape::Cone(12), Finish::Gloss, c, cube(at(x, 2.6 * s, z), at(2.0 * s, 3.2 * s, 2.0 * s)));
        }
    }
}

fn hills(w: &mut ChildSpawnerCommands, kit: &mut Kit) {
    let tones = [rgb(110, 200, 110), rgb(90, 185, 120), rgb(130, 210, 100)];
    for i in 0..7 {
        let k = i as f32;
        let x = -48.0 + k * 16.0;
        put(
            w,
            kit,
            Shape::Ball,
            Finish::Matte,
            tones[i % 3],
            cube(at(x, -2.0, -38.0 - (i % 2) as f32 * 7.0), at(32.0, 14.0 + (i % 3) as f32 * 4.0, 20.0)),
        );
    }
}

fn train_parts(t: &mut ChildSpawnerCommands, kit: &mut Kit, cars: usize) {
    let len = ENGINE_LEN + cars as f32 * CARRIAGE_LEN;
    let ex = len / 2.0 - ENGINE_LEN / 2.0;
    let red = rgb(232, 62, 58);
    let dark = rgb(60, 62, 80);
    put(t, kit, Shape::Cube, Finish::Matte, dark, cube(at(ex, 0.38, 0.0), at(ENGINE_LEN, 0.22, 1.1)));
    put(t, kit, Shape::Cylinder(24), Finish::Gloss, red, along_x(at(ex + 0.45, 0.98, 0.0), 1.9, 1.05));
    put(t, kit, Shape::Cylinder(24), Finish::Gloss, rgb(255, 205, 50), along_x(at(ex - 0.5, 0.98, 0.0), 0.09, 1.1));
    put(t, kit, Shape::Cylinder(24), Finish::Gloss, rgb(255, 205, 50), along_x(at(ex + 0.98, 0.98, 0.0), 0.09, 1.1));
    put(t, kit, Shape::Cylinder(24), Finish::Gloss, rgb(190, 40, 45), along_x(at(ex + 1.45, 0.98, 0.0), 0.2, 1.0));
    put(t, kit, Shape::Sphere, Finish::Glow, Col::new(1.0, 0.95, 0.6, 1.0), cube(at(ex + 1.58, 1.2, 0.0), at(0.32, 0.32, 0.32)));
    for side in [-1.0f32, 1.0] {
        put(t, kit, Shape::Sphere, Finish::Gloss, WHITE, cube(at(ex + 1.5, 0.86, 0.24 * side), at(0.28, 0.34, 0.2)));
        put(t, kit, Shape::Sphere, Finish::Gloss, rgb(30, 30, 50), cube(at(ex + 1.6, 0.84, 0.25 * side), at(0.12, 0.18, 0.1)));
    }
    put(t, kit, Shape::Cube, Finish::Matte, rgb(120, 25, 40), cube(at(ex + 1.57, 0.55, 0.0), at(0.08, 0.07, 0.34)));
    put(t, kit, Shape::Cylinder(14), Finish::Gloss, rgb(40, 40, 60), cube(at(ex + CHIMNEY_X - 0.1, 1.72, 0.0), at(0.34, 0.5, 0.34)));
    put(t, kit, Shape::Cylinder(14), Finish::Gloss, rgb(255, 205, 50), cube(at(ex + CHIMNEY_X - 0.1, 1.98, 0.0), at(0.52, 0.1, 0.52)));
    put(t, kit, Shape::Sphere, Finish::Gloss, rgb(255, 205, 50), cube(at(ex + 0.1, 1.55, 0.0), at(0.5, 0.42, 0.5)));
    put(t, kit, Shape::Cube, Finish::Matte, rgb(235, 90, 70), cube(at(ex - 1.0, 1.0, 0.0), at(1.0, 1.25, 1.12)));
    put(t, kit, Shape::Cube, Finish::Matte, rgb(60, 80, 160), cube(at(ex - 1.0, 1.7, 0.0), at(1.3, 0.14, 1.4)));
    for side in [-1.0f32, 1.0] {
        put(t, kit, Shape::Cube, Finish::Gloss, rgb(140, 210, 245), cube(at(ex - 1.0, 1.2, 0.575 * side), at(0.5, 0.5, 0.04)));
    }
    put(t, kit, Shape::Cube, Finish::Matte, rgb(255, 205, 50), cube(at(ex + 1.72, 0.3, 0.0), at(0.4, 0.3, 1.2)));
    for s in 0..4 {
        put(t, kit, Shape::Cube, Finish::Matte, rgb(60, 62, 80), cube(at(ex + 1.93, 0.3, (s as f32 - 1.5) * 0.28), at(0.06, 0.3, 0.08)));
    }
    for (dx, r) in [(-0.95, 0.4), (-0.15, 0.4), (1.05, 0.28)] {
        for side in [-1.0f32, 1.0] {
            wheel(t, kit, at(ex + dx, RAIL_Y + r, 0.66 * side), r);
        }
    }
    let palette = [rgb(255, 200, 50), rgb(70, 160, 240), rgb(95, 200, 100), rgb(240, 120, 190)];
    for c in 0..cars {
        let cx = ex - ENGINE_LEN / 2.0 - CARRIAGE_LEN * (c as f32 + 0.5);
        carriage(t, kit, cx, palette[c % palette.len()]);
        let hitch = cx + CARRIAGE_LEN / 2.0;
        put(t, kit, Shape::Cube, Finish::Matte, dark, cube(at(hitch, 0.45, 0.0), at(0.25, 0.08, 0.12)));
    }
}

fn carriage(t: &mut ChildSpawnerCommands, kit: &mut Kit, cx: f32, color: Col) {
    let l = CARRIAGE_LEN - 0.2;
    let floor_top = FLOOR_Y;
    put(t, kit, Shape::Cube, Finish::Matte, rgb(60, 62, 80), cube(at(cx, floor_top - 0.12, 0.0), at(l, 0.24, 1.1)));
    for side in [-1.0f32, 1.0] {
        put(t, kit, Shape::Cube, Finish::Gloss, color, cube(at(cx, floor_top + 0.3, 0.54 * side), at(l, 0.6, 0.1)));
        put(t, kit, Shape::Cube, Finish::Matte, WHITE, cube(at(cx, floor_top + 0.62, 0.54 * side), at(l + 0.04, 0.07, 0.14)));
    }
    for end in [-1.0f32, 1.0] {
        put(t, kit, Shape::Cube, Finish::Gloss, color, cube(at(cx + (l / 2.0 - 0.05) * end, floor_top + 0.3, 0.0), at(0.1, 0.6, 1.1)));
    }
    for dx in [-0.8f32, 0.8] {
        for side in [-1.0f32, 1.0] {
            wheel(t, kit, at(cx + dx, RAIL_Y + 0.26, 0.66 * side), 0.26);
        }
    }
    put(t, kit, Shape::Cube, Finish::Matte, rgb(170, 110, 70), cube(at(cx, floor_top + 0.03, 0.0), at(l - 0.3, 0.06, 0.9)));
}

fn wheel(t: &mut ChildSpawnerCommands, kit: &mut Kit, pos: Vec3, r: f32) {
    let thick = 0.14;
    t.spawn((
        Role::Wheel(r),
        Transform::from_translation(pos).with_rotation(Quat::from_rotation_x(FRAC_PI_2)),
        Visibility::default(),
    ))
    .with_children(|w| {
        put(w, kit, Shape::Cylinder(18), Finish::Gloss, rgb(50, 55, 80), cube(Vec3::ZERO, at(r * 2.0, thick, r * 2.0)));
        put(w, kit, Shape::Cylinder(14), Finish::Gloss, rgb(255, 205, 50), cube(at(0.0, 0.01, 0.0), at(r * 1.1, thick + 0.03, r * 1.1)));
        put(w, kit, Shape::Sphere, Finish::Gloss, rgb(235, 90, 70), cube(at(r * 0.62, thick / 2.0 + 0.02, 0.0), at(r * 0.28, r * 0.28, r * 0.28)));
    });
}

fn spawn_rider(out: &mut ChildSpawnerCommands, kit: &mut Kit, look: u8, arms_up: bool) {
    let mut m = Model::new();
    let st = Stance {
        arms_up,
        ..Default::default()
    };
    body(&mut m, &guest_look(look, false), st);
    for p in m.pieces {
        out.spawn(kit.part(p.shape, p.paint, p.at));
    }
}

fn faded(a: f32) -> f32 {
    ((a * ALPHA_STEPS).ceil() / ALPHA_STEPS).clamp(0.0, 1.0)
}

type Nodes<'a> = (
    Entity,
    &'a Role,
    &'a mut Transform,
    &'a mut Visibility,
    Option<&'a mut Stand>,
);

type Puffs<'a> = (
    &'a Role,
    &'a mut Transform,
    &'a mut Visibility,
    &'a mut Part,
    &'a mut MeshMaterial3d<StandardMaterial>,
);

#[allow(clippy::too_many_arguments)]
fn sync(
    mut commands: Commands,
    frame: NonSend<crate::Frame>,
    pen: Res<crate::paint::Pen>,
    mut kit: Kit,
    mut roots: Query<(Entity, &mut Root)>,
    mut nodes: Query<Nodes, (Without<Part>, Without<Eye>)>,
    mut puffs: Query<Puffs, Without<Eye>>,
    mut eye: Lens,
) {
    let Some(p) = playing::<TrainsPlay>(&frame) else {
        return;
    };
    let g = &p.g;
    let view = view_of(&pen);
    let cam = g.cam_of(&view);
    let sh = g.shake.offset();
    let nudge = v3(cam.right() * sh.x * 0.012 - cam.up() * sh.y * 0.012);
    eye.aim(Rig {
        eye: v3(cam.eye) + nudge,
        target: v3(cam.target) + nudge,
        fov: cam.fov,
        near: 0.5,
        far: 420.0,
        sky: SKY,
        shadow_reach: 48.0,
    });
    let Ok((root_e, mut root)) = roots.single_mut() else {
        return;
    };
    let cars = carriages(g.riders.len());
    if root.carriages != cars {
        root.carriages = cars;
        if let Some((train, ..)) = nodes.iter().find(|n| *n.1 == Role::Train) {
            let mut e = commands.entity(train);
            e.despawn_children();
            e.with_children(|t| train_parts(t, &mut kit, cars));
        }
    }
    if root.seed != g.question.seed || root.riders != g.riders.len() {
        root.seed = g.question.seed;
        root.riders = g.riders.len();
        for (e, role, ..) in nodes.iter() {
            if matches!(role, Role::Rider(_)) {
                commands.entity(e).despawn();
            }
        }
        commands.entity(root_e).with_children(|w| {
            for i in 0..g.riders.len() {
                let look = g.looks[i];
                w.spawn((
                    Role::Rider(i),
                    Stand {
                        look,
                        arms_up: false,
                    },
                    Transform::default(),
                    Visibility::Hidden,
                ))
                .with_children(|b| spawn_rider(b, &mut kit, look, false));
            }
        });
    }
    pose(&mut commands, g, &view, &mut kit, &mut nodes, &mut puffs);
}

fn pose(
    commands: &mut Commands,
    g: &Trains,
    view: &View,
    kit: &mut Kit,
    nodes: &mut Query<Nodes, (Without<Part>, Without<Eye>)>,
    puffs: &mut Query<Puffs, Without<Eye>>,
) {
    let center = g.train_center();
    let speed = g.train_speed();
    let moving = speed > 0.5;
    let rock = if moving {
        (g.t * 22.0).sin() * 0.012 * (speed / 10.0).min(1.0)
    } else {
        (g.t * 1.6).sin() * 0.006
    };
    let squash = 1.0 + g.puff_kick.min(1.0) * 0.015;
    let f = view.full();
    let cam = g.cam_of(view);
    let fov_h = 2.0 * SKY_DEPTH * (cam.fov / 2.0).tan();
    for (e, role, mut t, mut v, stand) in nodes.iter_mut() {
        match *role {
            Role::Sky => {
                let s = crate::art::size(SHEET, "bg");
                let k = (f.w / s.x).max(f.h / s.y) * 1.02;
                let unit = fov_h / f.h;
                t.set_if_neq(Transform {
                    translation: Vec3::new(0.0, 0.0, -SKY_DEPTH),
                    rotation: Quat::IDENTITY,
                    scale: Vec3::new(s.x * k * unit, s.y * k * unit, 1.0),
                });
            }
            Role::Train => {
                t.set_if_neq(Transform {
                    translation: Vec3::new(center, rock.abs() * 2.0, 0.0),
                    rotation: Quat::from_rotation_z(rock),
                    scale: Vec3::new(1.0, squash, 1.0),
                });
            }
            Role::Wheel(r) => {
                let spin = Quat::from_rotation_y(-center / r);
                t.rotation = Quat::from_rotation_x(FRAC_PI_2) * spin;
            }
            Role::Rider(i) => {
                let Some(state) = g.riders.get(i) else {
                    continue;
                };
                let hopping = matches!(state, kg_core::games::trains::Rider::Hopping(_));
                if let Some(mut s) = stand {
                    if s.arms_up != hopping {
                        s.arms_up = hopping;
                        let look = s.look;
                        let mut c = commands.entity(e);
                        c.despawn_children();
                        c.with_children(|b| spawn_rider(b, kit, look, hopping));
                    }
                }
                let (tr, show) = rider_transform(g, i, state);
                t.set_if_neq(tr);
                v.set_if_neq(if show {
                    Visibility::Inherited
                } else {
                    Visibility::Hidden
                });
            }
            Role::Smoke(_) => {}
        }
    }
    let len = train_len(g.riders.len());
    let chimney = Vec3::new(center + len / 2.0 - ENGINE_LEN / 2.0 + CHIMNEY_X - 0.1, CHIMNEY_Y, 0.0);
    for (role, mut t, mut v, mut part, mut mat) in puffs.iter_mut() {
        let Role::Smoke(k) = *role else {
            continue;
        };
        let phase = (g.t * 0.7 + k as f32 / SMOKE as f32).rem_euclid(1.0);
        let kick = 1.0 + g.puff_kick.min(1.5) * 0.7;
        let drift = -phase * (1.4 + speed * 0.09);
        let size = (0.3 + phase * 0.75) * kick;
        t.set_if_neq(Transform::from_translation(chimney + Vec3::new(drift, phase * 1.5 * kick, 0.0))
            .with_scale(Vec3::splat(size)));
        let a = faded(0.55 * (1.0 - phase) * (0.4 + 0.6 * (phase * 6.0).min(1.0)));
        let on = a > 0.0;
        v.set_if_neq(if on { Visibility::Inherited } else { Visibility::Hidden });
        kit.repaint(
            &mut part,
            &mut mat,
            Paint::new(Finish::Ghost, Col::new(0.97, 0.97, 1.0, a.max(0.125))),
        );
    }
}

pub fn rider_transform(g: &Trains, i: usize, state: &kg_core::games::trains::Rider) -> (Transform, bool) {
    use kg_core::games::trains::Rider::*;
    let pos = crate::game3d::v3(g.rider_world(i));
    let moving = g.train_speed() > 0.5;
    match *state {
        Aboard => {
            let hide = g.hide_k();
            let sway = if moving {
                (g.t * 18.0 + i as f32).sin().abs() * 0.04
            } else {
                (g.t * 2.2 + i as f32 * 0.9).sin() * 0.02
            };
            let k = 1.0 - kg_core::fx::ease_out_cubic(hide);
            let s = Vec3::new(1.0, k.max(0.001), 1.0);
            (
                Transform::from_translation(pos + Vec3::Y * sway).with_scale(s),
                hide < 0.98,
            )
        }
        Hopping(t) => {
            let u = (t / HOP_TIME).clamp(0.0, 1.0);
            let stretch = (u * std::f32::consts::PI).sin();
            (
                Transform::from_translation(pos)
                    .with_rotation(Quat::from_rotation_y(u * TAU))
                    .with_scale(Vec3::new(1.0 - 0.12 * stretch, 1.0 + 0.2 * stretch, 1.0 - 0.12 * stretch)),
                true,
            )
        }
        Off(t) => {
            let land = (-t * 9.0).exp();
            let cheer = if matches!(g.phase, kg_core::games::trains::Phase::Resolve | kg_core::games::trains::Phase::Depart) {
                ((g.t * 7.0 + i as f32 * 1.3).sin().max(0.0)) * 0.22
            } else {
                0.0
            };
            let leave = if g.phase == kg_core::games::trains::Phase::Depart {
                ((g.phase_t - DEPART_TIME * 0.65) / (DEPART_TIME * 0.3)).clamp(0.0, 1.0)
            } else {
                0.0
            };
            let s = (1.0 - leave).max(0.001);
            (
                Transform::from_translation(pos + Vec3::Y * cheer).with_scale(Vec3::new(
                    (1.0 + 0.22 * land) * s,
                    (1.0 - 0.28 * land) * s,
                    (1.0 + 0.22 * land) * s,
                )),
                leave < 0.98,
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game3d::testing;
    use crate::games::trains::TrainsPlay;
    use crate::games::Play;
    use kg_core::engine::Input;

    fn fresh() -> TrainsPlay {
        kg_core::save::reset_for_test();
        kg_core::profile::create("Mia", 0);
        TrainsPlay::new()
    }

    fn open(play: TrainsPlay, view: View) -> App {
        let mut app = testing::with_game(Box::new(play), view);
        testing::step(&mut app, 3);
        app
    }

    fn game(app: &mut App) -> &mut TrainsPlay {
        testing::game::<TrainsPlay>(app)
    }

    fn riders(app: &mut App) -> usize {
        let w = app.world_mut();
        w.query::<&Role>()
            .iter(w)
            .filter(|r| matches!(r, Role::Rider(_)))
            .count()
    }

    fn wheels(app: &mut App) -> usize {
        let w = app.world_mut();
        w.query::<&Role>()
            .iter(w)
            .filter(|r| matches!(r, Role::Wheel(_)))
            .count()
    }

    #[test]
    fn opening_and_closing_the_station_leaves_the_menu_as_it_was() {
        let mut app = testing::app(crate::Scene::Menu(Box::default()), View::ipad_landscape());
        testing::step(&mut app, 3);
        let menu = testing::count(&mut app);
        for _ in 0..3 {
            if let crate::Scene::Menu(m) = &mut testing::frame(&mut app).scene {
                m.current = Some((0, Box::new(fresh())));
            }
            testing::step(&mut app, 3);
            assert!(testing::count(&mut app) > menu + 150);
            assert_eq!(testing::open(&app), World3d::Trains);
            testing::close(&mut app);
            testing::step(&mut app, 3);
            assert_eq!(testing::count(&mut app), menu);
            assert_eq!(testing::open(&app), World3d::Flat);
        }
    }

    #[test]
    fn the_train_has_a_rider_for_every_seat_and_wheels_to_roll() {
        let mut app = open(fresh(), View::ipad_portrait());
        let n = game(&mut app).g.riders.len();
        assert_eq!(riders(&mut app), n);
        let cars = carriages(n);
        assert_eq!(wheels(&mut app), 6 + cars * 4);
    }

    #[test]
    fn a_new_question_swaps_the_riders_without_leaking_entities() {
        let mut app = open(fresh(), View::ipad_landscape());
        for level in [0usize, 1, 3, 1, 0] {
            game(&mut app).g.start_level(level);
            testing::step(&mut app, 3);
            let n = game(&mut app).g.riders.len();
            assert_eq!(riders(&mut app), n);
        }
        let before = testing::count(&mut app);
        for _ in 0..4 {
            let g = &mut game(&mut app).g;
            let level = g.ladder.level;
            g.start_level(level);
            testing::step(&mut app, 3);
        }
        let n = game(&mut app).g.riders.len();
        let after = testing::count(&mut app);
        assert_eq!(riders(&mut app), n);
        assert!(after.abs_diff(before) < 400, "{before} -> {after}");
    }

    #[test]
    fn hopped_riders_stand_on_the_platform_and_boarded_ones_in_the_carriage() {
        let v = View::ipad_landscape();
        let mut app = open(fresh(), v);
        {
            let p = game(&mut app);
            p.g.question = Question::from_pair(0, 5, 2);
            p.g.start_level(0);
            p.g.question = Question::from_pair(0, 5, 2);
            p.g.riders = vec![kg_core::games::trains::Rider::Aboard; 5];
            p.g.looks = (0..5).collect();
            p.g.spots = vec![0; 5];
            let mut guard = 0;
            while p.g.phase != kg_core::games::trains::Phase::Remove && guard < 3000 {
                p.update(&Input::default(), &v, 1.0 / 60.0);
                guard += 1;
            }
            p.g.tap_rider(4);
            for _ in 0..60 {
                p.update(&Input::default(), &v, 1.0 / 60.0);
            }
        }
        testing::step(&mut app, 2);
        let w = app.world_mut();
        let mut q = w.query::<(&Role, &Transform)>();
        let mut platform = 0;
        for (r, t) in q.iter(w) {
            if let Role::Rider(4) = r {
                assert!(t.translation.z > 2.0, "{:?}", t.translation);
                platform += 1;
            }
            if let Role::Rider(0) = r {
                assert!(t.translation.z.abs() < 0.1);
            }
        }
        assert_eq!(platform, 1);
    }

    #[test]
    fn smoke_puffs_fade_in_and_out_without_new_materials_every_frame() {
        let mut app = open(fresh(), View::ipad_landscape());
        testing::step(&mut app, 120);
        let mats = app.world().resource::<Assets<StandardMaterial>>().len();
        testing::step(&mut app, 120);
        let later = app.world().resource::<Assets<StandardMaterial>>().len();
        assert!(later <= mats + 8, "{mats} -> {later}");
    }
}
