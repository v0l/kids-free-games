use super::land;
use super::{PaperPlay, SLAB};
use crate::game3d::*;
use bevy::asset::RenderAssetUsages;
use bevy::camera::visibility::NoFrustumCulling;
use bevy::light::NotShadowCaster;
use bevy::mesh::{Indices, Mesh, PrimitiveTopology};
use bevy::prelude::*;
use kg_core::engine::View;
use kg_core::fx::ease_out_back;
use kg_core::games::paper::camera::FOV;
use kg_core::games::paper::world::BLOCKS;
use kg_core::games::paper::*;
use kg_core::geom::{Color as Col, WHITE};

const SKY: Col = Col::new(0.45, 0.93, 0.86, 1.0);
const MINT: Col = Col::new(0.9, 0.99, 0.95, 1.0);
const SEA: Col = Col::new(0.46, 0.95, 0.88, 1.0);
const REBUILDS_PER_FRAME: usize = 24;
const TRAIL_WIDTH: f32 = 0.95;
const TRAIL_ALPHA: f32 = 0.5;
const HEAD_SIZE: f32 = 1.7;

pub struct PaperWorld;

impl Plugin for PaperWorld {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                spawn.run_if(opened(World3d::Paper)),
                sync.run_if(is_open(World3d::Paper)),
            )
                .chain()
                .in_set(Sync3d),
        );
    }
}

#[derive(Component)]
pub struct Roster {
    uid: u64,
    stamps: Vec<u64>,
    flashed: Vec<bool>,
    clocks: Vec<f32>,
}

#[derive(Component)]
pub struct Block(pub usize);

#[derive(Component)]
pub struct Ribbon(pub usize);

#[derive(Component)]
pub struct Mover(pub usize);

#[derive(Component)]
pub struct Debris(pub usize);

fn spawn(mut commands: Commands, mut kit: Kit) {
    let world = spawn_world(
        &mut commands,
        Roster {
            uid: 0,
            stamps: vec![u64::MAX; BLOCKS * BLOCKS],
            flashed: vec![false; BLOCKS * BLOCKS],
            clocks: vec![-1.0; BLOCKS * BLOCKS],
        },
        World3d::Paper,
        Lighting {
            ambient: 380.0,
            illuminance: 4200.0,
            color: WHITE,
            toward: Vec3::new(-0.4, -1.0, 0.45),
        },
    );
    let ground = |kit: &mut Kit, col: Col, at: Transform| {
        let part = kit.part(Shape::Cube, Paint::new(Finish::Floor, col), at);
        (part, NotShadowCaster)
    };
    let side = SIZE + 0.0;
    commands.entity(world.root).with_children(|w| {
        w.spawn(ground(
            &mut kit,
            MINT,
            Transform::from_xyz(side / 2.0, -0.25, side / 2.0)
                .with_scale(Vec3::new(side, 0.5, side)),
        ));
        w.spawn(ground(
            &mut kit,
            SEA,
            Transform::from_xyz(side / 2.0, -0.52, side / 2.0).with_scale(Vec3::new(
                side + 900.0,
                0.02,
                side + 900.0,
            )),
        ));
        for b in 0..BLOCKS * BLOCKS {
            w.spawn((
                Block(b),
                NoFrustumCulling,
                Mesh3d::default(),
                MeshMaterial3d(kit.mat(Paint::new(Finish::Matte, WHITE))),
                Transform::default(),
                Visibility::Hidden,
                NotShadowCaster,
            ));
        }
        for id in 0..MAX_PLAYERS {
            let col = color(id);
            w.spawn((Mover(id), Transform::default(), Visibility::Hidden))
                .with_children(|m| {
                    let paint = |c: Col| Paint::new(Finish::Gloss, c);
                    m.spawn(kit.part(
                        Shape::Cube,
                        paint(col),
                        Transform::from_xyz(0.0, 0.0, 0.0).with_scale(Vec3::new(
                            HEAD_SIZE,
                            HEAD_SIZE * 0.82,
                            HEAD_SIZE,
                        )),
                    ));
                    m.spawn(kit.part(
                        Shape::Cube,
                        paint(col.lerp(WHITE, 0.45)),
                        Transform::from_xyz(-0.05, HEAD_SIZE * 0.5, 0.0).with_scale(Vec3::new(
                            HEAD_SIZE * 0.6,
                            HEAD_SIZE * 0.3,
                            HEAD_SIZE * 0.6,
                        )),
                    ));
                    for z in [-0.28, 0.28] {
                        m.spawn(
                            kit.part(
                                Shape::Cube,
                                paint(Col::new(0.1, 0.12, 0.2, 1.0)),
                                Transform::from_xyz(
                                    HEAD_SIZE * 0.5,
                                    HEAD_SIZE * 0.1,
                                    z * HEAD_SIZE,
                                )
                                .with_scale(Vec3::new(0.12, 0.26, 0.2)),
                            ),
                        );
                    }
                });
            w.spawn((
                Ribbon(id),
                NoFrustumCulling,
                Mesh3d::default(),
                MeshMaterial3d(kit.mat(Paint::new(Finish::Glow, WHITE))),
                Transform::default(),
                Visibility::Hidden,
                NotShadowCaster,
            ));
        }
        for k in 0..MAX_CHUNKS {
            w.spawn((
                Debris(k),
                kit.part(
                    Shape::Cube,
                    Paint::new(Finish::Matte, color(k % MAX_PLAYERS)),
                    Transform::default(),
                ),
            ));
        }
    });
}

