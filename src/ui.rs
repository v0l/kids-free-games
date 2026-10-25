use crate::engine::*;
use crate::fx::{ease_out_back, ease_out_cubic, text_outline};
use macroquad::prelude::*;

crate::sprites!("ui");

pub const INK: Color = Color::new(0.16, 0.12, 0.24, 1.0);
pub const BODY: Color = Color::new(0.33, 0.30, 0.42, 1.0);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tone {
    Yellow,
    Green,
    Blue,
    Red,
    Grey,
}

impl Tone {
    fn button(self) -> &'static str {
        match self {
            Tone::Yellow => "btn_yellow",
            Tone::Green => "btn_green",
            Tone::Blue => "btn_blue",
            Tone::Red => "btn_red",
            Tone::Grey => "btn_grey",
        }
    }

    fn ink(self) -> Color {
        match self {
            Tone::Yellow => Color::from_rgba(150, 90, 0, 255),
            Tone::Green => Color::from_rgba(10, 100, 60, 255),
            Tone::Blue => Color::from_rgba(10, 70, 140, 255),
            Tone::Red => Color::from_rgba(140, 20, 40, 255),
            Tone::Grey => INK,
        }
    }
}

pub fn button(r: Rect, label: &str, tone: Tone, press: f32) {
    let s = 1.0 - press * 0.06;
    let r = Rect::new(
        r.center().x - r.w * s / 2.0,
        r.center().y - r.h * s / 2.0 + press * 3.0,
        r.w * s,
        r.h * s,
    );
    nine(tone.button(), r, 18.0, r.h / 64.0, WHITE);
    let size = (r.h * 0.46).min(r.w / (label.len().max(1) as f32 * 0.55));
    text_outline(
        label,
        r.center().x,
        r.y + r.h * 0.44,
        size,
        WHITE,
        tone.ink(),
    );
}

pub fn round_button(r: Rect, icon: &str, tone: Tone, press: f32) {
    let s = 1.0 - press * 0.08;
    let c = r.center() + vec2(0.0, press * 2.0);
    let base = match tone {
        Tone::Yellow => "round_yellow",
        Tone::Blue => "round_blue",
        _ => "round_white",
    };
    let d = r.h.min(r.w) * s;
    sprite(base, Rect::new(c.x - d / 2.0, c.y - d / 2.0, d, d));
    let ic = d * 0.5;
    let tint = if base == "round_white" { BODY } else { WHITE };
    sprite_ex(
        icon,
        Rect::new(c.x - ic / 2.0, c.y - ic / 2.0 - d * 0.05, ic, ic),
        crate::art::Draw {
            tint,
            ..Default::default()
        },
    );
}

pub fn painted_round(r: Rect, icon: &str, press: f32) {
    let s = 1.0 - press * 0.08;
    let c = r.center() + vec2(0.0, press * 2.0);
    let d = r.h.min(r.w) * s;
    sprite("p_round", Rect::new(c.x - d / 2.0, c.y - d / 2.0, d, d));
    let ic = d * 0.48;
    sprite_ex(
        icon,
        Rect::new(c.x - ic / 2.0, c.y - ic / 2.0 - d * 0.03, ic, ic),
        crate::art::Draw {
            tint: Color::from_rgba(150, 70, 0, 255),
            ..Default::default()
        },
    );
    sprite_ex(
        icon,
        Rect::new(c.x - ic / 2.0, c.y - ic / 2.0 - d * 0.06, ic, ic),
        crate::art::Draw::default(),
    );
}

pub fn card(r: Rect, tint: Color) {
    nine(
        "panel",
        r,
        14.0,
        (r.w.min(r.h) / 64.0).clamp(0.6, 2.5),
        tint,
    );
}

pub fn chip_box(r: Rect) {
    round_rect(
        Rect::new(r.x, r.y + 4.0, r.w, r.h),
        r.h / 2.0,
        Color::new(0.05, 0.02, 0.15, 0.3),
    );
    round_rect(r, r.h / 2.0, Color::new(0.2, 0.08, 0.35, 0.72));
    round_rect(
        Rect::new(r.x + 3.0, r.y + 3.0, r.w - 6.0, r.h * 0.45),
        r.h / 2.0,
        Color::new(1.0, 1.0, 1.0, 0.1),
    );
}

