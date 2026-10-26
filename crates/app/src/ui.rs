use crate::draw::{measure, Draw, PLUM};
use crate::paint::Pen;
use glam::{vec2, Vec2};
use kg_core::engine::View;
use kg_core::fx::{ease_out_back, ease_out_cubic, Particle, Particles, Popups, Shape, Side};
use kg_core::geom::{Color, Rect, WHITE};

pub const INK: Color = Color::new(0.16, 0.12, 0.24, 1.0);
pub const BODY: Color = Color::new(0.33, 0.30, 0.42, 1.0);
pub const GOLD_TEXT: Color = Color::from_rgba(255, 200, 40, 255);

pub const SPRITES: [&str; 22] = [
    "p_panel",
    "p_btn_green",
    "p_btn_orange",
    "p_btn_blue",
    "p_ribbon",
    "p_round",
    "p_coin",
    "p_star",
    "p_medal",
    "icon_home",
    "icon_music",
    "icon_music_off",
    "icon_audio",
    "icon_audio_off",
    "icon_gear",
    "medal_gold",
    "medal_silver",
    "medal_bronze",
    "star_yellow",
    "star_white",
    "puff",
    "cloud1",
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tone {
    Green,
    Orange,
    Blue,
}

impl Tone {
    fn sprite(self) -> &'static str {
        match self {
            Tone::Green => "p_btn_green",
            Tone::Orange => "p_btn_orange",
            Tone::Blue => "p_btn_blue",
        }
    }

    fn ink(self) -> Color {
        match self {
            Tone::Orange => Color::from_rgba(170, 70, 0, 255),
            Tone::Blue => Color::from_rgba(10, 80, 150, 255),
            Tone::Green => Color::from_rgba(40, 120, 20, 255),
        }
    }
}

pub fn medal_for(score: u32, best: u32) -> Option<&'static str> {
    if best == 0 || score == 0 {
        return None;
    }
    let f = score as f32 / best as f32;
    Some(if f >= 1.0 {
        "medal_gold"
    } else if f >= 0.6 {
        "medal_silver"
    } else {
        "medal_bronze"
    })
}

pub const CHIP_Y: f32 = 12.0;
pub const CHIP_H: f32 = 48.0;
const CHIP_LABEL: f32 = 20.0;
const CHIP_VALUE: f32 = 34.0;
const CHIP_PAD: f32 = 16.0;

pub struct Results<'a> {
    pub game: &'a str,
    pub title: &'a str,
    pub line: &'a str,
    pub rank: Option<usize>,
    pub action: &'a str,
    pub format: fn(i64) -> String,
}

pub fn plain_score(s: i64) -> String {
    s.to_string()
}

pub const RESULT_ROWS: usize = 5;

pub fn results_rows(board_len: usize, rank: Option<usize>) -> Vec<usize> {
    let mut rows: Vec<usize> = (0..board_len.min(RESULT_ROWS)).collect();
    if let Some(r) = rank.filter(|r| *r >= RESULT_ROWS) {
        rows.pop();
        rows.push(r);
    }
    rows
}

pub struct ShownRow {
    pub place: usize,
    pub name: String,
    pub avatar: usize,
    pub score: i64,
    pub mine: bool,
}

pub fn shown_rows(game: &str, rank: Option<usize>) -> Vec<ShownRow> {
    if let Some(online) = kg_core::net::online_board(game) {
        return online
            .into_iter()
            .map(|r| ShownRow {
                place: r.place,
                name: r.name,
                avatar: r.avatar,
                score: r.score,
                mine: r.mine,
            })
            .collect();
    }
    let board = kg_core::profile::board(game);
    results_rows(board.len(), rank)
        .into_iter()
        .map(|i| ShownRow {
            place: i + 1,
            name: kg_core::profile::player_name(&board[i].player),
            avatar: kg_core::profile::player_avatar(&board[i].player),
            score: board[i].score,
            mine: Some(i) == rank,
        })
        .collect()
}

pub fn chip_rect(view: &View, side: Side, label: &str, value: &str) -> Rect {
    kg_core::fx::chip_rect(view, side, label, value)
}

impl Pen {
    pub fn ui(&mut self, name: &str, dest: Rect, d: Draw) {
        self.sprite("ui", name, dest, d);
    }

