use super::Play;
use crate::draw::Draw;
use crate::paint::Pen;
use crate::ui::{Results, Tone, BODY};
use glam::{vec2, Vec2};
use kg_core::engine::{Game, Input, View, HUD_LEFT};
use kg_core::fx::{ease_out_back, ease_out_cubic};
use kg_core::games::spell::*;
use kg_core::geom::{darker, with_alpha, Color, Rect, WHITE};
use kg_core::learn::ui_scale;
use kg_core::profile;

pub const KEY: &str = kg_core::games::spell::KEY;
pub const SHEET: &str = "spell";
const GOLD: Color = Color::new(1.0, 0.82, 0.24, 1.0);

pub struct SpellPlay {
    pub g: Spell,
}

impl SpellPlay {
    pub fn new() -> Self {
        Self { g: Spell::new() }
    }
}

impl Default for SpellPlay {
    fn default() -> Self {
        Self::new()
    }
}

impl Play for SpellPlay {
    fn update(&mut self, input: &Input, view: &View, dt: f32) {
        self.g.update(input, view, dt);
    }

    fn draw(&self, pen: &mut Pen, view: &View) {
        draw(&self.g, pen, view);
    }

    fn over(&self) -> bool {
        matches!(self.g.screen, Screen::Final(_))
    }

    fn score(&self) -> u32 {
        profile::total_stars(KEY)
    }

