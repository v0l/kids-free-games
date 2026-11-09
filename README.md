# Learn and Play Kids Games

Ad-free versions of popular free mobile games, in one app, for iPad, iPhone, Android, the web and desktop. No ads, no purchases, no email or passwords. Progress and leaderboards sync through a small server (see [PRIVACY.md](PRIVACY.md)); the games also work offline.

Written in Rust on [Bevy](https://bevy.org) 0.20: `crates/core` holds the game logic, save, profiles and sync with no drawing, and `crates/app` is the Bevy app. `docs/BEVY.md` explains how a game is split between the two.

Art is painted with FLUX.1-dev. The 3D models (the runner hero, its clips, the trees and every prop) are `.gcad` text in `assets/models`, built into meshes when the game starts by the [gcad-bevy](https://github.com/v0l/gcad-engines) asset loader: edit the `.gcad` and run the game, there is no export step (`docs/ASSETS-3D.md`). Sound effects and voice lines are from [Kenney](https://kenney.nl) (CC0). Music is Carnival Rides, Summer Park and Happy Lullaby from [OpenGameArt](https://opengameart.org) (CC0). The font is Fredoka (SIL OFL). `docs/DESIGN.md` explains the process and `docs/STYLE.md` the rules. Sprite sheets and the sound pack are built into `assets/` by `scripts/assets.py` from the manifests in `art/` and `sound.txt`; see `art/README.md`.

See [GAMES.md](GAMES.md) for the list of popular games and which ones are done.

## Licence

GPL-3.0-or-later, see [LICENSE](LICENSE). Asset sources and their licences, and the app store question, are in [docs/LICENSES.md](docs/LICENSES.md).

## Privacy

No ads or third-party tracking. The app syncs a random player ID, name, avatar, saves, scores and play time to our own server. See [PRIVACY.md](PRIVACY.md).

## Games

Flappy Bird, Paper Land (Paper.io), Happy Hotel (hotel management), Snake, 2048, Fruit Slice, Stack Tower, Brick Breaker, Sky Jumper (Doodle Jump), Memory Match, Road Hopper (Crossy Road), Helix Drop (Helix Jump), Block Drop (Tetris), Math Pop, Count It and Spell It (learning games).

All of them work with touch (tap and swipe), mouse and keyboard (arrows or WASD, space), in portrait and landscape. The house button top left goes back to the menu.

## Layout

- `crates/core`: game logic, save, profiles, leaderboards, mastery, sync, the audio mixer and text outlining, unit tested without a window.
- `crates/app`: the Bevy app. `paint.rs` and `draw.rs` draw shapes, sprites and outlined text; `ui.rs` is the painted UI kit; `menu.rs` is the game picker; `games/` has one renderer per ported game.
- `scripts/shots.sh`: render the numbered scenes in `crates/app/src/shots.rs` in headless Chrome for a screenshot sheet.

To add a game, follow `docs/BEVY.md`.

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

## Releasing

A release is a pushed tag. `.github/workflows/release.yml` builds the signed Android arm64 APK, the web bundle and desktop archives for Linux, macOS and Windows from the committed assets, then publishes a GitHub release with `SHA256SUMS` and the CHANGELOG section as the notes. Running the workflow by hand (`gh workflow run release.yml -f tag=v0.1.1`) builds everything and publishes nothing.

1. Set the version in `Cargo.toml` (`[workspace.package]`) and move the Unreleased entries in `CHANGELOG.md` under a `## [x.y.z] - date` heading. The Android version name is that version and the version code is `major*10000 + minor*100 + patch`.
2. `git tag vX.Y.Z && git push origin vX.Y.Z`. The check job fails the run if the tag differs from the workspace version or the CHANGELOG has no entry.

The APK is signed with the same release key as agentbench, from four repository secrets that use the same names: `SIGNING_KEYSTORE_BASE64`, `SIGNING_STORE_PASSWORD`, `SIGNING_KEY_PASSWORD` and `SIGNING_KEY_ALIAS`. A run without them fails. To build the APK on your own machine:

```sh
KEYSTORE=~/.keystore KEYSTORE_PASS=... KEY_ALIAS=upload scripts/release.sh
```

The same Android job also builds the App Bundle for Google Play, `kids_games-<tag>-android.aab`. Play only accepts a bundle signed with the upload key registered for the app, so it uses a second secret set, `PLAY_UPLOAD_KEYSTORE_BASE64`, `PLAY_UPLOAD_STORE_PASSWORD`, `PLAY_UPLOAD_KEY_PASSWORD` and `PLAY_UPLOAD_KEY_ALIAS`, made from the upload keystore (`~/kgdist/upload.jks`). A tag build fails without them; a manual dry run falls back to the APK key. Set them with `base64 -w0 ~/kgdist/upload.jks | gh secret set PLAY_UPLOAD_KEYSTORE_BASE64 --repo v0l/kids-free-games` and `gh secret set` for the other three.

The macOS job ships `kids_games-<tag>-aarch64-apple-darwin.dmg` (a `KidsGames.app` with the icon, assets and the version in `Info.plist`, ad-hoc signed) beside the tar.gz. There is no Developer ID Application certificate, so it cannot be notarised: Gatekeeper asks the user to right-click and open it the first time.

### Apple store builds

CI cannot sign for Apple. From the desktop, with the Mac reachable (`ssh mac`, unlocked after a reboot, see the FileVault note in the Mac dev server memory):

```sh
scripts/release-apple.sh            # build and copy to dist/
scripts/release-apple.sh --attach   # also gh release upload --clobber to the existing tag
```

It syncs the repo to `mac:git/kids-free-games`, runs `scripts/ios-appstore.sh` (Apple Distribution identity and the "Kids Free Games App Store" profile) and `scripts/macos-pkg.sh` (sandboxed app signed with Apple Distribution, packaged with the 3rd Party Mac Developer Installer identity), then writes `dist/kids_games-<tag>-ios.ipa`, `dist/kids_games-<tag>-macos.pkg` and a SHA-256 file, and checks the IPA's version against Cargo.toml. The build number is `major*10000 + minor*100 + patch`.

The Mac package needs a Mac App Store provisioning profile for `io.v0l.kidsgames` (Mac App Store Connect type, created in the developer portal); until one is installed the `.pkg` is signed but App Store Connect will reject it. Nothing is uploaded by the scripts. The manual last steps:

```sh
ssh mac 'ASC_PW=$(cat ~/kgdist/asc_app_password) xcrun altool --upload-app -f ~/git/kids-free-games/target/ios/KidsGames.ipa -t ios -u $(cat ~/kgdist/asc_user) -p @env:ASC_PW'
ssh mac 'ASC_PW=$(cat ~/kgdist/asc_app_password) xcrun altool --upload-app -f ~/git/kids-free-games/target/macos/KidsGames.pkg -t macos -u $(cat ~/kgdist/asc_user) -p @env:ASC_PW'
```

The AAB is uploaded by hand in the Play Console (Early access track).

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
