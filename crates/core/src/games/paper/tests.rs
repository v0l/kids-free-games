use super::pilot::Pilot;
use super::world::*;
use super::*;
use crate::rng as rand;
use std::f32::consts::{FRAC_PI_2, PI};

const RIGHT: f32 = 0.0;
const DOWN: f32 = FRAC_PI_2;
const LEFT: f32 = PI;
const UP: f32 = -FRAC_PI_2;

fn solo() -> World {
    let mut w = World::new(0.5);
    w.add_player(vec2(20.0, 20.0), RIGHT, false);
    w
}

fn go(w: &mut World, id: usize, angle: f32, dist: f32) {
    let speeds: Vec<f32> = w.players.iter().map(|p| p.speed).collect();
    for (o, p) in w.players.iter_mut().enumerate() {
        if o != id {
            p.speed = 0.0;
        }
    }
    walk(w, id, angle, dist);
    for (p, s) in w.players.iter_mut().zip(speeds) {
        p.speed = s;
    }
}

fn walk(w: &mut World, id: usize, angle: f32, dist: f32) {
    w.players[id].want = angle;
    let mut moved = 0.0;
    let mut guard = 0;
    while w.players[id].alive && moved < dist && guard < 20_000 {
        let before = w.players[id].pos;
        w.substep();
        moved += w.players[id].pos.distance(before);
        guard += 1;
    }
}

fn box_loop(w: &mut World, side: f32) {
    go(w, 0, RIGHT, side);
    go(w, 0, DOWN, side);
    go(w, 0, LEFT, side);
    go(w, 0, UP, side);
    go(w, 0, UP, 4.0);
}

fn owned(w: &World, p: Vec2, id: usize) -> bool {
    w.owner_at(p) == Some(id as u8)
}

fn fresh_player() {
    crate::save::reset_for_test();
    crate::profile::create("Mia", 0);
}

fn still(w: &mut World, id: usize) {
    w.players[id].speed = 0.0;
}

fn place(w: &mut World, id: usize, at: Vec2, angle: f32) {
    let p = &mut w.players[id];
    p.pos = at;
    p.angle = angle;
    p.want = angle;
}

#[test]
fn closing_a_loop_fills_the_inside_and_clears_the_trail() {
    let mut w = solo();
    let before = w.counts[0];
    box_loop(&mut w, 8.0);
    assert!(w.players[0].alive);
    assert!(!w.players[0].out());
    assert!(w.trail.iter().all(|t| t.is_none()));
    for p in [vec2(26.0, 25.0), vec2(24.0, 26.0), vec2(27.0, 22.0)] {
        assert!(owned(&w, p, 0), "{p}");
    }
    assert!(w.counts[0] as usize > before as usize + 40 * (RES * RES) as usize);
    assert_eq!(
        w.counts[0] as usize,
        w.owner.iter().filter(|o| **o == Some(0)).count()
    );
    assert_eq!(w.claims.len(), 1);
}

#[test]
fn the_fill_sweeps_out_from_the_loop() {
    let mut w = solo();
    box_loop(&mut w, 12.0);
    let near = idx(cell_at(vec2(31.0, 26.0)));
    let deep = idx(cell_at(vec2(27.0, 27.0)));
    assert!(w.changed[deep] > w.changed[near]);
    w.clock = w.changed[near] + 0.001;
    assert_eq!(w.shown(near), Some(0));
    assert_eq!(w.shown(deep), None);
    w.clock += 5.0;
    assert_eq!(w.shown(deep), Some(0));
}

#[test]
fn heads_steer_at_any_angle_and_turn_smoothly() {
    let mut w = solo();
    go(&mut w, 0, RIGHT, 5.0);
    let diag = PI / 5.0;
    w.players[0].want = diag;
    let mut last = w.players[0].angle;
    let mut biggest: f32 = 0.0;
    let mut jump: f32 = 0.0;
    let mut pos = w.players[0].pos;
    for _ in 0..240 {
        w.substep();
        let a = w.players[0].angle;
        biggest = biggest.max(wrap_angle(a - last).abs());
        jump = jump.max(w.players[0].pos.distance(pos));
        last = a;
        pos = w.players[0].pos;
    }
    assert!((w.players[0].angle - diag).abs() < 1e-3);
    assert!(
        biggest <= PLAYER_TURN * SUB + 1e-4,
        "turned {biggest} in one step"
    );
    assert!(jump < 0.1, "jumped {jump}");
    let h = w.players[0].heading();
    assert!(h.x > 0.5 && h.y > 0.4, "{h}");
    let pts = w.players[0].trail_points();
    let bends = pts
        .windows(3)
        .map(|t| wrap_angle(angle_of(t[2] - t[1]) - angle_of(t[1] - t[0])).abs())
        .fold(0.0, f32::max);
    assert!(bends < 0.5, "trail kinks by {bends}");
}

