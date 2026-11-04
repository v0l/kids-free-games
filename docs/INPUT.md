# Input

All games read one `Input` per frame (`kg_core::input`, re-exported from `kg_core::engine`). Only `crates/app/src/input.rs` touches Bevy input; a test fails if a game calls the platform directly.

## Pipeline

`app/src/input.rs` gathers touches, mouse, keys (arrows, WASD, numpad), gamepad, wheel, focus loss and cursor leaving into a `Raw`. `InputTracker::poll` turns it into `Input`:

- `pos`, `prev_pos`, `start` in view units (safe area applied), `down`, `pressed`, `released`, `travel`, `hold` seconds.
- First finger owns the pointer, a second finger never steals it. `pointers` lists every finger.
- `swipe` (one per direction change, threshold in mm), `key_dir` (arrow pressed this frame), `axis` (keys, numpad or pad as a unit-limited `Vec2`), `action`, `back`, `typed`, `scroll`, `pinch`.
- `cancelled`: focus lost, touch cancelled or cursor left while held. It ends the press without a tap or swipe.
- `mm()`: view units per millimetre. Size every touch target and dead zone in mm.

Helpers: `tap()`, `tapped_in(rect)`, `tap_released()`, `dragging()`, `holding()`, `drag_delta()`, `aim(from, dead_mm)`, `kinds()` (the `InputKind`s active this frame), `direction()`.

## Direction

`Direction` wraps a `Vec2` of length at most 1 with `four()`, `eight()`, `angle()`, `unit()`. `DirControl::new(mode)` is the one direction control, `read(input, centre, dt)` returns a `Direction`. Keys and pad always win. Modes:

| Mode | Behaviour | Used by |
|---|---|---|
| `Stick` | floating stick, 9 mm radius, 12% dead zone, origin follows the finger | Hotel |
| `Steer` | aim at the finger from `centre`, 2.5 mm dead zone | Paper, Clash |
| `Swipe` | one unit direction per swipe | grid games |
| `Keys` | keys and pad only | any |

`smoothed(rate)` adds exponential smoothing. Games that only need a left/right axis read `input.axis.x`; tap games read `input.tap()`.

## Tests and replay

`Script::new(view)` builds `Input` frames without a device: `tap`, `press`, `drag_to`, `release`, `swipe`, `hold`, `keys`, `pad`, `cancel`, then `Script::feed(&mut game, &view, &frames)`. Raw frames can also be fed to an `InputTracker` directly (the jev and brain harnesses build `Input` values the same way).

## Adding a game

1. Take `&Input` in `update`. Never import Bevy input types.
2. Pick the control: `input.tap()` or `tapped_in`, `input.axis` or a `DirControl`, `input.swipe`.
3. Size things in mm via `input.mm()`.
4. Add a test in `crates/core/src/games/input_tests.rs` that drives the game through `Script`.

Custom mappings that stay in the game: Fruit's blade (`prev_pos` to `pos` segments), Paper's free-angle steering (`aim` plus ground projection), Blocks' drag-to-column. They use the shared pointer fields only.
