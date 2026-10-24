use kids_games::engine::*;
use kids_games::games;
use kids_games::safe::Insets;
use kids_games::{begin_frame_safe, menu::App, window_conf};
use macroquad::prelude::*;

fn scripted(game: &str, f: usize, view: &View) -> Input {
    let c = view.center();
    let mut i = Input {
        pos: c,
        prev_pos: c,
        ..Default::default()
    };
    let tap = |i: &mut Input, p: Vec2| {
        i.pos = p;
        i.prev_pos = p;
        i.pressed = true;
        i.down = true;
    };
    match game {
        "Flappy Bird" if f.is_multiple_of(24) => tap(&mut i, c),
        "Paper Land" => {
            if f == 1 {
                i.pressed = true;
            }
            let leg = (f / 50) % 4;
            if f.is_multiple_of(50) {
                i.swipe = Some([Dir::Up, Dir::Right, Dir::Down, Dir::Left][leg]);
            }
        }
        "Snake" => {
            if f == 1 {
                i.pressed = true;
            }
            if f % 60 == 30 {
                i.swipe = Some([Dir::Down, Dir::Right, Dir::Up, Dir::Right][(f / 60) % 4]);
            }
        }
        "2048" if f.is_multiple_of(5) => i.swipe = Some(Dir::ALL[(f / 5) % 4]),
        "Fruit Slice" => {
            if f == 1 {
                i.pressed = true;
            }
            if f > 200 && f % 40 < 10 {
                let k = (f % 40) as f32 / 10.0;
                i.down = true;
                i.prev_pos = vec2(view.w * (0.1 + 0.8 * (k - 0.1)), view.h * 0.45);
                i.pos = vec2(view.w * (0.1 + 0.8 * k), view.h * 0.4);
            }
        }
        "Stack Tower" if f % 50 == 49 => tap(&mut i, c),
        "Helix Drop" => {
            if f == 1 {
                i.pressed = true;
            }
            if f > 5 {
                i.down = true;
                i.pos = vec2(c.x + (f as f32 * 0.05).sin() * 300.0, c.y);
            }
        }
        "Block Drop" => {
            if f == 1 {
                i.pressed = true;
            }
            if f > 5 && f.is_multiple_of(20) {
                i.key_dir =
                    Some([Dir::Left, Dir::Down, Dir::Right, Dir::Up, Dir::Down][(f / 20) % 5]);
            }
        }
        "Road Hopper" if f.is_multiple_of(30) && f < 200 => i.swipe = Some(Dir::Up),
        "Brick Breaker" => {
            if f == 2 {
                i.released = true;
            }
        }
        "Sky Jumper" => {
            if f == 1 {
                i.pressed = true;
            }
        }
        "Memory Match" if f.is_multiple_of(30) && f < 300 => {
            let n = 16;
            let k = (f / 30) % n;
            let cols = if view.w >= view.h { 8 } else { 4 };
            let rows = n / cols;
            let size = ((view.w - 40.0) / cols as f32).min((view.h - 100.0) / rows as f32);
            let ox = (view.w - size * cols as f32) / 2.0;
            let oy = 80.0 + (view.h - 100.0 - size * rows as f32) / 2.0;
            tap(
                &mut i,
                vec2(
                    ox + (k % cols) as f32 * size + size / 2.0,
                    oy + (k / cols) as f32 * size + size / 2.0,
                ),
            );
        }
        "Hotel Hop" if f.is_multiple_of(45) => {
            let top = 80.0;
            let lobby_w = (view.w * 0.26).clamp(220.0, 320.0);
            let gx = 16.0 + lobby_w + 16.0;
            let cw = (view.w - gx - 16.0) / 4.0;
            let ch = (view.h - top - 16.0) / 3.0;
            let room = (f / 45) % 3;
            tap(
                &mut i,
                vec2(
                    gx + (room % 4) as f32 * cw + cw / 2.0,
                    top + 2.0 * ch + ch / 2.0,
                ),
            );
        }
        _ => {}
    }
    i
}

fn app_icon(view: &View) {
    sky(
        view,
        Color::from_rgba(255, 200, 120, 255),
        Color::from_rgba(255, 140, 170, 255),
    );
    let all = games::all();
    let s = view.w.min(view.h);
    let o = vec2((view.w - s) / 2.0, (view.h - s) / 2.0);
    let cell = s * 0.36;
    let gap = s * 0.06;
    let start = o + vec2((s - cell * 2.0 - gap) / 2.0, (s - cell * 2.0 - gap) / 2.0);
    for (k, i) in [0usize, 1, 2, 4].iter().enumerate() {
        let r = Rect::new(
            start.x + (k % 2) as f32 * (cell + gap),
            start.y + (k / 2) as f32 * (cell + gap),
            cell,
            cell,
        );
        round_rect(
            Rect::new(r.x, r.y + s * 0.012, r.w, r.h),
            s * 0.06,
            Color::from_rgba(0, 0, 0, 50),
        );
        round_rect(r, s * 0.06, all[*i].color);
        (all[*i].icon)(Rect::new(
            r.x + r.w * 0.12,
            r.y + r.h * 0.12,
            r.w * 0.76,
            r.h * 0.76,
        ));
    }
}

#[cfg(target_arch = "wasm32")]
#[link(wasm_import_module = "env")]
unsafe extern "C" {
    fn shot_index() -> i32;
    fn shot_inset() -> i32;
}

#[cfg(target_arch = "wasm32")]
fn pick() -> i32 {
    unsafe { shot_index() }
}

#[cfg(target_arch = "wasm32")]
fn fake_inset() -> Insets {
    let (w, h) = macroquad::miniquad::window::screen_size();
    let k = unsafe { shot_inset() } as f32;
    if k == 0.0 {
        return Insets::default();
    }
    if h > w {
        Insets {
            left: 0.0,
            top: k,
            right: 0.0,
            bottom: k * 0.35,
        }
    } else {
        Insets {
            left: k,
            top: 0.0,
            right: 0.0,
            bottom: k * 0.35,
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn fake_inset() -> Insets {
    Insets::default()
}

#[cfg(not(target_arch = "wasm32"))]
fn pick() -> i32 {
    std::env::var("SHOT")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(0)
}

#[macroquad::main(window_conf)]
async fn main() {
    let frames = 420;
    rand::srand(42);
    let which = pick();
    next_frame().await;
    let view = begin_frame_safe(fake_inset());
    let all = games::all();
    let mut game = (which > 0).then(|| {
        let e = &all[which as usize - 1];
        set_accent(e.color);
        let mut g = (e.make)();
        for f in 0..frames {
            g.update(&scripted(e.name, f, &view), &view, 1.0 / 60.0);
            tick_overlay(1.0 / 60.0);
        }
        g
    });
    let mut app = App::new();
    for _ in 0..90 {
        app.update(&Input::default(), &view, 1.0 / 60.0);
    }
    loop {
        let view = begin_frame_safe(fake_inset());
        match &mut game {
            Some(g) => g.draw(&view),
            None if which < 0 => app_icon(&view),
            None => app.draw(&view),
        }
        next_frame().await;
    }
}
