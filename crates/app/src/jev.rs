use crate::games::clash::ClashPlay;
use crate::games::flappy::FlappyPlay;
use crate::games::Play;
use crate::paint::Pen;
use kg_core::engine::Input;
use kg_core::engine::View;
use kg_core::games::clash::{Mode, Screen};
use kg_core::geom::{Color, Rect, WHITE};
use kg_core::jev::clash::ClashPlayer;
use kg_core::jev::{mode_from, yalr_key, Player};

pub struct Jev {
    pub player: Option<Player>,
    pub game: FlappyPlay,
}

pub fn requested_brain() -> bool {
    crate::web::query("brain").is_some_and(|g| g == "paper")
}

pub fn requested_clash() -> bool {
    crate::web::query("jev").is_some_and(|g| g == "clash")
}

pub fn requested() -> bool {
    crate::web::query("jev").is_some_and(|g| g == "flappy")
}

impl Jev {
    pub fn new() -> Self {
        kg_core::save::use_memory_only();
        if kg_core::profile::current().is_none() {
            kg_core::profile::create("Jev", 4);
        }
        kg_core::profile::set(kg_core::games::flappy::GAME, "passed", 1);
        let player = crate::web::query("key")
            .filter(|k| !k.is_empty())
            .or_else(yalr_key)
            .map(|k| {
                let mut p = Player::new(k);
                p.mode = mode_from(crate::web::query("jev-mode").as_deref());
                p
            });
        Self {
            player,
            game: FlappyPlay::new(),
        }
    }

    pub fn frame(&mut self, pen: &mut Pen, view: &View, dt: f32) {
        if !crate::art::ready("flappy") {
            pen.fill_screen(view, Color::new(0.06, 0.03, 0.15, 1.0));
            return;
        }
        let Some(player) = &mut self.player else {
            pen.fill_screen(view, Color::new(0.06, 0.03, 0.15, 1.0));
            pen.text_center(
                "No Yalr key: set YALR_KEY or ?key=",
                view.w / 2.0,
                view.h / 2.0,
                36.0,
                WHITE,
            );
            return;
        };
        let dt = dt * player.clock();
        let input = player.drive(&self.game.g, view, dt);
        self.game.update(&input, view, dt);
        self.game.draw(pen, view);
        pen.offset = glam::Vec2::ZERO;
        let lines = player.lines(self.game.g.score);
        let w = 380.0f32.min(view.w * 0.45);
        let r = Rect::new(view.w - w - 16.0, 70.0, w, 34.0 + lines.len() as f32 * 26.0);
        pen.round_rect(r, 18.0, Color::new(0.06, 0.03, 0.15, 0.78));
        let gold = Color::from_rgba(255, 210, 70, 255);
        let dim = Color::new(0.75, 0.78, 0.9, 1.0);
        for (i, (l, strong)) in lines.iter().enumerate() {
            let shown = fit(l, w - 32.0, 22.0);
            pen.text_left(
                &shown,
                r.x + 16.0,
                r.y + 27.0 + i as f32 * 26.0,
                22.0,
                if *strong { gold } else { dim },
            );
        }
    }
}

pub fn fit(s: &str, width: f32, size: f32) -> String {
    if crate::draw::measure(s, size) <= width {
        return s.to_string();
    }
    let mut out: String = s.to_string();
    while !out.is_empty() && crate::draw::measure(&format!("{out}..."), size) > width {
        out.pop();
    }
    format!("{}...", out.trim_end())
}

pub struct JevClash {
    pub player: Option<ClashPlayer>,
    pub game: ClashPlay,
    was_playing: bool,
}

impl JevClash {
    pub fn new() -> Self {
        kg_core::save::use_memory_only();
        if kg_core::profile::current().is_none() {
            kg_core::profile::create("Jev", 4);
        }
        let player = crate::web::query("key")
            .filter(|k| !k.is_empty())
            .or_else(yalr_key)
            .map(ClashPlayer::new);
        let mut game = ClashPlay::new();
        game.g.start(Mode::Arena, 1);
        Self {
            player,
            game,
            was_playing: true,
        }
    }

