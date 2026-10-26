use super::{HelixPlay, SHEET};
use crate::game3d::*;
use crate::game3d::{Lens, Rig};
use bevy::ecs::hierarchy::ChildSpawnerCommands;
use bevy::light::NotShadowCaster;
use bevy::prelude::*;
use kg_core::engine::View;
use kg_core::games::helix::State as Phase;
use kg_core::games::helix::*;
use kg_core::geom::{hsv, Color as Col, WHITE};
use std::f32::consts::{FRAC_PI_2, TAU};

const SKY: Col = Col::new(0.55, 0.78, 0.98, 1.0);
const SKY_DEPTH: f32 = 2.4;
const CLOUD_DEPTH: f32 = 1.6;
const CLOUDS: usize = 6;
const TRAIL: usize = 6;
const TILE_CROP: [f32; 4] = [0.14, 0.14, 0.72, 0.72];
const WHOLE: [f32; 4] = [0.0, 0.0, 1.0, 1.0];
const FADE_STEPS: f32 = 10.0;
const BALL: Col = Col::new(1.0, 0.52, 0.12, 1.0);
const POLE: Col = Col::new(0.86, 0.8, 0.95, 1.0);
const STEP: f32 = TAU / SEGMENTS as f32;

pub struct HelixWorld;

impl Plugin for HelixWorld {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                spawn.run_if(opened(World3d::Helix)),
                sync.run_if(is_open(World3d::Helix)),
            )
                .chain()
                .in_set(Sync3d),
        );
    }
}

#[derive(Component)]
pub struct Root {
    serial: u64,
    splats: usize,
}

#[derive(Component, Clone, Copy, PartialEq, Eq, Debug)]
pub enum Role {
    Sky,
    Cloud(usize),
    Ball,
    Trail(usize),
    DeadSplat,
    Segment(usize),
    Splat,
    Pole,
}

#[derive(Component, Clone, Copy, PartialEq, Eq, Debug)]
pub enum Node {
    Stage,
    Tower,
    Ring(usize),
}

fn node(out: &mut ChildSpawnerCommands, n: Node, at: Transform) -> Entity {
    out.spawn((n, at, Visibility::default())).id()
}

fn sprite(name: &'static str, crop: [f32; 4]) -> Skin {
    Skin::Sprite(SHEET, name, crop)
}

fn flat_decal() -> Quat {
    Quat::from_rotation_x(-FRAC_PI_2)
}

fn spawn(mut commands: Commands, mut kit: Kit) {
    let world = spawn_world(
        &mut commands,
        Root {
            serial: 0,
            splats: 0,
        },
        World3d::Helix,
        Lighting {
            ambient: 420.0,
            illuminance: 9000.0,
            color: WHITE,
            toward: Vec3::new(-0.45, -1.0, -0.55),
        },
    );
    commands.entity(world.eye).with_children(|c| {
        piece(
            c,
            &mut kit,
            Role::Sky,
            Shape::Quad,
            Paint::skinned(Finish::Backdrop, WHITE, sprite("bg", WHOLE)),
            Transform::default(),
        );
        for i in 0..CLOUDS {
            let name = if i % 2 == 0 { "cloud1" } else { "cloud2" };
            piece(
                c,
                &mut kit,
                Role::Cloud(i),
                Shape::Quad,
                Paint::skinned(Finish::Sticker, WHITE.with_alpha(0.85), sprite(name, WHOLE)),
                Transform::default(),
            );
        }
    });
    commands.entity(world.root).with_children(|w| {
        let stage = node(w, Node::Stage, Transform::default());
        w.commands().entity(stage).with_children(|s| {
            node(s, Node::Tower, Transform::default());
            piece(
                s,
                &mut kit,
                Role::Ball,
                Shape::Ball,
                Paint::new(Finish::Satin, BALL),
                Transform::default(),
            );
            for k in 0..TRAIL {
                piece(
                    s,
                    &mut kit,
                    Role::Trail(k),
                    Shape::Ball,
                    Paint::new(Finish::Glow, Col::new(1.0, 0.6, 0.2, 0.3)),
                    Transform::default(),
                );
            }
            piece(
                s,
                &mut kit,
                Role::DeadSplat,
                Shape::Quad,
                Paint::skinned(Finish::Sticker, WHITE, sprite("splat", WHOLE)),
                Transform::default(),
            );
        });
    });
}

