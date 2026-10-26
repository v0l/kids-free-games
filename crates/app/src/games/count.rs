use super::Play;
use crate::draw::{measure, Draw};
use crate::paint::Pen;
use crate::ui::Results;
use glam::{vec2, Vec2};
use kg_core::engine::{Game, Input, View, HUD_LEFT};
use kg_core::fx::{ease_out_back, ease_out_cubic, Bump};
use kg_core::games::count::*;
use kg_core::geom::{darker, with_alpha, Color, Rect, WHITE};
use kg_core::learn::{hud_bottom, ui_scale, Ladder, UP_STREAK};

pub const KEY: &str = GAME;
const SHEET: &str = "count";
const CLOUDS: [&str; 8] = [
    "cloud1", "cloud2", "cloud3", "cloud4", "cloud5", "cloud6", "cloud7", "cloud8",
];
const INK: Color = Color::from_rgba(40, 30, 60, 255);
const STAR_GOLD: Color = Color::from_rgba(255, 210, 60, 255);
const BLUE_INK: Color = Color::from_rgba(10, 70, 140, 255);

pub struct CountPlay {
    pub g: Count,
}

impl CountPlay {
    pub fn new() -> Self {
        Self { g: Count::new() }
    }
}

impl Default for CountPlay {
    fn default() -> Self {
        Self::new()
    }
}

fn cleared(g: &Count) -> bool {
    matches!(g.mode, Mode::Report(_)) && matches!(g.report, Some(Report::Cleared { .. }))
}