    pub fn frame(&mut self, pen: &mut Pen, view: &View, dt: f32) {
        let Some(player) = &mut self.player else {
            pen.fill_screen(view, Color::new(0.06, 0.03, 0.15, 1.0));
            pen.text_center(
                "No Yalr key: set YALR_KEY or ?key=",
                view.w / 2.0,
                view.h / 2.0,
                36.0,
                WHITE,
            );
            return;
        };
        let g = &mut self.game.g;
        let playing = g.screen == Screen::Playing;
        if playing && g.ending == 0.0 {
            if let Some(a) = player.drive(&g.world) {
                g.steer = a;
            }
        }
        if self.was_playing && !playing {
            player.end_run(g.world.player().level());
        }
        self.was_playing = playing;
        if g.screen == Screen::Over && self.game.results_age > 2.5 {
            self.game.g.start(Mode::Arena, 1);
        }
        let held = Input {
            mm: 6.0,
            ..Default::default()
        };
        self.game.update(&held, view, dt);
        self.game.draw(pen, view);
        pen.offset = glam::Vec2::ZERO;
        let level = self.game.g.world.player().level();
        let mut lines = vec![
            ("JEV 1.13 playing Snake Clash".to_string(), true),
            (
                format!("calls {}   avg {:.0} ms", player.calls, player.avg_ms()),
                false,
            ),
            (
                format!(
                    "run {}   level {}   best {}",
                    player.runs + 1,
                    level,
                    player.best.max(level)
                ),
                false,
            ),
        ];
        if !player.last_seen.is_empty() {
            lines.push((format!("saw: {}", player.last_seen), false));
        }
        lines.extend(player.log.iter().map(|l| (l.clone(), false)));
        let w = 420.0f32.min(view.w * 0.45);
        let r = Rect::new(
            16.0,
            view.h - 30.0 - lines.len() as f32 * 26.0 - 16.0,
            w,
            30.0 + lines.len() as f32 * 26.0,
        );
        pen.round_rect(r, 18.0, Color::new(0.06, 0.03, 0.15, 0.78));
        let gold = Color::from_rgba(255, 210, 70, 255);
        let dim = Color::new(0.75, 0.78, 0.9, 1.0);
        for (i, (l, strong)) in lines.iter().enumerate() {
            pen.text_left(
                &fit(l, w - 32.0, 22.0),
                r.x + 16.0,
                r.y + 27.0 + i as f32 * 26.0,
                22.0,
                if *strong { gold } else { dim },
            );
        }
    }
}

pub const MAX_SPEED: f32 = 32.0;

pub struct BrainPaper {
    pub brain: Option<kg_core::brain::Brain>,
    pub rl: bool,
    pub game: crate::games::paper::PaperPlay,
    pub effort: f32,
    pub speed: f32,
    since: f32,
    dead_for: f32,
    runs: u32,
    best: f32,
    was_dead: bool,
}

impl BrainPaper {
    pub fn new() -> Self {
        kg_core::save::use_memory_only();
        if kg_core::profile::current().is_none() {
            kg_core::profile::create("Brain", 5);
        }
        let effort = crate::web::query("effort")
            .and_then(|e| e.parse().ok())
            .unwrap_or(1.0);
        let brain = kg_core::games::paper::world::shipped_net()
            .map(|n| kg_core::brain::Brain::new(n.clone(), kg_core::brain::Effort::new(effort)));
        let rl = crate::web::query("model").is_some_and(|m| m == "rl")
            && kg_core::games::paper::rl::shipped().is_some();
        Self {
            brain,
            rl,
            game: crate::games::paper::PaperPlay::new(),
            effort,
            speed: crate::web::query("speed")
                .and_then(|v| v.parse::<f32>().ok())
                .unwrap_or(1.0)
                .clamp(1.0, MAX_SPEED),
            since: 0.0,
            dead_for: 0.0,
            runs: 0,
            best: 0.0,
            was_dead: false,
        }
    }

