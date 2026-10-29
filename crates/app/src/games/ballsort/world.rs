use super::{BallSortPlay, SHEET};
use crate::game3d::*;
use crate::game3d::{Lens, Rig};
use bevy::prelude::*;
use kg_core::engine::View;
use kg_core::games::ballsort::*;
use kg_core::geom::{Color as Col, WHITE};
use std::collections::HashMap;

const SKY: Col = Col::new(0.55, 0.85, 0.82, 1.0);
const BACK_DEPTH: f32 = 120.0;
const FADE_STEPS: f32 = 10.0;
const HIDDEN: Col = Col::new(0.42, 0.38, 0.52, 1.0);
const GLASS: Col = Col::new(0.82, 0.95, 1.0, 0.2);
const RIM: Col = Col::new(0.92, 0.97, 1.0, 1.0);
const FOOT: Col = Col::new(0.55, 0.45, 0.85, 1.0);
const SHADE: Col = Col::new(0.2, 0.08, 0.02, 0.28);
const WOOD: Col = Col::new(0.9, 0.74, 0.58, 1.0);
const WOOD_EDGE: Col = Col::new(0.62, 0.36, 0.18, 1.0);

pub struct BallSortWorld;

impl Plugin for BallSortWorld {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                spawn.run_if(opened(World3d::BallSort)),
                sync.run_if(is_open(World3d::BallSort)),
            )
                .chain()
                .in_set(Sync3d),
        );
    }
}

#[derive(Component, Default)]
pub struct Root {
    key: (u64, Vec<(i32, i32)>),
}

#[derive(Component)]
pub struct Board;

#[derive(Component, Clone, Copy, PartialEq, Eq, Debug)]
pub enum Role {
    Sky,
    Ball(u8),
    Glow(usize),
    Glass(usize),
    Static,
}

fn spawn(mut commands: Commands, mut kit: Kit) {
    let world = spawn_world(
        &mut commands,
        Root::default(),
        World3d::BallSort,
        Lighting {
            ambient: 650.0,
            illuminance: 9500.0,
            color: Col::new(1.0, 0.97, 0.92, 1.0),
            toward: Vec3::new(-0.35, -1.0, -0.45),
        },
    );
    commands.entity(world.eye).with_children(|c| {
        c.spawn((
            kit.part(
                Shape::Quad,
                Paint::skinned(
                    Finish::Backdrop,
                    WHITE,
                    Skin::Sprite(SHEET, "bg", [0.0, 0.0, 1.0, 1.0]),
                ),
                Transform::default(),
            ),
            Role::Sky,
        ));
    });
    commands.entity(world.root).with_children(|w| {
        w.spawn((Board, Transform::default(), Visibility::default()));
    });
}

fn view_of(pen: &crate::paint::Pen) -> View {
    pen.view.unwrap_or_else(View::ipad_landscape)
}

fn fade(a: f32) -> f32 {
    (a * FADE_STEPS).round() / FADE_STEPS
}

fn ball_paint(color: u8, revealed: bool) -> Paint {
    let c = if revealed {
        kg_core::games::ballsort::ball_color(color)
    } else {
        HIDDEN
    };
    Paint::new(Finish::Gloss, Col::new(c.r, c.g, c.b, 1.0))
}