    fn as_any(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

fn plural_stars(n: i64) -> String {
    if n == 1 {
        "1 star".to_string()
    } else {
        format!("{n} stars")
    }
}

fn star_row(pen: &mut Pen, c: Vec2, r: f32, gap: f32, have: u8, total: u8, pop: f32) {
    for i in 0..total {
        let x = c.x + (i as f32 - (total as f32 - 1.0) / 2.0) * gap;
        let lit = i < have;
        let k = if lit { pop } else { 1.0 };
        pen.star(
            vec2(x, c.y + 2.0),
            r * 1.08 * k,
            0.0,
            Color::new(0.1, 0.05, 0.25, 0.3),
        );
        pen.star(
            vec2(x, c.y),
            r * k,
            0.0,
            if lit {
                GOLD
            } else {
                Color::new(0.75, 0.72, 0.82, 0.8)
            },
        );
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

fn tile(pen: &mut Pen, r: Rect, ch: char, color: Color, alpha: f32) {
    pen.sprite(
        SHEET,
        "tile",
        Rect::new(r.x, r.y + r.h * 0.06, r.w, r.h),
        Draw::tint(Color::new(0.0, 0.0, 0.0, 0.2 * alpha)),
    );
    pen.sprite(SHEET, "tile", r, Draw::tint(with_alpha(color, alpha)));
    pen.text_outline(
        &ch.to_string(),
        r.x + r.w / 2.0,
        r.y + r.h * 0.47,
        r.h * 0.62,
        with_alpha(WHITE, alpha),
        with_alpha(darker(color, 0.45), alpha),
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
    for i in 0..4usize {
        let span = view.w + 500.0;
        let x = (i as f32 * 0.27 * span + t * (6.0 + i as f32 * 3.0)) % span - 250.0;
        let name = format!("cloud{}", (i * 2) % 8 + 1);
        pen.sprite_w(
            "ui",
            &name,
            vec2(x, view.h * (0.12 + 0.17 * (i as f32 * 0.61 % 1.0))),
            160.0 + (i % 3) as f32 * 50.0,
            Draw::tint(with_alpha(WHITE, 0.85)),
        );
    }
}

fn draw_picture(pen: &mut Pen, word: &str, c: Vec2, s: f32, t: f32) {
    let rotation = match word {
        "sun" => t * 0.4,
        "bee" => (t * 6.0).sin() * 0.08,
        "fish" => (t * 4.0).sin() * 0.08,
        "star" => (t * 2.0).sin() * 0.15,
        "flower" | "kite" => (t * 1.5).sin() * 0.05,
        _ => 0.0,
    };
    let size = crate::art::size(SHEET, word);
    let k = 1.85 * s / size.x.max(size.y);
    let (w, h) = (size.x * k, size.y * k);
    pen.soft_shadow(c + vec2(0.0, h * 0.5), w * 0.38, s * 0.07, 0.5);
    pen.sprite(
        SHEET,
        word,
        Rect::new(c.x - w / 2.0, c.y - h / 2.0, w, h),
        Draw::rot(rotation),
    );
}

pub fn draw(g: &Spell, pen: &mut Pen, view: &View) {
    draw_backdrop(pen, view, g.t);
    let l = layout(view, g.board.word.len(), g.board.tiles.len());
    let k = ui_scale(view);
    pen.offset = g.shake.offset();
    for i in 0..g.deck.len() {
        let c = vec2(
            view.w / 2.0 + (i as f32 - (g.deck.len() as f32 - 1.0) / 2.0) * 44.0 * k,
            l.stars_y,
        );
        let s = if i == g.index && g.results.len() == i {
            1.0 + (g.t * 4.0).sin() * 0.08
        } else {
            1.0
        };
        let tint = match g.results.get(i) {
            Some(true) => WHITE,
            Some(false) => Color::new(0.45, 0.45, 0.5, 0.6),
            None if i == g.index => Color::new(0.7, 0.7, 0.75, 0.9),
            None => Color::new(0.5, 0.5, 0.55, 0.45),
        };
        pen.sprite_at("ui", "p_star", c, 38.0 * k * s, Draw::tint(tint));
    }
    let p = l.picture;
    pen.painted_panel(p, WHITE);
    let bob = (g.t * 2.0).sin() * p.h * 0.015;
    let sc = g.picture_bump.scale(0.12);
    draw_picture(
        pen,
        g.word(),
        p.center() + vec2(0.0, bob),
        p.w.min(p.h) * 0.42 * sc,
        g.t,
    );
    let glow = g.glow();
    let won = match g.phase {
        Phase::Won(t) => Some(t),
        _ => None,
    };
    for (i, r) in l.slots.iter().enumerate() {
        if i >= g.board.filled {
            let next = i == g.board.filled && shows_next_slot(g.level);
            pen.sprite(
                SHEET,
                "tile",
                *r,
                Draw::tint(Color::new(1.0, 1.0, 1.0, if next { 0.7 } else { 0.35 })),
            );
            let line = if next {
                Color::from_rgba(255, 140, 120, 255)
            } else {
                Color::new(0.5, 0.3, 0.4, 0.3)
            };
            let pulse = if next {
                1.0 + (g.t * 5.0).sin() * 0.04
            } else {
                1.0
            };
            pen.rect(
                Rect::new(
                    r.center().x - r.w * 0.3 * pulse,
                    r.y + r.h * 0.8,
                    r.w * 0.6 * pulse,
                    6.0,
                ),
                line,
            );
            continue;
        }
        let wave = won
            .map(|t| ((t * 9.0 - i as f32 * 0.9).sin().max(0.0)) * r.h * 0.25)
            .unwrap_or(0.0);
        let pop = g.pop.get(i).copied().unwrap_or(0.0);
        let s = 1.0 + pop * 0.25;
        let rr = Rect::new(
            r.center().x - r.w * s / 2.0,
            r.center().y - r.h * s / 2.0 - wave,
            r.w * s,
            r.h * s,
        );
        tile(
            pen,
            rr,
            g.board.word[i].to_ascii_uppercase(),
            letter_color(i),
            1.0,
        );
    }
    for (i, r) in l.tiles.iter().enumerate() {
        if let Some(slot) = g.board.used[i] {
            let k2 = ease_out_cubic(g.slide[i]);
            if k2 < 1.0 {
                let to = l.slots[slot];
                let at = Rect::new(
                    r.x + (to.x - r.x) * k2,
                    r.y + (to.y - r.y) * k2,
                    r.w + (to.w - r.w) * k2,
                    r.h + (to.h - r.h) * k2,
                );
                tile(
                    pen,
                    at,
                    g.board.tiles[i].to_ascii_uppercase(),
                    letter_color(slot),
                    1.0,
                );
            }
            let inner = Rect::new(r.x + r.w * 0.1, r.y + r.h * 0.1, r.w * 0.8, r.h * 0.8);
            pen.round_rect(inner, r.w * 0.18, Color::new(0.55, 0.3, 0.4, 0.12));
            continue;
        }
        let mut rr = *r;
        let w = g.wobble[i];
        rr.x += (w * 30.0).sin() * w * 14.0;
        if glow == Some(i) {
            let gl = 8.0 + (g.t * 6.0).sin().abs() * 8.0;
            pen.round_rect(
                Rect::new(rr.x - gl, rr.y - gl, rr.w + gl * 2.0, rr.h + gl * 2.0),
                rr.w * 0.3,
                Color::new(1.0, 0.95, 0.4, 0.7),
            );
        }
        let col = if w > 0.0 {
            Color::from_rgba(240, 90, 90, 255)
        } else {
            Color::from_rgba(110, 140, 240, 255)
        };
        tile(pen, rr, g.board.tiles[i].to_ascii_uppercase(), col, 1.0);
    }
    if g.hint_available() {
        let h = l.hint;
        let hp = 1.0
            + if g.hint_on.is_some() {
                0.0
            } else {
                (g.t * 2.0).sin() * 0.03
            };
        let hr = Rect::new(
            h.center().x - h.w * hp / 2.0,
            h.center().y - h.h * hp / 2.0,
            h.w * hp,
            h.h * hp,
        );
        pen.painted_round(hr, "icon_question", 0.0);
        pen.text_outline(
            &g.hints_left.to_string(),
            h.x + h.w * 0.9,
            h.y + h.h * 0.12,
            h.h * 0.34,
            WHITE,
            Color::from_rgba(150, 60, 20, 255),
        );
    }
    pen.particles(&g.fx);
    pen.popups(&g.pops);
    draw_hud(g, pen, view, &l);
    pen.offset = Vec2::ZERO;
    match g.screen {
        Screen::Levels => draw_levels(g, pen, view),
        Screen::Done(age) => draw_done(g, pen, view, age),
        Screen::Final(age) => {
            let total = profile::total_stars(KEY);
            let line = format!("{total} of {} stars", LEVELS.len() * 3);
            pen.results(
                view,
                &Results {
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

fn draw_hud(g: &Spell, pen: &mut Pen, view: &View, l: &Layout) {
    let k = ui_scale(view);
    let label = format!("Level {}", g.level + 1);
    let tw = crate::draw::measure(&label, 28.0 * k) + 44.0 * k;
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
        Color::from_rgba(150, 60, 30, 255),
    );
    pen.painted_button(levels_button(view), "Levels", Tone::Blue, g.press, 1.0);
    if g.teach && g.screen == Screen::Play && g.phase == Phase::Playing {
        let pulse = 1.0 + (g.t * 4.0).sin() * 0.03;
        pen.hint(
            "Tap the letters to spell the word!",
            vec2(view.w / 2.0, l.stars_y + 44.0 * k),
            pulse,
        );
    } else {
        let goal = format!("{}: spell {} words", LEVELS[g.level].name, g.deck.len());
        pen.text_outline(
            &goal,
            view.w / 2.0,
            l.stars_y + 40.0 * k,
            26.0 * k,
            WHITE,
            Color::from_rgba(120, 60, 40, 255),
        );
    }
}

fn draw_levels(g: &Spell, pen: &mut Pen, view: &View) {
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
        let current = i == g.level;
        let base = if locked {
            Color::from_rgba(200, 196, 214, 255)
        } else if current {
            Color::from_rgba(255, 214, 90, 255)
        } else {
            Color::from_rgba(255, 150, 120, 255)
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
                1.0,
            );
        }
    }
    pen.text_center(
        LEVELS[g.level].name,
        view.w / 2.0,
        l.back.y - 26.0,
        30.0,
        BODY,
    );
    pen.painted_button(l.back, "Back", Tone::Green, 0.0, 1.0);
}

fn draw_done(g: &Spell, pen: &mut Pen, view: &View, age: f32) {
    let l = done_layout(view);
    let fade = ease_out_cubic(age / 0.2);
    pen.fill_screen(view, Color::new(0.06, 0.04, 0.16, 0.5 * fade));
    let s = ease_out_back(age / 0.32).max(0.01);
    let c = l.panel.center();
    let p = Rect::new(
        c.x - l.panel.w * s / 2.0,
        c.y - l.panel.h * s / 2.0,
        l.panel.w * s,
        l.panel.h * s,
    );
    pen.painted_panel(p, WHITE);
    pen.ribbon(view, "Level done!", vec2(c.x, p.y + 8.0 * s), age);
    let name = format!("{}: {}", g.level + 1, LEVELS[g.level].name);
    pen.text_center(&name, c.x, p.y + 100.0 * s, 32.0 * s, BODY);
    let have = stars_for(g.slips);
    let pop = |i: u8| ease_out_back(((age - 0.25 - i as f32 * 0.18) / 0.3).clamp(0.0, 1.0));
    for i in 0..3u8 {
        let x = c.x + (i as f32 - 1.0) * 100.0 * s;
        let lit = i < have;
        let kk = if lit { pop(i) } else { 1.0 };
        let r = 46.0 * s * kk;
        pen.star(
            vec2(x, p.y + 190.0 * s + 3.0),
            r * 1.05,
            0.0,
            Color::new(0.1, 0.05, 0.25, 0.3),
        );
        pen.star(
            vec2(x, p.y + 190.0 * s),
            r,
            0.0,
            if lit {
                GOLD
            } else {
                Color::new(0.75, 0.72, 0.82, 0.8)
            },
        );
    }
    let a = ((age - 0.3) / 0.2).clamp(0.0, 1.0);
    let pulse = 1.0 + (age * 4.5).sin() * 0.03;
    let nb = l.next;
    let nb = Rect::new(
        nb.center().x - nb.w * pulse / 2.0,
        nb.center().y - nb.h * pulse / 2.0,
        nb.w * pulse,
        nb.h * pulse,
    );
    pen.painted_button(nb, "Next level", Tone::Green, 0.0, a);
    pen.painted_button(l.again, "Again", Tone::Orange, 0.0, a);
    pen.painted_button(l.levels, "Levels", Tone::Blue, 0.0, a);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paint::Tex;

    fn started() -> SpellPlay {
        kg_core::save::reset_for_test();
        kg_core::profile::create("Mia", 0);
        SpellPlay::new()
    }

    fn drawn(p: &SpellPlay, v: &View) -> Pen {
        let mut pen = Pen::default();
        pen.begin(*v, 1.0);
        p.draw(&mut pen, v);
        pen
    }

    #[test]
    fn the_play_screen_draws_the_picture_tiles_and_text() {
        let v = View::ipad_landscape();
        let p = started();
        let pen = drawn(&p, &v);
        let texs: Vec<_> = pen.used().iter().filter_map(|b| b.tex).collect();
        assert!(texs.iter().any(|t| matches!(t, Tex::Page("spell", _))));
        assert!(texs.contains(&Tex::Glyphs));
        assert!(texs.iter().any(|t| matches!(t, Tex::Page("ui", _))));
    }

    #[test]
    fn every_screen_draws_in_both_orientations() {
        for v in [View::ipad_landscape(), View::ipad_portrait()] {
            for screen in [
                Screen::Play,
                Screen::Levels,
                Screen::Done(1.0),
                Screen::Final(1.0),
            ] {
                let mut p = started();
                p.g.screen = screen;
                let pen = drawn(&p, &v);
                assert!(pen.vertex_count() > 200, "{screen:?}");
            }
        }
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
        let p = started();
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
}
