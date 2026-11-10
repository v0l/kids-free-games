# UI sizing

Every modal, menu and overlay sizes itself from the safe area (`view.w`, `view.h`) and from physical millimetres (`view.mm`, view units per mm). The rule lives in `crates/core/src/sizing.rs`.

## The rule

| Constant | Value | Use |
| --- | --- | --- |
| `TOUCH_MM` | 12 mm | Smallest side of any button, tile or icon a child taps (about 48 dp). |
| `KEY_W_MM` | 7.6 mm | Smallest width of a keyboard or code-pad key. Keys are still 12 mm tall. |
| `KEY_GAP_MM` | 1.5 mm | Smallest gap between two keys. |
| `KID_MM` | 15 mm | Preferred size for the main kid targets where there is room. |
| `TEXT_MM` | 4.2 mm | Body text height floor; `view.text_k()` scales design sizes up to reach it. |

- `view.tap(design)` and `view.kid_tap(design)` give a height that is never under 12 mm (or 15 mm for kids where it fits).
- `view.sheet_w(design)` makes a panel take at least 92% of the width in portrait, and the design width times `text_k` in landscape.
- `view.home()`, `view.top_right(i)`, `view.hud_left()`, `view.hud_top()` place the corner buttons.
- Text sizes go through `font_params`, so they stay in the `GLYPH_SIZES` buckets. `check_text` fails a layout whose text would pass the largest bucket.

## Building a screen

Use a `Stack` for any panel. It shrinks its design scale until it fits the safe area, wraps text, puts buttons side by side when their labels fit and stacks them when not, and never lets a button drop under 12 mm.

```rust
let mut s = kg_core::sizing::Stack::new(view, 580.0).ribbon("Welcome!");
let line = s.text(26.0, "Make a player or link this device to another one.");
let b = s.buttons(84.0, &["Make a player", "Link to another device"]);
let sheet = s.layout();
let (make, link) = (sheet.button(b, 0), sheet.button(b, 1));
```

Draw it with `pen.painted_panel(sheet.panel, WHITE)`, `pen.ribbon_in(view, title, sheet.ribbon.unwrap().0, t)`, `pen.sheet_text(&sheet, line, None, BODY)` and `pen.painted_button(make, ...)`.

Other helpers: `title_bar` (Back / title / action row), `label_button` (a button sized to its label), `bottom_button`, `bottom_row`, `corner_button`, `grid`, `keypad` (keys at 12 mm by 7.6 mm, adds columns as the width allows), `picker_layout` (level pickers), `done_layout` / `stars_sheet` (level done panels), `results_layout` / `results_for` (game over with the board).

## Tests

`crates/core/src/sizing.rs` and `crates/core/src/games/sizing_tests.rs` check every layout at five views: phone portrait (390x844 at 3x, 460 ppi), phone landscape, tablet portrait (820x1180 at 2x, 264 ppi), tablet landscape and desktop (1920x1080, 96 ppi). The checkers are `check_buttons`, `check_keys`, `check_apart`, `check_inside`, `check_sheet` and `check_sheet_text`. App screens are checked in `menu.rs`, `profiles.rs` and `link.rs`. A new screen adds its rects to one of these tests.

## Inventory

| Screen | Where | Layout |
| --- | --- | --- |
| Main menu header, Home, music, mute, section labels, tiles | `app/src/menu.rs` | `view.home`, `view.top_right`, `label_h`, `TILE_MM` |
| Lock sheet | `app/src/menu.rs` `lock_rects` | `Stack` |
| Report a player sheet | `app/src/menu.rs` `report_rects` | `Stack` |
| Who's playing (cards, gear, Link device) | `app/src/profiles.rs` | `title_bar`, `card_rects`, `gear_rect` |
| New / edit player (animals, name, ABC keys, Save, Delete, Back, grown-up hold) | `app/src/profiles.rs` `edit_layout` | `title_bar_in`, `keypad`, carousel and two-pane fallbacks |
| Link a device (tabs, code box, code pad, Join, New code, Back) | `app/src/link.rs` `Link::layout` | `title_bar_in`, `keypad` |
| Game over with the board, report buttons | `app/src/ui.rs` `results_with` | `results_layout` |
| Ready and report panels | `app/src/ui.rs` `panel` | `panel_layout` |
| Level done (Math Pop, Spell It) | `core/src/games/{mathpop,spell}.rs` `done_layout` | `done_layout` |
| Level pickers (Math Pop, Spell It) | `levels_layout` | `picker_layout` |
| Level clear (Breakout, 2048, Memory, Helix Drop, Lane Runner) | `cleared_sheet`, `complete_sheet`, `done_layout`, `win_sheet` | `stars_sheet` / `Stack` |
| Ball Sort win (Next, Skip, Levels) and Breakout lost (Try again, Levels) | `win_results`, `lost_levels_button` | `results_for` |
| Level selects (Ball Sort, Helix Drop, Lane Runner, Stack, Breakout, 2048, Memory, Count, Trains, Garden) | each game's `select_layout`, `picker_layout`, `level_tiles`, `level_rects`, `bed_tiles`, `tiles` | `grid` or own grid with `view.touch()` floor |
| Levels / Beds / Play buttons, bottom rows | each game's `levels_button`, `play_button`, `beds_button`, `buttons`, `bottom_buttons` | `corner_button`, `bottom_button`, `bottom_row` |
| Snake Clash title, Tower picker, munched Home | `core/src/games/clash/mod.rs` | `view.tap`, side column in landscape |
| Paper Land death panel and Play again | `core/src/games/paper.rs` | `sheet_w`, `bottom_button` |
| Road Hopper checkpoint | `core/src/games/crossy.rs` | `label_button` |
| Answer buttons (Count, Trains) | `button_rects` | `kid_tap` |
| Hotel level up and move overlays | `app/src/games/hotel/hud.rs` | no buttons; owned by the HUD work |