    pub fn painted_panel(&mut self, r: Rect, tint: Color) {
        let k = (r.w.min(r.h) / 400.0).clamp(0.3, 2.0);
        self.nine("ui", "p_panel", r, 70.0, k, tint);
    }

    pub fn painted_button(&mut self, r: Rect, label: &str, tone: Tone, press: f32, alpha: f32) {
        let s = 1.0 - press * 0.06;
        let r = Rect::new(
            r.center().x - r.w * s / 2.0,
            r.center().y - r.h * s / 2.0 + press * 3.0,
            r.w * s,
            r.h * s,
        );
        self.nine(
            "ui",
            tone.sprite(),
            r,
            60.0,
            r.h / 150.0,
            WHITE.with_alpha(alpha),
        );
        self.text_outline(
            label,
            r.center().x,
            r.y + r.h * 0.45,
            r.h * 0.46,
            WHITE.with_alpha(alpha),
            tone.ink().with_alpha(alpha),
        );
    }

    pub fn painted_round(&mut self, r: Rect, icon: &str, press: f32) {
        let s = 1.0 - press * 0.08;
        let c = r.center() + vec2(0.0, press * 2.0);
        let d = r.h.min(r.w) * s;
        self.ui(
            "p_round",
            Rect::new(c.x - d / 2.0, c.y - d / 2.0, d, d),
            Draw::default(),
        );
        let ic = d * 0.48;
        self.ui(
            icon,
            Rect::new(c.x - ic / 2.0, c.y - ic / 2.0 - d * 0.03, ic, ic),
            Draw::tint(Color::from_rgba(150, 70, 0, 255)),
        );
        self.ui(
            icon,
            Rect::new(c.x - ic / 2.0, c.y - ic / 2.0 - d * 0.06, ic, ic),
            Draw::default(),
        );
    }

    pub fn ribbon(&mut self, view: &View, title: &str, c: Vec2, t: f32) {
        let size = 54.0;
        let tw = measure(title, size);
        let w = (tw + 260.0).min(view.w - 20.0);
        let rs = crate::art::size("ui", "p_ribbon");
        let h = w * rs.y / rs.x;
        let wob = (t * 2.5).sin() * 0.015;
        self.ui(
            "p_ribbon",
            Rect::new(c.x - w / 2.0, c.y - h / 2.0, w, h),
            Draw::rot(wob),
        );
        self.text_outline(
            title,
            c.x,
            c.y - h * 0.08,
            size.min(w * 0.7 / tw * size),
            WHITE,
            Color::from_rgba(140, 20, 30, 255),
        );
    }

    pub fn hint(&mut self, label: &str, c: Vec2, pulse: f32) {
        let size = 32.0 * pulse;
        let w = measure(label, size) + 70.0;
        let r = Rect::new(c.x - w / 2.0, c.y - 32.0 * pulse, w, 64.0 * pulse);
        self.round_rect(
            r.offset(vec2(0.0, 4.0)),
            r.h / 2.0,
            Color::new(0.0, 0.0, 0.0, 0.18),
        );
        self.round_rect(r, r.h / 2.0, Color::new(1.0, 1.0, 1.0, 0.92));
        self.text_center(label, c.x, c.y, size, BODY);
    }

    pub fn chip_box(&mut self, r: Rect) {
        self.round_rect(
            r.offset(vec2(0.0, 4.0)),
            r.h / 2.0,
            Color::new(0.05, 0.02, 0.15, 0.3),
        );
        self.round_rect(r, r.h / 2.0, Color::new(0.2, 0.08, 0.35, 0.72));
        self.round_rect(
            Rect::new(r.x + 3.0, r.y + 3.0, r.w - 6.0, r.h * 0.45),
            r.h / 2.0,
            Color::new(1.0, 1.0, 1.0, 0.1),
        );
    }

