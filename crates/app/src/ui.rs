use crate::draw::{measure, Draw, PLUM};
use crate::paint::Pen;
use glam::{vec2, Vec2};
use kg_core::engine::View;
use kg_core::fx::{ease_out_back, ease_out_cubic, Particle, Particles, Popups, Shape, Side};
use kg_core::geom::{Color, Rect, WHITE};

pub const INK: Color = Color::new(0.16, 0.12, 0.24, 1.0);
pub const BODY: Color = Color::new(0.33, 0.30, 0.42, 1.0);
pub const GOLD_TEXT: Color = Color::from_rgba(255, 200, 40, 255);

pub const SPRITES: [&str; 23] = [
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
    "icon_report",
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
    pub player: String,
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
                player: if r.mine { String::new() } else { r.player },
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
            player: String::new(),
            place: i + 1,
            name: kg_core::profile::player_name(&board[i].player),
            avatar: kg_core::profile::player_avatar(&board[i].player),
            score: board[i].score,
            mine: Some(i) == rank,
        })
        .collect()
}

#[derive(Clone, Debug, PartialEq)]
pub struct ReportHit {
    pub rect: Rect,
    pub player: String,
    pub name: String,
    pub game: String,
}

thread_local! {
    static REPORT_HITS: std::cell::RefCell<Vec<ReportHit>> = const { std::cell::RefCell::new(Vec::new()) };
}

pub fn clear_report_hits() {
    REPORT_HITS.with(|h| h.borrow_mut().clear());
}

pub fn report_hit_at(p: Vec2) -> Option<ReportHit> {
    REPORT_HITS.with(|h| h.borrow().iter().find(|r| r.rect.contains(p)).cloned())
}

pub fn chip_rect(view: &View, side: Side, label: &str, value: &str) -> Rect {
    kg_core::fx::chip_rect(view, side, label, value)
}

pub struct Pop {
    c: Vec2,
    s: f32,
    drop: f32,
}

impl Pop {
    pub fn new(c: Vec2, s: f32, drop: f32) -> Self {
        Self { c, s, drop }
    }

    pub fn point(&self, p: Vec2) -> Vec2 {
        self.c + (p - self.c) * self.s + vec2(0.0, self.drop)
    }

    pub fn rect(&self, r: Rect) -> Rect {
        let p = self.point(r.point());
        Rect::new(p.x, p.y, r.w * self.s, r.h * self.s)
    }
}

