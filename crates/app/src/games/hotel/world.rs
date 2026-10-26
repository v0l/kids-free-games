use super::build::{arc_dot, arc_shown, Anim, Model, Rest, Xf, ARC_DOTS, LAYER};
use super::models::*;
use super::{HotelPlay, SHEET};
use crate::game3d::*;
use bevy::ecs::hierarchy::ChildSpawnerCommands;
use bevy::prelude::*;
use glam::{vec2, vec3};
use kg_core::engine::View;
use kg_core::games::hotel::layout::*;
use kg_core::games::hotel::sim::*;
use kg_core::games::hotel::Hotel;
use kg_core::geom::{Color as Col, Rect, WHITE};
use std::collections::hash_map::DefaultHasher;
use std::f32::consts::{FRAC_PI_2, PI, TAU};
use std::hash::{Hash, Hasher};

const GOLD: Col = Col::new(1.0, 0.82, 0.24, 1.0);
const PLUM: Col = Col::new(0.24, 0.12, 0.3, 1.0);
const SHOE_COLORS: [Col; 2] = [Col::new(0.2, 0.6, 1.0, 1.0), Col::new(1.0, 0.75, 0.1, 1.0)];
const ROOM_FLOOR_Y: f32 = LAYER * 2.0;
const WATER: f32 = LAYER * 3.0;
const SPLASH_LIFE: f32 = 0.8;
const ALPHA_STEPS: f32 = 10.0;

pub struct HotelWorld;

impl Plugin for HotelWorld {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                spawn.run_if(opened(World3d::Hotel)),
                sync.run_if(is_open(World3d::Hotel)),
            )
                .chain()
                .in_set(Sync3d),
        );
    }
}

pub fn stage_sits(st: Stage) -> bool {
    matches!(st, Stage::WaitSnack(_) | Stage::Eat(..) | Stage::Toilet(..))
}

#[derive(Component)]
pub struct Root;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Sec {
    Ground,
    Shell,
    Room(usize),
    Toilets,
    Cafe,
    Pool,
    Coins,
}

#[derive(Component, Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Thing {
    Sec(Sec),
    Pad(usize),
    Guest(u32),
    Staff(usize),
    Player,
    Splash(usize),
    Body,
}

#[derive(Component)]
pub struct Built(u64);

