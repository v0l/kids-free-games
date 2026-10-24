# Kids Free Games

Ad-free versions of popular free mobile games, in one app, for iPad, iPhone, Android, the web and desktop. No ads, no purchases, no accounts, no network.

Written in Rust with [macroquad](https://macroquad.rs), so one codebase builds for every platform. Everything is drawn in code; the only asset is the Fredoka font (`assets/`, SIL OFL).

See [GAMES.md](GAMES.md) for the list of popular games and which ones are done.

## Games

Flappy Bird, Paper Land (Paper.io), Hotel Hop (hotel management), Snake, 2048, Fruit Slice, Stack Tower, Brick Breaker, Sky Jumper (Doodle Jump), Memory Match, Road Hopper (Crossy Road), Helix Drop (Helix Jump), Block Drop (Tetris), Math Pop, Count It and Spell It (learning games).

All of them work with touch (tap and swipe), mouse and keyboard (arrows or WASD, space), in portrait and landscape. The house button top left goes back to the menu.

## Layout

- `src/fx.rs`: particles, screen shake, popups, easing, animated panels.
- `src/engine.rs`: the shared layer every game uses. Resolution-independent `View`, `Input` (tap, swipe, keys), text, buttons and drawing helpers.
- `src/games/*.rs`: one file per game, each a `Game` with `update` and `draw`. Game logic is plain Rust, unit tested without a window.
- `src/menu.rs`: the game picker.
- `examples/shots.rs` and `scripts/shots.sh`: render every game in headless Chrome for a screenshot sheet.

To add a game, write `src/games/<name>.rs` with a `Game` impl and an `icon`, then add an `Entry` in `src/games/mod.rs`.

## Run

```sh
cargo run --release
cargo test
```

## Android

Needs Docker. Builds a signed debug APK for arm64, armv7 and x86_64:

```sh
scripts/android.sh
adb install -r target/android-artifacts/kids_games.apk
```

To put it on a phone without adb, copy the APK across and open it (allow installs from unknown sources).

## iPad and iPhone

Build on a Mac with Xcode and the Rust iOS targets (`rustup target add aarch64-apple-ios aarch64-apple-ios-sim`).

Simulator:

```sh
scripts/ios.sh
xcrun simctl install booted target/ios/KidsGames.app
xcrun simctl launch booted io.v0l.kidsgames
```

A real device needs an Apple developer signing identity and a provisioning profile:

```sh
TARGET=aarch64-apple-ios SIGN_IDENTITY="Apple Development: ..." BUNDLE_ID=your.bundle.id scripts/ios.sh
cp path/to/profile.mobileprovision target/ios/KidsGames.app/embedded.mobileprovision
codesign --force --sign "Apple Development: ..." target/ios/KidsGames.app
xcrun devicectl device install app --device <id> target/ios/KidsGames.app
```

## Web (any tablet browser)

```sh
scripts/web.sh
python3 -m http.server -d dist 8080
```

Open it on the iPad and use Share > Add to Home Screen to get a full-screen app icon with no App Store needed.