#[test]
fn a_diagonal_loop_claims_a_diamond() {
    let mut w = solo();
    let s = 7.0;
    for k in 0..4 {
        go(&mut w, 0, PI / 4.0 + k as f32 * FRAC_PI_2, s);
    }
    go(&mut w, 0, PI / 4.0, 3.0);
    assert!(w.players[0].alive);
    let c = vec2(20.0, 20.0) + vec2(0.0, s * 0.7);
    assert!(owned(&w, c, 0), "centre of the diamond");
    assert!(!owned(&w, vec2(20.0 + s * 0.65, 20.0), 0), "outside corner");
}

#[test]
fn crossing_your_own_trail_kills_you() {
    let mut w = solo();
    go(&mut w, 0, RIGHT, 8.0);
    go(&mut w, 0, DOWN, 3.0);
    go(&mut w, 0, LEFT, 3.0);
    go(&mut w, 0, UP, 4.0);
    assert!(!w.players[0].alive);
    assert_eq!(w.deaths[0].cause, Cause::OwnTrail);
    assert_eq!(w.deaths[0].killer, None);
    assert_eq!(w.counts[0], 0);
}

#[test]
fn the_tightest_turn_does_not_bite_your_own_trail() {
    let mut w = solo();
    go(&mut w, 0, RIGHT, 6.0);
    go(&mut w, 0, LEFT, 2.0);
    assert!(w.players[0].alive, "{:?}", w.deaths);
}

#[test]
fn touching_an_enemy_trail_kills_its_owner_and_credits_you() {
    let mut w = solo();
    let bot = w.add_player(vec2(40.0, 20.0), DOWN, false);
    go(&mut w, bot, DOWN, 7.0);
    still(&mut w, bot);
    assert_eq!(w.trail[idx(cell_at(vec2(40.0, 25.0)))], Some(bot as u8));
    place(&mut w, 0, vec2(36.0, 25.0), RIGHT);
    go(&mut w, 0, RIGHT, 5.0);
    assert!(!w.players[bot].alive);
    assert!(w.players[0].alive);
    assert_eq!(w.deaths[0].victim, bot);
    assert_eq!(w.deaths[0].killer, Some(0));
    assert_eq!(w.deaths[0].cause, Cause::Trail);
    assert_eq!(w.players[0].kills, 1);
    assert_eq!(w.counts[bot], 0);
    assert!(w.trail.iter().all(|t| *t != Some(bot as u8)));
}

#[test]
fn a_diagonal_pass_still_cuts_a_trail() {
    let mut w = solo();
    let bot = w.add_player(vec2(60.0, 20.0), DOWN, false);
    go(&mut w, bot, DOWN, 10.0);
    still(&mut w, bot);
    place(&mut w, 0, vec2(55.0, 22.0), PI / 4.0);
    go(&mut w, 0, PI / 4.0, 9.0);
    assert!(!w.players[bot].alive);
}

#[test]
fn a_bot_touching_your_trail_kills_you() {
    let mut w = solo();
    let bot = w.add_player(vec2(40.0, 30.0), LEFT, false);
    go(&mut w, 0, RIGHT, 8.0);
    go(&mut w, 0, DOWN, 6.0);
    still(&mut w, 0);
    place(&mut w, bot, vec2(33.0, 22.0), LEFT);
    go(&mut w, bot, LEFT, 9.0);
    assert!(!w.players[0].alive);
    assert_eq!(w.deaths[0].killer, Some(bot));
}

