use crate::games::flappy::FlappyPlay;
use crate::games::Play;
use crate::paint::Pen;
use kg_core::engine::View;
use kg_core::geom::{Color, Rect, WHITE};
use kg_core::jev::{mode_from, yalr_key, Player};

pub struct Jev {
    pub player: Option<Player>,
    pub game: FlappyPlay,
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
        let Some(player) = &mut self.player else {
            pen.fill_screen(view, Color::new(0.06, 0.03, 0.15, 1.0));
            pen.text_center("No Yalr key: set YALR_KEY or ?key=", view.w / 2.0, view.h / 2.0, 36.0, WHITE);
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
            pen.text_left(&shown, r.x + 16.0, r.y + 27.0 + i as f32 * 26.0, 22.0, if *strong { gold } else { dim });
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