#[derive(Component, Clone, Copy, Debug, Default, PartialEq)]
pub struct Pose {
    pub swing: f32,
    pub bob: f32,
    pub hop: f32,
    pub squash: f32,
    pub sweep: f32,
    pub size: f32,
    pub face: f32,
    pub t: f32,
    pub lie: bool,
    pub swim: bool,
    pub moving: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Holder {
    Root,
    Body,
}

struct Group {
    holder: Holder,
    model: Model,
}

impl Group {
    fn on(holder: Holder, model: Model) -> Self {
        Self { holder, model }
    }
}

fn hash_with(f: impl FnOnce(&mut DefaultHasher)) -> u64 {
    let mut h = DefaultHasher::new();
    f(&mut h);
    h.finish()
}

fn bits(c: Col) -> [u32; 4] {
    [c.r.to_bits(), c.g.to_bits(), c.b.to_bits(), c.a.to_bits()]
}

fn view_of(pen: &crate::paint::Pen) -> View {
    pen.view.unwrap_or_else(View::ipad_landscape)
}

fn spawn(mut commands: Commands) {
    spawn_world(
        &mut commands,
        Root,
        World3d::Hotel,
        Lighting {
            ambient: 700.0,
            illuminance: 9000.0,
            color: Col::new(1.0, 0.97, 0.9, 1.0),
            toward: Vec3::new(6.0, -14.0, -8.0),
        },
    );
}

fn sections(h: &Hotel) -> Vec<Sec> {
    let mut s = vec![
        Sec::Ground,
        Sec::Shell,
        Sec::Toilets,
        Sec::Cafe,
        Sec::Pool,
        Sec::Coins,
    ];
    s.extend((0..h.sim.lay.rooms.len()).map(Sec::Room));
    s
}

fn sec_key(h: &Hotel, sec: Sec) -> u64 {
    let sim = &h.sim;
    hash_with(|k| {
        sim.number().hash(k);
        sec.hash(k);
        match sec {
            Sec::Ground => {}
            Sec::Shell => sim.built.hash(k),
            Sec::Room(r) => {
                sim.room_open(r).hash(k);
                sim.decor(r).hash(k);
                sim.rooms[r].dirty.hash(k);
            }
            Sec::Toilets => {
                sim.has(Build::Toilets).hash(k);
                for st in &sim.stalls {
                    st.paper.hash(k);
                }
            }
            Sec::Cafe => sim.has(Build::Cafe).hash(k),
            Sec::Pool => sim.has(Build::Pool).hash(k),
            Sec::Coins => {
                for p in PILES {
                    sim.pile(p).hash(k);
                }
                for r in &sim.rooms {
                    r.tip.hash(k);
                }
            }
        }
    })
}

struct Stance3 {
    look: Look,
    stance: Stance,
    swim: bool,
    eat: bool,
    carry: Option<(Item, u8)>,
    size: f32,
    shoes_trail: Option<Col>,
}

fn guest_stance(g: &Guest) -> Stance3 {
    Stance3 {
        look: guest_look(g.look, g.vip),
        stance: Stance {
            sit: stage_sits(g.stage),
            lie: matches!(g.stage, Stage::Sleep(..)),
            arms_up: matches!(g.stage, Stage::Eat(..)),
            shoes: None,
        },
        swim: matches!(g.stage, Stage::Swim(..)),
        eat: matches!(g.stage, Stage::Eat(..)),
        carry: None,
        size: 0.85,
        shoes_trail: None,
    }
}

fn staff_kind(role: Build) -> u8 {
    match role {
        Build::Clerk => 0,
        Build::Maid => 1,
        Build::Janitor => 2,
        Build::Waiter => 3,
        _ => 4,
    }
}

fn staff_stance(st: &Staff) -> Stance3 {
    Stance3 {
        look: staff_look(staff_kind(st.role)),
        stance: Stance::default(),
        swim: false,
        eat: false,
        carry: st.carry.map(|i| (i, 1)),
        size: 0.85,
        shoes_trail: None,
    }
}

fn player_stance(sim: &Sim) -> Stance3 {
    let p = &sim.player;
    let shoes = match sim.shoes() {
        0 => None,
        l => Some(SHOE_COLORS[l - 1]),
    };
    Stance3 {
        look: player_look(),
        stance: Stance {
            arms_up: p.carry.is_some(),
            shoes,
            ..Default::default()
        },
        swim: false,
        eat: false,
        carry: p.carry.map(|i| (i, p.count)),
        size: 1.0,
        shoes_trail: shoes.map(|c| c.with_alpha(0.45)),
    }
}

fn stance_of(h: &Hotel, thing: Thing) -> Option<Stance3> {
    let sim = &h.sim;
    match thing {
        Thing::Guest(id) => sim.guests.iter().find(|g| g.id == id).map(guest_stance),
        Thing::Staff(k) => sim.staff.get(k).map(staff_stance),
        Thing::Player => Some(player_stance(sim)),
        _ => None,
    }
}

fn stance_key(s: &Stance3) -> u64 {
    hash_with(|k| {
        let l = &s.look;
        for c in [l.body, l.legs, l.skin, l.hair, l.hat.unwrap_or(WHITE)] {
            bits(c).hash(k);
        }
        l.hat.is_some().hash(k);
        l.wide.to_bits().hash(k);
        (s.stance.sit, s.stance.lie, s.stance.arms_up).hash(k);
        s.stance.shoes.map(bits).hash(k);
        (s.swim, s.eat).hash(k);
        s.carry.map(|(i, n)| (i as u8, n)).hash(k);
        s.size.to_bits().hash(k);
    })
}

fn wanted(h: &Hotel) -> Vec<(Thing, u64)> {
    let sim = &h.sim;
    let mut out: Vec<(Thing, u64)> = sections(h)
        .into_iter()
        .map(|s| (Thing::Sec(s), sec_key(h, s)))
        .collect();
    for i in sim.visible_pads() {
        out.push((Thing::Pad(i), hash_with(|k| (sim.number(), i).hash(k))));
    }
    for t in sim
        .guests
        .iter()
        .map(|g| Thing::Guest(g.id))
        .chain((0..sim.staff.len()).map(Thing::Staff))
        .chain([Thing::Player])
    {
        if let Some(s) = stance_of(h, t) {
            out.push((t, stance_key(&s)));
        }
    }
    for (i, sp) in h.splashes.iter().enumerate() {
        out.push((
            Thing::Splash(i),
            hash_with(|k| (sp.at.x.to_bits(), sp.at.y.to_bits()).hash(k)),
        ));
    }
    out
}

fn build(h: &Hotel, thing: Thing) -> Vec<Group> {
    match thing {
        Thing::Sec(sec) => {
            let mut m = Model::new();
            let w = Scene3 { h };
            match sec {
                Sec::Ground => w.ground(&mut m),
                Sec::Shell => w.shell(&mut m),
                Sec::Room(r) => {
                    if h.sim.room_open(r) {
                        let c = room_center(r);
                        m.xf = Xf::at(vec3(-c.x, 0.0, -c.y), 0.0, 1.0);
                        w.room(&mut m, r);
                    }
                }
                Sec::Toilets => {
                    if h.sim.lay.has_toilets && h.sim.has(Build::Toilets) {
                        w.toilets(&mut m);
                    }
                }
                Sec::Cafe => {
                    if h.sim.lay.has_cafe && h.sim.has(Build::Cafe) {
                        w.cafe(&mut m);
                    }
                }
                Sec::Pool => {
                    if h.sim.lay.has_pool && h.sim.has(Build::Pool) {
                        w.pool(&mut m);
                    }
                }
                Sec::Coins => w.coins(&mut m),
            }
            vec![Group::on(Holder::Root, m)]
        }
        Thing::Pad(_) => vec![pad_group()],
        Thing::Body => vec![],
        Thing::Splash(_) => vec![splash_group()],
        t => stance_of(h, t).map(agent_groups).unwrap_or_default(),
    }
}

fn pad_group() -> Group {
    let mut m = Model::new();
    m.flat_rect(
        Vec2::ZERO,
        vec2(1.5, 1.5),
        LAYER * 4.0,
        PLUM.with_alpha(0.4),
    );
    m.ring(Vec2::ZERO, 0.75, LAYER * 5.0, WHITE);
    m.tag(Anim::PadRing);
    for i in 0..ARC_DOTS {
        dot(
            &mut m,
            arc_dot(Vec2::ZERO, 0.75, i, LAYER * 6.0),
            rgb(120, 230, 90),
        );
        m.tag(Anim::Fill(i));
    }
    for i in 0..ARC_DOTS {
        dot(&mut m, arc_dot(Vec2::ZERO, 0.6, i, LAYER * 7.0), GOLD);
        m.tag(Anim::Charge(i));
    }
    Group::on(Holder::Root, m)
}

fn dot(m: &mut Model, at: Transform, color: Col) {
    m.pieces.push(super::build::Piece {
        shape: Shape::Disc,
        paint: Paint::new(Finish::Glow, color),
        at,
        anim: None,
    });
}

fn splash_group() -> Group {
    let mut m = Model::new();
    for i in 0..6 {
        m.sphere(
            Vec3::ZERO,
            Vec3::splat(0.05),
            Col::new(0.85, 0.95, 1.0, 1.0),
        );
        m.tag(Anim::Drop(i));
    }
    m.ring(Vec2::ZERO, 0.5, LAYER * 5.0, Col::new(1.0, 1.0, 1.0, 0.7));
    m.tag(Anim::Wave);
    Group::on(Holder::Root, m)
}

fn agent_groups(s: Stance3) -> Vec<Group> {
    let mut root = Model::new();
    let mut person = Model::new();
    if s.swim {
        wake(&mut root, WATER);
        swimmer(&mut person, &s.look);
    } else {
        if !s.stance.lie {
            root.disc(
                Vec2::ZERO,
                vec2(0.36, 0.36 * 0.9) * (s.size / 0.85).max(1.0),
                LAYER * 5.0,
                Col::new(0.1, 0.05, 0.15, 0.22),
            );
        }
        if let Some(c) = s.shoes_trail {
            root.disc(Vec2::ZERO, vec2(0.18, 0.14), LAYER * 6.0, c);
            root.tag(Anim::Trail);
        }
        body(&mut person, &s.look, s.stance);
        let local = |y: f32, fwd: f32| vec3(0.0, y / s.size, fwd / s.size);
        if s.eat {
            burger(&mut person, local(1.15, 0.2));
        }
        if let Some((item, n)) = s.carry {
            let (y, fwd) = if s.size >= 1.0 {
                (1.2, 0.3)
            } else {
                (0.95, 0.32)
            };
            for k in 0..n.max(1) {
                let p = local(y, fwd) + vec3(0.0, k as f32 * 0.2, 0.0);
                match item {
                    Item::Paper => roll(&mut person, p),
                    Item::Snack => burger(&mut person, p),
                    Item::Towel => towel(&mut person, p),
                }
            }
        }
    }
    vec![
        Group::on(Holder::Root, root),
        Group::on(Holder::Body, person),
    ]
}

fn emit(out: &mut ChildSpawnerCommands, kit: &mut Kit, m: Model) {
    for p in m.pieces {
        let mut e = out.spawn(kit.part(p.shape, p.paint, p.at));
        if let Some(a) = p.anim {
            e.insert((a, Rest(p.at)));
        }
    }
}

fn fill(commands: &mut ChildSpawnerCommands, kit: &mut Kit, groups: Vec<Group>) {
    for g in groups {
        match g.holder {
            Holder::Root => emit(commands, kit, g.model),
            Holder::Body => {
                commands
                    .spawn((
                        Thing::Body,
                        Built(0),
                        Pose::default(),
                        Transform::default(),
                        Visibility::default(),
                    ))
                    .with_children(|b| emit(b, kit, g.model));
            }
        }
    }
}

fn reconcile(
    commands: &mut Commands,
    h: &Hotel,
    kit: &mut Kit,
    root: Entity,
    things: &mut Query<Placed, Without<Eye>>,
) {
    let mut want = wanted(h);
    want.sort_unstable_by_key(|(t, _)| *t);
    let mut have = vec![false; want.len()];
    for (e, thing, _, _, _, mut built) in things.iter_mut() {
        if *thing == Thing::Body {
            continue;
        }
        match want.binary_search_by_key(thing, |(t, _)| *t) {
            Err(_) => commands.entity(e).despawn(),
            Ok(i) => {
                have[i] = true;
                let key = want[i].1;
                if built.0 != key {
                    built.0 = key;
                    let groups = build(h, *thing);
                    let mut ec = commands.entity(e);
                    ec.despawn_children();
                    ec.with_children(|c| fill(c, kit, groups));
                }
            }
        }
    }
    for (i, (thing, key)) in want.into_iter().enumerate() {
        if have[i] {
            continue;
        }
        let groups = build(h, thing);
        commands.entity(root).with_children(|w| {
            w.spawn((
                thing,
                Built(key),
                Pose::default(),
                Transform::default(),
                Visibility::default(),
            ))
            .with_children(|c| fill(c, kit, groups));
        });
    }
}

fn agent_pose(h: &Hotel, thing: Thing) -> Option<(Vec2, Pose)> {
    let sim = &h.sim;
    match thing {
        Thing::Guest(id) => {
            let g = sim.guests.iter().find(|g| g.id == id)?;
            if let Stage::Swim(_, t) = g.stage {
                return Some((
                    swim_pos(&sim.lay.pool, t, g.id),
                    Pose {
                        face: swim_dir(t),
                        t: h.t + g.id as f32,
                        swim: true,
                        size: 0.85,
                        ..Default::default()
                    },
                ));
            }
            let lie = matches!(g.stage, Stage::Sleep(..));
            let mut pos = g.pos;
            let mut face = g.face;
            if let Stage::Sleep(k, _) = g.stage {
                pos = room_bed(k);
                face = bed_yaw(k) + PI;
            }
            if let Stage::WaitTowel(b) = g.stage {
                pos = sim.lay.sunbeds[b] + vec2(0.0, 0.7);
            }
            let walk = if g.path.is_empty() { 0.0 } else { g.walk };
            let hop = if g.happy && !g.path.is_empty() {
                (h.t * 9.0 + g.id as f32).sin().abs() * 0.05
            } else {
                0.0
            };
            Some((
                pos,
                walking(
                    walk,
                    face,
                    0.85,
                    Pose {
                        hop,
                        lie,
                        ..Default::default()
                    },
                ),
            ))
        }
        Thing::Staff(k) => {
            let st = sim.staff.get(k)?;
            let walk = if st.path.is_empty() { 0.0 } else { st.walk };
            let sweep = if st.role == Build::Maid && st.path.is_empty() {
                st.work
            } else {
                0.0
            };
            Some((
                st.pos,
                walking(
                    walk,
                    st.face,
                    0.85,
                    Pose {
                        sweep,
                        ..Default::default()
                    },
                ),
            ))
        }
        Thing::Player => {
            let p = &sim.player;
            let squash = if p.moving {
                0.0
            } else {
                (h.t * 2.4).sin() * 0.015
            };
            let walk = if p.moving { p.walk } else { 0.0 };
            Some((
                p.pos,
                walking(
                    walk,
                    p.face,
                    1.0,
                    Pose {
                        squash,
                        sweep: p.work,
                        moving: p.moving,
                        ..Default::default()
                    },
                ),
            ))
        }
        _ => None,
    }
}

fn walking(walk: f32, face: f32, size: f32, base: Pose) -> Pose {
    let (swing, bob) = if walk > 0.0 {
        (walk.sin(), (walk * 2.0).sin().abs() * 0.06)
    } else {
        (0.0, 0.0)
    };
    Pose {
        swing,
        bob,
        face,
        size,
        ..base
    }
}

pub fn body_transform(p: &Pose) -> Transform {
    let yaw = Quat::from_rotation_y(p.face);
    if p.swim {
        return Transform::from_xyz(0.0, WATER, 0.0)
            .with_rotation(yaw)
            .with_scale(Vec3::splat(0.85));
    }
    if p.lie {
        return Transform::from_xyz(0.0, p.bob + p.hop + 0.55, 0.0)
            .with_rotation(yaw)
            .with_scale(Vec3::splat(p.size));
    }
    let sq = 1.0 + p.squash;
    Transform::from_xyz(0.0, p.bob + p.hop, 0.0)
        .with_rotation(yaw)
        .with_scale(Vec3::new(
            p.size / sq.sqrt(),
            p.size * sq,
            p.size / sq.sqrt(),
        ))
}

fn pad_pulse(h: &Hotel) -> f32 {
    1.0 + (h.t * 3.0).sin() * 0.03
}

fn room_pop(h: &Hotel, r: usize) -> f32 {
    let r = r as u8;
    h.pop_of(Build::Room(r))
        .min(h.pop_of(Build::Decor(r, 2)))
        .min(h.pop_of(Build::Decor(r, 3)))
}

type Placed<'a> = (
    Entity,
    &'a Thing,
    Option<&'a ChildOf>,
    &'a mut Transform,
    &'a mut Pose,
    &'a mut Built,
);

