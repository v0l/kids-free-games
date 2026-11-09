use glam::{vec2, Vec2};
use kg_core::engine::{Dir, Game, Input, Script, View};
use kg_core::games::runner::{lane_x, Runner, State};

const DT: f32 = 1.0 / 60.0;

fn fresh(lane: i32) -> Runner {
    kg_core::save::reset_for_test();
    kg_core::profile::create("Mia", 0);
    let mut g = Runner::with_level(5);
    g.state = State::Playing;
    g.lane = lane;
    g.x = lane_x(lane);
    for _ in 0..20 {
        g.update(&Input::default(), &View::ipad_landscape(), DT);
    }
    g
}

fn drag(sc: &mut Script, from: Vec2, dir: Dir, mm_per_frame: f32, frames: usize) -> Vec<Input> {
    let per = dir.delta().as_vec2() * mm_per_frame * 6.0;
    let mut out = vec![sc.press(from)];
    for i in 1..=frames {
        out.push(sc.drag_to(from + per * i as f32));
    }
    out.push(sc.release());
    for _ in 0..30 {
        out.push(sc.idle());
    }
    out
}

fn first_frame(g: &mut Runner, frames: &[Input], hit: impl Fn(&Runner) -> bool) -> Option<usize> {
    let view = View::ipad_landscape();
    for (i, f) in frames.iter().enumerate() {
        g.update(f, &view, DT);
        if hit(g) {
            return Some(i);
        }
    }
    None
}

fn ms(f: Option<usize>) -> String {
    f.map_or("never".into(), |f| format!("{:5.1}", f as f32 * DT * 1000.0))
}

fn main() {
    let view = View::ipad_landscape();
    let from = vec2(view.w / 2.0, view.h / 2.0);
    println!("finger mm/s | move | decided | visible(30% of lane) | event");
    for speed in [100.0f32, 200.0, 400.0] {
        let per_frame = speed * DT;
        let frames = (20.0 / per_frame).ceil() as usize;

        let mut sc = Script::new(view);
        let mut g = fresh(0);
        let inputs = drag(&mut sc, from, Dir::Left, per_frame, frames);
        let lane = first_frame(&mut g, &inputs, |g| g.lane == -1);
        let mut g = fresh(0);
        let vis = first_frame(&mut g, &inputs, |g| g.x < -0.3 * kg_core::games::runner::LANE_W);
        println!("{speed:>6} | lane  | {} ms | {} ms", ms(lane), ms(vis));

        let mut sc = Script::new(view);
        let mut g = fresh(0);
        let inputs = drag(&mut sc, from, Dir::Up, per_frame, frames);
        let up = first_frame(&mut g, &inputs, |g| g.y > 0.0);
        println!("{speed:>6} | jump  | {} ms |", ms(up));

        let mut sc = Script::new(view);
        let mut g = fresh(0);
        let inputs = drag(&mut sc, from, Dir::Down, per_frame, frames);
        let dn = first_frame(&mut g, &inputs, |g| g.sliding());
        println!("{speed:>6} | slide | {} ms |", ms(dn));

        let long = (40.0 / per_frame).ceil() as usize;
        let mut sc = Script::new(view);
        let mut g = fresh(1);
        let inputs = drag(&mut sc, from, Dir::Left, per_frame, long);
        let two = first_frame(&mut g, &inputs, |g| g.lane == -1);
        println!("{speed:>6} | 2 lanes in one 40mm touch | {} ms |", ms(two));
    }
}
