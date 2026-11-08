use super::{RunnerPlay, SHEET};
use crate::game3d::*;
use bevy::animation::graph::AnimationGraphHandle;
use bevy::animation::transition::AnimationTransitions;
use bevy::ecs::hierarchy::ChildSpawnerCommands;
use bevy::pbr::{DistanceFog, FogFalloff};
use bevy::prelude::*;
use gcad_bevy::GcadAssets;
use kg_core::engine::View;
use kg_core::games::runner::State as Phase;
use kg_core::games::runner::*;
use kg_core::geom::{Color as Col, WHITE};
use std::collections::HashMap;
use std::time::Duration;

const SKY: Col = Col::new(0.66, 0.88, 0.99, 1.0);
const SKY_DEPTH: f32 = 140.0;
const SLOT: f32 = 9.0;
const SLOTS: usize = 14;
const TREE_SLOT: f32 = 13.0;
const TREES: usize = 9;
const BEHIND: f32 = 14.0;
const WHOLE: [f32; 4] = [0.0, 0.0, 1.0, 1.0];
const ROAD_W: f32 = 5.4;
const ROAD_PIECE: f32 = 9.0;
const ROAD_SINK: f32 = 0.04;
const WALK_W: f32 = 2.0;

pub struct RunnerWorld;

impl Plugin for RunnerWorld {
    fn build(&self, app: &mut App) {
        app.init_resource::<Models>().add_systems(
            Update,
            (
                spawn.run_if(opened(World3d::Runner)),
                sync.run_if(is_open(World3d::Runner)),
                animate_hero.run_if(is_open(World3d::Runner)),
                mark_loaded.run_if(is_open(World3d::Runner)),
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
    Shade,
    Obstacle(usize, usize),
    Coin(usize),
    Building(usize),
    Look(usize, usize),
    Tree(usize),
}

type Parts<'w, 's> = Query<
    'w,
    's,
    (
        &'static Role,
        &'static mut Transform,
        &'static mut Visibility,
    ),
    (With<Part>, Without<Eye>),
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

fn matte(c: Col) -> Paint {
    Paint::new(Finish::Matte, c)
}

fn node(out: &mut ChildSpawnerCommands, role: Role, at: Transform) -> Entity {
    out.spawn((role, at, Visibility::default())).id()
}

fn spawn(
    mut commands: Commands,
    mut kit: Kit,
    loaded: Res<Models>,
    assets: Option<Res<AssetServer>>,
    graphs: Option<ResMut<Assets<AnimationGraph>>>,
) {
    let models = match (assets, graphs) {
        (Some(a), Some(mut g)) if loaded.scenes.is_empty() => Models::load(&a, &mut g),
        _ => loaded.clone(),
    };
    commands.insert_resource(models.clone());
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
            .with_children(|p| kid(p, &models));
        w.spawn((
            Role::Shade,
            kit.part(
                Shape::Disc,
                Paint::new(Finish::Ghost, Col::new(0.05, 0.05, 0.15, 0.32)),
                Transform::default(),
            ),
        ));
        for i in 0..SLOTS * 2 {
            let b = node(w, Role::Building(i), Transform::default());
            w.commands().entity(b).with_children(|c| {
                for (v, name) in BUILDINGS.iter().enumerate() {
                    let look = node(c, Role::Look(i, v), Transform::default());
                    c.commands().entity(look).with_children(|m| {
                        model(m, &models, name, Transform::default());
                    });
                }
            });
        }
        for i in 0..TREES * 2 {
            let t = node(w, Role::Tree(i), Transform::default());
            w.commands().entity(t).with_children(|c| {
                prop(c, &models, "tree", 3.6, Vec3::ZERO);
            });
        }
    });
}

const HERO_H: f32 = 1.8;
const TRAIN_H: f32 = 1.9;
const STRIDE_METRES: f32 = 6.0;
const HERO_CLIPS: [&str; 6] = ["fail", "happy", "idle", "jump", "run", "slide"];
pub const PROPS: [(&str, Option<f32>); 13] = [
    ("barrier", None),
    ("hurdle", None),
    ("gate", None),
    ("train", None),
    ("coin", None),
    ("tree", None),
    ("arch", None),
    ("road", Some(0.01)),
    ("building1", Some(0.01)),
    ("building2", Some(0.01)),
    ("building3", Some(0.01)),
    ("building4", Some(0.01)),
    ("building5", Some(0.01)),
];
const BUILDINGS: [&str; 5] = [
    "building1",
    "building2",
    "building3",
    "building4",
    "building5",
];

#[derive(Resource, Default, Clone)]
pub struct Models {
    scenes: HashMap<&'static str, Handle<WorldAsset>>,
    graph: Option<Handle<AnimationGraph>>,
    clips: Vec<AnimationNodeIndex>,
}

impl Models {
    pub fn load(assets: &AssetServer, graphs: &mut Assets<AnimationGraph>) -> Self {
        let hero = crate::gcad::settings(None);
        let mut scenes = HashMap::new();
        scenes.insert("hero", assets.load_gcad("models/hero.gcad", hero.clone()));
        for (name, scale) in PROPS {
            scenes.insert(
                name,
                assets.load_gcad(
                    format!("models/prop_{name}.gcad"),
                    crate::gcad::settings(scale),
                ),
            );
        }
        let clips = HERO_CLIPS
            .iter()
            .map(|c| assets.load_gcad_clip("models/hero.gcad", c, hero.clone()));
        let (graph, clips) = AnimationGraph::from_clips(clips);
        Self {
            scenes,
            graph: Some(graphs.add(graph)),
            clips,
        }
    }

