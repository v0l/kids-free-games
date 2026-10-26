use super::SHEET;
use crate::draw::Draw;
use crate::paint::Pen;
use glam::{vec2, vec3, Vec2};
use kg_core::engine::View;
use kg_core::fx::{ease_out_back, ease_out_cubic};
use kg_core::games::hotel::layout::*;
use kg_core::games::hotel::sim::*;
use kg_core::games::hotel::*;
use kg_core::geom::{Color, Rect, WHITE};

pub const GOLD: Color = Color::new(1.0, 0.82, 0.24, 1.0);
pub const PLUM: Color = Color::new(0.24, 0.12, 0.3, 1.0);
pub const ARROW: Color = Color::from_rgba(255, 205, 40, 255);

fn rgb(r: u8, g: u8, b: u8) -> Color {
    Color::from_rgba(r, g, b, 255)
}

pub struct Hud<'a> {
    pub h: &'a Hotel,
    pub cam: Cam,
    pub view: View,
}

fn icon(pen: &mut Pen, name: &str, c: Vec2, box_size: f32, d: Draw) {
    let sz = crate::art::size(SHEET, name);
    let fit = box_size / sz.x.max(sz.y);
    pen.sprite(
        SHEET,
        name,
        Rect::new(c.x - sz.x * fit / 2.0, c.y - sz.y * fit / 2.0, sz.x * fit, sz.y * fit),
        d,
    );
}

pub fn ring_arc(pen: &mut Pen, c: Vec2, r: f32, w: f32, k: f32, col: Color) {
    let n = 36;
    let end = (n as f32 * k.clamp(0.0, 1.0)).ceil() as usize;
    for i in 0..end {
        let a0 = -std::f32::consts::FRAC_PI_2 + i as f32 / n as f32 * std::f32::consts::TAU;
        let a1 = -std::f32::consts::FRAC_PI_2 + (i + 1) as f32 / n as f32 * std::f32::consts::TAU;
        let (o0, o1) = (vec2(a0.cos(), a0.sin()), vec2(a1.cos(), a1.sin()));
        pen.tri(c + o0 * (r - w), c + o0 * r, c + o1 * r, col);
        pen.tri(c + o0 * (r - w), c + o1 * r, c + o1 * (r - w), col);
    }
}

pub fn patience_color(k: f32) -> Color {
    if k > 0.5 {
        rgb(90, 210, 70)
    } else if k > 0.25 {
        rgb(255, 200, 40)
    } else {
        rgb(240, 70, 60)
    }
}

pub fn bubble(pen: &mut Pen, c: Vec2, r: f32, name: &str, k: Option<f32>) {
    pen.circle(c + vec2(0.0, 3.0), r, Color::new(0.1, 0.05, 0.2, 0.25));
    pen.circle(c, r, WHITE);
    pen.tri(
        c + vec2(-r * 0.35, r * 0.75),
        c + vec2(r * 0.35, r * 0.75),
        c + vec2(0.0, r * 1.3),
        WHITE,
    );
    icon(pen, name, c, r * 1.45, Draw::default());
    if let Some(k) = k {
        ring_arc(pen, c, r + 4.0, 5.0, k, patience_color(k));
    }
}

