pub mod world;

use super::Play;
use crate::draw::measure;
use crate::game3d::World3d;
use crate::paint::Pen;
use crate::ui::{plain_score, Results, Tone, BODY, INK};
use glam::vec2;
use kg_core::engine::{Game, Input, View};
use kg_core::fx::{ease_out_back, ease_out_cubic, Side};
use kg_core::games::runner::*;
use kg_core::geom::{with_alpha, Color, Rect, WHITE};

pub const KEY: &str = GAME;
const SHEET: &str = "runner";

pub struct RunnerPlay {
    pub g: Runner,
    results_age: f32,
}

impl RunnerPlay {
    pub fn new() -> Self {
        Self {
            g: Runner::new(),
            results_age: 0.0,
        }
    }
}

impl Default for RunnerPlay {
    fn default() -> Self {
        Self::new()
    }
}

impl Play for RunnerPlay {
    fn update(&mut self, input: &Input, view: &View, dt: f32) {
        let showing = self.g.shows_results();
        self.g.update(input, view, dt);
        if self.g.shows_results() {
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
        World3d::Runner
    }

    fn as_any(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

fn levels_pill(pen: &mut Pen, view: &View) {
    pen.painted_button(levels_button(view), "Levels", Tone::Blue, 0.0, 1.0);
}

fn draw_select(g: &Runner, pen: &mut Pen, view: &View) {
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

fn draw_win(g: &Runner, pen: &mut Pen, view: &View) {
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


fn draw_hearts(g: &Runner, pen: &mut Pen, view: &View) {
    for k in 0..HEARTS {
        let on = k < g.hearts;
        let c = vec2(view.w - 150.0 + k as f32 * 46.0, 118.0);
        let pop = if on { 1.0 } else { 0.7 };
        let tint = if on {
            WHITE
        } else {
            Color::new(0.2, 0.15, 0.3, 0.5)
        };
        let col = if on {
            Color::from_rgba(255, 70, 90, 255)
        } else {
            Color::new(0.25, 0.2, 0.3, 0.5)
        };
        let r = 17.0 * pop;
        pen.circle(c + vec2(-r * 0.5, -r * 0.2), r * 0.62, col);
        pen.circle(c + vec2(r * 0.5, -r * 0.2), r * 0.62, col);
        pen.tri(
            c + vec2(-r * 1.04, 0.0),
            c + vec2(r * 1.04, 0.0),
            c + vec2(0.0, r * 1.15),
            col,
        );
        pen.circle(c + vec2(-r * 0.55, -r * 0.4), r * 0.18, with_alpha(tint, 0.6));
    }
}

fn draw_coach(g: &Runner, pen: &mut Pen, view: &View) {
    let Some(coach) = g.coach() else {
        return;
    };
    let t = g.t * 1.6 % 1.0;
    let k = ease_out_cubic(t);
    let base = vec2(view.w / 2.0, view.h * 0.78);
    let (off, label) = match coach {
        Coach::Dodge => (vec2((k - 0.5) * 220.0, 0.0), "Swipe to change lane"),
        Coach::Jump => (vec2(0.0, -k * 120.0), "Swipe up to jump!"),
        Coach::Slide => (vec2(0.0, k * 120.0), "Swipe down to slide!"),
    };
    pen.hint(label, base + vec2(0.0, 90.0), 1.0 + (g.t * 3.0).sin() * 0.03);
    let a = (1.0 - t).clamp(0.0, 1.0).min(t * 6.0);
    let hand = base + off;
    pen.circle(hand, 34.0, Color::new(1.0, 1.0, 1.0, 0.85 * a));
    pen.ring(hand, 34.0, 5.0, Color::new(0.2, 0.1, 0.4, 0.8 * a));
    pen.line(base, hand, 8.0, Color::new(1.0, 1.0, 1.0, 0.35 * a));
}

fn draw_progress(g: &Runner, pen: &mut Pen, view: &View) {
    let progress = g.progress();
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
    let runner_x = bar.x + (bar.w - bar.h) * progress + bar.h / 2.0;
    pen.circle(vec2(runner_x, bar.y + 13.0), 9.0, Color::from_rgba(240, 90, 80, 255));
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
    let left = (g.length - g.dist).max(0.0) as u32;
    let goal = if left == 0 {
        "You made it!".to_string()
    } else {
        format!("{left} m to go")
    };
    pen.text_shadow(&goal, view.w / 2.0, 72.0, 24.0, WHITE);
}

pub fn draw(g: &Runner, pen: &mut Pen, view: &View, results_age: f32) {
    if g.state == State::Select {
        draw_select(g, pen, view);
        return;
    }
    if g.flash_hit > 0.0 {
        pen.fill_screen(view, Color::new(1.0, 0.25, 0.2, 0.22 * g.flash_hit));
    }
    pen.vignette(view, 0.4 + g.speed / 40.0);
    pen.particles(&g.fx);
    pen.popups(&g.pops);
    draw_progress(g, pen, view);
    pen.chip(view, Side::Left, "COINS", &g.coins.to_string(), g.bump.t);
    pen.chip(view, Side::Right, "BEST", &g.best.0.to_string(), 1.0);
    draw_hearts(g, pen, view);
    if g.combo >= 3 && g.state == State::Playing {
        pen.text_outline(
            &format!("x{}", g.combo),
            view.w / 2.0,
            view.h * 0.2,
            44.0 + (g.t * 8.0).sin() * 2.0,
            Color::from_rgba(255, 220, 60, 255),
            Color::from_rgba(130, 50, 0, 255),
        );
    }
    let pulse = 1.0 + (g.t * 3.0).sin() * 0.03;
    match g.state {
        State::Ready => {
            pen.hint(
                "Tap to run! Swipe to dodge",
                vec2(view.w / 2.0, view.h * 0.8),
                pulse,
            );
            levels_pill(pen, view);
        }
        State::Playing => draw_coach(g, pen, view),
        State::Dead if g.shows_results() => {
            let line = format!("Level {}, {} coins", g.level, g.coins);
            pen.results(
                view,
                &Results {
                    game: GAME,
                    title: "Oh no!",
                    line: &line,
                    rank: g.rank,
                    action: "Try again",
                    format: plain_score,
                },
                results_age,
            );
            levels_pill(pen, view);
        }
        State::Won if g.shows_results() => {
            draw_win(g, pen, view);
            levels_pill(pen, view);
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kg_core::engine::Input;

    fn drawn(g: &Runner, v: View) -> Pen {
        let mut pen = Pen::default();
        pen.begin(v, 1.0);
        draw(g, &mut pen, &v, 1.0);
        pen
    }

    #[test]
    fn every_screen_draws_in_both_orientations() {
        kg_core::save::reset_for_test();
        for v in [View::ipad_landscape(), View::ipad_portrait()] {
            let mut g = Runner::with_level(1);
            assert!(drawn(&g, v).vertex_count() > 50);
            g.state = State::Playing;
            assert!(drawn(&g, v).vertex_count() > 50);
            g.open_select();
            assert!(drawn(&g, v).vertex_count() > 50);
            g.enter(1);
            g.state = State::Dead;
            g.end_t = 2.0;
            assert!(drawn(&g, v).vertex_count() > 50);
            g.state = State::Won;
            g.stars = 3;
            g.end_t = 3.0;
            assert!(drawn(&g, v).vertex_count() > 50);
        }
    }

    #[test]
    fn a_minute_of_play_with_the_autopilot_draws_every_frame() {
        kg_core::save::reset_for_test();
        let v = View::ipad_portrait();
        let mut p = RunnerPlay::new();
        p.update(&Input::press_at(v.center()), &v, 1.0 / 60.0);
        for _ in 0..3600 {
            let i = p.g.autopilot(1.0);
            p.update(&i, &v, 1.0 / 60.0);
            let _ = drawn(&p.g, v);
        }
        assert!(p.g.coins > 0);
    }
}
