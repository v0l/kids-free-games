use crate::engine::{Dir, Game, Input, Key, Script, View};
use crate::geom::Rect;
use glam::{vec2, Vec2};

const DT: f32 = 1.0 / 60.0;

fn fresh() {
    crate::save::reset_for_test();
    crate::profile::create("Mia", 0);
}

fn view() -> View {
    View::ipad_landscape()
}

fn script() -> Script {
    Script::new(view())
}

fn feed<G: Game>(g: &mut G, frames: &[Input]) {
    Script::feed(g, &view(), frames);
}

fn tap_at<G: Game>(g: &mut G, at: Vec2) {
    let mut s = script();
    feed(g, &s.tap(at));
}

fn tap_rect<G: Game>(g: &mut G, r: Rect) {
    tap_at(g, r.center());
}

#[test]
fn flappy_flaps_on_a_scripted_tap() {
    use super::flappy::*;
    fresh();
    let mut g = Flappy::new();
    tap_at(&mut g, vec2(400.0, 300.0));
    assert_eq!(g.state, State::Playing);
    assert!(g.vy < 0.0);
}

#[test]
fn fruit_is_cut_by_a_scripted_drag() {
    use super::fruit::*;
    fresh();
    let mut g = Fruit::new();
    g.state = State::Playing;
    g.things.push(Thing {
        pos: vec2(300.0, 300.0),
        vel: Vec2::ZERO,
        r: 40.0,
        kind: Kind::Fruit(1),
        spin: 0.0,
    });
    let mut s = script();
    let frames = [
        s.press(vec2(200.0, 300.0)),
        s.drag_to(vec2(300.0, 300.0)),
        s.drag_to(vec2(400.0, 300.0)),
        s.release(),
    ];
    feed(&mut g, &frames);
    assert_eq!(g.halves.len(), 2);
}

#[test]
fn stack_drops_a_block_on_a_scripted_tap() {
    use super::stack::*;
    fresh();
    let mut s = Stack::for_level(1);
    tap_at(&mut s, vec2(400.0, 300.0));
    assert!(s.tower.len() == 2 || s.state != State::Playing);
}

#[test]
fn snake_turns_on_a_scripted_swipe() {
    use super::snake::*;
    fresh();
    let mut g = Snake::new();
    let mut sc = script();
    feed(&mut g, &sc.swipe(vec2(400.0, 300.0), Dir::Up, 3));
    assert_eq!(g.state, State::Playing);
    assert!(g.queued.contains(&Dir::Up) || g.dir == Dir::Up);
}

#[test]
fn snake_turns_on_the_arrow_keys_too() {
    use super::snake::*;
    fresh();
    let mut g = Snake::new();
    let mut sc = script();
    feed(&mut g, &sc.keys(&[Key::W], 1));
    assert_eq!(g.state, State::Playing);
    assert!(g.queued.contains(&Dir::Up) || g.dir == Dir::Up);
}

#[test]
fn breakout_paddle_follows_a_scripted_drag_and_the_keys() {
    use super::breakout::*;
    fresh();
    let mut b = Breakout::new();
    let mut sc = script();
    let start = b.paddle_x;
    feed(&mut b, &sc.keys(&[Key::Right], 10));
    assert!(b.paddle_x > start);
    let mut b = Breakout::new();
    let mut frames = vec![sc.press(vec2(120.0, 600.0))];
    frames.extend((0..30).map(|_| sc.drag_to(vec2(120.0, 600.0))));
    feed(&mut b, &frames);
    assert!((b.paddle_x - 120.0).abs() < 40.0);
}

#[test]
fn jumper_starts_on_a_tap_and_steers_with_the_keys() {
    use super::jumper::*;
    fresh();
    let mut j = Jumper::new();
    tap_at(&mut j, vec2(100.0, 100.0));
    assert_eq!(j.state, State::Playing);
    let x = j.pos.x;
    let mut sc = script();
    feed(&mut j, &sc.keys(&[Key::Right], 20));
    assert!(j.pos.x != x);
}

#[test]
fn merge_slides_on_a_scripted_swipe() {
    use super::merge::*;
    fresh();
    let mut g = Merge::at(1);
    let before = g.board;
    let mut sc = script();
    for f in sc.swipe(vec2(400.0, 300.0), Dir::Left, 3) {
        g.update(&f, &view(), SLIDE + 0.01);
    }
    assert_ne!(g.board, before);
}

#[test]
fn memory_flips_the_card_under_a_scripted_tap() {
    use super::memory::*;
    fresh();
    let mut m = Memory::for_level(1);
    m.screen = Screen::Play;
    let rects = Memory::layout(&view(), m.pairs);
    tap_rect(&mut m, rects[0]);
    assert_eq!(m.cards[0].1, Face::Up);
}

#[test]
fn crossy_hops_on_a_scripted_swipe_and_a_tap() {
    use super::crossy::*;
    fresh();
    let mut g = Crossy::new();
    let mut sc = script();
    feed(&mut g, &sc.swipe(vec2(480.0, 400.0), Dir::Up, 3));
    feed(&mut g, &vec![Input::default(); 30]);
    assert_eq!(g.state, State::Playing);
    assert!(g.row >= 1);
}

