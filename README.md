# Kids Free Games

Ad-free versions of popular free mobile games, in one app, for iPad, iPhone, Android, the web and desktop. No ads, no purchases, no accounts, no network.

Written in Rust. This `bevy` branch moves the app to [Bevy](https://bevy.org) 0.20: `crates/core` holds the game logic, save, profiles and sync with no drawing, and `crates/app` is the Bevy app. Flappy Bird is ported; the other games are in `legacy/` until they move over. `docs/BEVY.md` is the porting guide.

Art is painted with FLUX.1-dev. Sound effects and voice lines are from [Kenney](https://kenney.nl) (CC0). Music is Carnival Rides, Summer Park and Happy Lullaby from [OpenGameArt](https://opengameart.org) (CC0). The font is Fredoka (SIL OFL). `docs/DESIGN.md` explains the process and `docs/STYLE.md` the rules. Sprite sheets and the sound pack are built into `assets/` by `scripts/assets.py` from the manifests in `art/` and `sound.txt`; see `art/README.md`.

See [GAMES.md](GAMES.md) for the list of popular games and which ones are done.

## Games

Flappy Bird, Paper Land (Paper.io), Hotel Hop (hotel management), Snake, 2048, Fruit Slice, Stack Tower, Brick Breaker, Sky Jumper (Doodle Jump), Memory Match, Road Hopper (Crossy Road), Helix Drop (Helix Jump), Block Drop (Tetris), Math Pop, Count It and Spell It (learning games).

All of them work with touch (tap and swipe), mouse and keyboard (arrows or WASD, space), in portrait and landscape. The house button top left goes back to the menu.

## Layout

- `crates/core`: game logic, save, profiles, leaderboards, mastery, sync, the audio mixer and text outlining, unit tested without a window.
- `crates/app`: the Bevy app. `paint.rs` and `draw.rs` draw shapes, sprites and outlined text; `ui.rs` is the painted UI kit; `menu.rs` is the game picker; `games/` has one renderer per ported game.
- `scripts/shots.sh`: render the numbered scenes in `crates/app/src/shots.rs` in headless Chrome for a screenshot sheet.

To port a game, follow `docs/BEVY.md`.

## Run

```sh
cargo run --release -p kids_games
cargo test --workspace
```

## Android

Needs the Android SDK and NDK 28 (`~/Android/Sdk`), cargo-ndk and Java 21. Builds an arm64 APK signed with the debug key (`ABI=x86_64` for the emulator):

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

`scripts/deploy-web.sh` publishes to the Cloudflare Pages branch `bevy` (https://bevy.kids-free-games.pages.dev).

Open it on the iPad and use Share > Add to Home Screen to get a full-screen app icon with no App Store needed.