    pub fn chip(&mut self, view: &View, side: Side, label: &str, value: &str, bump: f32) -> Rect {
        let r = chip_rect(view, side, label, value);
        let k = 1.0 + 0.18 * (1.0 - ease_out_cubic(bump));
        self.chip_box(r);
        let mut x = r.x + CHIP_PAD;
        if !label.is_empty() {
            let lw = measure(label, CHIP_LABEL);
            self.text_center(
                label,
                x + lw / 2.0,
                r.y + r.h / 2.0 + 1.0,
                CHIP_LABEL,
                Color::new(1.0, 1.0, 1.0, 0.75),
            );
            x += lw + 10.0;
        }
        let vw = measure(value, CHIP_VALUE);
        self.text_outline(
            value,
            x + vw / 2.0,
            r.y + r.h / 2.0,
            CHIP_VALUE * k,
            GOLD_TEXT,
            INK,
        );
        r
    }

    pub fn icon(&mut self, game: &str, r: Rect) {
        self.sprite("icons", &format!("icon_{game}"), r, Draw::default());
    }

    pub fn spark(&mut self, c: Vec2, size: f32, angle: f32, color: Color) {
        let gold = color.r > 0.8 && color.g > 0.6 && color.b < 0.5;
        let (name, tint) = if gold {
            ("star_yellow", WHITE.with_alpha(color.a))
        } else {
            ("star_white", color)
        };
        self.ui(
            name,
            Rect::new(c.x - size, c.y - size, size * 2.0, size * 2.0),
            Draw {
                tint,
                rotation: angle,
                ..Default::default()
            },
        );
    }

    pub fn puff(&mut self, c: Vec2, size: f32, color: Color) {
        self.ui(
            "puff",
            Rect::new(c.x - size, c.y - size, size * 2.0, size * 2.0),
            Draw::tint(color),
        );
    }

    pub fn particle(&mut self, p: &Particle) {
        let (a, s) = kg_core::fx::particle_look(p);
        let c = p.color.with_alpha(a);
        match p.shape {
            Shape::Circle if s > 5.0 && c.r > 0.9 && c.g > 0.9 && c.b > 0.9 => {
                self.puff(p.pos, s * 1.3, c)
            }
            Shape::Circle => self.circle(p.pos, s, c),
            Shape::Square => {
                let r = Rect::new(p.pos.x - s, p.pos.y - s, s * 2.0, s * 2.0);
                self.rect(r, c)
            }
            Shape::Star => self.spark(p.pos, s * 1.5, p.angle, c),
            Shape::Confetti => {
                let w = s * (p.angle * 2.0).cos().abs().max(0.15) * 1.6;
                let (sn, cs) = (p.angle * 0.3).sin_cos();
                let rot = |q: Vec2| p.pos + vec2(q.x * cs - q.y * sn, q.x * sn + q.y * cs);
                let (hw, hh) = (w / 2.0, s / 2.0);
                let h = Vec2::splat(0.5);
                self.quad(
                    crate::paint::Tex::White,
                    [
                        rot(vec2(-hw, -hh)),
                        rot(vec2(hw, -hh)),
                        rot(vec2(hw, hh)),
                        rot(vec2(-hw, hh)),
                    ],
                    [h; 4],
                    c,
                );
            }
        }
    }

    pub fn particles(&mut self, fx: &Particles) {
        for p in &fx.list {
            self.particle(p);
        }
    }

    pub fn popups(&mut self, pops: &Popups) {
        for p in &pops.list {
            let (pop, fade) = kg_core::fx::popup_scale(p);
            self.text_outline(
                &p.text,
                p.pos.x,
                p.pos.y,
                p.size * pop,
                p.color.with_alpha(fade),
                PLUM.with_alpha(fade),
            );
        }
    }

