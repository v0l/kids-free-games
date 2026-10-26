use crate::games::crossy::CrossyPlay;
use crate::games::flappy::FlappyPlay;
use crate::games::Play;
use crate::link::{Link, Tab};
use crate::menu::App;
use crate::paint::Pen;
use crate::profiles::{Mode, Profiles};
use glam::vec2;
use kg_core::engine::{Input, View};
use kg_core::games::crossy;
use kg_core::games::flappy::{self, Pipe, State, PIPE_W};
use kg_core::profile;

pub const IDLE: u32 = 1000;

mod snake_scene;
mod spell_scene;

pub const SCENES: &[(u32, &str)] = &[
    (0, "menu"),
    (1, "menu scrolled to Learn"),
    (2, "menu with the second tile focused"),
    (3, "who's playing"),
    (4, "new player"),
    (5, "link: show code"),
    (6, "link: type code"),
    (10, "flappy ready"),
    (11, "flappy first flight with the hint"),
    (12, "flappy mid run"),
    (13, "flappy crossing ten"),
    (14, "flappy moving pipes at night"),
    (15, "flappy game over with the board"),
    (16, "flappy new best"),
    (20, "road hopper ready"),
    (21, "road hopper ready with a checkpoint to start from"),
    (22, "road hopper first road with the coach"),
    (23, "road hopper busy roads"),
    (24, "road hopper river"),
    (25, "road hopper train tracks"),
    (26, "road hopper checkpoint banner"),
    (27, "road hopper hurry up"),
    (28, "road hopper game over with the board"),
    (29, "road hopper oops, hopped back"),
    (30, "snake ready"),
    (31, "snake first moves with the hint"),
    (32, "snake mid run"),
    (33, "snake stage up"),
    (34, "snake long and fast"),
    (35, "snake bonk"),
    (36, "snake game over with the board"),
    (37, "snake new best"),
    (160, "spell first word with the hint line"),
    (161, "spell a wrong tap"),
    (162, "spell stuck, next letter glowing"),
    (163, "spell word won"),
    (164, "spell level picker"),
    (165, "spell level done"),
    (166, "spell all levels done"),
    (167, "spell long word on level nine"),
];

pub fn requested() -> Option<u32> {
    crate::web::query("shot")
        .or_else(crate::web::hash)
        .and_then(|s| s.parse().ok())
}

pub enum Stage {
    Menu(Box<App>),
    Profiles(Box<Profiles>),
    Flappy(Box<FlappyPlay>),
    Game(Box<dyn Play>),
}

#[cfg(test)]
pub fn game<T: Play + 'static>(s: Stage) -> Box<T> {
    match s {
        Stage::Game(p) => match p.into_any().downcast::<T>() {
            Ok(t) => t,
            Err(_) => panic!("scene is a different game"),
        },
        _ => panic!("scene is not a game"),
    }
}

pub struct Shot {
    pub n: u32,
    stage: Option<Stage>,
    size: (f32, f32),
}

const DT: f32 = 1.0 / 60.0;

fn players() {
    kg_core::save::reset_for_test();
    let mia = profile::create("Mia", 2).unwrap();
    profile::create("Leo", 5);
    profile::create("Ava", 9);
    profile::select(&mia.id);
}

fn flap() -> Input {
    Input::press_at(vec2(500.0, 500.0))
}

fn fly(p: &mut FlappyPlay, view: &View, frames: usize) {
    for _ in 0..frames {
        let g = &p.g;
        let bx = flappy::Flappy::bird_x(view);
        let target = g
            .pipes
            .iter()
            .find(|q| q.x + PIPE_W > bx - flappy::BIRD_R)
            .map(|q| q.gap_y + q.gap * 0.15)
            .unwrap_or(view.h * 0.5);
        let input = if g.y > target && g.vy > 0.0 {
            flap()
        } else {
            Input::default()
        };
        p.update(&input, view, DT);
        if p.g.state == State::Dead {
            break;
        }
    }
}