#[test]
fn head_on_kills_the_shorter_trail() {
    let mut w = solo();
    let bot = w.add_player(vec2(50.0, 40.0), LEFT, false);
    go(&mut w, 0, DOWN, 9.0);
    go(&mut w, 0, RIGHT, 10.0);
    go(&mut w, bot, LEFT, 4.0);
    let (mine, theirs) = (w.players[0].odo, w.players[bot].odo);
    assert!(mine > theirs, "{mine} {theirs}");
    let at = w.players[0].pos + vec2(4.0, 0.0);
    place(&mut w, bot, at, LEFT);
    for _ in 0..200 {
        w.substep();
    }
    assert!(w.players[0].alive);
    assert!(!w.players[bot].alive);
    let d = w.deaths.iter().find(|d| d.victim == bot).unwrap();
    assert_eq!(d.cause, Cause::HeadOn);
    assert_eq!(d.killer, Some(0));
}

#[test]
fn head_on_with_equal_trails_kills_both_even_at_home() {
    let mut w = World::new(0.5);
    let a = w.add_player(vec2(20.0, 20.0), RIGHT, false);
    let b = w.add_player(vec2(26.0, 20.0), LEFT, false);
    for _ in 0..200 {
        w.substep();
    }
    assert!(!w.players[a].alive && !w.players[b].alive);
    assert!(w.deaths.iter().all(|d| d.cause == Cause::HeadOn));
}

#[test]
fn enclosing_an_enemy_takes_their_land_but_not_their_life() {
    let mut w = solo();
    let bot = w.add_player(vec2(27.0, 27.0), UP, false);
    still(&mut w, bot);
    let theirs = w.counts[bot];
    assert!(theirs > 0);
    box_loop(&mut w, 14.0);
    assert!(!w.players[0].out());
    assert!(owned(&w, vec2(27.0, 27.0), 0));
    assert!(w.claims[0].taken >= theirs as usize);
    assert_eq!(w.counts[bot], 0);
    assert!(!w.players[bot].alive);
    assert_eq!(w.deaths[0].cause, Cause::Land);
}

#[test]
fn enclosing_part_of_an_enemy_leaves_them_alive() {
    let mut w = solo();
    let bot = w.add_player(vec2(36.0, 26.0), UP, false);
    still(&mut w, bot);
    let theirs = w.counts[bot];
    go(&mut w, 0, RIGHT, 13.0);
    go(&mut w, 0, DOWN, 10.0);
    go(&mut w, 0, LEFT, 13.0);
    go(&mut w, 0, UP, 12.0);
    assert!(w.players[0].alive);
    assert!(w.players[bot].alive);
    assert!(w.counts[bot] < theirs && w.counts[bot] > 0);
    assert!(w.deaths.is_empty());
}

#[test]
fn walls_turn_you_aside_and_never_kill() {
    let mut w = solo();
    place(&mut w, 0, vec2(3.0, 20.0), LEFT);
    for _ in 0..600 {
        w.substep();
    }
    assert!(w.players[0].alive);
    assert!(w.players[0].pos.x < 1.0);
    assert!(
        (w.players[0].pos.y - 20.0).abs() > 2.0,
        "slid along the fence"
    );
}

#[test]
fn counts_match_the_grid_after_a_busy_game() {
    rand::srand(5);
    let mut g = Paper::with_best(200);
    g.state = State::Playing;
    g.w.players[0].speed = 0.0;
    for _ in 0..60 * 40 {
        g.w.advance(1.0 / 60.0);
        g.w.respawn_bots(1.0 / 60.0);
    }
    for id in 0..g.w.players.len() {
        let n = g.w.owner.iter().filter(|o| **o == Some(id as u8)).count();
        assert_eq!(g.w.counts[id] as usize, n, "player {id}");
    }
}

fn bots_only(skill: f32, seed: u64, secs: usize) -> (World, usize, usize) {
    rand::srand(seed);
    let mut w = World::new(skill);
    let spots = spread_spots(10);
    w.add_player(spots[0], UP, true);
    for s in &spots[1..] {
        w.add_player(*s, rand::gen_range(-PI, PI), true);
    }
    let mut stuck = 0;
    let mut kills = 0;
    let mut last: Vec<Vec2> = w.players.iter().map(|p| p.pos).collect();
    let mut still_for = vec![0usize; w.players.len()];
    for frame in 0..secs * 60 {
        w.advance(1.0 / 60.0);
        kills += w.deaths.iter().filter(|d| d.killer.is_some()).count();
        w.deaths.clear();
        w.claims.clear();
        w.respawn_bots(1.0 / 60.0);
        if frame % 60 == 0 {
            for (id, p) in w.players.iter().enumerate() {
                if p.alive && p.pos.distance(last[id]) < 0.5 {
                    still_for[id] += 1;
                    stuck = stuck.max(still_for[id]);
                } else {
                    still_for[id] = 0;
                }
                last[id] = p.pos;
            }
        }
    }
    (w, stuck, kills)
}