fn piece(
    out: &mut ChildSpawnerCommands,
    kit: &mut Kit,
    role: Role,
    shape: Shape,
    paint: Paint,
    at: Transform,
) {
    out.spawn((kit.part(shape, paint, at), role));
}

fn tile_paint(seg: Seg, hue: f32, alpha: f32) -> Paint {
    let name = if seg == Seg::Danger {
        "tile_red"
    } else {
        floor_tile(hue)
    };
    Paint::skinned(
        Finish::Satin,
        WHITE.with_alpha(alpha),
        sprite(name, TILE_CROP),
    )
}

type Nodes<'a> = (Entity, &'a Node, &'a mut Transform, &'a mut Visibility);

#[allow(clippy::too_many_arguments)]
fn sync(
    mut commands: Commands,
    frame: NonSend<crate::Frame>,
    pen: Res<crate::paint::Pen>,
    mut kit: Kit,
    mut root: Query<&mut Root>,
    mut nodes: Query<Nodes, (Without<Role>, Without<Eye>)>,
    mut eye: Lens,
    mut parts: Query<Posed, Without<Eye>>,
) {
    let Some(p) = playing::<HelixPlay>(&frame) else {
        return;
    };
    build(
        &mut commands,
        p,
        &pen,
        &mut kit,
        &mut root,
        &nodes,
        &mut eye,
    );
    pose(&mut commands, p, &pen, &mut kit, &mut nodes, &mut parts);
}

fn build(
    commands: &mut Commands,
    p: &HelixPlay,
    pen: &crate::paint::Pen,
    kit: &mut Kit,
    root: &mut Query<&mut Root>,
    nodes: &Query<Nodes, (Without<Role>, Without<Eye>)>,
    eye: &mut Lens,
) {
    let Ok(mut root) = root.single_mut() else {
        return;
    };
    let g = &p.g;
    let l = lens(g, &view_of(pen));
    {
        eye.aim(Rig {
            eye: l.eye,
            target: l.eye + l.fwd * l.dist,
            fov: FOV,
            near: l.dist * 0.3,
            far: l.dist * 4.0,
            sky: SKY,
            shadow_reach: l.dist * 2.0,
        });
    }
    let ring_of = |i: usize| {
        nodes
            .iter()
            .find(|(_, n, _, _)| **n == Node::Ring(i))
            .map(|(e, ..)| e)
    };
    if root.serial != g.serial {
        root.serial = g.serial;
        root.splats = 0;
        if let Some((tower, ..)) = nodes.iter().find(|(_, n, _, _)| **n == Node::Tower) {
            let mut t = commands.entity(tower);
            t.despawn_children();
            t.with_children(|t| tower_parts(t, kit, g));
        }
        return;
    }
    for (e, n, _, _) in nodes.iter() {
        if let Node::Ring(i) = *n {
            if g.floors
                .get(i)
                .is_some_and(|f| f.broken > 0.0 && fade(f.broken) <= 0.0)
            {
                commands.entity(e).despawn();
            }
        }
    }
    while root.splats < g.splats.len() {
        let (fi, seg, _) = g.splats[root.splats];
        root.splats += 1;
        let Some(ring) = ring_of(fi) else {
            continue;
        };
        if g.floors[fi].broken > 0.0 {
            continue;
        }
        let a = (seg + 0.5) * STEP;
        let r = (RING_OUTER + RING_INNER) / 2.0;
        let at = Transform {
            translation: Vec3::new(a.cos() * r, 0.4, a.sin() * r),
            rotation: flat_decal(),
            scale: Vec3::new(BALL_R * 2.6, BALL_R * 2.6 * 1.05, 1.0),
        };
        let paint = Paint::skinned(
            Finish::Sticker,
            WHITE.with_alpha(0.92),
            sprite("splat", WHOLE),
        );
        commands.entity(ring).with_children(|r| {
            piece(r, kit, Role::Splat, Shape::Quad, paint, at);
        });
    }
}

