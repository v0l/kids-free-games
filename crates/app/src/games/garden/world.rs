use super::{GardenPlay, SHEET};
use crate::game3d::*;
use bevy::prelude::*;
use kg_core::engine::View;
use kg_core::games::garden::*;
use kg_core::geom::{Color as Col, WHITE};

const SKY: Col = Col::new(0.55, 0.8, 1.0, 1.0);
const MAX_FLOWERS: usize = 144;
const BEDS: usize = LEVELS;
const CRITTERS: usize = 4;
const FLOWER_H: f32 = 1.5;
const SPROUT_AT: f32 = 0.35;
const LAYER: f32 = 0.02;
const OVER_ROWS: usize = 3;
const OVER_COLS: usize = 4;
const NAMES: [&str; 6] = ["red", "yellow", "pink", "blue", "purple", "orange"];

pub struct GardenWorld;

impl Plugin for GardenWorld {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                spawn.run_if(opened(World3d::Garden)),
                sync.run_if(is_open(World3d::Garden)),
            )
                .chain()
                .in_set(Sync3d),
        );
    }
}

#[derive(Component, Clone, Copy, PartialEq, Eq, Debug)]
pub enum Role {
    Ground,
    Soil(usize),
    Shade(usize),
    Flower(usize),
    Critter(usize),
    Can,
}

fn sprite(name: &'static str) -> Paint {
    Paint::skinned(
        Finish::Sticker,
        WHITE,
        Skin::Sprite(SHEET, name, [0.0, 0.0, 1.0, 1.0]),
    )
}

fn soil_paint(alpha: f32) -> Paint {
    Paint::skinned(
        Finish::Backdrop,
        WHITE.with_alpha(alpha),
        Skin::Tile(SHEET, "soil", Vec2::splat(1.0)),
    )
}

fn spawn(mut commands: Commands, mut kit: Kit) {
    let world = spawn_world(
        &mut commands,
        (),
        World3d::Garden,
        Lighting {
            ambient: 600.0,
            illuminance: 6000.0,
            color: WHITE,
            toward: Vec3::new(-0.4, -1.0, -0.5),
        },
    );
    commands.entity(world.root).with_children(|w| {
        let mut put = |role: Role, shape: Shape, paint: Paint, at: Transform| {
            w.spawn((kit.part(shape, paint, at), role));
        };
        put(
            Role::Ground,
            Shape::Plane,
            Paint::new(Finish::Backdrop, Col::new(0.42, 0.74, 0.28, 1.0)),
            Transform::from_scale(Vec3::new(80.0, 1.0, 80.0)),
        );
        for b in 0..BEDS {
            put(
                Role::Soil(b),
                Shape::Plane,
                soil_paint(1.0),
                Transform::from_xyz(0.0, LAYER, 0.0),
            );
        }
        for i in 0..MAX_FLOWERS {
            put(
                Role::Shade(i),
                Shape::Disc,
                Paint::new(Finish::Glow, Col::new(0.1, 0.05, 0.0, 0.3)),
                Transform::from_xyz(0.0, LAYER * 3.0, 0.0),
            );
        }
        for i in 0..MAX_FLOWERS {
            put(
                Role::Flower(i),
                Shape::Quad,
                sprite("sprout"),
                Transform::default(),
            );
        }
        for k in 0..CRITTERS {
            put(
                Role::Critter(k),
                Shape::Quad,
                sprite(if k == 0 { "bee" } else { "butterfly" }),
                Transform::default(),
            );
        }
        put(Role::Can, Shape::Quad, sprite("can"), Transform::default());
    });
}

fn view_of(pen: &crate::paint::Pen) -> View {
    pen.view.unwrap_or_else(View::ipad_landscape)
}

pub struct Spot {
    pub at: Vec3,
    pub scale: f32,
}

pub fn overview_beds(view: &View) -> Vec<(Vec3, f32)> {
    let cam = overview_cam(view);
    bed_tiles(view)
        .iter()
        .map(|t| {
            let c = ground_at(&cam, view, t.center());
            let l = ground_at(&cam, view, glam::vec2(t.x + t.w * 0.08, t.center().y));
            let r = ground_at(&cam, view, glam::vec2(t.right() - t.w * 0.08, t.center().y));
            (v3(c), (r.x - l.x).abs())
        })
        .collect()
}