#[test]
fn no_bot_ever_gets_stuck() {
    for seed in 0..3 {
        let (_, stuck, _) = bots_only(0.6, seed, 90);
        assert!(stuck <= 1, "seed {seed}: a bot sat still {stuck}s");
    }
}

#[test]
fn bots_wander_in_curves_not_on_a_grid() {
    rand::srand(3);
    let mut w = World::new(0.6);
    for s in spread_spots(8) {
        w.add_player(s, rand::gen_range(-PI, PI), true);
    }
    let mut off_axis = 0;
    let mut samples = 0;
    for _ in 0..60 * 30 {
        w.advance(1.0 / 60.0);
        w.respawn_bots(1.0 / 60.0);
        for p in w.players.iter().filter(|p| p.alive) {
            let a = p.angle.rem_euclid(FRAC_PI_2);
            samples += 1;
            if a > 0.2 && a < FRAC_PI_2 - 0.2 {
                off_axis += 1;
            }
        }
    }
    let share = off_axis as f32 / samples as f32;
    assert!(
        share > 0.25,
        "only {share:.2} of headings were off the axes"
    );
}

#[test]
fn bots_leave_trails_grow_and_eventually_kill() {
    let mut kills = 0;
    let mut grew = 0;
    let mut trails = 0;
    for seed in 0..3 {
        rand::srand(seed);
        let mut w = World::new(1.0);
        for s in spread_spots(10) {
            w.add_player(s, rand::gen_range(-PI, PI), true);
        }
        let start = w.counts.clone();
        for _ in 0..120 * 60 {
            w.advance(1.0 / 60.0);
            trails = trails.max(w.trail.iter().filter(|t| t.is_some()).count());
            kills += w.deaths.iter().filter(|d| d.killer.is_some()).count();
            w.deaths.clear();
            w.respawn_bots(1.0 / 60.0);
        }
        grew += (0..10).filter(|i| w.counts[*i] > start[*i] * 2).count();
    }
    assert!(
        trails > 20 * (RES * RES) as usize,
        "bots never went out: {trails}"
    );
    assert!(grew >= 6, "only {grew} bots grew");
    assert!(kills >= 1, "bots never killed anyone");
}

#[test]
fn gentle_bots_rarely_die_alone() {
    let (w, _, _) = bots_only(0.0, 11, 60);
    let alive = w.players.iter().filter(|p| p.alive).count();
    assert!(alive >= 7, "{alive}");
}

#[test]
fn nobody_hunts_a_new_beginner() {
    rand::srand(2);
    let mut w = World::new(0.0);
    w.add_player(vec2(55.0, 55.0), RIGHT, false);
    for s in spread_spots(9).into_iter().skip(1) {
        w.add_player(s, UP, true);
    }
    go(&mut w, 0, DOWN, 5.0);
    go(&mut w, 0, LEFT, 15.0);
    still(&mut w, 0);
    assert!(w.players[0].out());
    for _ in 0..60 * 30 {
        w.advance(1.0 / 60.0);
        w.respawn_bots(1.0 / 60.0);
    }
    assert!(w.players[0].alive, "{:?}", w.deaths);
}

fn play(g: &mut Paper, v: &View, bot: &mut Pilot, secs: f32) {
    let mut t = 0.0;
    while t < secs && g.state != State::Dead {
        if let Some(a) = bot.decide(g) {
            g.aim(a);
        }
        g.update(&Input::default(), v, 1.0 / 60.0);
        t += 1.0 / 60.0;
    }
}

fn start(g: &mut Paper, v: &View) {
    g.update(&Input::key(Dir::Right), v, 1.0 / 60.0);
}

