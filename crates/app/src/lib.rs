pub mod art;
pub mod audio;
pub mod draw;
pub mod game3d;
pub mod games;
pub mod jev;
pub mod link;
pub mod menu;
pub mod packs;
pub mod paint;
pub mod platform;
pub mod profiles;
pub mod shots;
pub mod text;
pub mod ui;
pub mod web;

use bevy::input::keyboard::{Key as BKey, KeyboardInput};
use bevy::input::touch::TouchPhase;
use bevy::input::ButtonState;
use bevy::prelude::*;
use bevy::window::{PrimaryWindow, WindowResolution};
use glam::vec2;
use kg_core::engine::{Finger, InputTracker, Key, Pointer, Raw, View};
use kg_core::safe::SafeArea;

pub enum Scene {
    Menu(Box<menu::App>),
    Shot(shots::Shot),
    Jev(Box<jev::Jev>),
    JevClash(Box<jev::JevClash>),
    BrainPaper(Box<jev::BrainPaper>),
    BrainClash(Box<jev::BrainClash>),
}

pub struct Frame {
    pub scene: Scene,
    demo: Option<u32>,
    tracker: InputTracker,
    safe: SafeArea,
    audio: Option<audio::Audio>,
    per_mm: Option<Option<f32>>,
    last_size: (f32, f32),
}

impl Frame {
    pub fn new(scene: Scene) -> Self {
        Self {
            scene,
            demo: web::query("demo").map(|_| 0),
            tracker: InputTracker::default(),
            safe: SafeArea::default(),
            audio: None,
            per_mm: None,
            last_size: (0.0, 0.0),
        }
    }

    pub fn play(&self) -> Option<&dyn games::Play> {
        match &self.scene {
            Scene::Menu(app) => app.current.as_ref().map(|(_, p)| &**p),
            Scene::Shot(s) => s.play(),
            Scene::Jev(_) => None,
            Scene::JevClash(j) => Some(&j.game),
            Scene::BrainPaper(j) => Some(&j.game),
            Scene::BrainClash(j) => Some(&j.game),
        }
    }

    pub fn play_mut(&mut self) -> Option<&mut dyn games::Play> {
        match &mut self.scene {
            Scene::Menu(app) => app
                .current
                .as_mut()
                .map(|(_, p)| &mut **p as &mut dyn games::Play),
            Scene::Shot(s) => s.play_mut(),
            Scene::Jev(_) => None,
            Scene::JevClash(j) => Some(&mut j.game),
            Scene::BrainPaper(j) => Some(&mut j.game),
            Scene::BrainClash(j) => Some(&mut j.game),
        }
    }
}

#[derive(Default)]
struct Gathered {
    touches: Vec<(u64, Finger, glam::Vec2)>,
    typed: Vec<char>,
    pressed: Vec<Key>,
}

fn map_key(k: KeyCode) -> Option<Key> {
    Some(match k {
        KeyCode::ArrowUp => Key::Up,
        KeyCode::ArrowDown => Key::Down,
        KeyCode::ArrowLeft => Key::Left,
        KeyCode::ArrowRight => Key::Right,
        KeyCode::KeyW => Key::W,
        KeyCode::KeyA => Key::A,
        KeyCode::KeyS => Key::S,
        KeyCode::KeyD => Key::D,
        KeyCode::Space => Key::Space,
        KeyCode::Enter | KeyCode::NumpadEnter => Key::Enter,
        KeyCode::Escape => Key::Escape,
        KeyCode::Backspace => Key::Backspace,
        _ => return None,
    })
}

const KEYS: [KeyCode; 13] = [
    KeyCode::ArrowUp,
    KeyCode::ArrowDown,
    KeyCode::ArrowLeft,
    KeyCode::ArrowRight,
    KeyCode::KeyW,
    KeyCode::KeyA,
    KeyCode::KeyS,
    KeyCode::KeyD,
    KeyCode::Space,
    KeyCode::Enter,
    KeyCode::NumpadEnter,
    KeyCode::Escape,
    KeyCode::Backspace,
];

