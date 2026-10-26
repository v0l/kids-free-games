use super::Play;
use crate::draw::Draw;
use crate::paint::Pen;
use crate::ui::{Tone, BODY};
use glam::{vec2, Vec2};
use kg_core::engine::{Game, Input, View, HUD_LEFT};
use kg_core::fx::{ease_out_back, ease_out_cubic};
use kg_core::games::mathpop::*;
use kg_core::geom::{darker, with_alpha, Color, Rect, WHITE};
use kg_core::learn::{hud_bottom, ui_scale};
use kg_core::profile;

pub const KEY: &str = "mathpop";
const SHEET: &str = "mathpop";
const GOLD: Color = Color::new(1.0, 0.82, 0.24, 1.0);
const PINK: Color = Color::new(0.92, 0.39, 0.67, 1.0);

pub struct MathPopPlay {
    pub g: MathPop,
    final_shown: bool,
}

impl MathPopPlay {
    pub fn new() -> Self {
        Self {
            g: MathPop::new(),
            final_shown: false,
        }
    }
}

impl Default for MathPopPlay {
    fn default() -> Self {
        Self::new()
    }
}

impl Play for MathPopPlay {
    fn update(&mut self, input: &Input, view: &View, dt: f32) {
        self.g.update(input, view, dt);
        let final_now = matches!(self.g.screen, Screen::Final(_));
        if final_now && !self.final_shown {
            kg_core::net::request_board(KEY);
        }
        self.final_shown = final_now;
    }

    fn draw(&self, pen: &mut Pen, view: &View) {
        draw(&self.g, pen, view);
    }

    fn score(&self) -> u32 {
        profile::total_stars(KEY) as u32
    }

    fn over(&self) -> bool {
        matches!(self.g.screen, Screen::Final(_))
    }

