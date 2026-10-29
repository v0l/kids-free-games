use super::Play;
use crate::draw::{Draw, PLUM};
use crate::paint::Pen;
use crate::ui::{plain_score, Results, Tone};
use glam::{vec2, Vec2};
use kg_core::engine::{Input, View};
use kg_core::fx::{ease_out_back, ease_out_cubic, Particle, Side};
use kg_core::games::clash::sim::{level_progress, Snake};
use kg_core::games::clash::skins::{Skin, SKINS};
use kg_core::games::clash::tower::Goal;
use kg_core::games::clash::*;
use kg_core::games::clash::{rating, sight};
use kg_core::geom::{Color, Rect, WHITE};
use std::f32::consts::TAU;

pub const KEY: &str = GAME;
const SHEET: &str = "clash";

pub struct ClashPlay {
    pub g: Clash,
    pub results_age: f32,
}

impl ClashPlay {
    pub fn new() -> Self {
        Self {
            g: Clash::new(),
            results_age: 0.0,
        }
    }
}

impl Default for ClashPlay {
    fn default() -> Self {
        Self::new()
    }
}

impl Play for ClashPlay {
    fn update(&mut self, input: &Input, view: &View, dt: f32) {
        let showing = self.g.shows_results();
        self.g.update(input, view, dt);
        if self.g.shows_results() {
            if !showing {
                self.results_age = 0.0;
            }
            self.results_age += dt;
        }
    }

    fn draw(&self, pen: &mut Pen, view: &View) {
        draw(&self.g, pen, view, self.results_age);
    }

    fn score(&self) -> u32 {
        self.g.world.player().level()
    }

    fn over(&self) -> bool {
        self.g.screen == Screen::Over
    }