    pub fn names(&self) -> impl Iterator<Item = &'static str> + '_ {
        self.scenes.keys().copied()
    }

    pub fn scene(&self, name: &str) -> Option<&Handle<WorldAsset>> {
        self.scenes.get(name)
    }
}

pub fn hero_clip(g: &Runner) -> &'static str {
    if g.state == Phase::Dead {
        "fail"
    } else if g.state == Phase::Won && g.end_t > 0.4 {
        "happy"
    } else if g.sliding() {
        "slide"
    } else if g.y > 0.05 {
        "jump"
    } else if g.state == Phase::Ready || g.speed < 0.5 {
        "idle"
    } else {
        "run"
    }
}

fn model(c: &mut ChildSpawnerCommands, models: &Models, name: &'static str, at: Transform) {
    if let Some(scene) = models.scenes.get(name) {
        c.spawn((WorldAssetRoot(scene.clone()), at));
    }
}

fn kid(p: &mut ChildSpawnerCommands, models: &Models) {
    let body = node(p, Role::Body, Transform::default());
    p.commands().entity(body).with_children(|b| {
        model(
            b,
            models,
            "hero",
            Transform::from_rotation(Quat::from_rotation_y(std::f32::consts::PI))
                .with_scale(Vec3::splat(HERO_H)),
        );
    });
}

fn prop(c: &mut ChildSpawnerCommands, models: &Models, name: &'static str, h: f32, at: Vec3) {
    model(
        c,
        models,
        name,
        Transform::from_translation(at).with_scale(Vec3::splat(h)),
    );
}

fn obstacle(c: &mut ChildSpawnerCommands, models: &Models, kind: Kind) {
    match kind {
        Kind::Barrier => prop(c, models, "barrier", 1.25, Vec3::ZERO),
        Kind::Hurdle => prop(c, models, "hurdle", 0.8, Vec3::ZERO),
        Kind::Overhead => prop(c, models, "gate", 2.5, Vec3::ZERO),
        Kind::Train => prop(c, models, "train", TRAIN_H, Vec3::ZERO),
    }
}

fn clip_secs(
    graphs: &Assets<AnimationGraph>,
    clips: &Assets<AnimationClip>,
    graph: &Handle<AnimationGraph>,
    node: AnimationNodeIndex,
) -> f32 {
    graphs
        .get(graph)
        .and_then(|g| g.get(node))
        .and_then(|n| match &n.node_type {
            bevy::animation::graph::AnimationNodeType::Clip(c) => clips.get(c),
            _ => None,
        })
        .map_or(1.0, |c| c.duration())
}

fn mark_loaded(
    mut frame: NonSendMut<crate::Frame>,
    models: Res<Models>,
    assets: Option<Res<AssetServer>>,
) {
    let Some(p) = frame
        .play_mut()
        .and_then(|p| p.as_any().downcast_mut::<RunnerPlay>())
    else {
        return;
    };
    if p.loaded {
        return;
    }
    p.loaded = match (assets, models.scenes.get("hero")) {
        (Some(a), Some(h)) => a.is_loaded_with_dependencies(h),
        _ => true,
    };
}

