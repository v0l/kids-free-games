use super::pilot::Pilot;
use super::world::*;
use super::*;
use crate::rng as rand;
use glam::ivec2;

fn solo() -> World {
    let mut w = World::new(0.5);
    w.add_player(ivec2(20, 20), Dir::Right, false);
    w
}

fn go(w: &mut World, id: usize, d: Dir, cells: usize) {
    let speeds: Vec<f32> = w.players.iter().map(|p| p.speed).collect();
    for (o, p) in w.players.iter_mut().enumerate() {
        if o != id {
            p.speed = 0.0;
        }
    }
    walk(w, id, d, cells);
    for (p, s) in w.players.iter_mut().zip(speeds) {
        p.speed = s;
    }
}

fn walk(w: &mut World, id: usize, d: Dir, cells: usize) {
    w.players[id].wants(d);
    for _ in 0..cells {
        let start = w.players[id].cell;
        let mut guard = 0;
        while w.players[id].alive && w.players[id].cell == start && guard < 2000 {
            w.substep();
            guard += 1;
        }
        if !w.players[id].alive {
            return;
        }
    }
}

fn box_loop(w: &mut World) {
    go(w, 0, Dir::Right, 6);
    go(w, 0, Dir::Down, 6);
    go(w, 0, Dir::Left, 6);
    go(w, 0, Dir::Up, 6);
}

fn fresh_player() {
    crate::save::reset_for_test();
    crate::profile::create("Mia", 0);
}

fn still(w: &mut World, id: usize) {
    w.players[id].speed = 0.0;
}

#[test]
fn closing_a_loop_fills_the_inside_and_clears_the_trail() {
    let mut w = solo();
    let before = w.counts[0];
    box_loop(&mut w);
    assert!(w.players[0].alive);
    assert!(w.players[0].trail.is_empty());
    assert!(w.trail.iter().all(|t| t.is_none()));
    for c in [ivec2(24, 24), ivec2(25, 25), ivec2(23, 21)] {
        assert_eq!(w.owner[idx(c)], Some(0), "{c}");
    }
    assert!(w.counts[0] as usize > before as usize + 30);
    assert_eq!(
        w.counts[0] as usize,
        w.owner.iter().filter(|o| **o == Some(0)).count()
    );
    assert_eq!(w.claims.len(), 1);
}

#[test]
fn the_fill_sweeps_out_from_the_loop() {
    let mut w = solo();
    go(&mut w, 0, Dir::Right, 10);
    go(&mut w, 0, Dir::Down, 10);
    go(&mut w, 0, Dir::Left, 10);
    go(&mut w, 0, Dir::Up, 10);
    let near = idx(ivec2(29, 25));
    let deep = idx(ivec2(25, 26));
    assert!(w.changed[deep] > w.changed[near]);
    w.clock = w.changed[near] + 0.001;
    assert_eq!(w.shown(near), Some(0));
    assert_eq!(w.shown(deep), None);
    w.clock += 5.0;
    assert_eq!(w.shown(deep), Some(0));
}

#[test]
fn crossing_your_own_trail_kills_you() {
    let mut w = solo();
    go(&mut w, 0, Dir::Right, 6);
    go(&mut w, 0, Dir::Down, 3);
    go(&mut w, 0, Dir::Left, 2);
    go(&mut w, 0, Dir::Up, 3);
    assert!(!w.players[0].alive);
    assert_eq!(w.deaths[0].cause, Cause::OwnTrail);
    assert_eq!(w.deaths[0].killer, None);
    assert_eq!(w.counts[0], 0);
}