#[allow(clippy::too_many_arguments)]
fn sync(
    mut commands: Commands,
    frame: NonSend<crate::Frame>,
    pen: Res<crate::paint::Pen>,
    mut kit: Kit,
    root: Query<Entity, With<Root>>,
    mut eye: Lens,
    mut things: Query<Placed, Without<Eye>>,
    mut parts: Query<Animated, (Without<Thing>, Without<Eye>)>,
) {
    let (Some(p), Ok(root)) = (playing::<HotelPlay>(&frame), root.single()) else {
        return;
    };
    reconcile(&mut commands, &p.h, &mut kit, root, &mut things);
    place(&p.h, &pen, &mut eye, &mut things);
    animate(&p.h, &mut kit, &mut parts, &things);
}

fn place(
    h: &Hotel,
    pen: &crate::paint::Pen,
    eye: &mut Lens,
    things: &mut Query<Placed, Without<Eye>>,
) {
    let cam = h.cam(&view_of(pen));
    let shake = h.shake.offset();
    let nudge = cam.right() * shake.x * 0.02 + cam.up() * -shake.y * 0.02;
    {
        eye.aim(Rig {
            eye: cam.eye + nudge,
            target: cam.target + nudge,
            fov: cam.fov,
            near: 0.5,
            far: 200.0,
            sky: h.sim.lay.theme.outside,
            shadow_reach: (40.0 / h.zoom.max(0.3)).max(20.0),
        });
    }
    let mut posed = vec![];
    let mut bodies = vec![];
    for (e, thing, parent, mut t, mut pose, _) in things.iter_mut() {
        let want = match *thing {
            Thing::Body => {
                if let Some(p) = parent {
                    bodies.push((e, p.parent()));
                }
                continue;
            }
            Thing::Sec(Sec::Room(r)) => {
                let c = room_center(r);
                Transform::from_xyz(c.x, 0.0, c.y).with_scale(Vec3::splat(room_pop(h, r)))
            }
            Thing::Sec(_) | Thing::Splash(_) => Transform::default(),
            Thing::Pad(i) => {
                let at = h.sim.lay.pads[i].pos;
                let k = pad_pulse(h);
                Transform::from_xyz(at.x, 0.0, at.y).with_scale(Vec3::new(k, 1.0, k))
            }
            agent => match agent_pose(h, agent) {
                Some((at, ap)) => {
                    posed.push((e, ap));
                    pose.set_if_neq(ap);
                    Transform::from_xyz(at.x, shadow_tie(agent), at.y)
                }
                None => *t,
            },
        };
        t.set_if_neq(want);
    }
    for (body, owner) in bodies {
        let pose = posed.iter().find(|(e, _)| *e == owner).map(|(_, p)| p);
        if let (Some(pose), Ok((_, _, _, mut t, mut p, _))) = (pose, things.get_mut(body)) {
            p.set_if_neq(*pose);
            t.set_if_neq(body_transform(pose));
        }
    }
}

