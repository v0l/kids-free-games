use super::{RunnerPlay, SHEET};
use crate::game3d::*;
use bevy::ecs::hierarchy::ChildSpawnerCommands;
use bevy::pbr::{DistanceFog, FogFalloff};
use bevy::prelude::*;
use kg_core::engine::View;
use kg_core::games::runner::State as Phase;
use kg_core::games::runner::*;
use kg_core::geom::{Color as Col, WHITE};
use std::f32::consts::FRAC_PI_2;

const SKY: Col = Col::new(0.66, 0.88, 0.99, 1.0);
const SKY_DEPTH: f32 = 140.0;
const SLOT: f32 = 9.0;
const SLOTS: usize = 14;
const TREE_SLOT: f32 = 13.0;
const TREES: usize = 9;
const BEHIND: f32 = 14.0;
const WHOLE: [f32; 4] = [0.0, 0.0, 1.0, 1.0];
const ROAD_W: f32 = 5.4;
const ROAD_TILE: f32 = 8.0;
const WALK_W: f32 = 2.0;
const HEIGHTS: [f32; 4] = [5.0, 7.5, 10.0, 13.0];
const TINTS: [Col; 5] = [
    Col::new(1.0, 0.82, 0.78, 1.0),
    Col::new(0.84, 0.92, 1.0, 1.0),
    Col::new(1.0, 0.94, 0.72, 1.0),
    Col::new(0.82, 1.0, 0.86, 1.0),
    Col::new(0.94, 0.84, 1.0, 1.0),
];
const SKIN: Col = Col::new(1.0, 0.78, 0.62, 1.0);
const JACKET: Col = Col::new(1.0, 0.76, 0.1, 1.0);
const JEANS: Col = Col::new(0.2, 0.36, 0.7, 1.0);
const CAP: Col = Col::new(0.95, 0.2, 0.2, 1.0);
const PACK: Col = Col::new(0.55, 0.12, 0.25, 1.0);
const GOLD: Col = Col::new(1.0, 0.8, 0.1, 1.0);

pub struct RunnerWorld;

impl Plugin for RunnerWorld {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                spawn.run_if(opened(World3d::Runner)),
                sync.run_if(is_open(World3d::Runner)),
            )
                .chain()
                .in_set(Sync3d),
        );
    }
}

#[derive(Component)]
pub struct Root {
    serial: u64,
}

#[derive(Component, Clone, Copy, PartialEq, Eq, Debug)]
pub enum Role {
    Sky,
    Course,
    Player,
    Body,
    Leg(usize),
    Arm(usize),
    Shade,
    Obstacle(usize, usize),
    Coin(usize),
    Building(usize),
    Tree(usize),
}

type Parts<'w, 's> = Query<
    'w,
    's,
    (
        &'static Role,
        &'static mut Transform,
        &'static mut Visibility,
        &'static mut Part,
        &'static mut MeshMaterial3d<StandardMaterial>,
    ),
    Without<Eye>,
>;
type Nodes<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static Role,
        &'static mut Transform,
        &'static mut Visibility,
    ),
    (Without<Part>, Without<Eye>),
>;

fn sprite(name: &'static str) -> Skin {
    Skin::Sprite(SHEET, name, WHOLE)
}

fn tile(name: &'static str, repeat: Vec2) -> Skin {
    Skin::Tile(SHEET, name, repeat)
}

fn matte(c: Col) -> Paint {
    Paint::new(Finish::Matte, c)
}

fn gloss(c: Col) -> Paint {
    Paint::new(Finish::Gloss, c)
}

fn put(
    out: &mut ChildSpawnerCommands,
    kit: &mut Kit,
    shape: Shape,
    paint: Paint,
    at: Vec3,
    size: Vec3,
) {
    out.spawn(kit.part(
        shape,
        paint,
        Transform::from_translation(at).with_scale(size),
    ));
}

fn node(out: &mut ChildSpawnerCommands, role: Role, at: Transform) -> Entity {
    out.spawn((role, at, Visibility::default())).id()
}

