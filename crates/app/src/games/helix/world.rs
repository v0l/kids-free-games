use super::{HelixPlay, SHEET};
use crate::game3d::*;
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
        app.add_systems(OnEnter(World3d::Helix), spawn).add_systems(
            Update,
            (rebuild_tower, (aim, backdrop, spin, smash, splats, ball))
                .chain()
                .in_set(Sync3d)
                .run_if(in_state(World3d::Helix)),
        );
    }
}

#[derive(Component)]
pub struct Root {
    serial: u64,
    splats: usize,
}

#[derive(Component)]
pub struct Eye;

#[derive(Component)]
struct Sun;

#[derive(Component)]
struct Sky;

#[derive(Component)]
struct Cloud(usize);

#[derive(Component)]
struct Stage;

#[derive(Component)]
pub struct Tower;

#[derive(Component)]
pub struct Ring(pub usize);

#[derive(Component)]
pub struct Segment(pub usize);

#[derive(Component)]
pub struct Splat;

#[derive(Component)]
pub struct Ball;

#[derive(Component)]
struct Trail(usize);

#[derive(Component)]
pub struct DeadSplat;

fn sprite(name: &'static str, crop: [f32; 4]) -> Skin {
    Skin::Sprite(SHEET, name, crop)
}

fn flat_decal() -> Quat {
    Quat::from_rotation_x(-FRAC_PI_2)
}

