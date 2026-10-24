use crate::engine::*;
use crate::fx::*;
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
    since_menu: f32,
    pressed_tile: Option<usize>,
    press: Vec<f32>,
    launch: Option<(usize, f32)>,
    curtain: f32,
    home_press: f32,
    fx: Particles,
}

const LAUNCH_TIME: f32 = 0.35;

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
        let entries = games::all();
        let n = entries.len();
        Self {
            entries,
            current: None,
            scroll: 0.0,
            drag_from: None,
            dragged: false,
            t: 0.0,
            since_menu: 0.0,
            pressed_tile: None,
            press: vec![0.0; n],
            launch: None,
            curtain: 0.0,
            home_press: 0.0,
            fx: Particles::default(),
        }
    }

    pub fn open(&mut self, i: usize) {
        set_accent(self.entries[i].color);
        self.current = Some((i, (self.entries[i].make)()));
        self.curtain = 1.0;
    }

    fn go_home(&mut self) {
        self.current = None;
        self.since_menu = 0.0;
        self.curtain = 1.0;
        self.pressed_tile = None;
    }

    fn max_scroll(&self, view: &View) -> f32 {
        let rects = tile_rects(view, self.entries.len());
        let bottom = rects.last().map(|r| r.y + r.h + 30.0).unwrap_or(0.0);
        (bottom - view.h).max(0.0)
    }

    pub fn update(&mut self, input: &Input, view: &View, dt: f32) {
        self.t += dt;
        self.since_menu += dt;
        self.curtain = (self.curtain - dt * 3.5).max(0.0);
        self.fx.update(dt);
        tick_overlay(dt);
        if let Some((_, game)) = &mut self.current {
            self.home_press = approach(
                self.home_press,
                if input.down && HOME.contains(input.pos) {
                    1.0
                } else {
                    0.0
                },
                20.0,
                dt,
            );
            if input.tapped_in(HOME) || is_key_pressed(KeyCode::Escape) {
                self.go_home();
                return;
            }
            game.update(input, view, dt);
            return;
        }
        if let Some((i, t)) = &mut self.launch {
            *t += dt;
            if *t >= LAUNCH_TIME {
                let i = *i;
                self.launch = None;
                self.open(i);
            }
            return;
        }
        let rects = tile_rects(view, self.entries.len());
        let hit =
            |p: Vec2, scroll: f32| rects.iter().position(|r| r.contains(p + vec2(0.0, scroll)));
        if input.pressed {
            self.drag_from = Some((input.pos, self.scroll));
            self.dragged = false;
            self.pressed_tile = hit(input.pos, self.scroll);
        }
        if let (true, Some((start, s0))) = (input.down, self.drag_from) {
            let dy = input.pos.y - start.y;
            if dy.abs() > 12.0 {
                self.dragged = true;
                self.pressed_tile = None;
            }
            if self.dragged {
                self.scroll = (s0 - dy).clamp(0.0, self.max_scroll(view));
            }
        }
        if input.released {
            if !self.dragged {
                if let Some(i) =
                    hit(input.pos, self.scroll).filter(|i| Some(*i) == self.pressed_tile)
                {
                    self.launch = Some((i, 0.0));
                    let r = rects[i];
                    self.fx.sparkle(
                        vec2(r.center().x, r.center().y - self.scroll),
                        self.entries[i].color,
                    );
                }
            }
            self.drag_from = None;
            self.pressed_tile = None;
        }
        for (i, p) in self.press.iter_mut().enumerate() {
            let held = self.pressed_tile == Some(i) && input.down;
            *p = approach(*p, if held { 1.0 } else { 0.0 }, 18.0, dt);
        }
        self.scroll = self.scroll.clamp(0.0, self.max_scroll(view));
    }

    fn draw_home_button(&self) {
        let s = 1.0 - self.home_press * 0.12;
        let r = Rect::new(
            HOME.center().x - HOME.w * s / 2.0,
            HOME.center().y - HOME.h * s / 2.0,
            HOME.w * s,
            HOME.h * s,
        );
        round_rect(
            Rect::new(r.x, r.y + 4.0, r.w, r.h),
            14.0,
            Color::from_rgba(0, 0, 0, 60),
        );
        round_rect(r, 14.0, with_alpha(WHITE, 0.94));
        let c = r.center();
        let col = Color::from_rgba(80, 80, 110, 255);
        draw_triangle(
            vec2(c.x - 16.0 * s, c.y - 1.0),
            vec2(c.x + 16.0 * s, c.y - 1.0),
            vec2(c.x, c.y - 16.0 * s),
            col,
        );
        draw_rectangle(c.x - 11.0 * s, c.y - 2.0, 22.0 * s, 16.0 * s, col);
        draw_rectangle(c.x - 4.0 * s, c.y + 5.0 * s, 8.0 * s, 9.0 * s, WHITE);
    }

    fn draw_backdrop(&self, view: &View) {
        sky(
            view,
            Color::from_rgba(255, 190, 110, 255),
            Color::from_rgba(255, 120, 165, 255),
        );
        let rays = 14;
        let c = vec2(view.w / 2.0, -view.h * 0.15 - self.scroll * 0.3);
        let len = view.w.max(view.h) * 1.6;
        for k in 0..rays {
            let a0 = self.t * 0.08 + k as f32 * std::f32::consts::TAU / rays as f32;
            let a1 = a0 + std::f32::consts::TAU / rays as f32 * 0.5;
            draw_triangle(
                c,
                c + vec2(a0.cos(), a0.sin()) * len,
                c + vec2(a1.cos(), a1.sin()) * len,
                Color::new(1.0, 1.0, 1.0, 0.06),
            );
        }
        for k in 0..18 {
            let seed = k as f32 * 12.9898;
            let x = (seed.sin() * 43758.545).fract().abs() * view.w;
            let speed = 12.0 + (k % 5) as f32 * 6.0;
            let y = view.h + 40.0 - (self.t * speed + k as f32 * 97.0).rem_euclid(view.h + 120.0);
            let wob = (self.t * 1.3 + k as f32).sin() * 14.0;
            let r = 6.0 + (k % 4) as f32 * 5.0;
            let col = [
                Color::new(1.0, 1.0, 1.0, 0.22),
                Color::new(1.0, 0.95, 0.6, 0.25),
                Color::new(0.7, 0.9, 1.0, 0.22),
            ][k % 3];
            if k % 2 == 0 {
                draw_circle(x + wob, y, r, col);
                draw_circle(
                    x + wob - r * 0.3,
                    y - r * 0.3,
                    r * 0.3,
                    Color::new(1.0, 1.0, 1.0, 0.25),
                );
            } else {
                star(vec2(x + wob, y), r * 1.2, self.t * 0.6 + k as f32, col);
            }
        }
    }

    fn draw_title(&self, view: &View) {
        let title = "Kids Free Games";
        let size = (view.w / 11.0).min(72.0);
        let total = text_size(title, size).width;
        let mut x = view.w / 2.0 - total / 2.0;
        let y = 66.0 - self.scroll;
        for (i, ch) in title.chars().enumerate() {
            let s = ch.to_string();
            let w = text_size(&s, size).width;
            let intro = ease_out_back(((self.since_menu - i as f32 * 0.035) / 0.5).clamp(0.0, 1.0));
            let wave = (self.t * 3.0 - i as f32 * 0.45).sin() * 6.0;
            let col = hsv(i as f32 * 0.07 + self.t * 0.05, 0.55, 1.0);
            if ch != ' ' {
                text_outline(
                    &s,
                    x + w / 2.0,
                    y + wave - (1.0 - intro) * 60.0,
                    size * intro.max(0.01),
                    col,
                    Color::from_rgba(120, 40, 70, 255),
                );
            }
            x += w;
        }
        let tag = "No ads. No tracking. Just play.";
        let a = ((self.since_menu - 0.4) / 0.4).clamp(0.0, 1.0);
        let tw = text_size(tag, 26.0).width + 40.0;
        let r = Rect::new(view.w / 2.0 - tw / 2.0, y + 32.0, tw, 38.0);
        round_rect(r, 19.0, Color::new(1.0, 1.0, 1.0, 0.35 * a));
        text_center(
            tag,
            view.w / 2.0,
            r.y + 19.0,
            26.0,
            with_alpha(Color::from_rgba(130, 40, 70, 255), a),
        );
    }

    pub fn draw(&self, view: &View) {
        if let Some((_, game)) = &self.current {
            game.draw(view);
            self.draw_home_button();
            self.draw_curtain(view);
            return;
        }
        self.draw_backdrop(view);
        self.draw_title(view);
        for (i, (r, e)) in tile_rects(view, self.entries.len())
            .iter()
            .zip(&self.entries)
            .enumerate()
        {
            let r = Rect::new(r.x, r.y - self.scroll, r.w, r.h);
            if r.y > view.h || r.y + r.h < 0.0 {
                continue;
            }
            let intro =
                ease_out_back(((self.since_menu - 0.15 - i as f32 * 0.05) / 0.45).clamp(0.0, 1.0));
            if intro <= 0.0 {
                continue;
            }
            let bob = (self.t * 2.0 + i as f32 * 0.7).sin() * 4.0;
            let mut scale = intro * (1.0 - self.press[i] * 0.08);
            if let Some((li, lt)) = self.launch {
                if li == i {
                    scale *= 1.0 + ease_out_back(lt / LAUNCH_TIME) * 0.15;
                } else {
                    scale *= 1.0 - ease_out_cubic(lt / LAUNCH_TIME) * 0.15;
                }
            }
            let c = r.center() + vec2(0.0, bob + (1.0 - intro) * 40.0);
            let r = Rect::new(
                c.x - r.w * scale / 2.0,
                c.y - r.h * scale / 2.0,
                r.w * scale,
                r.h * scale,
            );
            draw_tile(r, e, self.t + i as f32 * 0.4);
        }
        self.fx.draw();
        if let Some((i, lt)) = self.launch {
            let k =
                ease_out_cubic(((lt - LAUNCH_TIME * 0.4) / (LAUNCH_TIME * 0.6)).clamp(0.0, 1.0));
            if k > 0.0 {
                let rects = tile_rects(view, self.entries.len());
                let c = rects[i].center() - vec2(0.0, self.scroll);
                let rad = k * view.w.hypot(view.h);
                draw_circle(c.x, c.y, rad, self.entries[i].color);
            }
        }
        self.draw_curtain(view);
    }

    fn draw_curtain(&self, view: &View) {
        if self.curtain <= 0.0 {
            return;
        }
        let color = self
            .current
            .as_ref()
            .map(|(i, _)| self.entries[*i].color)
            .unwrap_or(Color::from_rgba(255, 160, 140, 255));
        let k = ease_out_cubic(self.curtain);
        let rad = k * view.w.hypot(view.h) * 0.6;
        let c = view.center();
        draw_circle(c.x, c.y, rad, color);
    }
}

