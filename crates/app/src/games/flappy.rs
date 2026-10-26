use super::Play;
use crate::draw::Draw;
use crate::paint::Pen;
use crate::ui::{plain_score, Results};
use glam::vec2;
use kg_core::engine::{Game, Input, View};
use kg_core::fx::Side;
use kg_core::games::flappy::*;
use kg_core::geom::{Color, Rect, WHITE};

pub const KEY: &str = GAME;
const SHEET: &str = "flappy";

pub struct FlappyPlay {
    pub g: Flappy,
    results_age: f32,
}

impl FlappyPlay {
    pub fn new() -> Self {
        Self {
            g: Flappy::new(),
            results_age: 0.0,
        }
    }
}

impl Default for FlappyPlay {
    fn default() -> Self {
        Self::new()
    }
}

impl Play for FlappyPlay {
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

    fn as_any(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

fn band(pen: &mut Pen, name: &str, view: &View, bottom: f32, h: f32, scroll: f32, tint: Color) {
    let f = view.full();
    let s = crate::art::size(SHEET, name);
    let w = h * s.x / s.y;
    for x in tile_x(f.x, f.w, w, scroll) {
        pen.sprite(
            SHEET,
            name,
            Rect::new(x, bottom - h, w + 1.0, h),
            Draw::tint(tint),
        );
    }
}

fn draw_pipe(pen: &mut Pen, p: &Pipe, view: &View) {
    let [top, bottom] = Flappy::pipe_rects(p, view);
    let cap = crate::art::size(SHEET, "pipe_cap");
    let cw = PIPE_W * 1.24;
    let ch = cw * cap.y / cap.x;
    let bw = PIPE_W * 1.02;
    let cx = p.x + PIPE_W / 2.0;
    let column = |pen: &mut Pen, from: f32, to: f32| {
        if to > from {
            pen.sprite(
                SHEET,
                "pipe_body",
                Rect::new(cx - bw / 2.0, from, bw, to - from),
                Draw::default(),
            );
        }
    };
    column(pen, top.y, top.y + top.h - ch * 0.6);
    pen.sprite(
        SHEET,
        "pipe_cap",
        Rect::new(cx - cw / 2.0, top.y + top.h - ch, cw, ch),
        Draw {
            flip_y: true,
            ..Default::default()
        },
    );
    column(pen, bottom.y + ch * 0.6, bottom.y + bottom.h);
    pen.sprite(
        SHEET,
        "pipe_cap",
        Rect::new(cx - cw / 2.0, bottom.y, cw, ch),
        Draw::default(),
    );
}

fn draw_bird(g: &Flappy, pen: &mut Pen, view: &View) {
    let bx = Flappy::bird_x(view);
    let sc = bird_scale(g.vy, g.kick);
    let ground_y = view.h - GROUND;
    let height = (ground_y - g.y).max(0.0);
    pen.soft_shadow(
        vec2(bx, ground_y + 6.0),
        BIRD_R * (1.4 - (height / 600.0).min(0.7)),
        5.0,
        0.5,
    );
    let frame = g.bird_frame();
    let h = BIRD_R * 3.4;
    let s = crate::art::size(SHEET, frame);
    let w = h * s.x / s.y;
    pen.sprite(
        SHEET,
        frame,
        Rect::new(
            bx - w * sc.x / 2.0,
            g.y - h * sc.y / 2.0,
            w * sc.x,
            h * sc.y,
        ),
        Draw::rot(g.bird_angle()),
    );
}

fn draw_world(g: &Flappy, pen: &mut Pen, view: &View) {
    let f = view.full();
    let gy = view.h - GROUND;
    let tint = stage_tint(stage_of(g.score));
    band(pen, "sky", view, f.y + f.h, f.h, g.scroll * 0.08, tint);
    band(
        pen,
        "hills",
        view,
        gy + 20.0,
        (f.h * 0.42).max(260.0),
        g.scroll * 0.35,
        tint,
    );
    for p in &g.pipes {
        draw_pipe(pen, p, view);
    }
    let gs = crate::art::size(SHEET, "ground");
    let gh = GROUND + 34.0 + view.inset.bottom + 40.0;
    let top_frac = 116.0 / gs.y;
    let h = (GROUND + 34.0) / (1.0 - top_frac);
    let w = h * gs.x / gs.y;
    for x in tile_x(f.x, f.w, w, g.scroll) {
        pen.sprite(
            SHEET,
            "ground",
            Rect::new(x, gy - 34.0 - h * top_frac, w + 1.0, h.max(gh)),
            Draw::default(),
        );
    }
    draw_bird(g, pen, view);
}

fn draw_goal(g: &Flappy, pen: &mut Pen, view: &View) {
    let frac = g.goal_progress();
    let w = 300.0f32.min(view.w * 0.5);
    let r = Rect::new(view.w / 2.0 - w / 2.0, 118.0, w, 22.0);
    pen.round_rect(r, 11.0, Color::new(0.1, 0.05, 0.2, 0.45));
    if frac > 0.0 {
        let fill = Rect::new(r.x + 2.0, r.y + 2.0, (r.w - 4.0) * frac, r.h - 4.0);
        pen.round_rect(fill, fill.h / 2.0, Color::from_rgba(255, 200, 40, 255));
    }
    let label = format!("Next: {}", stage_name(stage_of(g.score) + 1));
    pen.text_shadow(&label, view.w / 2.0, r.y + 46.0, 26.0, WHITE);
}

pub fn draw(g: &Flappy, pen: &mut Pen, view: &View, results_age: f32) {
    pen.offset = g.shake.offset();
    draw_world(g, pen, view);
    pen.particles(&g.fx);
    pen.popups(&g.popups);
    pen.offset = glam::Vec2::ZERO;
    if g.flash > 0.0 {
        pen.fill_screen(view, WHITE.with_alpha(g.flash * 0.7));
    }
    if g.state == State::Playing {
        pen.big_score(view, &g.score.to_string(), 80.0, g.bump.t);
        draw_goal(g, pen, view);
        if g.training {
            let pulse = 1.0 + (g.t * 5.0).sin() * 0.05;
            pen.hint(
                "Tap to fly up, fly through the gap",
                vec2(view.w / 2.0, view.h * 0.8),
                pulse,
            );
        }
    }
    pen.chip(view, Side::Right, "BEST", &g.best.0.to_string(), 1.0);
    match g.state {
        State::Ready => {
            let bob = (g.t * 4.0).sin() * 6.0;
            pen.ribbon(
                view,
                "Get Ready!",
                vec2(view.w / 2.0, view.h * 0.2 + bob),
                g.t,
            );
            let pulse = 1.0 + (g.t * 5.0).sin() * 0.05;
            pen.hint("Tap to flap", vec2(view.w / 2.0, view.h * 0.74), pulse);
            pen.text_shadow(&g.ready_line(), view.w / 2.0, view.h * 0.84, 32.0, WHITE);
        }
        State::Dead if g.shows_results() => {
            let line = g.result_line();
            pen.results(
                view,
                &Results {
                    game: GAME,
                    title: "Bonk!",
                    line: &line,
                    rank: g.rank,
                    action: "Fly again",
                    format: plain_score,
                },
                results_age,
            );
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paint::Tex;

    fn trained() -> FlappyPlay {
        kg_core::save::reset_for_test();
        kg_core::profile::create("Mia", 0);
        kg_core::profile::set(GAME, "passed", 1);
        FlappyPlay::new()
    }

    fn drawn(p: &FlappyPlay, v: &View) -> Pen {
        let mut pen = Pen::default();
        pen.begin(*v, 1.0);
        p.draw(&mut pen, v);
        pen
    }

    #[test]
    fn the_ready_screen_draws_sky_pipes_bird_and_text() {
        let v = View::ipad_landscape();
        let p = trained();
        let pen = drawn(&p, &v);
        let texs: Vec<_> = pen.used().iter().filter_map(|b| b.tex).collect();
        assert!(texs.iter().any(|t| matches!(t, Tex::Page("flappy", _))));
        assert!(texs.contains(&Tex::Glyphs));
        assert!(texs.iter().any(|t| matches!(t, Tex::Page("ui", _))));
    }

    #[test]
    fn the_world_covers_the_whole_screen_past_the_safe_area() {
        let v = View::fit_safe(
            2400.0,
            1080.0,
            kg_core::safe::Insets {
                left: 120.0,
                bottom: 40.0,
                ..Default::default()
            },
        );
        let p = trained();
        let pen = drawn(&p, &v);
        let f = v.full();
        let sky = &pen.used()[0];
        let (mut lo, mut hi) = (f32::MAX, f32::MIN);
        let (mut top, mut bot) = (f32::MAX, f32::MIN);
        for q in &sky.pos {
            lo = lo.min(q[0]);
            hi = hi.max(q[0]);
            top = top.min(-q[1]);
            bot = bot.max(-q[1]);
        }
        assert!(
            lo <= f.x + 0.01 && hi >= f.x + f.w,
            "sky {lo}..{hi} vs {f:?}"
        );
        assert!(top <= f.y + 0.5 && bot >= f.y + f.h - 0.5);
    }

    #[test]
    fn the_results_panel_appears_after_a_crash_and_its_age_restarts_each_run() {
        let v = View::ipad_landscape();
        let mut p = trained();
        p.update(&Input::press_at(vec2(500.0, 500.0)), &v, 1.0 / 60.0);
        for _ in 0..400 {
            p.update(&Input::default(), &v, 1.0 / 60.0);
        }
        assert!(p.g.shows_results());
        assert!(p.results_age > 1.0);
        let pen = drawn(&p, &v);
        assert!(pen.vertex_count() > 500);
        p.update(&Input::press_at(vec2(500.0, 500.0)), &v, 1.0 / 60.0);
        assert_eq!(p.g.state, State::Ready);
        p.update(&Input::press_at(vec2(500.0, 500.0)), &v, 1.0 / 60.0);
        for _ in 0..400 {
            p.update(&Input::default(), &v, 1.0 / 60.0);
        }
        assert!(p.results_age < 6.0);
    }
}