fn spawn(mut commands: Commands, mut kit: Kit) {
    commands
        .spawn((
            Root {
                serial: 0,
                splats: 0,
            },
            DespawnOnExit(World3d::Helix),
            Transform::default(),
            Visibility::default(),
        ))
        .with_children(|w| {
            w.spawn((camera(SKY, 420.0), Eye)).with_children(|c| {
                c.spawn((
                    kit.part(
                        Shape::Quad,
                        Paint::skinned(Finish::Backdrop, WHITE, sprite("bg", WHOLE)),
                        Transform::default(),
                    ),
                    Sky,
                ));
                for i in 0..CLOUDS {
                    let name = if i % 2 == 0 { "cloud1" } else { "cloud2" };
                    c.spawn((
                        kit.part(
                            Shape::Quad,
                            Paint::skinned(
                                Finish::Sticker,
                                WHITE.with_alpha(0.85),
                                sprite(name, WHOLE),
                            ),
                            Transform::default(),
                        ),
                        Cloud(i),
                    ));
                }
            });
            w.spawn((
                sun(9000.0, WHITE, Vec3::new(-0.45, -1.0, -0.55), 2000.0),
                Sun,
            ));
            w.spawn((Stage, Transform::default(), Visibility::Hidden))
                .with_children(|s| {
                    s.spawn((Tower, Transform::default(), Visibility::default()));
                    s.spawn((
                        kit.part(
                            Shape::Ball,
                            Paint::new(Finish::Satin, BALL),
                            Transform::default(),
                        ),
                        Ball,
                    ));
                    for k in 0..TRAIL {
                        s.spawn((
                            kit.part(
                                Shape::Ball,
                                Paint::new(Finish::Glow, Col::new(1.0, 0.6, 0.2, 0.3)),
                                Transform::default(),
                            ),
                            Trail(k),
                        ))
                        .insert(Visibility::Hidden);
                    }
                    s.spawn((
                        kit.part(
                            Shape::Quad,
                            Paint::skinned(Finish::Sticker, WHITE, sprite("splat", WHOLE)),
                            Transform::default(),
                        ),
                        DeadSplat,
                    ))
                    .insert(Visibility::Hidden);
                });
        });
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

fn rebuild_tower(
    mut commands: Commands,
    frame: NonSend<crate::Frame>,
    mut kit: Kit,
    mut root: Query<&mut Root>,
    tower: Query<Entity, With<Tower>>,
) {
    let Some(p) = playing::<HelixPlay>(&frame) else {
        return;
    };
    let (Ok(mut root), Ok(tower)) = (root.single_mut(), tower.single()) else {
        return;
    };
    let g = &p.g;
    if root.serial == g.serial {
        return;
    }
    root.serial = g.serial;
    root.splats = 0;
    let mut t = commands.entity(tower);
    t.despawn_children();
    t.with_children(|t| {
        let top = -Helix::floor_y(0) + 900.0;
        let bottom = -Helix::floor_y(g.floors.len()) - 500.0;
        t.spawn(kit.part(
            Shape::Cylinder(32),
            Paint::new(Finish::Satin, POLE),
            Transform::from_xyz(0.0, (top + bottom) / 2.0, 0.0).with_scale(Vec3::new(
                RING_INNER * 2.06,
                top - bottom,
                RING_INNER * 2.06,
            )),
        ));
        for (i, f) in g.floors.iter().enumerate() {
            if f.broken > 0.0 && fade(f.broken) <= 0.0 {
                continue;
            }
            t.spawn((
                Ring(i),
                Transform::from_xyz(0.0, -Helix::floor_y(i), 0.0),
                Visibility::default(),
            ))
            .with_children(|r| {
                for (k, seg) in f.segs.iter().enumerate() {
                    if *seg == Seg::Gap {
                        continue;
                    }
                    r.spawn((
                        kit.part(
                            Shape::Sector,
                            tile_paint(*seg, f.hue, 1.0),
                            segment_at_rest(k),
                        ),
                        Segment(k),
                    ));
                }
            });
        }
    });
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

struct Lens {
    eye: Vec3,
    fwd: Vec3,
    dist: f32,
}

fn lens(g: &Helix, view: &View) -> Lens {
    let rig = Rig::new(view, g.cam);
    let (eye, target) = (v3(rig.eye), v3(rig.target));
    let fwd = (target - eye).normalize();
    let right = fwd.cross(Vec3::Y).normalize();
    let up = right.cross(fwd);
    let sh = g.shake.offset();
    Lens {
        eye: eye + right * -sh.x + up * -sh.y,
        fwd,
        dist: rig.distance(),
    }
}

type EyeParts<'a> = (&'a mut Transform, &'a mut Projection);

fn aim(
    frame: NonSend<crate::Frame>,
    pen: Res<crate::paint::Pen>,
    mut eye: Query<EyeParts, (With<Eye>, Without<Sun>)>,
    mut sun: Query<&mut bevy::light::CascadeShadowConfig, With<Sun>>,
) {
    let Some(p) = playing::<HelixPlay>(&frame) else {
        return;
    };
    let l = lens(&p.g, &view_of(&pen));
    if let Ok((mut t, mut proj)) = eye.single_mut() {
        crate::game3d::aim(
            &mut t,
            &mut proj,
            &Aim {
                eye: l.eye,
                target: l.eye + l.fwd * l.dist,
                fov: FOV,
                near: l.dist * 0.3,
                far: l.dist * 4.0,
            },
        );
    }
    if let Ok(mut c) = sun.single_mut() {
        let want = l.dist * 2.0;
        if c.bounds.first() != Some(&want) {
            *c = shadow_reach(want);
        }
    }
}

type SkyParts<'a> = (
    &'a mut Transform,
    &'a mut Part,
    &'a mut MeshMaterial3d<StandardMaterial>,
);

fn backdrop(
    frame: NonSend<crate::Frame>,
    pen: Res<crate::paint::Pen>,
    mut kit: Kit,
    mut sky: Query<SkyParts, (With<Sky>, Without<Cloud>)>,
    mut clouds: Query<(&Cloud, &mut Transform), Without<Sky>>,
) {
    let Some(p) = playing::<HelixPlay>(&frame) else {
        return;
    };
    let g = &p.g;
    let view = view_of(&pen);
    let f = view.full();
    let dist = lens(g, &view).dist;
    let c = f.center();
    let place = |depth: f32, at: glam::Vec2, w: f32, aspect: f32| Transform {
        translation: Vec3::new((at.x - c.x) * depth, -(at.y - c.y) * depth, -dist * depth),
        rotation: Quat::IDENTITY,
        scale: Vec3::new(w * depth, w * depth * aspect, 1.0),
    };
    if let Ok((mut t, mut part, mut mat)) = sky.single_mut() {
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
    for (cl, mut t) in &mut clouds {
        let i = cl.0;
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
}

fn spin(
    frame: NonSend<crate::Frame>,
    mut stage: Query<&mut Visibility, With<Stage>>,
    mut tower: Query<&mut Transform, With<Tower>>,
) {
    let Some(p) = playing::<HelixPlay>(&frame) else {
        return;
    };
    if let Ok(mut v) = stage.single_mut() {
        v.set_if_neq(if p.g.state == Phase::Select {
            Visibility::Hidden
        } else {
            Visibility::Inherited
        });
    }
    if let Ok(mut t) = tower.single_mut() {
        t.set_if_neq(Transform::from_rotation(Quat::from_rotation_y(-p.g.angle)));
    }
}

type SegParts<'a> = (
    &'a Segment,
    &'a mut Transform,
    &'a mut Part,
    &'a mut MeshMaterial3d<StandardMaterial>,
);

fn smash(
    mut commands: Commands,
    frame: NonSend<crate::Frame>,
    mut kit: Kit,
    rings: Query<(Entity, &Ring, &Children)>,
    splats: Query<(), With<Splat>>,
    mut segs: Query<SegParts>,
) {
    let Some(p) = playing::<HelixPlay>(&frame) else {
        return;
    };
    let g = &p.g;
    for (e, ring, children) in &rings {
        let Some(f) = g.floors.get(ring.0) else {
            continue;
        };
        if f.broken <= 0.0 {
            continue;
        }
        let alpha = fade(f.broken);
        if alpha <= 0.0 {
            commands.entity(e).despawn();
            continue;
        }
        for c in children.iter() {
            if splats.contains(c) {
                commands.entity(c).despawn();
            }
        }
        for c in children.iter() {
            let Ok((seg, mut t, mut part, mut mat)) = segs.get_mut(c) else {
                continue;
            };
            t.set_if_neq(flying(seg.0, f.broken));
            if part.paint.color.a > 0.999 {
                commands.entity(c).insert(NotShadowCaster);
            }
            let paint = tile_paint(f.segs[seg.0], f.hue, alpha.min(0.98));
            kit.repaint(&mut part, &mut mat, paint);
        }
    }
}

fn splats(
    mut commands: Commands,
    frame: NonSend<crate::Frame>,
    mut kit: Kit,
    mut root: Query<&mut Root>,
    rings: Query<(Entity, &Ring)>,
) {
    let Some(p) = playing::<HelixPlay>(&frame) else {
        return;
    };
    let Ok(mut root) = root.single_mut() else {
        return;
    };
    let g = &p.g;
    if root.serial != g.serial {
        return;
    }
    while root.splats < g.splats.len() {
        let (fi, seg, _) = g.splats[root.splats];
        root.splats += 1;
        let Some((ring, _)) = rings.iter().find(|(_, r)| r.0 == fi) else {
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
            r.spawn((kit.part(Shape::Quad, paint, at), Splat));
        });
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

type BallParts<'a> = (&'a mut Transform, &'a mut Visibility);

#[allow(clippy::type_complexity)]
fn ball(
    frame: NonSend<crate::Frame>,
    mut ball: Query<BallParts, (With<Ball>, Without<Trail>, Without<DeadSplat>)>,
    mut trail: Query<
        (&Trail, &mut Transform, &mut Visibility),
        (Without<Ball>, Without<DeadSplat>),
    >,
    mut dead: Query<BallParts, (With<DeadSplat>, Without<Ball>, Without<Trail>)>,
) {
    let Some(p) = playing::<HelixPlay>(&frame) else {
        return;
    };
    let g = &p.g;
    let base = v3(g.ball_world());
    let is_dead = g.state == Phase::Dead;
    let shown = |on: bool| {
        if on {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        }
    };
    if let Ok((mut t, mut v)) = ball.single_mut() {
        v.set_if_neq(shown(!is_dead));
        t.set_if_neq(ball_shape(g));
    }
    let blazing = g.streak >= SMASH_FLOORS && g.state == Phase::Playing;
    for (tr, mut t, mut v) in &mut trail {
        let k = (tr.0 + 1) as f32;
        v.set_if_neq(shown(blazing));
        let r = BALL_R * 2.0 * (1.0 - k * 0.12);
        t.set_if_neq(
            Transform::from_xyz(base.x, base.y + k * 12.0, base.z).with_scale(Vec3::splat(r)),
        );
    }
    if let Ok((mut t, mut v)) = dead.single_mut() {
        v.set_if_neq(shown(is_dead));
        let floor = Helix::floor_y(g.passed.min(g.floors.len() - 1));
        t.set_if_neq(Transform {
            translation: Vec3::new(base.x, -floor + 0.6, base.z),
            rotation: flat_decal(),
            scale: Vec3::new(BALL_R * 3.6, BALL_R * 3.8, 1.0),
        });
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

    fn count<C: Component>(app: &mut App) -> usize {
        let w = app.world_mut();
        w.query_filtered::<Entity, With<C>>().iter(w).count()
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
            assert_eq!(count::<Tower>(&mut app), 1);
            testing::close(&mut app);
            testing::step(&mut app, 3);
            assert_eq!(testing::count(&mut app), menu);
            assert_eq!(
                *app.world()
                    .resource::<bevy::state::state::State<World3d>>()
                    .get(),
                World3d::Flat
            );
        }
    }

    #[test]
    fn the_tower_is_one_parent_with_a_ring_per_floor() {
        let mut app = open(fresh());
        let floors = helix(&mut app).g.floors.len();
        assert_eq!(count::<Tower>(&mut app), 1);
        assert_eq!(count::<Ring>(&mut app), floors);
        let solid: usize = helix(&mut app)
            .g
            .floors
            .iter()
            .map(|f| f.segs.iter().filter(|s| **s != Seg::Gap).count())
            .sum();
        assert_eq!(count::<Segment>(&mut app), solid);
        let w = app.world_mut();
        let tower = w.query_filtered::<Entity, With<Tower>>().single(w).unwrap();
        for (r, parent) in w.query::<(&Ring, &ChildOf)>().iter(w) {
            assert_eq!(parent.parent(), tower, "ring {} hangs off the tower", r.0);
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
        let t = w
            .query_filtered::<&Transform, With<Tower>>()
            .single(w)
            .unwrap();
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
        let rings = count::<Ring>(&mut app);
        helix(&mut app).g.floors[0].broken = 0.2;
        testing::step(&mut app, 1);
        {
            let w = app.world_mut();
            let mut q = w.query::<(&Segment, &ChildOf, &Transform, &Part)>();
            let ring0 = w
                .query::<(Entity, &Ring)>()
                .iter(w)
                .find(|(_, r)| r.0 == 0)
                .unwrap()
                .0;
            let flying: Vec<_> = q
                .iter(w)
                .filter(|(_, p, _, _)| p.parent() == ring0)
                .collect();
            assert!(!flying.is_empty());
            for (_, _, t, part) in flying {
                assert!(t.translation.y < -10.0, "a smashed segment falls");
                assert!(part.paint.color.a < 1.0, "and fades");
            }
        }
        helix(&mut app).g.floors[0].broken = 0.7;
        testing::step(&mut app, 2);
        assert_eq!(count::<Ring>(&mut app), rings - 1);
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
        assert_eq!(count::<Ring>(&mut app), floors - 1);
    }

    #[test]
    fn a_bounce_leaves_a_splat_on_the_floor_it_hit() {
        let mut app = open(fresh());
        helix(&mut app).g.splats.push((1, 3.0, 0.0));
        testing::step(&mut app, 2);
        assert_eq!(count::<Splat>(&mut app), 1);
        helix(&mut app).g.floors[1].broken = 0.1;
        testing::step(&mut app, 2);
        assert_eq!(count::<Splat>(&mut app), 0);
    }

    #[test]
    fn a_new_level_rebuilds_the_tower_without_leaking() {
        let mut app = open(fresh());
        let before = testing::count(&mut app);
        let parts_before = count::<Segment>(&mut app);
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
        assert_eq!(count::<Segment>(&mut app), solid);
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
        assert_eq!(count::<Ring>(&mut app), live);
    }
}
