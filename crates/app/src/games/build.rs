use super::Play;
use crate::draw::Draw;
use crate::paint::Pen;
use crate::ui::{Results, Tone};
use glam::{vec2, Vec2};
use kg_core::engine::{Game, Input, View};
use kg_core::fx::ease_out_back;
use kg_core::games::build::*;
use kg_core::geom::{darker, hsv, with_alpha, Color, Rect, WHITE};

pub const KEY: &str = GAME;
const WORDS: &str = "spell";
const INK: Color = Color::from_rgba(40, 30, 60, 255);
const STAR_GOLD: Color = Color::from_rgba(255, 210, 60, 255);
const BLUE_INK: Color = Color::from_rgba(10, 70, 140, 255);
const ACCENT: Color = Color::from_rgba(70, 160, 230, 255);

pub struct BuildPlay {
    pub g: Build,
}

impl BuildPlay {
    pub fn new() -> Self {
        Self { g: Build::new() }
    }
}

impl Default for BuildPlay {
    fn default() -> Self {
        Self::new()
    }
}

fn cleared(g: &Build) -> bool {
    matches!(g.mode, Mode::Report(_)) && matches!(g.report, Some(Report::Cleared { .. }))
}

impl Play for BuildPlay {
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

fn letter_color(i: usize) -> Color {
    hsv(0.02 + i as f32 * 0.13, 0.6, 0.98)
}

fn draw_backdrop(pen: &mut Pen, view: &View) {
    let f = view.full();
    let side = f.w.max(f.h);
    pen.sprite(
        WORDS,
        "bg",
        Rect::new(f.x + (f.w - side) / 2.0, f.y + f.h - side, side, side),
        Draw::default(),
    );
}

fn block(pen: &mut Pen, r: Rect, ch: char, color: Color) {
    pen.sprite(
        WORDS,
        "tile",
        Rect::new(r.x, r.y + r.h * 0.06, r.w, r.h),
        Draw::tint(Color::new(0.0, 0.0, 0.0, 0.2)),
    );
    pen.sprite(WORDS, "tile", r, Draw::tint(color));
    pen.text_outline(
        &ch.to_ascii_uppercase().to_string(),
        r.x + r.w / 2.0,
        r.y + r.h * 0.47,
        r.h * 0.62,
        WHITE,
        darker(color, 0.45),
    );
}

fn draw_picture(g: &Build, pen: &mut Pen, r: Rect) {
    pen.painted_panel(r, WHITE);
    let word = g.question.word;
    let size = crate::art::size(WORDS, word);
    let s = r.w.min(r.h) * 0.42 * g.picture_bump.scale(0.12);
    let k = 1.85 * s / size.x.max(size.y);
    let (w, h) = (size.x * k, size.y * k);
    let c = r.center() + vec2(0.0, (g.t * 2.0).sin() * r.h * 0.015);
    pen.soft_shadow(c + vec2(0.0, h * 0.5), w * 0.38, s * 0.07, 0.5);
    pen.sprite(
        WORDS,
        word,
        Rect::new(c.x - w / 2.0, c.y - h / 2.0, w, h),
        Draw::default(),
    );
}

fn draw_word(g: &Build, pen: &mut Pen, l: &Layout) {
    let q = &g.question;
    let letters = q.letters();
    let shown = q.shown(g.filled);
    let next = q.gaps.get(g.filled).copied();
    for (i, r) in l.slots.iter().enumerate() {
        let wave = g.celebrate.map_or(0.0, |t| {
            ((t * 9.0 - i as f32 * 0.9).sin().max(0.0)) * r.h * 0.25
        });
        match shown[i] {
            Some(ch) => {
                let rr = Rect::new(r.x, r.y - wave, r.w, r.h);
                let is_gap = q.gaps.contains(&i);
                let c = if is_gap {
                    letter_color(i)
                } else {
                    Color::from_rgba(150, 140, 190, 255)
                };
                block(pen, rr, ch, c);
            }
            None => {
                let on = next == Some(i);
                let pulse = if on {
                    1.0 + (g.t * 5.0).sin() * 0.04
                } else {
                    1.0
                };
                pen.sprite(
                    WORDS,
                    "tile",
                    *r,
                    Draw::tint(Color::new(1.0, 1.0, 1.0, if on { 0.7 } else { 0.35 })),
                );
                pen.rect(
                    Rect::new(
                        r.center().x - r.w * 0.3 * pulse,
                        r.y + r.h * 0.8,
                        r.w * 0.6 * pulse,
                        6.0,
                    ),
                    Color::from_rgba(255, 140, 120, 255),
                );
            }
        }
    }
    let _ = letters;
}

fn draw_tiles(g: &Build, pen: &mut Pen, l: &Layout) {
    let glow = g.glow();
    for (i, r) in l.tiles.iter().enumerate() {
        if g.used[i] {
            let inner = Rect::new(r.x + r.w * 0.1, r.y + r.h * 0.1, r.w * 0.8, r.h * 0.8);
            pen.round_rect(inner, r.w * 0.18, Color::new(0.55, 0.3, 0.4, 0.12));
            continue;
        }
        let ch = g.question.tiles[i];
        if g.held == Some(i) {
            let inner = Rect::new(r.x + r.w * 0.1, r.y + r.h * 0.1, r.w * 0.8, r.h * 0.8);
            pen.round_rect(inner, r.w * 0.18, Color::new(0.55, 0.3, 0.4, 0.12));
            let s = r.w * 1.15;
            let c = g.held_pos;
            block(
                pen,
                Rect::new(c.x - s / 2.0, c.y - s / 2.0, s, s),
                ch,
                Color::from_rgba(110, 140, 240, 255),
            );
            continue;
        }
        let w = g.wobble[i];
        let mut rr = *r;
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
        block(pen, rr, ch, col);
    }
}

fn draw_levels(g: &Build, pen: &mut Pen, view: &View) {
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

fn draw_report(g: &Build, pen: &mut Pen, view: &View, age: f32) {
    match g.report {
        Some(Report::Level { level, stars }) => {
            let title = format!("Level {} done!", level + 1);
            let next = if level + 1 < LEVELS {
                format!("Next: {}", level_name(level + 1))
            } else {
                "Word builder star!".to_string()
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
                    title: "Word builder star!",
                    line: &line,
                    rank,
                    action: "Keep building",
                    format: stars_score,
                },
                age,
            );
        }
        None => {}
    }
}

pub fn draw(g: &Build, pen: &mut Pen, view: &View) {
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
    let l = layout(view, g.question.word.len(), g.question.tiles.len());
    let pop = ease_out_back(g.picture_bump.t).max(0.3);
    let _ = pop;
    draw_picture(g, pen, l.picture);
    draw_word(g, pen, &l);
    draw_tiles(g, pen, &l);
    if g.first_time && g.mode == Mode::Play && g.celebrate.is_none() && g.held.is_none() {
        let pulse = 1.0 + (g.t * 4.0).sin() * 0.03;
        let y = l.slots[0].y - view.gap() - 24.0;
        let x = l.slots[0].x + (l.slots.last().map_or(0.0, |r| r.right()) - l.slots[0].x) / 2.0;
        pen.hint("Drag the missing letter in, or tap it", vec2(x, y), pulse);
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

    fn fresh() -> BuildPlay {
        kg_core::save::reset_for_test();
        kg_core::profile::create("Mia", 0);
        BuildPlay::new()
    }

    fn drawn(p: &BuildPlay, v: &View) -> Pen {
        let mut pen = Pen::default();
        pen.begin(*v, 1.0);
        p.draw(&mut pen, v);
        pen
    }

    #[test]
    fn every_word_in_play_has_a_painted_picture() {
        let have = crate::art::names(WORDS);
        for l in 0..LEVELS {
            for w in pool(l) {
                assert!(have.iter().any(|h| h == w), "no picture for {w}");
            }
        }
        assert!(crate::art::names("build").iter().any(|h| h == "icon_build"));
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
}