    fn as_any(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

const THEMES: [([u8; 3], [u8; 3], [u8; 3]); 5] = [
    ([118, 210, 116], [104, 196, 106], [60, 120, 80]),
    ([150, 214, 244], [132, 198, 232], [70, 110, 160]),
    ([244, 208, 128], [232, 192, 112], [150, 110, 70]),
    ([204, 160, 236], [190, 144, 224], [100, 70, 150]),
    ([250, 168, 168], [240, 150, 154], [150, 70, 90]),
];

fn rgb(c: [u8; 3]) -> Color {
    Color::from_rgba(c[0], c[1], c[2], 255)
}

fn theme(g: &Clash) -> usize {
    if g.mode == Mode::Tower && g.screen != Screen::Title {
        g.floor.theme
    } else {
        0
    }
}

fn hash(x: i32, y: i32) -> u32 {
    let mut h = (x as u32).wrapping_mul(374761393) ^ (y as u32).wrapping_mul(668265263);
    h = (h ^ (h >> 13)).wrapping_mul(1274126177);
    h ^ (h >> 16)
}

fn draw_floor(g: &Clash, pen: &mut Pen, view: &View) {
    let (a, b, edge) = THEMES[theme(g)];
    let f = view.full();
    pen.fill_screen(view, rgb(edge).darker(0.55));
    let size = g.world.size;
    let tl = g.to_screen(view, Vec2::ZERO);
    let br = g.to_screen(view, Vec2::splat(size));
    pen.round_rect(
        Rect::new(
            tl.x - 10.0,
            tl.y - 10.0,
            br.x - tl.x + 20.0,
            br.y - tl.y + 20.0,
        ),
        24.0 * g.zoom,
        rgb(edge),
    );
    let tile = 150.0;
    let lo = g.to_world(view, vec2(f.x, f.y));
    let hi = g.to_world(view, vec2(f.x + f.w, f.y + f.h));
    let (x0, x1) = (
        (lo.x / tile).floor().max(0.0) as i32,
        (hi.x / tile).ceil().min(size / tile) as i32,
    );
    let (y0, y1) = (
        (lo.y / tile).floor().max(0.0) as i32,
        (hi.y / tile).ceil().min(size / tile) as i32,
    );
    let ts = tile * g.zoom;
    for ty in y0..y1 {
        for tx in x0..x1 {
            let p = g.to_screen(view, vec2(tx as f32 * tile, ty as f32 * tile));
            let col = if (tx + ty) % 2 == 0 { rgb(a) } else { rgb(b) };
            pen.rect(Rect::new(p.x, p.y, ts + 1.0, ts + 1.0), col);
            let h = hash(tx, ty);
            if h.is_multiple_of(4) {
                let jx = (h >> 8) % 100;
                let jy = (h >> 16) % 100;
                let c = p + vec2(jx as f32 / 100.0, jy as f32 / 100.0) * ts * 0.7;
                pen.sprite_at(
                    SHEET,
                    "tuft",
                    c,
                    ts * 0.32,
                    Draw::tint(WHITE.with_alpha(0.55)),
                );
            }
        }
    }
}

fn draw_rocks(g: &Clash, pen: &mut Pen, view: &View) {
    for r in &g.world.rocks {
        let c = g.to_screen(view, r.pos);
        let rr = r.r * g.zoom;
        pen.soft_shadow(c + vec2(0.0, rr * 0.9), rr * 1.1, rr * 0.3, 0.6);
        pen.circle(c, rr + 4.0, PLUM);
        pen.circle(c, rr, Color::from_rgba(150, 150, 170, 255));
        pen.circle(
            c - vec2(rr * 0.15, rr * 0.2),
            rr * 0.75,
            Color::from_rgba(180, 180, 200, 255),
        );
        pen.circle(
            c - vec2(rr * 0.3, rr * 0.4),
            rr * 0.25,
            Color::new(1.0, 1.0, 1.0, 0.5),
        );
    }
}

fn on_screen(view: &View, p: Vec2, pad: f32) -> bool {
    let f = view.full();
    p.x > f.x - pad && p.x < f.x + f.w + pad && p.y > f.y - pad && p.y < f.y + f.h + pad
}

fn draw_eggs(g: &Clash, pen: &mut Pen, view: &View) {
    for e in &g.world.eggs {
        let c = g.to_screen(view, e.pos);
        if !on_screen(view, c, 40.0) {
            continue;
        }
        let bob = (g.t * 3.0 + e.age * 2.0 + e.pos.x * 0.01).sin() * 0.06;
        let h = (if e.value > 1 { 40.0 } else { 27.0 } + e.value.min(8) as f32) * g.zoom.max(0.5);
        let tint = match e.kind {
            0 => WHITE,
            1 => Color::from_rgba(255, 190, 190, 255),
            2 => Color::from_rgba(190, 240, 255, 255),
            _ => Color::from_rgba(210, 255, 190, 255),
        };
        pen.sprite_at(
            SHEET,
            "egg",
            c + vec2(0.0, bob * h),
            h * (1.0 + bob),
            Draw::tint(tint),
        );
    }
}

fn draw_powers(g: &Clash, pen: &mut Pen, view: &View) {
    use kg_core::games::clash::powers::{self, Kind};
    for p in &g.world.powers {
        let c = g.to_screen(view, p.pos);
        if !on_screen(view, c, 60.0) {
            continue;
        }
        let col = powers::color(p.kind);
        let pop = kg_core::fx::ease_out_back((p.age / 0.4).min(1.0)).max(0.05);
        let r = 26.0 * g.zoom.max(0.5) * pop * (1.0 + 0.08 * (g.t * 4.0 + p.pos.x).sin());
        let at = c + vec2(0.0, (g.t * 2.5 + p.pos.y).sin() * r * 0.15);
        pen.soft_shadow(c + vec2(0.0, r * 0.9), r * 0.8, r * 0.3, 0.35);
        pen.circle(at, r * 1.45, col.with_alpha(0.22));
        pen.circle(at, r + 3.0, PLUM);
        pen.circle(at, r, col);
        pen.circle(
            at - vec2(r * 0.3, r * 0.35),
            r * 0.38,
            col.lerp(WHITE, 0.55),
        );
        let ink = WHITE;
        let w = (r * 0.16).max(2.5);
        match p.kind {
            Kind::Speed => {
                let pts = [
                    at + vec2(r * 0.15, -r * 0.6),
                    at + vec2(-r * 0.25, r * 0.05),
                    at + vec2(r * 0.15, r * 0.05),
                    at + vec2(-r * 0.15, r * 0.6),
                ];
                for k in 0..3 {
                    pen.line(pts[k], pts[k + 1], w, ink);
                }
            }
            Kind::Magnet => {
                pen.ring(at, r * 0.42, w, ink);
                pen.rect(
                    kg_core::geom::Rect::new(at.x - r * 0.5, at.y - r * 0.05, r, r * 0.55),
                    col,
                );
                pen.line(
                    at + vec2(-r * 0.42, 0.0),
                    at + vec2(-r * 0.42, r * 0.4),
                    w,
                    ink,
                );
                pen.line(
                    at + vec2(r * 0.42, 0.0),
                    at + vec2(r * 0.42, r * 0.4),
                    w,
                    ink,
                );
            }
            Kind::Shield => {
                pen.ring(at, r * 0.55, w, ink);
                pen.circle(at, r * 0.2, ink);
            }
            Kind::Grow => {
                pen.line(
                    at - vec2(r * 0.45, 0.0),
                    at + vec2(r * 0.45, 0.0),
                    w * 1.2,
                    ink,
                );
                pen.line(
                    at - vec2(0.0, r * 0.45),
                    at + vec2(0.0, r * 0.45),
                    w * 1.2,
                    ink,
                );
            }
        }
    }
}

const SHIELD: Color = Color::new(0.6, 0.9, 1.0, 1.0);

struct Look {
    mark: Option<Color>,
}

fn wobble(g: &Clash, s: &Snake, i: usize, dir: Vec2) -> Vec2 {
    if i == 0 {
        return Vec2::ZERO;
    }
    let k = (g.t * 7.0 - i as f32 * 0.6 + s.phase).sin();
    vec2(-dir.y, dir.x) * k * s.radius() * 0.07 * (i as f32 / 3.0).min(1.0)
}

fn draw_snake(g: &Clash, pen: &mut Pen, view: &View, s: &Snake, look: &Look) {
    let zoom = g.zoom;
    let skin = SKINS[s.skin.min(SKINS.len() - 1)];
    let n = s.segs.len();
    let pts: Vec<Vec2> = (0..n)
        .map(|i| {
            let prev = s.segs[i.saturating_sub(1)];
            let next = s.segs[(i + 1).min(n - 1)];
            let d = (prev - next).normalize_or_zero();
            g.to_screen(view, s.segs[i] + wobble(g, s, i, d))
        })
        .collect();
    let squash = 1.0 + 0.18 * (s.pop * std::f32::consts::PI).sin();
    let radii: Vec<f32> = (0..n)
        .map(|i| s.seg_radius(i) * zoom * if i < 4 { squash } else { 1.0 })
        .collect();
    let visible: Vec<usize> = (0..n)
        .filter(|i| on_screen(view, pts[*i], radii[*i] + 8.0))
        .collect();
    if visible.is_empty() {
        return;
    }
    let glow = 0.0;
    if s.shield > 0.0 {
        let pulse = 0.5 + 0.5 * (g.t * 6.0).sin();
        for &i in visible.iter().rev() {
            pen.circle(
                pts[i],
                radii[i] + (8.0 + 3.0 * pulse) * zoom.max(0.6),
                SHIELD,
            );
        }
    }
    pen.soft_shadow(
        pts[0] + vec2(0.0, radii[0] * 0.5),
        radii[0] * 1.2,
        radii[0] * 0.5,
        0.35,
    );
    for &i in visible.iter().rev() {
        pen.circle(
            pts[i] + vec2(0.0, radii[i] * 0.08),
            radii[i] + 3.0 * zoom.max(0.6),
            PLUM,
        );
    }
    for &i in visible.iter().rev() {
        let col = if i == 0 {
            skin.head()
        } else {
            skin.segment(i, n)
        };
        let col = col.lerp(SHIELD, glow);
        pen.circle(pts[i], radii[i], col);
        pen.circle(
            pts[i] - vec2(radii[i] * 0.22, radii[i] * 0.3),
            radii[i] * 0.52,
            col.lerp(WHITE, 0.28).with_alpha(0.8),
        );
    }
    if let Some(mark) = look.mark {
        pen.ring(pts[0], radii[0] * 1.35, 3.0, mark.with_alpha(0.8));
    }
    if s.shield > 0.0 {
        pen.ring(pts[0], radii[0] * 1.6, 5.0, SHIELD);
    }
    if s.boosts.magnet > 0.0 {
        let pulse = (g.t * 3.0).fract();
        let col = kg_core::games::clash::powers::color(kg_core::games::clash::powers::Kind::Magnet);
        pen.ring(
            pts[0],
            radii[0] * (1.6 + 2.2 * pulse),
            3.0,
            col.with_alpha(0.5 * (1.0 - pulse)),
        );
    }
    if s.boosts.speed > 0.0 {
        let col = kg_core::games::clash::powers::color(kg_core::games::clash::powers::Kind::Speed);
        let back = -s.dir();
        let side = vec2(-back.y, back.x);
        for k in -1..=1 {
            let from = pts[0] + back * radii[0] * 1.1 + side * k as f32 * radii[0] * 0.5;
            let len = radii[0] * (1.4 + 0.5 * ((g.t * 20.0 + k as f32).sin() * 0.5 + 0.5));
            pen.line(from, from + back * len, 3.0, col.with_alpha(0.7));
        }
    }
    draw_face(pen, pts[0], s.dir(), radii[0], s.stun > 0.0, g.t + s.phase);
}

fn draw_face(pen: &mut Pen, c: Vec2, dir: Vec2, r: f32, dizzy: bool, t: f32) {
    let side = vec2(-dir.y, dir.x);
    let blink = (t * 1.3).sin() > 0.97;
    for k in [-1.0, 1.0] {
        let e = c + dir * r * 0.32 + side * k * r * 0.5;
        let er = r * 0.42;
        pen.circle(e, er + 1.5, PLUM);
        pen.circle(e, er, WHITE);
        if blink {
            pen.line(e - side * er * 0.8, e + side * er * 0.8, 2.5, PLUM);
        } else if dizzy {
            pen.ring(e, er * 0.5, 2.0, PLUM);
        } else {
            pen.circle(e + dir * er * 0.35, er * 0.52, PLUM);
            pen.circle(
                e + dir * er * 0.35 - Vec2::splat(er * 0.14),
                er * 0.16,
                WHITE,
            );
        }
    }
    let m = c + dir * r * 0.82;
    pen.circle(m, r * 0.1, PLUM.with_alpha(0.7));
}

fn level_pill(g: &Clash, pen: &mut Pen, view: &View, s: &Snake, mine: u32) {
    let head = g.to_screen(view, s.head());
    let r = s.radius() * g.zoom;
    if !on_screen(view, head, r + 60.0) {
        return;
    }
    let lv = s.level();
    let (bg, ink) = if s.boss {
        (Color::from_rgba(230, 60, 80, 255), WHITE)
    } else if s.player {
        (Color::from_rgba(255, 214, 70, 255), PLUM)
    } else if lv > mine {
        (Color::from_rgba(235, 80, 90, 255), WHITE)
    } else if lv < mine {
        (Color::from_rgba(90, 200, 110, 255), WHITE)
    } else {
        (Color::from_rgba(230, 230, 240, 255), PLUM)
    };
    let label = lv.to_string();
    let size = (26.0 * g.zoom.clamp(0.8, 1.3)).max(22.0);
    let w = crate::draw::measure(&label, size) + 22.0;
    let h = size + 10.0;
    let at = head - vec2(0.0, r + h * 0.8 + 6.0);
    let pop = 1.0 + 0.3 * (s.pop * std::f32::consts::PI).sin();
    let rect = Rect::new(at.x - w * pop / 2.0, at.y - h * pop / 2.0, w * pop, h * pop);
    pen.round_rect(
        rect.offset(vec2(0.0, 3.0)),
        rect.h / 2.0,
        Color::new(0.0, 0.0, 0.1, 0.3),
    );
    pen.round_rect(rect, rect.h / 2.0, PLUM);
    let inner = Rect::new(rect.x + 2.5, rect.y + 2.5, rect.w - 5.0, rect.h - 5.0);
    pen.round_rect(inner, inner.h / 2.0, bg);
    pen.text_center(&label, at.x, at.y + 1.0, size * pop, ink);
    if s.boss {
        pen.sprite_at(
            "profiles",
            "crown",
            at - vec2(0.0, h * 0.9),
            h * 1.3,
            Draw::default(),
        );
    }
    if !s.player {
        let name = s.name.as_str();
        pen.text_shadow(
            name,
            at.x,
            at.y - h * 0.85 - if s.boss { h } else { 0.0 },
            20.0,
            WHITE,
        );
    }
}

fn draw_arrows(g: &Clash, pen: &mut Pen, view: &View) {
    let c = g.centre(view);
    let f = view.full();
    let pad = 46.0;
    for arrow in sight::arrows(&g.world, 0) {
        let p = g.to_screen(view, arrow.at);
        if on_screen(view, p, 0.0) {
            continue;
        }
        let d = p - c;
        let k =
            ((f.w / 2.0 - pad) / d.x.abs().max(1.0)).min((f.h / 2.0 - pad) / d.y.abs().max(1.0));
        let at = c + d * k;
        let at = vec2(
            at.x.clamp(f.x + pad, f.x + f.w - pad),
            at.y.clamp(f.y + pad, f.y + f.h - pad),
        );
        let dir = d.normalize_or_zero();
        let side = vec2(-dir.y, dir.x);
        let beat = 1.0 + 0.12 * (g.t * 8.0).sin();
        let col = Color::from_rgba(235, 70, 80, 255);
        pen.tri(
            at + dir * 20.0 * beat,
            at - dir * 12.0 + side * 18.0,
            at - dir * 12.0 - side * 18.0,
            PLUM,
        );
        pen.tri(
            at + dir * 15.0 * beat,
            at - dir * 9.0 + side * 13.0,
            at - dir * 9.0 - side * 13.0,
            col,
        );
        pen.text_shadow(
            &arrow.level.to_string(),
            at.x - dir.x * 36.0,
            at.y - dir.y * 36.0,
            24.0,
            WHITE,
        );
    }
}

fn draw_fx(g: &Clash, pen: &mut Pen, view: &View) {
    for (at, age, col) in &g.rings {
        let k = ease_out_cubic(age / 0.6);
        let c = g.to_screen(view, *at);
        pen.ring(
            c,
            (40.0 + 160.0 * k) * g.zoom.max(0.6),
            8.0 * (1.0 - k),
            col.with_alpha(1.0 - k),
        );
    }
    for p in &g.fx.list {
        let q = Particle {
            pos: g.to_screen(view, p.pos),
            size: p.size * g.zoom.clamp(0.7, 1.5),
            ..*p
        };
        pen.particle(&q);
    }
    for p in &g.popups.list {
        let (pop, fade) = kg_core::fx::popup_scale(p);
        let c = g.to_screen(view, p.pos);
        pen.text_outline(
            &p.text,
            c.x,
            c.y,
            p.size * pop,
            p.color.with_alpha(fade),
            PLUM.with_alpha(fade),
        );
    }
}

fn draw_world(g: &Clash, pen: &mut Pen, view: &View) {
    draw_floor(g, pen, view);
    draw_rocks(g, pen, view);
    draw_eggs(g, pen, view);
    draw_powers(g, pen, view);
    let me = g.world.player().level();
    let mut order: Vec<&Snake> = g.world.snakes.iter().filter(|s| s.alive).collect();
    order.sort_by_key(|s| (s.player, s.level()));
    for s in &order {
        let look = Look {
            mark: if s.player {
                None
            } else if s.boss || s.level() > me {
                Some(Color::from_rgba(235, 70, 80, 255))
            } else if s.level() < me {
                Some(Color::from_rgba(90, 210, 110, 255))
            } else {
                None
            },
        };
        draw_snake(g, pen, view, s, &look);
    }
    for s in &order {
        level_pill(g, pen, view, s, me);
    }
}

fn draw_aim(g: &Clash, pen: &mut Pen, view: &View) {
    let p = g.world.player();
    if !p.alive || g.screen != Screen::Playing {
        return;
    }
    let c = g.to_screen(view, p.head());
    let dir = vec2(g.steer.cos(), g.steer.sin());
    let r = p.radius() * g.zoom;
    let fade = if g.run_t < 6.0 { 0.9 } else { 0.35 };
    for k in 0..3 {
        let at = c + dir * (r * 2.2 + k as f32 * r * 0.9);
        pen.circle(
            at,
            r * (0.22 - k as f32 * 0.04),
            Color::new(1.0, 1.0, 1.0, fade - k as f32 * 0.15),
        );
    }
}

fn draw_minimap(g: &Clash, pen: &mut Pen, view: &View) {
    let d = (view.w.min(view.h) * 0.24).clamp(110.0, 170.0);
    let r = Rect::new(
        view.w - d - 14.0,
        view.h - d - 14.0 - view.inset.bottom.min(24.0),
        d,
        d,
    );
    pen.round_rect(
        r.offset(vec2(0.0, 3.0)),
        12.0,
        Color::new(0.0, 0.0, 0.1, 0.3),
    );
    pen.round_rect(r, 12.0, Color::new(0.1, 0.06, 0.25, 0.62));
    for (at, dot) in sight::minimap(&g.world, 0) {
        let p = r.point() + at / g.world.size * d;
        let col = match dot {
            sight::Dot::Boss => Color::from_rgba(255, 60, 80, 255),
            sight::Dot::Bigger => Color::from_rgba(240, 90, 100, 255),
            sight::Dot::Smaller => Color::from_rgba(110, 220, 130, 255),
            sight::Dot::Even => Color::from_rgba(220, 220, 230, 255),
        };
        pen.circle(p, if dot == sight::Dot::Boss { 5.5 } else { 3.0 }, col);
    }
    if g.world.player_alive() {
        let p = r.point() + g.world.player().head() / g.world.size * d;
        pen.circle(p, 6.0, PLUM);
        pen.circle(p, 4.5, Color::from_rgba(255, 214, 70, 255));
    }
}

fn draw_board(g: &Clash, pen: &mut Pen, view: &View) {
    let rows = g.world.leaderboard();
    let w = 190.0f32.min(view.w * 0.3);
    let rh = 30.0;
    let top = 76.0;
    let x = view.w - w - 14.0;
    let shown: Vec<_> = rows.iter().take(5).collect();
    let me_rank = rows.iter().position(|r| r.3);
    pen.round_rect(
        Rect::new(
            x,
            top,
            w,
            12.0 + rh * (shown.len() + usize::from(me_rank.is_some_and(|r| r >= 5))) as f32,
        ),
        14.0,
        Color::new(0.1, 0.06, 0.25, 0.55),
    );
    let line = |pen: &mut Pen, i: usize, rank: usize, name: &str, xp: u32, mine: bool| {
        let y = top + 6.0 + rh * i as f32;
        if mine {
            pen.round_rect(
                Rect::new(x + 4.0, y, w - 8.0, rh - 2.0),
                10.0,
                Color::from_rgba(255, 214, 70, 230),
            );
        }
        let ink = if mine { PLUM } else { WHITE };
        pen.text_left(&format!("{}", rank + 1), x + 10.0, y + rh / 2.0, 20.0, ink);
        pen.text_left(
            &name.chars().take(8).collect::<String>(),
            x + 34.0,
            y + rh / 2.0,
            20.0,
            ink,
        );
        let lv = sim::level_of(xp).to_string();
        let lw = crate::draw::measure(&lv, 22.0);
        pen.text_left(&lv, x + w - 12.0 - lw, y + rh / 2.0, 22.0, ink);
    };
    for (i, r) in shown.iter().enumerate() {
        line(pen, i, i, &r.1, r.2, r.3);
    }
    if let Some(m) = me_rank.filter(|m| *m >= 5) {
        line(pen, shown.len(), m, &rows[m].1, rows[m].2, true);
    }
}

fn goal_icon(pen: &mut Pen, g: &Clash, c: Vec2, r: f32) {
    match g.floor.goal {
        Goal::Level(_) => pen.star(c, r, 0.0, Color::from_rgba(255, 214, 70, 255)),
        Goal::Eat(_) => {
            pen.circle(c, r, PLUM);
            pen.circle(c, r * 0.85, Color::from_rgba(230, 100, 120, 255));
            for k in [-1.0, 1.0] {
                pen.circle(c + vec2(k * r * 0.35, -r * 0.15), r * 0.25, WHITE);
                pen.circle(c + vec2(k * r * 0.35, -r * 0.1), r * 0.12, PLUM);
            }
            pen.arc(c + vec2(0.0, r * 0.1), r * 0.5, 0.2, 2.9, PLUM);
        }
        Goal::Survive(_) => {
            pen.circle(c, r, PLUM);
            pen.circle(c, r * 0.85, WHITE);
            pen.line(c, c + vec2(0.0, -r * 0.6), 3.0, PLUM);
            pen.line(c, c + vec2(r * 0.4, 0.0), 3.0, PLUM);
        }
        Goal::Boss(_) => pen.sprite_at("profiles", "crown", c, r * 2.2, Draw::default()),
    }
}

fn goal_number(g: &Clash) -> String {
    match g.floor.goal {
        Goal::Level(n) | Goal::Eat(n) | Goal::Boss(n) => n.to_string(),
        Goal::Survive(t) => format!("{}s", t as u32),
    }
}

fn draw_hud(g: &Clash, pen: &mut Pen, view: &View) {
    let p = g.world.player();
    pen.chip(view, Side::Left, "LV", &p.level().to_string(), g.bump.t);
    let w = 220.0f32.min(view.w * 0.34);
    let bar = Rect::new(90.0, 70.0, w, 16.0);
    pen.round_rect(bar, 8.0, Color::new(0.1, 0.06, 0.25, 0.55));
    let fill = level_progress(p.xp).max(0.04);
    pen.round_rect(
        Rect::new(bar.x + 2.0, bar.y + 2.0, (bar.w - 4.0) * fill, bar.h - 4.0),
        6.0,
        Color::from_rgba(255, 214, 70, 255),
    );
    if g.mode == Mode::Tower {
        let bw = 240.0f32.min(view.w * 0.36);
        let r = Rect::new(view.w / 2.0 - bw / 2.0 + 30.0, 16.0, bw, 30.0);
        pen.round_rect(
            r.offset(vec2(0.0, 3.0)),
            15.0,
            Color::new(0.0, 0.0, 0.1, 0.3),
        );
        pen.round_rect(r, 15.0, Color::new(0.1, 0.06, 0.25, 0.7));
        let k = g.floor.progress(&g.world).max(0.03);
        pen.round_rect(
            Rect::new(r.x + 3.0, r.y + 3.0, (r.w - 6.0) * k, r.h - 6.0),
            12.0,
            Color::from_rgba(110, 220, 130, 255),
        );
        goal_icon(pen, g, vec2(r.x - 6.0, r.center().y), 24.0);
        pen.text_shadow(
            &goal_number(g),
            r.x + r.w / 2.0,
            r.center().y + 1.0,
            24.0,
            WHITE,
        );
    }
    if g.mode == Mode::Survival {
        let left = g.world.snakes_left();
        let r = Rect::new(view.w / 2.0 - 90.0, 16.0, 180.0, 40.0);
        pen.round_rect(
            r.offset(vec2(0.0, 3.0)),
            20.0,
            Color::new(0.0, 0.0, 0.1, 0.3),
        );
        pen.round_rect(r, 20.0, Color::new(0.1, 0.06, 0.25, 0.7));
        pen.text_shadow(
            &format!("{left} left"),
            r.x + r.w / 2.0,
            r.center().y + 1.0,
            26.0,
            WHITE,
        );
    }
    draw_board(g, pen, view);
    draw_minimap(g, pen, view);
}

fn draw_play(g: &Clash, pen: &mut Pen, view: &View) {
    pen.offset = g.shake.offset();
    draw_world(g, pen, view);
    draw_aim(g, pen, view);
    draw_fx(g, pen, view);
    draw_arrows(g, pen, view);
    pen.offset = Vec2::ZERO;
    pen.vignette(view, 0.5);
    if g.flash > 0.0 {
        pen.fill_screen(view, WHITE.with_alpha(g.flash * 0.5));
    }
    draw_hud(g, pen, view);
    if g.combo >= 3 && g.combo_t > 0.0 {
        pen.text_outline(
            &format!("x{}", g.combo),
            view.w / 2.0,
            view.h * 0.2,
            44.0 * kg_core::fx::bump_scale(g.bump.t, 0.3),
            Color::from_rgba(255, 230, 90, 255),
            PLUM,
        );
    }
    if let Some((t, left)) = &g.toast {
        let k = ease_out_back((2.2 - left).min(0.4) / 0.4).max(0.0) * (left / 0.4).min(1.0);
        pen.ribbon(
            view,
            t,
            vec2(view.w / 2.0, view.h * 0.18 - (1.0 - k) * 60.0),
            g.t,
        );
    }
    if g.run_t < 7.0 && g.ending == 0.0 {
        let pulse = 1.0 + (g.t * 5.0).sin() * 0.05;
        let (label, y) = if g.training {
            ("Drag to steer, eat the gold eggs", view.h * 0.82)
        } else {
            ("Eat smaller snakes", view.h * 0.86)
        };
        if g.training || g.run_t < 4.0 {
            pen.hint(label, vec2(view.w / 2.0, y), pulse);
        }
    }
}

fn draw_skin_preview(pen: &mut Pen, skin: Skin, c: Vec2, scale: f32, t: f32) {
    let n = 22;
    let r = 30.0 * scale;
    let pts: Vec<Vec2> = (0..n)
        .map(|i| {
            let k = i as f32 / (n - 1) as f32;
            let a = -0.5 + k * TAU * 0.9;
            let rad = r * (3.2 - k * 1.6);
            c + vec2(
                a.cos() * rad,
                a.sin() * rad * 0.5 + 8.0 * (t * 2.0 + k * 6.0).sin() * scale,
            )
        })
        .collect();
    let pts: Vec<Vec2> = pts.into_iter().rev().collect();
    let radius = |i: usize| r * (1.0 - 0.38 * i as f32 / (n - 1) as f32);
    let order: Vec<usize> = (0..n).collect();
    for &i in &order {
        pen.circle(pts[i], radius(i) + 3.5, PLUM);
    }
    for &i in &order {
        let col = if i == 0 {
            skin.head()
        } else {
            skin.segment(i, n)
        };
        pen.circle(pts[i], radius(i), col);
        pen.circle(
            pts[i] - vec2(radius(i) * 0.22, radius(i) * 0.3),
            radius(i) * 0.5,
            col.lerp(WHITE, 0.28),
        );
    }
    let dir = (pts[0] - pts[1]).normalize_or_zero();
    draw_face(pen, pts[0], dir, radius(0), false, t);
}

fn lock_icon(pen: &mut Pen, c: Vec2, s: f32) {
    pen.ring(c - vec2(0.0, s * 0.45), s * 0.38, s * 0.16, PLUM);
    pen.round_rect(
        Rect::new(c.x - s * 0.55, c.y - s * 0.15, s * 1.1, s * 0.85),
        s * 0.18,
        PLUM,
    );
    pen.round_rect(
        Rect::new(c.x - s * 0.47, c.y - s * 0.07, s * 0.94, s * 0.69),
        s * 0.14,
        Color::from_rgba(255, 190, 60, 255),
    );
    pen.circle(c + vec2(0.0, s * 0.2), s * 0.12, PLUM);
}

fn need_icon(pen: &mut Pen, need: skins::Need, c: Vec2, r: f32) {
    use skins::Need;
    match need {
        Need::Level(_) => pen.star(c, r, 0.0, Color::from_rgba(255, 214, 70, 255)),
        Need::Eat(_) => {
            pen.circle(c, r, PLUM);
            pen.circle(c, r * 0.85, Color::from_rgba(230, 100, 120, 255));
            for k in [-1.0, 1.0] {
                pen.circle(c + vec2(k * r * 0.35, -r * 0.15), r * 0.25, WHITE);
            }
        }
        Need::Floor(_) => {
            pen.round_rect(
                Rect::new(c.x - r * 0.8, c.y - r, r * 1.6, r * 2.0),
                r * 0.2,
                PLUM,
            );
            pen.round_rect(
                Rect::new(c.x - r * 0.65, c.y - r * 0.85, r * 1.3, r * 1.7),
                r * 0.15,
                Color::from_rgba(150, 120, 230, 255),
            );
            pen.round_rect(
                Rect::new(c.x - r * 0.2, c.y + r * 0.1, r * 0.4, r * 0.75),
                r * 0.1,
                PLUM,
            );
        }
        Need::Games(_) => pen.sprite_at(SHEET, "egg", c, r * 2.2, Draw::default()),
        Need::Free => {}
    }
}

fn arrow_button(pen: &mut Pen, r: Rect, left: bool, on: bool, bump: f32) {
    let k = 1.0 + 0.12 * (1.0 - ease_out_cubic(bump));
    let c = r.center();
    let d = r.w * k;
    pen.circle(c + vec2(0.0, 4.0), d / 2.0, Color::new(0.0, 0.0, 0.1, 0.3));
    pen.circle(
        c,
        d / 2.0,
        if on {
            Color::from_rgba(255, 255, 255, 235)
        } else {
            Color::new(1.0, 1.0, 1.0, 0.35)
        },
    );
    let s = if left { -1.0 } else { 1.0 };
    pen.tri(
        c + vec2(s * d * 0.2, 0.0),
        c + vec2(-s * d * 0.12, -d * 0.24),
        c + vec2(-s * d * 0.12, d * 0.24),
        if on {
            Color::from_rgba(150, 90, 230, 255)
        } else {
            PLUM.with_alpha(0.4)
        },
    );
}

fn draw_title(g: &Clash, pen: &mut Pen, view: &View) {
    draw_world(g, pen, view);
    pen.fill_screen(view, Color::new(0.1, 0.05, 0.25, 0.5));
    let l = title_layout(view);
    let bob = (g.t * 2.0).sin() * 5.0;
    pen.ribbon(
        view,
        "Snake Clash",
        vec2(view.w / 2.0, view.h * 0.12 + bob),
        g.t,
    );
    let skin = SKINS[g.skin];
    let open = g.skin_open(g.skin);
    draw_skin_preview(
        pen,
        skin,
        l.preview,
        (view.w.min(view.h) / 760.0).clamp(0.6, 1.1),
        g.t,
    );
    if !open {
        let p = l.preview + vec2(0.0, 30.0);
        pen.round_rect(
            Rect::new(p.x - 130.0, p.y - 56.0, 260.0, 112.0),
            28.0,
            Color::new(1.0, 1.0, 1.0, 0.92),
        );
        lock_icon(pen, p + vec2(-70.0, 0.0), 42.0);
        need_icon(pen, skin.need, p + vec2(0.0, -14.0), 17.0);
        pen.text_outline(
            &skin.need.amount().to_string(),
            p.x + 38.0,
            p.y - 14.0,
            36.0,
            WHITE,
            PLUM,
        );
        let bar = Rect::new(p.x - 20.0, p.y + 16.0, 120.0, 16.0);
        pen.round_rect(bar, 8.0, PLUM.with_alpha(0.25));
        let k = skin.need.progress(&g.stats);
        pen.round_rect(
            Rect::new(
                bar.x + 2.0,
                bar.y + 2.0,
                (bar.w - 4.0) * k.max(0.05),
                bar.h - 4.0,
            ),
            6.0,
            Color::from_rgba(110, 220, 130, 255),
        );
    }
    arrow_button(pen, l.prev, true, true, g.bump.t);
    arrow_button(pen, l.next, false, true, g.bump.t);
    let n = SKINS.len();
    let dots_y = l.preview.y + 150.0;
    for i in 0..n {
        let x = view.w / 2.0 + (i as f32 - (n as f32 - 1.0) / 2.0) * 16.0;
        let on = i == g.skin;
        let col = if g.skin_open(i) {
            WHITE
        } else {
            Color::new(1.0, 1.0, 1.0, 0.35)
        };
        pen.circle(
            vec2(x, dots_y),
            if on { 7.0 } else { 4.5 },
            if on {
                Color::from_rgba(255, 214, 70, 255)
            } else {
                col
            },
        );
    }
    pen.painted_button(l.arena, "Arena", Tone::Green, 0.0, 1.0);
    pen.painted_button(l.survival, "Survival", Tone::Blue, 0.0, 1.0);
    pen.painted_button(l.tower, "Tower", Tone::Orange, 0.0, 1.0);
    let best = g.best.0;
    if best > 0 {
        pen.chip(view, Side::Right, "BEST LV", &best.to_string(), 1.0);
    }
    {
        let r = l.survival;
        let c = vec2(r.x + r.w - 62.0, r.y + r.h * 0.45);
        pen.star(c, 17.0, 0.0, PLUM);
        pen.star(c, 14.0, 0.0, Color::from_rgba(255, 214, 70, 255));
        pen.text_outline(
            &rating::tier(g.stats.rating).to_string(),
            c.x + 32.0,
            c.y,
            30.0,
            WHITE,
            Color::from_rgba(20, 90, 170, 255),
        );
    }
    if g.stats.floor > 0 {
        let r = l.tower;
        pen.text_outline(
            &format!("{}", g.stats.floor),
            r.x + r.w - 38.0,
            r.y + r.h * 0.45,
            34.0,
            WHITE,
            Color::from_rgba(170, 70, 0, 255),
        );
    }
}

fn draw_tower(g: &Clash, pen: &mut Pen, view: &View) {
    draw_world(g, pen, view);
    pen.fill_screen(view, Color::new(0.1, 0.05, 0.25, 0.62));
    let l = tower_layout(view);
    pen.ribbon(view, "Tower", vec2(view.w / 2.0, view.h * 0.12), g.t);
    let f = tower::floor(g.pick_floor);
    let top = g.stats.floor + 1;
    pen.painted_panel(l.card, WHITE);
    let c = l.card.center();
    pen.text_outline(
        &format!("{}", f.n),
        c.x,
        c.y - 70.0,
        110.0 * kg_core::fx::bump_scale(g.bump.t, 0.25),
        WHITE,
        PLUM,
    );
    let boss = tower::is_boss(f.n);
    let ic = vec2(c.x - 50.0, c.y + 50.0);
    match f.goal {
        Goal::Level(_) => pen.star(ic, 34.0, 0.0, Color::from_rgba(255, 214, 70, 255)),
        Goal::Eat(_) => {
            pen.circle(ic, 34.0, PLUM);
            pen.circle(ic, 29.0, Color::from_rgba(230, 100, 120, 255));
            for s in [-1.0, 1.0] {
                pen.circle(ic + vec2(s * 12.0, -5.0), 9.0, WHITE);
                pen.circle(ic + vec2(s * 12.0, -3.0), 4.0, PLUM);
            }
        }
        Goal::Survive(_) => {
            pen.circle(ic, 34.0, PLUM);
            pen.circle(ic, 29.0, WHITE);
            pen.line(ic, ic + vec2(0.0, -20.0), 4.0, PLUM);
            pen.line(ic, ic + vec2(14.0, 0.0), 4.0, PLUM);
        }
        Goal::Boss(_) => pen.sprite_at("profiles", "crown", ic, 80.0, Draw::default()),
    }
    let num = match f.goal {
        Goal::Level(n) | Goal::Eat(n) | Goal::Boss(n) => n.to_string(),
        Goal::Survive(t) => format!("{}s", t as u32),
    };
    pen.text_outline(&num, c.x + 40.0, c.y + 50.0, 64.0, WHITE, PLUM);
    if boss {
        let s = SKINS[f.boss_skin];
        draw_skin_preview(pen, s, vec2(c.x, c.y + 150.0), 0.28, g.t);
    }
    arrow_button(pen, l.prev, true, g.pick_floor > 1, g.bump.t);
    arrow_button(pen, l.next, false, g.pick_floor < top, g.bump.t);
    let n = top.min(12) as usize;
    let start = (g.pick_floor as i32 - 6).clamp(1, (top as i32 - 11).max(1)) as u32;
    for i in 0..n as u32 {
        let fl = start + i;
        let x = view.w / 2.0 + (i as f32 - (n as f32 - 1.0) / 2.0) * 22.0;
        let y = l.card.y + l.card.h + 28.0;
        let on = fl == g.pick_floor;
        let done = fl <= g.stats.floor;
        let col = if on {
            Color::from_rgba(255, 214, 70, 255)
        } else if done {
            Color::from_rgba(110, 220, 130, 255)
        } else {
            WHITE.with_alpha(0.5)
        };
        pen.circle(vec2(x, y), if on { 9.0 } else { 6.0 }, col);
    }
    pen.painted_button(l.play, "Play", Tone::Green, 0.0, 1.0);
    pen.painted_round(l.back, "icon_home", 0.0);
}

fn draw_over(g: &Clash, pen: &mut Pen, view: &View, age: f32) {
    pen.offset = Vec2::ZERO;
    let (_, menu) = over_buttons(view);
    if !g.shows_results() {
        return;
    }
    let o = &g.outcome;
    let (title, line, action) = match (g.mode, o.cleared) {
        (Mode::Tower, true) => ("Floor clear!", format!("Floor {}", o.floor), "Next floor"),
        (Mode::Tower, false) => ("Munched!", format!("Floor {}", o.floor), "Try again"),
        (Mode::Survival, true) => ("You win!", rank_line(o), "Play again"),
        (Mode::Survival, false) => (
            "Munched!",
            format!("{} left, {}", o.left, rank_line(o)),
            "Try again",
        ),
        (Mode::Arena, _) => ("Munched!", format!("Level {}", o.level), "Play again"),
    };
    pen.results(
        view,
        &Results {
            game: GAME,
            title,
            line: &line,
            rank: g.rank,
            action,
            format: plain_score,
        },
        age,
    );
    if !o.new_skins.is_empty() {
        let k = ease_out_back(((age - 0.5) / 0.4).clamp(0.0, 1.0));
        let y = 64.0 + (1.0 - k) * -40.0;
        let w = 280.0 * k;
        let r = Rect::new(view.w / 2.0 - w / 2.0, y, w, 130.0 * k);
        pen.painted_panel(r, WHITE);
        let s = SKINS[o.new_skins[0]];
        draw_skin_preview(pen, s, r.center() + vec2(0.0, 6.0), 0.3 * k, g.t);
        pen.text_outline(
            "NEW!",
            r.x + 50.0,
            r.y + 22.0,
            28.0 * k,
            Color::from_rgba(255, 214, 70, 255),
            PLUM,
        );
    }
    pen.painted_round(menu, "icon_home", 0.0);
}

fn rank_line(o: &kg_core::games::clash::Outcome) -> String {
    let (was, now) = o.rating;
    let (a, b) = (rating::tier(was), rating::tier(now));
    if b > a {
        format!("Rank up! Rank {b}")
    } else {
        format!("Rank {b}")
    }
}

pub fn draw(g: &Clash, pen: &mut Pen, view: &View, results_age: f32) {
    match g.screen {
        Screen::Title => draw_title(g, pen, view),
        Screen::Tower => draw_tower(g, pen, view),
        Screen::Playing => draw_play(g, pen, view),
        Screen::Over => {
            draw_play(g, pen, view);
            draw_over(g, pen, view, results_age);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paint::Tex;

    fn fresh() -> ClashPlay {
        kg_core::save::reset_for_test();
        kg_core::profile::create("Mia", 0);
        ClashPlay::new()
    }

    fn drawn(p: &ClashPlay, v: &View) -> Pen {
        let mut pen = Pen::default();
        pen.begin(*v, 1.0);
        p.draw(&mut pen, v);
        pen
    }

    #[test]
    fn every_screen_draws_art_text_and_the_ui_kit() {
        let v = View::ipad_landscape();
        let mut p = fresh();
        for screen in [Screen::Title, Screen::Tower, Screen::Playing] {
            p.g.screen = screen;
            let pen = drawn(&p, &v);
            let texs: Vec<_> = pen.used().iter().filter_map(|b| b.tex).collect();
            assert!(texs.contains(&Tex::Glyphs), "{screen:?}");
            assert!(pen.vertex_count() > 800, "{screen:?}");
        }
    }

    #[test]
    fn a_shielded_snake_stays_opaque_so_its_segments_never_show_through() {
        let v = View::ipad_landscape();
        let mut p = fresh();
        p.g.start(Mode::Arena, 1);
        for frame in 0..8 {
            p.g.t = frame as f32 / 14.0;
            let s = p.g.world.player().clone();
            assert!(s.shield > 0.0);
            let mut pen = Pen::default();
            pen.begin(v, 1.0);
            draw_snake(&p.g, &mut pen, &v, &s, &Look { mark: None });
            let see_through = pen
                .used()
                .iter()
                .flat_map(|b| b.color.iter())
                .filter(|c| c[3] > 0.15 && c[3] < 0.69)
                .count();
            assert_eq!(see_through, 0, "frame {frame}");
        }
    }

    #[test]
    fn portrait_and_landscape_both_draw_a_running_arena() {
        for v in [View::ipad_portrait(), View::ipad_landscape()] {
            let mut p = fresh();
            p.update(
                &Input::press_at(title_layout(&v).arena.center()),
                &v,
                1.0 / 60.0,
            );
            for _ in 0..120 {
                p.update(&Input::default(), &v, 1.0 / 60.0);
            }
            let pen = drawn(&p, &v);
            assert!(
                p.g.screen == Screen::Playing && pen.vertex_count() > 1500,
                "{} {:?}",
                pen.vertex_count(),
                p.g.screen
            );
        }
    }
}