pub fn view_of(pen: &crate::paint::Pen) -> View {
    pen.view.unwrap_or_else(View::ipad_landscape)
}

pub fn rig(g: &Paper, view: &View) -> Rig {
    let cam = g.eye(view);
    let d = cam.distance();
    Rig {
        eye: v3(cam.eye),
        target: v3(cam.target),
        fov: FOV,
        near: d * 0.2,
        far: d * 4.0,
        sky: SKY,
        shadow_reach: d * 1.6,
    }
}

pub fn chaikin(pts: &[glam::Vec2], rounds: usize) -> Vec<glam::Vec2> {
    let mut cur = pts.to_vec();
    for _ in 0..rounds {
        if cur.len() < 3 {
            break;
        }
        let mut next = Vec::with_capacity(cur.len() * 2);
        next.push(cur[0]);
        for w in cur.windows(2) {
            next.push(w[0].lerp(w[1], 0.25));
            next.push(w[0].lerp(w[1], 0.75));
        }
        next.push(*cur.last().unwrap());
        cur = next;
    }
    cur
}

pub fn ribbon_mesh(pts: &[glam::Vec2], base: Col, clock: f32) -> Option<Mesh> {
    let pts = chaikin(pts, 2);
    if pts.len() < 2 {
        return None;
    }
    let lo = crate::paint::linear(base.lerp(WHITE, 0.3));
    let hi = crate::paint::linear(base.lerp(WHITE, 0.6));
    let (mut pos, mut nor, mut col, mut idx) = (vec![], vec![], vec![], vec![]);
    let n = pts.len();
    for (strip, (w, c, a)) in [(TRAIL_WIDTH, lo, TRAIL_ALPHA), (TRAIL_WIDTH * 0.4, hi, 0.4)]
        .into_iter()
        .enumerate()
    {
        let y = SLAB + 0.03 + strip as f32 * 0.01;
        let first = pos.len() as u32;
        for k in 0..n {
            let prev = pts[k.saturating_sub(1)];
            let next = pts[(k + 1).min(n - 1)];
            let side = (next - prev).normalize_or(glam::Vec2::X).perp();
            let from_tail = (n - 1 - k) as f32;
            let wave = 1.0 + 0.06 * (clock * 9.0 - from_tail * 0.25).sin();
            let half = w * wave * 0.5;
            let taper = (k as f32 / 6.0).clamp(0.25, 1.0);
            for s in [1.0, -1.0] {
                let p = pts[k] + side * half * s * taper;
                pos.push([p.x, y, p.y]);
                nor.push([0.0, 1.0, 0.0]);
                col.push([c[0], c[1], c[2], a]);
            }
        }
        for k in 0..n as u32 - 1 {
            let b = first + k * 2;
            idx.extend([b, b + 3, b + 1, b, b + 2, b + 3]);
        }
    }
    let mut m = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::RENDER_WORLD,
    );
    m.insert_attribute(Mesh::ATTRIBUTE_POSITION, pos);
    m.insert_attribute(Mesh::ATTRIBUTE_NORMAL, nor);
    m.insert_attribute(Mesh::ATTRIBUTE_COLOR, col);
    m.insert_indices(Indices::U32(idx));
    Some(m)
}

