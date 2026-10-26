use super::Play;
use crate::draw::{measure, Draw};
use crate::paint::Pen;
use crate::ui::{Results, Tone, BODY, INK};
use glam::{ivec2, vec2, vec4, Vec2};
use kg_core::engine::{Dir, Game, Input, View, HUD_LEFT};
use kg_core::fx::{ease_out_back, ease_out_cubic};
use kg_core::games::paper::arena::{self, MAX_LEVEL};
use kg_core::games::paper::*;
use kg_core::geom::{darker, with_alpha, Color, Rect, WHITE};
use kg_core::profile;

pub const KEY: &str = GAME;
const SHEET: &str = "paper";
const TOP: f32 = 100.0;

pub struct PaperPlay {
    pub g: Paper,
}

impl PaperPlay {
    pub fn new() -> Self {
        Self { g: Paper::new() }
    }
}

impl Default for PaperPlay {
    fn default() -> Self {
        Self::new()
    }
}

impl Play for PaperPlay {
    fn update(&mut self, input: &Input, view: &View, dt: f32) {
        let before = self.g.state;
        self.g.update(input, view, dt);
        if self.g.state == State::Lost && before != State::Lost {
            kg_core::net::request_board(GAME);
        }
    }

    fn draw(&self, pen: &mut Pen, view: &View) {
        draw(&self.g, pen, view);
    }

    fn score(&self) -> u32 {
        (self.g.share(0) * 100.0) as u32
    }

    fn over(&self) -> bool {
        matches!(self.g.state, State::Won | State::Lost)
    }