fn tower_parts(t: &mut ChildSpawnerCommands, kit: &mut Kit, g: &Helix) {
    let top = -Helix::floor_y(0) + 900.0;
    let bottom = -Helix::floor_y(g.floors.len()) - 500.0;
    piece(
        t,
        kit,
        Role::Pole,
        Shape::Cylinder(32),
        Paint::new(Finish::Satin, POLE),
        Transform::from_xyz(0.0, (top + bottom) / 2.0, 0.0).with_scale(Vec3::new(
            RING_INNER * 2.06,
            top - bottom,
            RING_INNER * 2.06,
        )),
    );
    for (i, f) in g.floors.iter().enumerate() {
        if f.broken > 0.0 && fade(f.broken) <= 0.0 {
            continue;
        }
        let ring = node(
            t,
            Node::Ring(i),
            Transform::from_xyz(0.0, -Helix::floor_y(i), 0.0),
        );
        t.commands().entity(ring).with_children(|r| {
            for (k, seg) in f.segs.iter().enumerate() {
                if *seg != Seg::Gap {
                    piece(
                        r,
                        kit,
                        Role::Segment(k),
                        Shape::Sector,
                        tile_paint(*seg, f.hue, 1.0),
                        segment_at_rest(k),
                    );
                }
            }
        });
    }
}

fn segment_at_rest(k: usize) -> Transform {
    Transform::from_rotation(Quat::from_rotation_y(-(k as f32) * STEP))
}

pub fn flying(k: usize, broken: f32) -> Transform {
    let mid = k as f32 * STEP + STEP / 2.0;
    let fall = broken * broken * 900.0;
    let out = Vec3::new(mid.cos(), 0.0, mid.sin());
    let tumble = Quat::from_axis_angle(Vec3::new(-mid.sin(), 0.0, mid.cos()), broken * 2.5);
    Transform {
        translation: out * fall * 0.6 - Vec3::Y * fall,
        rotation: tumble * Quat::from_rotation_y(-(k as f32) * STEP),
        scale: Vec3::ONE,
    }
}

pub fn fade(broken: f32) -> f32 {
    let f = (1.0 - broken * 1.5).max(0.0);
    (f * FADE_STEPS).ceil() / FADE_STEPS
}

fn view_of(pen: &crate::paint::Pen) -> View {
    pen.view.unwrap_or_else(View::ipad_landscape)
}

struct Shot {
    eye: Vec3,
    fwd: Vec3,
    dist: f32,
}

fn lens(g: &Helix, view: &View) -> Shot {
    let rig = kg_core::games::helix::Rig::new(view, g.cam);
    let (eye, target) = (v3(rig.eye), v3(rig.target));
    let fwd = (target - eye).normalize();
    let right = fwd.cross(Vec3::Y).normalize();
    let up = right.cross(fwd);
    let sh = g.shake.offset();
    Shot {
        eye: eye + right * -sh.x + up * -sh.y,
        fwd,
        dist: rig.distance(),
    }
}

pub fn ball_shape(g: &Helix) -> Transform {
    let base = v3(g.ball_world());
    let sq = g.squash * 0.35;
    let stretch = (g.vy.abs() / 1200.0).min(0.25) * (1.0 - g.squash);
    let sx = 1.0 + sq - stretch * 0.5;
    let sy = 1.0 - sq + stretch;
    Transform::from_xyz(base.x, base.y - BALL_R * (1.0 - sy), base.z)
        .with_scale(Vec3::new(sx, sy, sx) * BALL_R * 2.0)
}

type Posed<'a> = (
    Entity,
    &'a Role,
    Option<&'a ChildOf>,
    &'a mut Transform,
    &'a mut Visibility,
    &'a mut Part,
    &'a mut MeshMaterial3d<StandardMaterial>,
);

fn shown(on: bool) -> Visibility {
    if on {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    }
}