#[test]
fn a_weak_player_who_makes_small_loops_still_grows() {
    let v = View::ipad_landscape();
    let mut grew = 0;
    for seed in 0..6 {
        rand::srand(seed);
        fresh_player();
        let mut g = Paper::new();
        start(&mut g, &v);
        let first = g.share(0);
        play(&mut g, &v, &mut Pilot::new(3, false), 40.0);
        if g.run.peak > first * 2.0 {
            grew += 1;
        }
    }
    assert!(grew >= 5, "{grew}/6 weak runs grew");
}

#[test]
fn a_strong_player_with_big_loops_climbs_past_the_bots() {
    let v = View::ipad_landscape();
    let mut tops = 0;
    for seed in 0..4 {
        rand::srand(seed);
        fresh_player();
        let mut g = Paper::new();
        start(&mut g, &v);
        play(&mut g, &v, &mut Pilot::new(9, true), 90.0);
        if g.run.peak > 0.05 && g.my_rank() <= 3 {
            tops += 1;
        }
    }
    assert!(tops >= 3, "{tops}/4 strong runs reached the top three");
}

#[test]
fn a_strong_player_still_meets_real_danger_once_the_bots_are_good() {
    let v = View::ipad_landscape();
    let mut deaths = 0;
    for seed in 0..4 {
        rand::srand(seed + 40);
        fresh_player();
        let mut g = Paper::with_best(400);
        start(&mut g, &v);
        play(&mut g, &v, &mut Pilot::new(14, false), 180.0);
        if g.state == State::Dead {
            deaths += 1;
        }
    }
    assert!(
        deaths >= 1,
        "a careless player was never caught by skilled bots"
    );
}

#[test]
fn the_ramp_follows_your_best() {
    fresh_player();
    let easy = Paper::with_best(0);
    let hard = Paper::with_best(400);
    assert!(easy.w.players.len() < hard.w.players.len());
    assert!(easy.w.players.len() >= 9 && hard.w.players.len() <= 12);
    let pace = |g: &Paper| {
        g.w.players.iter().skip(1).map(|p| p.speed).sum::<f32>() / (g.w.players.len() - 1) as f32
    };
    assert!(pace(&easy) < pace(&hard));
    let fastest = hard
        .w
        .players
        .iter()
        .skip(1)
        .map(|p| p.speed)
        .fold(0.0, f32::max);
    assert!(fastest < arena::PLAYER_SPEED);
}

#[test]
fn bots_respawn_with_new_names() {
    rand::srand(4);
    let mut w = World::new(0.5);
    w.add_player(vec2(20.0, 20.0), RIGHT, false);
    let bot = w.add_player(vec2(60.0, 60.0), UP, true);
    let first = w.players[bot].name;
    w.kill(bot, None, Cause::Trail);
    for _ in 0..600 {
        w.respawn_bots(0.01);
    }
    assert!(w.players[bot].alive);
    assert_ne!(w.players[bot].name, first);
    assert!(w.counts[bot] > 0);
}

#[test]
fn the_map_is_big_and_the_camera_zooms_out_as_you_grow() {
    fresh_player();
    let v = View::ipad_landscape();
    let mut g = Paper::new();
    let small = g.target_zoom();
    let near = g.cell_px(&v);
    for y in 30 * RES..70 * RES {
        for x in 30 * RES..70 * RES {
            let i = idx(glam::ivec2(x, y));
            g.w.claim(i, Some(0));
        }
    }
    assert!(g.target_zoom() > small * 1.5);
    for _ in 0..300 {
        g.update(&Input::default(), &v, 1.0 / 60.0);
    }
    assert!(g.cell_px(&v) < near * 0.8);
}

#[test]
fn the_head_stays_on_screen() {
    fresh_player();
    for v in [
        View::ipad_landscape(),
        View::ipad_portrait(),
        View::fit(1080.0, 2400.0),
    ] {
        let mut g = Paper::new();
        g.update(&Input::key(Dir::Up), &v, 1.0 / 60.0);
        for _ in 0..120 {
            g.update(&Input::default(), &v, 1.0 / 60.0);
        }
        let s = g.to_screen(&v, g.me().pos);
        assert!(
            s.x > 0.0 && s.x < v.w && s.y > TOP && s.y < v.h,
            "{s} {v:?}"
        );
    }
}

