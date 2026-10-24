# Art and sound manifests

Each `art/<sheet>.txt` lists sprites as `name | kenney-pack/path.png | options`.
`scripts/assets.py art` downloads the Kenney packs into `~/.cache/kids-games/kenney`,
trims, scales and packs each manifest into `assets/art/<sheet>.kga`.
`sound.txt` in the repo root does the same for `assets/sound.kgs`.

Options: `max=N` longest side in px, `solo` own texture (for tiling backgrounds),
`keep` keep transparent margins, `flip` mirror, `crop=x,y,w,h`.

In code: `crate::sprites!("<sheet>");` inside a module gives `sprite`, `sprite_ex`,
`sprite_at` (by height), `sprite_w` (by width), `nine` (9-slice), `size` and `names`.

All art and sound effects are Kenney (www.kenney.nl), CC0.
Music: Carnival Rides, Summer Park and Happy Lullaby from opengameart.org, CC0.