fn pose(
    commands: &mut Commands,
    p: &HelixPlay,
    pen: &crate::paint::Pen,
    kit: &mut Kit,
    nodes: &mut Query<Nodes, (Without<Role>, Without<Eye>)>,
    parts: &mut Query<Posed, Without<Eye>>,
) {
    let g = &p.g;
    let view = view_of(pen);
    let f = view.full();
    let dist = lens(g, &view).dist;
    let c = f.center();
    let place = |depth: f32, at: glam::Vec2, w: f32, aspect: f32| Transform {
        translation: Vec3::new((at.x - c.x) * depth, -(at.y - c.y) * depth, -dist * depth),
        rotation: Quat::IDENTITY,
        scale: Vec3::new(w * depth, w * depth * aspect, 1.0),
    };
    for (_, n, mut t, mut v) in nodes.iter_mut() {
        match n {
            Node::Stage => {
                v.set_if_neq(shown(g.state != Phase::Select));
            }
            Node::Tower => {
                t.set_if_neq(Transform::from_rotation(Quat::from_rotation_y(-g.angle)));
            }
            Node::Ring(_) => {}
        }
    }
    let floor_of = |parent: Option<&ChildOf>| {
        parent
            .and_then(|r| nodes.get(r.parent()).ok())
            .and_then(|(_, n, _, _)| match n {
                Node::Ring(i) => g.floors.get(*i),
                _ => None,
            })
    };
    let base = v3(g.ball_world());
    let is_dead = g.state == Phase::Dead;
    let blazing = g.streak >= SMASH_FLOORS && g.state == Phase::Playing;
    for (e, role, parent, mut t, mut v, mut part, mut mat) in parts.iter_mut() {
        match *role {
            Role::Sky => {
                let s = crate::art::size(SHEET, "bg");
                let k = (f.w / s.x).max(f.h / s.y) * 1.02;
                t.set_if_neq(place(SKY_DEPTH, c, s.x * k, s.y / s.x.max(1.0)));
                let hue = (g.level as f32 * 0.07).rem_euclid(1.0);
                let paint = Paint {
                    color: hsv(hue, 0.12, 1.0),
                    ..part.paint
                };
                kit.repaint(&mut part, &mut mat, paint);
            }
            Role::Cloud(i) => {
                let depth = 0.08 + (i % 3) as f32 * 0.05;
                let span = view.w + 400.0;
                let x = (i as f32 * 233.0 + g.t * (5.0 + depth * 40.0)).rem_euclid(span) - 200.0;
                let y = ((i * 131 % 700) as f32 - g.cam * depth).rem_euclid(view.h + 200.0) - 100.0;
                let name = if i % 2 == 0 { "cloud1" } else { "cloud2" };
                let a = crate::art::size(SHEET, name);
                t.set_if_neq(place(
                    CLOUD_DEPTH,
                    glam::vec2(x, y),
                    150.0 + depth * 900.0,
                    a.y / a.x.max(1.0),
                ));
            }
            Role::Ball => {
                v.set_if_neq(shown(!is_dead));
                t.set_if_neq(ball_shape(g));
            }
            Role::Trail(k) => {
                let k = (k + 1) as f32;
                v.set_if_neq(shown(blazing));
                let r = BALL_R * 2.0 * (1.0 - k * 0.12);
                t.set_if_neq(
                    Transform::from_xyz(base.x, base.y + k * 12.0, base.z)
                        .with_scale(Vec3::splat(r)),
                );
            }
            Role::DeadSplat => {
                v.set_if_neq(shown(is_dead));
                let floor = Helix::floor_y(g.passed.min(g.floors.len() - 1));
                t.set_if_neq(Transform {
                    translation: Vec3::new(base.x, -floor + 0.6, base.z),
                    rotation: flat_decal(),
                    scale: Vec3::new(BALL_R * 3.6, BALL_R * 3.8, 1.0),
                });
            }
            Role::Segment(k) => {
                let Some(fl) = floor_of(parent) else {
                    continue;
                };
                if fl.broken <= 0.0 {
                    continue;
                }
                t.set_if_neq(flying(k, fl.broken));
                if part.paint.color.a > 0.999 {
                    commands.entity(e).insert(NotShadowCaster);
                }
                let paint = tile_paint(fl.segs[k], fl.hue, fade(fl.broken).min(0.98));
                kit.repaint(&mut part, &mut mat, paint);
            }
            Role::Splat => {
                let gone = floor_of(parent).is_some_and(|fl| fl.broken > 0.0);
                if gone {
                    commands.entity(e).despawn();
                }
            }
            Role::Pole => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game3d::testing;
    use crate::games::Play;
    use kg_core::engine::Input;

    fn open(play: HelixPlay) -> App {
        let mut app = testing::with_game(Box::new(play), View::ipad_portrait());
        testing::step(&mut app, 3);
        app
    }

    fn helix(app: &mut App) -> &mut HelixPlay {
        testing::game::<HelixPlay>(app)
    }

    fn nodes(app: &mut App, f: impl Fn(&Node) -> bool) -> usize {
        let w = app.world_mut();
        w.query::<&Node>().iter(w).filter(|n| f(n)).count()
    }

    fn roles(app: &mut App, f: impl Fn(&Role) -> bool) -> usize {
        let w = app.world_mut();
        w.query::<&Role>().iter(w).filter(|r| f(r)).count()
    }

    fn fresh() -> HelixPlay {
        kg_core::save::reset_for_test();
        let mut p = HelixPlay::new();
        p.g = Helix::with_level(5);
        p
    }

    #[test]
    fn opening_and_closing_helix_leaves_the_menu_as_it_was() {
        let mut app = testing::app(crate::Scene::Menu(Box::default()), View::ipad_landscape());
        testing::step(&mut app, 3);
        let menu = testing::count(&mut app);
        for _ in 0..3 {
            if let crate::Scene::Menu(m) = &mut testing::frame(&mut app).scene {
                m.current = Some((0, Box::new(fresh())));
            }
            testing::step(&mut app, 3);
            let open = testing::count(&mut app);
            assert!(open > menu + 100, "{menu} -> {open}");
            assert_eq!(nodes(&mut app, |n| *n == Node::Tower), 1);
            testing::close(&mut app);
            testing::step(&mut app, 3);
            assert_eq!(testing::count(&mut app), menu);
            assert_eq!(testing::open(&app), World3d::Flat);
        }
    }

    #[test]
    fn the_tower_is_one_parent_with_a_ring_per_floor() {
        let mut app = open(fresh());
        let floors = helix(&mut app).g.floors.len();
        assert_eq!(nodes(&mut app, |n| *n == Node::Tower), 1);
        assert_eq!(nodes(&mut app, |n| matches!(n, Node::Ring(_))), floors);
        let solid: usize = helix(&mut app)
            .g
            .floors
            .iter()
            .map(|f| f.segs.iter().filter(|s| **s != Seg::Gap).count())
            .sum();
        assert_eq!(roles(&mut app, |r| matches!(r, Role::Segment(_))), solid);
        let w = app.world_mut();
        let all: Vec<(Entity, Node, Option<Entity>)> = w
            .query::<(Entity, &Node, Option<&ChildOf>)>()
            .iter(w)
            .map(|(e, n, c)| (e, *n, c.map(|c| c.parent())))
            .collect();
        let tower = all.iter().find(|(_, n, _)| *n == Node::Tower).unwrap().0;
        for (_, n, parent) in &all {
            if let Node::Ring(i) = n {
                assert_eq!(*parent, Some(tower), "ring {i} hangs off the tower");
            }
        }
    }

    #[test]
    fn turning_the_tower_moves_only_its_transform() {
        let mut app = open(fresh());
        helix(&mut app).g.state = Phase::Playing;
        testing::step(&mut app, 1);
        let before = testing::count(&mut app);
        helix(&mut app).g.angle += 1.0;
        testing::step(&mut app, 1);
        assert_eq!(testing::count(&mut app), before);
        let w = app.world_mut();
        let t = *w
            .query::<(&Node, &Transform)>()
            .iter(w)
            .find(|(n, _)| **n == Node::Tower)
            .unwrap()
            .1;
        let fwd = t.rotation * Vec3::X;
        let want = Quat::from_rotation_y(-helix_angle(&mut app)) * Vec3::X;
        assert!((fwd - want).length() < 1e-4);
    }

    fn helix_angle(app: &mut App) -> f32 {
        helix(app).g.angle
    }

    #[test]
    fn a_smashed_floor_flies_off_and_then_is_gone() {
        let mut app = open(fresh());
        let rings = nodes(&mut app, |n| matches!(n, Node::Ring(_)));
        helix(&mut app).g.floors[0].broken = 0.2;
        testing::step(&mut app, 1);
        {
            let w = app.world_mut();
            let mut q = w.query::<(&Role, &ChildOf, &Transform, &Part)>();
            let ring0 = w
                .query::<(Entity, &Node)>()
                .iter(w)
                .find(|(_, n)| **n == Node::Ring(0))
                .unwrap()
                .0;
            let flying: Vec<_> = q
                .iter(w)
                .filter(|(r, p, _, _)| matches!(r, Role::Segment(_)) && p.parent() == ring0)
                .collect();
            assert!(!flying.is_empty());
            for (_, _, t, part) in flying {
                assert!(t.translation.y < -10.0, "a smashed segment falls");
                assert!(part.paint.color.a < 1.0, "and fades");
            }
        }
        helix(&mut app).g.floors[0].broken = 0.7;
        testing::step(&mut app, 2);
        assert_eq!(nodes(&mut app, |n| matches!(n, Node::Ring(_))), rings - 1);
    }

    #[test]
    fn the_ball_squashes_on_a_bounce_and_stretches_when_falling() {
        let mut g = Helix::with_level(1);
        g.squash = 1.0;
        g.vy = 0.0;
        let s = ball_shape(&g).scale;
        assert!(s.x > s.y * 1.5, "squashed {s:?}");
        g.squash = 0.0;
        g.vy = 1200.0;
        let s = ball_shape(&g).scale;
        assert!(s.y > s.x * 1.2, "stretched {s:?}");
    }

    #[test]
    fn a_tower_built_mid_smash_still_shows_the_falling_rings() {
        let mut p = fresh();
        p.g.floors[0].broken = 0.2;
        p.g.floors[1].broken = 0.9;
        let mut app = open(p);
        let floors = helix(&mut app).g.floors.len();
        assert_eq!(nodes(&mut app, |n| matches!(n, Node::Ring(_))), floors - 1);
    }

    #[test]
    fn a_bounce_leaves_a_splat_on_the_floor_it_hit() {
        let mut app = open(fresh());
        helix(&mut app).g.splats.push((1, 3.0, 0.0));
        testing::step(&mut app, 2);
        assert_eq!(roles(&mut app, |r| *r == Role::Splat), 1);
        helix(&mut app).g.floors[1].broken = 0.1;
        testing::step(&mut app, 2);
        assert_eq!(roles(&mut app, |r| *r == Role::Splat), 0);
    }

    #[test]
    fn a_new_level_rebuilds_the_tower_without_leaking() {
        let mut app = open(fresh());
        let before = testing::count(&mut app);
        let parts_before = roles(&mut app, |r| matches!(r, Role::Segment(_)));
        let g = &mut helix(&mut app).g;
        let level = g.level;
        g.enter(level);
        testing::step(&mut app, 2);
        let solid: usize = helix(&mut app)
            .g
            .floors
            .iter()
            .map(|f| f.segs.iter().filter(|s| **s != Seg::Gap).count())
            .sum();
        assert_eq!(roles(&mut app, |r| matches!(r, Role::Segment(_))), solid);
        let after = testing::count(&mut app);
        assert_eq!(
            after as i64 - before as i64,
            solid as i64 - parts_before as i64
        );
    }

    #[test]
    fn playing_a_level_keeps_the_world_in_step() {
        let mut app = open(fresh());
        let view = View::ipad_portrait();
        let p = helix(&mut app);
        p.update(
            &Input::press_at(glam::vec2(500.0, 500.0)),
            &view,
            1.0 / 60.0,
        );
        for _ in 0..600 {
            helix(&mut app).update(&Input::default(), &view, 1.0 / 60.0);
            testing::step(&mut app, 1);
        }
        let g = &helix(&mut app).g;
        let live = g.floors.iter().filter(|f| fade(f.broken) > 0.0).count();
        assert_eq!(nodes(&mut app, |n| matches!(n, Node::Ring(_))), live);
    }
}
