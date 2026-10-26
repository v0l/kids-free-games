use super::Play;
use crate::draw::Draw;
use crate::paint::Pen;
use crate::ui::{BODY, INK};
use glam::{vec2, Vec2};
use kg_core::engine::{Game, Input, View};
use kg_core::fx::{ease_out_back, ease_out_cubic, Side};
use kg_core::games::memory::*;
use kg_core::geom::{Color, Rect, WHITE};

pub const SHEET: &str = "memory";
pub const KEY: &str = kg_core::games::memory::KEY;

pub struct MemoryPlay {
    pub g: Memory,
}

impl MemoryPlay {
    pub fn new() -> Self {
        Self { g: Memory::new() }
    }
}

impl MemoryPlay {
    fn shows_done(&self) -> bool {
        self.g.won() && self.g.celebrated >= DONE_DELAY
    }
}

impl Default for MemoryPlay {
    fn default() -> Self {
        Self::new()
    }
}

impl Play for MemoryPlay {
    fn update(&mut self, input: &Input, view: &View, dt: f32) {
        let showing = self.shows_done();
        self.g.update(input, view, dt);
        if !showing && self.shows_done() && self.g.level >= MAX_LEVEL {
            kg_core::net::request_board(KEY);
        }
    }

    fn draw(&self, pen: &mut Pen, view: &View) {
        draw(&self.g, pen, view);
    }

    fn over(&self) -> bool {
        self.g.won()
    }

