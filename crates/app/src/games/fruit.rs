use super::Play;
use crate::draw::{Draw, PLUM};
use crate::paint::Pen;
use crate::ui::{plain_score, Results};
use glam::{vec2, Vec2};
use kg_core::engine::{Game, Input, View, HUD_LEFT};
use kg_core::fx::ease_out_back;
use kg_core::games::fruit::*;
use kg_core::geom::{Color, Rect, WHITE};

pub const KEY: &str = GAME;
const SHEET: &str = "fruit";
const GOLD: Color = Color::from_rgba(255, 220, 60, 255);

pub struct FruitPlay {
    pub g: Fruit,
    overlay_age: f32,
}

impl FruitPlay {
    pub fn new() -> Self {
        Self {
            g: Fruit::new(),
            overlay_age: 0.0,
        }
    }
}

impl Default for FruitPlay {
    fn default() -> Self {
        Self::new()
    }
}

impl Play for FruitPlay {
    fn update(&mut self, input: &Input, view: &View, dt: f32) {
        let before = self.g.state;
        self.g.update(input, view, dt);
        if self.g.state != before {
            self.overlay_age = 0.0;
            if self.g.state == State::Over {
                kg_core::net::request_board(GAME);
            }
        }
        self.overlay_age += dt;
    }

    fn draw(&self, pen: &mut Pen, view: &View) {
        draw(&self.g, pen, view, self.overlay_age);
    }

    fn score(&self) -> u32 {
        self.g.score
    }

    fn over(&self) -> bool {
        self.g.state == State::Over
    }