fn spawn(mut commands: Commands, mut kit: Kit) {
    let world = spawn_world(
        &mut commands,
        Root { serial: 0 },
        World3d::Runner,
        Lighting {
            ambient: 650.0,
            illuminance: 11000.0,
            color: WHITE,
            toward: Vec3::new(-0.5, -1.0, -0.35),
        },
    );
    commands.entity(world.eye).insert(DistanceFog {
        color: Color::srgb(SKY.r, SKY.g, SKY.b),
        falloff: FogFalloff::Linear {
            start: 28.0,
            end: 118.0,
        },
        ..default()
    });
    commands.entity(world.eye).with_children(|c| {
        c.spawn((
            Role::Sky,
            kit.part(
                Shape::Quad,
                Paint::skinned(Finish::Backdrop, WHITE, sprite("bg")),
                Transform::default(),
            ),
        ));
    });
    commands.entity(world.root).with_children(|w| {
        node(w, Role::Course, Transform::default());
        let player = node(w, Role::Player, Transform::default());
        w.commands()
            .entity(player)
            .with_children(|p| kid(p, &mut kit));
        w.spawn((
            Role::Shade,
            kit.part(
                Shape::Disc,
                Paint::new(Finish::Ghost, Col::new(0.05, 0.05, 0.15, 0.32)),
                Transform::default(),
            ),
        ));
        for i in 0..SLOTS * 2 {
            w.spawn((
                Role::Building(i),
                kit.part(
                    Shape::Cube,
                    Paint::skinned(Finish::Matte, TINTS[0], tile("wall", Vec2::new(2.0, 2.0))),
                    Transform::default(),
                ),
            ));
        }
        for i in 0..TREES * 2 {
            let t = node(w, Role::Tree(i), Transform::default());
            w.commands().entity(t).with_children(|c| {
                put(
                    c,
                    &mut kit,
                    Shape::Cylinder(8),
                    matte(Col::new(0.5, 0.33, 0.2, 1.0)),
                    Vec3::new(0.0, 0.7, 0.0),
                    Vec3::new(0.3, 1.4, 0.3),
                );
                put(
                    c,
                    &mut kit,
                    Shape::Sphere,
                    gloss(Col::new(0.34, 0.72, 0.28, 1.0)),
                    Vec3::new(0.0, 2.2, 0.0),
                    Vec3::new(1.9, 1.7, 1.9),
                );
                put(
                    c,
                    &mut kit,
                    Shape::Sphere,
                    gloss(Col::new(0.45, 0.82, 0.32, 1.0)),
                    Vec3::new(0.25, 2.8, 0.1),
                    Vec3::new(1.2, 1.1, 1.2),
                );
            });
        }
    });
}

fn kid(p: &mut ChildSpawnerCommands, kit: &mut Kit) {
    let body = node(p, Role::Body, Transform::default());
    p.commands().entity(body).with_children(|b| {
        put(
            b,
            kit,
            Shape::Sphere,
            gloss(JACKET),
            Vec3::new(0.0, 0.88, 0.0),
            Vec3::new(0.62, 0.7, 0.5),
        );
        put(
            b,
            kit,
            Shape::Cube,
            matte(PACK),
            Vec3::new(0.0, 0.92, 0.3),
            Vec3::new(0.44, 0.5, 0.2),
        );
        put(
            b,
            kit,
            Shape::Sphere,
            gloss(SKIN),
            Vec3::new(0.0, 1.4, 0.0),
            Vec3::splat(0.6),
        );
        put(
            b,
            kit,
            Shape::Sphere,
            gloss(CAP),
            Vec3::new(0.0, 1.52, 0.02),
            Vec3::new(0.66, 0.42, 0.66),
        );
        put(
            b,
            kit,
            Shape::Cube,
            matte(CAP.darker(0.8)),
            Vec3::new(0.0, 1.45, -0.36),
            Vec3::new(0.46, 0.06, 0.3),
        );
        for s in 0..2 {
            let side = if s == 0 { -1.0 } else { 1.0 };
            let leg = node(b, Role::Leg(s), Transform::from_xyz(side * 0.17, 0.55, 0.0));
            b.commands().entity(leg).with_children(|l| {
                put(
                    l,
                    kit,
                    Shape::Cylinder(10),
                    matte(JEANS),
                    Vec3::new(0.0, -0.22, 0.0),
                    Vec3::new(0.2, 0.46, 0.2),
                );
                put(
                    l,
                    kit,
                    Shape::Cube,
                    matte(CAP),
                    Vec3::new(0.0, -0.5, -0.05),
                    Vec3::new(0.22, 0.12, 0.34),
                );
            });
            let arm = node(b, Role::Arm(s), Transform::from_xyz(side * 0.38, 1.05, 0.0));
            b.commands().entity(arm).with_children(|a| {
                put(
                    a,
                    kit,
                    Shape::Cylinder(10),
                    gloss(JACKET),
                    Vec3::new(0.0, -0.2, 0.0),
                    Vec3::new(0.16, 0.42, 0.16),
                );
                put(
                    a,
                    kit,
                    Shape::Sphere,
                    gloss(SKIN),
                    Vec3::new(0.0, -0.44, 0.0),
                    Vec3::splat(0.17),
                );
            });
        }
    });
}