fn crash(p: &mut FlappyPlay, view: &View) {
    p.g.y = 100.0;
    p.g.pipes
        .push(Pipe::new(flappy::Flappy::bird_x(view) - 10.0, 450.0));
    p.update(&Input::default(), view, DT);
    for _ in 0..90 {
        p.update(&Input::default(), view, DT);
    }
}

fn board(scores: &[(&str, usize, i64)]) {
    let me = profile::current_id();
    for (name, avatar, score) in scores {
        let p = profile::create(name, *avatar).unwrap();
        profile::select(&p.id);
        profile::submit(flappy::GAME, *score, profile::Better::Higher);
    }
    if let Some(me) = me {
        profile::select(&me);
    }
}

fn crossy_at(stage: i32, row_in_stage: i32, view: &View) -> CrossyPlay {
    kg_core::profile::set(crossy::GAME, "stage", stage as i64);
    let mut p = CrossyPlay::new();
    p.update(&Input::default(), view, DT);
    p.g = crossy::Crossy::starting_at(stage);
    p.g.state = crossy::State::Playing;
    p.g.row = stage * crossy::STAGE_ROWS + row_in_stage;
    p.g.top = p.g.row;
    p.g.ensure_rows();
    p.g.score = row_in_stage as u32;
    p.g.from = vec2(p.g.x, p.g.row as f32);
    p.g.cam = (p.g.row as f32 - 2.0).max(0.0);
    for _ in 0..30 {
        p.update(&Input::default(), view, DT);
    }
    p
}

fn crossy_scene(n: u32, view: &View) -> Stage {
    kg_core::save::reset_for_test();
    let mia = profile::create("Mia", 2).unwrap();
    profile::select(&mia.id);
    let mut p = match n {
        20 => CrossyPlay::new(),
        21 => {
            profile::set(crossy::GAME, "stage", 2);
            CrossyPlay::new()
        }
        22 => crossy_at(0, 2, view),
        23 => {
            profile::set(crossy::GAME, "stage", 1);
            let mut p = crossy_at(1, 3, view);
            p.g.idle = 0.0;
            p
        }
        24 => crossy_at(2, 1, view),
        25 => {
            profile::set(crossy::GAME, "stage", 3);
            let mut p = crossy_at(3, 1, view);
            let rail = (p.g.row as usize..p.g.rows.len())
                .find(|i| matches!(p.g.rows[*i].lane, crossy::Lane::Rail { .. }))
                .unwrap();
            p.g.rows[rail].things = vec![-1.0];
            p
        }
        26 => {
            let mut p = crossy_at(0, 8, view);
            for _ in 0..2 {
                p.g.row += 1;
                p.g.top = p.g.row;
                p.g.idle = 0.0;
                p.update(&Input::key(kg_core::engine::Dir::Up), view, DT);
                for _ in 0..12 {
                    p.update(&Input::default(), view, DT);
                }
            }
            p.g.banner = Some((1, 0.6));
            p
        }
        27 => {
            profile::set(crossy::GAME, "stage", 1);
            let mut p = crossy_at(1, 0, view);
            p.g.idle = 5.6;
            p
        }
        28 => {
            board(&[("Leo", 5, 23), ("Ava", 9, 14), ("Sam", 3, 6)]);
            let mut p = crossy_at(1, 8, view);
            p.g.gentle = false;
            p.g.die(crossy::Death::Car);
            for _ in 0..90 {
                p.update(&Input::default(), view, DT);
            }
            p
        }
        _ => {
            let mut p = crossy_at(0, 4, view);
            p.g.rows[5].lane = crossy::Lane::Road {
                speed: 0.0,
                gap: 5.0,
            };
            p.g.rows[5].things = vec![p.g.x - 0.5];
            p.g.row = 5;
            p.g.from = vec2(p.g.x, 5.0);
            p.g.hop = 1.0;
            p.update(&Input::default(), view, DT);
            p
        }
    };
    p.update(&Input::default(), view, DT);
    kg_core::sfx::take_cues();
    Stage::Game(Box::new(p))
}