#[test]
fn the_game_waits_for_the_first_touch() {
    fresh_player();
    let v = View::ipad_landscape();
    let mut g = Paper::new();
    let at = g.me().pos;
    for _ in 0..120 {
        g.update(&Input::default(), &v, 1.0 / 60.0);
    }
    assert_eq!(g.state, State::Ready);
    assert_eq!(g.me().pos, at);
    let below = g.to_screen(&v, at) + vec2(0.0, 200.0);
    g.update(&Input::press_at(below), &v, 1.0 / 60.0);
    assert_eq!(g.state, State::Playing);
    assert!((g.me().want - DOWN).abs() < 0.1);
}

#[test]
fn holding_a_finger_steers_the_head_toward_it() {
    fresh_player();
    let v = View::ipad_landscape();
    let mut g = Paper::new();
    g.state = State::Playing;
    g.w.players[0].speed = arena::PLAYER_SPEED;
    let hold = |g: &Paper, off: Vec2| Input {
        pos: g.to_screen(&v, g.me().pos) + off,
        down: true,
        ..Default::default()
    };
    let ahead = g.me().pos + vec2(-9.0, -9.0);
    let target = -PI * 0.75;
    let off = g.to_screen(&v, ahead) - g.to_screen(&v, g.me().pos);
    for _ in 0..40 {
        let i = hold(&g, off);
        g.update(&i, &v, 1.0 / 60.0);
    }
    assert!((g.me().angle - target).abs() < 0.12, "{}", g.me().angle);
    let i = hold(&g, vec2(3.0, 0.0));
    g.update(&i, &v, 1.0 / 60.0);
    assert!(
        (g.me().want - g.me().angle).abs() < 0.2,
        "a finger on the head is ignored"
    );
}

#[test]
fn arrow_keys_steer_in_eight_directions() {
    fresh_player();
    let v = View::ipad_landscape();
    let mut g = Paper::new();
    g.state = State::Playing;
    let keys = Input {
        right: true,
        down_key: true,
        ..Default::default()
    };
    g.update(&keys, &v, 1.0 / 60.0);
    assert!((g.me().want - PI / 4.0).abs() < 1e-3);
}

#[test]
fn dying_shows_the_run_saves_the_best_and_play_again_respawns() {
    fresh_player();
    let v = View::ipad_landscape();
    let mut g = Paper::new();
    g.state = State::Playing;
    g.w.players[0].peak = 0.034;
    g.w.players[0].kills = 2;
    crate::sfx::take_pending();
    g.w.kill(0, Some(1), Cause::Trail);
    g.update(&Input::default(), &v, 1.0 / 60.0);
    assert_eq!(g.state, State::Dead);
    assert_eq!(g.run.kills, 2);
    assert!((g.run.peak - 0.034).abs() < 1e-6);
    assert_eq!(g.run.killer, Some(g.w.players[1].name));
    assert!(g.new_best);
    assert_eq!(crate::profile::personal_best(GAME), Some(34));
    assert!(crate::sfx::take_pending().contains(&Sfx::Lose));
    assert!(g.feed[0].text.ends_with("got you"));
    let b = play_again_button(&v);
    g.update(&Input::press_at(b.center()), &v, 1.0 / 60.0);
    assert_eq!(g.state, State::Dead, "too soon");
    for _ in 0..60 {
        g.update(&Input::default(), &v, 1.0 / 60.0);
    }
    g.update(&Input::press_at(b.center()), &v, 1.0 / 60.0);
    assert_eq!(g.state, State::Playing);
    assert!(g.me().alive && g.w.counts[0] > 0);
    assert_eq!(g.lives, 1);
    assert_eq!(Paper::new().best, 34);
}

#[test]
fn eliminating_a_bot_lands_in_the_feed() {
    fresh_player();
    let v = View::ipad_landscape();
    let mut g = Paper::new();
    g.state = State::Playing;
    let name = g.w.players[3].name;
    g.w.kill(3, Some(0), Cause::Trail);
    g.update(&Input::default(), &v, 1.0 / 60.0);
    assert_eq!(g.feed[0].text, format!("You eliminated {name}"));
    assert!(g.feed[0].mine);
    for _ in 0..(FEED_LIFE * 60.0) as usize + 5 {
        g.update(&Input::default(), &v, 1.0 / 60.0);
    }
    assert!(g.feed.iter().all(|f| !f.mine));
}