type Movers<'a> = (&'a Mover, &'a mut Transform, &'a mut Visibility);
type Debrises<'a> = (&'a Debris, &'a mut Transform, &'a mut Visibility);
type Blocks<'a> = (&'a Block, &'a mut Mesh3d, &'a mut Visibility);
type Ribbons<'a> = (&'a Ribbon, &'a mut Mesh3d, &'a mut Visibility);

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn sync(
    frame: NonSend<crate::Frame>,
    pen: Res<crate::paint::Pen>,
    mut kit: Kit,
    mut roster: Query<&mut Roster>,
    mut eye: Lens,
    mut movers: Query<
        Movers,
        (
            Without<Eye>,
            Without<Debris>,
            Without<Block>,
            Without<Ribbon>,
        ),
    >,
    mut debris: Query<
        Debrises,
        (
            Without<Eye>,
            Without<Mover>,
            Without<Block>,
            Without<Ribbon>,
        ),
    >,
    mut blocks: Query<
        Blocks,
        (
            Without<Eye>,
            Without<Mover>,
            Without<Debris>,
            Without<Ribbon>,
        ),
    >,
    mut ribbons: Query<
        Ribbons,
        (
            Without<Eye>,
            Without<Mover>,
            Without<Debris>,
            Without<Block>,
        ),
    >,
) {
    let Some(p) = playing::<PaperPlay>(&frame) else {
        return;
    };
    let g = &p.g;
    eye.aim(rig(g, &view_of(&pen)));
    let Ok(mut roster) = roster.single_mut() else {
        return;
    };
    if roster.uid != g.w.uid {
        roster.uid = g.w.uid;
        roster.stamps.fill(u64::MAX);
    }
    let mut budget = REBUILDS_PER_FRAME;
    for (Block(b), mut mesh, mut vis) in &mut blocks {
        let b = *b;
        let flash = land::flashing(g, b);
        let stamp = land::stamp(g, b);
        let moved = roster.clocks[b] != g.w.clock;
        if roster.stamps[b] == stamp && !(flash && moved) && !(roster.flashed[b] && !flash) {
            continue;
        }
        if budget == 0 {
            continue;
        }
        budget -= 1;
        roster.stamps[b] = stamp;
        roster.flashed[b] = flash;
        roster.clocks[b] = g.w.clock;
        if !land::has_land(g, b) {
            vis.set_if_neq(Visibility::Hidden);
            continue;
        }
        let built = land::build(g, b);
        if built.vertices() == 0 {
            vis.set_if_neq(Visibility::Hidden);
            continue;
        }
        kit.refill(&mut mesh.0, built.mesh());
        vis.set_if_neq(Visibility::Inherited);
    }
    for (Ribbon(id), mut mesh, mut vis) in &mut ribbons {
        let pl = g.w.players.get(*id);
        let live = pl.is_some_and(|p| p.alive && !p.trail.is_empty());
        let m = live
            .then(|| {
                let pts = pl.map(|p| p.trail_points()).unwrap_or_default();
                ribbon_mesh(&pts, color(*id), g.clock)
            })
            .flatten();
        match m {
            Some(m) => {
                kit.refill(&mut mesh.0, m);
                vis.set_if_neq(Visibility::Inherited);
            }
            None => {
                vis.set_if_neq(Visibility::Hidden);
            }
        }
    }
    for (Mover(id), mut t, mut vis) in &mut movers {
        let Some(pl) = g.w.players.get(*id).filter(|p| p.alive) else {
            vis.set_if_neq(Visibility::Hidden);
            continue;
        };
        vis.set_if_neq(Visibility::Inherited);
        let spawn = ease_out_back((pl.age / 0.4).min(1.0)).max(0.05);
        let bob = ((g.clock * 10.0 + *id as f32).sin() * 0.04).max(-1.0);
        let lean = (pl.spin / pl.turn.max(0.1)).clamp(-1.0, 1.0) * 0.35;
        let at = Vec3::new(pl.pos.x, SLAB + HEAD_SIZE * 0.45 + bob, pl.pos.y);
        t.translation = at;
        t.rotation = Quat::from_rotation_y(-pl.angle) * Quat::from_rotation_x(lean);
        t.scale = Vec3::splat(spawn);
    }
    for (Debris(k), mut t, mut vis) in &mut debris {
        match g.chunks.get(*k) {
            Some(c) => {
                vis.set_if_neq(Visibility::Inherited);
                let fade = (1.0 - c.age / c.life).clamp(0.0, 1.0).sqrt();
                t.translation = Vec3::new(c.pos.x, c.pos.y, c.pos.z);
                t.rotation = Quat::from_rotation_y(c.yaw) * Quat::from_rotation_x(c.yaw * 0.7);
                t.scale = Vec3::splat(c.size * fade.max(0.05));
            }
            None => {
                vis.set_if_neq(Visibility::Hidden);
            }
        }
    }
}
