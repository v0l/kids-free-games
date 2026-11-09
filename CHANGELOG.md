# Changelog

All notable changes to Learn and Play Kids Games are listed here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and versions follow
[Semantic Versioning](https://semver.org/).

## [Unreleased]

## [0.1.2] - 2026-11-09

### Added

- Helix ball and tower pole, and Paper Land heads and debris cubes, are `.gcad` files (`helix_*`, `paper_*`) built at startup.
- Happy Hotel models (guests, staff, swimmers, furniture, cars, coins) are `.gcad` files in `assets/models/`, built at startup by gcad-bevy and recoloured per guest by part name. A guest is 1696 triangles instead of 4952.
- The release workflow builds a signed Android App Bundle (`kids_games-<tag>-android.aab`), signed with the Play upload key from the `PLAY_UPLOAD_*` secrets.
- The macOS build ships a `KidsGames.app` in a `.dmg` next to the tar.gz.
- `scripts/release-apple.sh` builds the signed iOS IPA and the sandboxed Mac App Store `.pkg` on the Mac and copies them to `dist/`.

### Changed

- Removed the unused `Ball`, `Cylinder`, `Cone` and `Tube` shapes and their mesh builders from `game3d.rs`; every fixed model now comes from `.gcad`. Web wasm 3.9 KB smaller, Android arm64 APK 0.8 KB smaller, iOS simulator app unchanged.
- Every runner prop (barrier, hurdle, gate, train, coin, finish arch, road, five buildings) is built from its `.gcad` source by gcad-mesh at startup, like the hero and the trees. The 12 prebuilt GLBs, `art/3d/props/build.sh` and the glTF loader are gone; gcad is pinned to 888da75. The iOS app is 1.5 MB smaller, the Android APK 0.4 MB and the web bundle 2.0 MB.
- Models load through the `gcad-bevy` crate (v0l/gcad-engines 7d1fc40, gcad 75e25dd) instead of the game's own loader. `crates/app/src/gcad.rs` keeps only the fit settings and the startup warm-up. Desktop builds keep built models in `~/.cache/gcad/bevy`, so a second start has every model ready in about 25 ms instead of 485 ms. Web is 222 KB bigger, the APK 128 KB and the iOS app 160 KiB.

## [0.1.1] - 2026-11-07

### Changed
- The Android APK is signed with the same release key as agentbench, from the same four secrets (`SIGNING_KEYSTORE_BASE64`, `SIGNING_STORE_PASSWORD`, `SIGNING_KEY_PASSWORD`, `SIGNING_KEY_ALIAS`), and a build without them fails instead of falling back to the debug key.
- The release builds the arm64 APK only, named `kids_games-<tag>-android-arm64.apk`; the x86_64 emulator build is gone.

## [0.1.0] - 2026-11-07

### Added
- The games on Bevy 0.20 for Android, iOS, web and desktop, with the game logic in `kg_core` and the drawing in `kids_games`.
- Release builds from a pushed tag: Android arm64 APK, web bundle and desktop archives for Linux, macOS and Windows, with SHA-256 sums. `scripts/release.sh` builds the APK locally.
- The Android version name and version code come from the workspace version.

### Changed
- The game is licensed GPL-3.0-or-later, with the licences of the bundled art, sound and crates listed in `docs/LICENSES.md`.
- The hero, the trees and the runner props are built from gcad models when the game starts, and the `.glb` props come from `gcad export`.
