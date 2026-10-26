pub mod world;

use super::Play;
use crate::draw::measure;
use crate::game3d::World3d;
use crate::paint::Pen;
use crate::ui::{plain_score, Results, Tone, BODY, INK};
use glam::vec2;
use kg_core::engine::{Game, Input, View};
use kg_core::fx::{ease_out_back, ease_out_cubic, Side};
use kg_core::games::helix::*;
use kg_core::geom::{with_alpha, Color, Rect, WHITE};

pub const KEY: &str = GAME;
const SHEET: &str = "helix";

pub struct HelixPlay {
    pub g: Helix,
    results_age: f32,
}

impl HelixPlay {
    pub fn new() -> Self {
        Self {
            g: Helix::new(),
            results_age: 0.0,
        }
    }

    fn shows_results(&self) -> bool {
        self.g.state == State::Dead && self.g.end_t > 0.5
            || self.g.state == State::Won && self.g.end_t > WIN_DELAY
    }
}

impl Default for HelixPlay {
    fn default() -> Self {
        Self::new()
    }
}

impl Play for HelixPlay {
    fn update(&mut self, input: &Input, view: &View, dt: f32) {
        let showing = self.shows_results();
        self.g.update(input, view, dt);
        if self.shows_results() {
            if !showing {
                self.results_age = 0.0;
                kg_core::net::request_board(GAME);
            }
            self.results_age += dt;
        }
    }

    fn draw(&self, pen: &mut Pen, view: &View) {
        draw(&self.g, pen, view, self.results_age);
    }

    fn score(&self) -> u32 {
        self.g.score
    }

    fn over(&self) -> bool {
        self.g.state == State::Dead
    }

    fn world(&self) -> World3d {
        World3d::Helix
    }

