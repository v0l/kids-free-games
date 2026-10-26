use super::Play;
use crate::draw::{Draw, PLUM};
use crate::paint::Pen;
use crate::ui::{plain_score, Results, Tone, BODY};
use glam::{vec2, Vec2};
use kg_core::engine::{Game, Input, View};
use kg_core::fx::{ease_out_back, ease_out_cubic};
use kg_core::games::merge::*;
use kg_core::geom::{darker, with_alpha, Color, Rect, WHITE};

pub const KEY: &str = GAME;
const SHEET: &str = "merge";

pub struct MergePlay {
    pub g: Merge,
}

impl MergePlay {
    pub fn new() -> Self {
        Self { g: Merge::new() }
    }
}

impl Default for MergePlay {
    fn default() -> Self {
        Self::new()
    }
}

impl Play for MergePlay {
    fn update(&mut self, input: &Input, view: &View, dt: f32) {
        let before = self.g.screen;
        self.g.update(input, view, dt);
        if self.g.screen == Screen::Lost && before != Screen::Lost {
            kg_core::net::request_board(GAME);
        }
    }

    fn draw(&self, pen: &mut Pen, view: &View) {
        draw(&self.g, pen, view);
    }

    fn score(&self) -> u32 {
        self.g.score
    }

    fn over(&self) -> bool {
        self.g.screen == Screen::Lost
    }