impl Pen {
    pub fn sheet_text(
        &mut self,
        sheet: &kg_core::sizing::Sheet,
        part: usize,
        replace: Option<&str>,
        col: Color,
    ) {
        let (lines, size, r) = sheet.lines(part);
        let cx = r.center().x;
        if let Some(s) = replace {
            let size = kg_core::sizing::fit_size(s, size, r.w);
            let y = sheet.line_y(part, 0);
            self.text_center(s, cx, y, size, col);
            return;
        }
        for (n, l) in lines.to_vec().iter().enumerate() {
            self.text_center(l, cx, sheet.line_y(part, n), size, col);
        }
    }

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
            kg_core::sizing::label_size(label, r),
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
        self.ribbon_k(view, title, c, t, 1.0);
    }

    pub fn ribbon_k(&mut self, view: &View, title: &str, c: Vec2, t: f32, k: f32) {
        let (r, size) = kg_core::sizing::ribbon_rect(view, title, c.x, c.y, k);
        let wob = (t * 2.5).sin() * 0.015;
        self.ui("p_ribbon", r, Draw::rot(wob));
        self.text_outline(
            title,
            c.x,
            c.y - r.h * 0.08,
            size,
            WHITE,
            Color::from_rgba(140, 20, 30, 255),
        );
    }

    pub fn ribbon_in(&mut self, view: &View, title: &str, r: Rect, t: f32) {
        let size = kg_core::sizing::ribbon_font(view, title, r);
        let wob = (t * 2.5).sin() * 0.015;
        self.ui("p_ribbon", r, Draw::rot(wob));
        let size = kg_core::sizing::fit_size(title, size, r.w * 0.7);
        self.text_outline(
            title,
            r.center().x,
            r.center().y - r.h * 0.08,
            size,
            WHITE,
            Color::from_rgba(140, 20, 30, 255),
        );
    }

    pub fn hint(&mut self, label: &str, c: Vec2, pulse: f32) {
        let k = self.view.map_or(1.0, |v| v.text_k());
        let size = 32.0 * k * pulse;
        let w = measure(label, size) + 70.0 * k;
        let w = self.view.map_or(w, |v| w.min(v.w - 16.0));
        let size = kg_core::sizing::fit_size(label, size, w - 40.0 * k);
        let r = Rect::new(c.x - w / 2.0, c.y - 32.0 * k * pulse, w, 64.0 * k * pulse);
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

    pub fn results(&mut self, view: &View, r: &Results, age: f32) -> kg_core::sizing::Results {
        let board = shown_rows(r.game, r.rank);
        self.results_with(view, r, age, board)
    }

    pub fn results_actions(
        &mut self,
        view: &View,
        r: &Results,
        actions: &[&str],
        age: f32,
    ) -> kg_core::sizing::Results {
        let board = shown_rows(r.game, r.rank);
        let reportable = board.iter().any(|e| !e.player.is_empty());
        let lay = kg_core::sizing::results_layout(
            view,
            r.title,
            r.line,
            actions,
            board.len(),
            reportable,
        );
        self.results_layout(view, r, age, &board, &lay);
        lay
    }

    pub fn results_with(
        &mut self,
        view: &View,
        r: &Results,
        age: f32,
        board: Vec<ShownRow>,
    ) -> kg_core::sizing::Results {
        let reportable = board.iter().any(|e| !e.player.is_empty());
        let lay = kg_core::sizing::results_layout(
            view,
            r.title,
            r.line,
            &[r.action],
            board.len(),
            reportable,
        );
        self.results_layout(view, r, age, &board, &lay);
        lay
    }

    pub fn results_layout(
        &mut self,
        view: &View,
        r: &Results,
        age: f32,
        board: &[ShownRow],
        lay: &kg_core::sizing::Results,
    ) {
        let fade = ease_out_cubic(age / 0.2);
        self.fill_screen(view, Color::new(0.06, 0.04, 0.16, 0.55 * fade));
        let sheet = &lay.sheet;
        let s = ease_out_back(age / 0.32).max(0.01);
        let drop = (1.0 - ease_out_cubic(age / 0.32)) * 90.0;
        let pop = Pop::new(sheet.panel.center(), s, drop);
        let p = pop.rect(sheet.panel);
        let k = sheet.k * s;
        self.painted_panel(p, WHITE);
        if let Some((rr, _)) = sheet.ribbon {
            self.ribbon_k(view, r.title, pop.point(rr.center()), age, sheet.k * s);
        }
        let a = |d: f32| ((age - d) / 0.2).clamp(0.0, 1.0);
        let (lines, size, _) = sheet.lines(lay.line);
        for (n, l) in lines.iter().enumerate() {
            let at = pop.point(vec2(p.center().x, sheet.line_y(lay.line, n)));
            self.text_center(l, p.center().x, at.y, size * s, BODY.with_alpha(a(0.12)));
        }
        if r.rank == Some(0) {
            let kk = ease_out_back(((age - 0.35) / 0.3).clamp(0.0, 1.0));
            if kk > 0.0 {
                let wob = (age * 6.0).sin() * 0.06;
                let best = pop.rect(sheet.rect(lay.best));
                self.trophy(vec2(p.x + 46.0 * k, p.y + 70.0 * k), 92.0 * kk * sheet.k);
                let st = 70.0 * kk * sheet.k;
                self.ui(
                    "p_star",
                    Rect::new(p.x + p.w - 90.0 * k, p.y + 30.0 * k, st, st),
                    Draw::rot(wob),
                );
                self.text_outline(
                    "New best!",
                    best.center().x,
                    best.center().y,
                    30.0 * k * (1.0 + wob * 0.5),
                    GOLD_TEXT,
                    Color::from_rgba(130, 50, 0, 255),
                );
            }
        }
        let rk = lay.row_h / 46.0 * s;
        for (i, e) in board.iter().take(lay.rows).enumerate() {
            let la = a(0.2 + i as f32 * 0.05);
            let row = pop.rect(lay.row(i));
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
            let txt = (26.0 * rk).min(row.h * 0.62);
            self.text_center(
                &e.place.to_string(),
                row.x + 26.0 * rk,
                cy,
                txt,
                BODY.with_alpha(la),
            );
            if e.place == 1 {
                self.crown(vec2(row.x + 26.0 * rk, cy - row.h * 0.55), txt);
            }
            self.avatar(e.avatar, vec2(row.x + 72.0 * rk, cy), row.h * 0.95);
            let nw = measure(&e.name, txt);
            self.text_center(
                &e.name,
                row.x + 104.0 * rk + nw / 2.0,
                cy,
                txt,
                BODY.with_alpha(la),
            );
            let sc = (r.format)(e.score);
            let st = (30.0 * rk).min(row.h * 0.7);
            let sw = measure(&sc, st);
            if !e.player.is_empty() {
                let hit = lay.report_hit(lay.row(i), sw / s);
                let ic = (26.0 * rk).min(hit.w * 0.6);
                let hc = pop.point(hit.center());
                self.ui(
                    "icon_report",
                    Rect::new(hc.x - ic / 2.0, cy - ic / 2.0, ic, ic),
                    Draw::tint(Color::new(0.45, 0.35, 0.6, 0.55 * la)),
                );
                REPORT_HITS.with(|h| {
                    h.borrow_mut().push(ReportHit {
                        rect: hit,
                        player: e.player.clone(),
                        name: e.name.clone(),
                        game: r.game.to_string(),
                    })
                });
            }
            self.text_outline(
                &sc,
                row.x + row.w - 18.0 * rk - sw / 2.0,
                cy,
                st,
                WHITE.with_alpha(la),
                Color::from_rgba(70, 30, 100, 255).with_alpha(la),
            );
        }
        let pulse = 1.0 + (age * 4.5).sin() * 0.03;
        let tones = [Tone::Green, Tone::Blue, Tone::Orange];
        for (j, b) in lay.buttons().iter().enumerate() {
            let label = sheet.parts[lay.action]
                .lines
                .get(j)
                .map_or("", |s| s.as_str());
            let br = pop.rect(*b);
            let br = if j == 0 {
                br.scaled_about_center(pulse)
            } else {
                br
            };
            self.painted_button(br, label, tones[j % 3], 0.0, a(0.3));
        }
    }

    pub fn panel(
        &mut self,
        view: &View,
        title: &str,
        lines: &[&str],
        age: f32,
    ) -> kg_core::sizing::Sheet {
        let (sheet, ids) = kg_core::sizing::panel_layout(view, title, lines);
        let fade = ease_out_cubic(age / 0.2);
        self.fill_screen(view, Color::new(0.06, 0.04, 0.16, 0.5 * fade));
        let s = ease_out_back(age / 0.32).max(0.01);
        let pop = Pop::new(
            sheet.panel.center(),
            s,
            (1.0 - ease_out_cubic(age / 0.32)) * 90.0,
        );
        self.painted_panel(pop.rect(sheet.panel), WHITE);
        if let Some((rr, _)) = sheet.ribbon {
            self.ribbon_k(view, title, pop.point(rr.center()), age, sheet.k * s);
        }
        let cx = sheet.panel.center().x;
        for (i, id) in ids.iter().enumerate() {
            let la = ((age - 0.12 - i as f32 * 0.06) / 0.2).clamp(0.0, 1.0);
            let rise = (1.0 - la) * 16.0;
            let part = &sheet.parts[*id];
            if !part.rects.is_empty() {
                let pulse = 1.0 + (age * 4.5).sin() * 0.03;
                let br = pop.rect(part.rects[0]).scaled_about_center(pulse);
                self.painted_button(br.offset(vec2(0.0, rise)), lines[i], Tone::Green, 0.0, la);
            } else {
                for (n, l) in part.lines.iter().enumerate() {
                    let y = pop.point(vec2(cx, sheet.line_y(*id, n))).y + rise;
                    self.text_center(l, cx, y, part.size * s, BODY.with_alpha(la));
                }
            }
        }
        sheet
    }

    pub fn done_sheet(
        &mut self,
        view: &View,
        d: &kg_core::sizing::Done,
        title: &str,
        line: &str,
        summary: &str,
        labels: &[&str],
        age: f32,
    ) -> (Pop, f32) {
        let fade = ease_out_cubic(age / 0.2);
        self.fill_screen(view, Color::new(0.06, 0.04, 0.16, 0.5 * fade));
        let s = ease_out_back(age / 0.32).max(0.01);
        let pop = Pop::new(
            d.panel().center(),
            s,
            (1.0 - ease_out_cubic(age / 0.32)) * 90.0,
        );
        self.painted_panel(pop.rect(d.panel()), WHITE);
        if let Some((rr, _)) = d.sheet.ribbon {
            self.ribbon_in(view, title, pop.rect(rr), age);
        }
        let lr = pop.rect(d.line());
        let ls = kg_core::sizing::fit_size(line, d.text(32.0).max(view.text(26.0)) * s, lr.w);
        self.text_center(line, lr.center().x, lr.center().y, ls, BODY);
        let a = ((age - 0.3) / 0.2).clamp(0.0, 1.0);
        let sr = pop.rect(d.summary());
        let ss = kg_core::sizing::fit_size(summary, d.text(24.0).max(view.text(20.0)) * s, sr.w);
        self.text_center(
            summary,
            sr.center().x,
            sr.center().y,
            ss,
            BODY.with_alpha(a),
        );
        let pulse = 1.0 + (age * 4.5).sin() * 0.03;
        let tones = [Tone::Green, Tone::Orange, Tone::Blue];
        for (j, b) in d.buttons().iter().enumerate() {
            let br = pop.rect(*b);
            let br = if j == 0 {
                br.scaled_about_center(pulse)
            } else {
                br
            };
            self.painted_button(
                br,
                labels.get(j).copied().unwrap_or(""),
                tones[j % 3],
                0.0,
                a,
            );
        }
        (pop, d.sheet.k * s)
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

    fn row(place: usize, name: &str, player: &str, mine: bool) -> ShownRow {
        ShownRow {
            player: player.into(),
            place,
            name: name.into(),
            avatar: 0,
            score: 10,
            mine,
        }
    }

    #[test]
    fn other_players_rows_get_a_report_button_and_mine_does_not() {
        clear_report_hits();
        let v = View::ipad_landscape();
        let mut p = Pen::default();
        p.begin(v, 1.0);
        let r = Results {
            game: "snake",
            title: "Ouch!",
            line: "",
            rank: Some(1),
            action: "Again",
            format: plain_score,
        };
        let other = "123e4567-e89b-42d3-a456-426614174000";
        let rows = vec![row(1, "Leo", other, false), row(2, "Mia", "", true)];
        p.results_with(&v, &r, 1.0, rows);
        let hits: Vec<_> = REPORT_HITS.with(|h| h.borrow().clone());
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].player, other);
        assert_eq!(hits[0].game, "snake");
        assert_eq!(report_hit_at(hits[0].rect.center()).unwrap().name, "Leo");
        assert!(report_hit_at(vec2(-5.0, -5.0)).is_none());
    }
}