fn build_board(commands: &mut Commands, kit: &mut Kit, board: Entity, g: &BallSort) {
    let (mut lo, mut hi) = (glam::Vec2::splat(f32::MAX), glam::Vec2::splat(f32::MIN));
    for s in &g.spots {
        lo = lo.min(*s);
        hi = hi.max(*s);
    }
    let mid = (lo + hi) / 2.0;
    let size = hi - lo + glam::vec2(5.0, 5.0);
    let wood = Paint::skinned(
        Finish::Satin,
        WOOD,
        Skin::Tile(SHEET, "table", Vec2::new(size.x / 6.0, size.y / 6.0)),
    );
    commands.entity(board).with_children(|b| {
        b.spawn((
            kit.part(
                Shape::Cube,
                wood,
                Transform::from_xyz(mid.x, -0.3, mid.y).with_scale(Vec3::new(size.x, 0.6, size.y)),
            ),
            Role::Static,
        ));
        b.spawn((
            kit.part(
                Shape::Cube,
                Paint::new(Finish::Satin, WOOD_EDGE),
                Transform::from_xyz(mid.x, -0.75, mid.y).with_scale(Vec3::new(
                    size.x + 0.4,
                    0.3,
                    size.y + 0.4,
                )),
            ),
            Role::Static,
        ));
        for (i, s) in g.spots.iter().enumerate() {
            let x = s.x;
            let z = s.y;
            b.spawn((
                kit.part(
                    Shape::Disc,
                    Paint::new(Finish::Sticker, SHADE),
                    Transform::from_xyz(x + 0.1, 0.012, z + 0.12)
                        .with_scale(Vec3::new(1.45, 1.0, 1.35)),
                ),
                Role::Static,
            ));
            b.spawn((
                kit.part(
                    Shape::Cylinder(28),
                    Paint::new(Finish::Gloss, FOOT),
                    Transform::from_xyz(x, BASE / 2.0, z).with_scale(Vec3::new(1.2, BASE, 1.2)),
                ),
                Role::Static,
            ));
            b.spawn((
                kit.part(
                    Shape::Tube(28),
                    Paint::new(Finish::Glass, GLASS),
                    Transform::from_xyz(x, TUBE_H / 2.0, z).with_scale(Vec3::new(
                        TUBE_R * 2.06,
                        TUBE_H,
                        TUBE_R * 2.06,
                    )),
                ),
                Role::Glass(i),
            ));
            b.spawn((
                kit.part(
                    Shape::Torus,
                    Paint::new(Finish::Gloss, RIM),
                    Transform::from_xyz(x, TUBE_H, z).with_scale(Vec3::new(
                        TUBE_R * 2.16,
                        TUBE_R * 2.4,
                        TUBE_R * 2.16,
                    )),
                ),
                Role::Static,
            ));
            b.spawn((
                kit.part(
                    Shape::Cube,
                    Paint::new(Finish::Glow, WHITE.with_alpha(0.4)),
                    Transform::from_xyz(x - 0.3, TUBE_H * 0.5, z + 0.43).with_scale(Vec3::new(
                        0.07,
                        TUBE_H * 0.78,
                        0.02,
                    )),
                ),
                Role::Static,
            ));
            b.spawn((
                kit.part(
                    Shape::Torus,
                    Paint::new(Finish::Glow, WHITE.with_alpha(0.0)),
                    Transform::from_xyz(x, 0.04, z).with_scale(Vec3::splat(1.5)),
                ),
                Role::Glow(i),
            ));
        }
        for tube in &g.tubes {
            for ball in tube {
                b.spawn((
                    kit.part(
                        Shape::Ball,
                        ball_paint(ball.color, ball.revealed),
                        Transform::default().with_scale(Vec3::splat(BALL_D)),
                    ),
                    Role::Ball(ball.id),
                ));
            }
        }
    });
}