    fn step(&mut self, view: &View, dt: f32) {
        use kg_core::games::paper::State;
        let g = &mut self.game.g;
        let mut input = Input::default();
        match g.state {
            State::Ready => input.action = true,
            State::Dead => {
                if !self.was_dead {
                    self.runs += 1;
                    self.dead_for = 0.0;
                }
                self.dead_for += dt;
                if self.dead_for > 2.0 {
                    input =
                        Input::press_at(kg_core::games::paper::play_again_button(view).center());
                }
            }
            State::Playing => {
                self.since += dt;
                if self.rl {
                    let effort = kg_core::brain::Effort::new(self.effort);
                    if self.since >= effort.think_every(kg_core::games::paper::rl::DECIDE) {
                        self.since = 0.0;
                        if let Some(net) = kg_core::games::paper::rl::shipped() {
                            use kg_core::games::paper::rl;
                            let coarse = rl::Coarse::of(&g.w);
                            let a = net.choose(&rl::observe(&g.w, 0, &coarse), effort);
                            let me = g.me();
                            let want = me.angle + rl::TURNS[a];
                            g.aim(kg_core::games::paper::world::wrap_angle(want));
                        }
                    }
                } else if let Some(b) = &mut self.brain {
                    if self.since >= b.effort.think_every(0.16) {
                        self.since = 0.0;
                        let (aims, feats) = g.w.options(0);
                        let k = b.choose(&feats);
                        if let Some(a) = aims.get(k) {
                            g.aim(*a);
                        }
                    }
                }
            }
        }
        self.was_dead = g.state == State::Dead;
        self.best = self.best.max(g.run.peak.max(g.share(0)));
        self.game.update(&input, view, dt);
    }

    pub fn frame(&mut self, pen: &mut Pen, view: &View, dt: f32, typed: &[char]) {
        for c in typed {
            match c {
                '+' | '=' => self.speed = (self.speed * 2.0).min(MAX_SPEED),
                '-' | '_' => self.speed = (self.speed / 2.0).max(1.0),
                _ => {}
            }
        }
        let steps = self.speed.round().max(1.0) as usize;
        for _ in 0..steps {
            self.step(view, dt);
        }
        self.game.draw(pen, view);
        pen.offset = glam::Vec2::ZERO;
        let lines = [
            (
                format!(
                    "Brain playing, effort {:.2}, speed {}x",
                    self.effort, self.speed
                ),
                true,
            ),
            (
                format!(
                    "run {}   land {:.1}%   best {:.1}%",
                    self.runs + 1,
                    self.game.g.share(0) * 100.0,
                    self.best * 100.0
                ),
                false,
            ),
            (
                match (&self.brain, self.rl) {
                    (_, true) => format!(
                        "{} weights, conv net on a 32x32 view",
                        kg_core::games::paper::rl::shipped().map_or(0, |p| p.param_count())
                    ),
                    (Some(b), _) => format!(
                        "{} weights, thinks every {:.2} s",
                        b.net.weights.len(),
                        b.effort.think_every(0.16)
                    ),
                    (None, _) => "no brain file".into(),
                },
                false,
            ),
        ];
        let w = 470.0f32.min(view.w * 0.5);
        let r = Rect::new(
            16.0,
            view.h - 30.0 - lines.len() as f32 * 26.0 - 200.0,
            w,
            30.0 + lines.len() as f32 * 26.0,
        );
        pen.round_rect(r, 18.0, Color::new(0.06, 0.03, 0.15, 0.78));
        let gold = Color::from_rgba(255, 210, 70, 255);
        let dim = Color::new(0.75, 0.78, 0.9, 1.0);
        for (i, (l, strong)) in lines.iter().enumerate() {
            pen.text_left(
                &fit(l, w - 32.0, 22.0),
                r.x + 16.0,
                r.y + 27.0 + i as f32 * 26.0,
                22.0,
                if *strong { gold } else { dim },
            );
        }
    }
}

impl Default for BrainPaper {
    fn default() -> Self {
        Self::new()
    }
}

impl Default for JevClash {
    fn default() -> Self {
        Self::new()
    }
}

impl Default for Jev {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn long_overlay_lines_are_cut_to_the_panel() {
        let long = "flap now: safe, and the bird ends up far above the next gap";
        let cut = fit(long, 348.0, 22.0);
        assert!(cut.ends_with("..."));
        assert!(crate::draw::measure(&cut, 22.0) <= 348.0);
        assert_eq!(fit("run 5", 348.0, 22.0), "run 5");
    }
}
