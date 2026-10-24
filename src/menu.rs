use crate::engine::*;
use crate::games::{self, Entry};
use macroquad::prelude::*;

pub const HOME: Rect = Rect {
    x: 14.0,
    y: 12.0,
    w: 60.0,
    h: 48.0,
};

pub struct App {
    pub entries: Vec<Entry>,
    pub current: Option<(usize, Box<dyn Game>)>,
    scroll: f32,
    drag_from: Option<(Vec2, f32)>,
    dragged: bool,
    t: f32,
}

pub fn tile_rects(view: &View, count: usize) -> Vec<Rect> {
    let pad = 22.0;
    let top = 130.0;
    let fit = |cols: usize| {
        let rows = count.div_ceil(cols);
        let by_w = (view.w - pad * (cols as f32 + 1.0)) / cols as f32;
        let by_h = ((view.h - top - pad) / rows as f32 - pad) / 1.18;
        by_w.min(by_h)
    };
    let cols = (2..=6).max_by(|a, b| fit(*a).total_cmp(&fit(*b))).unwrap();
    let tw = fit(cols);
    let th = tw * 1.18;
    let left = (view.w - cols as f32 * tw - (cols as f32 - 1.0) * pad) / 2.0;
    (0..count)
        .map(|i| {
            let (c, r) = (i % cols, i / cols);
            Rect::new(
                left + c as f32 * (tw + pad),
                top + r as f32 * (th + pad),
                tw,
                th,
            )
        })
        .collect()
}

impl App {
    pub fn new() -> Self {
        Self {
            entries: games::all(),
            current: None,
            scroll: 0.0,
            drag_from: None,
            dragged: false,
            t: 0.0,
        }
    }

    pub fn open(&mut self, i: usize) {
        self.current = Some((i, (self.entries[i].make)()));
    }

    fn max_scroll(&self, view: &View) -> f32 {
        let rects = tile_rects(view, self.entries.len());
        let bottom = rects.last().map(|r| r.y + r.h + 30.0).unwrap_or(0.0);
        (bottom - view.h).max(0.0)
    }

    pub fn update(&mut self, input: &Input, view: &View, dt: f32) {
        self.t += dt;
        if let Some((_, game)) = &mut self.current {
            if input.tapped_in(HOME) || is_key_pressed(KeyCode::Escape) {
                self.current = None;
                return;
            }
            game.update(input, view, dt);
            return;
        }
        if input.pressed {
            self.drag_from = Some((input.pos, self.scroll));
            self.dragged = false;
        }
        if let (true, Some((start, s0))) = (input.down, self.drag_from) {
            let dy = input.pos.y - start.y;
            if dy.abs() > 12.0 {
                self.dragged = true;
            }
            if self.dragged {
                self.scroll = (s0 - dy).clamp(0.0, self.max_scroll(view));
            }
        }
        if input.released {
            if !self.dragged {
                let p = input.pos + vec2(0.0, self.scroll);
                if let Some(i) = tile_rects(view, self.entries.len())
                    .iter()
                    .position(|r| r.contains(p))
                {
                    self.open(i);
                }
            }
            self.drag_from = None;
        }
        self.scroll = self.scroll.clamp(0.0, self.max_scroll(view));
    }

    pub fn draw(&self, view: &View) {
        if let Some((_, game)) = &self.current {
            game.draw(view);
            round_rect(
                Rect::new(HOME.x, HOME.y + 4.0, HOME.w, HOME.h),
                14.0,
                Color::from_rgba(0, 0, 0, 60),
            );
            round_rect(HOME, 14.0, with_alpha(WHITE, 0.92));
            let c = HOME.center();
            let col = Color::from_rgba(80, 80, 110, 255);
            draw_triangle(
                vec2(c.x - 16.0, c.y - 1.0),
                vec2(c.x + 16.0, c.y - 1.0),
                vec2(c.x, c.y - 16.0),
                col,
            );
            draw_rectangle(c.x - 11.0, c.y - 2.0, 22.0, 16.0, col);
            draw_rectangle(c.x - 4.0, c.y + 5.0, 8.0, 9.0, WHITE);
            return;
        }
        sky(
            view,
            Color::from_rgba(255, 200, 120, 255),
            Color::from_rgba(255, 140, 170, 255),
        );
        let title_y = 64.0 - self.scroll;
        text_shadow("Kids Free Games", view.w / 2.0, title_y, 68.0, WHITE);
        text_center(
            "No ads. No tracking. Just play.",
            view.w / 2.0,
            title_y + 48.0,
            28.0,
            Color::from_rgba(130, 50, 60, 255),
        );
        for (i, (r, e)) in tile_rects(view, self.entries.len())
            .iter()
            .zip(&self.entries)
            .enumerate()
        {
            let r = Rect::new(r.x, r.y - self.scroll, r.w, r.h);
            if r.y > view.h || r.y + r.h < 0.0 {
                continue;
            }
            let bob = (self.t * 2.0 + i as f32 * 0.7).sin() * 3.0;
            let r = Rect::new(r.x, r.y + bob, r.w, r.h);
            round_rect(
                Rect::new(r.x, r.y + 8.0, r.w, r.h),
                26.0,
                Color::from_rgba(0, 0, 0, 50),
            );
            round_rect(r, 26.0, WHITE);
            let ir = Rect::new(r.x + 12.0, r.y + 12.0, r.w - 24.0, r.w - 24.0);
            round_rect(ir, 18.0, e.color);
            let inner = Rect::new(ir.x + ir.w * 0.1, ir.y + ir.h * 0.1, ir.w * 0.8, ir.h * 0.8);
            (e.icon)(inner);
            let size = (r.w * 0.9 / text_size(e.name, 10.0).width * 10.0).min(r.w * 0.15);
            text_center(
                e.name,
                r.x + r.w / 2.0,
                ir.y + ir.h + (r.h - ir.h - 12.0) / 2.0,
                size,
                Color::from_rgba(70, 70, 100, 255),
            );
        }
    }
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_tile_is_reachable_by_scrolling() {
        for v in [
            View::ipad_landscape(),
            View::ipad_portrait(),
            View::fit(390.0, 844.0),
        ] {
            let app = App::new();
            let rects = tile_rects(&v, app.entries.len());
            let ms = app.max_scroll(&v);
            for r in rects {
                assert!(r.x >= 0.0 && r.x + r.w <= v.w);
                assert!(r.y + r.h - ms <= v.h);
            }
        }
    }

    #[test]
    fn every_game_survives_a_minute_of_random_input() {
        rand::srand(1);
        for v in [View::ipad_landscape(), View::ipad_portrait()] {
            for e in games::all() {
                let mut g = (e.make)();
                for f in 0..3600 {
                    let pos = vec2(rand::gen_range(0.0, v.w), rand::gen_range(0.0, v.h));
                    let mut input = Input {
                        pos,
                        prev_pos: pos - vec2(30.0, 10.0),
                        down: f % 7 < 4,
                        ..Default::default()
                    };
                    input.pressed = f % 7 == 0;
                    input.released = f % 7 == 4;
                    if f % 11 == 0 {
                        input.swipe = Some(Dir::ALL[rand::gen_range(0, 4)]);
                    }
                    g.update(&input, &v, 1.0 / 60.0);
                }
            }
        }
    }
}
