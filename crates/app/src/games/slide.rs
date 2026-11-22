use super::Play;
use crate::draw::Draw;
use crate::paint::Pen;
use crate::ui::{plain_score, Results, Tone, BODY, INK};
use glam::vec2;
use kg_core::engine::{Game, Input, View};
use kg_core::fx::{ease_out_back, ease_out_cubic, Side};
use kg_core::games::slide::*;
use kg_core::geom::{with_alpha, Color, Rect, WHITE};

pub const KEY: &str = GAME;
const SHEET: &str = "slide";
const CLOUDS: [&str; 4] = ["cloud1", "cloud2", "cloud3", "cloud4"];

pub struct SlidePlay {
    pub g: Slide,
    results_age: f32,
}

impl SlidePlay {
    pub fn new() -> Self {
        Self {
            g: Slide::new(),
            results_age: 0.0,
        }
    }

    fn shows_results(&self) -> bool {
        self.g.state == State::Won && self.g.end_t > WIN_DELAY
    }
}

impl Default for SlidePlay {
    fn default() -> Self {
        Self::new()
    }
}

impl Play for SlidePlay {
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
        self.g.state == State::Won && self.g.end_t > WIN_DELAY + 0.5
    }

    fn as_any(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

fn backdrop(pen: &mut Pen, view: &View, t: f32) {
    pen.sky(
        view,
        Color::from_rgba(120, 200, 255, 255),
        Color::from_rgba(255, 236, 180, 255),
    );
    for i in 0..4usize {
        let span = view.w + 500.0;
        let x = (i as f32 * 0.31 * span + t * (6.0 + i as f32 * 3.0)) % span - 250.0;
        let y = 120.0 + (i as f32 * 0.37 % 1.0) * view.h * 0.6;
        pen.sprite_w(
            "ui",
            CLOUDS[i % CLOUDS.len()],
            vec2(x, y),
            150.0 + (i % 3) as f32 * 40.0,
            Draw::tint(with_alpha(WHITE, 0.8)),
        );
    }
}

fn star_row(pen: &mut Pen, at: glam::Vec2, size: f32, earned: u8, shown: f32) {
    for k in 0..3usize {
        let on = k < earned as usize;
        let pop = if on {
            ease_out_back(((shown - STAR_START - STAR_GAP * k as f32) / 0.3).clamp(0.0, 1.0))
        } else {
            (shown / 0.3).clamp(0.0, 1.0)
        };
        let tint = if on {
            WHITE
        } else {
            Color::new(0.4, 0.4, 0.46, 0.85)
        };
        let lift = if k == 1 { -size * 0.14 } else { 0.0 };
        pen.sprite_at(
            "ui",
            "p_star",
            vec2(at.x + (k as f32 - 1.0) * size * 1.2, at.y + lift),
            size * pop,
            Draw::tint(tint),
        );
    }
}

fn draw_select(g: &Slide, pen: &mut Pen, view: &View) {
    backdrop(pen, view, g.t);
    let (at, k) = kg_core::games::helix::select_ribbon(view);
    pen.ribbon_k(view, "Pick a level", at, g.t, k);
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
        pen.nine(
            "ui",
            "panel",
            *r,
            14.0,
            (r.w.min(r.h) / 64.0).clamp(0.6, 2.5),
            tint,
        );
        let ink = if locked {
            Color::new(0.4, 0.38, 0.5, 0.7)
        } else {
            INK
        };
        pen.text_center(&n.to_string(), r.center().x, r.y + r.h * 0.36, r.h * 0.34, ink);
        if !locked {
            let size = spec(n).n;
            pen.text_center(
                &format!("{size}x{size}"),
                r.center().x,
                r.y + r.h * 0.62,
                r.h * 0.15,
                ink,
            );
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
                        r.y + r.h * 0.84,
                    ),
                    r.h * 0.2,
                    Draw::tint(tint),
                );
            }
        }
    }
    let label = format!("Play level {now}");
    pen.painted_button(l.play, &label, Tone::Green, 0.0, 1.0);
}