#[test]
fn touching_an_enemy_trail_kills_its_owner_and_credits_you() {
    let mut w = solo();
    let bot = w.add_player(ivec2(40, 20), Dir::Down, false);
    go(&mut w, bot, Dir::Down, 6);
    still(&mut w, bot);
    assert_eq!(w.trail[idx(ivec2(40, 25))], Some(bot as u8));
    w.players[0].cell = ivec2(36, 25);
    w.players[0].dir = Dir::Right;
    go(&mut w, 0, Dir::Right, 4);
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
fn a_bot_touching_your_trail_kills_you() {
    let mut w = solo();
    let bot = w.add_player(ivec2(40, 30), Dir::Left, false);
    go(&mut w, 0, Dir::Right, 6);
    go(&mut w, 0, Dir::Down, 5);
    still(&mut w, 0);
    w.players[bot].cell = ivec2(31, 22);
    w.players[bot].dir = Dir::Left;
    go(&mut w, bot, Dir::Left, 8);
    assert!(!w.players[0].alive);
    assert_eq!(w.deaths[0].killer, Some(bot));
}

#[test]
fn head_on_kills_the_shorter_trail() {
    let mut w = solo();
    let bot = w.add_player(ivec2(50, 40), Dir::Left, false);
    go(&mut w, 0, Dir::Down, 8);
    go(&mut w, 0, Dir::Right, 10);
    go(&mut w, bot, Dir::Left, 4);
    let mine = w.players[0].trail.len();
    let theirs = w.players[bot].trail.len();
    assert!(mine > theirs, "{mine} {theirs}");
    w.players[bot].cell = w.players[0].cell + ivec2(4, 0);
    w.players[bot].progress = 0.0;
    w.players[bot].dir = Dir::Left;
    w.players[bot].in_dir = Dir::Left;
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
    let a = w.add_player(ivec2(20, 20), Dir::Right, false);
    let b = w.add_player(ivec2(30, 20), Dir::Left, false);
    w.players[a].wants(Dir::Right);
    w.players[b].wants(Dir::Left);
    for _ in 0..200 {
        w.substep();
    }
    assert!(!w.players[a].alive && !w.players[b].alive);
    assert!(w.deaths.iter().all(|d| d.cause == Cause::HeadOn));
}

#[test]
fn enclosing_an_enemy_takes_their_land_but_not_their_life() {
    let mut w = solo();
    let bot = w.add_player(ivec2(26, 26), Dir::Up, false);
    still(&mut w, bot);
    let theirs = w.counts[bot];
    assert!(theirs > 0);
    go(&mut w, 0, Dir::Right, 12);
    go(&mut w, 0, Dir::Down, 12);
    go(&mut w, 0, Dir::Left, 12);
    go(&mut w, 0, Dir::Up, 12);
    assert!(w.players[0].trail.is_empty());
    assert_eq!(w.owner[idx(ivec2(26, 26))], Some(0));
    assert!(w.claims[0].taken >= theirs as usize);
    let lost_all = w.counts[bot] == 0;
    assert!(lost_all);
    assert!(!w.players[bot].alive);
    assert_eq!(w.deaths[0].cause, Cause::Land);
}

#[test]
fn enclosing_part_of_an_enemy_leaves_them_alive() {
    let mut w = solo();
    let bot = w.add_player(ivec2(32, 30), Dir::Up, false);
    still(&mut w, bot);
    let theirs = w.counts[bot];
    go(&mut w, 0, Dir::Right, 12);
    go(&mut w, 0, Dir::Down, 9);
    go(&mut w, 0, Dir::Left, 12);
    go(&mut w, 0, Dir::Up, 9);
    assert!(w.players[0].alive);
    assert!(w.players[bot].alive);
    assert!(w.counts[bot] < theirs && w.counts[bot] > 0);
    assert!(w.deaths.is_empty());
}

#[test]
fn walking_over_enemy_land_and_home_captures_it() {
    let mut w = solo();
    let bot = w.add_player(ivec2(29, 20), Dir::Up, false);
    still(&mut w, bot);
    w.players[bot].cell = ivec2(29, 40);
    let before = w.counts[bot];
    go(&mut w, 0, Dir::Right, 7);
    go(&mut w, 0, Dir::Down, 2);
    go(&mut w, 0, Dir::Left, 7);
    assert!(w.counts[bot] < before);
    assert_eq!(w.owner[idx(ivec2(27, 20))], Some(0));
}

#[test]
fn walls_turn_you_aside_and_never_kill() {
    let mut w = solo();
    w.players[0].cell = ivec2(3, 20);
    w.players[0].dir = Dir::Left;
    for _ in 0..600 {
        w.substep();
    }
    assert!(w.players[0].alive);
    assert_eq!(w.players[0].cell.x, 0);
}

#[test]
fn a_quick_double_turn_is_buffered() {
    let mut w = solo();
    go(&mut w, 0, Dir::Right, 3);
    let y = w.players[0].cell.y;
    w.players[0].wants(Dir::Down);
    w.players[0].wants(Dir::Left);
    for _ in 0..200 {
        w.substep();
    }
    assert!(w.players[0].cell.y > y);
    assert_eq!(w.players[0].dir, Dir::Left);
}

#[test]
fn a_reverse_press_is_ignored_and_does_not_eat_the_queue() {
    let mut w = solo();
    w.players[0].wants(Dir::Left);
    assert!(w.players[0].queue.is_empty());
    w.players[0].wants(Dir::Up);
    w.players[0].wants(Dir::Down);
    assert_eq!(w.players[0].queue.len(), 1);
}

#[test]
fn heads_move_smoothly_and_round_corners() {
    let mut w = solo();
    let mut last = w.players[0].look();
    let mut biggest: f32 = 0.0;
    w.players[0].wants(Dir::Down);
    for _ in 0..240 {
        w.substep();
        let now = w.players[0].look();
        biggest = biggest.max(now.distance(last));
        last = now;
    }
    assert!(biggest < 0.1, "jump {biggest}");
    let mut w = solo();
    go(&mut w, 0, Dir::Right, 2);
    w.players[0].wants(Dir::Down);
    while w.players[0].dir == w.players[0].in_dir {
        w.substep();
    }
    while w.players[0].progress < 0.5 {
        w.substep();
    }
    let p = w.players[0].look();
    let c = center(w.players[0].cell);
    assert!(p.x < c.x - 0.05 && p.y > c.y + 0.05, "{p} {c}");
    let pts = w.players[0].trail_points();
    for pair in pts.windows(2) {
        assert!(pair[0].distance(pair[1]) < 1.01);
    }
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
    w.add_player(spots[0], Dir::Up, true);
    for s in &spots[1..] {
        w.add_player(*s, Dir::ALL[rand::gen_range(0, 4)], true);
    }
    let mut stuck = 0;
    let mut kills = 0;
    let mut last: Vec<IVec2> = w.players.iter().map(|p| p.cell).collect();
    let mut still_for = vec![0usize; w.players.len()];
    for frame in 0..secs * 60 {
        w.advance(1.0 / 60.0);
        kills += w.deaths.iter().filter(|d| d.killer.is_some()).count();
        w.deaths.clear();
        w.claims.clear();
        w.respawn_bots(1.0 / 60.0);
        if frame % 60 == 0 {
            for (id, p) in w.players.iter().enumerate() {
                if p.alive && p.cell == last[id] {
                    still_for[id] += 1;
                    stuck = stuck.max(still_for[id]);
                } else {
                    still_for[id] = 0;
                }
                last[id] = p.cell;
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
fn bots_leave_trails_grow_and_eventually_kill() {
    let mut kills = 0;
    let mut grew = 0;
    let mut trails = 0;
    for seed in 0..3 {
        rand::srand(seed);
        let mut w = World::new(1.0);
        for s in spread_spots(10) {
            w.add_player(s, Dir::ALL[rand::gen_range(0, 4)], true);
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
    assert!(trails > 20, "bots never went out: {trails}");
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
    w.add_player(ivec2(55, 55), Dir::Right, false);
    for s in spread_spots(9).into_iter().skip(1) {
        w.add_player(s, Dir::Up, true);
    }
    w.players[0].speed = 0.0;
    w.players[0].trail = (40..55).map(|x| ivec2(x, 60)).collect();
    w.players[0].trail_from = ivec2(39, 60);
    for c in w.players[0].trail.clone() {
        w.trail[idx(c)] = Some(0);
    }
    for _ in 0..60 * 30 {
        w.advance(1.0 / 60.0);
        w.respawn_bots(1.0 / 60.0);
    }
    assert!(w.players[0].alive, "{:?}", w.deaths);
}

fn play(g: &mut Paper, v: &View, bot: &mut Pilot, secs: f32) {
    let mut t = 0.0;
    while t < secs && g.state != State::Dead {
        let input = Input {
            key_dir: bot.decide(g),
            ..Default::default()
        };
        g.update(&input, v, 1.0 / 60.0);
        t += 1.0 / 60.0;
    }
}

#[test]
fn a_weak_player_who_makes_small_loops_still_grows() {
    let v = View::ipad_landscape();
    let mut grew = 0;
    for seed in 0..6 {
        rand::srand(seed);
        fresh_player();
        let mut g = Paper::new();
        g.update(&Input::key(Dir::Right), &v, 1.0 / 60.0);
        let start = g.share(0);
        play(&mut g, &v, &mut Pilot::new(3, false), 40.0);
        if g.run.peak > start * 2.0 {
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
        g.update(&Input::key(Dir::Right), &v, 1.0 / 60.0);
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
        g.update(&Input::key(Dir::Right), &v, 1.0 / 60.0);
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
    assert!(g_max_speed(&hard) < arena::PLAYER_SPEED);
}

fn g_max_speed(g: &Paper) -> f32 {
    g.w.players
        .iter()
        .skip(1)
        .map(|p| p.speed)
        .fold(0.0, f32::max)
}

#[test]
fn bots_respawn_with_new_names() {
    rand::srand(4);
    let mut w = World::new(0.5);
    w.add_player(ivec2(20, 20), Dir::Right, false);
    let bot = w.add_player(ivec2(60, 60), Dir::Up, true);
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
    for y in 30..70 {
        for x in 30..70 {
            g.w.owner[idx(ivec2(x, y))] = Some(0);
        }
    }
    g.w.counts[0] = 1600;
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
        let s = g.to_screen(&v, g.me().look());
        assert!(
            s.x > 0.0 && s.x < v.w && s.y > TOP && s.y < v.h,
            "{s} {v:?}"
        );
    }
}

#[test]
fn the_game_waits_for_the_first_swipe() {
    fresh_player();
    let v = View::ipad_landscape();
    let mut g = Paper::new();
    let at = g.me().cell;
    for _ in 0..120 {
        g.update(&Input::default(), &v, 1.0 / 60.0);
    }
    assert_eq!(g.state, State::Ready);
    assert_eq!(g.me().cell, at);
    g.update(
        &Input {
            swipe: Some(Dir::Up),
            ..Default::default()
        },
        &v,
        1.0 / 60.0,
    );
    assert_eq!(g.state, State::Playing);
}

#[test]
fn dragging_steers_without_lifting_the_finger() {
    fresh_player();
    let v = View::ipad_landscape();
    let mut g = Paper::new();
    g.state = State::Playing;
    let o = v.center();
    let mm = Input::default().mm();
    let at = |p: Vec2, pressed: bool| Input {
        pos: p,
        down: true,
        pressed,
        ..Default::default()
    };
    g.update(&at(o, true), &v, 1.0 / 60.0);
    g.update(&at(o + vec2(0.0, 10.0 * mm), false), &v, 1.0 / 60.0);
    assert_eq!(g.steer, Some(Dir::Down));
    g.update(&at(o + vec2(-10.0 * mm, 10.0 * mm), false), &v, 1.0 / 60.0);
    assert_eq!(g.steer, Some(Dir::Left));
    assert_eq!(
        g.me().queue.len() + usize::from(g.me().dir != Dir::Right),
        2
    );
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
    assert!(g.feed.is_empty());
}

#[test]
fn the_leaderboard_ranks_by_land() {
    fresh_player();
    let mut g = Paper::new();
    let r = g.w.ranking();
    assert_eq!(r.len(), g.w.players.len());
    for c in 0..400 {
        let i = idx(ivec2(c % 20, c / 20));
        if g.w.owner[i].is_none() {
            g.w.owner[i] = Some(0);
            g.w.counts[0] += 1;
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
    let me = g.me().cell;
    g.w.players[2].cell = me + ivec2(18, 0);
    g.w.players[2].progress = 0.0;
    let t = g.threats(&v);
    let hit = t.iter().find(|(id, _, _)| *id == 2).expect("arrow");
    assert!(hit.1.x > 0.9);
}