pub fn shadow_tie(thing: Thing) -> f32 {
    let n = match thing {
        Thing::Guest(id) => id as usize * 7 + 1,
        Thing::Staff(k) => k * 7 + 3,
        _ => 5,
    };
    (n % 17) as f32 / 17.0 * LAYER * 0.9
}

fn arm_swing(p: &Pose, side: f32) -> f32 {
    if p.sweep > 0.0 {
        (p.sweep * 14.0).sin() * 0.18 * side
    } else {
        -p.swing * side * 0.14
    }
}

fn faded(a: f32) -> f32 {
    ((a * ALPHA_STEPS).ceil() / ALPHA_STEPS).clamp(0.0, 1.0)
}

type Animated<'a> = (
    &'a ChildOf,
    &'a Rest,
    &'a Anim,
    &'a mut Transform,
    &'a mut Visibility,
    &'a mut Part,
    &'a mut MeshMaterial3d<StandardMaterial>,
);

fn animate(
    h: &Hotel,
    kit: &mut Kit,
    parts: &mut Query<Animated, (Without<Thing>, Without<Eye>)>,
    owners: &Query<Placed, Without<Eye>>,
) {
    let sim = &h.sim;
    for (parent, rest, anim, mut t, mut vis, mut part, mut mat) in parts.iter_mut() {
        let Ok((_, mut thing, up, _, pose, _)) = owners.get(parent.parent()) else {
            continue;
        };
        if *thing == Thing::Body {
            if let Some(Ok((_, owner, ..))) = up.map(|u| owners.get(u.parent())) {
                thing = owner;
            }
        }
        let pose = *pose;
        let thing = Some(thing);
        let r = rest.0;
        let mut want = r;
        let mut show = true;
        match *anim {
            Anim::Bob => want.translation.y += coin_bob(h.t),
            Anim::Orbit(at, phase) => {
                let d = orbit(at, phase, h.t) - at;
                want.translation += Vec3::new(d.x, 0.0, d.y);
            }
            Anim::Ripple(phase, amp) => want.translation.z += (h.t * 1.3 + phase).sin() * amp,
            Anim::Leg(side) => want.translation.z += pose.swing * side * 0.12,
            Anim::Arm(side) => want.translation.z += arm_swing(&pose, side),
            Anim::Stroke(side) => want.translation = stroke(side, pose.t),
            Anim::Kick(side) => want.translation = kick(side, pose.t),
            Anim::Turn => want.translation.x += head_turn(pose.t),
            Anim::Wake(k) => {
                let rr = wake_radius(k, h.t);
                let back = -vec2(pose.face.sin(), pose.face.cos()) * (0.35 + k as f32 * 0.32);
                want.translation = Vec3::new(back.x, r.translation.y, back.y);
                want.scale = Vec3::new(rr * 2.0, 1.0, rr * 1.6);
            }
            _ => {}
        }
        match (*anim, thing) {
            (Anim::PadRing, Some(Thing::Pad(i))) => {
                let pad = &sim.lay.pads[*i];
                let afford = sim.wallet + sim.paid[*i] >= pad.cost;
                let c = if afford {
                    WHITE
                } else {
                    WHITE.with_alpha(0.55)
                };
                let finish = if afford { Finish::Glow } else { Finish::Ghost };
                kit.repaint(&mut part, &mut mat, Paint::new(finish, c));
            }
            (Anim::Fill(n), Some(Thing::Pad(i))) => {
                let k = sim.paid[*i] as f32 / sim.lay.pads[*i].cost as f32;
                show = k > 0.0 && n < arc_shown(k);
            }
            (Anim::Charge(n), Some(Thing::Pad(i))) => {
                let afford = sim.wallet + sim.paid[*i] >= sim.lay.pads[*i].cost;
                let arming = sim.pad_under_player() == Some(*i) && sim.pad_arm < PAD_ARM && afford;
                show = arming && n < arc_shown(sim.pad_arm / PAD_ARM);
            }
            (Anim::Trail, _) => {
                show = pose.moving;
                let back = -vec2(pose.face.sin(), pose.face.cos()) * 0.45;
                want.translation = Vec3::new(back.x, r.translation.y, back.y);
            }
            (Anim::Drop(n), Some(Thing::Splash(i))) => {
                if let Some(sp) = h.splashes.get(*i) {
                    let k = (sp.age / SPLASH_LIFE).min(1.0);
                    let a = n as f32 / 6.0 * TAU;
                    let rad = 0.2 + k * 0.6;
                    let up = (k * PI).sin() * 0.6;
                    want.translation =
                        Vec3::new(sp.at.x + a.cos() * rad, 0.1 + up, sp.at.y + a.sin() * rad);
                    want.scale = Vec3::splat((0.08 * (1.0 - k) + 0.02) * 2.0);
                    let paint =
                        Paint::new(Finish::Gloss, Col::new(0.85, 0.95, 1.0, faded(1.0 - k)));
                    let paint = if paint.color.a < 0.99 {
                        Paint::new(Finish::Ghost, paint.color)
                    } else {
                        paint
                    };
                    kit.repaint(&mut part, &mut mat, paint);
                }
            }
            (Anim::Wave, Some(Thing::Splash(i))) => {
                if let Some(sp) = h.splashes.get(*i) {
                    let k = (sp.age / SPLASH_LIFE).min(1.0);
                    let rad = 0.3 + k * 0.9;
                    want.translation = Vec3::new(sp.at.x, r.translation.y, sp.at.y);
                    want.scale = Vec3::new(rad * 2.0, 1.0, rad * 2.0);
                    kit.repaint(
                        &mut part,
                        &mut mat,
                        Paint::new(Finish::Ghost, WHITE.with_alpha(faded(0.7 * (1.0 - k)))),
                    );
                }
            }
            _ => {}
        }
        t.set_if_neq(want);
        vis.set_if_neq(if show {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        });
    }
}