pub fn kenney_chip_box(r: Rect) {
    let k = r.h / 64.0;
    nine(
        "panel",
        Rect::new(r.x, r.y + 3.0, r.w, r.h),
        14.0,
        k,
        Color::new(0.0, 0.0, 0.0, 0.18),
    );
    nine("panel", r, 14.0, k, WHITE);
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

mod icons {
    crate::sprites!("icons");
}

pub fn icon(game: &str, r: Rect) {
    icons::sprite(&format!("icon_{game}"), r);
}

pub fn has_icon(game: &str) -> bool {
    icons::frame(&format!("icon_{game}")).is_some()
}

pub fn painted_panel(r: Rect, tint: Color) {
    let k = (r.w.min(r.h) / 400.0).clamp(0.3, 2.0);
    nine("p_panel", r, 70.0, k, tint);
}

pub fn painted_button(r: Rect, label: &str, color: &str, press: f32, alpha: f32) {
    let s = 1.0 - press * 0.06;
    let r = Rect::new(
        r.center().x - r.w * s / 2.0,
        r.center().y - r.h * s / 2.0 + press * 3.0,
        r.w * s,
        r.h * s,
    );
    let name = match color {
        "orange" => "p_btn_orange",
        "blue" => "p_btn_blue",
        _ => "p_btn_green",
    };
    let ink = match color {
        "orange" => Color::from_rgba(170, 70, 0, 255),
        "blue" => Color::from_rgba(10, 80, 150, 255),
        _ => Color::from_rgba(40, 120, 20, 255),
    };
    nine(name, r, 60.0, r.h / 150.0, with_alpha(WHITE, alpha));
    text_outline(
        label,
        r.center().x,
        r.y + r.h * 0.45,
        r.h * 0.46,
        with_alpha(WHITE, alpha),
        with_alpha(ink, alpha),
    );
}

pub fn ribbon(view: &View, title: &str, c: Vec2, t: f32) {
    let size = 54.0;
    let tw = crate::outline::measure(FONT_BYTES, title, size);
    let w = (tw + 260.0).min(view.w - 20.0);
    let rs = size_of("p_ribbon");
    let h = w * rs.y / rs.x;
    let wob = (t * 2.5).sin() * 0.015;
    sprite_ex(
        "p_ribbon",
        Rect::new(c.x - w / 2.0, c.y - h / 2.0, w, h),
        crate::art::Draw {
            rotation: wob,
            ..Default::default()
        },
    );
    text_outline(
        title,
        c.x,
        c.y - h * 0.08,
        size.min(w * 0.7 / tw * size),
        WHITE,
        Color::from_rgba(140, 20, 30, 255),
    );
}

fn size_of(name: &str) -> Vec2 {
    size(name)
}

pub fn hint(view: &View, label: &str, c: Vec2, pulse: f32) {
    let size = 32.0 * pulse;
    let w = crate::outline::measure(FONT_BYTES, label, size) + 70.0;
    let r = Rect::new(c.x - w / 2.0, c.y - 32.0 * pulse, w, 64.0 * pulse);
    let _ = view;
    round_rect(
        Rect::new(r.x, r.y + 4.0, r.w, r.h),
        r.h / 2.0,
        Color::new(0.0, 0.0, 0.0, 0.18),
    );
    round_rect(r, r.h / 2.0, Color::new(1.0, 1.0, 1.0, 0.92));
    text_center(label, c.x, c.y, size, BODY);
}

pub fn panel(view: &View, title: &str, lines: &[&str], age: f32, accent: Color) {
    let _ = accent;
    let fade = ease_out_cubic(age / 0.2);
    fill_screen(view, Color::new(0.06, 0.04, 0.16, 0.5 * fade));
    let pw = 560.0f32.min(view.w - 40.0);
    let ph = 150.0 + lines.len() as f32 * 64.0;
    let s = ease_out_back(age / 0.32).max(0.01);
    let c = view.center() + vec2(0.0, 20.0 + (1.0 - ease_out_cubic(age / 0.32)) * 90.0);
    let r = Rect::new(c.x - pw * s / 2.0, c.y - ph * s / 2.0, pw * s, ph * s);
    painted_panel(r, WHITE);
    ribbon(view, title, vec2(c.x, r.y + 8.0 * s), age);
    let stars = ((age - 0.25) / 0.12).clamp(0.0, 3.0);
    let _ = stars;
    for (i, l) in lines.iter().enumerate() {
        let la = ((age - 0.12 - i as f32 * 0.06) / 0.2).clamp(0.0, 1.0);
        let last = i + 1 == lines.len();
        let y = r.y + 96.0 * s + i as f32 * 64.0 * s + (1.0 - la) * 16.0;
        if last && lines.len() > 1 {
            let pulse = 1.0 + (age * 4.5).sin() * 0.03;
            let bw = (crate::outline::measure(FONT_BYTES, l, 30.0) + 110.0) * s * pulse;
            let br = Rect::new(c.x - bw / 2.0, y - 8.0 * s, bw, 78.0 * s * pulse);
            painted_button(br, l, "green", 0.0, la);
        } else {
            text_center(l, c.x, y + 14.0 * s, 34.0 * s, with_alpha(BODY, la));
        }
    }
}

pub fn spark(c: Vec2, size: f32, angle: f32, color: Color) {
    let name = if color.r > 0.85 && color.g > 0.85 && color.b > 0.85 {
        "star_white"
    } else if color.r > 0.8 && color.g > 0.6 && color.b < 0.5 {
        "star_yellow"
    } else {
        "star_white"
    };
    let tint = if name == "star_white" {
        color
    } else {
        Color::new(1.0, 1.0, 1.0, color.a)
    };
    sprite_ex(
        name,
        Rect::new(c.x - size, c.y - size, size * 2.0, size * 2.0),
        crate::art::Draw {
            tint,
            rotation: angle,
            ..Default::default()
        },
    );
}

pub fn puff(c: Vec2, size: f32, color: Color) {
    sprite_ex(
        "puff",
        Rect::new(c.x - size, c.y - size, size * 2.0, size * 2.0),
        crate::art::Draw {
            tint: color,
            ..Default::default()
        },
    );
}

pub fn cloud(i: usize, c: Vec2, w: f32, tint: Color) {
    let name = [
        "cloud1", "cloud2", "cloud3", "cloud4", "cloud5", "cloud6", "cloud7", "cloud8",
    ][i % 8];
    sprite_w(
        name,
        c,
        w,
        crate::art::Draw {
            tint,
            ..Default::default()
        },
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_ui_sprite_the_code_names_exists() {
        let have = names();
        for n in [
            "panel",
            "btn_yellow",
            "btn_green",
            "btn_blue",
            "btn_red",
            "btn_grey",
            "round_yellow",
            "round_blue",
            "round_white",
            "icon_home",
            "icon_music",
            "icon_music_off",
            "icon_audio",
            "icon_audio_off",
            "medal_gold",
            "medal_silver",
            "medal_bronze",
            "star_yellow",
            "star_white",
            "puff",
            "cloud1",
            "cloud8",
            "coin",
            "star",
            "star_empty",
        ] {
            assert!(have.iter().any(|h| h == n), "ui sheet lacks {n}");
        }
    }

    #[test]
    fn every_game_has_a_painted_icon() {
        for g in [
            "flappy", "paper", "hotel", "snake", "merge", "fruit", "stack", "breakout", "jumper",
            "memory", "crossy", "helix", "blocks", "mathpop", "count", "spell",
        ] {
            assert!(has_icon(g), "no icon for {g}");
        }
        for n in [
            "p_panel",
            "p_btn_green",
            "p_btn_orange",
            "p_btn_blue",
            "p_ribbon",
            "p_round",
            "p_coin",
            "p_star",
            "p_medal",
        ] {
            assert!(names().iter().any(|h| h == n), "ui sheet lacks {n}");
        }
    }

    #[test]
    fn medals_track_how_close_you_got_to_your_best() {
        assert_eq!(medal_for(0, 10), None);
        assert_eq!(medal_for(10, 10), Some("medal_gold"));
        assert_eq!(medal_for(7, 10), Some("medal_silver"));
        assert_eq!(medal_for(2, 10), Some("medal_bronze"));
    }
}
