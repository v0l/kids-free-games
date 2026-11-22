use super::Play;
use crate::draw::Draw;
use crate::paint::Pen;
use crate::ui::{Results, Tone};
use glam::{vec2, Vec2};
use kg_core::engine::{Game, Input, View};
use kg_core::fx::ease_out_back;
use kg_core::games::share::*;
use kg_core::geom::{with_alpha, Color, Rect, WHITE};

pub const KEY: &str = GAME;
const SHEET: &str = "share";
const INK: Color = Color::from_rgba(40, 30, 60, 255);
const STAR_GOLD: Color = Color::from_rgba(255, 210, 60, 255);
const BLUE_INK: Color = Color::from_rgba(10, 70, 140, 255);
const ACCENT: Color = Color::from_rgba(240, 160, 70, 255);

pub struct SharePlay {
    pub g: Share,
}

impl SharePlay {
    pub fn new() -> Self {
        Self { g: Share::new() }
    }
}

impl Default for SharePlay {
    fn default() -> Self {
        Self::new()
    }
}

fn cleared(g: &Share) -> bool {
    matches!(g.mode, Mode::Report(_)) && matches!(g.report, Some(Report::Cleared { .. }))
}

impl Play for SharePlay {
    fn update(&mut self, input: &Input, view: &View, dt: f32) {
        let showing = cleared(&self.g);
        self.g.update(input, view, dt);
        if cleared(&self.g) && !showing {
            kg_core::net::request_board(GAME);
        }
    }

    fn draw(&self, pen: &mut Pen, view: &View) {
        draw(&self.g, pen, view);
    }

    fn score(&self) -> u32 {
        self.g.ladder.stars as u32
    }

    fn over(&self) -> bool {
        cleared(&self.g)
    }