fn setup(n: u32, view: &View) -> Stage {
    quad_rand::srand(7);
    match n {
        0..=2 => {
            players();
            let mut app = App::new();
            match n {
                1 => app.scroll_to_end(view),
                2 => {
                    app.update(&Input::key(kg_core::engine::Dir::Right), view, DT);
                    app.update(&Input::key(kg_core::engine::Dir::Right), view, DT);
                }
                _ => {}
            }
            for _ in 0..90 {
                app.update(&Input::default(), view, DT);
            }
            Stage::Menu(Box::new(app))
        }
        3..=6 => {
            players();
            let mut p = Profiles::new();
            match n {
                4 => {
                    p.mode = Mode::Edit {
                        id: None,
                        name: "Zoe".into(),
                        avatar: 7,
                        confirm_delete: false,
                    }
                }
                5 => p.link = Some(Link::new()),
                6 => {
                    let mut l = Link::new();
                    l.tab = Tab::Join;
                    l.typed = "K7QM2X".into();
                    p.link = Some(l);
                }
                _ => {}
            }
            for _ in 0..60 {
                p.update(&Input::default(), view, DT);
            }
            Stage::Profiles(Box::new(p))
        }
        20..=29 => crossy_scene(n, view),
        30..=39 => snake_scene::setup(n, view),
        160..=169 => spell_scene::setup(n, view),
        _ => {
            kg_core::save::reset_for_test();
            let mia = profile::create("Mia", 2).unwrap();
            if n != 11 {
                profile::set(flappy::GAME, "passed", 1);
            }
            profile::select(&mia.id);
            let mut p = FlappyPlay::new();
            match n {
                10 => {
                    for _ in 0..40 {
                        p.update(&Input::default(), view, DT);
                    }
                }
                11 => {
                    p.update(&flap(), view, DT);
                    fly(&mut p, view, 200);
                }
                12 => {
                    p.update(&flap(), view, DT);
                    fly(&mut p, view, 60 * 14);
                }
                13 => {
                    p.update(&flap(), view, DT);
                    for _ in 0..60 * 60 {
                        fly(&mut p, view, 1);
                        if p.g.score >= flappy::MILESTONE {
                            break;
                        }
                    }
                    fly(&mut p, view, 25);
                }
                14 => {
                    p.update(&flap(), view, DT);
                    p.g.score = 31;
                    fly(&mut p, view, 60 * 9);
                }
                15 => {
                    board(&[("Leo", 5, 23), ("Ava", 9, 14), ("Sam", 3, 6)]);
                    p = FlappyPlay::new();
                    p.update(&flap(), view, DT);
                    fly(&mut p, view, 60 * 6);
                    crash(&mut p, view);
                }
                _ => {
                    board(&[("Leo", 5, 2), ("Ava", 9, 1)]);
                    p = FlappyPlay::new();
                    p.update(&flap(), view, DT);
                    fly(&mut p, view, 60 * 12);
                    crash(&mut p, view);
                }
            }
            kg_core::sfx::take_cues();
            Stage::Flappy(Box::new(p))
        }
    }
}

impl Shot {
    pub fn new(n: u32) -> Self {
        kg_core::save::use_memory_only();
        Self {
            n,
            stage: None,
            size: (0.0, 0.0),
        }
    }