#[test]
fn the_leaderboard_ranks_by_land() {
    fresh_player();
    let mut g = Paper::new();
    let r = g.w.ranking();
    assert_eq!(r.len(), g.w.players.len());
    for c in 0..400 * (RES * RES) {
        let i = idx(glam::ivec2(c % (20 * RES), c / (20 * RES)));
        if g.w.owner[i].is_none() {
            g.w.claim(i, Some(0));
        }
    }
    assert_eq!(g.w.ranking()[0], 0);
    assert_eq!(g.my_rank(), 1);
}

#[test]
fn threats_point_at_bots_off_screen() {
    fresh_player();
    let v = View::ipad_landscape();
    let mut g = Paper::new();
    let me = g.me().pos;
    g.w.players[2].pos = me + vec2(18.0, 0.0);
    let t = g.threats(&v);
    let hit = t.iter().find(|(id, _, _)| *id == 2).expect("arrow");
    assert!(hit.1.x > 0.9);
}

#[test]
fn grazing_your_own_edge_makes_no_popup() {
    fresh_player();
    let v = View::ipad_landscape();
    let mut g = Paper::new();
    g.state = State::Playing;
    let at = g.me().pos;
    g.w.claims.push(Claim {
        id: 0,
        cells: 3,
        at,
        taken: 0,
    });
    g.update(&Input::default(), &v, 1.0 / 60.0);
    assert!(g.popups.list.is_empty());
}

fn contest(brains: bool, effort: Option<f32>, secs: f32) -> f32 {
    let env = learn::PaperEnv {
        secs,
        ..Default::default()
    };
    let setup = learn::Setup {
        lessons: false,
        skill: 1.0,
        brains,
        effort,
    };
    env.fitness(&setup, &[101, 102, 103, 104, 105, 106])
}

#[test]
fn the_shipped_brain_loads_and_matches_the_features() {
    let net =
        crate::brain::Net::from_bytes(include_bytes!("../../../../../assets/brains/paper.kgb"))
            .expect("paper.kgb");
    assert_eq!(net.inputs(), learn::FEATURES);
    let mut w = World::new(1.0);
    w.add_player(vec2(30.0, 30.0), RIGHT, true);
    assert!(w.players[0].brain.is_some());
    let (aims, feats) = w.options(0);
    assert_eq!(aims.len(), feats.len());
    assert!(feats
        .iter()
        .all(|f| f.len() == learn::FEATURES && f.iter().all(|x| (-2.01..=2.01).contains(x))));
}

#[test]
fn at_full_effort_the_brain_grows_more_land_than_the_old_bot() {
    let old = contest(false, None, 60.0);
    let new = contest(true, Some(1.0), 60.0);
    assert!(new > old, "brain {new:.3} vs old bot {old:.3}");
}

#[test]
fn bot_fitness_rises_with_effort() {
    let lazy = contest(true, Some(0.0), 60.0);
    let mid = contest(true, Some(0.5), 60.0);
    let best = contest(true, Some(1.0), 60.0);
    assert!(lazy < mid && mid < best, "{lazy:.3} {mid:.3} {best:.3}");
}

#[test]
fn a_low_effort_brain_never_gets_stuck() {
    for seed in 0..3 {
        let (_, stuck, _) = bots_only(0.0, seed, 90);
        assert!(stuck <= 1, "seed {seed}: a bot sat still {stuck}s");
    }
}

#[test]
fn block_counts_track_the_land_and_trail_grids_through_a_busy_game() {
    rand::srand(9);
    let mut g = Paper::with_best(200);
    g.state = State::Playing;
    for _ in 0..60 * 40 {
        g.w.advance(1.0 / 60.0);
        g.w.respawn_bots(1.0 / 60.0);
    }
    let mut owners = vec![[0u16; MAX_PLAYERS]; BLOCKS * BLOCKS];
    let mut trails = vec![0u16; BLOCKS * BLOCKS];
    for i in 0..g.w.owner.len() {
        if let Some(o) = g.w.owner[i] {
            owners[block_of(i)][o as usize] += 1;
        }
        if g.w.trail[i].is_some() {
            trails[block_of(i)] += 1;
        }
    }
    assert_eq!(owners, g.w.block_owner);
    assert_eq!(trails, g.w.block_trail);
    assert!(trails.iter().any(|t| *t > 0) || owners.iter().any(|o| o[1] > 0));
}