fn train_colour(row: usize) -> Col {
    [
        Col::new(0.25, 0.55, 0.95, 1.0),
        Col::new(0.2, 0.75, 0.6, 1.0),
        Col::new(0.7, 0.4, 0.9, 1.0),
    ][row % 3]
}

fn obstacle(c: &mut ChildSpawnerCommands, kit: &mut Kit, kind: Kind, row: usize) {
    let white = matte(Col::new(0.98, 0.98, 0.96, 1.0));
    let red = gloss(Col::new(0.93, 0.25, 0.22, 1.0));
    let orange = gloss(Col::new(1.0, 0.55, 0.12, 1.0));
    let grey = matte(Col::new(0.45, 0.46, 0.55, 1.0));
    match kind {
        Kind::Barrier => {
            for x in [-0.55, 0.55] {
                put(
                    c,
                    kit,
                    Shape::Cube,
                    grey,
                    Vec3::new(x, 0.5, 0.0),
                    Vec3::new(0.14, 1.0, 0.14),
                );
            }
            put(
                c,
                kit,
                Shape::Cube,
                red,
                Vec3::new(0.0, 0.8, 0.0),
                Vec3::new(1.4, 0.5, 0.2),
            );
            for k in 0..3 {
                put(
                    c,
                    kit,
                    Shape::Cube,
                    white,
                    Vec3::new(-0.45 + k as f32 * 0.45, 0.8, 0.0),
                    Vec3::new(0.2, 0.52, 0.22),
                );
            }
            put(
                c,
                kit,
                Shape::Sphere,
                Paint::new(Finish::Glow, Col::new(1.0, 0.7, 0.1, 1.0)),
                Vec3::new(0.0, 1.15, 0.0),
                Vec3::splat(0.2),
            );
        }
        Kind::Hurdle => {
            for x in [-0.62, 0.62] {
                put(
                    c,
                    kit,
                    Shape::Cube,
                    grey,
                    Vec3::new(x, 0.28, 0.0),
                    Vec3::new(0.12, 0.56, 0.12),
                );
            }
            put(
                c,
                kit,
                Shape::Cube,
                orange,
                Vec3::new(0.0, 0.5, 0.0),
                Vec3::new(1.5, 0.2, 0.16),
            );
            for k in 0..3 {
                put(
                    c,
                    kit,
                    Shape::Cube,
                    white,
                    Vec3::new(-0.45 + k as f32 * 0.45, 0.5, 0.0),
                    Vec3::new(0.18, 0.22, 0.18),
                );
            }
        }
        Kind::Overhead => {
            for x in [-0.72, 0.72] {
                put(
                    c,
                    kit,
                    Shape::Cube,
                    grey,
                    Vec3::new(x, 1.2, 0.0),
                    Vec3::new(0.16, 2.4, 0.16),
                );
            }
            put(
                c,
                kit,
                Shape::Cube,
                gloss(Col::new(0.2, 0.7, 0.45, 1.0)),
                Vec3::new(0.0, 1.5, 0.0),
                Vec3::new(1.6, 0.55, 0.26),
            );
            for k in 0..4 {
                put(
                    c,
                    kit,
                    Shape::Cube,
                    matte(GOLD),
                    Vec3::new(-0.6 + k as f32 * 0.4, 1.5, 0.0),
                    Vec3::new(0.14, 0.58, 0.28),
                );
            }
        }
        Kind::Train => {
            let body = train_colour(row);
            put(
                c,
                kit,
                Shape::Cube,
                gloss(body),
                Vec3::new(0.0, 1.3, 0.0),
                Vec3::new(1.45, 2.2, TRAIN_LEN),
            );
            put(
                c,
                kit,
                Shape::Cube,
                gloss(body.darker(1.15)),
                Vec3::new(0.0, 2.45, 0.0),
                Vec3::new(1.3, 0.2, TRAIN_LEN - 0.4),
            );
            put(
                c,
                kit,
                Shape::Cube,
                gloss(Col::new(0.75, 0.93, 1.0, 1.0)),
                Vec3::new(0.0, 1.85, 0.0),
                Vec3::new(1.47, 0.7, TRAIN_LEN - 1.0),
            );
            put(
                c,
                kit,
                Shape::Cube,
                matte(Col::new(0.2, 0.2, 0.3, 1.0)),
                Vec3::new(0.0, 0.3, 0.0),
                Vec3::new(1.2, 0.3, TRAIN_LEN - 0.6),
            );
            for x in [-0.4, 0.4] {
                put(
                    c,
                    kit,
                    Shape::Sphere,
                    Paint::new(Finish::Glow, Col::new(1.0, 0.95, 0.6, 1.0)),
                    Vec3::new(x, 0.9, TRAIN_LEN / 2.0 + 0.02),
                    Vec3::splat(0.3),
                );
            }
        }
    }
}

