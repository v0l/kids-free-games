use glam::{vec2, Vec2};
use kg_core::engine::{Dir, Game, Input, Script, View};
use kg_core::games::snake::{Snake, State, STAGE_APPLES};

const DT: f32 = 1.0 / 60.0;
const PX_PER_MM: f32 = 16.0;
const FINGER_MM_PER_S: f32 = 120.0;

fn drawn_head(g: &Snake) -> Vec2 {
    g.drawn_head()
}

fn visibly_turned(g: &Snake, y0: f32, face0: f32) -> bool {
    turned(vec2(0.0, y0), vec2(0.0, drawn_head(g).y))
        || (!path_only() && kg_core::games::snake::wrap_angle(g.face - face0).abs() > 0.17)
}

fn path_only() -> bool {
    std::env::var("PATH_ONLY").is_ok()
}

fn ready(score: u32, view: &View, phase: f32) -> Snake {
    let mut g = Snake::new();
    g.update(&Input::default(), view, DT);
    g.state = State::Playing;
    g.score = score;
    g.moves = 1000;
    g.best = kg_core::engine::Best(50);
    g.food = glam::ivec2(0, 0);
    let step = g.step_time();
    let mut t = 0.0;
    while t < step * 3.0 + phase * step {
        g.update(&Input::default(), view, DT);
        t += DT;
    }
    g
}

fn turned(before: Vec2, now: Vec2) -> bool {
    (now - before).y < -0.02
}

fn swipe_latency(score: u32, phase: f32) -> f32 {
    let view = View::fit(1080.0, 2400.0);
    let mut g = ready(score, &view, phase);
    let mut sc = Script::new(view).mm(PX_PER_MM);
    let from = vec2(view.w / 2.0, view.h / 2.0);
    let per_frame = FINGER_MM_PER_S * DT * PX_PER_MM / view.scale;
    let y0 = drawn_head(&g).y;
    let face0 = g.face;
    g.update(&sc.press(from), &view, DT);
    let mut f = 0;
    loop {
        f += 1;
        let input = if f <= 12 {
            sc.drag_to(from + vec2(0.0, -per_frame * f as f32))
        } else if f == 13 {
            sc.release()
        } else {
            sc.idle()
        };
        g.update(&input, &view, DT);
        if visibly_turned(&g, y0, face0) || f > 200 {
            break;
        }
    }
    f as f32 * DT * 1000.0
}

fn key_latency(score: u32, phase: f32) -> f32 {
    let view = View::fit(1080.0, 2400.0);
    let mut g = ready(score, &view, phase);
    let y0 = drawn_head(&g).y;
    let face0 = g.face;
    g.update(&Input::key(Dir::Up), &view, DT);
    let mut f = 1;
    while !visibly_turned(&g, y0, face0) && f < 200 {
        g.update(&Input::default(), &view, DT);
        f += 1;
    }
    f as f32 * DT * 1000.0
}

fn report(name: &str, score: u32) {
    let phases: Vec<f32> = (0..20).map(|i| i as f32 / 20.0).collect();
    let stats = |v: Vec<f32>| {
        let mean = v.iter().sum::<f32>() / v.len() as f32;
        let max = v.iter().cloned().fold(0.0, f32::max);
        let min = v.iter().cloned().fold(f32::MAX, f32::min);
        (min, mean, max)
    };
    kg_core::save::use_memory_only();
    kg_core::profile::create("Lat", 0);
    let step = {
        let mut g = Snake::new();
        g.score = score;
        g.step_time()
    };
    let (kmin, kmean, kmax) = stats(phases.iter().map(|p| key_latency(score, *p)).collect());
    let (smin, smean, smax) = stats(phases.iter().map(|p| swipe_latency(score, *p)).collect());
    println!(
        "{name:8} step {:3.0} ms | key {kmin:4.0} / {kmean:4.0} / {kmax:4.0} ms | swipe@{FINGER_MM_PER_S}mm/s {smin:4.0} / {smean:4.0} / {smax:4.0} ms",
        step * 1000.0
    );
}

fn main() {
    println!("input to visible turn, min / mean / max over 20 tick phases, 60 fps");
    report("start", 0);
    report("fastest", STAGE_APPLES * 20);
}