impl Play for CountPlay {
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

pub fn thing_height(name: &str, r: f32) -> f32 {
    let s = crate::art::size(SHEET, name);
    2.2 * r * s.y / s.x.max(s.y)
}

fn draw_thing(pen: &mut Pen, thing: Thing, c: Vec2, r: f32, t: f32) {
    let name = thing.sprite();
    let wiggle = match thing {
        Thing::Star => (t * 2.0).sin() * 0.15,
        Thing::Fish | Thing::Bee => (t * 5.0).sin() * 0.08,
        _ => 0.0,
    };
    pen.sprite_at(SHEET, name, c, thing_height(name, r), Draw::rot(wiggle));
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
    for i in 0..5usize {
        let span = view.w + 500.0;
        let x = (i as f32 * 0.23 * span + t * (7.0 + i as f32 * 3.0)) % span - 250.0;
        let y = hud_bottom(view) + 10.0 + (i as f32 * 0.37 % 1.0) * view.h * 0.25;
        pen.sprite_w(
            "ui",
            CLOUDS[(i * 2 + 1) % CLOUDS.len()],
            vec2(x, y),
            150.0 + (i % 3) as f32 * 50.0,
            Draw::tint(with_alpha(WHITE, 0.9)),
        );
    }
}

fn draw_hud(pen: &mut Pen, view: &View, ladder: &Ladder, label: &str, accent: Color, bump: &Bump) {
    let k = ui_scale(view);
    let tw = measure(label, 28.0 * k) + 44.0 * k;
    let chip = Rect::new(HUD_LEFT, 18.0, tw, 46.0 * k);
    pen.round_rect(
        Rect::new(chip.x, chip.y + 4.0, chip.w, chip.h),
        chip.h / 2.0,
        Color::new(0.0, 0.0, 0.0, 0.18),
    );
    pen.round_rect(chip, chip.h / 2.0, with_alpha(WHITE, 0.94));
    pen.text_center(
        label,
        chip.x + chip.w / 2.0,
        chip.y + chip.h / 2.0,
        28.0 * k,
        darker(accent, 0.6),
    );
    for i in 0..ladder.levels {
        let from_right = (ladder.levels - 1 - i) as f32;
        let c = vec2(
            view.w - 30.0 - 22.0 * k - from_right * 44.0 * k,
            chip.y + chip.h / 2.0,
        );
        pen.star(
            c + vec2(0.0, 2.0),
            22.0 * k,
            0.0,
            Color::new(0.1, 0.05, 0.25, 0.35),
        );
        let fill = if i < ladder.stars {
            STAR_GOLD
        } else {
            with_alpha(WHITE, 0.55)
        };
        pen.star(c, 20.0 * k, 0.0, fill);
    }
    let bar_w = (view.w - 180.0).min(520.0 * k);
    let gap = 8.0 * k;
    let seg = (bar_w - gap * (UP_STREAK as f32 - 1.0)) / UP_STREAK as f32;
    let x0 = view.w / 2.0 - bar_w / 2.0;
    for j in 0..UP_STREAK {
        let r = Rect::new(
            x0 + j as f32 * (seg + gap),
            chip.y + chip.h + 12.0,
            seg,
            18.0 * k,
        );
        let filled = j < ladder.streak;
        let last = filled && j + 1 == ladder.streak;
        let sc = if last { bump.scale(0.35) } else { 1.0 };
        let rr = Rect::new(
            r.x + r.w * (1.0 - sc) / 2.0,
            r.y + r.h * (1.0 - sc) / 2.0,
            r.w * sc,
            r.h * sc,
        );
        pen.round_rect(
            Rect::new(rr.x, rr.y + 3.0, rr.w, rr.h),
            rr.h / 2.0,
            Color::new(0.0, 0.0, 0.0, 0.15),
        );
        let col = if filled {
            STAR_GOLD
        } else {
            with_alpha(WHITE, 0.5)
        };
        pen.round_rect(rr, rr.h / 2.0, col);
        if filled {
            pen.gloss(rr, rr.h / 2.0);
        }
    }
}

fn draw_goal(g: &Count, pen: &mut Pen, view: &View) {
    let k = ui_scale(view);
    let y = goal_row_y(view);
    let button = levels_button(view);
    let room = if g.first_time {
        view.w - 2.0 * HUD_LEFT
    } else {
        button.x - 20.0 - HUD_LEFT
    };
    let text = g.goal_text();
    let size = (26.0 * k).min(room / text.len() as f32 * 2.0);
    let left = if g.first_time {
        view.w / 2.0
    } else {
        HUD_LEFT + room / 2.0
    };
    pen.text_outline(&text, left, y, size, WHITE, INK);
    if !g.first_time {
        pen.painted_button(button, "Levels", crate::ui::Tone::Blue, 0.0, 1.0);
    }
}

fn draw_levels(g: &Count, pen: &mut Pen, view: &View) {
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
        let tint = if open { WHITE } else { with_alpha(WHITE, 0.55) };
        pen.painted_panel(*r, tint);
        let (low, high) = RANGES[i];
        let label = format!("{low} to {high}");
        if open {
            pen.text_outline(
                &label,
                r.center().x,
                r.y + r.h * 0.38,
                r.h * 0.26,
                WHITE,
                BLUE_INK,
            );
            let earned = kg_core::profile::stars(GAME, i as u32);
            for j in 0..3u8 {
                let c = vec2(
                    r.center().x + (j as f32 - 1.0) * r.h * 0.26,
                    r.y + r.h * 0.72,
                );
                let fill = if j < earned {
                    STAR_GOLD
                } else {
                    Color::from_rgba(110, 90, 140, 150)
                };
                pen.star(c, r.h * 0.11, 0.0, fill);
            }
        } else {
            pen.sprite_at(
                "ui",
                "icon_lock",
                r.center() - vec2(0.0, r.h * 0.05),
                r.h * 0.34,
                Draw::tint(INK),
            );
            pen.text_center(
                &label,
                r.center().x,
                r.y + r.h * 0.8,
                r.h * 0.14,
                with_alpha(INK, 0.6),
            );
        }
    }
    pen.painted_button(play_button(view), "Play", crate::ui::Tone::Green, 0.0, 1.0);
}

fn draw_report(g: &Count, pen: &mut Pen, view: &View, age: f32) {
    match g.report {
        Some(Report::Level { level, stars }) => {
            let title = format!("{} done!", level_name(level));
            let next = if level + 1 < RANGES.len() {
                format!("Next: {}", level_name(level + 1))
            } else {
                "Counting champion!".to_string()
            };
            pen.panel(view, &title, &["", &next, &g.summary], age);
            let c = view.center() + vec2(0.0, 20.0);
            let ph = 150.0 + 3.0 * 64.0;
            let y = c.y - ph / 2.0 + 118.0;
            for j in 0..3u8 {
                let k = ease_out_back(((age - 0.3 - j as f32 * 0.15) / 0.3).clamp(0.0, 1.0));
                let fill = if j < stars {
                    STAR_GOLD
                } else {
                    Color::from_rgba(150, 140, 170, 140)
                };
                pen.star(vec2(c.x + (j as f32 - 1.0) * 90.0, y), 38.0 * k, 0.0, fill);
            }
        }
        Some(Report::Cleared { rank }) => {
            let total = kg_core::profile::total_stars(GAME);
            let line = format!("{total} stars earned");
            pen.results(
                view,
                &Results {
                    game: GAME,
                    title: "You can count to 20!",
                    line: &line,
                    rank,
                    action: "Keep counting",
                    format: stars_score,
                },
                age,
            );
        }
        None => {}
    }
}