fn course(c: &mut ChildSpawnerCommands, kit: &mut Kit, g: &Runner) {
    let end = g.length + FINISH_RUN + 130.0;
    let len = end + 20.0;
    let mid = (20.0 - end) / 2.0;
    c.spawn(kit.part(
        Shape::Plane,
        Paint::new(Finish::Floor, Col::new(0.55, 0.82, 0.4, 1.0)),
        Transform::from_xyz(0.0, -0.04, mid).with_scale(Vec3::new(260.0, 1.0, len)),
    ));
    c.spawn(kit.part(
        Shape::Plane,
        Paint::skinned(
            Finish::Floor,
            WHITE,
            tile("road", Vec2::new(1.0, len / ROAD_TILE)),
        ),
        Transform::from_xyz(0.0, 0.0, mid).with_scale(Vec3::new(ROAD_W, 1.0, len)),
    ));
    for side in [-1.0, 1.0] {
        let x = side * (ROAD_W / 2.0 + WALK_W / 2.0);
        c.spawn(kit.part(
            Shape::Plane,
            Paint::skinned(
                Finish::Floor,
                WHITE,
                tile("walk", Vec2::new(WALK_W / 2.0, len / 2.0)),
            ),
            Transform::from_xyz(x, 0.03, mid).with_scale(Vec3::new(WALK_W, 1.0, len)),
        ));
    }
    for (i, r) in g.rows.iter().enumerate() {
        for (k, kind) in r.lanes.iter().enumerate() {
            let Some(kind) = kind else {
                continue;
            };
            let z = -(r.at + kind.len() / 2.0);
            let n = node(
                c,
                Role::Obstacle(i, k),
                Transform::from_xyz(lane_x(k as i32 - 1), 0.0, z),
            );
            let kind = *kind;
            c.commands()
                .entity(n)
                .with_children(|o| obstacle(o, kit, kind, i));
        }
    }
    for (i, coin) in g.coins_list.iter().enumerate() {
        c.spawn((
            Role::Coin(i),
            kit.part(
                Shape::Cylinder(18),
                Paint::new(Finish::Gloss, GOLD),
                Transform::from_xyz(lane_x(coin.lane as i32 - 1), coin.h, -coin.at)
                    .with_scale(Vec3::new(0.5, 0.09, 0.5)),
            ),
        ));
    }
    let z = -g.length;
    let post = matte(Col::new(0.95, 0.95, 0.98, 1.0));
    for x in [-2.9, 2.9] {
        c.spawn(kit.part(
            Shape::Cube,
            post,
            Transform::from_xyz(x, 2.2, z).with_scale(Vec3::new(0.4, 4.4, 0.4)),
        ));
    }
    for k in 0..12 {
        for row in 0..2 {
            let dark = (k + row) % 2 == 0;
            let col = if dark {
                Col::new(0.1, 0.1, 0.15, 1.0)
            } else {
                WHITE
            };
            c.spawn(
                kit.part(
                    Shape::Cube,
                    matte(col),
                    Transform::from_xyz(-2.75 + k as f32 * 0.5, 4.5 - row as f32 * 0.5, z)
                        .with_scale(Vec3::new(0.5, 0.5, 0.3)),
                ),
            );
        }
    }
    c.spawn(kit.part(
        Shape::Plane,
        matte(WHITE),
        Transform::from_xyz(0.0, 0.015, z).with_scale(Vec3::new(ROAD_W, 1.0, 0.6)),
    ));
}

