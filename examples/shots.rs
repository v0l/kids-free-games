use kids_games::engine::*;
use kids_games::games::memory::{self, Memory};
use kids_games::games::{self, count, mathpop, merge};
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
        "Flappy Bird" if f.is_multiple_of(32) => tap(&mut i, c),
        "Paper Land" => {
            if f == 1 {
                i.pressed = true;
            }
            let legs = [
                (2, Dir::Right),
                (125, Dir::Down),
                (248, Dir::Left),
                (371, Dir::Up),
            ];
            for (at, dir) in legs {
                if f == at {
                    i.swipe = Some(dir);
                }
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
        "2048" if f == 300 => tap(&mut i, merge::complete_buttons(view).1.center()),
        "2048" if f < 300 && f.is_multiple_of(5) => i.swipe = Some(Dir::ALL[(f / 5) % 4]),
        "Fruit Slice" => {
            if f == 1 {
                i.pressed = true;
            }
            if f > 60 && f % 12 < 6 {
                let k = (f % 12) as f32 / 6.0;
                let y = view.h * (0.25 + 0.5 * ((f / 12) as f32 * 0.37).fract());
                let dir = if (f / 12).is_multiple_of(2) {
                    1.0
                } else {
                    -1.0
                };
                let x = |k: f32| view.w * (0.5 + dir * (k - 0.5) * 0.9);
                i.down = true;
                i.prev_pos = vec2(x(k - 0.17), y);
                i.pos = vec2(x(k), y);
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
        "Spell It" => {
            if f == 120 || f == 160 {
                let n = 3usize;
                let k = learn_tiles_x(view, n);
                tap(&mut i, k);
                i.released = true;
            }
        }
        "Road Hopper" if f.is_multiple_of(30) && f < 200 => i.swipe = Some(Dir::Up),
        "Brick Breaker" => {
            if f == 2 {
                i.pressed = true;
                i.down = true;
            }
            if f == 30 {
                i.released = true;
            }
            if f > 30 {
                i.down = true;
                i.pos = vec2(
                    view.w * (0.5 + 0.38 * (f as f32 * 0.045).sin()),
                    view.h - 60.0,
                );
            }
        }
        "Sky Jumper" => {
            if f == 1 {
                i.pressed = true;
            }
            if f > 5 {
                i.down = true;
                i.pos = vec2(
                    if (f / 40).is_multiple_of(2) {
                        100.0
                    } else {
                        view.w - 100.0
                    },
                    c.y,
                );
            }
        }
        "Math Pop" if f == 60 || f == 110 => {
            tap(&mut i, mathpop::balloon_rest(view, (f - 60) / 50));
        }
        "Count It" if f > 20 && f.is_multiple_of(25) => {
            let buttons = count::button_rects(view, 3);
            let area = count::play_area(view);
            let k = f / 25;
            if k.is_multiple_of(5) {
                tap(&mut i, buttons[(k / 5) % 3].center());
            } else {
                let g = (k % 5) as f32;
                tap(
                    &mut i,
                    vec2(
                        area.x + area.w * (0.15 + 0.2 * g),
                        area.y + area.h * (0.3 + 0.2 * (k % 2) as f32),
                    ),
                );
            }
        }
        "Hotel Hop" if f.is_multiple_of(45) => {
            let top = 80.0;
            let lobby_w = (view.w * 0.26).clamp(220.0, 320.0);
            let gx = 16.0 + lobby_w + 16.0;
            let cw = (view.w - gx - 16.0) / 4.0;
            let ch = (view.h - top - 16.0) / 3.0;
            let room = (f / 45) % 2;
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

fn spell_shot(raw: i32, view: &View) -> Option<Box<dyn Game>> {
    use games::spell::{Screen, Spell};
    if !(-23..=-20).contains(&raw) {
        return None;
    }
    kids_games::profile::create("Mia", 2);
    let key = games::spell::KEY;
    kids_games::profile::set(key, "played", 1);
    kids_games::profile::unlock(key, 5);
    for (level, stars) in [(0, 3), (1, 3), (2, 2), (3, 1), (4, 3)] {
        kids_games::profile::award_stars(key, level, stars);
    }
    kids_games::profile::set_level(key, 5);
    let mut g = Spell::new();
    match raw {
        -20 => g.screen = Screen::Levels,
        -21 => {
            g.start_level(4);
            g.screen = Screen::Done(1.0);
            g.finished = None;
        }
        -22 => {
            g.start_level(5);
        }
        _ => {
            kids_games::profile::unlock(key, 8);
            kids_games::profile::award_stars(key, 8, 3);
            kids_games::profile::submit(key, 3, kids_games::profile::Better::Higher);
            kids_games::profile::create("Leo", 5);
            kids_games::profile::submit(key, 1, kids_games::profile::Better::Higher);
            if let Some(mia) = kids_games::profile::list().first() {
                kids_games::profile::select(&mia.id);
            }
            g.start_level(8);
            g.rank = Some(0);
            g.screen = Screen::Final(1.0);
        }
    }
    for _ in 0..30 {
        g.update(&Input::default(), view, 1.0 / 60.0);
        tick_overlay(1.0 / 60.0);
    }
    Some(Box::new(g))
}

fn memory_clear(m: &mut Memory) {
    for id in 0..10u8 {
        let spots: Vec<usize> = (0..m.cards.len()).filter(|i| m.cards[*i].0 == id).collect();
        if spots.len() == 2 {
            m.pick(spots[0]);
            m.pick(spots[1]);
        }
    }
}

fn memory_shot(raw: i32, view: &View) -> Option<Memory> {
    let dt = 1.0 / 60.0;
    let mut m = match raw {
        10 | 110 | 210 | 310 | 410 | 510 => {
            kids_games::profile::create("Mia", 2);
            Memory::for_level(1)
        }
        _ => return None,
    };
    match raw {
        10 => {
            let rects = Memory::layout(view, m.pairs);
            for f in 0..90 {
                let mut i = Input::default();
                if f == 20 {
                    i = Input::press_at(rects[0].center());
                }
                m.update(&i, view, dt);
            }
        }
        110 => {
            kids_games::profile::submit(memory::KEY, 14, kids_games::profile::Better::Lower);
            kids_games::profile::create("Leo", 5);
            kids_games::profile::submit(memory::KEY, 18, kids_games::profile::Better::Lower);
            if let Some(mia) = kids_games::profile::list().first() {
                kids_games::profile::select(&mia.id);
            }
            m = Memory::for_level(memory::MAX_LEVEL);
            memory_clear(&mut m);
            for _ in 0..200 {
                m.update(&Input::default(), view, dt);
            }
        }
        210 => {
            m = Memory::for_level(3);
            m.pick(0);
            for _ in 0..30 {
                m.update(&Input::default(), view, dt);
            }
        }
        510 => {
            m = Memory::for_level(5);
            m.pick(0);
            for _ in 0..30 {
                m.update(&Input::default(), view, dt);
            }
        }
        310 => {
            m = Memory::for_level(2);
            memory_clear(&mut m);
            for _ in 0..200 {
                m.update(&Input::default(), view, dt);
            }
        }
        _ => {
            for level in 1..=2 {
                let mut done = Memory::for_level(level);
                memory_clear(&mut done);
            }
            m = Memory::for_level(3);
            m.open_select();
        }
    }
    Some(m)
}

fn learn_tiles_x(view: &View, n: usize) -> Vec2 {
    let l = kids_games::games::spell::layout(view, n, n);
    l.tiles[0].center()
}

fn word_grid(view: &View) {
    sky(
        view,
        Color::from_rgba(255, 236, 170, 255),
        Color::from_rgba(255, 190, 200, 255),
    );
    let words = kids_games::games::spell::WORDS;
    let cols = 6;
    let rows = words.len().div_ceil(cols);
    let cw = view.w / cols as f32;
    let ch = view.h / rows as f32;
    for (i, (w, draw)) in words.iter().enumerate() {
        let c = vec2(
            (i % cols) as f32 * cw + cw / 2.0,
            (i / cols) as f32 * ch + ch / 2.0 - 10.0,
        );
        round_rect(
            Rect::new(c.x - cw * 0.45, c.y - ch * 0.42, cw * 0.9, ch * 0.92),
            16.0,
            WHITE,
        );
        draw(&kids_games::games::spell::Pen {
            c,
            s: ch * 0.3,
            t: 0.0,
        });
        text_center(w, c.x, c.y + ch * 0.4, 22.0, BLACK);
    }
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

fn helix_scene(mode: &str, view: &View) -> Box<dyn Game> {
    use kids_games::games::helix::*;
    let mut g = Helix::new();
    let k = segment_at(ball_angle(), g.angle);
    match mode {
        "dead" => {
            g.floors[0].segs[k] = Seg::Danger;
            g.state = State::Playing;
            g.y = Helix::floor_y(0) - BALL_R - 5.0;
            g.vy = 400.0;
            g.score = 24;
        }
        "won" => {
            let n = g.floors.len();
            g.floors[n - 1].segs[k] = Seg::Gap;
            g.passed = n - 1;
            g.score = 60;
            g.bounces = 6;
            g.state = State::Playing;
            g.y = Helix::floor_y(n - 1) - BALL_R - 5.0;
            g.vy = 400.0;
        }
        "select" => g.open_select(),
        _ => {}
    }
    for _ in 0..150 {
        g.update(&Input::default(), view, 1.0 / 60.0);
        tick_overlay(1.0 / 60.0);
    }
    Box::new(g)
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
    let frames = if pick() == 102 { 760 } else { 420 };
    rand::srand(42);
    let which = pick();
    next_frame().await;
    let view = begin_frame_safe(fake_inset());
    let all = games::all();
    let memory = memory_shot(which, &view);
    let spell = spell_shot(which, &view);
    let has_spell = spell.is_some();
    let helix_mode = match which {
        212 => Some("dead"),
        312 => Some("won"),
        412 => Some("select"),
        512 => Some("start"),
        _ => None,
    };
    let which = if helix_mode.is_some() { 12 } else { which };
    let idle = which > 100;
    let which = if idle { which - 100 } else { which };
    if helix_mode.is_some() {
        kids_games::profile::create("Mia", 2);
        for l in 1..=7 {
            kids_games::profile::unlock("helix", l);
            kids_games::profile::award_stars("helix", l, (l % 3 + 1) as u8);
        }
        kids_games::profile::set_level("helix", 7);
    }
    if which == 7 {
        kids_games::profile::unlock("stack", 4);
        kids_games::profile::set_level("stack", 4);
        kids_games::profile::set("stack", "taught", 1);
    }
    let special: Option<Box<dyn Game>> = if which == -5 || which == -6 {
        kids_games::profile::create("Mia", 2);
        kids_games::profile::unlock("stack", 6);
        kids_games::profile::award_stars("stack", 1, 3);
        kids_games::profile::award_stars("stack", 2, 2);
        kids_games::profile::award_stars("stack", 3, 1);
        let mut g = games::stack::Stack::for_level(2);
        if which == -5 {
            for _ in 0..g.goal {
                g.moving.x = g.tower.last().unwrap().x;
                g.drop_block();
            }
        } else {
            g.picking = true;
        }
        for _ in 0..60 {
            g.update(&Input::default(), &view, 1.0 / 60.0);
            tick_overlay(1.0 / 60.0);
        }
        Some(Box::new(g))
    } else {
        None
    };
    let memory_game: Option<Box<dyn Game>> = memory.map(|m| Box::new(m) as Box<dyn Game>);
    let has_memory = memory_game.is_some();
    let mut game = special.or(spell).or(memory_game).or_else(|| {
        (which > 0).then(|| {
            let e = &all[which as usize - 1];
            set_accent(e.color);
            if idle && e.name == "Flappy Bird" {
                kids_games::profile::set("flappy", "passed", 1);
                for score in [14, 6] {
                    kids_games::profile::submit(
                        "flappy",
                        score,
                        kids_games::profile::Better::Higher,
                    );
                }
            }
            if idle && e.name == "Sky Jumper" {
                kids_games::profile::create("Mia", 2);
                kids_games::profile::set(e.icon_key, "runs", 5);
            }
            let mut g = (e.make)();
            if let Some(mode) = helix_mode {
                return helix_scene(mode, &view);
            }
            let frames = if idle && e.name == "Fruit Slice" {
                kids_games::profile::create("Mia", 2);
                kids_games::profile::set_level("fruit", 12);
                g = (e.make)();
                1500
            } else {
                frames
            };
            for f in 0..frames {
                let input = if idle && e.name == "Stack Tower" && (f == 1 || f == 5) {
                    Input::press_at(view.center())
                } else if idle && f > 1 && e.name == "Sky Jumper" {
                    Input {
                        right: f > 30,
                        ..Default::default()
                    }
                } else if idle && f > 1 && e.name == "Block Drop" {
                    if f % 3 == 0 {
                        Input::key(Dir::Down)
                    } else {
                        Input::default()
                    }
                } else if idle && f > 1 && e.name != "Paper Land" {
                    Input::default()
                } else if idle && e.name != "Paper Land" {
                    Input::press_at(view.center())
                } else {
                    scripted(e.name, f, &view)
                };
                g.update(&input, &view, 1.0 / 60.0);
                tick_overlay(1.0 / 60.0);
            }
            g
        })
    });
    if which != -3 && which != -4 && which != -5 && which != -6 && !has_memory && !has_spell {
        kids_games::profile::create("Mia", 2);
        if which > 100 {
            let g = &all[(which - 101) as usize];
            kids_games::profile::submit(g.icon_key, 3, kids_games::profile::Better::Higher);
            kids_games::profile::create("Leo", 5);
            kids_games::profile::submit(g.icon_key, 1, kids_games::profile::Better::Higher);
        }
    }
    if which == -4 {
        kids_games::profile::create("Mia", 2);
        kids_games::profile::create("Leo", 5);
        kids_games::profile::create("Ava", 9);
    }
    let mut app = App::new();
    if which == -4 {
        app.profiles = Some(kids_games::profiles::Profiles::new());
    }
    for _ in 0..90 {
        app.update(&Input::default(), &view, 1.0 / 60.0);
    }
    loop {
        let view = begin_frame_safe(fake_inset());
        match &mut game {
            Some(g) => {
                g.draw(&view);
                tick_overlay(0.25);
            }
            None if which == -2 => word_grid(&view),
            None if which == -3 || which == -4 => app.draw(&view),
            None if which < 0 => app_icon(&view),
            None => app.draw(&view),
        }
        next_frame().await;
    }
}
