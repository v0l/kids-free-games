# Licences

Learn and Play Kids Games is free software under the GNU General Public License, version 3 or later (`LICENSE`). The source is at https://github.com/v0l/kids-free-games. The app says so on the parent-only player edit screen (`profiles::SOURCE_NOTE`).

## Rust dependencies

Checked with `cargo metadata` for x86_64 Linux, aarch64 Android, aarch64 iOS and wasm32 (366 to 440 crates per target, 427 on Linux). Every crate is under a permissive licence that GPL-3.0 accepts: MIT, Apache-2.0 (alone or dual), BSD-2/3-Clause, ISC, Zlib, 0BSD, MIT-0, Unlicense, CC0-1.0, Unicode-3.0 and CDLA-Permissive-2.0 (the last two are data licences for Unicode tables and webpki roots). No crate is GPL, LGPL, MPL, EPL or unlicensed. `vendor/wgpu-hal` is a patched copy of wgpu-hal, MIT OR Apache-2.0.

`gcad-mesh` (v0l/gcad, GPL-3.0-or-later) builds the 3D models at startup; it is GPL-3.0 like the game.

Re-check after dependency changes:

```sh
cargo metadata --format-version 1 --filter-platform aarch64-linux-android | python3 -c 'import json,sys; d=json.load(sys.stdin); print(sorted({p.get("license") for p in d["packages"]}))'
```

## Assets

| Asset | Source | Licence | Redistributable under GPL? |
|---|---|---|---|
| Sprites, backgrounds, icons (`assets/art/*.kga`, from `art/gen`) | Generated in-house with FLUX.1-dev (Black Forest Labs) | FLUX.1 [dev] Non-Commercial License; it claims no rights in outputs and allows outputs for any purpose including commercial, with two duties: review outputs for unlawful content before distribution, and disclose AI generation where the law requires it | Yes as our own work, but **check**: the FLUX.1-dev licence itself is non-commercial for the model; output use is allowed by its output clause. No AI-generated disclosure exists in the app or store listings yet |
| A few UI icons and sound effects (`art/*.txt` and `sound.txt` entries with a Kenney pack path) | kenney.nl packs: interface-sounds, impact-sounds, new-platformer-pack, music-jingles, rpg-audio, casino-audio, ui-audio, sci-fi-sounds, ui-pack, voiceover | CC0 1.0 | Yes |
| Music: Carnival Rides, Summer Park, Happy Lullaby | opengameart.org (URLs in `sound.txt`) | CC0 1.0 | Yes |
| Voice lines in `sound/` (`say_*.ogg`, praise lines, Paper Land lines) | Generated with Maya1 (`maya-research/maya1`) since commit 6f83f4f; earlier takes were Kokoro-82M (voice Harper) | Maya1 Apache-2.0; Kokoro-82M Apache-2.0 | Yes |
| Spell It words (`assets/sound_words.kgs`) | Generated with Maya1 (`maya-research/maya1`, `scripts/spell_words.sh`) | Apache-2.0 | Yes |
| Font Fredoka SemiBold (`assets/Fredoka-SemiBold.ttf`) | Google Fonts | SIL Open Font License 1.1 (`assets/OFL.txt`) | Yes; OFL fonts may ship with GPL software, the font keeps its own licence |
| Bot brains (`assets/brains/*.kgb`) | Trained in-house (`crates/core/src/brain`, `tools/rl`) | Our own work | Yes |
| Hero, props, road, buildings (`assets/models/*.gcad`) | Modelled in-house in gcad | Our own work | Yes |
| Hero animation clips (`art/3d/hero/motion.gcad`, from `art/3d/motion/bvh/*.bvh`) | Generated with NVIDIA Kimodo (`kimodo-soma-rp-v1.2`, code Apache-2.0, model NVIDIA Open Model License), retargeted by `art/3d/hero/retarget.py` | The model licence says NVIDIA claims no ownership of outputs; no notice is required for shipping outputs (only for redistributing the model). Not Mixamo or captured data | Yes |
| Old Hunyuan3D-2 hero and props (`git log` before 2026-11-06) | Tencent Hunyuan 3D 2.0 Community License: excludes the EU, UK and South Korea, needs a NOTICE file for distribution | Not shipped any more: every model was rebuilt in gcad. `art/3d/props.py`, `art/3d/prop_build.py` and `art/3d/hero/build.py` are the old pipeline, kept for history | n/a |

Things for the owner to decide (not resolved here):

- FLUX.1-dev: the art is used under the output clause of a non-commercial model licence. That is generally read as allowing commercial use of outputs, but it is Black Forest Labs' licence to interpret; a FLUX.1 [dev] commercial licence or FLUX.1 [schnell] (Apache-2.0) removes the doubt. The licence also asks for AI-generation disclosure where the law requires it (for example the EU AI Act's transparency rules for synthetic content).
- The repo history still contains Hunyuan3D outputs (old `hero.glb` and `prop_*.glb`). Publishing the git history publishes them; the Hunyuan licence excludes the EU, UK and South Korea and asks for a NOTICE file.

## App stores and the GPL

Shipping GPL-3.0 code through the stores has a known tension. This is for the owner to decide; nothing here changes store distribution.

- Apple App Store: the App Store terms (usage rules limiting installs to the buyer's devices, DRM, no further distribution) add restrictions the GPL forbids (GPLv3 section 10, "no further restrictions"). The FSF's position (2010, the GNU Go case) is that the App Store terms are incompatible with the GPL. VLC was pulled from the App Store in 2011 after a VLC copyright holder complained, and came back in 2013 after VideoLAN relicensed libVLC to LGPL-2.1 and MPL-2.0. A sole copyright holder can still ship their own GPL code on the App Store (they can grant themselves anything), but every outside contributor's code would need their permission. Today every dependency is permissive, so the only GPL code is this repo and gcad, both owned by the same person.
- Google Play: its terms do not add usage restrictions on the app itself, and many GPL apps ship there (with the source offered). Corresponding source must be available for each shipped build (GPLv3 section 6), which the public repo does if the shipped commit is tagged or noted.
- Accepting outside contributions: if the App Store build stays, contributors should agree (a CLA or a note in CONTRIBUTING) that their code may be distributed through the stores, or the owner should keep the iOS build to their own code.
