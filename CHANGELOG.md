# Changelog

All notable changes to Learn and Play Kids Games are listed here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and versions follow
[Semantic Versioning](https://semver.org/).

## [Unreleased]

## [0.1.0] - 2026-11-08

### Added
- The games on Bevy 0.20 for Android, iOS, web and desktop, with the game logic in `kg_core` and the drawing in `kids_games`.
- Release builds from a pushed tag: Android arm64 APK, web bundle and desktop archives for Linux, macOS and Windows, with SHA-256 sums. `scripts/release.sh` builds the APK locally.
- The Android version name and version code come from the workspace version.

### Changed
- The game is licensed GPL-3.0-or-later, with the licences of the bundled art, sound and crates listed in `docs/LICENSES.md`.
- The hero, the trees and the runner props are built from gcad models when the game starts, and the `.glb` props come from `gcad export`.
