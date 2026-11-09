use super::{TrainsPlay, SHEET};
use crate::game3d::*;
use crate::gcad::Shelf;
use crate::games::hotel::build::Model;
use crate::games::hotel::models::{body, guest_look, rgb, Stance, PEOPLE};
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

pub static MODELS: &[&str] = &[
    "trains_track",
    "trains_platform",
    "trains_lamp",
    "trains_bench",
    "trains_sign",
    "trains_station",
    "trains_tree_round",
    "trains_tree_pine",
    "trains_hill",
    "trains_engine",
    "trains_carriage",
    "trains_wheel",
];

pub struct TrainsWorld;

impl Plugin for TrainsWorld {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                stock(PEOPLE).run_if(is_open(World3d::Trains)),
                stock(MODELS).run_if(is_open(World3d::Trains)),
                spawn.run_if(opened(World3d::Trains)),
                sync.run_if(is_open(World3d::Trains))
                    .run_if(stocked(PEOPLE))
                    .run_if(stocked(MODELS)),
            )
                .chain()
                .in_set(Sync3d),
        );
    }
}

#[derive(Component)]
pub struct Root {
    built: bool,
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

fn put(
    out: &mut ChildSpawnerCommands,
    kit: &mut Kit,
    shape: Shape,
    finish: Finish,
    color: Col,
    t: Transform,
) -> Entity {
    out.spawn(kit.part(shape, Paint::new(finish, color), t))
        .id()
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
    out.spawn(kit.part(
        Shape::Plane,
        paint,
        t.with_scale(Vec3::new(size.x, 1.0, size.y)),
    ));
}

fn spawn(mut commands: Commands, mut kit: Kit) {
    let world = spawn_world(
        &mut commands,
        Root {
            built: false,
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
        let paint = Paint::skinned(
            Finish::Backdrop,
            WHITE,
            Skin::Sprite(SHEET, "bg", [0.0, 0.0, 1.0, 1.0]),
        );
        c.spawn((
            kit.part(Shape::Quad, paint, Transform::default()),
            Role::Sky,
        ));
    });
    commands.entity(world.root).with_children(|w| {
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

fn finish_of(part: &str) -> Option<Finish> {
    match part {
        "bulb" | "headlamp" => Some(Finish::Glow),
        "pole" | "board" | "beacon" | "rail_l" | "rail_r" | "boiler" | "ring_back"
        | "ring_front" | "face" | "funnel" | "funnel_cap" | "dome" | "window_a" | "window_b"
        | "window_c" | "side_l" | "side_r" | "end_l" | "end_r" | "rim" | "hub" | "crown"
        | "eye_a" | "eye_b" | "pupil_a" | "pupil_b" | "knob" => Some(Finish::Gloss),
        _ => None,
    }
}

fn spawn_model(
    out: &mut ChildSpawnerCommands,
    kit: &mut Kit,
    shelf: &Shelf,
    name: &'static str,
    t: Transform,
    tint: impl Fn(&str) -> Option<Col>,
) {
    for (i, part) in shelf.get(name).iter().enumerate() {
        let finish = finish_of(&part.name).unwrap_or(if part.gloss {
            Finish::Gloss
        } else {
            Finish::Matte
        });
        let mut color = tint(&part.name).unwrap_or(part.color);
        if finish == Finish::Glow {
            color.a = 0.95;
        }
        out.spawn(kit.part(
            Shape::Model(name, i as u16),
            Paint::new(finish, color),
            t,
        ));
    }
}

fn plain(
    out: &mut ChildSpawnerCommands,
    kit: &mut Kit,
    shelf: &Shelf,
    name: &'static str,
    t: Transform,
) {
    spawn_model(out, kit, shelf, name, t, |_| None);
}

fn scenery(w: &mut ChildSpawnerCommands, kit: &mut Kit, shelf: &Shelf) {
    let grass = Col::new(0.45, 0.78, 0.3, 1.0);
    put(
        w,
        kit,
        Shape::Plane,
        Finish::Floor,
        grass,
        cube(at(0.0, -0.02, -20.0), at(300.0, 1.0, 120.0)),
    );
    tiled(
        w,
        kit,
        "grass",
        Vec2::new(160.0, 70.0),
        3.5,
        Transform::from_xyz(0.0, 0.0, -22.0),
    );
    plain(w, kit, shelf, "trains_track", Transform::default());
    tiled(
        w,
        kit,
        "gravel",
        Vec2::new(160.0, 2.8),
        1.4,
        Transform::from_xyz(0.0, 0.102, 0.0),
    );
    platform(w, kit, shelf);
    plain(
        w,
        kit,
        shelf,
        "trains_station",
        Transform::from_translation(at(-8.0, 0.0, -5.5)),
    );
    trees(w, kit, shelf);
    hills(w, kit, shelf);
}

fn platform(w: &mut ChildSpawnerCommands, kit: &mut Kit, shelf: &Shelf) {
    let (z0, z1) = (2.2, 8.5);
    plain(w, kit, shelf, "trains_platform", Transform::default());
    tiled(
        w,
        kit,
        "platform",
        Vec2::new(60.0, z1 - z0),
        2.0,
        Transform::from_xyz(0.0, PLATFORM_Y + 0.003, (z0 + z1) / 2.0),
    );
    for x in [-7.0f32, 7.5] {
        let t = Transform::from_translation(at(x, PLATFORM_Y, 7.2));
        plain(w, kit, shelf, "trains_lamp", t);
    }
    let bench = Transform::from_translation(at(-3.2, PLATFORM_Y, 7.6));
    plain(w, kit, shelf, "trains_bench", bench);
    let sign = Transform::from_translation(at(4.4, PLATFORM_Y, 7.4));
    plain(w, kit, shelf, "trains_sign", sign);
}

fn trees(w: &mut ChildSpawnerCommands, kit: &mut Kit, shelf: &Shelf) {
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
        let name = if i % 2 == 0 {
            "trains_tree_round"
        } else {
            "trains_tree_pine"
        };
        let t = Transform::from_translation(at(x, 0.0, z)).with_scale(Vec3::splat(s));
        spawn_model(w, kit, shelf, name, t, |part| {
            (part == "crown").then_some(c)
        });
    }
}

fn hills(w: &mut ChildSpawnerCommands, kit: &mut Kit, shelf: &Shelf) {
    let tones = [rgb(110, 200, 110), rgb(90, 185, 120), rgb(130, 210, 100)];
    for i in 0..7 {
        let k = i as f32;
        let x = -48.0 + k * 16.0;
        let t = cube(
            at(x, -2.0, -38.0 - (i % 2) as f32 * 7.0),
            at(32.0, 14.0 + (i % 3) as f32 * 4.0, 20.0),
        );
        spawn_model(w, kit, shelf, "trains_hill", t, |_| Some(tones[i % 3]));
    }
}

fn train_parts(t: &mut ChildSpawnerCommands, kit: &mut Kit, shelf: &Shelf, cars: usize) {
    let len = ENGINE_LEN + cars as f32 * CARRIAGE_LEN;
    let ex = len / 2.0 - ENGINE_LEN / 2.0;
    plain(
        t,
        kit,
        shelf,
        "trains_engine",
        Transform::from_translation(at(ex, 0.0, 0.0)),
    );
    for (dx, r) in [(-0.95, 0.4), (-0.15, 0.4), (1.05, 0.28)] {
        for side in [-1.0f32, 1.0] {
            wheel(t, kit, shelf, at(ex + dx, RAIL_Y + r, 0.66 * side), r);
        }
    }
    let palette = [
        rgb(255, 200, 50),
        rgb(70, 160, 240),
        rgb(95, 200, 100),
        rgb(240, 120, 190),
    ];
    for c in 0..cars {
        let cx = ex - ENGINE_LEN / 2.0 - CARRIAGE_LEN * (c as f32 + 0.5);
        let color = palette[c % palette.len()];
        spawn_model(
            t,
            kit,
            shelf,
            "trains_carriage",
            Transform::from_translation(at(cx, 0.0, 0.0)),
            |part| matches!(part, "side_l" | "side_r" | "end_l" | "end_r").then_some(color),
        );
        for dx in [-0.8f32, 0.8] {
            for side in [-1.0f32, 1.0] {
                wheel(t, kit, shelf, at(cx + dx, RAIL_Y + 0.26, 0.66 * side), 0.26);
            }
        }
    }
}

fn wheel(t: &mut ChildSpawnerCommands, kit: &mut Kit, shelf: &Shelf, pos: Vec3, r: f32) {
    t.spawn((
        Role::Wheel(r),
        Transform::from_translation(pos)
            .with_rotation(Quat::from_rotation_x(FRAC_PI_2))
            .with_scale(Vec3::new(r, 1.0, r)),
        Visibility::default(),
    ))
    .with_children(|w| plain(w, kit, shelf, "trains_wheel", Transform::default()));
}

fn spawn_rider(out: &mut ChildSpawnerCommands, kit: &mut Kit, look: u8, arms_up: bool) {
    let Some(shelf) = kit.shelf(PEOPLE) else {
        return;
    };
    let mut m = Model::new();
    let st = Stance {
        arms_up,
        ..Default::default()
    };
    body(&mut m, &shelf, &guest_look(look, false), st);
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
    let Some(shelf) = kit.shelf(MODELS) else {
        return;
    };
    if !root.built {
        root.built = true;
        commands
            .entity(root_e)
            .with_children(|w| scenery(w, &mut kit, &shelf));
    }
    let cars = carriages(g.riders.len());
    if root.carriages != cars {
        root.carriages = cars;
        if let Some((train, ..)) = nodes.iter().find(|n| *n.1 == Role::Train) {
            let mut e = commands.entity(train);
            e.despawn_children();
            e.with_children(|t| train_parts(t, &mut kit, &shelf, cars));
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
    let chimney = Vec3::new(
        center + len / 2.0 - ENGINE_LEN / 2.0 + CHIMNEY_X - 0.1,
        CHIMNEY_Y,
        0.0,
    );
    for (role, mut t, mut v, mut part, mut mat) in puffs.iter_mut() {
        let Role::Smoke(k) = *role else {
            continue;
        };
        let phase = (g.t * 0.7 + k as f32 / SMOKE as f32).rem_euclid(1.0);
        let kick = 1.0 + g.puff_kick.min(1.5) * 0.7;
        let drift = -phase * (1.4 + speed * 0.09);
        let size = (0.3 + phase * 0.75) * kick;
        t.set_if_neq(
            Transform::from_translation(chimney + Vec3::new(drift, phase * 1.5 * kick, 0.0))
                .with_scale(Vec3::splat(size)),
        );
        let a = faded(0.55 * (1.0 - phase) * (0.4 + 0.6 * (phase * 6.0).min(1.0)));
        let on = a > 0.0;
        v.set_if_neq(if on {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        });
        kit.repaint(
            &mut part,
            &mut mat,
            Paint::new(Finish::Ghost, Col::new(0.97, 0.97, 1.0, a.max(0.125))),
        );
    }
}

pub fn rider_transform(
    g: &Trains,
    i: usize,
    state: &kg_core::games::trains::Rider,
) -> (Transform, bool) {
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
                    .with_scale(Vec3::new(
                        1.0 - 0.12 * stretch,
                        1.0 + 0.2 * stretch,
                        1.0 - 0.12 * stretch,
                    )),
                true,
            )
        }
        Off(t) => {
            let land = (-t * 9.0).exp();
            let cheer = if matches!(
                g.phase,
                kg_core::games::trains::Phase::Resolve | kg_core::games::trains::Phase::Depart
            ) {
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
