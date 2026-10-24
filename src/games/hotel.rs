use crate::engine::*;
use macroquad::prelude::*;

pub const ROOMS: usize = 12;
pub const COLS: usize = 4;
pub const START_COINS: u32 = 0;
pub const QUEUE_MAX: usize = 5;
pub const PATIENCE: f32 = 18.0;
pub const STAY: f32 = 9.0;
pub const CLEAN_TIME: f32 = 1.2;
pub const REQUEST_WAIT: f32 = 6.0;
pub const MAX_LEVEL: u8 = 3;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Need {
    Towel,
    Food,
    Music,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Room {
    Locked,
    Free,
    Busy {
        guest: u8,
        left: f32,
        need: Option<(Need, f32)>,
        mood: i32,
    },
    Dirty {
        cleaning: Option<f32>,
    },
}

#[derive(Clone, Copy, Debug)]
pub struct Guest {
    pub look: u8,
    pub patience: f32,
}

pub struct Hotel {
    pub rooms: [Room; ROOMS],
    pub levels: [u8; ROOMS],
    pub queue: Vec<Guest>,
    pub coins: u32,
    pub stars: f32,
    pub served: u32,
    pub popups: Vec<(Vec2, String, f32)>,
    spawn_in: f32,
    t: f32,
}

pub fn room_price(index: usize) -> u32 {
    if index < 2 {
        0
    } else {
        30 + (index as u32 - 2) * 25
    }
}

pub fn upgrade_price(level: u8) -> u32 {
    40 * level as u32
}

pub fn pay(level: u8, mood: i32) -> u32 {
    (10 * level as i32 + mood * 3).max(2) as u32
}

impl Hotel {
    pub fn new() -> Self {
        let mut rooms = [Room::Locked; ROOMS];
        let levels = [1; ROOMS];
        for (i, r) in rooms.iter_mut().enumerate() {
            if room_price(i) == 0 {
                *r = Room::Free;
            }
        }
        Self {
            rooms,
            levels,
            queue: vec![],
            coins: START_COINS,
            stars: 3.0,
            served: 0,
            popups: vec![],
            spawn_in: 0.5,
            t: 0.0,
        }
    }

    pub fn arrival_gap(&self) -> f32 {
        let open = self.rooms.iter().filter(|r| **r != Room::Locked).count() as f32;
        (7.5 - open * 0.35 - self.stars * 0.3).max(1.8)
    }

    pub fn tap_room(&mut self, i: usize) {
        match self.rooms[i] {
            Room::Locked => {
                let next_locked = self.rooms.iter().position(|r| *r == Room::Locked);
                let price = room_price(i);
                if next_locked == Some(i) && self.coins >= price {
                    self.coins -= price;
                    self.rooms[i] = Room::Free;
                }
            }
            Room::Free => {
                if !self.queue.is_empty() {
                    let g = self.queue.remove(0);
                    let mood = if g.patience > PATIENCE * 0.5 { 1 } else { 0 };
                    self.rooms[i] = Room::Busy {
                        guest: g.look,
                        left: STAY,
                        need: None,
                        mood,
                    };
                }
            }
            Room::Busy {
                guest,
                left,
                need: Some(_),
                mood,
            } => {
                self.rooms[i] = Room::Busy {
                    guest,
                    left,
                    need: None,
                    mood: mood + 1,
                };
                self.coins += 3;
            }
            Room::Dirty { cleaning: None } => {
                self.rooms[i] = Room::Dirty {
                    cleaning: Some(CLEAN_TIME),
                };
            }
            _ => {}
        }
    }

    pub fn try_upgrade(&mut self, i: usize) -> bool {
        let lvl = self.levels[i];
        if self.rooms[i] == Room::Locked || lvl >= MAX_LEVEL || self.coins < upgrade_price(lvl) {
            return false;
        }
        self.coins -= upgrade_price(lvl);
        self.levels[i] += 1;
        true
    }

    pub fn step(&mut self, dt: f32) -> Vec<(usize, i32)> {
        self.t += dt;
        let mut events = vec![];
        self.spawn_in -= dt;
        if self.spawn_in <= 0.0 {
            self.spawn_in = self.arrival_gap() * rand::gen_range(0.7, 1.3);
            if self.queue.len() < QUEUE_MAX {
                self.queue.push(Guest {
                    look: rand::gen_range(0, 6),
                    patience: PATIENCE,
                });
            }
        }
        let before = self.queue.len();
        for g in &mut self.queue {
            g.patience -= dt;
        }
        self.queue.retain(|g| g.patience > 0.0);
        let left_angry = before - self.queue.len();
        self.stars = (self.stars - 0.25 * left_angry as f32).max(0.0);
        for i in 0..ROOMS {
            self.rooms[i] = match self.rooms[i] {
                Room::Busy {
                    guest,
                    left,
                    need,
                    mood,
                } => {
                    let left = left - dt;
                    if left <= 0.0 {
                        let earned = pay(self.levels[i], mood);
                        self.coins += earned;
                        self.served += 1;
                        self.stars =
                            (self.stars + if mood > 0 { 0.1 } else { -0.05 }).clamp(0.0, 5.0);
                        events.push((i, earned as i32));
                        Room::Dirty { cleaning: None }
                    } else {
                        let (need, mood) = match need {
                            Some((_, w)) if w - dt <= 0.0 => (None, mood - 1),
                            Some((n, w)) => (Some((n, w - dt)), mood),
                            None if left > 2.0 && rand::gen_range(0.0, 1.0) < dt * 0.12 => {
                                let n =
                                    [Need::Towel, Need::Food, Need::Music][rand::gen_range(0, 3)];
                                (Some((n, REQUEST_WAIT)), mood)
                            }
                            None => (None, mood),
                        };
                        Room::Busy {
                            guest,
                            left,
                            need,
                            mood,
                        }
                    }
                }
                Room::Dirty { cleaning: Some(c) } if c - dt <= 0.0 => Room::Free,
                Room::Dirty { cleaning: Some(c) } => Room::Dirty {
                    cleaning: Some(c - dt),
                },
                r => r,
            };
        }
        events
    }

    fn layout(view: &View) -> (Rect, Vec<Rect>) {
        let top = 80.0;
        let lobby_w = (view.w * 0.26).clamp(220.0, 320.0);
        let lobby = Rect::new(16.0, top, lobby_w, view.h - top - 16.0);
        let gx = lobby.x + lobby.w + 16.0;
        let gw = view.w - gx - 16.0;
        let rows = ROOMS.div_ceil(COLS);
        let cw = gw / COLS as f32;
        let ch = (view.h - top - 16.0) / rows as f32;
        let rects = (0..ROOMS)
            .map(|i| {
                let (c, r) = (i % COLS, rows - 1 - i / COLS);
                Rect::new(
                    gx + c as f32 * cw + 6.0,
                    top + r as f32 * ch + 6.0,
                    cw - 12.0,
                    ch - 12.0,
                )
            })
            .collect();
        (lobby, rects)
    }

    fn upgrade_rect(room: Rect) -> Rect {
        Rect::new(room.x + room.w - 46.0, room.y + 6.0, 40.0, 40.0)
    }
}

impl Default for Hotel {
    fn default() -> Self {
        Self::new()
    }
}

const SKIN: [(u8, u8, u8); 6] = [
    (255, 220, 180),
    (230, 180, 140),
    (190, 130, 90),
    (140, 90, 60),
    (255, 205, 160),
    (210, 160, 120),
];
const SHIRT: [(u8, u8, u8); 6] = [
    (240, 90, 90),
    (80, 150, 240),
    (120, 200, 90),
    (250, 180, 50),
    (180, 100, 220),
    (60, 190, 190),
];

fn rgb(c: (u8, u8, u8)) -> Color {
    Color::from_rgba(c.0, c.1, c.2, 255)
}

fn draw_guest(x: f32, y: f32, s: f32, look: u8, happy: bool) {
    let l = look as usize % 6;
    round_rect(
        Rect::new(x - 18.0 * s, y, 36.0 * s, 40.0 * s),
        12.0 * s,
        rgb(SHIRT[l]),
    );
    draw_circle(x, y - 6.0 * s, 20.0 * s, rgb(SKIN[l]));
    draw_circle(x - 7.0 * s, y - 9.0 * s, 3.0 * s, BLACK);
    draw_circle(x + 7.0 * s, y - 9.0 * s, 3.0 * s, BLACK);
    let mouth = if happy { 4.0 } else { -3.0 };
    draw_line(
        x - 6.0 * s,
        y + 1.0 * s,
        x,
        y + (1.0 + mouth) * s,
        2.0 * s,
        BLACK,
    );
    draw_line(
        x,
        y + (1.0 + mouth) * s,
        x + 6.0 * s,
        y + 1.0 * s,
        2.0 * s,
        BLACK,
    );
}

fn draw_need(c: Vec2, need: Need) {
    match need {
        Need::Towel => {
            draw_rectangle(
                c.x - 12.0,
                c.y - 10.0,
                24.0,
                20.0,
                Color::from_rgba(120, 200, 255, 255),
            );
            draw_rectangle(c.x - 12.0, c.y + 2.0, 24.0, 4.0, WHITE);
        }
        Need::Food => {
            draw_circle(c.x, c.y + 2.0, 12.0, Color::from_rgba(250, 170, 60, 255));
            draw_circle(
                c.x - 3.0,
                c.y - 2.0,
                3.0,
                Color::from_rgba(200, 60, 50, 255),
            );
            draw_circle(
                c.x + 4.0,
                c.y + 4.0,
                3.0,
                Color::from_rgba(200, 60, 50, 255),
            );
        }
        Need::Music => {
            draw_circle(
                c.x - 5.0,
                c.y + 7.0,
                5.0,
                Color::from_rgba(150, 80, 220, 255),
            );
            draw_rectangle(
                c.x - 1.0,
                c.y - 12.0,
                3.0,
                19.0,
                Color::from_rgba(150, 80, 220, 255),
            );
            draw_rectangle(
                c.x - 1.0,
                c.y - 12.0,
                11.0,
                4.0,
                Color::from_rgba(150, 80, 220, 255),
            );
        }
    }
}

impl Game for Hotel {
    fn update(&mut self, input: &Input, view: &View, dt: f32) {
        let (_, rects) = Self::layout(view);
        if input.pressed {
            for (i, r) in rects.iter().enumerate() {
                let up = Self::upgrade_rect(*r);
                if matches!(self.rooms[i], Room::Free | Room::Dirty { .. })
                    && up.contains(input.pos)
                {
                    if self.try_upgrade(i) {
                        self.popups.push((up.center(), "Upgrade!".into(), 1.0));
                    }
                } else if r.contains(input.pos) {
                    self.tap_room(i);
                }
            }
        }
        for (i, earned) in self.step(dt) {
            self.popups
                .push((rects[i].center(), format!("+{earned}"), 1.2));
        }
        for p in &mut self.popups {
            p.0.y -= 50.0 * dt;
            p.2 -= dt;
        }
        self.popups.retain(|p| p.2 > 0.0);
    }

    fn draw(&self, view: &View) {
        clear_background(Color::from_rgba(250, 236, 214, 255));
        draw_rectangle(0.0, 0.0, view.w, 70.0, Color::from_rgba(180, 70, 80, 255));
        text("Hotel Hop", HUD_LEFT, 48.0, 40.0, WHITE);
        draw_circle(
            view.w * 0.42,
            35.0,
            18.0,
            Color::from_rgba(255, 210, 60, 255),
        );
        text_center(
            "$",
            view.w * 0.42,
            35.0,
            26.0,
            Color::from_rgba(170, 120, 20, 255),
        );
        text(
            &self.coins.to_string(),
            view.w * 0.42 + 28.0,
            48.0,
            38.0,
            WHITE,
        );
        for s in 0..5 {
            let x = view.w * 0.62 + s as f32 * 38.0;
            let filled = self.stars - s as f32;
            let c = if filled >= 0.5 {
                Color::from_rgba(255, 210, 60, 255)
            } else {
                Color::from_rgba(120, 40, 50, 255)
            };
            draw_poly(x, 35.0, 5, 15.0, -90.0, c);
            draw_poly(x, 35.0, 5, 8.0, -54.0, c);
        }
        text(
            &format!("Guests {}", self.served),
            view.w - 190.0,
            48.0,
            32.0,
            WHITE,
        );

        let (lobby, rects) = Self::layout(view);
        round_rect(lobby, 18.0, Color::from_rgba(200, 160, 120, 255));
        round_rect(
            Rect::new(lobby.x + 10.0, lobby.y + 10.0, lobby.w - 20.0, 70.0),
            12.0,
            Color::from_rgba(130, 80, 50, 255),
        );
        text_center(
            "Lobby",
            lobby.x + lobby.w / 2.0,
            lobby.y + 45.0,
            34.0,
            WHITE,
        );
        let slot = ((lobby.h - 110.0) / QUEUE_MAX as f32).min(110.0);
        for (i, g) in self.queue.iter().enumerate() {
            let y = lobby.y + 100.0 + i as f32 * slot;
            let cx = lobby.x + 60.0;
            if i == 0 {
                round_rect(
                    Rect::new(lobby.x + 10.0, y, lobby.w - 20.0, slot - 8.0),
                    12.0,
                    Color::from_rgba(255, 240, 200, 255),
                );
            }
            draw_guest(
                cx,
                y + slot * 0.45,
                (slot / 110.0).max(0.6),
                g.look,
                g.patience > PATIENCE * 0.4,
            );
            let bw = lobby.w - 120.0;
            let k = g.patience / PATIENCE;
            draw_rectangle(
                lobby.x + 100.0,
                y + slot * 0.4,
                bw,
                14.0,
                Color::from_rgba(120, 80, 60, 255),
            );
            let c = if k > 0.5 {
                GREEN
            } else if k > 0.25 {
                ORANGE
            } else {
                RED
            };
            draw_rectangle(lobby.x + 100.0, y + slot * 0.4, bw * k, 14.0, c);
        }
        if self.queue.is_empty() {
            text_center(
                "Waiting for guests",
                lobby.x + lobby.w / 2.0,
                lobby.y + 140.0,
                26.0,
                Color::from_rgba(100, 60, 40, 255),
            );
        }

        let next_locked = self.rooms.iter().position(|r| *r == Room::Locked);
        for (i, r) in rects.iter().enumerate() {
            let lvl = self.levels[i];
            let wall = [
                Color::from_rgba(255, 250, 240, 255),
                Color::from_rgba(220, 240, 255, 255),
                Color::from_rgba(255, 230, 250, 255),
            ][(lvl - 1) as usize];
            match self.rooms[i] {
                Room::Locked => {
                    round_rect(*r, 14.0, Color::from_rgba(200, 190, 175, 255));
                    if next_locked == Some(i) {
                        let can = self.coins >= room_price(i);
                        let c = if can {
                            Color::from_rgba(50, 150, 60, 255)
                        } else {
                            Color::from_rgba(110, 100, 90, 255)
                        };
                        text_center("Build", r.x + r.w / 2.0, r.y + r.h * 0.38, 30.0, c);
                        text_center(
                            &format!("${}", room_price(i)),
                            r.x + r.w / 2.0,
                            r.y + r.h * 0.66,
                            34.0,
                            c,
                        );
                    } else {
                        text_center(
                            "?",
                            r.x + r.w / 2.0,
                            r.y + r.h / 2.0,
                            40.0,
                            Color::from_rgba(170, 160, 150, 255),
                        );
                    }
                    continue;
                }
                _ => round_rect(*r, 14.0, wall),
            }
            let bed = Rect::new(r.x + 12.0, r.y + r.h - 50.0, r.w * 0.55, 38.0);
            let bed_c = [
                Color::from_rgba(90, 140, 220, 255),
                Color::from_rgba(230, 120, 160, 255),
                Color::from_rgba(240, 190, 60, 255),
            ][(lvl - 1) as usize];
            round_rect(bed, 8.0, bed_c);
            round_rect(Rect::new(bed.x + 4.0, bed.y + 4.0, 26.0, 16.0), 5.0, WHITE);
            text(
                &format!("{}", i + 1),
                r.x + 10.0,
                r.y + 30.0,
                26.0,
                Color::from_rgba(150, 130, 110, 255),
            );
            for s in 0..lvl {
                draw_poly(
                    r.x + 46.0 + s as f32 * 18.0,
                    r.y + 21.0,
                    5,
                    8.0,
                    -90.0,
                    Color::from_rgba(255, 190, 40, 255),
                );
            }
            match self.rooms[i] {
                Room::Free => {
                    let pulse = if self.queue.is_empty() {
                        0.0
                    } else {
                        (self.t * 5.0).sin() * 0.5 + 0.5
                    };
                    draw_rectangle_lines(
                        r.x + 2.0,
                        r.y + 2.0,
                        r.w - 4.0,
                        r.h - 4.0,
                        4.0,
                        with_alpha(GREEN, 0.3 + 0.6 * pulse),
                    );
                    text_center(
                        "Free",
                        r.x + r.w * 0.6,
                        r.y + r.h * 0.45,
                        30.0,
                        Color::from_rgba(60, 160, 70, 255),
                    );
                }
                Room::Busy {
                    guest,
                    left,
                    need,
                    mood,
                } => {
                    draw_guest(r.x + r.w * 0.78, r.y + r.h - 60.0, 0.8, guest, mood >= 0);
                    let k = left / STAY;
                    draw_rectangle(
                        r.x + 10.0,
                        r.y + r.h - 8.0,
                        (r.w - 20.0) * k,
                        5.0,
                        Color::from_rgba(120, 180, 250, 255),
                    );
                    if let Some((n, w)) = need {
                        let c = vec2(r.x + r.w * 0.45, r.y + r.h * 0.38);
                        let bob = (self.t * 6.0).sin() * 3.0;
                        draw_circle(c.x, c.y + bob, 26.0, WHITE);
                        draw_circle_lines(
                            c.x,
                            c.y + bob,
                            26.0,
                            3.0,
                            if w < 2.0 { RED } else { ORANGE },
                        );
                        draw_need(vec2(c.x, c.y + bob), n);
                    }
                }
                Room::Dirty { cleaning } => {
                    for k in 0..5 {
                        let fx = r.x + 20.0 + (k * 37 % 70) as f32 / 70.0 * (r.w - 40.0);
                        let fy = r.y + 50.0 + (k * 53 % 50) as f32 / 50.0 * (r.h - 110.0).max(10.0);
                        draw_circle(fx, fy, 8.0, Color::from_rgba(150, 120, 80, 200));
                    }
                    match cleaning {
                        Some(c) => {
                            let k = 1.0 - c / CLEAN_TIME;
                            text_center(
                                "Cleaning",
                                r.x + r.w / 2.0,
                                r.y + r.h * 0.45,
                                26.0,
                                Color::from_rgba(80, 140, 200, 255),
                            );
                            draw_rectangle(
                                r.x + 14.0,
                                r.y + r.h * 0.55,
                                (r.w - 28.0) * k,
                                10.0,
                                Color::from_rgba(80, 160, 230, 255),
                            );
                        }
                        None => text_center(
                            "Tap to clean",
                            r.x + r.w / 2.0,
                            r.y + r.h * 0.45,
                            24.0,
                            Color::from_rgba(160, 100, 60, 255),
                        ),
                    }
                }
                Room::Locked => {}
            }
            if matches!(self.rooms[i], Room::Free | Room::Dirty { .. }) && lvl < MAX_LEVEL {
                let up = Self::upgrade_rect(*r);
                let can = self.coins >= upgrade_price(lvl);
                round_rect(
                    up,
                    10.0,
                    if can {
                        Color::from_rgba(250, 180, 40, 255)
                    } else {
                        Color::from_rgba(200, 190, 180, 255)
                    },
                );
                text_center("+", up.x + up.w / 2.0, up.y + up.h / 2.0, 34.0, WHITE);
            }
        }
        for (p, s, life) in &self.popups {
            text_shadow(
                s,
                p.x,
                p.y,
                34.0,
                with_alpha(Color::from_rgba(255, 200, 40, 255), life.min(1.0)),
            );
        }
    }
}

pub fn icon(r: Rect) {
    draw_rectangle(
        r.x + r.w * 0.2,
        r.y + r.h * 0.2,
        r.w * 0.6,
        r.h * 0.75,
        Color::from_rgba(180, 70, 80, 255),
    );
    draw_triangle(
        vec2(r.x + r.w * 0.12, r.y + r.h * 0.22),
        vec2(r.x + r.w * 0.88, r.y + r.h * 0.22),
        vec2(r.x + r.w * 0.5, r.y + r.h * 0.04),
        Color::from_rgba(120, 50, 60, 255),
    );
    for row in 0..3 {
        for col in 0..2 {
            let lit = (row + col) % 2 == 0;
            draw_rectangle(
                r.x + r.w * (0.29 + col as f32 * 0.25),
                r.y + r.h * (0.3 + row as f32 * 0.17),
                r.w * 0.17,
                r.h * 0.11,
                if lit {
                    Color::from_rgba(255, 220, 100, 255)
                } else {
                    Color::from_rgba(250, 236, 214, 255)
                },
            );
        }
    }
    draw_rectangle(
        r.x + r.w * 0.42,
        r.y + r.h * 0.8,
        r.w * 0.16,
        r.h * 0.15,
        Color::from_rgba(90, 50, 40, 255),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    fn with_guest() -> Hotel {
        let mut h = Hotel::new();
        h.queue.push(Guest {
            look: 0,
            patience: PATIENCE,
        });
        h.spawn_in = 999.0;
        h
    }

    #[test]
    fn full_guest_cycle_pays_and_frees_room() {
        let mut h = with_guest();
        h.tap_room(0);
        assert!(matches!(h.rooms[0], Room::Busy { .. }));
        assert!(h.queue.is_empty());
        let mut paid = 0;
        for _ in 0..(STAY * 10.0) as usize + 2 {
            if let Room::Busy { need: Some(_), .. } = h.rooms[0] {
                h.tap_room(0);
            }
            paid += h.step(0.1).len();
        }
        assert_eq!(paid, 1);
        assert!(h.coins >= pay(1, 1));
        assert_eq!(h.rooms[0], Room::Dirty { cleaning: None });
        h.tap_room(0);
        for _ in 0..(CLEAN_TIME * 10.0) as usize + 2 {
            h.step(0.1);
        }
        assert_eq!(h.rooms[0], Room::Free);
    }

    #[test]
    fn rooms_unlock_in_order_and_cost_coins() {
        let mut h = Hotel::new();
        h.coins = 1000;
        h.tap_room(5);
        assert_eq!(h.rooms[5], Room::Locked);
        h.tap_room(2);
        assert_eq!(h.rooms[2], Room::Free);
        assert_eq!(h.coins, 1000 - room_price(2));
    }

    #[test]
    fn cannot_build_without_coins() {
        let mut h = Hotel::new();
        h.coins = room_price(2) - 1;
        h.tap_room(2);
        assert_eq!(h.rooms[2], Room::Locked);
    }

    #[test]
    fn impatient_guest_leaves_and_costs_rating() {
        let mut h = with_guest();
        let stars = h.stars;
        for _ in 0..(PATIENCE * 10.0) as usize + 2 {
            h.step(0.1);
        }
        assert!(h.queue.is_empty());
        assert!(h.stars < stars);
    }

    #[test]
    fn upgrade_raises_pay() {
        let mut h = Hotel::new();
        h.coins = upgrade_price(1);
        assert!(h.try_upgrade(0));
        assert_eq!(h.levels[0], 2);
        assert_eq!(h.coins, 0);
        assert!(pay(2, 0) > pay(1, 0));
        assert!(!h.try_upgrade(0));
    }
}