struct Scene3<'a> {
    h: &'a Hotel,
}

impl Scene3<'_> {
    fn zone_open(&self, b: Build) -> bool {
        self.h.sim.has(b)
    }

    fn ground(&self, s: &mut Model) {
        let lay = &self.h.sim.lay;
        let th = lay.theme;
        s.flat_rect(
            vec2(0.0, (LOBBY_S + OUTSIDE_S) / 2.0 + 1.0),
            vec2(4.0, OUTSIDE_S - LOBBY_S + 2.0),
            0.0,
            th.path,
        );
        s.flat_rect(
            vec2(0.0, (LOBBY_S + lay.end_z()) / 2.0),
            vec2(CORR * 2.0 - 0.3, LOBBY_S - lay.end_z()),
            LAYER,
            th.carpet,
        );
        let hall = Rect::new(-HALL_W, lay.end_z(), HALL_W * 2.0, LOBBY_S - lay.end_z());
        s.floor_at(hall, 0.0, 2.5, SHEET, "parquet", th.floor_tint);
        for k in 0..3 {
            let x = [-4.6, 4.6, -4.6][k];
            let z = [OUTSIDE_S - 1.5, OUTSIDE_S - 1.5, OUTSIDE_S - 3.6][k];
            plant(s, vec2(x, z), 1.6);
        }
        for (i, c) in [rgb(240, 80, 80), rgb(80, 150, 240)].iter().enumerate() {
            car(s, vec2(-4.5 + i as f32 * 9.0, OUTSIDE_S - 0.2 + 1.6), *c);
        }
        desk(s, lay.desk, &th);
        sofa(s, vec2(-HALL_W + 0.7, 6.0), th.accent, FRAC_PI_2);
        sofa(s, vec2(HALL_W - 0.7, 6.0), th.accent, -FRAC_PI_2);
        plant(s, vec2(-CORR - 0.5, LOBBY_S - 0.5), 1.2);
        plant(s, vec2(CORR + 0.5, LOBBY_S - 0.5), 1.2);
        for side in [-1.0, 1.0] {
            s.cuboid2(
                vec3(side * (CORR + 0.15), 0.0, LOBBY_S),
                vec3(0.3, 2.0, 0.34),
                th.trim,
                darker(th.trim, 1.1),
            );
        }
        s.cuboid2(
            vec3(0.0, 2.0, LOBBY_S),
            vec3(CORR * 2.0 + 0.6, 0.35, 0.34),
            th.trim,
            darker(th.trim, 1.1),
        );
    }

    fn shell(&self, s: &mut Model) {
        let sim = &self.h.sim;
        let lay = &sim.lay;
        let th = lay.theme;
        let locked_zone = Col::new(0.3, 0.3, 0.35, 0.35);
        let zone = |s: &mut Model, r: Rect, y: f32, c: Col| {
            s.flat_rect(vec2(r.x + r.w / 2.0, r.y + r.h / 2.0), vec2(r.w, r.h), y, c);
        };
        if lay.has_toilets {
            if self.zone_open(Build::Toilets) {
                s.floor_at(
                    lay.toilet_room,
                    ROOM_FLOOR_Y,
                    2.5,
                    SHEET,
                    "parquet",
                    rgb(210, 235, 245),
                );
            } else {
                zone(s, lay.toilet_room, LAYER * 3.0, locked_zone);
            }
        }
        if lay.has_cafe {
            if self.zone_open(Build::Cafe) {
                s.floor_at(
                    lay.cafe,
                    ROOM_FLOOR_Y,
                    2.5,
                    SHEET,
                    "parquet",
                    rgb(255, 225, 190),
                );
            } else {
                zone(s, lay.cafe, LAYER * 3.0, locked_zone);
            }
        }
        if lay.has_pool {
            let ground = if self.zone_open(Build::Pool) {
                rgb(235, 225, 200)
            } else {
                darker(th.outside, 0.85)
            };
            zone(s, lay.garden, LAYER, ground);
        }
        for (k, r) in lay.rooms.iter().enumerate() {
            let tint = if sim.room_open(k) {
                th.room_floor
            } else {
                Col::new(0.5, 0.5, 0.55, 1.0)
            };
            s.floor_at(*r, ROOM_FLOOR_Y, 2.0, SHEET, "roomfloor", tint);
        }
        let built: Vec<Build> = lay
            .pads
            .iter()
            .zip(&sim.built)
            .filter(|(_, b)| **b)
            .map(|(p, _)| p.build)
            .collect();
        let things: Vec<Rect> = lay
            .solids(&|b| built.contains(&b))
            .into_iter()
            .filter(|(_, k)| *k != Solid::Wall)
            .map(|(r, _)| r)
            .collect();
        for w in sim.walls() {
            if things.contains(w) {
                continue;
            }
            let c = vec3(w.x + w.w / 2.0, 0.0, w.y + w.h / 2.0);
            s.cuboid2(c, vec3(w.w, 1.2, w.h), th.wall, th.trim);
        }
        for w in sim.locked() {
            locked_door(s, *w, &th);
        }
    }

    fn room(&self, s: &mut Model, r: usize) {
        let sim = &self.h.sim;
        let th = sim.lay.theme;
        let decor = sim.decor(r);
        let room = &sim.rooms[r];
        let spots = sim.spots(r);
        let dirty = |kind: SpotKind| {
            spots
                .iter()
                .position(|sp| sp.kind == kind)
                .map(|i| room.dirty[i])
                .unwrap_or(false)
        };
        let sheet = match decor {
            1 => rgb(150, 180, 230),
            2 => rgb(255, 255, 255),
            _ => th.accent,
        };
        let wood = match decor {
            1 => rgb(180, 120, 80),
            2 => th.trim,
            _ => rgb(250, 205, 80),
        };
        let bx = s.xf;
        s.xf = bx.then(Xf::at(
            vec3(room_bed(r).x, 0.0, room_bed(r).y),
            bed_yaw(r),
            if decor >= 3 { 1.12 } else { 1.0 },
        ));
        bed(
            s,
            Vec2::ZERO,
            if dirty(SpotKind::Bed) {
                rgb(200, 190, 175)
            } else {
                sheet
            },
            wood,
            decor >= 3,
        );
        s.xf = bx;
        if dirty(SpotKind::Bed) {
            let b = room_bed(r);
            s.sphere(
                vec3(b.x, 0.68, b.y + 0.3),
                vec3(0.5, 0.15, 0.4),
                rgb(220, 215, 205),
            );
        }
        let bin_at = spots
            .iter()
            .find(|sp| sp.kind == SpotKind::Bin)
            .map(|sp| sp.obj)
            .unwrap_or_default();
        bin(s, bin_at, dirty(SpotKind::Bin));
        plant(s, room_pt(r, 4.0, 2.2), 1.0);
        if decor >= 2 || sim.lay.extra_spots > 0 {
            let at = room_pt(r, 1.5, 1.0);
            rug(
                s,
                at,
                if decor >= 3 {
                    th.accent
                } else {
                    rgb(240, 200, 120)
                },
            );
            if dirty(SpotKind::Rug) {
                dust(s, at);
            }
        }
        if decor >= 2 {
            wardrobe(s, room_pt(r, 4.0, -2.15), wood, 0.0);
            lamp(s, room_pt(r, 4.2, 1.4), th.accent);
            if dirty(SpotKind::Wardrobe) {
                dust(s, room_pt(r, 3.4, -1.45));
            }
        }
        if decor >= 3 {
            tv(s, room_pt(r, 0.3, 2.1), PI);
            if dirty(SpotKind::Tv) {
                dust(s, room_pt(r, 0.95, 1.6));
            }
        }
    }

    fn toilets(&self, s: &mut Model) {
        let sim = &self.h.sim;
        let lay = &sim.lay;
        let r = lay.toilet_room;
        for (k, st) in lay.stalls.iter().enumerate() {
            let outer = s.xf;
            s.xf = outer.then(Xf::at(vec3(st.x, 0.0, st.y), FRAC_PI_2, 1.0));
            toilet(s, Vec2::ZERO);
            s.xf = outer;
            for q in 0..sim.stalls[k].paper {
                roll(s, vec3(r.x + 0.2, 0.55 + q as f32 * 0.22, st.y - 0.45));
            }
        }
        for k in 0..=lay.stalls.len() {
            let z = SIDE_N + 0.3 + STALL_H * k as f32;
            s.cuboid(
                vec3(r.x + 0.8, 0.0, z),
                vec3(1.6, 1.3, 0.1),
                rgb(120, 200, 235),
            );
        }
        shelf(s, lay.paper_shelf, WHITE);
        for q in 0..3 {
            roll(
                s,
                vec3(
                    lay.paper_shelf.x - 0.15 + q as f32 * 0.15,
                    0.95,
                    lay.paper_shelf.y,
                ),
            );
        }
        s.cuboid2(
            vec3(r.x + r.w - 0.5, 0.0, r.y + r.h - 0.9),
            vec3(0.6, 0.8, 1.2),
            WHITE,
            rgb(220, 240, 250),
        );
    }

    fn cafe(&self, s: &mut Model) {
        let lay = &self.h.sim.lay;
        let th = lay.theme;
        let c = lay.counter;
        s.cuboid2(
            vec3(c.x + c.w / 2.0, 0.0, c.y + c.h / 2.0),
            vec3(c.w, 1.0, c.h),
            rgb(240, 140, 60),
            rgb(255, 215, 130),
        );
        for k in 0..3 {
            burger(s, vec3(c.x + c.w / 2.0, 1.0, c.y + 0.5 + k as f32 * 1.0));
        }
        for tb in &lay.tables {
            s.cylinder(vec3(tb.x, 0.0, tb.y), 0.08, 0.7, 6, rgb(90, 90, 100));
            s.cylinder(vec3(tb.x, 0.7, tb.y), 0.5, 0.06, 14, rgb(230, 230, 235));
            for p in seat_spots(*tb) {
                s.cylinder(vec3(p.x, 0.0, p.y), 0.25, 0.42, 8, th.accent);
            }
        }
    }

    fn pool(&self, s: &mut Model) {
        let lay = &self.h.sim.lay;
        let th = lay.theme;
        let p = lay.pool;
        let c = vec2(p.x + p.w / 2.0, p.y + p.h / 2.0);
        s.flat_rect(
            c,
            vec2(p.w + 0.9, p.h + 0.9),
            LAYER * 2.0,
            rgb(245, 245, 252),
        );
        s.flat_rect(c, vec2(p.w, p.h), WATER, rgb(40, 160, 230));
        for k in 0..6 {
            let x = p.x + 0.6 + k as f32 * (p.w - 1.2) / 5.0;
            s.disc(
                vec2(x, c.y),
                vec2(0.5, 0.12),
                LAYER * 4.0,
                Col::new(0.6, 0.88, 1.0, 0.6),
            );
            s.tag(Anim::Ripple(k as f32, p.h * 0.3));
        }
        for k in 1..3 {
            let z = p.y + k as f32 * p.h / 3.0;
            s.flat_rect(
                vec2(c.x, z),
                vec2(p.w - 0.4, 0.05),
                0.011,
                rgb(255, 240, 120),
            );
        }
        let rim = rgb(250, 250, 255);
        let e = 0.22;
        s.cuboid(
            vec3(c.x, 0.0, p.y - e / 2.0),
            vec3(p.w + 2.0 * e, 0.16, e),
            rim,
        );
        s.cuboid(
            vec3(c.x, 0.0, p.y + p.h + e / 2.0),
            vec3(p.w + 2.0 * e, 0.16, e),
            rim,
        );
        s.cuboid(vec3(p.x - e / 2.0, 0.0, c.y), vec3(e, 0.16, p.h), rim);
        s.cuboid(vec3(p.x + p.w + e / 2.0, 0.0, c.y), vec3(e, 0.16, p.h), rim);
        for (k, sb) in lay.sunbeds.iter().enumerate() {
            s.cuboid2(
                vec3(sb.x, 0.0, sb.y),
                vec3(0.7, 0.3, 1.5),
                WHITE,
                if k % 2 == 0 {
                    th.accent
                } else {
                    rgb(255, 210, 80)
                },
            );
            if k % 2 == 0 {
                umbrella(s, *sb + vec2(0.8, -0.3), th.trim);
            }
        }
        shelf(s, lay.towel_shelf, rgb(120, 190, 250));
    }

    fn coins(&self, s: &mut Model) {
        let sim = &self.h.sim;
        for p in PILES {
            let amount = sim.pile(p);
            if amount > 0 {
                coin_stack(s, sim.pile_pos(p), amount);
            }
        }
        for (k, room) in sim.rooms.iter().enumerate() {
            if room.tip > 0 {
                coin_stack(s, room_pt(k, 1.6, 1.2), room.tip);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game3d::testing;
    use crate::games::Play;
    use kg_core::engine::Input;

    const LAYER_GAP: f32 = LAYER;

    fn fresh(number: u32, built: &[bool]) -> HotelPlay {
        kg_core::save::reset_for_test();
        kg_core::profile::create("Mia", 0);
        HotelPlay::with(Hotel::with(Sim::new(number, built, 50, 1)))
    }

    fn open(p: HotelPlay, v: View) -> App {
        let mut app = testing::with_game(Box::new(p), v);
        testing::step(&mut app, 3);
        app
    }

    fn hotel(app: &mut App) -> &mut HotelPlay {
        testing::game::<HotelPlay>(app)
    }

    fn things(app: &mut App, f: impl Fn(&Thing) -> bool) -> usize {
        let w = app.world_mut();
        w.query::<&Thing>().iter(w).filter(|t| f(t)).count()
    }

    type Layer = (f32, f32, f32, f32, f32, String);

    fn flat_layers(app: &mut App) -> Vec<Layer> {
        testing::parts(app)
            .into_iter()
            .filter(|(p, g)| {
                matches!(p.shape, Shape::Plane | Shape::Disc)
                    && g.to_scale_rotation_translation().1.is_near_identity()
            })
            .map(|(p, g)| {
                let (s, _, at) = g.to_scale_rotation_translation();
                (
                    at.x - s.x / 2.0,
                    at.x + s.x / 2.0,
                    at.z - s.z / 2.0,
                    at.z + s.z / 2.0,
                    at.y,
                    format!("{:?} {:?}", p.shape, p.paint.finish),
                )
            })
            .collect()
    }

    #[test]
    fn no_two_overlapping_floors_share_a_height() {
        let all: Vec<bool> = vec![true; 64];
        for number in 1..=4 {
            for v in [View::ipad_landscape(), View::ipad_portrait()] {
                let mut p = fresh(number, &all);
                p.h.sim.add_swimmers(3);
                for _ in 0..20 {
                    p.update(&Input::default(), &v, 1.0 / 30.0);
                }
                let mut app = open(p, v);
                let layers = flat_layers(&mut app);
                assert!(
                    layers.len() > 20,
                    "hotel {number}: {} flat layers",
                    layers.len()
                );
                for (i, a) in layers.iter().enumerate() {
                    for b in &layers[i + 1..] {
                        let overlap =
                            (a.1.min(b.1) - a.0.max(b.0)).min(a.3.min(b.3) - a.2.max(b.2));
                        let both_ghosts = a.5.contains("Ghost") && b.5.contains("Ghost");
                        let need = if both_ghosts { 1e-3 } else { LAYER_GAP * 0.99 };
                        if overlap > 0.05 {
                            assert!(
                                (a.4 - b.4).abs() >= need,
                                "hotel {number}: {} at y {} and {} at y {} overlap",
                                a.5,
                                a.4,
                                b.5,
                                b.4
                            );
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn opening_spawns_the_hotel_once_and_closing_despawns_all_of_it() {
        let mut app = testing::app(crate::Scene::Menu(Box::default()), View::ipad_landscape());
        testing::step(&mut app, 3);
        let menu = testing::count(&mut app);
        for _ in 0..3 {
            if let crate::Scene::Menu(m) = &mut testing::frame(&mut app).scene {
                m.current = Some((0, Box::new(fresh(1, &[]))));
            }
            testing::step(&mut app, 3);
            let open = testing::count(&mut app);
            assert!(open > menu + 150, "{menu} -> {open}");
            assert_eq!(testing::open(&app), World3d::Hotel);
            testing::step(&mut app, 5);
            assert_eq!(
                testing::count(&mut app),
                open,
                "a quiet hotel spawns nothing per frame"
            );
            testing::close(&mut app);
            testing::step(&mut app, 3);
            assert_eq!(testing::count(&mut app), menu);
        }
    }

    #[test]
    fn buying_a_room_rebuilds_only_that_room() {
        let lay = Layout::new(2);
        let at = |b: Build| lay.pads.iter().position(|p| p.build == b).unwrap();
        let mut built = vec![false; lay.pads.len()];
        built[at(Build::Room(1))] = true;
        let mut app = open(fresh(2, &built), View::ipad_landscape());
        let before = testing::count(&mut app);
        let ids = |app: &mut App, sec: Sec| {
            let w = app.world_mut();
            let e = w
                .query::<(Entity, &Thing)>()
                .iter(w)
                .find(|(_, t)| **t == Thing::Sec(sec))
                .unwrap()
                .0;
            w.get::<Children>(e).map(|c| c.to_vec()).unwrap_or_default()
        };
        let ground = ids(&mut app, Sec::Ground);
        built[at(Build::Room(2))] = true;
        let wallet = hotel(&mut app).h.sim.wallet;
        hotel(&mut app).h.sim = Sim::new(2, &built, wallet, 1);
        testing::step(&mut app, 2);
        assert!(testing::count(&mut app) > before + 30);
        assert_eq!(
            ids(&mut app, Sec::Ground),
            ground,
            "untouched sections keep their entities"
        );
        assert!(!ids(&mut app, Sec::Room(2)).is_empty());
    }

    #[test]
    fn guests_arrive_and_leave_as_their_own_entities() {
        let mut app = open(fresh(1, &[]), View::ipad_landscape());
        let guests = |app: &mut App| things(app, |t| matches!(t, Thing::Guest(_)));
        assert_eq!(guests(&mut app), hotel(&mut app).h.sim.guests.len());
        let v = View::ipad_landscape();
        let mut seen = 0;
        for _ in 0..60 * 40 {
            let h = hotel(&mut app);
            h.h.sim.autopilot(1.0 / 60.0);
            h.update(&Input::default(), &v, 1.0 / 60.0);
            testing::step(&mut app, 1);
            seen = seen.max(guests(&mut app));
            let n = hotel(&mut app).h.sim.guests.len();
            assert_eq!(guests(&mut app), n);
        }
        assert!(seen >= 2, "guests came: {seen}");
    }

    #[test]
    fn a_walking_player_swings_their_legs() {
        let mut app = open(fresh(1, &[]), View::ipad_landscape());
        let legs = |app: &mut App| {
            let w = app.world_mut();
            w.query::<(&Anim, &Transform)>()
                .iter(w)
                .filter(|(a, _)| matches!(a, Anim::Leg(_)))
                .map(|(_, t)| t.translation.z)
                .collect::<Vec<_>>()
        };
        let still = legs(&mut app);
        let p = &mut hotel(&mut app).h.sim.player;
        p.moving = true;
        p.walk = 1.2;
        testing::step(&mut app, 1);
        let moved = legs(&mut app);
        assert_eq!(still.len(), moved.len());
        assert!(still.iter().zip(&moved).any(|(a, b)| (a - b).abs() > 0.05));
    }

    #[test]
    fn a_build_spot_fills_its_ring_as_you_pay() {
        let mut app = open(fresh(1, &[]), View::ipad_landscape());
        let shown = |app: &mut App| {
            let w = app.world_mut();
            let all: Vec<(Entity, Anim)> = w
                .query::<(Entity, &Anim)>()
                .iter(w)
                .map(|(e, m)| (e, *m))
                .collect();
            all.into_iter()
                .filter(|(e, m)| matches!(m, Anim::Fill(_)) && visible(w, *e))
                .count()
        };
        assert_eq!(shown(&mut app), 0);
        let h = &mut hotel(&mut app).h.sim;
        let i = h.visible_pads()[0];
        h.paid[i] = h.lay.pads[i].cost / 2;
        testing::step(&mut app, 1);
        assert_eq!(shown(&mut app), ARC_DOTS / 2);
    }

    #[test]
    fn every_hotel_fully_built_spawns_its_world() {
        for n in 1..=MAX_HOTEL {
            let pads = Layout::new(n).pads.len();
            let mut p = fresh(n, &vec![true; pads - 1]);
            p.h.sim.add_swimmers(3);
            let mut app = open(p, View::ipad_portrait());
            let parts = testing::parts(&mut app).len();
            assert!(parts > 300, "hotel {n}: {parts} parts");
        }
    }

    #[test]
    fn moving_to_the_next_hotel_swaps_the_whole_building() {
        let mut app = open(fresh(1, &[]), View::ipad_landscape());
        let one = things(&mut app, |t| matches!(t, Thing::Sec(Sec::Room(_))));
        hotel(&mut app).h.sim = Sim::new(4, &[], 0, 1);
        testing::step(&mut app, 2);
        let four = things(&mut app, |t| matches!(t, Thing::Sec(Sec::Room(_))));
        assert_eq!(four, hotel(&mut app).h.sim.lay.rooms.len());
        assert!(four > one);
    }
}
