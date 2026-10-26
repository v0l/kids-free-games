use super::Play;
use crate::draw::Draw;
use crate::paint::Pen;
use crate::ui::{plain_score, Results, Tone, BODY};
use glam::{vec2, Vec2};
use kg_core::engine::{Game, Input, View};
use kg_core::fx::{ease_out_back, ease_out_cubic, Side, CHIP_Y, INK};
use kg_core::games::breakout::*;
use kg_core::geom::{with_alpha, Color, Rect, WHITE};
use kg_core::profile;

pub const KEY: &str = kg_core::games::breakout::KEY;
const SHEET: &str = "breakout";

pub struct BreakoutPlay {
    pub g: Breakout,
}

impl BreakoutPlay {
    pub fn new() -> Self {
        Self { g: Breakout::new() }
    }
}

impl Default for BreakoutPlay {
    fn default() -> Self {
        Self::new()
    }
}

impl Play for BreakoutPlay {
    fn update(&mut self, input: &Input, view: &View, dt: f32) {
        let before = self.g.state;
        self.g.update(input, view, dt);
        if self.g.state != before && matches!(self.g.state, State::Lost | State::Cleared) {
            kg_core::net::request_board(KEY);
        }
    }

    fn draw(&self, pen: &mut Pen, view: &View) {
        draw(&self.g, pen, view);
    }

    fn score(&self) -> u32 {
        self.g.score
    }

    fn over(&self) -> bool {
        matches!(self.g.state, State::Lost | State::Cleared)
    }