    fn as_any(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

const GOLD: Color = Color::from_rgba(255, 220, 70, 255);
const DIM_STAR: Color = Color::new(0.25, 0.2, 0.4, 0.45);

fn star_row(pen: &mut Pen, c: Vec2, size: f32, lit: f32, scale: f32) {
    for i in 0..3 {
        let x = c.x + (i as f32 - 1.0) * size * 1.15;
        let on = (i as f32) < lit;
        pen.sprite_w(
            "ui",
            "p_star",
            vec2(x, c.y),
            size * if on { scale } else { 1.0 },
            Draw::tint(if on { WHITE } else { DIM_STAR }),
        );
    }
}

fn fit_card(r: Rect, aspect: f32) -> Rect {
    let w = r.w.min(r.h * aspect);
    Rect::new(r.center().x - w / 2.0, r.y, w, r.h)
}

fn draw_card(g: &Memory, pen: &mut Pen, i: usize, r: Rect, shake: Vec2, pulse: f32) {
    let back = crate::art::size(SHEET, "back");
    let r = fit_card(r, back.x / back.y);
    let (w, lift) = flip_shape(g.flip[i]);
    let matched = g.cards[i].1 == Face::Matched;
    let pop = if matched { g.pop[i].scale(0.18) } else { pulse };
    let c = r.center() + shake + vec2(wobble_offset(g.wobble[i]), -lift * 14.0);
    let hs = 1.0 + 0.1 * lift;
    let rr = Rect::new(
        c.x - r.w * w * pop / 2.0,
        c.y - r.h * hs * pop / 2.0,
        r.w * w * pop,
        r.h * hs * pop,
    );
    pen.soft_shadow(
        vec2(r.center().x + shake.x, r.y + r.h - 2.0 + shake.y),
        r.w * 0.5 * (0.4 + 0.6 * w),
        r.h * 0.04,
        0.6 - lift * 0.3,
    );
    if g.flip[i] < 0.5 {
        pen.sprite(SHEET, "back", rr, Draw::default());
        return;
    }
    let tint = if matched {
        Color::from_rgba(214, 255, 214, 255)
    } else {
        WHITE
    };
    pen.sprite(SHEET, "face", rr, Draw::tint(tint));
    if w > 0.2 {
        let bounce = if matched { (g.t * 4.0 + i as f32).sin() * 3.0 } else { 0.0 };
        let d = r.w * 0.82;
        pen.sprite_w(
            SHEET,
            ANIMALS[g.cards[i].0 as usize % MAX_PAIRS],
            vec2(rr.x + rr.w / 2.0, rr.y + rr.h / 2.0 + bounce),
            d * w,
            Draw::default(),
        );
        if matched {
            pen.shine_sweep(rr, g.t * 1.5 + i as f32 * 0.2);
        }
    }
}

fn draw_hud(g: &Memory, pen: &mut Pen, view: &View) {
    pen.chip(view, Side::Left, "MOVES", &g.moves.to_string(), g.moves_bump.t);
    pen.painted_button(Memory::levels_button(view), "Levels", crate::ui::Tone::Blue, 0.0, 1.0);
    let lit = stars_for(g.pairs, g.moves);
    star_row(pen, vec2(view.w / 2.0, 34.0), 34.0, lit as f32, g.stars_bump.scale(0.3));
    pen.text_outline(
        &format!("{} of {} pairs", g.matched_pairs(), g.pairs),
        view.w / 2.0,
        68.0,
        24.0,
        WHITE,
        INK,
    );
}

fn draw_hint(g: &Memory, pen: &mut Pen, view: &View) {
    if !g.teach || g.won() {
        return;
    }
    let label = if g.wait > 0.0 {
        "Not a match. Try again!"
    } else if g.up().len() == 1 {
        "Now find its twin!"
    } else {
        "Tap a card to flip it"
    };
    let pulse = 1.0 + 0.04 * (g.t * 4.0).sin();
    pen.hint(label, vec2(view.w / 2.0, view.h - 40.0), pulse);
}

fn draw_tile(g: &Memory, pen: &mut Pen, i: usize, r: Rect) {
    let level = i as u32 + 1;
    let open = level <= g.progress.unlocked;
    let current = level == g.progress.resume;
    let bob = if current { (g.t * 4.0).sin() * 3.0 } else { 0.0 };
    let r = Rect::new(r.x, r.y + bob, r.w, r.h);
    let tint = if open { WHITE } else { Color::new(0.7, 0.68, 0.8, 1.0) };
    pen.painted_panel(r, tint);
    pen.sprite_w(
        SHEET,
        ANIMALS[i % MAX_PAIRS],
        vec2(r.center().x, r.y + r.h * 0.28),
        r.w * 0.62,
        Draw::tint(if open { WHITE } else { Color::new(0.25, 0.2, 0.35, 0.7) }),
    );
    pen.text_outline(
        &level.to_string(),
        r.center().x,
        r.y + r.h * 0.55,
        r.w * 0.3,
        Color::from_rgba(255, 200, 40, 255),
        INK,
    );
    pen.text_center(
        &format!("{} pairs", pairs_for(level)),
        r.center().x,
        r.y + r.h * 0.69,
        (r.w * 0.125).max(16.0),
        BODY,
    );
    if open {
        star_row(
            pen,
            vec2(r.center().x, r.y + r.h * 0.86),
            r.w * 0.24,
            g.progress.stars[i] as f32,
            1.0,
        );
    } else {
        let s = r.w * 0.26;
        pen.ui(
            "icon_lock",
            Rect::new(r.center().x - s / 2.0, r.y + r.h * 0.86 - s / 2.0, s, s),
            Draw::tint(Color::from_rgba(120, 100, 150, 255)),
        );
    }
}

fn draw_select(g: &Memory, pen: &mut Pen, view: &View) {
    let tiles = Memory::tiles(view);
    pen.ribbon(view, "Memory Match", vec2(view.w / 2.0, tiles[0].y - 70.0), g.t);
    pen.chip(
        view,
        Side::Right,
        "STARS",
        &format!("{}/{}", g.progress.total(), LEVELS.len() * 3),
        1.0,
    );
    for (i, r) in tiles.iter().enumerate() {
        draw_tile(g, pen, i, *r);
    }
    let play = Memory::play_button(view);
    let pulse = 1.0 + (g.t * 4.5).sin() * 0.03;
    let grown = Rect::new(
        play.center().x - play.w * pulse / 2.0,
        play.center().y - play.h * pulse / 2.0,
        play.w * pulse,
        play.h * pulse,
    );
    pen.painted_button(
        grown,
        &format!("Play level {}", g.progress.resume),
        crate::ui::Tone::Green,
        0.0,
        1.0,
    );
    pen.popups(&g.popups);
}

fn draw_done(g: &Memory, pen: &mut Pen, view: &View, age: f32) {
    if g.level >= MAX_LEVEL {
        let line = format!("{} pairs, {} moves, {} stars", g.pairs, g.moves, g.stars);
        pen.results(
            view,
            &crate::ui::Results {
                game: KEY,
                title: "All done!",
                line: &line,
                rank: g.rank,
                action: "Play again",
                format: moves_label,
            },
            age,
        );
        return;
    }
    pen.fill_screen(view, Color::new(0.06, 0.04, 0.16, 0.55 * ease_out_cubic(age / 0.2)));
    let p = Memory::done_panel(view);
    let s = ease_out_back(age / 0.32).max(0.01);
    let c = p.center() + vec2(0.0, (1.0 - ease_out_cubic(age / 0.32)) * 90.0);
    let sp = Rect::new(c.x - p.w * s / 2.0, c.y - p.h * s / 2.0, p.w * s, p.h * s);
    pen.painted_panel(sp, WHITE);
    pen.ribbon(view, &format!("Level {} clear!", g.level), vec2(c.x, sp.y + 8.0 * s), age);
    let size = 100.0 * s;
    for i in 0..3 {
        let x = c.x + (i as f32 - 1.0) * size * 1.15;
        let y = c.y - p.h * 0.5 * s + 170.0 * s;
        let earned = i < g.stars as usize;
        let k = ease_out_back(((age - 0.3 - i as f32 * 0.25) / 0.3).clamp(0.0, 1.0));
        pen.sprite_w(
            "ui",
            "p_star",
            vec2(x, y),
            size * if earned { k } else { 1.0 },
            Draw {
                tint: if earned { WHITE } else { Color::new(0.25, 0.2, 0.4, 0.4) },
                rotation: if earned { (1.0 - k) * 0.8 } else { 0.0 },
                ..Default::default()
            },
        );
    }
    let a = |d: f32| ((age - d) / 0.2).clamp(0.0, 1.0);
    pen.text_center(
        &format!("Done in {} moves", g.moves),
        c.x,
        c.y - p.h * 0.5 * s + 262.0 * s,
        34.0 * s,
        BODY.with_alpha(a(0.5)),
    );
    let sub = if g.new_best {
        "New best!".to_string()
    } else {
        format!("Best: {} moves", g.best.unwrap_or(g.moves))
    };
    pen.text_center(
        &sub,
        c.x,
        c.y - p.h * 0.5 * s + 304.0 * s,
        26.0 * s,
        if g.new_best { Color::from_rgba(230, 140, 0, 255) } else { BODY }.with_alpha(a(0.6)),
    );
    let [levels, again, next] = Memory::done_buttons(view);
    let pulse = 1.0 + (age * 4.5).sin() * 0.03;
    let shift = c - p.center();
    let place = |r: Rect, k: f32| {
        Rect::new(
            r.center().x - r.w * k * s / 2.0 + shift.x,
            r.center().y - r.h * k * s / 2.0 + shift.y,
            r.w * k * s,
            r.h * k * s,
        )
    };
    pen.painted_button(place(levels, 1.0), "Levels", crate::ui::Tone::Blue, 0.0, a(0.7));
    pen.painted_button(place(again, 1.0), "Again", crate::ui::Tone::Orange, 0.0, a(0.75));
    pen.painted_button(place(next, pulse), "Next", crate::ui::Tone::Green, 0.0, a(0.8));
}

pub fn draw(g: &Memory, pen: &mut Pen, view: &View) {
    let f = view.full();
    let side = f.w.max(f.h);
    pen.sprite(
        SHEET,
        "bg",
        Rect::new(f.x + (f.w - side) / 2.0, f.y + f.h - side, side, side),
        Draw::default(),
    );
    if g.screen == Screen::Select {
        draw_select(g, pen, view);
        return;
    }
    draw_hud(g, pen, view);
    let sh = g.shake.offset();
    let helper = g.helper();
    let teach_pulse = g.teach && g.moves == 0 && g.up().is_empty();
    for (i, r) in Memory::layout(view, g.pairs).iter().enumerate() {
        let pulse = if teach_pulse {
            1.0 + 0.04 * (g.t * 5.0 + i as f32 * 0.5).sin()
        } else {
            1.0
        };
        draw_card(g, pen, i, *r, sh, pulse);
        if helper == Some(i) {
            let k = 1.0 + 0.2 * (g.t * 6.0).sin();
            pen.spark(vec2(r.center().x, r.y - 6.0), r.w * 0.45 * k, g.t * 1.5, GOLD);
        }
    }
    pen.particles(&g.particles);
    pen.popups(&g.popups);
    draw_hint(g, pen, view);
    if g.won() && g.celebrated >= DONE_DELAY {
        draw_done(g, pen, view, g.celebrated - DONE_DELAY);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paint::Tex;

    fn started() -> MemoryPlay {
        kg_core::save::reset_for_test();
        kg_core::profile::create("Mia", 0);
        MemoryPlay::new()
    }

    fn drawn(p: &MemoryPlay, v: &View) -> Pen {
        let mut pen = Pen::default();
        pen.begin(*v, 1.0);
        p.draw(&mut pen, v);
        pen
    }

    fn pages(pen: &Pen, sheet: &str) -> bool {
        pen.used().iter().filter_map(|b| b.tex).any(|t| matches!(t, Tex::Page(s, _) if s == sheet))
    }

    #[test]
    fn the_memory_sprites_exist() {
        let have = crate::art::names(SHEET);
        let mut want = vec!["bg", "back", "face"];
        want.extend(ANIMALS);
        for n in want {
            assert!(have.iter().any(|h| h == n), "missing {n}");
        }
    }

    #[test]
    fn the_play_screen_draws_cards_hud_and_hint() {
        let v = View::ipad_landscape();
        let p = started();
        let pen = drawn(&p, &v);
        assert!(pages(&pen, SHEET));
        assert!(pages(&pen, "ui"));
        assert!(pen.used().iter().filter_map(|b| b.tex).any(|t| t == Tex::Glyphs));
    }

    #[test]
    fn the_level_select_draws_every_tile() {
        let v = View::ipad_portrait();
        let mut p = started();
        p.g.open_select();
        let pen = drawn(&p, &v);
        assert!(pages(&pen, SHEET) && pages(&pen, "ui"));
        assert!(pen.vertex_count() > 400);
    }

    #[test]
    fn the_background_covers_the_whole_screen_past_the_safe_area() {
        let v = View::fit_safe(
            2400.0,
            1080.0,
            kg_core::safe::Insets {
                left: 120.0,
                bottom: 40.0,
                ..Default::default()
            },
        );
        let f = v.full();
        for select in [false, true] {
            let mut p = started();
            if select {
                p.g.open_select();
            }
            let pen = drawn(&p, &v);
            let bg = &pen.used()[0];
            let (mut lo, mut hi) = (f32::MAX, f32::MIN);
            let (mut top, mut bot) = (f32::MAX, f32::MIN);
            for q in &bg.pos {
                lo = lo.min(q[0]);
                hi = hi.max(q[0]);
                top = top.min(-q[1]);
                bot = bot.max(-q[1]);
            }
            assert!(lo <= f.x + 0.01 && hi >= f.x + f.w, "bg {lo}..{hi} vs {f:?}");
            assert!(top <= f.y + 0.5 && bot >= f.y + f.h - 0.5);
        }
    }

    #[test]
    fn winning_a_level_shows_the_clear_panel_and_the_last_level_the_board() {
        let v = View::ipad_landscape();
        for level in [1, MAX_LEVEL] {
            let mut p = started();
            p.g.begin(level);
            let before = drawn(&p, &v).vertex_count();
            for id in 0..MAX_PAIRS as u8 {
                let at: Vec<usize> = (0..p.g.cards.len()).filter(|i| p.g.cards[*i].0 == id).collect();
                if at.len() == 2 {
                    p.g.pick(at[0]);
                    p.g.pick(at[1]);
                }
            }
            for _ in 0..200 {
                p.update(&Input::default(), &v, 1.0 / 60.0);
            }
            assert!(p.shows_done());
            assert!(drawn(&p, &v).vertex_count() > before + 100);
        }
    }
}
