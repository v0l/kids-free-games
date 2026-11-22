use super::Play;
use crate::draw::Draw;
use crate::paint::Pen;
use crate::ui::{Tone, BODY};
use glam::{vec2, Vec2};
use kg_core::engine::{Game, Input, View};
use kg_core::fx::ease_out_back;
use kg_core::games::phonics::*;
use kg_core::geom::{darker, with_alpha, Color, Rect, WHITE};
use kg_core::learn::{hud_bottom, ui_scale};
use kg_core::profile;

pub const KEY: &str = "phonics";
const SHEET: &str = "phonics";
const PIECES: &str = "mathpop";
const GOLD: Color = Color::new(1.0, 0.82, 0.24, 1.0);
const SKY: Color = Color::new(0.31, 0.67, 0.94, 1.0);

pub struct PhonicsPlay {
    pub g: Phonics,
    final_shown: bool,
}

impl PhonicsPlay {
    pub fn new() -> Self {
        Self {
            g: Phonics::new(),
            final_shown: false,
        }
    }
}

impl Default for PhonicsPlay {
    fn default() -> Self {
        Self::new()
    }
}

impl Play for PhonicsPlay {
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

fn draw_bunnies(g: &Phonics, pen: &mut Pen, view: &View) {
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
            PIECES,
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

fn draw_backdrop(g: &Phonics, pen: &mut Pen, view: &View) {
    let f = view.full();
    let side = f.w.max(f.h);
    pen.sprite(
        SHEET,
        "bg",
        Rect::new(f.x + (f.w - side) / 2.0, f.y + f.h - side, side, side),
        Draw::default(),
    );
    for i in 0..4usize {
        let span = view.w + 500.0;
        let x = (i as f32 * 0.27 * span + g.t * (7.0 + i as f32 * 3.0)) % span - 250.0;
        let y = hud_bottom(view) + 20.0 + (i as f32 * 0.37 % 1.0) * view.h * 0.35;
        pen.sprite_w(
            "ui",
            ["cloud1", "cloud3", "cloud5", "cloud7"][i],
            vec2(x, y),
            150.0 + (i % 3) as f32 * 50.0,
            Draw::tint(with_alpha(WHITE, 0.85)),
        );
    }
    draw_bunnies(g, pen, view);
}

fn draw_hud(g: &Phonics, pen: &mut Pen, view: &View) {
    let h = hud(view);
    let label = format!("Level {}: {}", g.level + 1, LEVEL_NAMES[g.level]);
    pen.hud_title(view, &h, &label, darker(SKY, 0.6));
    pen.painted_button(levels_button(view), "Levels", Tone::Blue, g.press, 1.0);
    let bar = h.progress.unwrap_or_default();
    let n = QUESTIONS;
    let gap = (bar.h * 0.45).max(4.0);
    let seg = (bar.w - gap * (n as f32 - 1.0)) / n as f32;
    for j in 0..n {
        let r = Rect::new(bar.x + j as f32 * (seg + gap), bar.y, seg, bar.h);
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

fn draw_question(g: &Phonics, pen: &mut Pen, view: &View) {
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
    let icon = base.h * 0.62 * pop;
    let pulse = 1.0 + 0.08 * (g.t * 5.0).sin().max(0.0) * (g.since_said < 1.2) as i32 as f32;
    let words = if g.solved {
        g.question.target.to_string()
    } else {
        "Which sound?".to_string()
    };
    let text_size = kg_core::sizing::fit_size(&words, size * 0.62 * pop, base.w - icon * 2.0);
    pen.ui(
        "icon_audio",
        Rect::new(
            base.x + base.h * 0.25 - icon * (pulse - 1.0) / 2.0,
            c.y - icon * pulse / 2.0,
            icon * pulse,
            icon * pulse,
        ),
        Draw::tint(darker(SKY, 0.6)),
    );
    pen.text_center(&words, c.x + icon * 0.4, c.y, text_size, ink);
}

fn draw_balloon(pen: &mut Pen, c: Vec2, r: f32, b: &Balloon, t: f32) {
    let (sprite, color) = BALLOON_COLORS[b.color];
    let fade = if b.tried { 0.5 } else { 1.0 };
    let squash = 1.0 + 0.06 * (b.wobble * 30.0).sin() * b.wobble;
    let ry = r / squash;
    let height = ry * 3.1;
    let s = crate::art::size(PIECES, sprite);
    let width = height * s.x / s.y * squash;
    let rect = Rect::new(c.x - width / 2.0, c.y - ry, width, height);
    let base = if b.tried {
        Color::new(0.8, 0.8, 0.85, 1.0)
    } else {
        WHITE
    };
    pen.sprite(
        PIECES,
        sprite,
        rect,
        Draw {
            tint: with_alpha(base, fade),
            rotation: (t * 1.3).sin() * 0.05,
            pivot: vec2(0.5, 0.0),
            ..Default::default()
        },
    );
    let size = if b.value.len() > 1 { r * 0.8 } else { r * 1.05 };
    pen.text_outline(
        b.value,
        c.x,
        c.y - ry * 0.08,
        size,
        with_alpha(WHITE, fade),
        with_alpha(darker(color, 0.55), fade),
    );
}

fn draw_balloons(g: &Phonics, pen: &mut Pen, view: &View) {
    let r = balloon_radius(view);
    let nudge = g.teach && g.asked > 6.0;
    for (i, b) in g.balloons.iter().enumerate() {
        if b.popped {
            continue;
        }
        let c = g.center(view, i);
        if (g.revealed() || nudge) && b.value == g.question.target {
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

fn draw_levels(g: &Phonics, pen: &mut Pen, view: &View) {
    let l = levels_layout(view);
    pen.fill_screen(view, Color::new(0.06, 0.04, 0.16, 0.55));
    pen.painted_panel(l.sheet.panel, WHITE);
    if let Some((rr, _)) = l.sheet.ribbon {
        pen.ribbon_in(view, LEVELS_TITLE, rr, g.t);
    }
    let open = profile::unlocked(KEY) as usize;
    for (i, r) in l.tiles.iter().enumerate() {
        let locked = i > open;
        let base = if locked {
            Color::from_rgba(200, 196, 214, 255)
        } else if i == g.level {
            Color::from_rgba(255, 214, 90, 255)
        } else {
            Color::from_rgba(120, 190, 250, 255)
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
    let name = LEVEL_NAMES[g.level];
    let cs = kg_core::sizing::fit_size(name, view.text(30.0).min(l.caption.h * 0.8), l.caption.w);
    pen.text_center(name, l.caption.center().x, l.caption.center().y, cs, BODY);
    pen.painted_button(l.back, "Back", Tone::Green, 0.0, 1.0);
}

fn draw_done(g: &Phonics, pen: &mut Pen, view: &View, age: f32) {
    let l = done_layout(view);
    let name = format!("{}: {}", g.level + 1, LEVEL_NAMES[g.level]);
    let summary = g.session.summary();
    let (pop, k) = pen.done_sheet(
        view,
        &l.done,
        DONE_TITLE,
        &name,
        &summary,
        &DONE_LABELS,
        age,
    );
    let st = pop.rect(l.done.stars());
    let have = g.finished_stars();
    let popk = |i: u8| ease_out_back(((age - 0.25 - i as f32 * 0.18) / 0.3).clamp(0.0, 1.0));
    let r = (46.0 * k).min(st.h * 0.42);
    for i in 0..3u8 {
        let x = st.center().x + (i as f32 - 1.0) * r * 2.2;
        let lit = i < have;
        let kk = if lit { popk(i) } else { 1.0 };
        star_pair(pen, vec2(x, st.center().y), r * kk, lit);
    }
}

pub fn draw(g: &Phonics, pen: &mut Pen, view: &View) {
    draw_backdrop(g, pen, view);
    draw_hud(g, pen, view);
    draw_question(g, pen, view);
    draw_balloons(g, pen, view);
    pen.particles(&g.fx);
    pen.popups(&g.popups);
    if g.teach && g.screen == Screen::Play && !g.solved {
        let pulse = 1.0 + (g.t * 4.0).sin() * 0.03;
        pen.hint(
            "Pop the balloon with the sound you hear!",
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paint::Tex;

    fn fresh() -> PhonicsPlay {
        kg_core::save::reset_for_test();
        kg_core::profile::create("Mia", 0);
        PhonicsPlay::new()
    }

    fn drawn(p: &PhonicsPlay, v: &View) -> Pen {
        let mut pen = Pen::default();
        pen.begin(*v, 1.0);
        p.draw(&mut pen, v);
        pen
    }

    #[test]
    fn the_play_screen_draws_backdrop_balloons_text_and_ui() {
        let v = View::ipad_landscape();
        let mut p = fresh();
        for _ in 0..60 {
            p.update(&Input::default(), &v, 1.0 / 60.0);
        }
        let pen = drawn(&p, &v);
        let t: Vec<Tex> = pen.used().iter().filter_map(|b| b.tex).collect();
        assert!(t.iter().any(|t| matches!(t, Tex::Page("phonics", _))));
        assert!(t.iter().any(|t| matches!(t, Tex::Page("mathpop", _))));
        assert!(t.iter().any(|t| matches!(t, Tex::Page("ui", _))));
        assert!(t.contains(&Tex::Glyphs));
    }

    #[test]
    fn every_screen_draws_at_phone_and_desktop_sizes() {
        for v in [
            View::ipad_landscape(),
            View::ipad_portrait(),
            View::fit(390.0, 844.0),
            View::fit(1920.0, 1080.0),
        ] {
            let mut p = fresh();
            let base = drawn(&p, &v).vertex_count();
            p.g.screen = Screen::Levels;
            assert!(drawn(&p, &v).vertex_count() > base + 200);
            p.g.screen = Screen::Done(1.0);
            assert!(drawn(&p, &v).vertex_count() > base + 200);
            p.g.screen = Screen::Final(0.0);
            assert!(drawn(&p, &v).vertex_count() > 500);
        }
    }

    #[test]
    fn the_final_screen_asks_for_the_board_once() {
        let v = View::ipad_landscape();
        let mut p = fresh();
        p.g.screen = Screen::Final(0.0);
        p.update(&Input::default(), &v, 1.0 / 60.0);
        assert!(p.final_shown);
    }

    #[test]
    fn tapping_a_balloon_pops_it() {
        let v = View::ipad_landscape();
        let mut p = fresh();
        for _ in 0..90 {
            p.update(&Input::default(), &v, 1.0 / 60.0);
        }
        let slot = p.g.question.slot_of_answer();
        let at = p.g.center(&v, slot);
        p.update(&Input::press_at(at), &v, 1.0 / 60.0);
        assert!(p.g.solved);
    }
}