type Posed<'a> = (
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
    mut root: Query<&mut Root>,
    board: Query<Entity, With<Board>>,
    mut eye: Lens,
    mut parts: Query<Posed, Without<Eye>>,
) {
    let Some(p) = playing::<BallSortPlay>(&frame) else {
        return;
    };
    let g = &p.g;
    let view = view_of(&pen);
    let (Ok(mut root), Ok(board)) = (root.single_mut(), board.single()) else {
        return;
    };
    let sh = g.shake.offset();
    let fwd = (g.cam.target - g.cam.eye).normalize();
    let right = fwd.cross(glam::Vec3::Y).normalize();
    let up = right.cross(fwd);
    let eye_pos = g.cam.eye + (right * -sh.x + up * -sh.y) * 0.004 * g.cam.distance();
    let dist = g.cam.distance();
    eye.aim(Rig {
        eye: v3(eye_pos),
        target: v3(g.cam.target),
        fov: FOV,
        near: 0.5,
        far: BACK_DEPTH * 2.0,
        sky: SKY,
        shadow_reach: dist * 2.0,
    });
    let key = (
        g.serial,
        g.spots
            .iter()
            .map(|s| ((s.x * 100.0).round() as i32, (s.y * 100.0).round() as i32))
            .collect::<Vec<_>>(),
    );
    if root.key != key && !g.spots.is_empty() {
        root.key = key;
        let mut b = commands.entity(board);
        b.despawn_children();
        build_board(&mut commands, &mut kit, board, g);
        return;
    }
    let poses: HashMap<u8, kg_core::games::ballsort::BallPose> =
        g.balls().into_iter().map(|b| (b.ball.id, b)).collect();
    let f = view.full();
    let k = 2.0 * (FOV / 2.0).tan() / f.h;
    for (role, mut t, mut v, mut part, mut mat) in parts.iter_mut() {
        match *role {
            Role::Sky => {
                let s = crate::art::size(SHEET, "bg");
                let cover = (f.w / s.x).max(f.h / s.y) * 1.04;
                let (w, h) = (s.x * cover, s.y * cover);
                t.set_if_neq(Transform {
                    translation: Vec3::new(0.0, 0.0, -BACK_DEPTH),
                    rotation: Quat::IDENTITY,
                    scale: Vec3::new(w * k * BACK_DEPTH, h * k * BACK_DEPTH, 1.0),
                });
            }
            Role::Ball(id) => match poses.get(&id) {
                Some(b) => {
                    v.set_if_neq(Visibility::Inherited);
                    let sq = b.squash;
                    let (sx, sy) = (1.0 + sq * 0.5, 1.0 - sq);
                    let at =
                        Transform::from_xyz(b.pos.x, b.pos.y - BALL_D * (1.0 - sy) * 0.5, b.pos.z)
                            .with_scale(Vec3::new(sx, sy, sx) * BALL_D);
                    t.set_if_neq(at);
                    kit.repaint(
                        &mut part,
                        &mut mat,
                        ball_paint(b.ball.color, b.ball.revealed),
                    );
                }
                None => {
                    v.set_if_neq(Visibility::Hidden);
                }
            },
            Role::Glow(i) => {
                let (c, a) = g.glow_of(i);
                let a = fade(a * 0.9);
                let pulse = 1.0 + g.glow.get(i).copied().unwrap_or(0.0) * 0.25;
                t.set_if_neq(
                    Transform::from_xyz(g.spot(i).x, 0.04, g.spot(i).y)
                        .with_scale(Vec3::splat(1.5 * pulse)),
                );
                v.set_if_neq(if a > 0.0 {
                    Visibility::Inherited
                } else {
                    Visibility::Hidden
                });
                kit.repaint(
                    &mut part,
                    &mut mat,
                    Paint::new(Finish::Glow, c.with_alpha(a)),
                );
            }
            Role::Glass(_) | Role::Static => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game3d::testing;
    use crate::games::Play;
    use kg_core::engine::Input;

    fn fresh(level: u32) -> BallSortPlay {
        kg_core::save::reset_for_test();
        let mut p = BallSortPlay::new();
        p.g = BallSort::with_level(level);
        p
    }

    fn open(play: BallSortPlay, view: View) -> App {
        let mut p = play;
        p.update(&Input::default(), &view, 1.0 / 60.0);
        let mut app = testing::with_game(Box::new(p), view);
        testing::step(&mut app, 3);
        app
    }

    fn count<T: Component>(app: &mut App, f: impl Fn(&T) -> bool) -> usize {
        let w = app.world_mut();
        w.query::<&T>().iter(w).filter(|r| f(r)).count()
    }

    #[test]
    fn opening_and_closing_ball_sort_leaves_the_menu_as_it_was() {
        let mut app = testing::app(crate::Scene::Menu(Box::default()), View::ipad_landscape());
        testing::step(&mut app, 3);
        let menu = testing::count(&mut app);
        for _ in 0..3 {
            let mut p = fresh(3);
            p.update(&Input::default(), &View::ipad_landscape(), 1.0 / 60.0);
            if let crate::Scene::Menu(m) = &mut testing::frame(&mut app).scene {
                m.current = Some((0, Box::new(p)));
            }
            testing::step(&mut app, 3);
            assert!(testing::count(&mut app) > menu + 40);
            assert_eq!(testing::open(&app), World3d::BallSort);
            testing::close(&mut app);
            testing::step(&mut app, 3);
            assert_eq!(testing::count(&mut app), menu);
            assert_eq!(testing::open(&app), World3d::Flat);
        }
    }

    #[test]
    fn every_ball_and_tube_is_spawned_once() {
        for v in [View::ipad_landscape(), View::ipad_portrait()] {
            let mut app = open(fresh(5), v);
            let (tubes, balls) = {
                let g = &testing::game::<BallSortPlay>(&mut app).g;
                (g.tubes.len(), g.tubes.iter().map(Vec::len).sum::<usize>())
            };
            assert_eq!(
                count::<Role>(&mut app, |r| matches!(r, Role::Glass(_))),
                tubes
            );
            assert_eq!(
                count::<Role>(&mut app, |r| matches!(r, Role::Ball(_))),
                balls
            );
        }
    }

    #[test]
    fn a_ball_in_flight_moves_its_entity_and_lands_in_its_slot() {
        let v = View::ipad_landscape();
        let mut app = open(fresh(1), v);
        let (a, b, id) = {
            let g = &mut testing::game::<BallSortPlay>(&mut app).g;
            let path = kg_core::games::ballsort::solve::solve(&g.packed(), 100_000).unwrap();
            let (a, b) = (path[0].0 as usize, path[0].1 as usize);
            let id = g.tubes[a].last().unwrap().id;
            assert!(g.pour(a, b));
            (a, b, id)
        };
        testing::step(&mut app, 1);
        let at = |app: &mut App| {
            let w = app.world_mut();
            w.query::<(&Role, &Transform)>()
                .iter(w)
                .find(|(r, _)| **r == Role::Ball(id))
                .map(|(_, t)| t.translation)
                .unwrap()
        };
        let mid = at(&mut app);
        for _ in 0..120 {
            testing::game::<BallSortPlay>(&mut app).update(&Input::default(), &v, 1.0 / 60.0);
            testing::step(&mut app, 1);
        }
        let end = at(&mut app);
        let g = &testing::game::<BallSortPlay>(&mut app).g;
        let slot = g.slot_pos(b, g.tubes[b].len() - 1);
        assert!(
            (end - Vec3::new(slot.x, slot.y, slot.z)).length() < 0.05,
            "{end:?} {slot:?}"
        );
        assert!(mid.y > end.y - 0.01);
        let _ = a;
    }

    #[test]
    fn turning_the_screen_moves_the_tubes_with_the_balls() {
        let land = View::ipad_landscape();
        let tall = View::ipad_portrait();
        let mut app = open(fresh(9), land);
        for _ in 0..2 {
            testing::game::<BallSortPlay>(&mut app).update(&Input::default(), &tall, 1.0 / 60.0);
            app.insert_resource(testing::pen(tall));
            testing::step(&mut app, 2);
        }
        let g = &testing::game::<BallSortPlay>(&mut app).g;
        let spots: Vec<_> = (0..g.tubes.len()).map(|i| g.spot(i)).collect();
        let w = app.world_mut();
        let glass: Vec<(usize, Vec3)> = w
            .query::<(&Role, &GlobalTransform)>()
            .iter(w)
            .filter_map(|(r, t)| match r {
                Role::Glass(i) => Some((*i, t.translation())),
                _ => None,
            })
            .collect();
        assert_eq!(glass.len(), spots.len());
        for (i, at) in glass {
            let s = spots[i];
            assert!(
                (at.x - s.x).abs() < 0.01 && (at.z - s.y).abs() < 0.01,
                "tube {i} glass at {at:?}, its balls at {s:?}"
            );
        }
    }

    #[test]
    fn opening_from_the_menu_in_portrait_lines_tubes_up_with_balls() {
        let tall = View::ipad_portrait();
        let mut app = testing::app(crate::Scene::Menu(Box::default()), tall);
        testing::step(&mut app, 3);
        kg_core::save::reset_for_test();
        kg_core::profile::set_level(kg_core::games::ballsort::GAME, 3);
        if let crate::Scene::Menu(m) = &mut testing::frame(&mut app).scene {
            m.current = Some((0, Box::new(BallSortPlay::new())));
        }
        for _ in 0..6 {
            testing::game::<BallSortPlay>(&mut app).update(&Input::default(), &tall, 1.0 / 60.0);
            testing::step(&mut app, 1);
        }
        let g = &testing::game::<BallSortPlay>(&mut app).g;
        let spots: Vec<_> = (0..g.tubes.len()).map(|i| g.spot(i)).collect();
        let w = app.world_mut();
        let glass: Vec<(usize, Vec3)> = w
            .query::<(&Role, &GlobalTransform)>()
            .iter(w)
            .filter_map(|(r, t)| match r {
                Role::Glass(i) => Some((*i, t.translation())),
                _ => None,
            })
            .collect();
        assert_eq!(glass.len(), spots.len());
        for (i, at) in glass {
            let s = spots[i];
            assert!(
                (at.x - s.x).abs() < 0.01 && (at.z - s.y).abs() < 0.01,
                "tube {i} glass at {at:?}, its balls at {s:?}"
            );
        }
    }

    #[test]
    fn restarting_until_the_helper_tube_appears_keeps_tubes_under_balls() {
        let tall = View::ipad_portrait();
        let mut app = open(fresh(3), tall);
        for _ in 0..4 {
            testing::game::<BallSortPlay>(&mut app).g.restart();
            for _ in 0..3 {
                testing::game::<BallSortPlay>(&mut app).update(
                    &Input::default(),
                    &tall,
                    1.0 / 60.0,
                );
                testing::step(&mut app, 1);
            }
        }
        let g = &testing::game::<BallSortPlay>(&mut app).g;
        let spots: Vec<_> = (0..g.tubes.len()).map(|i| g.spot(i)).collect();
        let w = app.world_mut();
        let glass: Vec<(usize, Vec3)> = w
            .query::<(&Role, &GlobalTransform)>()
            .iter(w)
            .filter_map(|(r, t)| match r {
                Role::Glass(i) => Some((*i, t.translation())),
                _ => None,
            })
            .collect();
        assert_eq!(glass.len(), spots.len());
        for (i, at) in glass {
            let s = spots[i];
            assert!(
                (at.x - s.x).abs() < 0.01 && (at.z - s.y).abs() < 0.01,
                "tube {i} glass at {at:?}, its balls at {s:?}"
            );
        }
    }

    #[test]
    fn a_new_level_rebuilds_the_board_without_leaking_entities() {
        let mut app = open(fresh(3), View::ipad_portrait());
        let before = testing::count(&mut app);
        for _ in 0..3 {
            testing::game::<BallSortPlay>(&mut app).g.restart();
            testing::step(&mut app, 2);
        }
        assert_eq!(testing::count(&mut app), before);
        testing::game::<BallSortPlay>(&mut app).g.enter(3);
        testing::step(&mut app, 2);
        assert_eq!(testing::count(&mut app), before);
    }

    #[test]
    fn rotating_the_screen_rebuilds_the_layout_and_keeps_every_ball() {
        let mut app = open(fresh(8), View::ipad_landscape());
        let balls = count::<Role>(&mut app, |r| matches!(r, Role::Ball(_)));
        testing::game::<BallSortPlay>(&mut app).update(
            &Input::default(),
            &View::ipad_portrait(),
            1.0 / 60.0,
        );
        testing::step(&mut app, 2);
        assert_eq!(
            count::<Role>(&mut app, |r| matches!(r, Role::Ball(_))),
            balls
        );
    }

    #[test]
    fn hidden_balls_are_painted_neutral() {
        let a = ball_paint(2, false);
        let b = ball_paint(5, false);
        assert_eq!(a, b);
        assert_ne!(ball_paint(2, true), a);
    }
}