    fn as_any(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

fn fruit_sprite(
    pen: &mut Pen,
    f: usize,
    suffix: &str,
    c: Vec2,
    r: f32,
    rotation: f32,
    flip_x: bool,
) {
    let side = r * FRUIT_SPAN;
    let name = format!("{}{}", FRUITS[f].name, suffix);
    let (w, h) = fit_in(crate::art::size(SHEET, &name), side);
    pen.sprite(
        SHEET,
        &name,
        Rect::new(c.x - w / 2.0, c.y - h / 2.0, w, h),
        Draw {
            rotation,
            flip_x,
            ..Default::default()
        },
    );
}

fn draw_wall(pen: &mut Pen, view: &View) {
    let f = view.full();
    let side = f.w.max(f.h);
    pen.sprite(
        SHEET,
        "wall",
        Rect::new(
            f.x + (f.w - side) / 2.0,
            f.y + (f.h - side) / 2.0,
            side,
            side,
        ),
        Draw::default(),
    );
}

fn draw_stain(pen: &mut Pen, st: &Stain) {
    let a = stain_alpha(st.age);
    let c = st.color.with_alpha(a);
    let grow = ease_out_back(st.age / 0.25);
    pen.sprite_at(
        SHEET,
        SPLAT,
        st.pos,
        st.r * 2.2 * grow,
        Draw {
            tint: c,
            rotation: st.turn,
            ..Default::default()
        },
    );
    for (i, d) in st.drops.iter().enumerate() {
        let size = st.r * (0.4 + 0.12 * i as f32) * grow;
        pen.sprite_at(
            SHEET,
            SPLAT,
            st.pos + *d * grow,
            size,
            Draw {
                tint: c,
                rotation: st.turn + i as f32,
                ..Default::default()
            },
        );
    }
}

fn draw_half(pen: &mut Pen, h: &Half) {
    pen.soft_shadow(
        h.pos + vec2(h.r * 0.12, h.r * 0.8),
        h.r * 0.6,
        h.r * 0.14,
        0.3,
    );
    fruit_sprite(pen, h.fruit, "_half", h.pos, h.r, h.angle, h.spin > 0.0);
}

fn draw_thing(g: &Fruit, pen: &mut Pen, t: &Thing) {
    match t.kind {
        Kind::Bomb => {
            let pulse = 0.5 + 0.5 * (g.time * 12.0).sin();
            pen.circle(
                t.pos,
                t.r * 1.3,
                Color::new(1.0, 0.3, 0.2, 0.08 + 0.1 * pulse),
            );
            let (bw, bh) = fit_in(crate::art::size(SHEET, "bomb"), t.r * BOMB_SPAN);
            pen.sprite(
                SHEET,
                "bomb",
                Rect::new(t.pos.x - bw / 2.0, t.pos.y - bh / 2.0, bw, bh),
                Draw {
                    rotation: (t.spin * g.time).sin() * 0.4,
                    tint: Color::new(1.0, 1.0 - 0.25 * pulse, 1.0 - 0.3 * pulse, 1.0),
                    ..Default::default()
                },
            );
        }
        Kind::Fruit(f) => {
            pen.soft_shadow(
                t.pos + vec2(t.r * 0.12, t.r * 0.9),
                t.r * 0.8,
                t.r * 0.18,
                0.4,
            );
            fruit_sprite(pen, f, "", t.pos, t.r, t.spin * g.time, false);
        }
    }
}

fn draw_blade(g: &Fruit, pen: &mut Pen) {
    let glow = Color::from_rgba(140, 200, 255, 255);
    for w in g.blade.windows(2) {
        let k = w[1].1 / BLADE_LIFE;
        let (a, b) = (w[0].0, w[1].0);
        for (width, alpha) in [(26.0, 0.12), (16.0, 0.22), (8.0, 0.5)] {
            pen.line(a, b, width * k + 2.0, glow.with_alpha(alpha * k));
        }
        pen.line(a, b, 2.0 + 5.0 * k, WHITE.with_alpha(k));
    }
    if let Some((tip, k)) = g.blade.last() {
        let k = k / BLADE_LIFE;
        pen.circle(*tip, 14.0 * k, glow.with_alpha(0.35 * k));
        pen.circle(*tip, 6.0 * k, WHITE.with_alpha(0.9 * k));
    }
}

fn draw_hints(g: &Fruit, pen: &mut Pen, view: &View) {
    let pulse = 1.0 + (g.time * 5.0).sin() * 0.04;
    let at = vec2(view.w / 2.0, view.h * 0.86);
    if g.show_swipe_hint() {
        pen.hint("Swipe across the fruit!", at, pulse);
    } else if g.show_bomb_hint() {
        pen.hint("Don't slice the bombs!", at, pulse);
    }
}

fn draw_level_bar(g: &Fruit, pen: &mut Pen, view: &View) {
    let w = 240.0f32.min(view.w * 0.32);
    let cx = view.w / 2.0;
    pen.text_outline(&format!("Level {}", g.level()), cx, 30.0, 30.0, WHITE, PLUM);
    let track = Rect::new(cx - w / 2.0, 54.0, w, 18.0);
    pen.round_rect(track, 9.0, Color::new(0.1, 0.05, 0.2, 0.55));
    let k = g.level_progress();
    if k > 0.0 {
        let fill = Rect::new(track.x + 2.0, track.y + 2.0, (w - 4.0) * k, 14.0);
        pen.round_rect(fill, 7.0, Color::from_rgba(255, 210, 60, 255));
    }
}

fn draw_hud(g: &Fruit, pen: &mut Pen, view: &View) {
    draw_level_bar(g, pen, view);
    pen.text_outline(
        &g.score.to_string(),
        HUD_LEFT + 50.0,
        50.0,
        60.0 * g.score_bump.scale(0.35),
        GOLD,
        PLUM,
    );
    let pulse = g.heart_bump.scale(0.5);
    for i in 0..LIVES {
        let x = view.w - 50.0 - i as f32 * 54.0;
        let alive = i < g.lives;
        let k = if i == g.lives { pulse } else { 1.0 };
        pen.sprite_at(
            SHEET,
            "heart",
            vec2(x, 48.0),
            54.0 * k,
            Draw::tint(if alive {
                WHITE
            } else {
                Color::new(0.25, 0.25, 0.3, 0.45)
            }),
        );
    }
}

pub fn draw(g: &Fruit, pen: &mut Pen, view: &View, overlay_age: f32) {
    pen.offset = g.shake.offset();
    draw_wall(pen, view);
    for st in &g.stains {
        draw_stain(pen, st);
    }
    for t in &g.things {
        draw_thing(g, pen, t);
    }
    for h in &g.halves {
        draw_half(pen, h);
    }
    pen.particles(&g.fx);
    draw_blade(g, pen);
    pen.popups(&g.popups);
    pen.offset = Vec2::ZERO;
    pen.vignette(view, 0.6);
    draw_hud(g, pen, view);
    if g.flash > 0.0 {
        pen.fill_screen(view, WHITE.with_alpha(g.flash));
    }
    match g.state {
        State::Ready => {
            let resume = format!("Starting at level {}", g.start_level);
            if g.start_level > 1 {
                pen.panel(
                    view,
                    "Fruit Slice",
                    &["Swipe across fruit to slice it", &resume, "Tap to start"],
                    overlay_age,
                );
            } else {
                pen.panel(
                    view,
                    "Fruit Slice",
                    &["Swipe across fruit to slice it", "Tap to start"],
                    overlay_age,
                );
            }
        }
        State::Over => {
            let line = format!("Score {}   Level {}", g.score, g.level());
            pen.results(
                view,
                &Results {
                    game: GAME,
                    title: "Game over",
                    line: &line,
                    rank: g.rank,
                    action: "Play again",
                    format: plain_score,
                },
                overlay_age,
            );
        }
        State::Playing => draw_hints(g, pen, view),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paint::Tex;

    fn fresh() -> FruitPlay {
        kg_core::save::reset_for_test();
        kg_core::profile::create("Mia", 0);
        FruitPlay::new()
    }

    fn drawn(p: &FruitPlay, v: &View) -> Pen {
        let mut pen = Pen::default();
        pen.begin(*v, 1.0);
        p.draw(&mut pen, v);
        pen
    }

    fn has_fruit_page(pen: &Pen) -> bool {
        pen.used()
            .iter()
            .filter_map(|b| b.tex)
            .any(|t| matches!(t, Tex::Page("fruit", _)))
    }

    #[test]
    fn the_ready_screen_draws_the_wall_and_the_panel() {
        let p = fresh();
        let pen = drawn(&p, &View::ipad_landscape());
        let texs: Vec<_> = pen.used().iter().filter_map(|b| b.tex).collect();
        assert!(has_fruit_page(&pen));
        assert!(texs.contains(&Tex::Glyphs));
        assert!(texs.iter().any(|t| matches!(t, Tex::Page("ui", _))));
    }

    #[test]
    fn the_wall_covers_the_whole_screen_past_the_safe_area() {
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
        let wall = &pen.used()[0];
        let (mut lo, mut hi) = (f32::MAX, f32::MIN);
        let (mut top, mut bot) = (f32::MAX, f32::MIN);
        for q in &wall.pos {
            lo = lo.min(q[0]);
            hi = hi.max(q[0]);
            top = top.min(-q[1]);
            bot = bot.max(-q[1]);
        }
        assert!(
            lo <= f.x + 0.01 && hi >= f.x + f.w,
            "wall {lo}..{hi} vs {f:?}"
        );
        assert!(top <= f.y + 0.5 && bot >= f.y + f.h - 0.5);
    }

    #[test]
    fn a_slice_draws_halves_stains_and_the_blade() {
        let v = View::ipad_landscape();
        let mut p = fresh();
        p.g.state = State::Playing;
        p.g.things.push(Thing {
            pos: vec2(300.0, 300.0),
            vel: Vec2::ZERO,
            r: 40.0,
            kind: Kind::Fruit(2),
            spin: 0.0,
        });
        let before = drawn(&p, &v).vertex_count();
        p.g.slice(vec2(200.0, 300.0), vec2(400.0, 300.0));
        p.update(&Input::default(), &v, 0.2);
        let pen = drawn(&p, &v);
        assert!(!p.g.halves.is_empty() && !p.g.stains.is_empty());
        assert!(pen.vertex_count() > before);
    }

    #[test]
    fn the_results_panel_shows_after_game_over_and_restarts_its_age() {
        let v = View::ipad_landscape();
        let mut p = fresh();
        p.g.start_level = 2;
        p.update(&Input::press_at(vec2(100.0, 100.0)), &v, 0.01);
        p.g.lives = 0;
        p.update(&Input::default(), &v, 0.01);
        assert_eq!(p.g.state, State::Over);
        for _ in 0..90 {
            p.update(&Input::default(), &v, 1.0 / 60.0);
        }
        assert!(p.overlay_age > 1.0);
        let pen = drawn(&p, &v);
        assert!(pen.vertex_count() > 500);
        p.update(&Input::press_at(vec2(100.0, 100.0)), &v, 0.01);
        assert_eq!(p.g.state, State::Playing);
        assert!(p.overlay_age < 0.1);
    }

    #[test]
    fn a_bomb_in_play_draws_from_the_fruit_sheet() {
        let v = View::ipad_landscape();
        let mut p = fresh();
        p.g.state = State::Playing;
        p.g.things.push(Thing {
            pos: vec2(300.0, 300.0),
            vel: Vec2::ZERO,
            r: 34.0,
            kind: Kind::Bomb,
            spin: 1.0,
        });
        assert!(has_fruit_page(&drawn(&p, &v)));
    }
}