    pub fn results(&mut self, view: &View, r: &Results, age: f32) {
        let board = shown_rows(r.game, r.rank);
        let fade = ease_out_cubic(age / 0.2);
        self.fill_screen(view, Color::new(0.06, 0.04, 0.16, 0.55 * fade));
        let row_h = 46.0;
        let pw = 580.0f32.min(view.w - 30.0);
        let ph = 230.0 + board.len() as f32 * row_h + if board.is_empty() { 0.0 } else { 20.0 };
        let s = ease_out_back(age / 0.32).max(0.01);
        let c = view.center() + vec2(0.0, 24.0 + (1.0 - ease_out_cubic(age / 0.32)) * 90.0);
        let p = Rect::new(c.x - pw * s / 2.0, c.y - ph * s / 2.0, pw * s, ph * s);
        self.painted_panel(p, WHITE);
        self.ribbon(view, r.title, vec2(c.x, p.y + 8.0 * s), age);
        let a = |d: f32| ((age - d) / 0.2).clamp(0.0, 1.0);
        self.text_center(
            r.line,
            c.x,
            p.y + 92.0 * s,
            34.0 * s,
            BODY.with_alpha(a(0.12)),
        );
        if r.rank == Some(0) {
            let k = ease_out_back(((age - 0.35) / 0.3).clamp(0.0, 1.0));
            if k > 0.0 {
                let wob = (age * 6.0).sin() * 0.06;
                self.trophy(vec2(p.x + 46.0 * s, p.y + 70.0 * s), 92.0 * k);
                self.ui(
                    "p_star",
                    Rect::new(p.x + p.w - 90.0 * s, p.y + 30.0 * s, 70.0 * k, 70.0 * k),
                    Draw::rot(wob),
                );
                self.text_outline(
                    "New best!",
                    c.x,
                    p.y + 132.0 * s,
                    30.0 * s * (1.0 + wob * 0.5),
                    GOLD_TEXT,
                    Color::from_rgba(130, 50, 0, 255),
                );
            }
        }
        let top = p.y + 160.0 * s;
        for (i, e) in board.iter().enumerate() {
            let y = top + i as f32 * row_h * s;
            let la = a(0.2 + i as f32 * 0.05);
            let row = Rect::new(p.x + 40.0 * s, y, p.w - 80.0 * s, (row_h - 6.0) * s);
            if e.mine {
                self.round_rect(
                    row,
                    row.h / 2.0,
                    Color::from_rgba(255, 214, 90, 255).with_alpha(0.85 * la),
                );
            } else if i % 2 == 0 {
                self.round_rect(row, row.h / 2.0, Color::new(0.4, 0.3, 0.6, 0.08 * la));
            }
            let cy = row.center().y;
            self.text_center(
                &e.place.to_string(),
                row.x + 26.0 * s,
                cy,
                26.0 * s,
                BODY.with_alpha(la),
            );
            if e.place == 1 {
                self.crown(vec2(row.x + 26.0 * s, cy - 24.0 * s), 26.0 * s);
            }
            self.avatar(e.avatar, vec2(row.x + 72.0 * s, cy), row.h * 0.95);
            let nw = measure(&e.name, 26.0) * s;
            self.text_center(
                &e.name,
                row.x + 104.0 * s + nw / 2.0,
                cy,
                26.0 * s,
                BODY.with_alpha(la),
            );
            let sc = (r.format)(e.score);
            let sw = measure(&sc, 30.0) * s;
            self.text_outline(
                &sc,
                row.x + row.w - 18.0 * s - sw / 2.0,
                cy,
                30.0 * s,
                WHITE.with_alpha(la),
                Color::from_rgba(70, 30, 100, 255).with_alpha(la),
            );
        }
        let by = p.y + p.h - 46.0 * s;
        let pulse = 1.0 + (age * 4.5).sin() * 0.03;
        let bw = (measure(r.action, 30.0) + 110.0) * s * pulse;
        self.painted_button(
            Rect::new(c.x - bw / 2.0, by - 10.0 * s, bw, 78.0 * s * pulse),
            r.action,
            Tone::Green,
            0.0,
            a(0.3),
        );
    }

    pub fn panel(&mut self, view: &View, title: &str, lines: &[&str], age: f32) {
        let fade = ease_out_cubic(age / 0.2);
        self.fill_screen(view, Color::new(0.06, 0.04, 0.16, 0.5 * fade));
        let pw = 560.0f32.min(view.w - 40.0);
        let ph = 150.0 + lines.len() as f32 * 64.0;
        let s = ease_out_back(age / 0.32).max(0.01);
        let c = view.center() + vec2(0.0, 20.0 + (1.0 - ease_out_cubic(age / 0.32)) * 90.0);
        let r = Rect::new(c.x - pw * s / 2.0, c.y - ph * s / 2.0, pw * s, ph * s);
        self.painted_panel(r, WHITE);
        self.ribbon(view, title, vec2(c.x, r.y + 8.0 * s), age);
        for (i, l) in lines.iter().enumerate() {
            let la = ((age - 0.12 - i as f32 * 0.06) / 0.2).clamp(0.0, 1.0);
            let last = i + 1 == lines.len();
            let y = r.y + 96.0 * s + i as f32 * 64.0 * s + (1.0 - la) * 16.0;
            if last && lines.len() > 1 {
                let pulse = 1.0 + (age * 4.5).sin() * 0.03;
                let bw = (measure(l, 30.0) + 110.0) * s * pulse;
                let br = Rect::new(c.x - bw / 2.0, y - 8.0 * s, bw, 78.0 * s * pulse);
                self.painted_button(br, l, Tone::Green, 0.0, la);
            } else {
                self.text_center(l, c.x, y + 14.0 * s, 34.0 * s, BODY.with_alpha(la));
            }
        }
    }