    fn as_any(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

fn draw_tile(pen: &mut Pen, center: Vec2, cs: f32, scale: f32, v: u32, t: f32) {
    let tint = tile_tint(v);
    let s = cs * scale;
    let r = Rect::new(center.x - s / 2.0, center.y - s / 2.0, s, s);
    if v >= 128 {
        let glow = 0.35 + 0.15 * (t * 4.0).sin();
        pen.sprite(
            SHEET,
            "tile",
            Rect::new(r.x - 6.0, r.y - 6.0, r.w + 12.0, r.h + 12.0),
            Draw::tint(with_alpha(Color::new(1.0, 0.95, 0.5, 1.0), glow)),
        );
    }
    pen.sprite(
        SHEET,
        "tile",
        Rect::new(r.x, r.y + 5.0 * scale, r.w, r.h),
        Draw::tint(Color::new(0.0, 0.0, 0.0, 0.22)),
    );
    pen.sprite(SHEET, "tile", r, Draw::tint(tint));
    if v >= 2048 {
        let bob = (t * 3.0).sin() * s * 0.02;
        pen.sprite_at(
            SHEET,
            "crown",
            vec2(r.center().x, r.y + s * 0.1 + bob),
            s * 0.32,
            Draw::default(),
        );
    }
    let label = v.to_string();
    let fs = s * match label.len() {
        0..=2 => 0.5,
        3 => 0.4,
        _ => 0.32,
    };
    pen.text_outline(
        &label,
        center.x,
        center.y + s * 0.03,
        fs,
        WHITE,
        darker(tint, 0.45),
    );
}

fn draw_backdrop(pen: &mut Pen, view: &View, t: f32) {
    let f = view.full();
    let side = f.w.max(f.h);
    pen.sprite(
        SHEET,
        "bg",
        Rect::new(f.x + (f.w - side) / 2.0, f.y + f.h - side, side, side),
        Draw::default(),
    );
    for (i, (x, y, w)) in [(0.15, 0.1, 190.0), (0.62, 0.2, 240.0), (0.9, 0.06, 160.0)]
        .iter()
        .enumerate()
    {
        let drift = (t * (6.0 + i as f32 * 3.0) + i as f32 * 300.0) % (view.w + 400.0) - 200.0;
        let cx = (view.w * x + drift).rem_euclid(view.w + 400.0) - 200.0;
        let name = format!("cloud{}", (i * 3 + 1) % 8 + 1);
        pen.sprite_w(
            "ui",
            &name,
            vec2(cx, view.h * y + 90.0),
            *w,
            Draw::tint(with_alpha(WHITE, 0.85)),
        );
    }
}

fn draw_board(g: &Merge, pen: &mut Pen, view: &View) {
    let l = Layout::new(view, g.n);
    let n = g.n;
    let sh = g.shake.offset();
    let frame = Rect::new(l.origin.x + sh.x, l.origin.y + sh.y, l.size, l.size);
    let pad = l.size * 0.075;
    pen.sprite(
        SHEET,
        "board",
        Rect::new(
            frame.x - pad,
            frame.y - pad,
            frame.w + pad * 2.0,
            frame.h + pad * 2.0,
        ),
        Draw::default(),
    );
    for r in 0..n {
        for c in 0..n {
            let p = l.pos(r as f32, c as f32) + sh;
            pen.sprite(
                SHEET,
                "cell",
                Rect::new(p.x, p.y, l.cell, l.cell),
                Draw::tint(Color::new(1.0, 1.0, 1.0, 0.55)),
            );
        }
    }
    if g.sliding() {
        let k = ease_out_cubic(g.slide_t / SLIDE);
        for m in &g.slides {
            let from = l.pos(m.from.0 as f32, m.from.1 as f32);
            let to = l.pos(m.to.0 as f32, m.to.1 as f32);
            let p = from + (to - from) * k + sh + Vec2::splat(l.cell / 2.0);
            draw_tile(pen, p, l.cell, 1.0, m.value, g.t);
        }
    } else {
        for r in 0..n {
            for c in 0..n {
                let v = g.board[r][c];
                if v == 0 {
                    continue;
                }
                let s = born_scale(g.born[r][c]).max(0.01) * merge_scale(g.merged_at[r][c]);
                draw_tile(pen, l.center(r, c) + sh, l.cell, s, v, g.t);
            }
        }
    }
}

fn draw_hud(g: &Merge, pen: &mut Pen, view: &View) {
    let x = chip_x(view);
    let y = 44.0;
    for (i, (label, v)) in [("SCORE", g.score), ("BEST", g.best.0)].iter().enumerate() {
        let r = Rect::new(if i == 0 { x } else { view.w - x - 160.0 }, y, 160.0, 80.0);
        pen.chip_box(r);
        pen.text_center(label, r.x + r.w / 2.0, r.y + 22.0, 22.0, BODY);
        if i == 0 {
            pen.score_text(
                &v.to_string(),
                r.x + r.w / 2.0,
                r.y + 54.0,
                36.0,
                g.score_bump.t,
            );
        } else {
            pen.text_outline(
                &v.to_string(),
                r.x + r.w / 2.0,
                r.y + 54.0,
                36.0,
                Color::from_rgba(255, 200, 40, 255),
                PLUM,
            );
        }
    }
    let cx = view.w / 2.0;
    pen.text_outline(
        &format!("Level {}", g.level),
        cx,
        y + 14.0,
        44.0 * (1.0 + 0.02 * (g.t * 2.0).sin()),
        Color::from_rgba(255, 215, 90, 255),
        Color::from_rgba(119, 90, 60, 255),
    );
    let track = Rect::new(cx - 160.0, y + 58.0, 240.0, 22.0);
    pen.round_rect(track, 11.0, Color::new(0.1, 0.05, 0.2, 0.55));
    if g.bar > 0.01 {
        let w = (track.w - 4.0) * g.bar;
        pen.round_rect(
            Rect::new(track.x + 2.0, track.y + 2.0, w.max(18.0), 18.0),
            9.0,
            Color::from_rgba(255, 210, 60, 255),
        );
    }
    let goal = vec2(cx + 130.0, track.center().y);
    draw_tile(pen, goal, 56.0, 1.0, g.target(), g.t);
}

fn draw_hint(g: &Merge, pen: &mut Pen, view: &View) {
    if g.ended() {
        return;
    }
    let label = if !g.learned && g.moves == 0 {
        "Swipe to slide the tiles".to_string()
    } else if !g.learned {
        "Same numbers join up!".to_string()
    } else if g.moves < 2 {
        format!("Make a {} tile!", g.target())
    } else {
        return;
    };
    let pulse = 0.85 + (g.t * 5.0).sin() * 0.03;
    pen.hint(&label, vec2(view.w / 2.0, view.h - 40.0), pulse);
}

fn draw_buttons(g: &Merge, pen: &mut Pen, view: &View) {
    let (levels, undo) = bottom_buttons(view);
    pen.painted_button(levels, "Levels", Tone::Blue, 0.0, 1.0);
    let label = match g.undos_left {
        Some(n) => format!("Undo {n}"),
        None => "Undo".to_string(),
    };
    let alpha = if g.can_undo() { 1.0 } else { 0.4 };
    pen.painted_button(undo, &label, Tone::Orange, 0.0, alpha);
}

fn draw_complete(g: &Merge, pen: &mut Pen, view: &View) {
    let age = g.screen_t;
    let (panel, next, levels) = complete_layout(view);
    let fade = ease_out_cubic(age / 0.2);
    pen.fill_screen(view, Color::new(0.06, 0.04, 0.16, 0.5 * fade));
    let s = ease_out_back(age / 0.32).max(0.01);
    let c = panel.center();
    let r = Rect::new(
        c.x - panel.w * s / 2.0,
        c.y - panel.h * s / 2.0,
        panel.w * s,
        panel.h * s,
    );
    pen.painted_panel(r, WHITE);
    let title = if g.level == last_level() {
        "2048!".to_string()
    } else {
        format!("Level {} done!", g.level)
    };
    pen.ribbon(view, &title, vec2(c.x, r.y + 8.0 * s), age);
    for i in 0..3 {
        let k = ease_out_back(((age - 0.25 - i as f32 * 0.15) / 0.3).clamp(0.0, 1.0));
        let earned = (i as u8) < g.stars_earned;
        let size = if i == 1 { 130.0 } else { 104.0 };
        let at = vec2(
            c.x + (i as f32 - 1.0) * 130.0,
            r.y + 190.0 * s - (i == 1) as i32 as f32 * 14.0,
        );
        let tint = if earned {
            WHITE
        } else {
            Color::new(0.2, 0.15, 0.3, 0.3)
        };
        pen.ui(
            "p_star",
            Rect::new(
                at.x - size * k / 2.0,
                at.y - size * k / 2.0,
                size * k,
                size * k,
            ),
            Draw {
                tint,
                rotation: (i as f32 - 1.0) * 0.15,
                ..Default::default()
            },
        );
    }
    let line = format!("You made {}!   Score {}", g.target(), g.score);
    pen.text_center(
        &line,
        c.x,
        r.y + 290.0 * s,
        34.0 * s,
        with_alpha(BODY, ((age - 0.4) / 0.2).clamp(0.0, 1.0)),
    );
    if g.rank == Some(0) {
        pen.text_outline(
            "New best!",
            c.x,
            r.y + 340.0 * s,
            32.0 * s,
            Color::from_rgba(255, 200, 40, 255),
            Color::from_rgba(130, 50, 0, 255),
        );
    }
    let a = ((age - 0.5) / 0.2).clamp(0.0, 1.0);
    let pulse = 1.0 + (age * 4.5).sin() * 0.03;
    let label = if g.level == last_level() {
        "Play again"
    } else {
        "Next level"
    };
    pen.painted_button(next.scaled_about_center(pulse), label, Tone::Green, 0.0, a);
    pen.painted_button(levels, "Levels", Tone::Blue, 0.0, a);
}

fn draw_select(g: &Merge, pen: &mut Pen, view: &View) {
    pen.ribbon(view, "Choose a level", vec2(view.w / 2.0, 84.0), g.t);
    let open = open_levels(kg_core::profile::unlocked(GAME));
    let total = format!(
        "{} / {}",
        kg_core::profile::total_stars(GAME),
        LEVELS.len() * 3
    );
    let chip = Rect::new(view.w - 200.0, 24.0, 150.0, 48.0);
    pen.chip_box(chip);
    pen.text_outline(
        &total,
        chip.center().x + 14.0,
        chip.center().y,
        28.0,
        WHITE,
        PLUM,
    );
    pen.sprite_at(
        "ui",
        "p_star",
        vec2(chip.x + 30.0, chip.center().y),
        40.0,
        Draw::default(),
    );
    for (i, r) in level_rects(view).iter().enumerate() {
        let level = i as u32 + 1;
        let def = LEVELS[i];
        let locked = level > open;
        let current = level == g.level;
        let bob = if current {
            1.0 + (g.t * 4.0).sin() * 0.02
        } else {
            1.0
        };
        let r = r.scaled_about_center(bob);
        let tint = if locked {
            Color::new(0.6, 0.6, 0.7, 1.0)
        } else if current {
            Color::new(1.0, 0.97, 0.8, 1.0)
        } else {
            WHITE
        };
        pen.painted_panel(r, tint);
        pen.text_outline(
            &level.to_string(),
            r.center().x,
            r.y + r.h * 0.17,
            r.h * 0.17,
            WHITE,
            Color::from_rgba(119, 90, 60, 255),
        );
        let ts = r.w * 0.5;
        let tc = vec2(r.center().x, r.y + r.h * 0.46);
        if locked {
            pen.sprite_at(
                "ui",
                "icon_lock",
                tc,
                ts * 0.8,
                Draw::tint(Color::new(0.35, 0.3, 0.45, 0.8)),
            );
        } else {
            draw_tile(pen, tc, ts, 1.0, def.target, g.t);
        }
        pen.text_center(
            &format!("{0}x{0}", def.size),
            r.center().x,
            r.y + r.h * 0.69,
            r.h * 0.085,
            BODY,
        );
        let got = kg_core::profile::stars(GAME, level);
        for s in 0..3 {
            let at = vec2(
                r.center().x + (s as f32 - 1.0) * r.w * 0.27,
                r.y + r.h * 0.84,
            );
            let tint = if (s as u8) < got {
                WHITE
            } else {
                Color::new(0.2, 0.15, 0.3, 0.25)
            };
            pen.sprite_at("ui", "p_star", at, r.w * 0.26, Draw::tint(tint));
        }
    }
}

pub fn draw(g: &Merge, pen: &mut Pen, view: &View) {
    draw_backdrop(pen, view, g.t);
    if g.screen == Screen::Select {
        draw_select(g, pen, view);
        return;
    }
    draw_board(g, pen, view);
    draw_hud(g, pen, view);
    pen.particles(&g.particles);
    pen.popups(&g.popups);
    match g.screen {
        Screen::Complete => draw_complete(g, pen, view),
        Screen::Lost => {
            let line = format!("Score {}   Level {}", g.score, g.level);
            pen.results(
                view,
                &Results {
                    game: GAME,
                    title: "No more moves",
                    line: &line,
                    rank: g.rank,
                    action: "Try again",
                    format: plain_score,
                },
                g.screen_t,
            );
            draw_buttons(g, pen, view);
        }
        _ => {
            draw_hint(g, pen, view);
            draw_buttons(g, pen, view);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paint::Tex;
    use kg_core::engine::Dir;

    fn fresh() -> MergePlay {
        kg_core::save::reset_for_test();
        kg_core::profile::create("Mia", 0);
        MergePlay::new()
    }

    fn drawn(p: &MergePlay, v: &View) -> Pen {
        let mut pen = Pen::default();
        pen.begin(*v, 1.0);
        p.draw(&mut pen, v);
        pen
    }

    fn views() -> [View; 2] {
        [View::ipad_landscape(), View::ipad_portrait()]
    }

    fn sheets(pen: &Pen) -> Vec<Tex> {
        pen.used().iter().filter_map(|b| b.tex).collect()
    }

    #[test]
    fn the_play_screen_draws_board_tiles_text_and_buttons() {
        for v in views() {
            let pen = drawn(&fresh(), &v);
            let texs = sheets(&pen);
            assert!(texs.iter().any(|t| matches!(t, Tex::Page("merge", _))));
            assert!(texs.iter().any(|t| matches!(t, Tex::Page("ui", _))));
            assert!(texs.contains(&Tex::Glyphs));
        }
    }

    #[test]
    fn the_level_picker_draws_every_level_card() {
        for v in views() {
            let mut p = fresh();
            p.g.screen = Screen::Select;
            let pen = drawn(&p, &v);
            assert!(pen.vertex_count() > 400, "{}", pen.vertex_count());
        }
    }

    #[test]
    fn the_complete_and_lost_screens_draw_over_the_board() {
        let v = View::ipad_landscape();
        let mut p = fresh();
        let play = drawn(&p, &v).vertex_count();
        p.g.screen = Screen::Complete;
        p.g.screen_t = 2.0;
        assert!(drawn(&p, &v).vertex_count() > play);
        p.g.screen = Screen::Lost;
        assert!(drawn(&p, &v).vertex_count() > play);
    }

    #[test]
    fn the_backdrop_covers_the_whole_screen_past_the_safe_area() {
        let v = View::fit_safe(
            2400.0,
            1080.0,
            kg_core::safe::Insets {
                left: 120.0,
                bottom: 40.0,
                ..Default::default()
            },
        );
        let pen = drawn(&fresh(), &v);
        let f = v.full();
        let bg = &pen.used()[0];
        let (mut lo, mut hi, mut top, mut bot) = (f32::MAX, f32::MIN, f32::MAX, f32::MIN);
        for q in &bg.pos {
            lo = lo.min(q[0]);
            hi = hi.max(q[0]);
            top = top.min(-q[1]);
            bot = bot.max(-q[1]);
        }
        assert!(
            lo <= f.x + 0.01 && hi >= f.x + f.w,
            "bg {lo}..{hi} vs {f:?}"
        );
        assert!(top <= f.y + 0.5 && bot >= f.y + f.h - 0.5);
    }

    #[test]
    fn a_swipe_slides_tiles_and_the_slide_draws() {
        let v = View::ipad_landscape();
        let mut p = fresh();
        p.update(&Input::key(Dir::Left), &v, 0.01);
        assert!(p.g.sliding());
        assert!(drawn(&p, &v).vertex_count() > 200);
    }

    #[test]
    fn every_tile_size_draws_its_label() {
        let v = View::ipad_landscape();
        let mut p = fresh();
        p.g.board = [
            [2, 4, 8, 16],
            [32, 64, 128, 256],
            [512, 1024, 2048, 0],
            [0; 4],
        ];
        p.g.n = 4;
        assert!(sheets(&drawn(&p, &v)).contains(&Tex::Glyphs));
    }
}