#[allow(clippy::too_many_arguments)]
fn sync(
    mut commands: Commands,
    frame: NonSend<crate::Frame>,
    pen: Res<crate::paint::Pen>,
    mut kit: Kit,
    mut root: Query<&mut Root>,
    mut eye: Lens,
    mut nodes: Nodes<'_, '_>,
    mut parts: Parts<'_, '_>,
) {
    let Some(p) = playing::<RunnerPlay>(&frame) else {
        return;
    };
    let Ok(mut root) = root.single_mut() else {
        return;
    };
    let g = &p.g;
    let view = pen.view.unwrap_or_else(View::ipad_landscape);
    let cam = Cam::new(g, &view);
    eye.aim(Rig {
        eye: v3(cam.eye),
        target: v3(cam.target),
        fov: cam.fov,
        near: 0.3,
        far: 170.0,
        sky: SKY,
        shadow_reach: 60.0,
    });
    if root.serial != g.serial {
        root.serial = g.serial;
        if let Some((e, ..)) = nodes.iter().find(|(_, r, ..)| **r == Role::Course) {
            let mut c = commands.entity(e);
            c.despawn_children();
            c.with_children(|c| course(c, &mut kit, g));
        }
        return;
    }
    pose_nodes(g, &mut nodes);
    pose_parts(g, &view, &cam, &mut kit, &mut parts);
}

fn shown(on: bool) -> Visibility {
    if on {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    }
}

pub fn flight(age: f32, lane: usize) -> Transform {
    let side = lane as f32 - 1.0;
    let k = (1.0 - age / 0.9).clamp(0.0, 1.0);
    Transform {
        translation: Vec3::new(side * age * 4.0, age * 6.0 - age * age * 5.0, -age * 8.0),
        rotation: Quat::from_euler(EulerRot::XYZ, -age * 6.0, 0.0, side * age * 5.0 + age * 3.0),
        scale: Vec3::splat(k.max(0.001)),
    }
}

pub fn stride(dist: f32) -> f32 {
    dist * 1.9
}