fn draw_tile(r: Rect, e: &Entry, t: f32) {
    let rad = r.w * 0.14;
    round_rect(
        Rect::new(r.x + 2.0, r.y + r.w * 0.05, r.w, r.h),
        rad,
        Color::from_rgba(120, 30, 70, 60),
    );
    round_rect(
        Rect::new(r.x, r.y + r.w * 0.03, r.w, r.h),
        rad,
        darker(e.color, 0.7),
    );
    round_rect(r, rad, WHITE);
    let ir = Rect::new(r.x + r.w * 0.07, r.y + r.w * 0.07, r.w * 0.86, r.w * 0.86);
    round_rect(ir, rad * 0.75, e.color);
    let inner = Rect::new(ir.x + ir.w * 0.1, ir.y + ir.h * 0.1, ir.w * 0.8, ir.h * 0.8);
    (e.icon)(inner);
    gloss(ir, rad * 0.75);
    shine_sweep(ir, t);
    let size = (r.w * 0.86 / text_size(e.name, 10.0).width * 10.0).min(r.w * 0.14);
    text_center(
        e.name,
        r.x + r.w / 2.0,
        ir.y + ir.h + (r.y + r.h - ir.y - ir.h) / 2.0,
        size,
        darker(e.color, 0.6),
    );
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