    fn as_any(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

fn levels_pill(pen: &mut Pen, view: &View) {
    pen.painted_button(levels_button(view), "Levels", Tone::Blue, 0.0, 1.0);
}

fn draw_select(g: &Helix, pen: &mut Pen, view: &View) {
    pen.ribbon(view, "Pick a level", vec2(view.w / 2.0, 84.0), g.t);
    pen.chip(
        view,
        Side::Right,
        "STARS",
        &kg_core::profile::total_stars(GAME).to_string(),
        1.0,
    );
    let l = select_layout(view);
    let open = reached();
    let now = start_level();
    for (i, r) in l.cells.iter().enumerate() {
        let n = i as u32 + 1;
        let locked = n > open;
        let tint = if locked {
            Color::new(0.55, 0.55, 0.62, 0.8)
        } else if n == now {
            Color::from_rgba(255, 224, 120, 255)
        } else {
            WHITE
        };
        let pulse = if n == now {
            1.0 + (g.t * 4.0).sin() * 0.03
        } else {
            1.0
        };
        let rr = Rect::new(
            r.center().x - r.w * pulse / 2.0,
            r.center().y - r.h * pulse / 2.0,
            r.w * pulse,
            r.h * pulse,
        );
        pen.nine(
            "ui",
            "panel",
            rr,
            14.0,
            (rr.w.min(rr.h) / 64.0).clamp(0.6, 2.5),
            tint,
        );
        let ink = if locked {
            Color::new(0.4, 0.38, 0.5, 0.7)
        } else {
            INK
        };
        pen.text_center(
            &n.to_string(),
            r.center().x,
            r.y + r.h * 0.38,
            r.h * 0.4,
            ink,
        );
        if !locked {
            let earned = kg_core::profile::stars(GAME, n);
            for k in 0..3u8 {
                let tint = if k < earned {
                    WHITE
                } else {
                    Color::new(0.6, 0.6, 0.7, 0.6)
                };
                pen.sprite_at(
                    "ui",
                    if k < earned {
                        "star_yellow"
                    } else {
                        "star_white"
                    },
                    vec2(
                        r.center().x + (k as f32 - 1.0) * r.w * 0.26,
                        r.y + r.h * 0.74,
                    ),
                    r.h * 0.24,
                    crate::draw::Draw::tint(tint),
                );
            }
        }
    }
    let label = format!("Play level {now}");
    pen.painted_button(l.play, &label, Tone::Green, 0.0, 1.0);
}

fn draw_win(g: &Helix, pen: &mut Pen, view: &View) {
    let age = (g.end_t - WIN_DELAY).max(0.0);
    let fade = ease_out_cubic(age / 0.2);
    pen.fill_screen(view, Color::new(0.06, 0.04, 0.16, 0.5 * fade));
    let pw = 560.0f32.min(view.w - 40.0);
    let ph = 420.0;
    let s = ease_out_back(age / 0.32).max(0.01);
    let c = view.center() + vec2(0.0, 10.0 + (1.0 - ease_out_cubic(age / 0.32)) * 90.0);
    let r = Rect::new(c.x - pw * s / 2.0, c.y - ph * s / 2.0, pw * s, ph * s);
    pen.painted_panel(r, WHITE);
    pen.ribbon(view, "Hooray!", vec2(c.x, r.y + 8.0 * s), age);
    let a = |d: f32| ((age - d) / 0.2).clamp(0.0, 1.0);
    let line = format!("Level {} done!", g.level);
    pen.text_center(
        &line,
        c.x,
        r.y + 100.0 * s,
        36.0 * s,
        with_alpha(BODY, a(0.12)),
    );
    for k in 0..3usize {
        let earned = k < g.stars as usize;
        let pop = if earned {
            ease_out_back(((age - STAR_START - STAR_GAP * k as f32) / 0.3).clamp(0.0, 1.0))
        } else {
            a(STAR_START)
        };
        let tint = if earned {
            WHITE
        } else {
            Color::new(0.4, 0.4, 0.46, 0.85)
        };
        let lift = if k == 1 { -14.0 } else { 0.0 };
        pen.sprite_at(
            "ui",
            "p_star",
            vec2(c.x + (k as f32 - 1.0) * 120.0 * s, r.y + (205.0 + lift) * s),
            100.0 * s * pop,
            crate::draw::Draw::tint(tint),
        );
    }
    let score = format!("Score {}   Best {}", g.score, g.best.0);
    pen.text_center(
        &score,
        c.x,
        r.y + 292.0 * s,
        30.0 * s,
        with_alpha(BODY, a(0.5)),
    );
    let label = if g.level >= MAX_LEVEL {
        "Play again"
    } else {
        "Next level"
    };
    let by = r.y + r.h - 56.0 * s;
    let pulse = 1.0 + (age * 4.5).sin() * 0.03;
    let bw = (measure(label, 30.0) + 110.0) * s * pulse;
    pen.painted_button(
        Rect::new(c.x - bw / 2.0, by - 10.0 * s, bw, 78.0 * s * pulse),
        label,
        Tone::Green,
        0.0,
        a(0.6),
    );
}

fn draw_progress(g: &Helix, pen: &mut Pen, view: &View) {
    let progress = g.passed as f32 / g.floors.len() as f32;
    let bar = Rect::new(view.w / 2.0 - 180.0, 24.0, 360.0, 26.0);
    pen.nine("ui", "panel", bar, 14.0, bar.h / 64.0, WHITE);
    pen.nine(
        "ui",
        "btn_yellow",
        Rect::new(bar.x, bar.y, (bar.w * progress).max(bar.h), bar.h),
        18.0,
        bar.h / 64.0,
        WHITE,
    );
    for (k, x) in [(g.level, bar.x - 26.0), (g.level + 1, bar.x + bar.w + 26.0)] {
        let done = x < view.w / 2.0 || progress >= 1.0;
        pen.sprite_at(
            "ui",
            if done { "round_yellow" } else { "round_white" },
            vec2(x, bar.y + 13.0),
            46.0,
            Default::default(),
        );
        pen.text_outline(
            &k.to_string(),
            x,
            bar.y + 11.0,
            26.0,
            WHITE,
            if done {
                Color::from_rgba(150, 90, 0, 255)
            } else {
                INK
            },
        );
    }
    let left = g.floors.len() - g.passed;
    let goal = match left {
        0 => "You made it!".to_string(),
        1 => "1 floor to go".to_string(),
        n => format!("{n} floors to go"),
    };
    pen.text_shadow(&goal, view.w / 2.0, 72.0, 24.0, WHITE);
}

pub fn draw(g: &Helix, pen: &mut Pen, view: &View, results_age: f32) {
    if g.state == State::Select {
        draw_select(g, pen, view);
        return;
    }
    pen.particles(&g.fx);
    pen.popups(&g.pops);
    draw_progress(g, pen, view);
    pen.big_score(view, &g.score.to_string(), 128.0, g.bump.t);
    pen.chip(view, Side::Left, "LEVEL", &g.level.to_string(), 1.0);
    pen.chip(view, Side::Right, "BEST", &g.best.0.to_string(), 1.0);
    let pulse = 1.0 + (g.t * 3.0).sin() * 0.03;
    let at = vec2(view.w / 2.0, view.h * 0.8);
    match g.state {
        State::Ready => {
            pen.hint("Tap, then drag sideways to spin the tower", at, pulse);
            levels_pill(pen, view);
        }
        State::Playing if !g.taught => pen.hint("Line up the hole to drop through!", at, pulse),
        State::Dead if g.end_t > 0.5 => {
            let line = format!("Level {}, score {}", g.level, g.score);
            pen.results(
                view,
                &Results {
                    game: GAME,
                    title: "Splat!",
                    line: &line,
                    rank: g.rank,
                    action: "Try again",
                    format: plain_score,
                },
                results_age,
            );
            levels_pill(pen, view);
        }
        State::Won if g.end_t > WIN_DELAY => {
            draw_win(g, pen, view);
            levels_pill(pen, view);
        }
        _ => {}
    }
}