pub fn overview_spot(beds: &[(Vec3, f32)], idx: usize) -> Option<Spot> {
    let (b, m) = (idx / (OVER_ROWS * OVER_COLS), idx % (OVER_ROWS * OVER_COLS));
    let (center, width) = *beds.get(b)?;
    let sp = width / (OVER_COLS as f32 + 0.4);
    let (i, j) = (m / OVER_COLS, m % OVER_COLS);
    let local = plot(OVER_ROWS, OVER_COLS, i, j);
    Some(Spot {
        at: center + Vec3::new(local.x * sp, 0.0, local.z * sp * 0.85),
        scale: sp,
    })
}

fn facing(cam: &kg_core::games::hotel::Cam) -> Quat {
    Transform::default()
        .looking_to(v3(cam.forward()), Vec3::Y)
        .rotation
}

type Posed<'a> = (
    &'a Role,
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

fn sync(
    frame: NonSend<crate::Frame>,
    pen: Res<crate::paint::Pen>,
    mut kit: Kit,
    mut eye: Lens,
    mut parts: Query<Posed, Without<Eye>>,
) {
    let Some(p) = playing::<GardenPlay>(&frame) else {
        return;
    };
    let g = &p.g;
    let view = view_of(&pen);
    let cam = g.view_cam(&view);
    let sh = g.shake.offset();
    let nudge = v3(cam.right()) * sh.x * 0.01 + v3(cam.up()) * -sh.y * 0.01;
    eye.aim(Rig {
        eye: v3(cam.eye) + nudge,
        target: v3(cam.target) + nudge,
        fov: cam.fov,
        near: 0.5,
        far: 300.0,
        sky: SKY,
        shadow_reach: 40.0,
    });
    let face = facing(&cam);
    let over = g.cam_mix > 0.5;
    let beds = if over { overview_beds(&view) } else { vec![] };
    let (rows, cols) = (g.q.rows(), g.q.cols());
    let live = rows * cols;
    let open = unlocked_level();
    let sway = |i: usize| Quat::from_rotation_z((g.t * 1.6 + i as f32 * 0.7).sin() * 0.035);
    for (role, mut t, mut v, mut part, mut mat) in parts.iter_mut() {
        match *role {
            Role::Ground => {}
            Role::Soil(b) => {
                if over {
                    let Some((c, w)) = beds.get(b).copied() else {
                        v.set_if_neq(shown(false));
                        continue;
                    };
                    v.set_if_neq(shown(true));
                    t.set_if_neq(Transform::from_xyz(c.x, LAYER, c.z).with_scale(Vec3::new(
                        w,
                        1.0,
                        w * 0.78,
                    )));
                    let alpha = if b <= open { 1.0 } else { 0.55 };
                    let paint = Paint {
                        skin: Some(Skin::Tile(SHEET, "soil", Vec2::splat(2.0))),
                        ..soil_paint(alpha)
                    };
                    kit.repaint(&mut part, &mut mat, paint);
                } else if b == 0 {
                    v.set_if_neq(shown(true));
                    let w = cols as f32 * SPACING + 0.5;
                    let d = rows as f32 * SPACING + 0.5;
                    t.set_if_neq(Transform::from_xyz(0.0, LAYER, 0.0).with_scale(Vec3::new(w, 1.0, d)));
                    let rep = Vec2::new(w / 2.5, d / 2.5);
                    kit.repaint(
                        &mut part,
                        &mut mat,
                        Paint {
                            skin: Some(Skin::Tile(SHEET, "soil", rep)),
                            ..soil_paint(1.0)
                        },
                    );
                } else {
                    v.set_if_neq(shown(false));
                }
            }
            Role::Shade(i) | Role::Flower(i) => {
                let is_flower = matches!(*role, Role::Flower(_));
                let (spot, grown, level) = if over {
                    let spot = overview_spot(&beds, i);
                    let b = i / (OVER_ROWS * OVER_COLS);
                    let grown = if bloomed(b) {
                        1.0
                    } else if b == open {
                        0.3
                    } else {
                        0.0
                    };
                    (spot, grown, b)
                } else if i < live {
                    let (r, c) = (i / cols, i % cols);
                    let w = plot(rows, cols, r, c);
                    (
                        Some(Spot {
                            at: v3(w),
                            scale: 1.0,
                        }),
                        g.bloom(i),
                        g.level,
                    )
                } else {
                    (None, 0.0, 0)
                };
                let Some(spot) = spot.filter(|_| grown > 0.01) else {
                    v.set_if_neq(shown(false));
                    continue;
                };
                v.set_if_neq(shown(true));
                if !is_flower {
                    t.set_if_neq(
                        Transform::from_xyz(spot.at.x, LAYER * 3.0, spot.at.z)
                            .with_scale(Vec3::new(0.7, 1.0, 0.5) * spot.scale * grown.min(1.0)),
                    );
                    continue;
                }
                let (r, c) = if over {
                    let m = i % (OVER_ROWS * OVER_COLS);
                    (m / OVER_COLS, m % OVER_COLS)
                } else {
                    (i / cols, i % cols)
                };
                let growth = if over { grown.min(1.0) } else { g.growth(i) };
                let name = if growth < SPROUT_AT {
                    "sprout"
                } else {
                    NAMES[flower_color(level, r, c)]
                };
                kit.repaint(&mut part, &mut mat, sprite(name));
                let size = crate::art::size(SHEET, name);
                let aspect = size.x / size.y.max(1.0);
                let h = FLOWER_H * spot.scale * grown.max(0.0) * if name == "sprout" { 0.7 } else { 1.0 };
                t.set_if_neq(Transform {
                    translation: Vec3::new(spot.at.x, h / 2.0, spot.at.z),
                    rotation: face * sway(i),
                    scale: Vec3::new(h * aspect, h, 1.0),
                });
            }
            Role::Critter(k) => {
                let want = if k == 0 {
                    beds_bloomed() >= 1
                } else {
                    k <= 1 + beds_bloomed() / 3
                };
                v.set_if_neq(shown(want && !over || want && k < 3));
                let reach = if over { 6.5 } else { cols.max(rows) as f32 * 0.5 + 1.0 };
                let a = g.t * (0.5 + k as f32 * 0.17) + k as f32 * 2.1;
                let at = Vec3::new(
                    a.cos() * reach,
                    1.3 + (a * 2.3).sin() * 0.4 + k as f32 * 0.2,
                    (a * 1.3).sin() * reach * 0.8,
                );
                let flap = 0.65 + 0.35 * (g.t * (if k == 0 { 40.0 } else { 14.0 }) + k as f32).sin().abs();
                let name = if k == 0 { "bee" } else { "butterfly" };
                let size = crate::art::size(SHEET, name);
                let hh = if k == 0 { 0.7 } else { 0.8 };
                let heading = if a.sin() * (a * 0.0 + 1.0) > 0.0 { 1.0 } else { -1.0 };
                t.set_if_neq(Transform {
                    translation: at,
                    rotation: face,
                    scale: Vec3::new(hh * size.x / size.y.max(1.0) * flap * heading, hh, 1.0),
                });
            }
            Role::Can => {
                let pour = g.can_pour();
                let on = !over && (g.solved.is_some() || pour > 0.0);
                v.set_if_neq(shown(on));
                let at = v3(g.can_pos());
                let size = crate::art::size(SHEET, "can");
                let hh = 1.4;
                t.set_if_neq(Transform {
                    translation: at,
                    rotation: face * Quat::from_rotation_z(pour * 0.55),
                    scale: Vec3::new(-hh * size.x / size.y.max(1.0), hh, 1.0),
                });
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

    fn fresh() -> GardenPlay {
        kg_core::save::reset_for_test();
        kg_core::profile::create("Mia", 0);
        GardenPlay::new()
    }

    fn open(play: GardenPlay, view: View) -> App {
        let mut app = testing::with_game(Box::new(play), view);
        testing::step(&mut app, 3);
        app
    }

    fn garden(app: &mut App) -> &mut GardenPlay {
        testing::game::<GardenPlay>(app)
    }

    fn visible_flowers(app: &mut App) -> usize {
        let w = app.world_mut();
        let all: Vec<Entity> = w
            .query::<(Entity, &Role)>()
            .iter(w)
            .filter(|(_, r)| matches!(r, Role::Flower(_)))
            .map(|(e, _)| e)
            .collect();
        all.into_iter().filter(|e| visible(w, *e)).count()
    }

    #[test]
    fn opening_and_closing_the_garden_leaves_the_menu_as_it_was() {
        let mut app = testing::app(crate::Scene::Menu(Box::default()), View::ipad_landscape());
        testing::step(&mut app, 3);
        let menu = testing::count(&mut app);
        for _ in 0..3 {
            if let crate::Scene::Menu(m) = &mut testing::frame(&mut app).scene {
                m.current = Some((0, Box::new(fresh())));
            }
            testing::step(&mut app, 3);
            let open = testing::count(&mut app);
            assert!(open > menu + 200, "{menu} -> {open}");
            assert_eq!(testing::open(&app), World3d::Garden);
            testing::close(&mut app);
            testing::step(&mut app, 3);
            assert_eq!(testing::count(&mut app), menu);
            assert_eq!(testing::open(&app), World3d::Flat);
        }
    }

    #[test]
    fn flowers_grow_to_match_the_array_for_the_answer() {
        for v in [View::ipad_landscape(), View::ipad_portrait()] {
            let mut app = open(fresh(), v);
            let g = &mut garden(&mut app).g;
            g.q = Question::new(Fact { table: 3, other: 4 }, 0, 0, false, false);
            g.shown = 1.0;
            g.grow_age = 5.0;
            testing::step(&mut app, 2);
            assert_eq!(visible_flowers(&mut app), 12);
            let g = &mut garden(&mut app).g;
            g.q = Question::new(Fact { table: 7, other: 8 }, 0, 0, false, false);
            g.grow_age = 9.0;
            testing::step(&mut app, 2);
            assert_eq!(visible_flowers(&mut app), 56);
        }
    }

    #[test]
    fn a_hidden_array_shows_no_flowers_until_a_wrong_pick() {
        let mut app = open(fresh(), View::ipad_landscape());
        let g = &mut garden(&mut app).g;
        g.stage = 2;
        g.shown = 0.0;
        g.ask_t = 10.0;
        testing::step(&mut app, 2);
        assert_eq!(visible_flowers(&mut app), 0);
    }

    #[test]
    fn the_bed_picker_shows_a_bed_of_flowers_for_every_bloomed_table() {
        kg_core::save::reset_for_test();
        kg_core::profile::create("Mia", 0);
        kg_core::profile::unlock(GAME, 3);
        for l in [0usize, 1, 2] {
            kg_core::profile::set(GAME, &format!("bed.{l}"), 1);
        }
        let mut app = open(GardenPlay::new(), View::ipad_portrait());
        let g = &mut garden(&mut app).g;
        g.mode = Mode::Beds;
        g.cam_mix = 1.0;
        testing::step(&mut app, 2);
        assert_eq!(visible_flowers(&mut app), 3 * OVER_ROWS * OVER_COLS + OVER_ROWS * OVER_COLS);
    }

    #[test]
    fn overview_beds_sit_under_their_tiles_and_do_not_overlap() {
        for v in [View::ipad_landscape(), View::ipad_portrait()] {
            let beds = overview_beds(&v);
            assert_eq!(beds.len(), LEVELS);
            for (i, a) in beds.iter().enumerate() {
                for b in &beds[i + 1..] {
                    let dx = (a.0.x - b.0.x).abs();
                    let dz = (a.0.z - b.0.z).abs();
                    assert!(dx > a.1 * 0.8 || dz > a.1 * 0.5, "{a:?} {b:?}");
                }
            }
        }
    }

    #[test]
    fn answering_runs_the_world_without_leaking_entities() {
        let v = View::ipad_landscape();
        let mut app = open(fresh(), v);
        let before = testing::count(&mut app);
        for _ in 0..8 {
            let slot = garden(&mut app).g.q.slot_of_answer();
            let p = garden(&mut app).g.pick(slot);
            garden(&mut app).g.react(&v, slot, p);
            for _ in 0..120 {
                garden(&mut app).update(&Input::default(), &v, 1.0 / 60.0);
                testing::step(&mut app, 1);
            }
        }
        assert_eq!(testing::count(&mut app), before);
    }
}