fn animate_hero(
    mut commands: Commands,
    frame: NonSend<crate::Frame>,
    models: Res<Models>,
    fresh: Query<Entity, (With<AnimationPlayer>, Without<AnimationGraphHandle>)>,
    mut players: Query<(&mut AnimationPlayer, &mut AnimationTransitions)>,
    clips: Option<Res<Assets<AnimationClip>>>,
    graphs: Option<Res<Assets<AnimationGraph>>>,
) {
    let (Some(clips), Some(graphs)) = (clips, graphs) else {
        return;
    };
    let Some(graph) = models.graph.clone() else {
        return;
    };
    for e in &fresh {
        commands.entity(e).insert((
            AnimationGraphHandle(graph.clone()),
            AnimationTransitions::new(),
        ));
    }
    let Some(p) = playing::<RunnerPlay>(&frame) else {
        return;
    };
    let g = &p.g;
    let want = hero_clip(g);
    let Some(at) = HERO_CLIPS.iter().position(|c| *c == want) else {
        return;
    };
    let node = models.clips[at];
    for (mut player, mut tr) in &mut players {
        if tr.get_main_animation() != Some(node) {
            let a = tr.play(&mut player, node, Duration::from_millis(120));
            match want {
                "run" | "idle" => {
                    a.repeat();
                }
                "jump" | "slide" => {
                    let secs = clip_secs(&graphs, &clips, &graph, node);
                    let wanted = if want == "jump" {
                        2.0 * JUMP_V / GRAVITY
                    } else {
                        SLIDE_TIME
                    };
                    a.set_speed(secs / wanted);
                }
                _ => {}
            }
        }
        if want == "run" {
            if let Some(a) = player.animation_mut(node) {
                let secs = clip_secs(&graphs, &clips, &graph, node);
                a.set_speed((secs * g.speed / STRIDE_METRES).clamp(0.6, 3.0));
            }
        }
    }
}

fn course(c: &mut ChildSpawnerCommands, kit: &mut Kit, models: &Models, g: &Runner) {
    let end = g.length + FINISH_RUN + 130.0;
    let len = end + 20.0;
    let mid = (20.0 - end) / 2.0;
    c.spawn(kit.part(
        Shape::Plane,
        Paint::new(Finish::Floor, Col::new(0.55, 0.82, 0.4, 1.0)),
        Transform::from_xyz(0.0, -0.04, mid).with_scale(Vec3::new(260.0, 1.0, len)),
    ));
    let mut z = 20.0 - ROAD_PIECE / 2.0;
    while z > -end {
        prop(c, models, "road", 1.0, Vec3::new(0.0, -ROAD_SINK, z));
        z -= ROAD_PIECE;
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
                .with_children(|o| obstacle(o, models, kind));
        }
    }
    for (i, coin) in g.coins_list.iter().enumerate() {
        let n = node(
            c,
            Role::Coin(i),
            Transform::from_xyz(lane_x(coin.lane as i32 - 1), coin.h, -coin.at),
        );
        c.commands().entity(n).with_children(|k| {
            prop(k, models, "coin", 0.55, Vec3::new(0.0, -0.275, 0.0));
        });
    }
    let z = -g.length;
    prop(c, models, "arch", 4.8, Vec3::new(0.0, 0.0, z));
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
    models: Res<Models>,
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
            c.with_children(|c| course(c, &mut kit, &models, g));
        }
        return;
    }
    pose_nodes(g, &mut nodes);
    pose_parts(g, &view, &cam, &mut parts);
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