fn rl_bots_only(effort: f32, seed: u64, secs: usize) -> (World, usize) {
    rand::srand(seed);
    let mut w = World::new(0.0);
    w.rl_effort = Some(effort);
    for s in spread_spots(10) {
        w.add_player(s, rand::gen_range(-PI, PI), true);
    }
    let mut stuck = 0;
    let mut last: Vec<Vec2> = w.players.iter().map(|p| p.pos).collect();
    let mut still_for = vec![0usize; w.players.len()];
    for frame in 0..secs * 60 {
        w.advance(1.0 / 60.0);
        w.deaths.clear();
        w.claims.clear();
        if frame % 60 == 0 {
            for (id, p) in w.players.iter().enumerate() {
                if p.alive && p.pos.distance(last[id]) < 0.5 {
                    still_for[id] += 1;
                    stuck = stuck.max(still_for[id]);
                } else {
                    still_for[id] = 0;
                }
                last[id] = p.pos;
            }
        }
    }
    (w, stuck)
}

fn rl_fitness(r: &rl::Report) -> f32 {
    r.wins + r.outlived + 5.0 * r.peak - r.circling
}

#[test]
fn the_shipped_rl_brain_fits_the_survival_view() {
    let p = rl::shipped().expect("assets/brains/paper_rl.kgb fits rl::fits");
    assert!(rl::fits(p));
}

#[test]
fn every_bot_runs_the_rl_brain_at_the_players_skill() {
    fresh_player();
    for best in [0, 120, 400] {
        let g = Paper::with_best(best);
        let want = crate::brain::Effort::new(arena::rl_effort(g.skill));
        assert!(g.w.players[0].rl.is_none());
        assert!(g.w.players.iter().skip(1).all(|p| p.rl == Some(want)));
    }
    assert!(arena::rl_effort(0.0) < arena::rl_effort(0.5));
    assert!(arena::rl_effort(0.5) < arena::rl_effort(1.0));
}

#[test]
fn at_full_effort_the_rl_brain_outlives_the_hand_written_bot() {
    let p = rl::shipped().expect("policy");
    let bot = rl::evaluate(&rl::Pilot::Bot, 20, None);
    let brain = rl::evaluate(&rl::Pilot::Policy(p, crate::brain::Effort::BEST), 20, None);
    assert!(
        rl_fitness(&brain) > rl_fitness(&bot),
        "brain {brain:?} vs bot {bot:?}"
    );
    assert!(brain.outlived > bot.outlived && brain.circling < 0.25);
}

#[test]
fn rl_fitness_rises_with_effort() {
    let p = rl::shipped().expect("policy");
    let at = |e: f32| {
        rl_fitness(&rl::evaluate(
            &rl::Pilot::Policy(p, crate::brain::Effort::new(e)),
            64,
            None,
        ))
    };
    let (lazy, mid, best) = (at(0.0), at(0.8), at(1.0));
    assert!(lazy < mid && mid < best, "{lazy:.3} {mid:.3} {best:.3}");
}

#[test]
fn low_effort_rl_bots_never_get_stuck() {
    for seed in 0..3 {
        let (_, stuck) = rl_bots_only(0.15, seed, 60);
        assert!(stuck <= 1, "seed {seed}: a bot sat still {stuck}s");
    }
}

#[test]
fn rl_bots_leave_a_new_beginner_alone() {
    rand::srand(2);
    let mut w = World::new(0.0);
    w.rl_effort = Some(arena::rl_effort(0.0));
    w.add_player(vec2(55.0, 55.0), RIGHT, false);
    for s in spread_spots(9).into_iter().skip(1) {
        w.add_player(s, UP, true);
    }
    go(&mut w, 0, DOWN, 5.0);
    go(&mut w, 0, LEFT, 15.0);
    still(&mut w, 0);
    assert!(w.players[0].out());
    for _ in 0..60 * 30 {
        w.advance(1.0 / 60.0);
    }
    let by_bot = w.deaths.iter().any(|d| d.victim == 0 && d.killer.is_some());
    assert!(!by_bot, "{:?}", w.deaths);
}
