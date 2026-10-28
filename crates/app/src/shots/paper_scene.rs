use super::{Stage, DT};
use crate::games::paper::PaperPlay;
use crate::games::Play;
use kg_core::engine::{Dir, Input, View};
use kg_core::games::paper::pilot::Pilot;
use kg_core::games::paper::world::Cause;
use kg_core::games::paper::{self, idx, State};
use kg_core::profile;

fn board(scores: &[(&str, usize, i64)]) {
    let me = profile::current_id();
    for (name, avatar, score) in scores {
        let p = profile::create(name, *avatar).unwrap();
        profile::select(&p.id);
        profile::submit(paper::GAME, *score, profile::Better::Higher);
    }
    if let Some(me) = me {
        profile::select(&me);
    }
}

fn idle(p: &mut PaperPlay, view: &View, frames: usize) {
    for _ in 0..frames {
        p.update(&Input::default(), view, DT);
    }
}

fn fly(p: &mut PaperPlay, view: &View, pilot: &mut Pilot, secs: f32) {
    for _ in 0..(secs / DT) as usize {
        if p.g.state != State::Playing {
            break;
        }
        let input = Input {
            key_dir: pilot.decide(&p.g),
            ..Default::default()
        };
        p.update(&input, view, DT);
    }
}

fn go(p: &mut PaperPlay, view: &View) {
    p.update(&Input::key(Dir::Right), view, DT);
}

fn grow_square(p: &mut PaperPlay, half: i32) {
    let at = p.g.me().cell;
    for y in at.y - half..=at.y + half {
        for x in at.x - half..=at.x + half {
            let c = glam::ivec2(x, y);
            if paper::in_bounds(c) && p.g.w.owner[idx(c)].is_none() && p.g.w.trail[idx(c)].is_none()
            {
                p.g.w.owner[idx(c)] = Some(0);
                p.g.w.counts[0] += 1;
            }
        }
    }
    p.g.zoom = p.g.target_zoom();
}

pub fn setup(n: u32, view: &View) -> Stage {
    kg_core::save::reset_for_test();
    kg_core::rng::srand(n as u64 * 7 + 1);
    let mia = profile::create("Mia", 2).unwrap();
    profile::select(&mia.id);
    let best = match n {
        150..=154 | 158 => 0,
        156 | 157 => 80,
        _ => 400,
    };
    if best > 0 {
        board(&[("Leo", 3, best), ("Ava", 5, best / 2)]);
        profile::select(&mia.id);
        kg_core::save::player_set(&format!("{}.best", paper::GAME), best);
    }
    let mut p = PaperPlay::new();
    match n {
        150 => idle(&mut p, view, 60),
        151 => {
            go(&mut p, view);
            fly(&mut p, view, &mut Pilot::new(5, false), 2.4);
        }
        152 => {
            go(&mut p, view);
            fly(&mut p, view, &mut Pilot::new(6, true), 14.0);
            let mut pilot = Pilot::new(6, true);
            let claims = p.g.popups.list.len();
            for _ in 0..600 {
                fly(&mut p, view, &mut pilot, DT);
                if p.g.popups.list.len() > claims
                    && p.g.popups.list.iter().any(|q| q.text.starts_with('+'))
                {
                    break;
                }
            }
            idle(&mut p, view, 8);
        }
        153 => {
            go(&mut p, view);
            fly(&mut p, view, &mut Pilot::new(6, true), 8.0);
            p.g.w.kill(3, Some(0), Cause::Trail);
            p.g.w.kill(5, Some(2), Cause::Trail);
            idle(&mut p, view, 4);
            p.g.w.kill(4, Some(0), Cause::Trail);
            idle(&mut p, view, 16);
        }
        154 => {
            go(&mut p, view);
            grow_square(&mut p, 14);
            fly(&mut p, view, &mut Pilot::new(9, true), 6.0);
        }
        155 => {
            go(&mut p, view);
            fly(&mut p, view, &mut Pilot::new(8, true), 20.0);
        }
        156 => {
            go(&mut p, view);
            fly(&mut p, view, &mut Pilot::new(7, true), 18.0);
            p.g.w.players[0].kills = 2;
            p.g.w.kill(0, Some(1), Cause::Trail);
            idle(&mut p, view, 80);
        }
        157 => {
            go(&mut p, view);
            grow_square(&mut p, 22);
            fly(&mut p, view, &mut Pilot::new(9, true), 3.0);
            p.g.w.players[0].kills = 5;
            p.g.w.kill(0, Some(2), Cause::HeadOn);
            idle(&mut p, view, 80);
        }
        158 => {
            go(&mut p, view);
            fly(&mut p, view, &mut Pilot::new(6, true), 4.0);
            let me = p.g.me().cell;
            for (k, off) in [(1usize, glam::ivec2(15, -4)), (2, glam::ivec2(-14, 9))] {
                let b = &mut p.g.w.players[k];
                b.cell =
                    (me + off).clamp(glam::ivec2(1, 1), glam::ivec2(paper::GW - 2, paper::GH - 2));
                b.progress = 0.0;
            }
            idle(&mut p, view, 2);
        }
        _ => {
            go(&mut p, view);
            fly(&mut p, view, &mut Pilot::new(6, true), 6.0);
            p.g.w.kill(0, Some(1), Cause::Trail);
            idle(&mut p, view, 70);
            p.update(
                &Input::press_at(paper::play_again_button(view).center()),
                view,
                DT,
            );
            idle(&mut p, view, 20);
        }
    }
    kg_core::sfx::take_cues();
    kg_core::feel::take();
    Stage::Game(Box::new(p))
}

#[cfg(test)]
pub fn check() {
    for v in [View::ipad_landscape(), View::ipad_portrait()] {
        let get = |n: u32| super::game::<PaperPlay>(setup(n, &v));
        assert_eq!(get(150).g.state, State::Ready);
        let first = get(151);
        assert!(first.g.state == State::Playing && !first.g.me().trail.is_empty());
        assert!(get(152)
            .g
            .popups
            .list
            .iter()
            .any(|q| q.text.starts_with('+')));
        let feed = get(153);
        assert!(feed
            .g
            .feed
            .iter()
            .any(|f| f.text.starts_with("You eliminated")));
        assert!(get(154).g.share(0) > 0.05);
        assert!(get(155).g.w.players.len() >= 11);
        let dead = get(156);
        assert!(dead.g.state == State::Dead && !dead.g.new_best);
        let best = get(157);
        assert!(best.g.state == State::Dead && best.g.new_best);
        assert!(!get(158).g.threats(&v).is_empty());
        let again = get(159);
        assert!(again.g.state == State::Playing && again.g.lives == 1);
    }
}
