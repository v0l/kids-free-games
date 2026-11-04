use bevy::ecs::system::SystemParam;
use bevy::input::keyboard::{Key as BKey, KeyboardInput};
use bevy::input::mouse::{MouseScrollUnit, MouseWheel};
use bevy::input::touch::{TouchInput, TouchPhase};
use bevy::input::ButtonState;
use bevy::prelude::*;
use bevy::window::{CursorLeft, WindowFocused};
use glam::{vec2, Vec2};
use kg_core::engine::{Finger, Key, Pointer, Raw};

const KEYS: [(KeyCode, Key); 20] = [
    (KeyCode::ArrowUp, Key::Up),
    (KeyCode::ArrowDown, Key::Down),
    (KeyCode::ArrowLeft, Key::Left),
    (KeyCode::ArrowRight, Key::Right),
    (KeyCode::KeyW, Key::W),
    (KeyCode::KeyA, Key::A),
    (KeyCode::KeyS, Key::S),
    (KeyCode::KeyD, Key::D),
    (KeyCode::Numpad8, Key::Up),
    (KeyCode::Numpad2, Key::Down),
    (KeyCode::Numpad4, Key::Left),
    (KeyCode::Numpad6, Key::Right),
    (KeyCode::Numpad7, Key::NumUpLeft),
    (KeyCode::Numpad9, Key::NumUpRight),
    (KeyCode::Numpad1, Key::NumDownLeft),
    (KeyCode::Numpad3, Key::NumDownRight),
    (KeyCode::Space, Key::Space),
    (KeyCode::Enter, Key::Enter),
    (KeyCode::NumpadEnter, Key::Enter),
    (KeyCode::Escape, Key::Escape),
];

#[derive(SystemParam)]
pub struct Devices<'w, 's> {
    keys: Res<'w, ButtonInput<KeyCode>>,
    mouse: Res<'w, ButtonInput<MouseButton>>,
    touch: MessageReader<'w, 's, TouchInput>,
    typed: MessageReader<'w, 's, KeyboardInput>,
    wheel: MessageReader<'w, 's, MouseWheel>,
    focus: MessageReader<'w, 's, WindowFocused>,
    left: MessageReader<'w, 's, CursorLeft>,
    pads: Query<'w, 's, &'static Gamepad>,
}

#[derive(Default)]
pub struct Gather {
    cursor: Vec2,
}

impl Gather {
    pub fn raw(&mut self, d: &mut Devices, win: &Window, per_mm: Option<f32>, dt: f32) -> Raw {
        let mut cancel = false;
        let mut touches = Vec::new();
        for t in d.touch.read() {
            let f = match t.phase {
                TouchPhase::Started => Finger::Started,
                TouchPhase::Moved => Finger::Held,
                TouchPhase::Ended => Finger::Ended,
                TouchPhase::Canceled => {
                    cancel = true;
                    Finger::Ended
                }
            };
            touches.push((t.id, f, t.position));
        }
        let mut typed = Vec::new();
        for e in d.typed.read() {
            if e.state != ButtonState::Pressed {
                continue;
            }
            if e.key_code == KeyCode::Backspace {
                typed.push(crate::profiles::BACKSPACE);
            } else if let BKey::Character(s) = &e.logical_key {
                typed.extend(s.chars().filter(|c| !c.is_control()));
            }
        }
        let mut scroll = 0.0;
        for w in d.wheel.read() {
            scroll += match w.unit {
                MouseScrollUnit::Line => w.y * 40.0,
                MouseScrollUnit::Pixel => w.y,
            };
        }
        if d.focus.read().any(|f| !f.focused) {
            cancel = true;
        }
        let mouse_down = d.mouse.pressed(MouseButton::Left);
        if d.left.read().count() > 0 && mouse_down {
            cancel = true;
        }
        if let Some(p) = win.cursor_position() {
            self.cursor = p;
        }
        let mut keys_down: Vec<Key> = KEYS
            .iter()
            .filter(|(k, _)| d.keys.pressed(*k))
            .map(|(_, k)| *k)
            .collect();
        let mut keys_pressed: Vec<Key> = KEYS
            .iter()
            .filter(|(k, _)| d.keys.just_pressed(*k))
            .map(|(_, k)| *k)
            .collect();
        if d.keys.just_pressed(KeyCode::Backspace) {
            keys_pressed.push(Key::Backspace);
        }
        keys_down.dedup();
        let pad = d
            .pads
            .iter()
            .map(|g| {
                let s = g.left_stick();
                let dp = g.dpad();
                let v = if dp != Vec2::ZERO { dp } else { s };
                vec2(v.x, -v.y)
            })
            .find(|v| *v != Vec2::ZERO)
            .unwrap_or(Vec2::ZERO);
        Raw {
            touches,
            mouse: Pointer {
                pos: self.cursor,
                down: mouse_down,
                pressed: d.mouse.just_pressed(MouseButton::Left),
                released: d.mouse.just_released(MouseButton::Left),
            },
            keys_down,
            keys_pressed,
            typed,
            units_per_mm: per_mm,
            screen: vec2(win.width(), win.height()),
            pad,
            cancel,
            scroll,
            pinch: 0.0,
            dt,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    const PLATFORM_CALLS: [&str; 10] = [
        "ButtonInput",
        "MouseButton",
        "KeyCode",
        "TouchInput",
        "Gamepad",
        "cursor_position",
        "KeyboardInput",
        "MouseWheel",
        "CursorMoved",
        "WindowFocused",
    ];

    const ALLOWED: [&str; 1] = ["input.rs"];

    fn rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
        for e in std::fs::read_dir(dir).unwrap().flatten() {
            let p = e.path();
            if p.is_dir() {
                rust_files(&p, out);
            } else if p.extension().is_some_and(|x| x == "rs") {
                out.push(p);
            }
        }
    }

    fn offenders(root: &Path) -> Vec<String> {
        let mut files = Vec::new();
        rust_files(root, &mut files);
        let mut bad = Vec::new();
        for f in files {
            let name = f.file_name().unwrap().to_string_lossy().to_string();
            if f.parent() == Some(root) && ALLOWED.contains(&name.as_str()) {
                continue;
            }
            let text = std::fs::read_to_string(&f).unwrap();
            for (i, line) in text.lines().enumerate() {
                if PLATFORM_CALLS.iter().any(|c| line.contains(c)) {
                    bad.push(format!("{}:{}: {}", f.display(), i + 1, line.trim()));
                }
            }
        }
        bad
    }

    #[test]
    fn games_never_call_the_platform_for_pointer_or_key_input() {
        let app = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let core = Path::new(env!("CARGO_MANIFEST_DIR")).join("../core/src");
        let mut bad = offenders(&app.join("games"));
        bad.extend(offenders(&core.join("games")));
        assert!(bad.is_empty(), "direct platform input:\n{}", bad.join("\n"));
    }

    #[test]
    fn the_only_bevy_input_reader_is_the_input_module() {
        let app = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let bad: Vec<String> = offenders(&app)
            .into_iter()
            .filter(|l| !l.contains("/src/input.rs"))
            .collect();
        assert!(bad.is_empty(), "direct platform input:\n{}", bad.join("\n"));
    }

    #[test]
    fn the_scan_would_catch_a_violation() {
        let dir = std::env::temp_dir().join("kg_input_scan");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("bad.rs"),
            "fn f(k: Res<ButtonInput<KeyCode>>) {}\n",
        )
        .unwrap();
        assert_eq!(offenders(&dir).len(), 1);
    }
}