fn pose_nodes(g: &Runner, nodes: &mut Nodes<'_, '_>) {
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
                let sq = g.squash;
                t.set_if_neq(Transform {
                    translation: Vec3::ZERO,
                    rotation: Quat::IDENTITY,
                    scale: Vec3::new(1.0 + sq * 0.14, 1.0 - sq * 0.2, 1.0 + sq * 0.14),
                });
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
                    rotation: Quat::from_rotation_y(spin),
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
            Role::Building(i) => {
                let side = if i % 2 == 0 { -1.0 } else { 1.0 };
                let k = i / 2;
                let z = ring(k, SLOTS, SLOT, g.dist, 0.0);
                let off = ROAD_W / 2.0 + WALK_W + 2.6;
                v.set_if_neq(shown(z < g.length + FINISH_RUN + 20.0));
                t.set_if_neq(Transform::from_xyz(side * off, 0.0, -z).with_rotation(
                    Quat::from_rotation_y(if side > 0.0 {
                        std::f32::consts::PI
                    } else {
                        0.0
                    }),
                ));
            }
            Role::Look(i, look) => {
                let k = i / 2;
                let n = hash(slot_index(k, SLOTS, SLOT, g.dist) * 2 + (i % 2) as i64);
                v.set_if_neq(shown(n % BUILDINGS.len() == look));
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

fn pose_parts(g: &Runner, view: &View, cam: &Cam, parts: &mut Parts<'_, '_>) {
    for (role, mut t, mut v) in parts.iter_mut() {
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
            _ => {}
        }
    }
}

#[cfg(test)]
pub fn scenery(app: &mut App) -> usize {
    let w = app.world_mut();
    let all: Vec<Entity> = w
        .query::<(Entity, &Role)>()
        .iter(w)
        .filter(|(_, r)| matches!(r, Role::Building(_) | Role::Tree(_)))
        .map(|(e, _)| e)
        .collect();
    all.into_iter()
        .filter(|e| crate::game3d::visible(w, *e))
        .count()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game3d::testing;
    use crate::games::Play;
    use kg_core::engine::{Dir, Input};

    fn gcad_source(name: &str) -> String {
        let text = kg_core::assets::read(&format!("models/{name}.gcad"))
            .unwrap_or_else(|| panic!("models/{name}.gcad missing"));
        gcad_bevy::expand_includes(&String::from_utf8(text).unwrap(), &mut |inc| {
            kg_core::assets::read(&format!("models/{inc}"))
                .map(|b| String::from_utf8(b).unwrap())
                .ok_or_else(|| gcad_bevy::GcadError(format!("models/{inc} missing")))
        })
        .unwrap()
    }

    fn build(name: &str, scale: Option<f32>) -> gcad_bevy::Built {
        gcad_bevy::build(&gcad_source(name), &crate::gcad::settings(scale))
            .unwrap_or_else(|e| panic!("{name}: {e}"))
    }

    fn extent(model: &gcad_bevy::Built) -> Vec3 {
        let (lo, hi) = model.bounds.unwrap();
        (Vec3::from(hi) - Vec3::from(lo)) * model.fit.unwrap().0
    }

    #[test]
    fn every_prop_builds_from_gcad_at_the_size_the_runner_spawns_it() {
        let kernel = [
            ("barrier", [1.052, 1.0, 0.193]),
            ("hurdle", [1.682, 1.0, 0.628]),
            ("gate", [0.669, 1.0, 0.105]),
            ("train", [0.723, 1.0, 4.859]),
            ("coin", [1.0, 1.0, 0.16]),
            ("arch", [0.761, 1.0, 0.113]),
            ("road", [9.36, 0.24, 9.0]),
            ("building1", [5.66, 5.32, 7.84]),
            ("building2", [5.66, 7.82, 7.84]),
            ("building3", [5.66, 10.32, 7.84]),
            ("building4", [5.66, 12.82, 7.84]),
            ("building5", [5.66, 7.82, 7.84]),
        ];
        for (name, scale) in PROPS {
            let model = build(&format!("prop_{name}"), scale);
            let tris = model.triangles();
            assert!((1..=4_000).contains(&tris), "{name}: {tris} triangles");
            for body in &model.bodies {
                assert!(
                    !body.indices.is_empty(),
                    "{name}: body {} is empty",
                    body.name
                );
                assert!(
                    !body.parts.is_empty(),
                    "{name}: body {} has no parts",
                    body.name
                );
                for part in &body.parts {
                    assert!(!part.name.is_empty(), "{name}: unnamed part");
                    assert!(part.index_count > 0, "{name}: part {} empty", part.name);
                }
            }
            let Some((_, want)) = kernel.iter().find(|(n, _)| *n == name) else {
                continue;
            };
            let got = extent(&model);
            for i in 0..3 {
                assert!(
                    (got[i] - want[i]).abs() <= 0.02 * want[i].max(0.2),
                    "{name}: extent {got} against the kernel's {want:?}"
                );
            }
        }
    }

    fn loading_app() -> App {
        let root = kg_core::assets::dirs()
            .into_iter()
            .find(|d| d.join("models").is_dir())
            .expect("assets/models");
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            bevy::asset::AssetPlugin {
                file_path: root.canonicalize().unwrap().to_string_lossy().into_owned(),
                ..default()
            },
            bevy::transform::TransformPlugin,
            bevy::mesh::MeshPlugin,
            bevy::animation::AnimationPlugin,
            bevy::world_serialization::WorldSerializationPlugin,
        ))
        .init_asset::<StandardMaterial>()
        .init_asset::<Image>()
        .add_plugins(crate::gcad::ModelsPlugin);
        app
    }

    #[test]
    fn every_model_the_runner_loads_is_warmed_at_startup() {
        let mut app = loading_app();
        app.update();
        let models = app.world().resource::<Models>().clone();
        let mut names: Vec<&str> = models.names().collect();
        names.sort();
        let mut want: Vec<&str> = PROPS.iter().map(|(p, _)| *p).chain(["hero"]).collect();
        want.sort();
        assert_eq!(names, want);
        let started = std::time::Instant::now();
        loop {
            app.update();
            let server = app.world().resource::<AssetServer>();
            let pending: Vec<&str> = want
                .iter()
                .copied()
                .filter(|n| {
                    let h = models.scene(n).unwrap();
                    if let bevy::asset::LoadState::Failed(e) = server.load_state(h) {
                        panic!("{n}: {e}");
                    }
                    !server.is_loaded_with_dependencies(h)
                })
                .collect();
            if pending.is_empty() {
                break;
            }
            assert!(started.elapsed().as_secs() < 120, "{pending:?} did not load");
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        let clips = app.world().resource::<Assets<AnimationClip>>();
        let graphs = app.world().resource::<Assets<AnimationGraph>>();
        let graph = graphs.get(models.graph.as_ref().unwrap()).unwrap();
        for node in &models.clips {
            let Some(bevy::animation::graph::AnimationNodeType::Clip(clip)) =
                graph.get(*node).map(|n| &n.node_type)
            else {
                panic!("node {node:?} is not a clip");
            };
            let clip = clips.get(clip).expect("hero clip loaded");
            assert!(clip.duration() > 0.3 && !clip.curves().is_empty());
        }
        let world = app.world_mut();
        let hero = world
            .resource::<Assets<bevy::world_serialization::WorldAsset>>()
            .get(models.scene("hero").unwrap())
            .unwrap();
        let mut q = hero.world.try_query::<(&gcad_bevy::GcadModel, &Transform)>().unwrap();
        let (model, at) = q.single(&hero.world).unwrap();
        let height = (model.bounds_max.y - model.bounds_min.y) * at.scale.y;
        assert!((height - 1.0).abs() < 1e-3, "hero loads {height} high");
        assert_eq!(model.clips, HERO_CLIPS);
    }

    fn assets_root() -> std::path::PathBuf {
        kg_core::assets::dirs()
            .into_iter()
            .find(|d| d.join("models").is_dir())
            .expect("assets/models")
    }

    #[test]
    fn a_fresh_bake_of_the_models_passes_the_check() {
        let root = assets_root();
        let copy = std::env::temp_dir().join(format!("kg-bake-{}", std::process::id()));
        std::fs::create_dir_all(copy.join("models")).unwrap();
        for entry in std::fs::read_dir(root.join("models")).unwrap() {
            let path = entry.unwrap().path();
            std::fs::copy(&path, copy.join("models").join(path.file_name().unwrap())).unwrap();
        }
        let before = gcad_bevy::bake::check(&copy, crate::gcad::bake_settings);
        assert!(before.iter().all(|p| p.contains("index")), "{before:?}");
        let baked = gcad_bevy::bake::bake(&copy, crate::gcad::bake_settings).unwrap();
        assert_eq!(baked.index.entries.len(), PROPS.len() + 1);
        let problems = gcad_bevy::bake::check(&copy, crate::gcad::bake_settings);
        std::fs::write(copy.join("models/hero.gcad"), "").unwrap();
        let stale = gcad_bevy::bake::check(&copy, crate::gcad::bake_settings);
        std::fs::remove_dir_all(&copy).unwrap();
        assert!(problems.is_empty(), "{problems:#?}");
        assert!(stale.iter().any(|p| p.contains("hero")), "{stale:?}");
    }

    #[test]
    fn the_bake_key_is_the_key_models_load_asks_for() {
        let root = assets_root();
        let mut assets: Vec<(String, gcad_bevy::GcadSettings)> =
            vec![("models/hero.gcad".into(), crate::gcad::settings(None))];
        for (name, scale) in PROPS {
            assets.push((format!("models/prop_{name}.gcad"), crate::gcad::settings(scale)));
        }
        for (asset, loaded) in assets {
            let baked = crate::gcad::bake_settings(&asset).unwrap_or_else(|| panic!("{asset} not baked"));
            assert_eq!(baked, loaded, "{asset}");
            let text = std::fs::read_to_string(root.join(&asset)).unwrap();
            let sources = gcad_bevy::Sources::read(&asset, &text, |n| std::fs::read(root.join(n)).ok()).unwrap();
            assert_eq!(
                gcad_bevy::bake::key_of(&root, &asset, &baked).unwrap(),
                gcad_bevy::cache_key(&sources, &loaded).unwrap(),
                "{asset}"
            );
        }
    }

    #[test]
    fn every_model_the_runner_loads_is_on_disk() {
        for name in PROPS
            .iter()
            .map(|(p, _)| format!("prop_{p}"))
            .chain(["hero".to_string()])
        {
            assert!(
                kg_core::assets::read(&format!("models/{name}.gcad")).is_some(),
                "models/{name}.gcad missing"
            );
        }
    }

    #[test]
    fn the_hero_builds_from_gcad_with_every_clip_on_its_rig() {
        let model = build("hero", None);
        assert_eq!(model.bodies.len(), 1);
        let body = &model.bodies[0];
        assert_eq!(body.joints.len(), 15);
        assert_eq!(model.clip_names(), HERO_CLIPS);
        for clip in &body.clips {
            assert!(clip.duration > 0.3, "{} lasts {}", clip.name, clip.duration);
            assert!(
                clip.rotations.len() >= 10,
                "{} keys {} bones",
                clip.name,
                clip.rotations.len()
            );
        }
        assert!(
            model.triangles() <= 14_000,
            "{} triangles",
            model.triangles()
        );
        let (lo, hi) = model.bounds.unwrap();
        let height = hi[1] - lo[1];
        assert!((100.0..160.0).contains(&height), "hero is {height} tall");
        assert!(lo[1].abs() < 1.0, "hero feet at {}", lo[1]);
        let run = body.clips.iter().find(|c| c.name == "run").unwrap();
        assert!(
            run.rotations
                .iter()
                .any(|t| t.values.windows(2).any(|w| w[0] != w[1])),
            "the run clip does not move the rig"
        );
    }

    #[test]
    fn the_tree_builds_from_gcad_inside_its_budget() {
        let model = build("prop_tree", None);
        assert!(
            model.triangles() <= 4_000,
            "{} triangles",
            model.triangles()
        );
        let (lo, hi) = model.bounds.unwrap();
        let height = hi[1] - lo[1];
        assert!((150.0..300.0).contains(&height), "tree is {height} tall");
    }

    #[test]
    fn the_hero_clip_follows_the_game_state() {
        let mut g = Runner::with_level(1);
        g.state = Phase::Ready;
        assert_eq!(hero_clip(&g), "idle");
        g.state = Phase::Playing;
        g.speed = 6.0;
        assert_eq!(hero_clip(&g), "run");
        g.y = 1.0;
        assert_eq!(hero_clip(&g), "jump");
        g.state = Phase::Dead;
        assert_eq!(hero_clip(&g), "fail");
    }

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
    fn falling_and_sliding_are_left_to_the_hero_clips() {
        let mut app = open(fresh(1));
        for (dead, slide) in [(true, 0.0), (false, 0.3)] {
            {
                let g = &mut runner(&mut app).g;
                g.state = if dead { Phase::Dead } else { Phase::Playing };
                g.end_t = 1.0;
                g.slide_t = slide;
                g.y = 0.0;
            }
            testing::step(&mut app, 1);
            let w = app.world_mut();
            let t = *w
                .query::<(&Role, &Transform)>()
                .iter(w)
                .find(|(r, _)| **r == Role::Body)
                .unwrap()
                .1;
            assert!(
                t.rotation.angle_between(Quat::IDENTITY) < 1e-3,
                "body tilts {:?}",
                t.rotation
            );
            assert!(
                t.translation.y.abs() < 1e-3,
                "body dips {}",
                t.translation.y
            );
        }
    }

    #[test]
    fn the_body_does_not_shake_while_running_flat_out() {
        let mut app = open(fresh(20));
        let mut heights = vec![];
        for i in 0..40 {
            {
                let g = &mut runner(&mut app).g;
                g.state = Phase::Playing;
                g.speed = speed_of(20, 0);
                g.dist = 20.0 + i as f32 * g.speed / 60.0;
            }
            testing::step(&mut app, 1);
            let w = app.world_mut();
            let y = w
                .query::<(&Role, &Transform)>()
                .iter(w)
                .find(|(r, _)| **r == Role::Body)
                .unwrap()
                .1
                .translation
                .y;
            heights.push(y);
        }
        let swing = heights.iter().cloned().fold(f32::MIN, f32::max)
            - heights.iter().cloned().fold(f32::MAX, f32::min);
        assert!(swing < 1e-3, "body bobs {swing} on top of the run clip");
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