    pub fn avatar(&mut self, i: usize, c: Vec2, d: f32) {
        let avatars = kg_core::profile::AVATARS;
        let name = format!("avatar_{}", avatars[i % avatars.len()]);
        self.round_rect(
            Rect::new(c.x - d / 2.0, c.y - d / 2.0 + d * 0.05, d, d),
            d / 2.0,
            Color::new(0.1, 0.03, 0.25, 0.3),
        );
        self.circle(c, d / 2.0, Color::from_rgba(255, 240, 210, 255));
        let s = crate::art::size("profiles", &name);
        let k = d * 0.78 / s.x.max(s.y);
        self.sprite_at(
            "profiles",
            &name,
            c + vec2(0.0, d * 0.03),
            s.y * k,
            Draw::default(),
        );
        self.sprite_at("profiles", "avatar_frame", c, d * 1.08, Draw::default());
    }

    pub fn crown(&mut self, c: Vec2, h: f32) {
        self.sprite_at("profiles", "crown", c, h, Draw::default());
    }

    pub fn trophy(&mut self, c: Vec2, h: f32) {
        self.sprite_at("profiles", "trophy", c, h, Draw::default());
    }

    pub fn key_cap(&mut self, r: Rect, label: &str, press: f32) {
        let k = 1.0 - press * 0.08;
        let rr = r.scaled_about_center(k);
        self.round_rect(
            rr.offset(vec2(0.0, 5.0)),
            12.0,
            Color::new(0.1, 0.03, 0.25, 0.35),
        );
        self.round_rect(rr, 12.0, Color::new(1.0, 1.0, 1.0, 0.95));
        self.text_center(label, rr.center().x, rr.center().y, rr.h * 0.48, BODY);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn results_show_the_top_five_and_your_row_if_lower() {
        assert_eq!(results_rows(3, Some(1)), vec![0, 1, 2]);
        assert_eq!(results_rows(10, Some(2)), vec![0, 1, 2, 3, 4]);
        assert_eq!(results_rows(10, Some(8)), vec![0, 1, 2, 3, 8]);
        assert_eq!(results_rows(10, None), vec![0, 1, 2, 3, 4]);
        assert!(results_rows(0, None).is_empty());
    }

    #[test]
    fn medals_track_how_close_you_got_to_your_best() {
        assert_eq!(medal_for(0, 10), None);
        assert_eq!(medal_for(10, 10), Some("medal_gold"));
        assert_eq!(medal_for(7, 10), Some("medal_silver"));
        assert_eq!(medal_for(2, 10), Some("medal_bronze"));
    }

    #[test]
    fn the_results_panel_lists_the_local_board_with_your_row() {
        kg_core::save::reset_for_test();
        kg_core::profile::create("Mia", 2);
        kg_core::profile::submit("t_ui", 12, kg_core::profile::Better::Higher);
        let rows = shown_rows("t_ui", Some(0));
        assert_eq!(rows.len(), 1);
        assert!(rows[0].mine && rows[0].name == "Mia" && rows[0].score == 12);
        let mut p = Pen::default();
        p.begin(View::ipad_landscape(), 1.0);
        p.results(
            &View::ipad_landscape(),
            &Results {
                game: "t_ui",
                title: "Bonk!",
                line: "Score 12",
                rank: Some(0),
                action: "Fly again",
                format: plain_score,
            },
            1.0,
        );
        assert!(p.vertex_count() > 200);
    }
}