fn draw_prompt(g: &Count, pen: &mut Pen, view: &View) {
    let s = format!("How many {}?", g.question.thing.plural());
    let pop = ease_out_back(g.prompt_bump.t).max(0.2);
    pen.text_outline(
        &s,
        view.w / 2.0,
        prompt_y(view),
        prompt_size(view) * pop,
        WHITE,
        INK,
    );
}

fn draw_things(g: &Count, pen: &mut Pen, view: &View) {
    let area = play_area(view);
    pen.painted_panel(
        Rect::new(area.x - 24.0, area.y - 18.0, area.w + 48.0, area.h + 36.0),
        with_alpha(WHITE, 0.4),
    );
    let q = &g.question;
    let (points, r) = scatter(q.count as usize, q.seed, area);
    for (i, p) in points.iter().enumerate() {
        let k = (g.bounce[i] / TICK_BOUNCE).clamp(0.0, 1.0);
        let hop = (k * std::f32::consts::PI).sin() * r * 0.6;
        let party = g.celebrate.map_or(0.0, |t| {
            ((t * 9.0 - i as f32 * 0.7).sin().max(0.0)) * r * 0.25
        });
        let squash = 1.0 + 0.25 * (1.0 - ease_out_cubic(k)) * (k < 1.0) as i32 as f32;
        let c = *p - vec2(0.0, hop + party);
        pen.soft_shadow(*p + vec2(0.0, r * 0.85), r * 0.7, r * 0.16, 0.6);
        let intro = ease_out_back((g.t * 2.0 - i as f32 * 0.04).clamp(0.0, 1.0));
        draw_thing(pen, q.thing, c, r * squash * intro, g.t + i as f32);
    }
    for (i, p) in points.iter().enumerate() {
        let Some(n) = g.number_shown(i) else { continue };
        let k = (g.bounce[i] / TICK_BOUNCE).clamp(0.0, 1.0);
        let hop = (k * std::f32::consts::PI).sin() * r * 0.6;
        let party = g.celebrate.map_or(0.0, |t| {
            ((t * 9.0 - i as f32 * 0.7).sin().max(0.0)) * r * 0.25
        });
        let c = *p - vec2(0.0, hop + party);
        let b = r * 0.55;
        pen.ui(
            "p_round",
            Rect::new(c.x - b * 1.1, c.y + r * 0.1 - b * 1.1, b * 2.2, b * 2.2),
            Draw::default(),
        );
        pen.text_outline(
            &n.to_string(),
            c.x,
            c.y + r * 0.08,
            b * 1.2,
            WHITE,
            BLUE_INK,
        );
    }
}