#[allow(clippy::too_many_arguments)]
fn frame(
    mut state: NonSendMut<Frame>,
    mut pen: ResMut<paint::Pen>,
    time: Res<Time<Real>>,
    window: Query<&Window, With<PrimaryWindow>>,
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    mut touch_events: MessageReader<TouchInput>,
    mut key_events: MessageReader<KeyboardInput>,
    mut clear: ResMut<ClearColor>,
) {
    packs::pump();
    let Ok(win) = window.single() else { return };
    let dpi = win.scale_factor();
    let (lw, lh) = (win.width(), win.height());
    if lw < 2.0 || lh < 2.0 {
        return;
    }
    let state = &mut *state;
    let mut g = Gathered::default();
    let events: Vec<(u64, Finger, glam::Vec2)> = touch_events
        .read()
        .map(|t| {
            let f = match t.phase {
                TouchPhase::Started => Finger::Started,
                TouchPhase::Moved => Finger::Held,
                TouchPhase::Ended | TouchPhase::Canceled => Finger::Ended,
            };
            (t.id, f, t.position)
        })
        .collect();
    g.touches = events;
    for e in key_events.read() {
        if e.state != ButtonState::Pressed {
            continue;
        }
        if e.key_code == KeyCode::Backspace {
            g.typed.push(profiles::BACKSPACE);
        } else if let BKey::Character(s) = &e.logical_key {
            g.typed.extend(s.chars().filter(|c| !c.is_control()));
        }
    }
    let px_size = (lw * dpi, lh * dpi);
    if px_size != state.last_size {
        state.last_size = px_size;
    }
    let inset = state
        .safe
        .get(px_size, platform::insets_px)
        .scaled(1.0 / dpi);
    let view = View::fit_safe(lw, lh, inset);
    let per_mm = *state
        .per_mm
        .get_or_insert_with(|| platform::units_per_mm_px().map(|px| px / dpi));
    for k in KEYS {
        if keys.just_pressed(k) {
            g.pressed.extend(map_key(k));
        }
    }
    let raw = Raw {
        touches: g.touches,
        mouse: Pointer {
            pos: win.cursor_position().unwrap_or_default(),
            down: mouse.pressed(MouseButton::Left),
            pressed: mouse.just_pressed(MouseButton::Left),
            released: mouse.just_released(MouseButton::Left),
        },
        keys_down: KEYS
            .iter()
            .filter(|k| keys.pressed(**k))
            .filter_map(|k| map_key(*k))
            .collect(),
        keys_pressed: g.pressed,
        typed: g.typed,
        units_per_mm: per_mm,
        screen: vec2(lw, lh),
    };
    let mut raw = raw;
    if let Some(f) = &mut state.demo {
        *f += 1;
        demo_input(*f, &mut raw, &view);
    }
    let input = state.tracker.poll(&raw, &view);
    let dt = time.delta_secs().min(1.0 / 20.0);
    pen.begin(view, view.scale * dpi);
    match &mut state.scene {
        Scene::Menu(app) => {
            app.update(&input, &view, dt);
            app.draw(&mut pen, &view);
        }
        Scene::Shot(s) => s.frame(&mut pen, &view, dt),
        Scene::Jev(j) => j.frame(&mut pen, &view, dt),
        Scene::JevClash(j) => j.frame(&mut pen, &view, dt),
        Scene::BrainPaper(j) => j.frame(&mut pen, &view, dt, &input.typed),
        Scene::BrainClash(j) => j.frame(&mut pen, &view, dt, &input.typed),
    }
    let touched = input.pressed || input.action || !input.typed.is_empty();
    if let Some(a) = &mut state.audio {
        audio::tick(
            a,
            time.elapsed_secs_f64(),
            time.delta_secs().min(0.1),
            touched,
        );
    }
    let _ = &mut clear;
}