    pub fn frame(&mut self, pen: &mut Pen, view: &View, _dt: f32) {
        if self.size != (view.w, view.h) || self.stage.is_none() {
            self.size = (view.w, view.h);
            self.stage = Some(setup(self.n % IDLE, view));
        }
        if self.n >= IDLE {
            match &mut self.stage {
                Some(Stage::Flappy(p)) if p.g.state != State::Dead => {
                    p.update(&Input::default(), view, DT);
                }
                Some(Stage::Game(p)) if !p.over() => p.update(&Input::default(), view, DT),
                _ => {}
            }
        }
        match self.stage.as_ref().unwrap() {
            Stage::Menu(a) => a.draw(pen, view),
            Stage::Profiles(p) => {
                draw_backdrop(pen, view);
                pen.fill_screen(view, kg_core::geom::Color::new(0.1, 0.04, 0.25, 0.45));
                p.draw(pen, view);
            }
            Stage::Game(p) => {
                p.draw(pen, view);
                home_button(pen);
            }
            Stage::Flappy(p) => {
                p.draw(pen, view);
                pen.offset = glam::Vec2::ZERO;
                let r = kg_core::geom::Rect::new(
                    crate::menu::HOME.x,
                    crate::menu::HOME.y - 4.0,
                    crate::menu::HOME.h + 8.0,
                    crate::menu::HOME.h + 8.0,
                );
                pen.painted_round(r, "icon_home", 0.0);
            }
        }
    }
}

fn home_button(pen: &mut Pen) {
    pen.offset = glam::Vec2::ZERO;
    let r = kg_core::geom::Rect::new(
        crate::menu::HOME.x,
        crate::menu::HOME.y - 4.0,
        crate::menu::HOME.h + 8.0,
        crate::menu::HOME.h + 8.0,
    );
    pen.painted_round(r, "icon_home", 0.0);
}

fn draw_backdrop(pen: &mut Pen, view: &View) {
    let f = view.full();
    let name = if f.h > f.w * 1.05 {
        "bg_tall"
    } else {
        "bg_wide"
    };
    let sz = crate::art::size("menu", name);
    let k = (f.w / sz.x).max(f.h / sz.y) * 1.04;
    let (w, h) = (sz.x * k, sz.y * k);
    pen.sprite(
        "menu",
        name,
        kg_core::geom::Rect::new(f.x + (f.w - w) / 2.0, f.y + (f.h - h) / 2.0, w, h),
        crate::draw::Draw::default(),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_scene_builds_and_draws_in_both_orientations() {
        for v in [View::ipad_landscape(), View::ipad_portrait()] {
            for &(n, name) in SCENES {
                let mut s = Shot::new(n);
                let mut pen = Pen::default();
                pen.begin(v, 1.0);
                s.frame(&mut pen, &v, DT);
                assert!(pen.vertex_count() > 100, "scene {n} ({name}) drew nothing");
            }
        }
    }

    #[test]
    fn snake_scenes_land_in_the_state_they_are_named_for() {
        snake_scene::check();
    }

    #[test]
    fn spell_scenes_land_in_the_state_they_are_named_for() {
        spell_scene::check();
    }

    #[test]
    fn flappy_scenes_land_in_the_state_they_are_named_for() {
        let v = View::ipad_landscape();
        let get = |n: u32| match setup(n, &v) {
            Stage::Flappy(p) => p,
            _ => panic!(),
        };
        assert_eq!(get(10).g.state, State::Ready);
        let t = get(11);
        assert!(t.g.training && t.g.state == State::Playing);
        assert!(get(12).g.score >= 3);
        assert!(get(13).g.score >= flappy::MILESTONE);
        assert!(get(14).g.pipes.iter().any(|p| p.amp > 0.0));
        let over = get(15);
        assert!(over.g.shows_results() && over.g.rank.is_some_and(|r| r > 0));
        assert_eq!(get(16).g.rank, Some(0));
    }

    #[test]
    fn road_hopper_scenes_land_in_the_state_they_are_named_for() {
        let v = View::ipad_landscape();
        let get = |n: u32| game::<CrossyPlay>(setup(n, &v));
        assert_eq!(get(20).g.state, crossy::State::Ready);
        assert_eq!(get(21).g.checkpoint_offer(), Some(2));
        assert!(get(22).g.coach().is_some());
        assert!(get(24).g.rows[get(24).g.row as usize + 1].lane != crossy::Lane::Grass);
        assert!(get(26).g.banner.is_some());
        assert!(get(27).g.idle > 5.0);
        let over = get(28);
        assert!(over.g.shows_results() && over.g.rank.is_some());
        assert!(get(29).g.oops > 0.0);
    }
}