fn pose_nodes(g: &Runner, nodes: &mut Nodes<'_, '_>) {
    let airborne = g.y > 0.05;
    let phase = stride(g.dist);
    let moving = (g.speed / 8.0).clamp(0.0, 1.0);
    let cheer = g.state == Phase::Won && g.end_t > 0.4;
    for (_, role, mut t, mut v) in nodes.iter_mut() {
        match *role {
            Role::Player => {
                let sway = (g.stumble_t * 30.0).sin() * 0.25 * (g.stumble_t / 0.6).min(1.0);
                t.set_if_neq(Transform {
                    translation: Vec3::new(g.x, g.y, -g.dist),
                    rotation: Quat::from_euler(
                        EulerRot::YXZ,
                        -g.lean * 0.3,
                        0.0,
                        -g.lean * 0.28 + sway,
                    ),
                    scale: Vec3::ONE,
                });
                v.set_if_neq(shown(!g.blinking()));
            }
            Role::Body => {
                let slide = (g.slide_t / SLIDE_TIME).clamp(0.0, 1.0);
                let slide_k = (slide * 8.0).min(1.0) * if g.sliding() { 1.0 } else { 0.0 };
                let bob = if airborne || g.state == Phase::Ready {
                    0.0
                } else {
                    (phase * 2.0).sin().abs() * 0.07 * moving
                };
                let idle = if g.state == Phase::Ready {
                    (g.t * 3.0).sin() * 0.03
                } else {
                    0.0
                };
                let hop = if cheer {
                    (g.end_t * 7.0).sin().abs() * 0.35
                } else {
                    0.0
                };
                let fall = if g.state == Phase::Dead {
                    (g.end_t * 3.0).min(1.0)
                } else {
                    0.0
                };
                let sq = g.squash;
                t.set_if_neq(Transform {
                    translation: Vec3::new(0.0, bob + idle + hop + 0.0 - slide_k * 0.05, 0.0),
                    rotation: Quat::from_rotation_x(
                        slide_k * 1.25 - fall * 1.3 + if airborne { -0.12 } else { 0.1 * moving },
                    ),
                    scale: Vec3::new(1.0 + sq * 0.14, 1.0 - sq * 0.2, 1.0 + sq * 0.14),
                });
            }
            Role::Leg(s) => {
                let sgn = if s == 0 { 1.0 } else { -1.0 };
                let swing = if g.sliding() {
                    -1.2
                } else if airborne {
                    sgn * 0.7 - 0.3
                } else {
                    (phase).sin() * sgn * 0.95 * moving
                };
                t.set_if_neq(Transform {
                    translation: Vec3::new(if s == 0 { -0.17 } else { 0.17 }, 0.55, 0.0),
                    rotation: Quat::from_rotation_x(swing),
                    scale: Vec3::ONE,
                });
            }
            Role::Arm(s) => {
                let sgn = if s == 0 { -1.0 } else { 1.0 };
                let swing = if cheer {
                    2.8 + (g.end_t * 14.0).sin() * 0.3
                } else if airborne {
                    2.4
                } else if g.sliding() {
                    0.8
                } else {
                    (phase).sin() * sgn * 0.9 * moving
                };
                t.set_if_neq(Transform {
                    translation: Vec3::new(sgn * 0.38, 1.05, 0.0),
                    rotation: Quat::from_rotation_x(swing) * Quat::from_rotation_z(sgn * 0.12),
                    scale: Vec3::ONE,
                });
            }
            Role::Obstacle(i, k) => {
                let Some(r) = g.rows.get(i) else {
                    continue;
                };
                let kind = r.lanes[k].unwrap_or(Kind::Barrier);
                let base =
                    Transform::from_xyz(lane_x(k as i32 - 1), 0.0, -(r.at + kind.len() / 2.0));
                if r.hit && r.hit_lanes[k] && kind != Kind::Train {
                    let age = (g.t - r.hit_t).max(0.0);
                    let f = flight(age, k);
                    v.set_if_neq(shown(age < 0.9));
                    t.set_if_neq(Transform {
                        translation: base.translation + f.translation,
                        rotation: f.rotation,
                        scale: f.scale,
                    });
                } else {
                    t.set_if_neq(base);
                }
            }
            Role::Tree(i) => {
                let side = if i % 2 == 0 { -1.0 } else { 1.0 };
                let k = i / 2;
                let z = ring(k, TREES, TREE_SLOT, g.dist, 0.0);
                v.set_if_neq(shown(z < g.length + FINISH_RUN + 20.0));
                t.set_if_neq(Transform::from_xyz(
                    side * (ROAD_W / 2.0 + WALK_W - 0.2),
                    0.0,
                    -z,
                ));
            }
            _ => {}
        }
    }
}

fn ring(k: usize, count: usize, slot: f32, dist: f32, shift: f32) -> f32 {
    let span = count as f32 * slot;
    let own = k as f32 * slot + shift;
    own + span * ((dist - BEHIND - own) / span).floor().max(-1.0) + span
}

fn slot_index(k: usize, count: usize, slot: f32, dist: f32) -> i64 {
    let span = count as f32 * slot;
    let own = k as f32 * slot;
    ((dist - BEHIND - own) / span).floor() as i64 * count as i64 + k as i64 + count as i64
}

fn hash(n: i64) -> usize {
    let mut x = (n as u64).wrapping_mul(0x9E3779B97F4A7C15);
    x ^= x >> 29;
    (x.wrapping_mul(0xBF58476D1CE4E5B9) >> 33) as usize
}