    fn as_any(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

fn plural_stars(n: i64) -> String {
    format!("{n} stars")
}

fn scaled(r: Rect, k: f32) -> Rect {
    let c = r.center();
    Rect::new(c.x - r.w * k / 2.0, c.y - r.h * k / 2.0, r.w * k, r.h * k)
}

fn star_pair(pen: &mut Pen, c: Vec2, r: f32, lit: bool) {
    pen.star(
        vec2(c.x, c.y + 3.0),
        r * 1.05,
        0.0,
        Color::new(0.1, 0.05, 0.25, 0.3),
    );
    let col = if lit {
        GOLD
    } else {
        Color::new(0.75, 0.72, 0.82, 0.8)
    };
    pen.star(c, r, 0.0, col);
}

fn star_row(pen: &mut Pen, c: Vec2, r: f32, gap: f32, have: u8, total: u8) {
    for i in 0..total {
        let x = c.x + (i as f32 - (total as f32 - 1.0) / 2.0) * gap;
        star_pair(pen, vec2(x, c.y), r, i < have);
    }
}

fn padlock(pen: &mut Pen, c: Vec2, r: f32) {
    let ink = Color::from_rgba(120, 112, 150, 255);
    pen.ring(vec2(c.x, c.y - r * 0.55), r * 0.5, r * 0.18, ink);
    pen.round_rect(
        Rect::new(c.x - r * 0.7, c.y - r * 0.3, r * 1.4, r * 1.0),
        r * 0.2,
        ink,
    );
}

fn draw_bunnies(g: &MathPop, pen: &mut Pen, view: &View) {
    let h = 150.0 * ui_scale(view);
    let hop = match g.leave {
        Some(l) => (l * 14.0).sin().abs() * h * 0.2,
        None => 0.0,
    };
    let names = match g.mood() {
        "up" => ["bunny_up", "pink_up"],
        "sad" => ["bunny_sad", "pink_sad"],
        _ => ["bunny", "pink"],
    };
    let y = view.h - h * 0.5 - 10.0 - hop;
    for (i, name) in names.iter().enumerate() {
        let x = if i == 0 { 70.0 } else { view.w - 70.0 };
        pen.sprite_at(
            SHEET,
            name,
            vec2(x, y),
            h,
            Draw {
                flip_x: i != 0,
                ..Default::default()
            },
        );
    }
}

fn draw_backdrop(g: &MathPop, pen: &mut Pen, view: &View) {
    let f = view.full();
    let side = f.w.max(f.h);
    pen.sprite(
        SHEET,
        "bg",
        Rect::new(f.x + (f.w - side) / 2.0, f.y + f.h - side, side, side),
        Draw::default(),
    );
    for i in 0..5usize {
        let span = view.w + 500.0;
        let x = (i as f32 * 0.23 * span + g.t * (7.0 + i as f32 * 3.0)) % span - 250.0;
        let y = hud_bottom(view) + 20.0 + (i as f32 * 0.37 % 1.0) * view.h * 0.35;
        pen.cloud(
            i * 2,
            vec2(x, y),
            150.0 + (i % 3) as f32 * 50.0,
            with_alpha(WHITE, 0.9),
        );
    }
    draw_bunnies(g, pen, view);
}

fn draw_hud(g: &MathPop, pen: &mut Pen, view: &View) {
    let k = ui_scale(view);
    let label = format!("Level {}: {}", g.level + 1, LEVEL_NAMES[g.level]);
    let tw = kg_core::font::measure(&label, 28.0 * k) + 44.0 * k;
    let chip = Rect::new(HUD_LEFT, 18.0, tw, 46.0 * k);
    pen.round_rect(
        Rect::new(chip.x, chip.y + 4.0, chip.w, chip.h),
        chip.h / 2.0,
        Color::new(0.0, 0.0, 0.0, 0.18),
    );
    pen.round_rect(chip, chip.h / 2.0, with_alpha(WHITE, 0.94));
    pen.text_center(
        &label,
        chip.center().x,
        chip.center().y,
        28.0 * k,
        darker(PINK, 0.6),
    );
    pen.painted_button(levels_button(view), "Levels", Tone::Blue, g.press, 1.0);
    let bar_w = (view.w - 180.0).min(520.0 * k);
    let gap = 8.0 * k;
    let n = QUESTIONS;
    let seg = (bar_w - gap * (n as f32 - 1.0)) / n as f32;
    let x0 = view.w / 2.0 - bar_w / 2.0;
    for j in 0..n {
        let r = Rect::new(
            x0 + j as f32 * (seg + gap),
            chip.y + chip.h + 12.0,
            seg,
            18.0 * k,
        );
        let filled = j < g.answered;
        let last = filled && j + 1 == g.answered;
        let sc = if last { g.streak_bump.scale(0.35) } else { 1.0 };
        let rr = scaled(r, sc);
        pen.round_rect(
            Rect::new(rr.x, rr.y + 3.0, rr.w, rr.h),
            rr.h / 2.0,
            Color::new(0.0, 0.0, 0.0, 0.15),
        );
        pen.round_rect(
            rr,
            rr.h / 2.0,
            if filled { GOLD } else { with_alpha(WHITE, 0.5) },
        );
        if filled {
            pen.gloss(rr, rr.h / 2.0);
        }
    }
}

fn draw_question(g: &MathPop, pen: &mut Pen, view: &View) {
    let text = g.question.text(g.solved);
    let size = question_text_size(view);
    let pop = ease_out_back(g.question_bump.t).max(0.2);
    let base = question_rect(view);
    let c = base.center();
    pen.painted_panel(scaled(base, pop), WHITE);
    let ink = if g.solved {
        Color::from_rgba(60, 170, 90, 255)
    } else {
        Color::from_rgba(70, 60, 130, 255)
    };
    pen.text_center(&text, c.x, c.y, size * pop, ink);
}

fn draw_balloon(pen: &mut Pen, c: Vec2, r: f32, b: &Balloon, t: f32) {
    let (sprite, color) = BALLOON_COLORS[b.color];
    let fade = if b.tried { 0.5 } else { 1.0 };
    let squash = 1.0 + 0.06 * (b.wobble * 30.0).sin() * b.wobble;
    let ry = r / squash;
    let height = ry * 3.1;
    let s = crate::art::size(SHEET, sprite);
    let width = height * s.x / s.y * squash;
    let rect = Rect::new(c.x - width / 2.0, c.y - ry, width, height);
    let base = if b.tried {
        Color::new(0.8, 0.8, 0.85, 1.0)
    } else {
        WHITE
    };
    pen.sprite(
        SHEET,
        sprite,
        rect,
        Draw {
            tint: with_alpha(base, fade),
            rotation: (t * 1.3).sin() * 0.05,
            pivot: vec2(0.5, 0.0),
            ..Default::default()
        },
    );
    pen.text_outline(
        &b.value.to_string(),
        c.x,
        c.y - ry * 0.08,
        r * 0.95,
        with_alpha(WHITE, fade),
        with_alpha(darker(color, 0.55), fade),
    );
}

fn draw_balloons(g: &MathPop, pen: &mut Pen, view: &View) {
    let r = balloon_radius(view);
    let nudge = g.teach && g.asked > 6.0;
    for (i, b) in g.balloons.iter().enumerate() {
        if b.popped {
            continue;
        }
        let c = g.center(view, i);
        if (g.revealed() || nudge) && b.value == g.question.answer {
            let pulse = (g.t * 6.0).sin();
            pen.circle(
                c,
                r * (1.3 + 0.06 * pulse),
                Color::new(1.0, 0.95, 0.4, 0.4 + 0.15 * pulse),
            );
        }
        draw_balloon(pen, c, r, b, g.t + i as f32);
    }
}

fn draw_levels(g: &MathPop, pen: &mut Pen, view: &View) {
    let l = levels_layout(view);
    pen.fill_screen(view, Color::new(0.06, 0.04, 0.16, 0.55));
    pen.painted_panel(l.panel, WHITE);
    pen.ribbon(
        view,
        "Pick a level",
        vec2(view.w / 2.0, l.panel.y + 8.0),
        g.t,
    );
    let open = profile::unlocked(KEY) as usize;
    for (i, r) in l.tiles.iter().enumerate() {
        let locked = i > open;
        let base = if locked {
            Color::from_rgba(200, 196, 214, 255)
        } else if i == g.level {
            Color::from_rgba(255, 214, 90, 255)
        } else {
            Color::from_rgba(255, 150, 200, 255)
        };
        pen.round_rect(
            Rect::new(r.x, r.y + 5.0, r.w, r.h),
            16.0,
            darker(base, 0.65),
        );
        pen.round_rect(*r, 16.0, base);
        pen.text_outline(
            &(i + 1).to_string(),
            r.center().x,
            r.y + r.h * 0.33,
            r.h * 0.36,
            WHITE,
            with_alpha(darker(base, 0.45), if locked { 0.6 } else { 1.0 }),
        );
        if locked {
            padlock(pen, vec2(r.center().x, r.y + r.h * 0.74), r.h * 0.2);
        } else {
            star_row(
                pen,
                vec2(r.center().x, r.y + r.h * 0.76),
                r.h * 0.12,
                r.h * 0.3,
                profile::stars(KEY, i as u32),
                3,
            );
        }
    }
    pen.text_center(
        LEVEL_NAMES[g.level],
        view.w / 2.0,
        l.back.y - 26.0,
        30.0,
        BODY,
    );
    pen.painted_button(l.back, "Back", Tone::Green, 0.0, 1.0);
}

fn draw_done(g: &MathPop, pen: &mut Pen, view: &View, age: f32) {
    let l = done_layout(view);
    let fade = ease_out_cubic(age / 0.2);
    pen.fill_screen(view, Color::new(0.06, 0.04, 0.16, 0.5 * fade));
    let s = ease_out_back(age / 0.32).max(0.01);
    let c = l.panel.center();
    let p = scaled(l.panel, s);
    pen.painted_panel(p, WHITE);
    pen.ribbon(view, "Level done!", vec2(c.x, p.y + 8.0 * s), age);
    let name = format!("{}: {}", g.level + 1, LEVEL_NAMES[g.level]);
    pen.text_center(&name, c.x, p.y + 100.0 * s, 32.0 * s, BODY);
    let have = g.finished_stars();
    let pop = |i: u8| ease_out_back(((age - 0.25 - i as f32 * 0.18) / 0.3).clamp(0.0, 1.0));
    for i in 0..3u8 {
        let x = c.x + (i as f32 - 1.0) * 100.0 * s;
        let lit = i < have;
        let k = if lit { pop(i) } else { 1.0 };
        star_pair(pen, vec2(x, p.y + 190.0 * s), 46.0 * s * k, lit);
    }
    let a = ((age - 0.3) / 0.2).clamp(0.0, 1.0);
    pen.text_center(
        &g.session.summary(),
        c.x,
        p.y + 262.0 * s,
        24.0 * s,
        BODY.with_alpha(a),
    );
    let pulse = 1.0 + (age * 4.5).sin() * 0.03;
    pen.painted_button(scaled(l.next, pulse), "Next level", Tone::Green, 0.0, a);
    pen.painted_button(l.again, "Again", Tone::Orange, 0.0, a);
    pen.painted_button(l.levels, "Levels", Tone::Blue, 0.0, a);
}

pub fn draw(g: &MathPop, pen: &mut Pen, view: &View) {
    draw_backdrop(g, pen, view);
    draw_hud(g, pen, view);
    draw_question(g, pen, view);
    draw_balloons(g, pen, view);
    pen.particles(&g.fx);
    pen.popups(&g.popups);
    if g.teach && g.screen == Screen::Play && !g.solved {
        let pulse = 1.0 + (g.t * 4.0).sin() * 0.03;
        pen.hint(
            "Pop the balloon with the answer!",
            vec2(view.w / 2.0, view.h - 56.0),
            pulse,
        );
    }
    match g.screen {
        Screen::Levels => draw_levels(g, pen, view),
        Screen::Done(age) => draw_done(g, pen, view, age),
        Screen::Final(age) => {
            let total = profile::total_stars(KEY);
            let line = format!("{total} of {} stars", LEVEL_NAMES.len() * 3);
            pen.results(
                view,
                &crate::ui::Results {
                    game: KEY,
                    title: "You did it!",
                    line: &line,
                    rank: g.rank,
                    action: "Play again",
                    format: plural_stars,
                },
                age,
            );
        }
        Screen::Play => {}
    }
}

impl Pen {
    fn cloud(&mut self, i: usize, c: Vec2, w: f32, tint: Color) {
        let name = [
            "cloud1", "cloud2", "cloud3", "cloud4", "cloud5", "cloud6", "cloud7", "cloud8",
        ][i % 8];
        self.sprite_w("ui", name, c, w, Draw::tint(tint));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paint::Tex;

    fn fresh() -> MathPopPlay {
        kg_core::save::reset_for_test();
        kg_core::profile::create("Mia", 0);
        MathPopPlay::new()
    }

    fn drawn(p: &MathPopPlay, v: &View) -> Pen {
        let mut pen = Pen::default();
        pen.begin(*v, 1.0);
        p.draw(&mut pen, v);
        pen
    }

    fn texs(pen: &Pen) -> Vec<Tex> {
        pen.used().iter().filter_map(|b| b.tex).collect()
    }

    #[test]
    fn the_question_screen_draws_backdrop_balloons_text_and_ui() {
        let v = View::ipad_landscape();
        let mut p = fresh();
        for _ in 0..60 {
            p.update(&Input::default(), &v, 1.0 / 60.0);
        }
        let pen = drawn(&p, &v);
        let t = texs(&pen);
        assert!(t.iter().any(|t| matches!(t, Tex::Page("mathpop", _))));
        assert!(t.iter().any(|t| matches!(t, Tex::Page("ui", _))));
        assert!(t.contains(&Tex::Glyphs));
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
        assert!(
            lo <= f.x + 0.01 && hi >= f.x + f.w,
            "bg {lo}..{hi} vs {f:?}"
        );
        assert!(top <= f.y + 0.5 && bot >= f.y + f.h - 0.5);
    }

    #[test]
    fn the_level_picker_and_done_screens_draw_over_the_game() {
        let v = View::ipad_portrait();
        let mut p = fresh();
        let base = drawn(&p, &v).vertex_count();
        p.g.screen = Screen::Levels;
        assert!(drawn(&p, &v).vertex_count() > base + 200);
        p.g.screen = Screen::Done(1.0);
        assert!(drawn(&p, &v).vertex_count() > base + 200);
    }

    #[test]
    fn the_final_screen_asks_for_the_board_once() {
        let v = View::ipad_landscape();
        let mut p = fresh();
        p.g.screen = Screen::Final(0.0);
        p.update(&Input::default(), &v, 1.0 / 60.0);
        assert!(p.final_shown);
        let pen = drawn(&p, &v);
        assert!(pen.vertex_count() > 500);
    }

    #[test]
    fn tapping_a_balloon_pops_it_and_adds_particles() {
        let v = View::ipad_landscape();
        let mut p = fresh();
        for _ in 0..90 {
            p.update(&Input::default(), &v, 1.0 / 60.0);
        }
        let slot = p.g.question.slot_of_answer();
        let at = p.g.center(&v, slot);
        let before = drawn(&p, &v).vertex_count();
        p.update(&Input::press_at(at), &v, 1.0 / 60.0);
        assert!(p.g.solved);
        assert!(drawn(&p, &v).vertex_count() > before);
    }
}
