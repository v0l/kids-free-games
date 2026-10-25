---
name: tts-segments
description: >
  Generate spoken voice clips for the kids games (praise lines, prompts, numbers,
  spelling words) with the agentbench TTS server, check them, and pack them into
  assets/sound.kgs. Use when adding or changing anything the games say aloud.
---

# TTS segments

Every voice line in the games is one voice: **Harper** (agentbench, female American).
Don't mix voices or bring in stock voice packs.

## Generate

`scripts/tts.py` calls `POST $AGENTBENCH_URL/v1/audio/speech` (Kokoro on the agentbench host,
needs `AGENTBENCH_TOKEN`) and writes mono Ogg files into `sound/`:

```
python3 scripts/tts.py say_great.0='Great job!' say_great.1='Well done!' --proof /tmp/proof.wav
python3 scripts/tts.py --file /tmp/lines.txt --proof /tmp/proof.wav
```

A lines file holds one `name | text` per line. `GET $AGENTBENCH_URL/v1/audio/voices` lists
the other voices; use `--voice` only to audition them.

## Check

Open the proof WAV with `read_file`. It is transcribed by Whisper, and every phrase must come
back as written, in order. Re-roll any clip that is garbled, clipped or mis-stressed:
change the punctuation (`!` for excitement, `?` for "Ready?"), or spell a word out
phonetically. A transcript can't show tone, so ask the user to listen to new lines.

## Pack

1. Add one line per clip to `sound.txt`: `say_great.3 | repo:sound/say_great.3.ogg | db=-15`.
2. `python3 scripts/assets.py sound` rebuilds `assets/sound.kgs`.
3. `cargo test --lib sfx` checks every voice line is packed and praise has variety.
4. Commit `sound/*.ogg`, `sound.txt` and `assets/sound.kgs` together.

## Naming and playback

- `name.N` files are takes of one line. `sfx::say(Say::X)` plays a random take and never
  repeats the last one (`voice_line`, `pick_take` in `src/sfx.rs`).
- A new kind of line needs a `Say` variant and its key in `Say::key`.
- Words for a game (for example spelling words) go under their own prefix, such as
  `word_cat`, and are loaded by that game, not by `Say`.

## Writing the lines

- Children aged 3 to 8 are listening. Use short, warm words they know: "Great job!", "You did
  it!", "Oh no, try again!". No adult or gamer jargon ("objective achieved", "game over",
  "mission complete").
- Give praise and feedback at least 3 takes (more for lines heard every few seconds), so
  nothing repeats.
- Keep clips under about 2 s. Long ones step on the next prompt (`VOICE_GAP` is 0.9 s).
- Failure lines encourage. They never scold.