fn draw_buttons(g: &Count, pen: &mut Pen, view: &View) {
    use crate::ui::Tone;
    let rects = button_rects(view, g.question.options.len());
    for (i, (r, v)) in rects.iter().zip(&g.question.options).enumerate() {
        let tried = g.attempt.tried[i];
        let hint = g.attempt.revealed() && *v == g.question.count;
        let pop = g.press[i].scale(0.12);
        let wob = (g.wobble[i] * 36.0).sin() * 12.0 * g.wobble[i];
        let c = r.center() + vec2(wob, 0.0);
        let rr = Rect::new(
            c.x - r.w * pop / 2.0,
            c.y - r.h * pop / 2.0,
            r.w * pop,
            r.h * pop,
        );
        if hint {
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
        let correct_now = g.attempt.solved && *v == g.question.count;
        let tone = if correct_now {
            Tone::Green
        } else {
            [Tone::Orange, Tone::Blue, Tone::Green, Tone::Orange][i % 4]
        };
        let alpha = if tried { 0.45 } else { 1.0 };
        pen.painted_button(rr, &v.to_string(), tone, 0.0, alpha);
    }
}

pub fn draw(g: &Count, pen: &mut Pen, view: &View) {
    draw_backdrop(pen, view, g.t);
    if g.mode == Mode::Levels {
        draw_levels(g, pen, view);
        return;
    }
    draw_hud(
        pen,
        view,
        &g.ladder,
        &level_name(g.ladder.level),
        Color::from_rgba(250, 170, 60, 255),
        &g.streak_bump,
    );
    pen.offset = g.shake.offset();
    draw_goal(g, pen, view);
    draw_prompt(g, pen, view);
    draw_things(g, pen, view);
    draw_buttons(g, pen, view);
    if g.first_time && g.mode == Mode::Play {
        let pulse = 1.0 + (g.t * 4.0).sin() * 0.03;
        let y = button_rects(view, g.question.options.len())[0].y - 36.0;
        pen.hint(
            "Tap each one to count, then pick the number",
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
    use crate::paint::Tex;

    fn fresh() -> CountPlay {
        kg_core::save::reset_for_test();
        kg_core::profile::create("Mia", 0);
        CountPlay::new()
    }

    fn drawn(p: &CountPlay, v: &View) -> Pen {
        let mut pen = Pen::default();
        pen.begin(*v, 1.0);
        p.draw(&mut pen, v);
        pen
    }

    fn texs(pen: &Pen) -> Vec<Tex> {
        pen.used().iter().filter_map(|b| b.tex).collect()
    }

    #[test]
    fn count_sprites_all_exist() {
        let have = crate::art::names(SHEET);
        for n in ["bg"]
            .into_iter()
            .chain(Thing::ALL.iter().map(|t| t.sprite()))
        {
            assert!(have.iter().any(|h| h == n), "missing {n}");
        }
        let ui = crate::art::names("ui");
        for n in [
            "icon_lock",
            "p_round",
            "cloud1",
            "cloud2",
            "cloud6",
            "cloud8",
        ] {
            assert!(ui.iter().any(|h| h == n), "missing {n}");
        }
    }

    #[test]
    fn every_thing_fits_inside_its_scatter_cell() {
        for t in Thing::ALL {
            let s = crate::art::size(SHEET, t.sprite());
            let h = thing_height(t.sprite(), 50.0);
            assert!(h <= 2.2 * 50.0 + 0.01);
            assert!(h * s.x / s.y <= 2.2 * 50.0 + 0.01);
        }
    }

    #[test]
    fn the_question_screen_draws_backdrop_things_buttons_and_text() {
        let v = View::ipad_landscape();
        let pen = drawn(&fresh(), &v);
        let t = texs(&pen);
        assert!(t.iter().any(|t| matches!(t, Tex::Page("count", _))));
        assert!(t.iter().any(|t| matches!(t, Tex::Page("ui", _))));
        assert!(t.contains(&Tex::Glyphs));
    }

    #[test]
    fn the_level_picker_draws_in_both_orientations() {
        for v in [View::ipad_landscape(), View::ipad_portrait()] {
            let mut p = fresh();
            p.g.mode = Mode::Levels;
            assert!(drawn(&p, &v).vertex_count() > 500);
        }
    }

    #[test]
    fn both_reports_draw() {
        let v = View::ipad_landscape();
        let mut p = fresh();
        p.g.report = Some(Report::Level { level: 0, stars: 2 });
        p.g.mode = Mode::Report(1.0);
        assert!(drawn(&p, &v).vertex_count() > 800);
        p.g.report = Some(Report::Cleared { rank: Some(0) });
        assert!(drawn(&p, &v).vertex_count() > 800);
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
        let pen = drawn(&fresh(), &v);
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
    fn clearing_the_top_level_opens_the_board_report() {
        let v = View::ipad_landscape();
        let mut p = fresh();
        let last = RANGES.len() - 1;
        kg_core::profile::unlock(GAME, last as u32);
        p.g.start_level(last);
        for _ in 0..UP_STREAK {
            let slot = p.g.question.slot_of_answer();
            let tap = p.g.pick(slot);
            p.g.react(&v, slot, tap);
            for _ in 0..400 {
                p.update(&Input::default(), &v, 1.0 / 60.0);
            }
        }
        assert!(cleared(&p.g));
        assert!(drawn(&p, &v).vertex_count() > 800);
    }
}