#[test]
fn runner_changes_lane_on_a_scripted_swipe() {
    use super::runner::*;
    fresh();
    let mut g = Runner::with_level(5);
    g.state = State::Playing;
    let mut sc = script();
    feed(&mut g, &sc.swipe(vec2(480.0, 400.0), Dir::Left, 3));
    assert_eq!(g.lane, -1);
}

#[test]
fn helix_turns_with_the_scripted_direction_keys() {
    use super::helix::*;
    fresh();
    let mut g = Helix::with_level(1);
    g.state = State::Playing;
    let a = g.angle;
    let mut sc = script();
    feed(&mut g, &sc.keys(&[Key::D], 10));
    assert!(g.angle > a);
    let b = g.angle;
    feed(&mut g, &sc.pad(vec2(-1.0, 0.0), 10));
    assert!(g.angle < b);
}

#[test]
fn trains_opens_a_level_from_a_scripted_tap() {
    use super::trains::*;
    fresh();
    let mut g = Trains::new();
    g.mode = Mode::Levels;
    let tiles = level_tiles(&view());
    tap_rect(&mut g, tiles[0]);
    assert_eq!(g.mode, Mode::Play);
}

#[test]
fn ballsort_selects_the_tube_under_a_scripted_tap() {
    use super::ballsort::*;
    use glam::vec3;
    fresh();
    let v = view();
    let mut g = BallSort::with_level(1);
    g.update(&Input::default(), &v, DT);
    let s = g.spot(0);
    let at = g.cam.project(&v, vec3(s.x, TUBE_H * 0.5, s.y));
    tap_at(&mut g, at);
    assert!(g.selected.is_some());
}

#[test]
fn garden_opens_a_bed_from_a_scripted_tap() {
    use super::garden::*;
    fresh();
    let mut g = Garden::new();
    g.mode = Mode::Beds;
    let tiles = bed_tiles(&view());
    tap_rect(&mut g, tiles[0]);
    assert_eq!(g.mode, Mode::Play);
}

#[test]
fn blocks_shift_on_the_scripted_direction_keys() {
    use super::blocks::*;
    fresh();
    let mut g = Blocks::new();
    g.state = State::Playing;
    let x = g.piece.x;
    let mut sc = script();
    feed(&mut g, &sc.keys(&[Key::A], 1));
    assert!(g.piece.x < x);
}

#[test]
fn count_opens_a_level_from_a_scripted_tap() {
    use super::count::*;
    fresh();
    let mut g = Count::new();
    g.mode = Mode::Levels;
    let tiles = level_tiles(&view());
    tap_rect(&mut g, tiles[0]);
    assert_eq!(g.mode, Mode::Play);
}

#[test]
fn mathpop_pops_the_balloon_under_a_scripted_tap() {
    use super::mathpop::*;
    fresh();
    let v = view();
    let mut g = MathPop::new();
    g.asked = 5.0;
    let slot = g.question.slot_of_answer();
    let at = g.center(&v, slot);
    tap_at(&mut g, at);
    assert!(g.solved);
}

#[test]
fn spell_opens_a_level_from_a_scripted_tap() {
    use super::spell::*;
    fresh();
    crate::profile::unlock(KEY, 3);
    let mut g = Spell::new();
    g.screen = Screen::Levels;
    let l = levels_layout(&view());
    tap_rect(&mut g, l.tiles[3]);
    assert_eq!(g.screen, Screen::Play);
    assert_eq!(g.level, 3);
}

#[test]
fn clash_starts_on_enter_and_steers_toward_a_scripted_finger() {
    use super::clash::*;
    fresh();
    let v = view();
    let mut g = Clash::new();
    let mut sc = script();
    for f in sc.keys(&[Key::Enter], 1) {
        g.update(&f, &v, DT);
    }
    assert_eq!(g.screen, Screen::Playing);
    let below = g.centre(&v) + vec2(0.0, 250.0);
    let mut frames = vec![sc.press(below)];
    frames.push(sc.drag_to(below));
    for f in &frames {
        g.update(f, &v, DT);
    }
    assert!((g.steer - std::f32::consts::FRAC_PI_2).abs() < 0.05);
}

#[test]
fn paper_steers_with_keys_and_toward_a_scripted_finger() {
    use super::paper::*;
    fresh();
    let mut g = Paper::new();
    g.state = State::Playing;
    let mut sc = script();
    feed(&mut g, &sc.keys(&[Key::S], 1));
    assert!((g.me().want - std::f32::consts::FRAC_PI_2).abs() < 1e-3);
    feed(&mut g, &sc.pad(vec2(-1.0, 0.0), 1));
    assert!((g.me().want.abs() - std::f32::consts::PI).abs() < 1e-3);
}

#[test]
fn hotel_walks_with_the_shared_stick() {
    use super::hotel::*;
    fresh();
    let mut h = Hotel::fresh_for_test(1);
    let v = view();
    let start = h.sim.player.pos;
    let mut sc = script();
    let mut frames = vec![sc.press(v.center())];
    frames.extend((0..40).map(|_| sc.drag_to(v.center() + vec2(70.0, 0.0))));
    feed(&mut h, &frames);
    assert!(h.sim.player.pos.distance(start) > 0.3);
}