fn draw_board(g: &Slide, pen: &mut Pen, view: &View) {
    let b = board_rect(view);
    let frame = Rect::new(b.x - 10.0, b.y - 10.0, b.w + 20.0, b.h + 20.0);
    pen.round_rect(
        frame.offset(vec2(0.0, 6.0)),
        26.0,
        Color::new(0.0, 0.0, 0.0, 0.2),
    );
    pen.round_rect(frame, 26.0, Color::from_rgba(120, 80, 170, 255));
    pen.round_rect(
        Rect::new(b.x - 4.0, b.y - 4.0, b.w + 8.0, b.h + 8.0),
        20.0,
        Color::from_rgba(60, 40, 100, 255),
    );
    let n = g.n;
    let picture = PICTURES[g.picture];
    let won = g.state == State::Won;
    let blank_cell = g.vis[0];
    let blank_r = cell_rect(view, n, blank_cell);
    if !won {
        pen.round_rect(blank_r, 10.0, Color::new(0.0, 0.0, 0.0, 0.25));
    }
    for v in 1..n * n {
        let home = ((v - 1) % n, (v - 1) / n);
        let part = Rect::new(
            home.0 as f32 / n as f32,
            home.1 as f32 / n as f32,
            1.0 / n as f32,
            1.0 / n as f32,
        );
        let r = cell_rect(view, n, g.vis[v]);
        let cell = g.tiles.iter().position(|t| *t as usize == v).unwrap_or(0);
        let correct = g.at_home(cell);
        let mut tint = WHITE;
        if g.hint == Some(cell) {
            tint = Color::from_rgba(255, 245, 160, 255);
        }
        if won {
            let pulse = ((g.end_t * 4.0 - v as f32 * 0.1).sin() * 0.5 + 0.5) * 0.15;
            tint = Color::new(1.0 + pulse, 1.0 + pulse, 1.0, 1.0);
        }
        let dest = r;
        pen.sprite_part(SHEET, picture, part, dest, Draw::tint(tint));
        let edge = if correct {
            Color::from_rgba(255, 215, 60, 255)
        } else {
            Color::new(1.0, 1.0, 1.0, 0.7)
        };
        if !won {
            let w = (dest.w * 0.035).max(2.0);
            pen.rect(Rect::new(dest.x, dest.y, dest.w, w), edge);
            pen.rect(Rect::new(dest.x, dest.bottom() - w, dest.w, w), edge);
            pen.rect(Rect::new(dest.x, dest.y, w, dest.h), edge);
            pen.rect(Rect::new(dest.right() - w, dest.y, w, dest.h), edge);
        }
        if n <= 3 && !won {
            pen.text_outline(
                &v.to_string(),
                dest.x + dest.w * 0.2,
                dest.y + dest.h * 0.2,
                dest.h * 0.22,
                WHITE,
                Color::from_rgba(60, 40, 90, 255),
            );
        }
    }
    if g.peek > 0.0 {
        let a = (g.peek / 0.3).min(1.0);
        pen.sprite(SHEET, picture, b, Draw::tint(with_alpha(WHITE, a)));
    }
}

fn draw_hud(g: &Slide, pen: &mut Pen, view: &View) {
    pen.chip(view, Side::Left, "LEVEL", &g.level.to_string(), 1.0);
    pen.chip(view, Side::Right, "MOVES", &g.moves.to_string(), g.bump.t);
    star_row(pen, vec2(view.w / 2.0, 52.0), 30.0, g.stars_now(), 10.0);
    let b = buttons(view);
    pen.painted_button(b.restart, "Restart", Tone::Orange, 0.0, 1.0);
    pen.painted_button(b.hint, "Hint", Tone::Green, 0.0, 1.0);
    pen.painted_button(b.peek, "Peek", Tone::Blue, 0.0, 1.0);
    pen.painted_button(b.levels, "Levels", Tone::Blue, 0.0, 1.0);
    let pulse = 1.0 + (g.t * 3.0).sin() * 0.03;
    if !g.taught && g.moves == 0 {
        pen.hint("Tap a piece next to the gap", hint_pos(view), pulse);
    } else if g.hint.is_some() {
        pen.hint("Slide the glowing piece", hint_pos(view), pulse);
    }
}

fn draw_win(g: &Slide, pen: &mut Pen, view: &View, age: f32) {
    let line = format!("Level {} done, score {}", g.level, g.score);
    let next = if g.level >= MAX_LEVEL {
        "Play again"
    } else {
        "Next level"
    };
    let labels: Vec<&str> = if g.can_skip() {
        vec![next, "Skip ahead", "Levels"]
    } else {
        vec![next, "Levels"]
    };
    let lay = pen.results_actions_stars(
        view,
        &Results {
            game: GAME,
            title: "Solved!",
            line: &line,
            rank: g.rank,
            action: next,
            format: plain_score,
        },
        &labels,
        age,
    );
    if let Some(area) = lay.stars_rect() {
        let s = ease_out_back(age / 0.32).max(0.01);
        let size = (area.h * 0.9).min(area.w / 3.6);
        star_row(pen, area.center(), size * s, g.stars, age);
    }
}

pub fn draw(g: &Slide, pen: &mut Pen, view: &View, results_age: f32) {
    if g.state == State::Select {
        draw_select(g, pen, view);
        return;
    }
    backdrop(pen, view, g.t);
    draw_board(g, pen, view);
    pen.particles(&g.fx);
    pen.popups(&g.pops);
    if g.state == State::Playing {
        draw_hud(g, pen, view);
    } else if g.end_t > WIN_DELAY {
        draw_win(g, pen, view, results_age);
    } else {
        pen.chip(view, Side::Left, "LEVEL", &g.level.to_string(), 1.0);
        let fade = ease_out_cubic(g.end_t / 0.4);
        pen.text_outline(
            "Solved!",
            view.w / 2.0,
            view.h * 0.08,
            64.0 * (0.8 + 0.2 * fade),
            with_alpha(WHITE, fade),
            with_alpha(BODY, fade),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_picture_sheet_has_every_picture() {
        let have = crate::art::names(SHEET);
        for p in PICTURES {
            assert!(have.iter().any(|h| h == p), "slide sheet lacks {p}");
        }
    }
}
