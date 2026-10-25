use crate::engine::*;
use crate::feel::{self, Impact};
use crate::fx::*;
use crate::sfx::{self, Sfx};
use macroquad::prelude::*;

pub const GRAVITY: f32 = 1500.0;
pub const FLAP: f32 = -520.0;
pub const PIPE_W: f32 = 96.0;
pub const BIRD_R: f32 = 22.0;
pub const GROUND: f32 = 90.0;
pub const GAME: &str = "flappy";
pub const MILESTONE: u32 = 10;
pub const MOVING_FROM_STAGE: u32 = 2;
pub const TRAINING_PIPES: u32 = 2;
const STAGE_NAMES: [&str; 4] = ["Sunny Day", "Sunset", "Dusk", "Night"];

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tuning {
    pub speed: f32,
    pub gap: f32,
    pub spacing: f32,
    pub jitter: f32,
    pub amp: f32,
}

pub fn stage_of(score: u32) -> u32 {
    score / MILESTONE
}

pub fn next_goal(score: u32) -> u32 {
    (stage_of(score) + 1) * MILESTONE
}

pub fn stage_name(stage: u32) -> String {
    match STAGE_NAMES.get(stage as usize) {
        Some(n) => n.to_string(),
        None => format!("Level {}", stage + 1),
    }
}

pub fn stage_tint(stage: u32) -> Color {
    match stage % 4 {
        0 => WHITE,
        1 => Color::new(1.0, 0.82, 0.66, 1.0),
        2 => Color::new(0.82, 0.7, 1.0, 1.0),
        _ => Color::new(0.5, 0.56, 0.9, 1.0),
    }
}