    fn as_any(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

fn scale_about(r: Rect, k: f32) -> Rect {
    let c = r.center();
    Rect::new(c.x - r.w * k / 2.0, c.y - r.h * k / 2.0, r.w * k, r.h * k)
}

fn ink() -> Color {
    Color::from_rgba(30, 80, 40, 255)
}

fn draw_lawn(pen: &mut Pen, board: Vec2, origin: Vec2, view: &View) {
    let tiles = (GW / GRASS_TILE_CELLS, GH / GRASS_TILE_CELLS_Y);
    let size = vec2(board.x / tiles.0 as f32, board.y / tiles.1 as f32);
    let seen = Rect::new(-origin.x, -origin.y, view.w, view.h);
    for ty in 0..tiles.1 {
        for tx in 0..tiles.0 {
            let at = vec2(tx as f32 * size.x, ty as f32 * size.y);
            if at.x > seen.x + seen.w
                || at.y > seen.y + seen.h
                || at.x + size.x < seen.x
                || at.y + size.y < seen.y
            {
                continue;
            }
            pen.sprite(
                SHEET,
                "grass",
                Rect::new(at.x, at.y, size.x + 0.5, size.y + 0.5),
                Draw {
                    flip_x: tx % 2 == 1,
                    flip_y: ty % 2 == 1,
                    ..Default::default()
                },
            );
        }
    }
}

fn draw_fence(pen: &mut Pen, board: Vec2, origin: Vec2, view: &View, cell: f32) {
    let ph = cell * 2.4;
    let fs = crate::art::size(SHEET, "fence");
    let pw = fs.x * ph / fs.y;
    let n = (board.x / pw).ceil() as usize;
    let draw = Draw::default();
    let seen = Rect::new(
        -origin.x - pw,
        -origin.y - pw,
        view.w + pw * 2.0,
        view.h + pw * 2.0,
    );
    for k in 0..n {
        let x = k as f32 * pw + pw / 2.0;
        for y in [-ph * 0.35, board.y + ph * 0.05] {
            if seen.contains(vec2(x, y)) {
                pen.sprite(
                    SHEET,
                    "fence",
                    Rect::new(x - pw / 2.0, y - ph / 2.0, pw + 1.0, ph),
                    draw,
                );
            }
        }
    }
    let m = (board.y / pw).ceil() as usize;
    for k in 0..m {
        let y = k as f32 * pw + pw / 2.0;
        for x in [-ph * 0.35, board.x + ph * 0.35] {
            if seen.contains(vec2(x, y)) {
                pen.sprite(
                    SHEET,
                    "fence",
                    Rect::new(x - pw / 2.0, y - ph / 2.0, pw + 1.0, ph),
                    Draw {
                        rotation: std::f32::consts::FRAC_PI_2,
                        ..draw
                    },
                );
            }
        }
    }
}

fn draw_board(g: &Paper, pen: &mut Pen, view: &View, cell: f32, origin: Vec2) {
    let board = vec2(cell * GW as f32, cell * GH as f32);
    pen.rect(
        Rect::new(5.0, 9.0, board.x, board.y),
        Color::new(0.1, 0.25, 0.1, 0.25),
    );
    draw_lawn(pen, board, origin, view);
    let x0 = ((-origin.x / cell).floor() as i32).max(0);
    let y0 = ((-origin.y / cell).floor() as i32).max(0);
    let x1 = (((view.w - origin.x) / cell).ceil() as i32).min(GW);
    let y1 = (((view.h - origin.y) / cell).ceil() as i32).min(GH);
    for y in y0..y1 {
        for x in x0..x1 {
            let i = idx(ivec2(x, y));
            let px = x as f32 * cell;
            let py = y as f32 * cell;
            if let Some(o) = g.shown_owner(i) {
                let age = g.cell_age(i);
                let pop = if age < 0.0 {
                    1.0
                } else {
                    0.8 + 0.2 * tile_lift(age)
                };
                let d = (cell + 0.5) * pop;
                let c = vec2(px + cell / 2.0, py + cell / 2.0);
                pen.rect(
                    Rect::new(px, py, cell + 0.5, cell + 0.5),
                    darker(color(o as usize), 0.75),
                );
                pen.sprite(
                    SHEET,
                    "tile",
                    Rect::new(c.x - d / 2.0, c.y - d / 2.0, d, d),
                    Draw::tint(color(o as usize)),
                );
                if (0.0..0.3).contains(&age) {
                    pen.rect(
                        Rect::new(px, py, cell + 0.5, cell + 0.5),
                        with_alpha(WHITE, 0.55 * (1.0 - age / 0.3)),
                    );
                }
            } else if let Some(t) = tuft_at(x, y) {
                let s = cell * (0.95 + 0.15 * (t % 3) as f32);
                pen.sprite_at(
                    SHEET,
                    TUFTS[t % TUFTS.len()],
                    vec2(px + cell * 0.5, py + cell * 0.55),
                    s,
                    Draw::default(),
                );
            }
            if let Some(t) = g.trail[i] {
                let pulse = 0.5 + 0.5 * (g.clock * 10.0 - (x + y) as f32 * 0.6).sin();
                let base = color(t as usize);
                let d = cell * (0.62 + 0.1 * pulse);
                let c = vec2(px + cell / 2.0, py + cell / 2.0);
                pen.circle(
                    vec2(c.x, c.y + d * 0.1),
                    d * 0.5,
                    with_alpha(darker(base, 0.5), 0.5),
                );
                pen.circle(
                    c,
                    d * 0.5,
                    Color::from_vec(base.to_vec() * 0.55 + vec4(0.45, 0.45, 0.45, 0.0)),
                );
                pen.circle(
                    vec2(c.x - d * 0.14, c.y - d * 0.16),
                    d * 0.17,
                    with_alpha(WHITE, 0.85),
                );
            }
        }
    }
    draw_fence(pen, board, origin, view, cell);
    let frac = (g.timer / g.step).clamp(0.0, 1.0);
    for (id, p) in g.players.iter().enumerate() {
        if !p.alive {
            continue;
        }
        let at = p.prev.as_vec2().lerp(p.pos.as_vec2(), frac);
        let bob = (g.clock * 9.0 + id as f32).sin() * cell * 0.06;
        let c = at * cell + vec2(cell / 2.0, cell / 2.0 + bob);
        let s = cell * 0.95;
        let col = color(id);
        pen.soft_shadow(c + vec2(0.0, s * 0.95), s, s * 0.35, 0.8);
        let lean = match p.dir {
            Dir::Left => -0.18,
            Dir::Right => 0.18,
            _ => 0.0,
        };
        pen.sprite_at(SHEET, HEADS[id % HEADS.len()], c, s * 2.0, Draw::rot(lean));
        let tag = if id == 0 { "YOU" } else { name(id) };
        pen.text_outline(
            tag,
            c.x,
            c.y - s - 16.0,
            if id == 0 { 26.0 } else { 22.0 },
            WHITE,
            darker(col, 0.5),
        );
    }
}

fn draw_hud(g: &Paper, pen: &mut Pen, view: &View) {
    let pct = g.share(0) * 100.0;
    let f = view.full();
    let bar = Rect::new(f.x - 8.0, f.y - 8.0, f.w + 16.0, TOP - 4.0 - f.y + 8.0);
    pen.gradient(
        bar,
        Color::from_rgba(70, 150, 60, 255),
        Color::from_rgba(40, 110, 50, 255),
    );
    pen.rect(
        Rect::new(bar.x, bar.y + bar.h, bar.w, 4.0),
        Color::from_rgba(25, 70, 35, 255),
    );
    pen.rect(
        Rect::new(bar.x, bar.y + bar.h - 2.0, bar.w, 2.0),
        Color::from_rgba(150, 220, 110, 255),
    );
    let pill = Rect::new(HUD_LEFT, 4.0, 220.0, 56.0);
    pen.painted_button(pill, "", Tone::Blue, 0.0, 1.0);
    pen.score_text(
        &format!("{pct:.1}%"),
        pill.x + pill.w / 2.0,
        32.0,
        34.0,
        g.bump.t,
    );
    let mut x = view.w - 20.0;
    for id in (1..g.players.len()).rev() {
        let s = format!("{:.0}%", g.share(id) * 100.0);
        let w = measure(&s, 24.0) + 60.0;
        x -= w + 8.0;
        let chip = Rect::new(x, 12.0, w, 40.0);
        let alive = g.players[id].alive;
        pen.chip_box(chip);
        let tint = if alive {
            WHITE
        } else {
            Color::new(0.5, 0.5, 0.5, 0.6)
        };
        pen.sprite_at(
            SHEET,
            HEADS[id % HEADS.len()],
            vec2(chip.x + 22.0, chip.y + chip.h / 2.0),
            34.0,
            Draw::tint(tint),
        );
        pen.text_center(
            &s,
            chip.x + 44.0 + (chip.w - 44.0) / 2.0 - 4.0,
            chip.y + chip.h / 2.0 + 2.0,
            24.0,
            INK,
        );
    }
    draw_goal(g, pen, view);
}

fn draw_goal(g: &Paper, pen: &mut Pen, view: &View) {
    let want = g.arena.target;
    let have = (g.share(0) / want).clamp(0.0, 1.0);
    let r = Rect::new(HUD_LEFT, 66.0, view.w - HUD_LEFT - 24.0, 22.0);
    pen.round_rect(r, r.h / 2.0, Color::new(0.05, 0.2, 0.08, 0.55));
    if have > 0.0 {
        let w = (r.w * have).max(r.h);
        pen.round_rect(
            Rect::new(r.x, r.y, w, r.h),
            r.h / 2.0,
            Color::from_rgba(255, 205, 60, 255),
        );
        pen.round_rect(
            Rect::new(r.x + 4.0, r.y + 3.0, (w - 8.0).max(0.0), r.h * 0.35),
            r.h * 0.18,
            Color::new(1.0, 1.0, 1.0, 0.35),
        );
    }
    let label = format!(
        "Arena {}: grab {:.0}% of the land",
        g.arena.level,
        want * 100.0
    );
    pen.text_outline(
        &label,
        r.x + r.w / 2.0,
        r.center().y + 1.0,
        20.0,
        WHITE,
        ink(),
    );
    let (fast, slow) = g.arena.par();
    let live = if g.elapsed <= fast {
        3
    } else if g.elapsed <= slow {
        2
    } else {
        1
    };
    let clock = arena::clock_label(g.elapsed);
    let cw = measure(&clock, 22.0) + 90.0;
    let chip = Rect::new(HUD_LEFT + 232.0, 12.0, cw, 40.0);
    if chip.x + chip.w < view.w - 20.0 - g.players.len().saturating_sub(1) as f32 * 90.0 {
        pen.chip_box(chip);
        for i in 0..3 {
            pen.sprite_at(
                "ui",
                if i < live { "star" } else { "star_empty" },
                vec2(chip.x + 24.0 + i as f32 * 22.0, chip.y + chip.h / 2.0),
                22.0,
                Draw::default(),
            );
        }
        pen.text_center(
            &clock,
            chip.x + 82.0 + (chip.w - 82.0) / 2.0 - 4.0,
            chip.y + chip.h / 2.0 + 2.0,
            22.0,
            WHITE,
        );
    }
}

fn draw_hints(g: &Paper, pen: &mut Pen, view: &View) {
    let pulse = 1.0 + (g.clock * 4.0).sin() * 0.04;
    let at = vec2(view.w / 2.0, view.h - 60.0);
    if !g.started {
        pen.fill_screen(view, Color::new(0.05, 0.15, 0.08, 0.28));
        let c = vec2(view.w / 2.0, view.h * 0.3);
        pen.ribbon(view, &format!("Arena {}", g.arena.level), c, g.clock);
        let goal = format!("Grab {:.0}% of the land to win", g.arena.target * 100.0);
        pen.text_outline(&goal, c.x, c.y + 80.0, 36.0, WHITE, ink());
        pen.hint("Swipe to start", at, pulse);
        pen.painted_button(arenas_button(view), "Arenas", Tone::Blue, 0.0, 1.0);
    } else if g.training && !g.grabbed {
        let label = if g.players[0].trail.is_empty() {
            "Swipe to go out onto the grass"
        } else {
            "Now swipe round and come back home"
        };
        pen.hint(label, at, pulse);
    }
}

fn draw_win(g: &Paper, pen: &mut Pen, view: &View) {
    let age = g.since_end;
    pen.fill_screen(
        view,
        Color::new(0.06, 0.04, 0.16, 0.5 * ease_out_cubic(age / 0.2)),
    );
    let s = ease_out_back(age / 0.32).max(0.01);
    let r = win_panel(view);
    let c = r.center();
    let r = Rect::new(
        c.x - r.w * s / 2.0,
        c.y - r.h * s / 2.0 + (1.0 - ease_out_cubic(age / 0.32)) * 90.0,
        r.w * s,
        r.h * s,
    );
    pen.painted_panel(r, WHITE);
    pen.ribbon(
        view,
        &format!("Arena {} clear!", g.arena.level),
        vec2(c.x, r.y + 8.0 * s),
        age,
    );
    for i in 0..3usize {
        let k = ease_out_back(((age - 0.3 - i as f32 * 0.18) / 0.3).clamp(0.0, 1.0));
        let star = if (i as u8) < g.stars {
            "star"
        } else {
            "star_empty"
        };
        let size = 110.0 * s * if i == 1 { 1.15 } else { 1.0 };
        pen.sprite_at(
            "ui",
            star,
            vec2(
                c.x + (i as f32 - 1.0) * 120.0 * s,
                r.y + 140.0 * s - if i == 1 { 10.0 } else { 0.0 },
            ),
            size * k.max(0.01),
            Draw::rot((i as f32 - 1.0) * 0.18),
        );
    }
    let line = format!(
        "{:.0}% of the land in {}",
        g.share(0) * 100.0,
        arena::clock_label(g.elapsed)
    );
    pen.text_center(&line, c.x, r.y + 250.0 * s, 32.0 * s, BODY);
    let note = if g.rank == Some(0) {
        Some("New best!")
    } else if g.arena.level >= MAX_LEVEL {
        Some("You cleared every arena!")
    } else if g.stars < 3 {
        Some("Win faster for more stars")
    } else {
        None
    };
    if let Some(n) = note {
        pen.text_outline(
            n,
            c.x,
            r.y + 300.0 * s,
            32.0 * s,
            Color::from_rgba(255, 200, 40, 255),
            Color::from_rgba(130, 50, 0, 255),
        );
    }
    let pulse = 1.0 + (age * 4.5).sin() * 0.03;
    let next = if g.arena.level >= MAX_LEVEL {
        "Again"
    } else {
        "Next"
    };
    let a = ((age - 0.4) / 0.2).clamp(0.0, 1.0);
    pen.painted_button(
        scale_about(win_next_button(view), pulse),
        next,
        Tone::Green,
        0.0,
        a,
    );
    pen.painted_button(win_arenas_button(view), "Arenas", Tone::Blue, 0.0, a);
}

fn draw_select(g: &Paper, pen: &mut Pen, view: &View) {
    pen.sky(
        view,
        Color::from_rgba(150, 215, 120, 255),
        Color::from_rgba(70, 150, 70, 255),
    );
    let t = g.since_end;
    pen.ribbon(view, "Arenas", vec2(view.w / 2.0, 70.0), g.clock);
    let open = unlocked_arenas();
    let current = profile::level(GAME).max(1);
    let stars = profile::total_stars(GAME);
    let tally = format!("{} / {}", stars, MAX_LEVEL * 3);
    let tw = measure(&tally, 30.0) + 60.0;
    let chip = Rect::new(view.w / 2.0 - tw / 2.0, 118.0, tw, 40.0);
    pen.chip_box(chip);
    pen.sprite_at(
        "ui",
        "star",
        vec2(chip.x + 24.0, chip.center().y),
        28.0,
        Draw::default(),
    );
    pen.text_center(
        &tally,
        chip.x + 44.0 + (chip.w - 44.0) / 2.0 - 6.0,
        chip.center().y + 2.0,
        26.0,
        WHITE,
    );
    for (i, r) in select_cells(view).iter().enumerate() {
        let level = i as u32 + 1;
        let pop = ease_out_back(((t - i as f32 * 0.03) / 0.25).clamp(0.0, 1.0));
        let r = scale_about(*r, pop.max(0.01));
        let locked = level > open;
        let tint = if locked {
            Color::new(0.75, 0.75, 0.8, 1.0)
        } else {
            WHITE
        };
        pen.painted_panel(r, tint);
        if locked {
            pen.sprite_at(
                "ui",
                "icon_lock",
                r.center(),
                r.h * 0.4,
                Draw::tint(Color::new(0.45, 0.4, 0.55, 1.0)),
            );
            continue;
        }
        let ring = if level == current {
            Color::from_rgba(255, 190, 40, 255)
        } else {
            Color::from_rgba(60, 120, 60, 255)
        };
        pen.text_outline(
            &level.to_string(),
            r.center().x,
            r.y + r.h * 0.42,
            r.h * 0.42,
            WHITE,
            ring,
        );
        let got = profile::stars(GAME, level);
        for k in 0..3u8 {
            pen.sprite_at(
                "ui",
                if k < got { "star" } else { "star_empty" },
                vec2(
                    r.center().x + (k as f32 - 1.0) * r.w * 0.25,
                    r.y + r.h * 0.76,
                ),
                r.h * 0.22,
                Draw::default(),
            );
        }
    }
    let pulse = 1.0 + (g.clock * 4.5).sin() * 0.03;
    pen.painted_button(
        scale_about(select_back_button(view), pulse),
        "Play",
        Tone::Green,
        0.0,
        1.0,
    );
}

pub fn draw(g: &Paper, pen: &mut Pen, view: &View) {
    if g.state == State::Select {
        draw_select(g, pen, view);
        return;
    }
    let f = view.full();
    pen.rect(f, GRASS_OUTER);
    let (cell, origin) = g.layout(view);
    pen.offset = origin + g.shake.offset();
    draw_board(g, pen, view, cell, origin);
    pen.particles(&g.particles);
    pen.popups(&g.popups);
    pen.offset = Vec2::ZERO;
    draw_hud(g, pen, view);
    pen.particles(&g.screen_fx);
    match g.state {
        State::Playing => draw_hints(g, pen, view),
        State::Lost => {
            let line = format!(
                "Arena {}: {:.0}% of {:.0}%",
                g.arena.level,
                g.share(0) * 100.0,
                g.arena.target * 100.0
            );
            pen.results(
                view,
                &Results {
                    game: GAME,
                    title: "Oops!",
                    line: &line,
                    rank: g.rank,
                    action: "Try again",
                    format: arena::score_label,
                },
                g.since_end,
            );
            pen.painted_button(lost_arenas_button(view), "Arenas", Tone::Blue, 0.0, 1.0);
        }
        State::Won => draw_win(g, pen, view),
        State::Select => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paint::Tex;

    fn fresh() -> PaperPlay {
        kg_core::save::reset_for_test();
        kg_core::profile::create("Mia", 0);
        PaperPlay::new()
    }

    fn drawn(p: &PaperPlay, v: &View) -> Pen {
        let mut pen = Pen::default();
        pen.begin(*v, 1.0);
        p.draw(&mut pen, v);
        pen
    }

    fn pages(pen: &Pen, sheet: &str) -> bool {
        pen.used()
            .iter()
            .filter_map(|b| b.tex)
            .any(|t| matches!(t, Tex::Page(s, _) if s == sheet))
    }

    #[test]
    fn the_start_screen_draws_the_lawn_players_and_text() {
        let v = View::ipad_landscape();
        let p = fresh();
        let pen = drawn(&p, &v);
        assert!(pages(&pen, "paper") && pages(&pen, "ui"));
        assert!(pen.used().iter().any(|b| b.tex == Some(Tex::Glyphs)));
    }

    #[test]
    fn every_state_draws_in_both_orientations() {
        for v in [View::ipad_landscape(), View::ipad_portrait()] {
            let mut p = fresh();
            p.g.state = State::Won;
            p.g.stars = 2;
            assert!(drawn(&p, &v).vertex_count() > 300);
            p.g.state = State::Lost;
            p.g.since_end = 1.0;
            assert!(drawn(&p, &v).vertex_count() > 300);
            p.g.open_select();
            p.g.since_end = 1.0;
            let pen = drawn(&p, &v);
            assert!(pen.vertex_count() > 300);
            assert!(pages(&pen, "ui"));
        }
    }

    #[test]
    fn the_lawn_and_hud_bar_cover_the_screen_past_the_safe_area() {
        let v = View::fit_safe(
            2400.0,
            1080.0,
            kg_core::safe::Insets {
                left: 120.0,
                bottom: 40.0,
                ..Default::default()
            },
        );
        let p = fresh();
        let pen = drawn(&p, &v);
        let f = v.full();
        let bg = &pen.used()[0];
        let (mut lo, mut hi) = (f32::MAX, f32::MIN);
        let (mut top, mut bot) = (f32::MAX, f32::MIN);
        for q in &bg.pos {
            lo = lo.min(q[0]);
            hi = hi.max(q[0]);
            top = top.min(-q[1]);
            bot = bot.max(-q[1]);
        }
        assert!(lo <= f.x + 0.01 && hi >= f.x + f.w, "{lo}..{hi} vs {f:?}");
        assert!(top <= f.y + 0.5 && bot >= f.y + f.h - 0.5);
    }

    #[test]
    fn losing_asks_for_the_board_and_the_score_is_the_share() {
        let v = View::ipad_landscape();
        let mut p = fresh();
        p.g.started = true;
        p.g.players[0].alive = false;
        p.update(&Input::default(), &v, 1.0 / 60.0);
        assert_eq!(p.g.state, State::Lost);
        assert_eq!(p.score(), (p.g.share(0) * 100.0) as u32);
    }
}