    fn as_any(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

fn draw_backdrop(pen: &mut Pen, view: &View) {
    let f = view.full();
    let side = f.w.max(f.h);
    pen.sprite(
        SHEET,
        "bg",
        Rect::new(f.x + (f.w - side) / 2.0, f.y + f.h - side, side, side),
        Draw::default(),
    );
}

fn cookie(pen: &mut Pen, c: Vec2, size: f32, rot: f32) {
    pen.soft_shadow(c + vec2(0.0, size * 0.4), size * 0.4, size * 0.1, 0.5);
    pen.sprite_at(SHEET, "cookie", c, size, Draw::rot(rot));
}

fn draw_prompt(g: &Share, pen: &mut Pen, view: &View) {
    let r = prompt_rect(view);
    let pop = ease_out_back(g.prompt_bump.t).max(0.3);
    let c = r.center();
    let rr = Rect::new(
        c.x - r.w * pop / 2.0,
        c.y - r.h * pop / 2.0,
        r.w * pop,
        r.h * pop,
    );
    pen.painted_panel(rr, with_alpha(WHITE, 0.94));
    let text = g.prompt_text();
    let size = (rr.h * 0.5).min(rr.w * 0.9 / text.len().max(4) as f32 * 1.8);
    pen.text_outline(&text, c.x, c.y - rr.h * 0.02, size, WHITE, BLUE_INK);
}

fn draw_table(g: &Share, pen: &mut Pen, view: &View) {
    if !has_drag(g.ladder.level) {
        return;
    }
    let q = &g.question;
    let n = g.placed.len();
    for (i, cell) in cells(view, n).iter().enumerate() {
        let plate = plate_rect(*cell);
        let wob = (g.plate_wobble[i] * 30.0).sin() * 8.0 * g.plate_wobble[i];
        let bump = g.plate_bump[i].scale(0.12);
        let ah = animal_height(*cell);
        let name = ANIMALS[q.animal(i)];
        let hop = g.celebrate.map_or(0.0, |t| {
            ((t * 9.0 - i as f32 * 0.8).sin().max(0.0)) * ah * 0.08
        });
        pen.soft_shadow(
            plate.center() + vec2(0.0, plate.h * 0.3),
            plate.w * 0.5,
            plate.h * 0.1,
            0.4,
        );
        pen.sprite_at(
            SHEET,
            name,
            vec2(
                cell.center().x + wob,
                plate.y + plate.h * 0.2 - ah / 2.0 - hop,
            ),
            ah,
            Draw::default(),
        );
        pen.sprite_w(
            SHEET,
            "plate",
            plate.center() + vec2(wob, 0.0),
            plate.w * bump,
            Draw::default(),
        );
        let (spots, size) = plate_spots(plate, g.placed[i] as usize);
        for (k, s) in spots.iter().enumerate() {
            cookie(pen, *s + vec2(wob, 0.0), size * bump, k as f32 * 0.7);
        }
    }
}

fn draw_pile(g: &Share, pen: &mut Pen, view: &View) {
    if g.phase != Phase::Drag {
        return;
    }
    let pile = pile_rect(view);
    pen.painted_panel(pile, with_alpha(WHITE, 0.5));
    let left = g.remaining().max(0) as usize;
    let (slots, size) = pile_slots(pile, left);
    let shown = if g.held { left.saturating_sub(1) } else { left };
    for (i, s) in slots.iter().enumerate().take(shown) {
        cookie(pen, *s, size * 0.9, i as f32 * 0.9);
    }
    if g.held {
        cookie(pen, g.held_pos, size * 1.15, 0.2);
    }
}

fn draw_buttons(g: &Share, pen: &mut Pen, view: &View) {
    if g.phase != Phase::Choose {
        return;
    }
    let rects = button_rects(view);
    for (i, (r, v)) in rects.iter().zip(&g.question.options).enumerate() {
        let tried = g.attempt.tried[i];
        let pop = g.press[i].scale(0.12);
        let wob = (g.wobble[i] * 36.0).sin() * 12.0 * g.wobble[i];
        let c = r.center() + vec2(wob, 0.0);
        let rr = Rect::new(
            c.x - r.w * pop / 2.0,
            c.y - r.h * pop / 2.0,
            r.w * pop,
            r.h * pop,
        );
        if g.glow_slot() == Some(i) {
            let glow = 10.0 + 4.0 * (g.t * 6.0).sin();
            pen.round_rect(
                Rect::new(
                    rr.x - glow,
                    rr.y - glow,
                    rr.w + glow * 2.0,
                    rr.h + glow * 2.0,
                ),
                24.0,
                Color::new(1.0, 0.95, 0.4, 0.7),
            );
        }
        let correct_now = g.attempt.solved && *v == g.question.answer();
        let tone = if correct_now {
            Tone::Green
        } else {
            [Tone::Orange, Tone::Blue, Tone::Green][i % 3]
        };
        pen.painted_button(
            rr,
            &v.to_string(),
            tone,
            0.0,
            if tried { 0.45 } else { 1.0 },
        );
    }
}

fn draw_levels(g: &Share, pen: &mut Pen, view: &View) {
    draw_backdrop(pen, view);
    let unlocked = g.unlocked_level();
    pen.ribbon(view, "Pick a level", vec2(view.w / 2.0, 90.0), g.t);
    for (i, r) in level_tiles(view).iter().enumerate() {
        let open = i <= unlocked;
        if i == g.ladder.level {
            let glow = 8.0 + 3.0 * (g.t * 5.0).sin();
            pen.round_rect(
                Rect::new(r.x - glow, r.y - glow, r.w + glow * 2.0, r.h + glow * 2.0),
                34.0,
                Color::new(1.0, 0.95, 0.4, 0.8),
            );
        }
        pen.painted_panel(*r, if open { WHITE } else { with_alpha(WHITE, 0.55) });
        if open {
            pen.text_outline(
                &(i + 1).to_string(),
                r.x + r.w * 0.15,
                r.y + r.h * 0.24,
                r.h * 0.2,
                WHITE,
                ACCENT,
            );
            let ex = level_example(i);
            let size = (r.h * 0.24).min(r.w * 0.86 / ex.chars().count() as f32 * 1.7);
            pen.text_outline(ex, r.center().x, r.y + r.h * 0.46, size, WHITE, BLUE_INK);
            let earned = kg_core::profile::stars(GAME, i as u32);
            for j in 0..3u8 {
                let c = vec2(
                    r.center().x + (j as f32 - 1.0) * r.h * 0.24,
                    r.y + r.h * 0.76,
                );
                let fill = if j < earned {
                    STAR_GOLD
                } else {
                    Color::from_rgba(110, 90, 140, 150)
                };
                pen.star(c, r.h * 0.1, 0.0, fill);
            }
        } else {
            pen.sprite_at(
                "ui",
                "icon_lock",
                r.center() - vec2(0.0, r.h * 0.05),
                r.h * 0.34,
                Draw::tint(INK),
            );
        }
    }
    pen.painted_button(play_button(view), "Play", Tone::Green, 0.0, 1.0);
}

fn draw_report(g: &Share, pen: &mut Pen, view: &View, age: f32) {
    match g.report {
        Some(Report::Level { level, stars }) => {
            let title = format!("Level {} done!", level + 1);
            let next = if level + 1 < LEVELS {
                format!("Next: {}", level_name(level + 1))
            } else {
                "Fair sharing star!".to_string()
            };
            let sheet = pen.panel(view, &title, &["", &next, &g.summary], age);
            pen.panel_star_row(
                &sheet,
                age,
                stars,
                STAR_GOLD,
                Color::from_rgba(150, 140, 170, 140),
            );
        }
        Some(Report::Cleared { rank }) => {
            let total = kg_core::profile::total_stars(GAME);
            let line = format!("{total} stars earned");
            pen.results(
                view,
                &Results {
                    game: GAME,
                    title: "Fair sharing star!",
                    line: &line,
                    rank,
                    action: "Keep sharing",
                    format: stars_score,
                },
                age,
            );
        }
        None => {}
    }
}

pub fn draw(g: &Share, pen: &mut Pen, view: &View) {
    if g.mode == Mode::Levels {
        draw_levels(g, pen, view);
        return;
    }
    draw_backdrop(pen, view);
    let h = hud(view);
    crate::games::count::draw_hud(
        pen,
        view,
        &h,
        &g.ladder,
        &format!("Level {}", g.ladder.level + 1),
        ACCENT,
        &g.streak_bump,
    );
    pen.offset = g.shake.offset() * 0.5;
    crate::games::count::draw_goal(pen, view, &h, &g.goal_text(), g.first_time);
    draw_prompt(g, pen, view);
    draw_table(g, pen, view);
    draw_pile(g, pen, view);
    draw_buttons(g, pen, view);
    if g.first_time && g.mode == Mode::Play && g.phase == Phase::Drag && !g.held {
        let pulse = 1.0 + (g.t * 4.0).sin() * 0.03;
        let y = pile_rect(view).y - view.gap() - 24.0;
        pen.hint(
            "Drag a cookie onto a plate, or tap a plate",
            vec2(view.w / 2.0, y),
            pulse,
        );
    }
    pen.offset = Vec2::ZERO;
    pen.particles(&g.particles);
    pen.popups(&g.popups);
    if let Mode::Report(age) = g.mode {
        draw_report(g, pen, view, age);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fresh() -> SharePlay {
        kg_core::save::reset_for_test();
        kg_core::profile::create("Mia", 0);
        SharePlay::new()
    }

    fn drawn(p: &SharePlay, v: &View) -> Pen {
        let mut pen = Pen::default();
        pen.begin(*v, 1.0);
        p.draw(&mut pen, v);
        pen
    }

    #[test]
    fn share_sprites_all_exist() {
        let have = crate::art::names(SHEET);
        for n in ["bg", "cookie", "plate", "icon_share"]
            .into_iter()
            .chain(ANIMALS)
        {
            assert!(have.iter().any(|h| h == n), "missing {n}");
        }
    }

    #[test]
    fn every_level_draws_in_both_orientations() {
        for v in [View::ipad_landscape(), View::ipad_portrait()] {
            for l in 0..LEVELS {
                let mut p = fresh();
                kg_core::profile::unlock(GAME, l as u32);
                p.g.start_level(l);
                assert!(drawn(&p, &v).vertex_count() > 300, "level {l}");
            }
        }
    }

    #[test]
    fn the_picker_and_both_reports_draw() {
        for v in [View::ipad_landscape(), View::ipad_portrait()] {
            let mut p = fresh();
            p.g.mode = Mode::Levels;
            assert!(drawn(&p, &v).vertex_count() > 500);
            p.g.mode = Mode::Report(1.0);
            p.g.report = Some(Report::Level { level: 0, stars: 2 });
            assert!(drawn(&p, &v).vertex_count() > 800);
            p.g.report = Some(Report::Cleared { rank: Some(0) });
            assert!(drawn(&p, &v).vertex_count() > 800);
        }
    }

    #[test]
    fn dragging_from_the_pile_onto_a_plate_places_a_cookie() {
        let v = View::ipad_landscape();
        let mut p = fresh();
        p.g.first_time = false;
        p.update(&Input::press_at(pile_rect(&v).center()), &v, 1.0 / 60.0);
        assert!(p.g.held);
        let n = p.g.placed.len();
        let at = cells(&v, n)[0].center();
        let mut up = Input::press_at(at);
        up.pressed = false;
        up.down = false;
        up.released = true;
        p.update(&up, &v, 1.0 / 60.0);
        assert_eq!(p.g.placed[0], 1);
    }
}