pub fn tuning(score: u32, help: u32) -> Tuning {
    let k = (score as f32 / 30.0).min(1.0);
    let fade = (1.0 - score as f32 / 25.0).clamp(0.0, 1.0);
    let help = help.min(3) as f32;
    let stage = stage_of(score);
    let amp = if stage >= MOVING_FROM_STAGE {
        (25.0 + 10.0 * (stage - MOVING_FROM_STAGE) as f32).min(60.0)
    } else {
        0.0
    };
    Tuning {
        speed: 150.0 + 70.0 * k - 10.0 * help * fade,
        gap: 330.0 - 90.0 * k + 20.0 * help * fade,
        spacing: 390.0 - 50.0 * k,
        jitter: 40.0 + 160.0 * k,
        amp,
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum State {
    Ready,
    Playing,
    Dead,
}

pub struct Pipe {
    pub x: f32,
    pub gap_y: f32,
    pub gap: f32,
    pub base_y: f32,
    pub amp: f32,
    pub phase: f32,
    pub scored: bool,
}

impl Pipe {
    pub fn new(x: f32, gap_y: f32) -> Self {
        Self {
            x,
            gap_y,
            gap: 230.0,
            base_y: gap_y,
            amp: 0.0,
            phase: 0.0,
            scored: false,
        }
    }

    pub fn sway(&mut self, t: f32) {
        self.gap_y = self.base_y + self.amp * (self.phase + t * 1.6).sin();
    }
}

pub fn layer_x(i: usize, spacing: f32, scroll: f32, factor: f32, span: f32, margin: f32) -> f32 {
    (i as f32 * spacing - scroll * factor).rem_euclid(span) - margin
}

pub fn bird_scale(vy: f32, kick: f32) -> Vec2 {
    let len = 1.0 + (vy.abs() / 1400.0).min(0.2) + kick.clamp(0.0, 1.0) * 0.12;
    vec2(len, 1.0 / len)
}

pub struct Flappy {
    pub state: State,
    pub y: f32,
    pub vy: f32,
    pub pipes: Vec<Pipe>,
    pub score: u32,
    pub best: Best,
    pub rank: Option<usize>,
    pub speed: f32,
    pub training: bool,
    pub help: u32,
    pub furthest: u32,
    t: f32,
    dead_t: f32,
    scroll: f32,
    kick: f32,
    flash: f32,
    fx: Particles,
    shake: Shake,
    popups: Popups,
    bump: Bump,
}

impl Flappy {
    pub fn new() -> Self {
        Self {
            state: State::Ready,
            y: 300.0,
            vy: 0.0,
            pipes: vec![],
            score: 0,
            best: Best::load(GAME),
            rank: None,
            speed: tuning(0, 0).speed,
            training: crate::profile::get(GAME, "passed") == 0,
            help: crate::profile::get(GAME, "struggle").clamp(0, 3) as u32,
            furthest: crate::profile::unlocked(GAME),
            t: 0.0,
            dead_t: 0.0,
            scroll: 0.0,
            kick: 0.0,
            flash: 0.0,
            fx: Particles::default(),
            shake: Shake::default(),
            popups: Popups::default(),
            bump: Bump::default(),
        }
    }

    fn bird_x(view: &View) -> f32 {
        view.w * 0.3
    }

    fn reset(&mut self, view: &View) {
        let best = self.best;
        *self = Self::new();
        self.best = best;
        self.speed = tuning(0, self.current_help()).speed;
        self.y = view.h * 0.42;
    }

    pub fn current_help(&self) -> u32 {
        if self.training {
            3
        } else {
            self.help
        }
    }

    pub fn current_tuning(&self) -> Tuning {
        tuning(self.score, self.current_help())
    }

    pub fn pipe_rects(p: &Pipe, view: &View) -> [Rect; 2] {
        let top = p.gap_y - p.gap / 2.0;
        let bottom = p.gap_y + p.gap / 2.0;
        let roof = -view.inset.top - 40.0;
        [
            Rect::new(p.x, roof, PIPE_W, top - roof),
            Rect::new(p.x, bottom, PIPE_W, view.h - bottom),
        ]
    }

    fn spawn_pipe(&mut self, x: f32, view: &View) {
        let tune = self.current_tuning();
        let lo = tune.gap / 2.0 + 40.0 + tune.amp;
        let hi = (view.h - GROUND - tune.gap / 2.0 - 40.0 - tune.amp).max(lo + 1.0);
        let mid = (lo + hi) / 2.0;
        let reach = tune.jitter.min((hi - lo) / 2.0);
        let base_y = mid + rand::gen_range(-reach, reach);
        let mut pipe = Pipe::new(x, base_y);
        pipe.gap = tune.gap;
        pipe.amp = tune.amp;
        pipe.phase = rand::gen_range(0.0, std::f32::consts::TAU);
        pipe.sway(self.t);
        self.pipes.push(pipe);
    }

    fn milestone(&mut self, view: &View) {
        let stage = stage_of(self.score);
        crate::profile::unlock(GAME, stage);
        self.furthest = self.furthest.max(stage);
        sfx::play(Sfx::LevelUp);
        sfx::say(if stage % 2 == 1 {
            sfx::Say::Great
        } else {
            sfx::Say::Congrats
        });
        feel::impact_tinted(Impact::Large, Color::from_rgba(255, 210, 40, 255));
        self.fx.confetti(view);
        let what = match stage {
            MOVING_FROM_STAGE => "Moving pipes!".to_string(),
            _ => stage_name(stage),
        };
        self.popups.add(
            vec2(view.w / 2.0, view.h * 0.3),
            what,
            Color::from_rgba(255, 230, 90, 255),
            64.0,
        );
    }

    fn record_run(&mut self) {
        self.rank = self.best.submit(GAME, self.score);
        let help = if self.score < 5 {
            (self.help + 1).min(3)
        } else if self.score >= MILESTONE {
            0
        } else {
            self.help.saturating_sub(1)
        };
        self.help = help;
        crate::profile::set(GAME, "struggle", help as i64);
        crate::profile::unlock(GAME, stage_of(self.score));
    }

    fn flap(&mut self, view: &View) {
        sfx::play(Sfx::Flap);
        self.vy = FLAP;
        self.kick = 1.0;
        let at = vec2(Self::bird_x(view) - BIRD_R * 0.8, self.y + BIRD_R * 0.3);
        self.fx.emit(
            at,
            &[WHITE, Color::from_rgba(255, 245, 200, 255)],
            Burst {
                count: 5,
                speed: (40.0, 120.0),
                size: (3.0, 6.0),
                life: (0.3, 0.55),
                gravity: 160.0,
                drag: 2.5,
                spread: 1.2,
                angle: std::f32::consts::PI * 0.8,
                ..Default::default()
            },
        );
    }

    fn die(&mut self, view: &View) {
        sfx::play(Sfx::Hit);
        sfx::play(Sfx::Lose);
        if self.score > self.best.0 && self.best.0 > 0 && !self.training {
            sfx::say(sfx::Say::Best);
        }
        feel::impact(Impact::Large);
        self.state = State::Dead;
        self.dead_t = 0.0;
        self.record_run();
        self.shake.add(0.9);
        self.flash = 1.0;
        self.fx.emit(
            vec2(Self::bird_x(view), self.y),
            &[
                Color::from_rgba(255, 210, 40, 255),
                WHITE,
                Color::from_rgba(255, 120, 40, 255),
            ],
            Burst {
                count: 22,
                speed: (120.0, 420.0),
                size: (4.0, 9.0),
                life: (0.5, 0.9),
                gravity: 700.0,
                shape: Shape::Star,
                ..Default::default()
            },
        );
    }
}

impl Default for Flappy {
    fn default() -> Self {
        Self::new()
    }
}

impl Game for Flappy {
    fn update(&mut self, input: &Input, view: &View, dt: f32) {
        self.t += dt;
        self.kick = approach(self.kick, 0.0, 9.0, dt);
        self.flash = (self.flash - dt * 2.5).max(0.0);
        self.fx.update(dt);
        self.shake.update(dt);
        self.popups.update(dt);
        self.bump.update(dt);
        let ground_y = view.h - GROUND;
        match self.state {
            State::Ready => {
                self.y = view.h * 0.42 + (self.t * 3.0).sin() * 10.0;
                self.scroll += self.speed * dt;
                if input.tap() {
                    self.state = State::Playing;
                    sfx::say(sfx::Say::Go);
                    self.flap(view);
                }
            }
            State::Playing => {
                if input.tap() {
                    self.flap(view);
                }
                self.vy += GRAVITY * dt;
                self.y += self.vy * dt;
                if self.y < BIRD_R {
                    self.y = BIRD_R;
                    self.vy = 0.0;
                }
                let tune = self.current_tuning();
                self.speed = approach(self.speed, tune.speed, 1.5, dt);
                self.scroll += self.speed * dt;
                let t = self.t;
                for p in &mut self.pipes {
                    p.x -= self.speed * dt;
                    p.sway(t);
                }
                self.pipes.retain(|p| p.x > -PIPE_W - 10.0);
                let last_x = self.pipes.last().map(|p| p.x);
                match last_x {
                    None => self.spawn_pipe(view.w + 40.0, view),
                    Some(x) if x < view.w - tune.spacing => self.spawn_pipe(x + tune.spacing, view),
                    _ => {}
                }
                if self.training && self.y + BIRD_R >= ground_y {
                    self.y = ground_y - BIRD_R;
                    self.vy = FLAP * 0.85;
                    self.kick = 1.0;
                    sfx::play(Sfx::Bounce);
                    self.fx.puff(vec2(Self::bird_x(view), ground_y), WHITE);
                }
                let bird = vec2(Self::bird_x(view), self.y);
                let mut hit = !self.training && self.y + BIRD_R >= ground_y;
                let mut scored_at = None;
                let mut cushioned = None;
                for p in &mut self.pipes {
                    if Self::pipe_rects(p, view)
                        .iter()
                        .any(|r| circle_hits_rect(bird, BIRD_R - 5.0, *r))
                    {
                        if self.training {
                            cushioned = Some(p.gap_y);
                        } else {
                            hit = true;
                        }
                    }
                    if !p.scored && p.x + PIPE_W < bird.x {
                        p.scored = true;
                        self.score += 1;
                        self.bump.kick();
                        scored_at = Some(vec2(p.x + PIPE_W / 2.0, p.gap_y));
                    }
                }
                if let Some(gap_y) = cushioned {
                    self.y = approach(self.y, gap_y, 10.0, dt);
                    self.vy = 0.0;
                }
                if let Some(at) = scored_at {
                    if self.training && self.score >= TRAINING_PIPES {
                        self.training = false;
                        crate::profile::set(GAME, "passed", 1);
                    }
                    sfx::play_step(Sfx::Score, self.score as usize % 8);
                    self.fx.sparkle(at, Color::from_rgba(255, 210, 40, 255));
                    self.popups.add(
                        vec2(bird.x + 50.0, self.y - 40.0),
                        "+1",
                        Color::from_rgba(255, 230, 90, 255),
                        44.0,
                    );
                    if self.score.is_multiple_of(MILESTONE) {
                        self.milestone(view);
                    }
                }
                if hit {
                    self.die(view);
                }
            }
            State::Dead => {
                self.dead_t += dt;
                if self.y + BIRD_R < ground_y {
                    self.vy += GRAVITY * dt;
                    self.y = (self.y + self.vy * dt).min(ground_y - BIRD_R);
                }
                if self.dead_t > 0.6 && input.tap() {
                    self.reset(view);
                }
            }
        }
    }

    fn draw(&self, view: &View) {
        set_view_camera(view, self.shake.offset());
        self.draw_world(view);
        self.fx.draw();
        self.popups.draw();
        set_view_camera(view, Vec2::ZERO);
        if self.flash > 0.0 {
            fill_screen(view, with_alpha(WHITE, self.flash * 0.7));
        }
        if self.state == State::Playing {
            big_score(view, &self.score.to_string(), 80.0, self.bump.t);
            self.draw_goal(view);
            if self.training {
                let pulse = 1.0 + (self.t * 5.0).sin() * 0.05;
                crate::ui::hint(
                    view,
                    "Tap to fly up, fly through the gap",
                    vec2(view.w / 2.0, view.h * 0.8),
                    pulse,
                );
            }
        }
        chip(view, Side::Right, "BEST", &self.best.0.to_string(), 1.0);
        match self.state {
            State::Ready => {
                let bob = (self.t * 4.0).sin() * 6.0;
                crate::ui::ribbon(
                    view,
                    "Get Ready!",
                    vec2(view.w / 2.0, view.h * 0.2 + bob),
                    self.t,
                );
                let pulse = 1.0 + (self.t * 5.0).sin() * 0.05;
                crate::ui::hint(
                    view,
                    "Tap to flap",
                    vec2(view.w / 2.0, view.h * 0.74),
                    pulse,
                );
                let goal = format!(
                    "Goal: reach {}   Furthest: {}",
                    MILESTONE,
                    self.furthest * MILESTONE
                );
                text_shadow(&goal, view.w / 2.0, view.h * 0.84, 32.0, WHITE);
            }
            State::Dead if self.dead_t > 0.4 => {
                let line = self.result_line();
                results(
                    view,
                    &crate::ui::Results {
                        game: GAME,
                        title: "Bonk!",
                        line: &line,
                        rank: self.rank,
                        action: "Fly again",
                        format: crate::ui::plain_score,
                    },
                );
            }
            _ => {}
        }
    }
}

mod art {
    crate::sprites!("flappy");
}

pub fn tile_x(view_x: f32, view_w: f32, tile_w: f32, scroll: f32) -> Vec<f32> {
    let off = scroll.rem_euclid(tile_w);
    let mut out = vec![];
    let mut x = view_x - off;
    while x < view_x + view_w {
        out.push(x);
        x += tile_w;
    }
    out
}

fn band(name: &str, view: &View, bottom: f32, h: f32, scroll: f32, tint: Color) {
    let f = view.full();
    let s = art::size(name);
    let w = h * s.x / s.y;
    for x in tile_x(f.x, f.w, w, scroll) {
        art::sprite_ex(
            name,
            Rect::new(x, bottom - h, w + 1.0, h),
            crate::art::Draw {
                tint,
                ..Default::default()
            },
        );
    }
}

impl Flappy {
    pub fn result_line(&self) -> String {
        let stage = stage_of(self.score);
        if self.score >= MILESTONE {
            format!("Score {}   {}", self.score, stage_name(stage))
        } else {
            format!(
                "Score {}   {} more to reach {}",
                self.score,
                MILESTONE - self.score,
                MILESTONE
            )
        }
    }

    fn draw_goal(&self, view: &View) {
        let goal = next_goal(self.score);
        let from = goal - MILESTONE;
        let frac = (self.score - from) as f32 / MILESTONE as f32;
        let w = 300.0f32.min(view.w * 0.5);
        let r = Rect::new(view.w / 2.0 - w / 2.0, 118.0, w, 22.0);
        round_rect(r, 11.0, Color::new(0.1, 0.05, 0.2, 0.45));
        if frac > 0.0 {
            let fill = Rect::new(r.x + 2.0, r.y + 2.0, (r.w - 4.0) * frac, r.h - 4.0);
            round_rect(fill, fill.h / 2.0, Color::from_rgba(255, 200, 40, 255));
        }
        let label = format!("Next: {}", stage_name(stage_of(self.score) + 1));
        text_shadow(&label, view.w / 2.0, r.y + 46.0, 26.0, WHITE);
    }

    fn draw_world(&self, view: &View) {
        let f = view.full();
        let gy = view.h - GROUND;
        let tint = stage_tint(stage_of(self.score));
        band("sky", view, f.y + f.h, f.h, self.scroll * 0.08, tint);
        band(
            "hills",
            view,
            gy + 20.0,
            (f.h * 0.42).max(260.0),
            self.scroll * 0.35,
            tint,
        );
        for p in &self.pipes {
            Self::draw_pipe(p, view);
        }
        let gs = art::size("ground");
        let gh = GROUND + 34.0 + view.inset.bottom + 40.0;
        let top_frac = 116.0 / gs.y;
        let h = (GROUND + 34.0) / (1.0 - top_frac);
        let w = h * gs.x / gs.y;
        for x in tile_x(f.x, f.w, w, self.scroll) {
            art::sprite(
                "ground",
                Rect::new(x, gy - 34.0 - h * top_frac, w + 1.0, h.max(gh)),
            );
        }
        self.draw_bird(view);
    }

    fn draw_pipe(p: &Pipe, view: &View) {
        let [top, bottom] = Self::pipe_rects(p, view);
        let cap = art::size("pipe_cap");
        let body = art::size("pipe_body");
        let cw = PIPE_W * 1.24;
        let ch = cw * cap.y / cap.x;
        let bw = PIPE_W * 1.02;
        let _ = body;
        let seg = bw * body.y / body.x;
        let cx = p.x + PIPE_W / 2.0;
        let _ = seg;
        let column = |from: f32, to: f32| {
            if to > from {
                art::sprite("pipe_body", Rect::new(cx - bw / 2.0, from, bw, to - from));
            }
        };
        column(top.y, top.y + top.h - ch * 0.6);
        art::sprite_ex(
            "pipe_cap",
            Rect::new(cx - cw / 2.0, top.y + top.h - ch, cw, ch),
            crate::art::Draw {
                flip_y: true,
                ..Default::default()
            },
        );
        column(bottom.y + ch * 0.6, bottom.y + bottom.h);
        art::sprite("pipe_cap", Rect::new(cx - cw / 2.0, bottom.y, cw, ch));
    }

    fn bird_frame(&self) -> &'static str {
        if self.state == State::Dead {
            return "bird_hurt";
        }
        if self.kick > 0.55 {
            "bird_down"
        } else if self.kick > 0.2 || self.state == State::Ready && (self.t * 6.0).sin() > 0.0 {
            "bird_mid"
        } else {
            "bird_up"
        }
    }

    fn draw_bird(&self, view: &View) {
        let bx = Self::bird_x(view);
        let angle = (self.vy / 900.0).clamp(-0.45, 1.1);
        let sc = bird_scale(self.vy, self.kick);
        let ground_y = view.h - GROUND;
        let height = (ground_y - self.y).max(0.0);
        soft_shadow(
            vec2(bx, ground_y + 6.0),
            BIRD_R * (1.4 - (height / 600.0).min(0.7)),
            5.0,
            0.5,
        );
        let frame = self.bird_frame();
        let h = BIRD_R * 3.4;
        let w = h * art::size(frame).x / art::size(frame).y;
        art::sprite_ex(
            frame,
            Rect::new(
                bx - w * sc.x / 2.0,
                self.y - h * sc.y / 2.0,
                w * sc.x,
                h * sc.y,
            ),
            crate::art::Draw {
                rotation: angle,
                ..Default::default()
            },
        );
    }
}

pub fn icon(r: Rect) {
    crate::ui::icon("flappy", r);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flappy_sprites_all_exist() {
        let have = art::names();
        for n in [
            "sky",
            "hills",
            "ground",
            "pipe_cap",
            "pipe_body",
            "bird_up",
            "bird_mid",
            "bird_down",
            "bird_hurt",
        ] {
            assert!(have.iter().any(|h| h == n), "missing {n}");
        }
    }

    #[test]
    fn flapping_and_crashing_make_sounds() {
        let v = View::ipad_landscape();
        let mut g = trained();
        crate::sfx::take_pending();
        g.update(&Input::press_at(vec2(500.0, 500.0)), &v, 1.0 / 60.0);
        assert!(crate::sfx::take_pending().contains(&Sfx::Flap));
        for _ in 0..240 {
            g.update(&Input::default(), &v, 1.0 / 60.0);
        }
        assert!(crate::sfx::take_pending().contains(&Sfx::Hit));
    }

    fn trained() -> Flappy {
        crate::save::reset_for_test();
        crate::profile::create("Mia", 0);
        crate::profile::set(GAME, "passed", 1);
        Flappy::new()
    }

    fn step(g: &mut Flappy, input: &Input, n: usize) {
        let v = View::ipad_landscape();
        for _ in 0..n {
            g.update(input, &v, 1.0 / 60.0);
        }
    }

    #[test]
    fn bird_without_flaps_hits_ground() {
        let mut g = trained();
        step(&mut g, &Input::press_at(vec2(500.0, 500.0)), 1);
        assert_eq!(g.state, State::Playing);
        step(&mut g, &Input::default(), 240);
        assert_eq!(g.state, State::Dead);
    }

    #[test]
    fn passing_a_pipe_scores() {
        let v = View::ipad_landscape();
        let mut g = trained();
        g.state = State::Playing;
        g.y = 300.0;
        g.pipes
            .push(Pipe::new(Flappy::bird_x(&v) - PIPE_W + 1.0, 300.0));
        g.update(&Input::press_at(Vec2::ZERO), &v, 1.0 / 60.0);
        assert_eq!(g.score, 1);
        assert_eq!(g.state, State::Playing);
    }

    #[test]
    fn flying_into_a_pipe_kills() {
        let v = View::ipad_landscape();
        let mut g = trained();
        g.state = State::Playing;
        g.y = 100.0;
        g.pipes.push(Pipe::new(Flappy::bird_x(&v) - 10.0, 450.0));
        g.update(&Input::press_at(Vec2::ZERO), &v, 1.0 / 60.0);
        assert_eq!(g.state, State::Dead);
    }

    #[test]
    fn ground_and_sky_tiles_cover_the_screen_at_any_scroll() {
        for scroll in [0.0, 13.0, 777.7, 123456.0] {
            let xs = tile_x(-40.0, 1100.0, 300.0, scroll);
            assert!(xs[0] <= -40.0);
            assert!(xs.last().unwrap() + 300.0 >= 1060.0);
            assert!(xs.windows(2).all(|w| (w[1] - w[0] - 300.0).abs() < 1e-3));
        }
    }

    #[test]
    fn pipe_columns_reach_past_the_top_edge_and_down_to_the_ground() {
        let v = View::fit_safe(
            2400.0,
            1080.0,
            crate::safe::Insets {
                top: 0.0,
                left: 100.0,
                ..Default::default()
            },
        );
        let p = Pipe::new(300.0, 300.0);
        let [top, bottom] = Flappy::pipe_rects(&p, &v);
        assert!(top.y < -v.inset.top);
        assert_eq!(top.y + top.h, 300.0 - 230.0 / 2.0);
        assert_eq!(bottom.y, 300.0 + 230.0 / 2.0);
        assert!(bottom.y + bottom.h >= v.h - GROUND);
    }

    #[test]
    fn the_bird_flaps_its_wings_down_after_a_tap() {
        let v = View::ipad_landscape();
        let mut g = trained();
        g.reset(&v);
        g.state = State::Playing;
        g.kick = 0.0;
        g.vy = 200.0;
        assert_eq!(g.bird_frame(), "bird_up");
        g.flap(&v);
        assert_eq!(g.bird_frame(), "bird_down");
        g.state = State::Dead;
        assert_eq!(g.bird_frame(), "bird_hurt");
    }

    #[test]
    fn flapping_puffs_feathers_and_squashes() {
        let mut g = trained();
        step(&mut g, &Input::press_at(vec2(500.0, 500.0)), 1);
        assert!(!g.fx.list.is_empty());
        assert!(g.kick > 0.5);
        step(&mut g, &Input::default(), 120);
        assert!(g.kick < 0.01);
    }

    #[test]
    fn scoring_bumps_and_sparkles() {
        let v = View::ipad_landscape();
        let mut g = trained();
        g.state = State::Playing;
        g.y = 300.0;
        g.pipes
            .push(Pipe::new(Flappy::bird_x(&v) - PIPE_W + 1.0, 300.0));
        g.update(&Input::default(), &v, 1.0 / 60.0);
        assert_eq!(g.score, 1);
        assert!(g.bump.t < 1.0);
        assert!(g.fx.list.iter().any(|p| p.shape == Shape::Star));
        assert_eq!(g.popups.list.len(), 1);
    }

    #[test]
    fn hitting_a_pipe_shakes_and_flashes() {
        let v = View::ipad_landscape();
        let mut g = trained();
        g.state = State::Playing;
        g.y = 100.0;
        g.pipes.push(Pipe::new(Flappy::bird_x(&v) - 10.0, 450.0));
        g.update(&Input::default(), &v, 1.0 / 60.0);
        assert_eq!(g.state, State::Dead);
        assert!(g.shake.active());
        assert!(g.flash > 0.9);
        step(&mut g, &Input::default(), 120);
        assert!(!g.shake.active());
        assert_eq!(g.flash, 0.0);
    }

    #[test]
    fn restart_clears_effects() {
        let mut g = trained();
        step(&mut g, &Input::press_at(vec2(500.0, 500.0)), 1);
        step(&mut g, &Input::default(), 240);
        assert_eq!(g.state, State::Dead);
        step(&mut g, &Input::press_at(vec2(500.0, 500.0)), 1);
        assert_eq!(g.state, State::Ready);
        assert!(g.fx.list.is_empty());
        assert!(!g.shake.active());
    }

    #[test]
    fn bird_scale_preserves_volume_and_stretches_when_fast() {
        let rest = bird_scale(0.0, 0.0);
        assert_eq!(rest, vec2(1.0, 1.0));
        let fast = bird_scale(-520.0, 1.0);
        assert!(fast.x > 1.1 && fast.y < 0.9);
        assert!((fast.x * fast.y - 1.0).abs() < 1e-4);
    }

    #[test]
    fn parallax_layers_wrap_inside_their_span() {
        for scroll in [0.0, 137.0, 99999.0] {
            for i in 0..8 {
                let x = layer_x(i, 220.0, scroll, 0.4, 1400.0, 220.0);
                assert!((-220.0..1180.0).contains(&x));
            }
        }
        let slow = layer_x(1, 220.0, 100.0, 0.1, 1400.0, 220.0);
        let fast = layer_x(1, 220.0, 100.0, 0.7, 1400.0, 220.0);
        assert!(slow > fast);
    }

    fn fresh_child() -> Flappy {
        crate::save::reset_for_test();
        crate::profile::create("Mia", 0);
        Flappy::new()
    }

    fn bot(g: &Flappy, view: &View) -> bool {
        let bx = Flappy::bird_x(view);
        let target = g
            .pipes
            .iter()
            .find(|p| p.x + PIPE_W > bx - BIRD_R)
            .map(|p| p.gap_y + p.gap * 0.15)
            .unwrap_or(view.h * 0.5);
        g.y > target && g.vy > 0.0
    }

    fn play_bot(g: &mut Flappy, seconds: f32) {
        let v = View::ipad_landscape();
        g.update(&Input::press_at(vec2(500.0, 500.0)), &v, 1.0 / 60.0);
        for _ in 0..(seconds * 60.0) as usize {
            let input = if bot(g, &v) {
                Input::press_at(vec2(500.0, 500.0))
            } else {
                Input::default()
            };
            g.update(&input, &v, 1.0 / 60.0);
            if g.state == State::Dead {
                break;
            }
        }
    }

    #[test]
    fn a_new_child_cannot_lose_during_the_first_pipes() {
        let mut g = fresh_child();
        assert!(g.training);
        step(&mut g, &Input::press_at(vec2(500.0, 500.0)), 1);
        step(&mut g, &Input::default(), 60 * 5);
        assert_eq!(g.state, State::Playing);
        assert!(g.training);
        assert!(g.y < View::ipad_landscape().h);
    }

    #[test]
    fn the_first_two_pipes_are_wide_and_slow() {
        let g = fresh_child();
        let easy = tuning(0, g.current_help());
        let normal = tuning(0, 0);
        assert!(easy.gap > normal.gap + 50.0);
        assert!(easy.speed < normal.speed);
        assert!(easy.gap >= 380.0);
    }

    #[test]
    fn the_training_hint_ends_after_two_pipes_and_is_remembered() {
        let v = View::ipad_landscape();
        let mut g = fresh_child();
        g.state = State::Playing;
        g.y = 300.0;
        for score in 0..TRAINING_PIPES {
            g.score = score;
            g.pipes
                .push(Pipe::new(Flappy::bird_x(&v) - PIPE_W + 1.0, 300.0));
            g.update(&Input::default(), &v, 1.0 / 60.0);
            g.y = 300.0;
            g.vy = 0.0;
        }
        assert_eq!(g.score, TRAINING_PIPES);
        assert!(!g.training);
        assert!(!Flappy::new().training);
    }

    #[test]
    fn a_player_who_flies_the_gaps_reaches_ten_on_the_first_try() {
        let mut g = fresh_child();
        play_bot(&mut g, 90.0);
        assert!(g.score >= MILESTONE, "scored {}", g.score);
    }

    #[test]
    fn difficulty_climbs_with_score() {
        let a = tuning(0, 0);
        let b = tuning(15, 0);
        let c = tuning(40, 0);
        assert!(a.speed < b.speed && b.speed < c.speed);
        assert!(a.gap > b.gap && b.gap > c.gap);
        assert!(a.spacing > c.spacing);
        assert!(a.jitter < c.jitter);
        assert!(c.speed <= 230.0 && c.gap >= 220.0);
    }

    #[test]
    fn pipes_start_moving_from_the_third_stage() {
        assert_eq!(tuning(MILESTONE * MOVING_FROM_STAGE - 1, 0).amp, 0.0);
        assert!(tuning(MILESTONE * MOVING_FROM_STAGE, 0).amp > 0.0);
        assert!(tuning(200, 0).amp <= 60.0);
    }

    #[test]
    fn moving_pipes_sway_around_their_base() {
        let mut p = Pipe::new(500.0, 300.0);
        p.amp = 30.0;
        let mut lo: f32 = 1000.0;
        let mut hi: f32 = 0.0;
        for i in 0..200 {
            p.sway(i as f32 * 0.05);
            lo = lo.min(p.gap_y);
            hi = hi.max(p.gap_y);
        }
        assert!(lo >= 270.0 - 0.01 && hi <= 330.0 + 0.01);
        assert!(hi - lo > 50.0);
    }

    #[test]
    fn spawned_gaps_stay_inside_the_playfield() {
        let v = View::ipad_landscape();
        for score in [0, 10, 25, 60] {
            let mut g = trained();
            g.score = score;
            for _ in 0..50 {
                g.pipes.clear();
                g.spawn_pipe(900.0, &v);
                let p = &g.pipes[0];
                let amp = p.amp;
                assert!(p.base_y - amp - p.gap / 2.0 >= 30.0);
                assert!(p.base_y + amp + p.gap / 2.0 <= v.h - GROUND - 30.0);
            }
        }
    }

    #[test]
    fn crossing_ten_celebrates_and_saves_the_furthest_stage() {
        let v = View::ipad_landscape();
        let mut g = trained();
        g.state = State::Playing;
        g.y = 300.0;
        g.score = MILESTONE - 1;
        g.pipes
            .push(Pipe::new(Flappy::bird_x(&v) - PIPE_W + 1.0, 300.0));
        crate::sfx::take_pending();
        g.update(&Input::default(), &v, 1.0 / 60.0);
        assert_eq!(g.score, MILESTONE);
        assert!(crate::sfx::take_pending().contains(&Sfx::LevelUp));
        assert!(g.fx.list.iter().any(|p| p.shape == Shape::Confetti));
        assert_eq!(crate::profile::unlocked(GAME), 1);
        assert_eq!(Flappy::new().furthest, 1);
    }

    #[test]
    fn scenery_changes_each_stage() {
        assert_eq!(stage_of(9), 0);
        assert_eq!(stage_of(10), 1);
        assert_ne!(stage_tint(0), stage_tint(1));
        assert_eq!(next_goal(0), 10);
        assert_eq!(next_goal(10), 20);
        assert_eq!(stage_name(0), "Sunny Day");
        assert_eq!(stage_name(7), "Level 8");
    }

    #[test]
    fn a_bad_run_gives_the_next_one_wider_gaps() {
        let mut g = trained();
        g.state = State::Playing;
        g.y = 100.0;
        g.pipes.push(Pipe::new(
            Flappy::bird_x(&View::ipad_landscape()) - 10.0,
            450.0,
        ));
        g.update(&Input::default(), &View::ipad_landscape(), 1.0 / 60.0);
        assert_eq!(g.state, State::Dead);
        assert_eq!(g.help, 1);
        let next = Flappy::new();
        assert_eq!(next.help, 1);
        assert!(tuning(0, 1).gap > tuning(0, 0).gap);
        assert!(tuning(0, 1).speed < tuning(0, 0).speed);
    }

    #[test]
    fn help_fades_as_the_score_climbs_and_a_good_run_clears_it() {
        assert_eq!(tuning(30, 3).gap, tuning(30, 0).gap);
        let mut g = trained();
        g.help = 2;
        g.score = 12;
        g.record_run();
        assert_eq!(g.help, 0);
        assert_eq!(Flappy::new().help, 0);
        g.score = 7;
        g.help = 2;
        g.record_run();
        assert_eq!(g.help, 1);
    }

    #[test]
    fn restarting_never_starts_at_full_speed() {
        let v = View::ipad_landscape();
        let mut g = trained();
        g.speed = 230.0;
        g.score = 35;
        g.state = State::Dead;
        g.dead_t = 1.0;
        step(&mut g, &Input::press_at(vec2(500.0, 500.0)), 1);
        assert_eq!(g.state, State::Ready);
        assert_eq!(g.score, 0);
        assert!(g.speed <= tuning(0, 0).speed + 1.0);
        let _ = v;
    }

    #[test]
    fn game_over_ranks_the_run_and_remembers_the_best_per_player() {
        crate::save::reset_for_test();
        let mia = crate::profile::create("Mia", 0).unwrap();
        crate::profile::set(GAME, "passed", 1);
        let mut g = Flappy::new();
        g.score = 8;
        g.die(&View::ipad_landscape());
        assert_eq!(g.rank, Some(0));
        let leo = crate::profile::create("Leo", 1).unwrap();
        assert_eq!(Flappy::new().best.0, 0);
        assert!(Flappy::new().training);
        crate::profile::select(&mia.id);
        assert_eq!(Flappy::new().best.0, 8);
        assert!(!Flappy::new().training);
        let _ = leo;
    }

    #[test]
    fn the_result_line_says_how_far_off_the_goal_was() {
        let mut g = trained();
        g.score = 6;
        assert_eq!(g.result_line(), "Score 6   4 more to reach 10");
        g.score = 13;
        assert_eq!(g.result_line(), "Score 13   Sunset");
    }
}