impl Hud<'_> {
    fn at(&self, p: glam::Vec3) -> Option<Vec2> {
        self.cam.project(&self.view, p)
    }

    pub fn bubble_radius(&self) -> f32 {
        let p = self.h.sim.player.pos;
        (self.cam.px_per_unit(&self.view, vec3(p.x, 0.0, p.y)) * 0.3).clamp(16.0, 34.0)
    }

    pub fn draw_overhead(&self, pen: &mut Pen) {
        let h = self.h;
        let sim = &h.sim;
        let r = self.bubble_radius();
        for g in &sim.guests {
            let Some(name) = want_icon(g.stage) else {
                continue;
            };
            let mut at = g.pos;
            if let Stage::Sleep(k, _) = g.stage {
                at = room_bed(k);
            }
            if let Stage::WaitTowel(b) = g.stage {
                at = sim.lay.sunbeds[b] + vec2(0.0, 0.7);
            }
            let waiting = matches!(
                g.stage,
                Stage::Queue | Stage::WaitStall | Stage::WaitSnack(_) | Stage::WaitTowel(_)
            );
            let Some(c) = self.at(vec3(at.x, 2.1, at.y)) else {
                continue;
            };
            let bob = (h.t * 3.0 + g.id as f32).sin() * 3.0;
            bubble(pen, c + vec2(0.0, bob), r * 0.8, name, waiting.then(|| g.patience / g.max));
        }
        for (k, room) in sim.rooms.iter().enumerate() {
            if !room.is_dirty() {
                continue;
            }
            for (i, sp) in sim.spots(k).iter().enumerate() {
                if !room.dirty[i] {
                    continue;
                }
                let Some(c) = self.at(vec3(sp.obj.x, 1.6, sp.obj.y)) else {
                    continue;
                };
                let bob = (h.t * 4.0 + i as f32).sin() * 3.0;
                let work = room.spot_work[i];
                bubble(pen, c + vec2(0.0, bob), r * 0.6, "broom", (work > 0.0).then_some(1.0 - work));
            }
        }
        for (k, st) in sim.stalls.iter().enumerate() {
            if st.paper == 0 && sim.has(Build::Toilets) {
                let p = sim.lay.stalls[k];
                if let Some(c) = self.at(vec3(p.x, 1.9, p.y)) {
                    bubble(pen, c, r * 0.7, "paper", None);
                }
            }
        }
        if sim.front_ready().is_some() && sim.at_desk() && sim.free_room().is_some() {
            if let Some(c) = self.at(vec3(0.0, 2.4, 1.3)) {
                ring_arc(pen, c, r, 7.0, 1.0, Color::new(1.0, 1.0, 1.0, 0.5));
                ring_arc(pen, c, r, 7.0, sim.desk, rgb(120, 230, 90));
            }
        }
        if sim.front_ready().is_some() && sim.free_room().is_none() {
            if let Some(c) = self.at(vec3(0.0, 2.4, 2.6)) {
                bubble(pen, c, r * 0.7, "broom", None);
            }
        }
        self.draw_pad_labels(pen, r);
        for w in sim.locked() {
            let mid = w.center();
            if let Some(c) = self.at(vec3(mid.x, 1.6, mid.y)) {
                let d = (r * 1.1).max(22.0);
                pen.sprite("ui", "icon_lock", Rect::new(c.x - d / 2.0, c.y - d / 2.0, d, d), Draw::tint(PLUM));
            }
        }
        if let Some(task) = h.guide() {
            let to = sim.task_pos(task);
            if let Some(c) = self.at(vec3(to.x, 2.6, to.y)) {
                guide_arrow(pen, c + vec2(0.0, -(h.t * 6.0).sin().abs() * 10.0), r * 0.9);
            }
        }
        self.draw_pops(pen);
    }

    fn draw_pad_labels(&self, pen: &mut Pen, r: f32) {
        let sim = &self.h.sim;
        let px = self.cam.px_per_unit(&self.view, vec3(sim.player.pos.x, 0.0, sim.player.pos.y));
        let _ = r;
        for i in sim.visible_pads() {
            let p = &sim.lay.pads[i];
            let Some(c) = self.at(vec3(p.pos.x, 0.0, p.pos.y)) else {
                continue;
            };
            let size = (px * 0.5).clamp(24.0, 52.0);
            icon(pen, build_icon(p.build), c + vec2(0.0, -size * 0.3), size, Draw::default());
            let left = p.cost - sim.paid[i];
            let col = if sim.wallet >= left { GOLD } else { WHITE };
            pen.text_outline(&format!("${left}"), c.x, c.y + size * 0.4, (size * 0.5).round().max(20.0), col, PLUM);
            if (sim.player.pos - p.pos).length() < 2.2 {
                pen.text_outline(&build_label(p.build), c.x, c.y - size * 1.05, 20.0, WHITE, PLUM);
            }
        }
    }

    fn draw_pops(&self, pen: &mut Pen) {
        for pop in &self.h.pops {
            let Some(c) = self.at(pop.at + vec3(0.0, pop.age * 1.2, 0.0)) else {
                continue;
            };
            let a = 1.0 - ((pop.age - 0.7) / 0.4).clamp(0.0, 1.0);
            let k = ease_out_back((pop.age / 0.25).min(1.0)).max(0.1);
            let d = Draw::tint(WHITE.with_alpha(a));
            match pop.kind {
                PopKind::Cash(n) => pen.text_outline(&format!("+{n}"), c.x, c.y, (36.0 * k).round(), GOLD.with_alpha(a), PLUM.with_alpha(a)),
                PopKind::Heart => pen.sprite_at(SHEET, "heart", c, 44.0 * k, d),
                PopKind::Angry => pen.text_outline("!", c.x, c.y, (56.0 * k).round(), rgb(240, 80, 70).with_alpha(a), PLUM.with_alpha(a)),
                PopKind::Sparkle => pen.sprite_at("ui", "p_star", c, 48.0 * k, d),
                PopKind::Money => pen.sprite_at(SHEET, "cash", c, 40.0 * k, d),
            }
        }
    }

    pub fn draw_flyers(&self, pen: &mut Pen) {
        let view = &self.view;
        for f in &self.h.flyers {
            if !f.started() {
                continue;
            }
            let k = f.k();
            let to = if f.xp { Hotel::xp_target(view) } else { Hotel::coin_target(view) };
            let Some(from) = self.at(f.from) else {
                continue;
            };
            let p = fly_path(from, to, k);
            if f.xp {
                pen.sprite_at("ui", "p_star", p, 26.0 - 6.0 * k, Draw::default());
            } else {
                pen.sprite_at("ui", "p_coin", p, 28.0 - 8.0 * k, Draw::default());
            }
        }
    }

    pub fn draw_hud(&self, pen: &mut Pen) {
        let h = self.h;
        let view = &self.view;
        let w = Hotel::wallet_rect(view);
        pen.chip_box(w);
        let k = h.wallet_bump.scale(0.25);
        pen.sprite_at("ui", "p_coin", vec2(w.x + 30.0, w.y + w.h / 2.0), 42.0 * k, Draw::default());
        pen.text_outline(&format!("{}", h.shown_wallet.round() as u32), w.x + 108.0, w.y + w.h / 2.0, (34.0 * k).round(), GOLD, PLUM);
        let bar = Hotel::level_rect(view);
        let maxed = h.sim.level >= h.sim.lay.max_level;
        pen.round_rect(Rect::new(bar.x - 3.0, bar.y - 3.0, bar.w + 6.0, bar.h + 6.0), 15.0, Color::new(0.1, 0.05, 0.2, 0.6));
        let fill = if maxed { 1.0 } else { h.shown_xp.clamp(0.0, 1.0) };
        if fill > 0.0 {
            pen.round_rect(Rect::new(bar.x, bar.y, (bar.w * fill).max(bar.h), bar.h), 13.0, rgb(90, 160, 255));
        }
        let label = if maxed { "MAX".to_string() } else { format!("{}/{}", h.sim.xp, h.sim.xp_needed()) };
        pen.text_outline(&label, bar.center().x + 14.0, bar.center().y, 20.0, WHITE, PLUM);
        let kx = h.xp_bump.scale(0.3);
        let star = Hotel::xp_target(view);
        pen.sprite_at("ui", "p_star", star, 52.0 * kx, Draw::default());
        pen.text_outline(&format!("{}", h.sim.level), star.x, star.y + 2.0, (24.0 * kx).round(), WHITE, PLUM);
        pen.text_outline(h.sim.lay.theme.name, view.w / 2.0, bar.y + bar.h + 22.0, 22.0, WHITE, PLUM);
        if let Some(next) = h.sim.next_locked_level() {
            if h.sim.visible_pads().is_empty() {
                pen.text_outline(&format!("Level {next} opens more"), view.w / 2.0, bar.y + bar.h + 48.0, 20.0, rgb(200, 230, 255), PLUM);
            }
        }
        if let Some(item) = h.sim.player.carry {
            let r = Hotel::carry_rect(view);
            pen.chip_box(r);
            icon(pen, item_icon(item), vec2(r.x + 32.0, r.center().y), 40.0, Draw::default());
            pen.text_outline(&format!("{}/{}", h.sim.player.count, h.sim.capacity()), r.x + 96.0, r.center().y, 28.0, WHITE, PLUM);
        }
        if !h.learned_move && h.mode == Mode::Play {
            let pulse = 1.0 + (h.t * 4.0).sin() * 0.05;
            pen.hint("Drag to walk", vec2(view.w / 2.0, view.h - 70.0), pulse);
        }
    }

    pub fn draw_stick(&self, pen: &mut Pen, mm: f32) {
        let s = &self.h.stick;
        if !s.active || self.h.mode != Mode::Play {
            return;
        }
        let r = Stick::radius_px(mm).max(30.0);
        pen.circle(s.origin, r, Color::new(1.0, 1.0, 1.0, 0.18));
        let k = s.origin + s.knob.clamp_length_max(r);
        pen.circle(k, r * 0.45, Color::new(1.0, 1.0, 1.0, 0.55));
    }

    pub fn draw_overlay(&self, pen: &mut Pen) {
        let h = self.h;
        let view = &self.view;
        match h.mode {
            Mode::LevelUp(level, t) => {
                let a = (t / 0.25).min(1.0) * (1.0 - ((t - 1.8) / 0.4).clamp(0.0, 1.0));
                pen.fill_screen(view, Color::new(0.1, 0.04, 0.25, 0.35 * a));
                let k = ease_out_back((t / 0.4).min(1.0)).max(0.01);
                let c = vec2(view.w / 2.0, view.h * 0.42);
                pen.sprite_at("ui", "p_star", c, 150.0 * k, Draw::tint(WHITE.with_alpha(a)));
                pen.text_outline(&format!("{level}"), c.x, c.y + 6.0, (64.0 * k).round(), WHITE.with_alpha(a), PLUM.with_alpha(a));
                pen.text_outline("Level up!", c.x, c.y + 110.0, 46.0, GOLD.with_alpha(a), PLUM.with_alpha(a));
            }
            Mode::Moving(t) => {
                let a = (t / 0.4).min(1.0);
                pen.fill_screen(view, Color::new(0.1, 0.04, 0.25, 0.55 * a));
                pen.ribbon(view, "Hotel done!", vec2(view.w / 2.0, view.h * 0.32), t);
                for s in 0..3 {
                    let k = ease_out_back(((t - 0.3 - s as f32 * 0.2) / 0.4).clamp(0.0, 1.0));
                    pen.sprite_at("ui", "p_star", vec2(view.w / 2.0 + (s as f32 - 1.0) * 90.0, view.h * 0.5), 84.0 * k.max(0.01), Draw::default());
                }
                pen.text_outline(&format!("Next: {}", h.next_theme().name), view.w / 2.0, view.h * 0.66, 40.0, WHITE, PLUM);
            }
            Mode::Done(t) if t < 4.0 => {
                let a = 1.0 - (t - 3.0).max(0.0);
                pen.fill_screen(view, Color::new(0.1, 0.04, 0.25, 0.45 * a));
                pen.ribbon(view, "Hotel boss!", vec2(view.w / 2.0, view.h * 0.4), t);
            }
            _ => {}
        }
    }
}

pub fn guide_arrow(pen: &mut Pen, p: Vec2, size: f32) {
    pen.tri(
        p + vec2(-size * 0.6, -size * 0.6),
        p + vec2(size * 0.6, -size * 0.6),
        p + vec2(0.0, size * 0.3),
        ARROW,
    );
    pen.rect(Rect::new(p.x - size * 0.25, p.y - size * 1.3, size * 0.5, size * 0.75), ARROW);
}

pub fn eased(k: f32) -> f32 {
    ease_out_cubic(k)
}