fn demo_input(f: u32, raw: &mut Raw, view: &View) {
    match f {
        20 => raw.typed.extend("Demo".chars()),
        30 => raw.keys_pressed.push(Key::Enter),
        60 => raw.keys_pressed.push(Key::Right),
        90 => raw.keys_pressed.push(Key::Enter),
        _ if f > 150 && f.is_multiple_of(22) => {
            let at = view.to_screen(view.center());
            raw.mouse = Pointer {
                pos: at,
                down: true,
                pressed: true,
                released: false,
            };
        }
        _ => {}
    }
}

fn asset_root() -> String {
    #[cfg(not(target_arch = "wasm32"))]
    if let Some(d) = kg_core::assets::dirs()
        .into_iter()
        .find(|d| d.join("models").is_dir())
    {
        return d
            .canonicalize()
            .unwrap_or(d)
            .to_string_lossy()
            .into_owned();
    }
    "assets".into()
}

pub fn window() -> Window {
    Window {
        title: "Kids Free Games".into(),
        canvas: Some("#bevy".into()),
        fit_canvas_to_parent: true,
        prevent_default_event_handling: true,
        resolution: WindowResolution::new(1024, 768),
        mode: if cfg!(any(target_os = "android", target_os = "ios")) {
            bevy::window::WindowMode::BorderlessFullscreen(bevy::window::MonitorSelection::Primary)
        } else {
            bevy::window::WindowMode::Windowed
        },
        prefers_home_indicator_hidden: true,
        prefers_status_bar_hidden: true,
        ..default()
    }
}

fn sync_on_close(
    mut closing: MessageReader<bevy::window::WindowCloseRequested>,
    mut life: MessageReader<bevy::window::AppLifecycle>,
) {
    if life
        .read()
        .any(|l| *l == bevy::window::AppLifecycle::WillSuspend)
    {
        kg_core::net::going_to_background();
    }
    if closing.read().count() > 0 {
        kg_core::net::flush_before_exit();
    }
}

pub fn build(scene: Scene, with_audio: bool) -> App {
    let mut app = App::new();
    app.add_plugins(
        DefaultPlugins
            .set(AssetPlugin {
                file_path: asset_root(),
                ..default()
            })
            .set(WindowPlugin {
                primary_window: Some(window()),
                ..default()
            })
            .set(bevy::log::LogPlugin {
                filter: "wgpu=error,naga=warn,bevy_render=warn".into(),
                ..default()
            }),
    )
    .insert_resource(ClearColor(Color::srgb(1.0, 0.7, 0.47)))
    .add_plugins(paint::PaintPlugin)
    .insert_non_send(Frame::new(scene))
    .add_systems(Update, frame.in_set(game3d::Step))
    .add_systems(
        Last,
        sync_on_close.before(bevy::window::close_when_requested),
    )
    .add_plugins(game3d::Game3dPlugin);
    if with_audio {
        app.add_systems(Startup, |mut f: NonSendMut<Frame>| {
            f.audio = Some(audio::Audio::new());
        });
    }
    app
}

pub fn run() {
    if let Some(d) = platform::save_dir() {
        kg_core::save::set_dir(d);
    }
    platform::install_asset_reader();
    art::predecode();
    packs::begin();
    text::prewarm();
    let scene = match shots::requested() {
        Some(n) => Scene::Shot(shots::Shot::new(n)),
        None if jev::requested() => Scene::Jev(Box::default()),
        None if jev::requested_clash() => Scene::JevClash(Box::default()),
        None if jev::requested_brain() => Scene::BrainPaper(Box::default()),
        None if jev::requested_brain_clash() => Scene::BrainClash(Box::default()),
        None => Scene::Menu(Box::default()),
    };
    let audio = !matches!(scene, Scene::Shot(_));
    build(scene, audio).run();
}

#[bevy_main]
pub fn main() {
    run();
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen(start)]
pub fn start() {
    run();
}