fn pose_parts(g: &Runner, view: &View, cam: &Cam, kit: &mut Kit, parts: &mut Parts<'_, '_>) {
    for (role, mut t, mut v, mut part, mut mat) in parts.iter_mut() {
        match *role {
            Role::Sky => {
                let f = view.full();
                let vh = 2.0 * SKY_DEPTH * (cam.fov / 2.0).tan();
                let vw = vh * f.w / f.h;
                let s = crate::art::size(SHEET, "bg");
                let k = (vw / s.x).max(vh / s.y) * 1.05;
                let fwd = (cam.target - cam.eye).normalize();
                let pitch = (-fwd.y).asin();
                let horizon = SKY_DEPTH * pitch.tan();
                let (w, h) = (s.x * k, s.y * k);
                t.set_if_neq(
                    Transform::from_xyz(0.0, horizon + h * 0.15, -SKY_DEPTH)
                        .with_scale(Vec3::new(w, h, 1.0)),
                );
            }
            Role::Shade => {
                let k = (1.0 - g.y * 0.25).max(0.3);
                t.set_if_neq(
                    Transform::from_xyz(g.x, 0.045, -g.dist).with_scale(Vec3::new(
                        0.9 * k,
                        1.0,
                        0.9 * k,
                    )),
                );
                v.set_if_neq(shown(g.state != Phase::Select));
            }
            Role::Coin(i) => {
                let Some(c) = g.coins_list.get(i) else {
                    continue;
                };
                v.set_if_neq(shown(!c.taken));
                let spin = g.t * 4.0 + i as f32 * 0.7;
                t.set_if_neq(Transform {
                    translation: Vec3::new(
                        lane_x(c.lane as i32 - 1),
                        c.h + (g.t * 3.0 + i as f32).sin() * 0.06,
                        -c.at,
                    ),
                    rotation: Quat::from_rotation_y(spin) * Quat::from_rotation_z(FRAC_PI_2),
                    scale: Vec3::new(0.5, 0.09, 0.5),
                });
            }
            Role::Building(i) => {
                let side = if i % 2 == 0 { -1.0 } else { 1.0 };
                let k = i / 2;
                let z = ring(k, SLOTS, SLOT, g.dist, 0.0);
                let n = hash(slot_index(k, SLOTS, SLOT, g.dist) * 2 + (i % 2) as i64);
                let h = HEIGHTS[n % HEIGHTS.len()];
                let tint = TINTS[(n / 7) % TINTS.len()];
                let depth = SLOT - 1.4;
                let off = ROAD_W / 2.0 + WALK_W + 2.6;
                v.set_if_neq(shown(z < g.length + FINISH_RUN + 20.0));
                t.set_if_neq(
                    Transform::from_xyz(side * off, h / 2.0, -z)
                        .with_scale(Vec3::new(5.2, h, depth)),
                );
                let paint = Paint::skinned(
                    Finish::Matte,
                    tint,
                    Skin::Tile(SHEET, "wall", Vec2::new(2.0, (h / 2.5).round())),
                );
                kit.repaint(&mut part, &mut mat, paint);
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game3d::testing;
    use crate::games::Play;
    use kg_core::engine::{Dir, Input};

    fn open(play: RunnerPlay) -> App {
        let mut app = testing::with_game(Box::new(play), View::ipad_portrait());
        testing::step(&mut app, 3);
        app
    }

    fn runner(app: &mut App) -> &mut RunnerPlay {
        testing::game::<RunnerPlay>(app)
    }

    fn roles(app: &mut App, f: impl Fn(&Role) -> bool) -> usize {
        let w = app.world_mut();
        w.query::<&Role>().iter(w).filter(|r| f(r)).count()
    }

    fn fresh(level: u32) -> RunnerPlay {
        kg_core::save::reset_for_test();
        kg_core::rng::srand(5);
        let mut p = RunnerPlay::new();
        p.g = Runner::with_level(level);
        p
    }

    #[test]
    fn opening_and_closing_the_runner_leaves_the_menu_as_it_was() {
        let mut app = testing::app(crate::Scene::Menu(Box::default()), View::ipad_landscape());
        testing::step(&mut app, 3);
        let menu = testing::count(&mut app);
        for _ in 0..3 {
            if let crate::Scene::Menu(m) = &mut testing::frame(&mut app).scene {
                m.current = Some((0, Box::new(fresh(8))));
            }
            testing::step(&mut app, 3);
            let open = testing::count(&mut app);
            assert!(open > menu + 100, "{menu} -> {open}");
            assert_eq!(testing::open(&app), World3d::Runner);
            testing::close(&mut app);
            testing::step(&mut app, 3);
            assert_eq!(testing::count(&mut app), menu);
            assert_eq!(testing::open(&app), World3d::Flat);
        }
    }

    #[test]
    fn the_course_has_a_node_per_blocked_lane_and_a_part_per_coin() {
        let mut app = open(fresh(8));
        let g = &runner(&mut app).g;
        let blocked: usize = g.rows.iter().map(|r| r.blocked()).sum();
        let coins = g.coins_list.len();
        assert_eq!(
            roles(&mut app, |r| matches!(r, Role::Obstacle(..))),
            blocked
        );
        assert_eq!(roles(&mut app, |r| matches!(r, Role::Coin(_))), coins);
        assert_eq!(roles(&mut app, |r| *r == Role::Player), 1);
    }

    #[test]
    fn the_runner_node_follows_the_player_lane_and_distance() {
        let mut app = open(fresh(1));
        {
            let g = &mut runner(&mut app).g;
            g.state = Phase::Playing;
            g.lane = 1;
            g.x = lane_x(1);
            g.dist = 12.0;
        }
        testing::step(&mut app, 2);
        let w = app.world_mut();
        let t = *w
            .query::<(&Role, &Transform)>()
            .iter(w)
            .find(|(r, _)| **r == Role::Player)
            .unwrap()
            .1;
        assert!((t.translation.x - lane_x(1)).abs() < 1e-3);
        assert!((t.translation.z + 12.0).abs() < 1e-3);
    }

    #[test]
    fn a_collected_coin_disappears() {
        let mut app = open(fresh(1));
        let n = runner(&mut app).g.coins_list.len();
        assert!(n > 0);
        runner(&mut app).g.coins_list[0].taken = true;
        testing::step(&mut app, 2);
        let w = app.world_mut();
        let hidden = w
            .query::<(Entity, &Role)>()
            .iter(w)
            .filter(|(e, r)| matches!(r, Role::Coin(_)) && !visible(w, *e))
            .count();
        assert_eq!(hidden, 1);
    }

    #[test]
    fn a_smashed_barrier_flies_off_and_vanishes() {
        let mut app = open(fresh(1));
        {
            let g = &mut runner(&mut app).g;
            g.state = Phase::Playing;
            g.rows[0].hit = true;
            g.rows[0].hit_lanes = g.rows[0].lanes.map(|l| l.is_some());
            g.rows[0].hit_t = g.t - 0.3;
        }
        testing::step(&mut app, 2);
        let (lane, base) = {
            let g = &runner(&mut app).g;
            let k = g.rows[0].lanes.iter().position(|l| l.is_some()).unwrap();
            (k, g.rows[0].at)
        };
        let w = app.world_mut();
        let t = *w
            .query::<(&Role, &Transform)>()
            .iter(w)
            .find(|(r, _)| **r == Role::Obstacle(0, lane))
            .unwrap()
            .1;
        assert!(t.translation.y > 0.3);
        assert!(t.translation.z < -(base + 0.4));
        assert!(flight(1.0, lane).scale.x < 0.01);
    }

    #[test]
    fn a_new_level_rebuilds_the_course_without_leaking() {
        let mut app = open(fresh(3));
        for _ in 0..3 {
            runner(&mut app).g.enter(3);
            testing::step(&mut app, 3);
            let g = &runner(&mut app).g;
            let blocked: usize = g.rows.iter().map(|r| r.blocked()).sum();
            let coins = g.coins_list.len();
            assert_eq!(
                roles(&mut app, |r| matches!(r, Role::Obstacle(..))),
                blocked
            );
            assert_eq!(roles(&mut app, |r| matches!(r, Role::Coin(_))), coins);
            assert_eq!(roles(&mut app, |r| *r == Role::Course), 1);
        }
    }

    #[test]
    fn the_buildings_recycle_ahead_of_the_player_as_they_run() {
        let mut app = open(fresh(1));
        runner(&mut app).g.state = Phase::Playing;
        for d in [0.0, 60.0, 400.0] {
            runner(&mut app).g.dist = d;
            testing::step(&mut app, 2);
            let w = app.world_mut();
            let zs: Vec<f32> = w
                .query::<(&Role, &Transform)>()
                .iter(w)
                .filter(|(r, _)| matches!(r, Role::Building(_)))
                .map(|(_, t)| -t.translation.z)
                .collect();
            assert!(zs
                .iter()
                .all(|z| *z > d - BEHIND - SLOT && *z < d + SLOTS as f32 * SLOT + 1.0));
            assert!(zs.iter().filter(|z| **z > d + 40.0).count() >= 6);
        }
    }

    #[test]
    fn a_minute_of_autopilot_keeps_the_world_in_step() {
        let mut app = open(fresh(5));
        let view = View::ipad_portrait();
        runner(&mut app).update(
            &Input::press_at(glam::vec2(500.0, 500.0)),
            &view,
            1.0 / 60.0,
        );
        let count = testing::count(&mut app);
        for _ in 0..1800 {
            let i = runner(&mut app).g.autopilot(1.0);
            runner(&mut app).update(&i, &view, 1.0 / 60.0);
            testing::step(&mut app, 1);
        }
        assert_eq!(testing::count(&mut app), count);
        let _ = Dir::Up;
    }
}
