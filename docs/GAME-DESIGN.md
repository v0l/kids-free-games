# Game design rules

Every game agent reads this before changing a game, along with the `game-feel`, `game-ui-ux` and `audio-design` skills. The players are children aged about 3 to 8, on a phone or tablet, often unable to read.

## Difficulty must be real

- Difficulty comes from the skill the game teaches or tests, not from clutter. Adding wrong options, more noise or less time is padding, not difficulty. Ask what the child has to know or do better on level 8 than on level 1.
- Learning games grade content by the skill, not by surface length. For spelling that means phonics bands (CVC, blends, digraphs, long vowels, irregular words). For maths it means number range and operation, carrying and borrowing, and missing-number forms.
- Arcade games ramp one variable at a time (speed, gap, hazard type) and introduce each new thing alone before mixing it with others.
- Take the scaffolding away step by step: glow on the answer, then glow after a pause, then no glow, then fewer letters or numbers given.

## Content volume

- A child plays the same game daily. If they meet the same item twice in a session, or the same set two days running, the game is broken. Size the pool for at least 30 sessions without a repeat.
- Track per player what they have seen and how they did, in `crate::profile`. Show new items first, bring back missed items after a gap (spaced repetition), and let mastered items appear rarely.
- Generated content beats a fixed list wherever it can be checked (numbers, layouts, levels). Fixed lists need hundreds of entries, not dozens.

## Adapt to the child

- Use a mastery estimate per skill band. Move up after a streak of clean answers and step back after repeated misses, inside a level as well as between levels.
- Never let a struggling child grind on a wall. Never let a strong child sit in the baby levels; offer a skip ahead after a perfect level.

## Pre-readers

- Every instruction is spoken, or shown by an animated hand, not written. Text is only a backup.
- Every learning item can be heard (word clips, number clips) and replayed by tapping it.

## Failure

- No harsh game over in learning games. A miss shows the right answer, keeps the child moving, and costs at most a star.
- Arcade game overs are short: one tap to retry, with the best score and progress shown.

## Goals and reward

- There is always a visible near goal (the next star, the next milestone) and a far one (the level map, the total stars).
- Reward effort and progress, and scale the celebration to the achievement (see the `game-feel` tiers).

## Touch

- Thresholds are in millimetres (`input.mm()`, `TAP_SLOP_MM`, `SWIPE_MM`), never view units.
- Actions trigger on touch-down when they can. Release-based taps need the slop check.
- Targets are at least 9 mm and spaced so a small finger doesn't hit the neighbour.

## Verify

- Write a test for each rule that applies: no repeats across N sessions, difficulty rising from level to level, a scripted weak player who still progresses, and a scripted strong player who isn't held back.
- Look at the screenshots in landscape and portrait before calling the work done.