    fn as_any(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

fn draw_arena(pen: &mut Pen, view: &View, clock: f32) {
    let f = view.full();
    let s = crate::art::size(SHEET, "bg");
    let k = (f.w / s.x).max(f.h / s.y);
    let (w, h) = (s.x * k, s.y * k);
    pen.sprite(
        SHEET,
        "bg",
        Rect::new(f.x + (f.w - w) / 2.0, f.y + (f.h - h) / 2.0, w, h),
        Draw::default(),
    );
    for k in 0..14 {
        let x = (k as f32 * 197.0 + 40.0) % view.w;
        let y = (k as f32 * 131.0 + 20.0) % view.h;
        let tw = 0.5 + 0.5 * (clock * 1.5 + k as f32).sin();
        pen.spark(
            vec2(x, y),
            6.0 + tw * 4.0,
            clock * 0.2,
            with_alpha(WHITE, 0.2 + 0.25 * tw),
        );
    }
}

fn draw_brick(pen: &mut Pen, row: usize, r: Rect) {
    let grow = r.h * 0.09;
    pen.sprite(
        SHEET,
        &format!("brick{}", row % ROWS),
        Rect::new(r.x - grow, r.y - grow, r.w + grow * 2.0, r.h + grow * 2.0),
        Draw::default(),
    );
}

fn off_star() -> Color {
    Color::from_rgba(90, 70, 130, 140)
}

fn draw_progress(g: &Breakout, pen: &mut Pen, view: &View) {
    let w = 240.0;
    let r = Rect::new(view.w / 2.0 - w / 2.0, CHIP_Y + 36.0, w, 14.0);
    pen.round_rect(r, 7.0, Color::new(0.0, 0.0, 0.1, 0.55));
    let f = g.cleared() as f32 / g.total.max(1) as f32;
    if f > 0.0 {
        let fill = Rect::new(r.x + 2.0, r.y + 2.0, ((w - 4.0) * f).max(10.0), r.h - 4.0);
        pen.round_rect(fill, fill.h / 2.0, Color::from_rgba(255, 200, 40, 255));
    }
}

fn draw_stars(g: &Breakout, pen: &mut Pen, view: &View) {
    let have = stars_for(g.misses);
    for i in 0..3 {
        let on = (i as u8) < have;
        pen.sprite_at(
            "ui",
            "p_star",
            vec2(view.w - 36.0 - i as f32 * 40.0, 36.0),
            36.0,
            Draw::tint(if on { WHITE } else { off_star() }),
        );
    }
}

fn draw_cleared(g: &Breakout, pen: &mut Pen, view: &View) {
    let age = g.age;
    let fade = ease_out_cubic(age / 0.2);
    pen.fill_screen(view, Color::new(0.06, 0.04, 0.16, 0.5 * fade));
    let s = ease_out_back(age / 0.32).max(0.01);
    let base = panel_rect(view);
    let c = base.center() + vec2(0.0, (1.0 - ease_out_cubic(age / 0.32)) * 90.0);
    let r = Rect::new(
        c.x - base.w * s / 2.0,
        c.y - base.h * s / 2.0,
        base.w * s,
        base.h * s,
    );
    pen.painted_panel(r, WHITE);
    pen.ribbon(view, "Level clear!", vec2(c.x, r.y + 8.0 * s), age);
    let earned = stars_for(g.misses);
    for i in 0..3u8 {
        let t = ((age - 0.3 - i as f32 * 0.25) / 0.3).clamp(0.0, 1.0);
        let on = i < earned;
        let k = if on { ease_out_back(t) } else { t };
        let rise = if i == 1 { -14.0 } else { 0.0 };
        pen.sprite_at(
            "ui",
            "p_star",
            vec2(c.x + (i as f32 - 1.0) * 110.0 * s, r.y + (122.0 + rise) * s),
            (if i == 1 { 110.0 } else { 92.0 }) * s * k.max(0.01),
            Draw {
                tint: if on { WHITE } else { off_star() },
                rotation: if on {
                    (age * 5.0 + i as f32).sin() * 0.05
                } else {
                    0.0
                },
                ..Default::default()
            },
        );
    }
    let line = if g.rank == Some(0) {
        format!("New best! Score {}", g.score)
    } else {
        format!("Score {}", g.score)
    };
    let la = ((age - 0.3) / 0.3).clamp(0.0, 1.0);
    pen.text_center(&line, c.x, r.y + 218.0 * s, 36.0 * s, with_alpha(BODY, la));
    let last = g.level >= LEVELS as u32;
    let label = if last { "All levels" } else { "Next level" };
    let pulse = 1.0 + (age * 4.5).sin() * 0.03;
    let place = |b: Rect, k: f32| {
        let at = c + (b.center() - base.center()) * s;
        Rect::new(
            at.x - b.w * s * k / 2.0,
            at.y - b.h * s * k / 2.0,
            b.w * s * k,
            b.h * s * k,
        )
    };
    pen.painted_button(place(next_button(view), pulse), label, Tone::Green, 0.0, la);
    if !last {
        pen.painted_button(
            place(levels_button(view), 1.0),
            "Levels",
            Tone::Blue,
            0.0,
            la,
        );
    }
}

fn draw_select(g: &Breakout, pen: &mut Pen, view: &View) {
    pen.fill_screen(view, Color::new(0.06, 0.04, 0.2, 0.45));
    pen.ribbon(view, "Pick a level", vec2(view.w / 2.0, 100.0), g.clock);
    let open = profile::unlocked(KEY).clamp(1, LEVELS as u32);
    let next = profile::level(KEY).clamp(1, open);
    for (i, r) in level_tiles(view).iter().enumerate() {
        let lvl = i as u32 + 1;
        let locked = lvl > open;
        let appear = ease_out_back(((g.age - i as f32 * 0.03) / 0.3).clamp(0.0, 1.0));
        let pulse = if lvl == next {
            1.0 + 0.04 * (g.clock * 5.0).sin()
        } else {
            1.0
        };
        let k = appear.max(0.01) * pulse;
        let c = r.center();
        let rr = Rect::new(c.x - r.w * k / 2.0, c.y - r.h * k / 2.0, r.w * k, r.h * k);
        let tint = if locked {
            Color::new(0.55, 0.55, 0.65, 1.0)
        } else if lvl == next {
            Color::from_rgba(255, 244, 190, 255)
        } else {
            WHITE
        };
        pen.painted_panel(rr, tint);
        let inner = Rect::new(
            rr.x + rr.w * 0.14,
            rr.y + rr.h * 0.2,
            rr.w * 0.72,
            rr.h * 0.42,
        );
        let (cw, ch) = (inner.w / COLS as f32, inner.h / ROWS as f32);
        for (j, t) in layout_tints(lvl).iter().enumerate() {
            if let Some(t) = t {
                let col = if locked {
                    Color::new(0.5, 0.5, 0.58, 0.6)
                } else {
                    brick_color(*t)
                };
                pen.rect(
                    Rect::new(
                        inner.x + (j % COLS) as f32 * cw + 0.5,
                        inner.y + (j / COLS) as f32 * ch + 0.5,
                        cw - 1.0,
                        ch - 1.0,
                    ),
                    col,
                );
            }
        }
        pen.text_outline(
            &lvl.to_string(),
            rr.center().x,
            rr.y + rr.h * 0.12,
            rr.h * 0.2,
            WHITE,
            INK,
        );
        if locked {
            pen.sprite_at(
                "ui",
                "icon_lock",
                rr.center() + vec2(0.0, rr.h * 0.02),
                rr.h * 0.3,
                Draw::tint(Color::from_rgba(70, 60, 100, 230)),
            );
        } else {
            let got = profile::stars(KEY, lvl);
            for j in 0..3u8 {
                pen.sprite_at(
                    "ui",
                    "p_star",
                    vec2(
                        rr.center().x + (j as f32 - 1.0) * rr.w * 0.26,
                        rr.y + rr.h * 0.8,
                    ),
                    rr.h * 0.24,
                    Draw::tint(if j < got {
                        WHITE
                    } else {
                        Color::from_rgba(90, 70, 130, 120)
                    }),
                );
            }
        }
    }
    let total = profile::total_stars(KEY);
    pen.chip(
        view,
        Side::Right,
        "STARS",
        &format!("{total}/{}", LEVELS * 3),
        g.bump.t,
    );
    let top = career_score();
    if top > 0 {
        pen.chip(view, Side::Left, "SCORE", &top.to_string(), g.bump.t);
    }
}

fn draw_bricks(g: &Breakout, pen: &mut Pen) {
    for (i, b) in g.bricks.iter().enumerate() {
        let Some(r) = b else { continue };
        let age = g.pops[i];
        if age < 0.0 {
            continue;
        }
        let k = ease_out_back(age / 0.35).max(0.01);
        let c = r.center();
        draw_brick(
            pen,
            g.tints[i],
            Rect::new(c.x - r.w * k / 2.0, c.y - r.h * k / 2.0, r.w * k, r.h * k),
        );
    }
}

fn draw_ball_and_paddle(g: &Breakout, pen: &mut Pen) {
    for (i, p) in g.trail.iter().enumerate() {
        let k = (i + 1) as f32 / g.trail.len() as f32;
        pen.sprite_at(
            SHEET,
            "ball",
            *p,
            BALL_R * 2.0 * (0.3 + 0.6 * k),
            Draw::tint(Color::new(1.0, 1.0, 1.0, 0.28 * k)),
        );
    }
    let p = g.paddle_rect();
    let sq = squash_scale(g.paddle_age);
    let pc = p.center();
    let p = Rect::new(
        pc.x - p.w * sq.x / 2.0,
        p.y + p.h - p.h * sq.y,
        p.w * sq.x,
        p.h * sq.y,
    );
    pen.soft_shadow(vec2(pc.x, p.y + p.h + 10.0), p.w * 0.45, 7.0, 0.6);
    pen.sprite(SHEET, "paddle", p, Draw::default());
    let glow = 0.22 + 0.08 * (g.clock * 6.0).sin();
    pen.sprite_at(
        SHEET,
        "ball",
        g.ball,
        BALL_R * 4.0,
        Draw::tint(Color::new(1.0, 1.0, 1.0, glow)),
    );
    pen.sprite_at(SHEET, "ball", g.ball, BALL_R * 2.3, Draw::default());
}

fn draw_lives(g: &Breakout, pen: &mut Pen, view: &View) {
    let n = if g.forgiving() { 0 } else { g.lives };
    for i in 0..n {
        let c = vec2(
            view.w - 30.0 - i as f32 * 36.0,
            35.0 + view.inset.top.min(0.0),
        );
        pen.sprite_at(
            SHEET,
            "ball",
            c + vec2(0.0, 2.0),
            28.0,
            Draw::tint(Color::new(0.0, 0.0, 0.0, 0.3)),
        );
        pen.sprite_at(SHEET, "ball", c, 28.0, Draw::default());
    }
}

fn draw_hud(g: &Breakout, pen: &mut Pen, view: &View) {
    pen.vignette(view, 0.6);
    pen.chip(view, Side::Left, "SCORE", &g.score.to_string(), g.bump.t);
    pen.text_outline(
        &format!("Level {}", g.level),
        view.w / 2.0,
        CHIP_Y + 16.0,
        30.0,
        Color::from_rgba(200, 200, 255, 255),
        INK,
    );
    draw_progress(g, pen, view);
    if g.combo >= 2 && g.state == State::Playing {
        let pulse = 1.0 + 0.06 * (g.clock * 14.0).sin();
        pen.text_outline(
            &format!("Combo x{}", g.combo),
            view.w / 2.0,
            70.0,
            28.0 * pulse,
            Color::from_rgba(255, 220, 90, 255),
            Color::from_rgba(40, 30, 60, 255),
        );
    }
    if g.forgiving() {
        draw_stars(g, pen, view);
    }
    draw_lives(g, pen, view);
}

fn draw_prompts(g: &Breakout, pen: &mut Pen, view: &View) {
    let learning = g.level == 1 && !g.first_win();
    let spot = vec2(view.w / 2.0, view.h * 0.62);
    match g.state {
        State::Serve => {
            let label = if learning {
                "Drag to move, let go to launch"
            } else {
                "Let go to launch"
            };
            pen.hint(label, spot, 1.0 + 0.04 * (g.clock * 4.0).sin());
            if g.first_win() {
                pen.painted_button(serve_levels_button(view), "Levels", Tone::Blue, 0.0, 1.0);
            }
        }
        State::Playing => {
            if learning && g.cleared() == 0 {
                pen.hint(
                    "Keep the ball up!",
                    spot,
                    1.0 + 0.04 * (g.clock * 4.0).sin(),
                );
            }
        }
        State::Lost => {
            let line = format!("Cleared {} of {} bricks", g.cleared(), g.total);
            pen.results(
                view,
                &Results {
                    game: KEY,
                    title: "Nice try!",
                    line: &line,
                    rank: g.rank,
                    action: "Try again",
                    format: plain_score,
                },
                g.age,
            );
            if g.first_win() {
                pen.painted_button(lost_levels_button(view), "Levels", Tone::Blue, 0.0, 1.0);
            }
        }
        State::Cleared => draw_cleared(g, pen, view),
        State::Select => {}
    }
}

pub fn draw(g: &Breakout, pen: &mut Pen, view: &View) {
    draw_arena(pen, view, g.clock);
    if g.state == State::Select {
        draw_select(g, pen, view);
        return;
    }
    pen.offset = g.shake.offset();
    draw_bricks(g, pen);
    draw_ball_and_paddle(g, pen);
    pen.particles(&g.fx);
    pen.popups(&g.popups);
    pen.offset = Vec2::ZERO;
    draw_hud(g, pen, view);
    draw_prompts(g, pen, view);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paint::Tex;

    fn fresh() -> BreakoutPlay {
        kg_core::save::reset_for_test();
        profile::create("Mia", 0);
        BreakoutPlay::new()
    }

    fn drawn(p: &BreakoutPlay, v: &View) -> Pen {
        let mut pen = Pen::default();
        pen.begin(*v, 1.0);
        p.draw(&mut pen, v);
        pen
    }

    fn uses(pen: &Pen, sheet: &str) -> bool {
        pen.used()
            .iter()
            .filter_map(|b| b.tex)
            .any(|t| matches!(t, Tex::Page(s, _) if s == sheet))
    }

    #[test]
    fn the_serve_screen_draws_bricks_ball_text_and_the_hint() {
        let v = View::ipad_landscape();
        let p = fresh();
        let pen = drawn(&p, &v);
        assert!(uses(&pen, "breakout"));
        assert!(uses(&pen, "ui"));
        assert!(pen
            .used()
            .iter()
            .filter_map(|b| b.tex)
            .any(|t| t == Tex::Glyphs));
    }

    #[test]
    fn the_level_select_draws_panels_and_stars() {
        let v = View::ipad_portrait();
        kg_core::save::reset_for_test();
        profile::create("Mia", 0);
        profile::unlock(KEY, 4);
        let p = BreakoutPlay::new();
        assert_eq!(p.g.state, State::Select);
        let pen = drawn(&p, &v);
        assert!(uses(&pen, "ui"));
        assert!(pen.vertex_count() > 500);
    }

    #[test]
    fn the_cleared_and_lost_panels_draw() {
        let v = View::ipad_landscape();
        for state in [State::Cleared, State::Lost] {
            let mut p = fresh();
            p.g.state = state;
            p.g.age = 1.0;
            let pen = drawn(&p, &v);
            assert!(pen.vertex_count() > 500, "{state:?}");
        }
    }

    #[test]
    fn the_arena_covers_the_whole_screen_past_the_safe_area() {
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
}
