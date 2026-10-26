use crate::geom::*;
use glam::{vec2, Vec2};

pub const MAX_HOTEL: u32 = 4;
pub const CORR: f32 = 1.6;
pub const HALL_W: f32 = 6.0;
pub const LOBBY_S: f32 = 8.0;
pub const WING_D: f32 = 6.0;
pub const SIDE_W: f32 = 5.0;
pub const SIDE_N: f32 = 1.0;
pub const SIDE_S: f32 = 7.0;
pub const SIDE_DOOR: f32 = 4.0;
pub const DOOR_HALF: f32 = 0.7;
pub const WALL_T: f32 = 0.24;
pub const STALL_H: f32 = 1.7;
pub const GARDEN_D: f32 = 8.0;
pub const OUTSIDE_S: f32 = 12.5;
pub const SPAWN: Vec2 = Vec2::new(0.0, 13.5);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Build {
    Room(u8),
    Decor(u8, u8),
    Gate(u8),
    Toilets,
    Cafe,
    Pool,
    Clerk,
    Maid,
    Janitor,
    Waiter,
    Lifeguard,
    Mop,
    Shoes(u8),
    Stack(u8),
    Next,
}

impl Build {
    pub fn is_staff(self) -> bool {
        matches!(
            self,
            Build::Clerk | Build::Maid | Build::Janitor | Build::Waiter | Build::Lifeguard
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PadSpec {
    pub build: Build,
    pub cost: u32,
    pub level: u32,
    pub pos: Vec2,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Solid {
    Wall,
    Locked,
    Thing,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SpotKind {
    Bed,
    Bin,
    Rug,
    Wardrobe,
    Tv,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Spot {
    pub kind: SpotKind,
    pub stand: Vec2,
    pub obj: Vec2,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Theme {
    pub name: &'static str,
    pub wall: Color,
    pub trim: Color,
    pub outside: Color,
    pub path: Color,
    pub carpet: Color,
    pub room_floor: Color,
    pub accent: Color,
    pub floor_tint: Color,
}

pub const THEMES: [Theme; 4] = [
    Theme {
        name: "Sunny Inn",
        wall: Color::new(0.55, 0.82, 0.6, 1.0),
        trim: Color::new(0.98, 0.84, 0.66, 1.0),
        outside: Color::new(0.42, 0.75, 0.32, 1.0),
        path: Color::new(0.9, 0.86, 0.7, 1.0),
        carpet: Color::new(0.16, 0.45, 0.32, 1.0),
        room_floor: Color::new(0.95, 0.62, 0.42, 1.0),
        accent: Color::new(0.95, 0.55, 0.15, 1.0),
        floor_tint: Color::new(1.0, 1.0, 0.92, 1.0),
    },
    Theme {
        name: "Snowy Lodge",
        wall: Color::new(0.78, 0.55, 0.38, 1.0),
        trim: Color::new(0.96, 0.92, 0.86, 1.0),
        outside: Color::new(0.9, 0.95, 1.0, 1.0),
        path: Color::new(0.8, 0.86, 0.94, 1.0),
        carpet: Color::new(0.62, 0.16, 0.2, 1.0),
        room_floor: Color::new(0.85, 0.6, 0.4, 1.0),
        accent: Color::new(0.3, 0.55, 0.95, 1.0),
        floor_tint: Color::new(0.92, 0.85, 0.8, 1.0),
    },
    Theme {
        name: "City Tower",
        wall: Color::new(0.62, 0.72, 0.92, 1.0),
        trim: Color::new(0.92, 0.94, 1.0, 1.0),
        outside: Color::new(0.5, 0.74, 0.38, 1.0),
        path: Color::new(0.78, 0.78, 0.82, 1.0),
        carpet: Color::new(0.2, 0.22, 0.45, 1.0),
        room_floor: Color::new(0.72, 0.6, 0.9, 1.0),
        accent: Color::new(0.55, 0.4, 0.95, 1.0),
        floor_tint: Color::new(0.88, 0.9, 1.0, 1.0),
    },
    Theme {
        name: "Royal Palace",
        wall: Color::new(0.98, 0.72, 0.8, 1.0),
        trim: Color::new(1.0, 0.86, 0.45, 1.0),
        outside: Color::new(0.4, 0.72, 0.4, 1.0),
        path: Color::new(0.95, 0.88, 0.7, 1.0),
        carpet: Color::new(0.55, 0.1, 0.3, 1.0),
        room_floor: Color::new(0.92, 0.4, 0.5, 1.0),
        accent: Color::new(0.95, 0.72, 0.15, 1.0),
        floor_tint: Color::new(1.0, 0.92, 0.85, 1.0),
    },
];

#[derive(Clone, Debug)]
pub struct Layout {
    pub number: u32,
    pub theme: Theme,
    pub wings: u8,
    pub has_toilets: bool,
    pub has_cafe: bool,
    pub has_pool: bool,
    pub rooms: Vec<Rect>,
    pub desk: Rect,
    pub desk_zone: Rect,
    pub desk_front: Vec2,
    pub desk_pile: Vec2,
    pub clerk_spot: Vec2,
    pub toilet_room: Rect,
    pub stalls: Vec<Vec2>,
    pub stall_front: Vec<Vec2>,
    pub toilet_wait: Vec2,
    pub paper_shelf: Vec2,
    pub toilet_pile: Vec2,
    pub cafe: Rect,
    pub counter: Rect,
    pub counter_spot: Vec2,
    pub tables: Vec<Vec2>,
    pub cafe_pile: Vec2,
    pub garden: Rect,
    pub pool: Rect,
    pub sunbeds: Vec<Vec2>,
    pub towel_shelf: Vec2,
    pub pool_pile: Vec2,
    pub pads: Vec<PadSpec>,
    pub room_price: u32,
    pub patience: f32,
    pub arrive_every: f32,
    pub vip_share: f32,
    pub extra_spots: usize,
    pub max_level: u32,
    pub xp_scale: f32,
    pub guide_after: f32,
}

pub fn room_side(r: usize) -> f32 {
    if r.is_multiple_of(2) {
        -1.0
    } else {
        1.0
    }
}

pub fn room_rect(r: usize) -> Rect {
    let w = (r / 2) as f32;
    let z1 = -w * WING_D;
    let x = if r.is_multiple_of(2) { -HALL_W } else { CORR };
    Rect::new(x, z1 - WING_D, HALL_W - CORR, WING_D)
}

pub fn wing_z(w: usize) -> f32 {
    -(w as f32) * WING_D - WING_D / 2.0
}

pub fn room_pt(r: usize, u: f32, v: f32) -> Vec2 {
    vec2(room_side(r) * (CORR + u), wing_z(r / 2) + v)
}

pub fn room_door(r: usize) -> Vec2 {
    room_pt(r, -0.7, 0.0)
}

pub fn room_center(r: usize) -> Vec2 {
    room_pt(r, 2.2, 0.0)
}

pub fn room_bed(r: usize) -> Vec2 {
    room_pt(r, 3.3, 0.0)
}

pub fn bed_yaw(r: usize) -> f32 {
    if r.is_multiple_of(2) {
        std::f32::consts::FRAC_PI_2
    } else {
        -std::f32::consts::FRAC_PI_2
    }
}

pub fn room_spots(r: usize, decor: u8, extra: usize) -> Vec<Spot> {
    let mut v = vec![
        Spot {
            kind: SpotKind::Bed,
            stand: room_pt(r, 3.0, 1.35),
            obj: room_bed(r),
        },
        Spot {
            kind: SpotKind::Bin,
            stand: room_pt(r, 0.95, -1.55),
            obj: room_pt(r, 0.45, -2.1),
        },
    ];
    if extra > 0 || decor >= 2 {
        v.push(Spot {
            kind: SpotKind::Rug,
            stand: room_pt(r, 1.5, 1.0),
            obj: room_pt(r, 1.5, 1.0),
        });
    }
    if decor >= 2 {
        v.push(Spot {
            kind: SpotKind::Wardrobe,
            stand: room_pt(r, 3.4, -1.45),
            obj: room_pt(r, 4.0, -2.05),
        });
    }
    if decor >= 3 {
        v.push(Spot {
            kind: SpotKind::Tv,
            stand: room_pt(r, 0.95, 1.6),
            obj: room_pt(r, 0.3, 2.1),
        });
    }
    v
}

pub fn queue_slot(k: usize) -> Vec2 {
    vec2(0.0, 3.75 + 0.9 * k as f32)
}

pub fn seat_spots(table: Vec2) -> [Vec2; 2] {
    [table + vec2(-0.95, 0.0), table + vec2(0.95, 0.0)]
}

fn wall_v(x: f32, z0: f32, z1: f32) -> Rect {
    Rect::new(x - WALL_T / 2.0, z0.min(z1), WALL_T, (z1 - z0).abs())
}

fn wall_h(z: f32, x0: f32, x1: f32) -> Rect {
    Rect::new(x0.min(x1), z - WALL_T / 2.0, (x1 - x0).abs(), WALL_T)
}

fn wall_v_door(out: &mut Vec<(Rect, Solid)>, x: f32, z0: f32, z1: f32, door: f32, open: bool) {
    out.push((wall_v(x, z0, door - DOOR_HALF), Solid::Wall));
    out.push((wall_v(x, door + DOOR_HALF, z1), Solid::Wall));
    if !open {
        out.push((wall_v(x, door - DOOR_HALF, door + DOOR_HALF), Solid::Locked));
    }
}

impl Layout {
    pub fn new(number: u32) -> Self {
        let number = number.clamp(1, MAX_HOTEL);
        let i = number as usize - 1;
        let wings = [2u8, 3, 3, 4][i];
        let has_toilets = true;
        let has_cafe = number >= 2;
        let has_pool = number >= 3;
        let stalls_n = [2, 2, 3, 3][i];
        let rooms = (0..wings as usize * 2).map(room_rect).collect();
        let toilet_room = Rect::new(-HALL_W - SIDE_W, SIDE_N, SIDE_W, SIDE_S - SIDE_N);
        let stalls: Vec<Vec2> = (0..stalls_n)
            .map(|k| {
                vec2(
                    -HALL_W - SIDE_W + 0.75,
                    SIDE_N + 0.3 + STALL_H * (k as f32 + 0.5),
                )
            })
            .collect();
        let stall_front = stalls.iter().map(|s| vec2(s.x + 1.9, s.y)).collect();
        let cafe = Rect::new(HALL_W, SIDE_N, SIDE_W, SIDE_S - SIDE_N);
        let counter = Rect::new(HALL_W + SIDE_W - 0.9, SIDE_N + 0.6, 0.8, 3.2);
        let tables = if number == 2 {
            vec![vec2(HALL_W + 1.7, 2.4), vec2(HALL_W + 1.7, 5.6)]
        } else {
            vec![
                vec2(HALL_W + 1.7, 2.4),
                vec2(HALL_W + 1.7, 5.6),
                vec2(HALL_W + 3.6, 5.9),
            ]
        };
        let z0 = -(wings as f32) * WING_D;
        let garden = Rect::new(-HALL_W, z0 - GARDEN_D, HALL_W * 2.0, GARDEN_D);
        let pool = Rect::new(-4.6, z0 - GARDEN_D + 1.0, 7.0, 3.2);
        let sunbeds = (0..4)
            .map(|k| vec2(-4.2 + k as f32 * 1.9, z0 - 2.0))
            .collect();
        let mut lay = Self {
            number,
            theme: THEMES[i],
            wings,
            has_toilets,
            has_cafe,
            has_pool,
            rooms,
            desk: Rect::new(-1.6, 2.1, 3.2, 0.9),
            desk_zone: Rect::new(-1.4, 0.55, 2.8, 1.45),
            desk_front: queue_slot(0),
            desk_pile: vec2(2.5, 3.6),
            clerk_spot: vec2(0.0, 1.35),
            toilet_room,
            stalls,
            stall_front,
            toilet_wait: vec2(-HALL_W - 1.2, SIDE_S - 1.2),
            paper_shelf: vec2(-HALL_W - 0.7, SIDE_N + 0.7),
            toilet_pile: vec2(-HALL_W - 1.6, SIDE_S - 2.6),
            cafe,
            counter,
            counter_spot: vec2(counter.x - 0.75, counter.y + 1.0),
            tables,
            cafe_pile: vec2(counter.x - 0.6, SIDE_S - 0.9),
            garden,
            pool,
            sunbeds,
            towel_shelf: vec2(4.4, z0 - 1.6),
            pool_pile: vec2(4.4, z0 - 4.4),
            pads: vec![],
            room_price: [6, 10, 15, 22][i],
            patience: [42.0, 36.0, 30.0, 26.0][i],
            arrive_every: [6.5, 5.6, 4.9, 4.2][i],
            vip_share: [0.0, 0.0, 0.15, 0.3][i],
            extra_spots: [0, 0, 1, 1][i],
            max_level: [6, 7, 8, 9][i],
            xp_scale: [1.0, 1.25, 1.5, 1.8][i],
            guide_after: [0.0, 4.0, 8.0, 12.0][i],
        };
        lay.pads = lay.plan();
        lay
    }

    fn plan(&self) -> Vec<PadSpec> {
        let f = [1.0, 1.7, 2.6, 3.8][self.number as usize - 1];
        let mut v: Vec<(Build, u32, f32)> = vec![(Build::Room(1), 1, 12.0)];
        if self.has_toilets {
            v.push((Build::Toilets, 2, 30.0));
        }
        v.push((Build::Shoes(1), 2, 25.0));
        v.push((Build::Clerk, 3, 50.0));
        v.push((Build::Gate(1), 3, 40.0));
        v.push((Build::Room(2), 3, 45.0));
        v.push((Build::Room(3), 4, 55.0));
        if self.has_cafe {
            v.push((Build::Cafe, 3, 60.0));
        }
        v.push((Build::Stack(1), 4, 45.0));
        for k in 0..4u8 {
            v.push((Build::Decor(k, 2), 4 + k as u32 / 2, 40.0 + 8.0 * k as f32));
        }
        v.push((Build::Maid, 4, 80.0));
        v.push((Build::Janitor, 5, 80.0));
        if self.has_cafe {
            v.push((Build::Waiter, 5, 90.0));
        }
        v.push((Build::Shoes(2), 5, 70.0));
        for w in 2..self.wings {
            v.push((Build::Gate(w), 4 + w as u32, 70.0 + 20.0 * w as f32));
            v.push((Build::Room(w * 2), 4 + w as u32, 80.0 + 20.0 * w as f32));
            v.push((Build::Room(w * 2 + 1), 5 + w as u32, 90.0 + 20.0 * w as f32));
        }
        if self.has_pool {
            v.push((Build::Pool, 6, 120.0));
            v.push((Build::Lifeguard, 7, 120.0));
        }
        v.push((Build::Stack(2), 6, 90.0));
        v.push((Build::Mop, 6, 100.0));
        for k in 0..4u8 {
            v.push((Build::Decor(k, 3), 6, 80.0 + 10.0 * k as f32));
        }
        for k in 4..self.wings * 2 {
            v.push((Build::Decor(k, 2), 7, 90.0));
        }
        v.push((Build::Next, self.max_level, 260.0));
        v.retain(|(_, l, _)| *l <= self.max_level);
        v.sort_by_key(|(_, l, _)| *l);
        v.into_iter()
            .map(|(build, level, cost)| PadSpec {
                build,
                level,
                cost: ((cost * f) / 5.0).round() as u32 * 5,
                pos: self.pad_pos(build),
            })
            .collect()
    }

    pub fn end_z(&self) -> f32 {
        -(self.wings as f32) * WING_D
    }

    fn pad_pos(&self, b: Build) -> Vec2 {
        match b {
            Build::Room(k) => {
                let dz = if k.is_multiple_of(2) { 1.1 } else { -1.1 };
                room_door(k as usize) + vec2(-room_side(k as usize) * 0.15, dz)
            }
            Build::Decor(k, _) => self.pad_pos(Build::Room(k)),
            Build::Gate(w) => vec2(0.0, -(w as f32) * WING_D),
            Build::Pool => vec2(0.0, self.end_z()),
            Build::Toilets => vec2(-HALL_W + 1.3, SIDE_DOOR),
            Build::Cafe => vec2(HALL_W - 1.3, SIDE_DOOR),
            Build::Clerk => vec2(-3.2, 1.6),
            Build::Maid => vec2(3.2, 1.6),
            Build::Janitor => vec2(-HALL_W + 1.0, 6.9),
            Build::Waiter => vec2(HALL_W - 1.0, 6.9),
            Build::Lifeguard => vec2(3.8, self.end_z() - 6.6),
            Build::Shoes(_) => vec2(-3.4, 5.4),
            Build::Stack(_) => vec2(3.4, 5.4),
            Build::Mop => vec2(-3.4, -1.2),
            Build::Next => vec2(3.6, 10.4),
        }
    }

    pub fn bounds(&self) -> Rect {
        let top = if self.has_pool {
            self.garden.y
        } else {
            self.end_z()
        };
        let x0 = if self.has_toilets {
            -HALL_W - SIDE_W
        } else {
            -HALL_W
        };
        let x1 = if self.has_cafe {
            HALL_W + SIDE_W
        } else {
            HALL_W
        };
        Rect::new(x0, top, x1 - x0, OUTSIDE_S - top)
    }

    pub fn has(&self, b: Build) -> bool {
        self.pads.iter().any(|p| p.build == b)
    }

    pub fn pad(&self, b: Build) -> Option<&PadSpec> {
        self.pads.iter().find(|p| p.build == b)
    }

    pub fn idle_spot(&self, b: Build) -> Vec2 {
        match b {
            Build::Clerk => self.clerk_spot,
            Build::Maid => vec2(-2.4, -0.9),
            Build::Janitor => self.paper_shelf + vec2(-1.2, 1.4),
            Build::Waiter => self.counter_spot + vec2(-0.8, 1.2),
            Build::Lifeguard => self.towel_shelf + vec2(-1.0, -0.9),
            _ => Vec2::ZERO,
        }
    }

    pub fn solids(&self, built: &dyn Fn(Build) -> bool) -> Vec<(Rect, Solid)> {
        let mut c = vec![];
        let end = self.end_z();
        let side_w = |b: Build, has: bool| has && built(b);
        if self.has_toilets {
            wall_v_door(
                &mut c,
                -HALL_W,
                0.0,
                LOBBY_S,
                SIDE_DOOR,
                built(Build::Toilets),
            );
            let r = self.toilet_room;
            c.push((wall_h(r.y, r.x, r.x + r.w), Solid::Wall));
            c.push((wall_h(r.y + r.h, r.x, r.x + r.w), Solid::Wall));
            c.push((wall_v(r.x, r.y, r.y + r.h), Solid::Wall));
            if built(Build::Toilets) {
                for k in 0..=self.stalls.len() {
                    let z = SIDE_N + 0.3 + STALL_H * k as f32;
                    c.push((wall_h(z, r.x, r.x + 1.6), Solid::Wall));
                }
            }
        } else {
            c.push((wall_v(-HALL_W, 0.0, LOBBY_S), Solid::Wall));
        }
        if self.has_cafe {
            wall_v_door(&mut c, HALL_W, 0.0, LOBBY_S, SIDE_DOOR, built(Build::Cafe));
            let r = self.cafe;
            c.push((wall_h(r.y, r.x, r.x + r.w), Solid::Wall));
            c.push((wall_h(r.y + r.h, r.x, r.x + r.w), Solid::Wall));
            c.push((wall_v(r.x + r.w, r.y, r.y + r.h), Solid::Wall));
            if built(Build::Cafe) {
                c.push((self.counter, Solid::Thing));
                for t in &self.tables {
                    c.push((Rect::new(t.x - 0.45, t.y - 0.45, 0.9, 0.9), Solid::Thing));
                }
            }
        } else {
            c.push((wall_v(HALL_W, 0.0, LOBBY_S), Solid::Wall));
        }
        c.push((wall_h(LOBBY_S, -HALL_W, -CORR), Solid::Wall));
        c.push((wall_h(LOBBY_S, CORR, HALL_W), Solid::Wall));
        c.push((wall_h(0.0, -HALL_W, -CORR), Solid::Wall));
        c.push((wall_h(0.0, CORR, HALL_W), Solid::Wall));
        c.push((self.desk, Solid::Thing));
        for w in 0..self.wings as usize {
            let z1 = -(w as f32) * WING_D;
            let z0 = z1 - WING_D;
            let zc = wing_z(w);
            c.push((wall_v(-HALL_W, z0, z1), Solid::Wall));
            c.push((wall_v(HALL_W, z0, z1), Solid::Wall));
            if w > 0 {
                c.push((wall_h(z1, -HALL_W, -CORR), Solid::Wall));
                c.push((wall_h(z1, CORR, HALL_W), Solid::Wall));
                if !built(Build::Gate(w as u8)) {
                    c.push((wall_h(z1, -CORR, CORR), Solid::Locked));
                }
            }
            for side in 0..2 {
                let r = w * 2 + side;
                let open = r == 0 || built(Build::Room(r as u8));
                let x = room_side(r) * CORR;
                wall_v_door(&mut c, x, z0, z1, zc, open);
                if open {
                    let b = room_bed(r);
                    c.push((Rect::new(b.x - 1.05, b.y - 0.75, 2.1, 1.5), Solid::Thing));
                }
            }
        }
        c.push((wall_h(end, -HALL_W, -CORR), Solid::Wall));
        c.push((wall_h(end, CORR, HALL_W), Solid::Wall));
        if !side_w(Build::Pool, self.has_pool) {
            c.push((wall_h(end, -CORR, CORR), Solid::Locked));
        }
        if self.has_pool {
            let g = self.garden;
            c.push((wall_v(g.x, g.y, g.y + g.h), Solid::Wall));
            c.push((wall_v(g.x + g.w, g.y, g.y + g.h), Solid::Wall));
            c.push((wall_h(g.y, g.x, g.x + g.w), Solid::Wall));
            if built(Build::Pool) {
                c.push((self.pool, Solid::Thing));
            }
        }
        c
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_hotel_plans_its_next_hotel_last_at_its_top_level() {
        for n in 1..=MAX_HOTEL {
            let lay = Layout::new(n);
            let last = lay.pads.last().unwrap();
            assert_eq!(last.build, Build::Next);
            assert_eq!(last.level, lay.max_level);
            assert!(lay.pads.windows(2).all(|w| w[0].level <= w[1].level));
        }
    }

    #[test]
    fn build_spots_never_overlap() {
        for n in 1..=MAX_HOTEL {
            let lay = Layout::new(n);
            for (i, a) in lay.pads.iter().enumerate() {
                for b in &lay.pads[i + 1..] {
                    let same_chain = matches!(
                        (a.build, b.build),
                        (Build::Shoes(_), Build::Shoes(_)) | (Build::Stack(_), Build::Stack(_))
                    ) || matches!((a.build, b.build), (Build::Decor(x, _), Build::Decor(y, _)) if x == y)
                        || matches!((a.build, b.build), (Build::Room(x), Build::Decor(y, _)) | (Build::Decor(y, _), Build::Room(x)) if x == y);
                    if !same_chain {
                        assert!(
                            (a.pos - b.pos).length() > 1.9,
                            "hotel {n}: {:?} and {:?} overlap",
                            a.build,
                            b.build
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn room_upgrades_sit_in_the_corridor_by_the_room_door() {
        for n in 1..=MAX_HOTEL {
            let lay = Layout::new(n);
            for p in &lay.pads {
                if let Build::Decor(k, _) = p.build {
                    let k = k as usize;
                    assert!(
                        p.pos.x.abs() < CORR,
                        "hotel {n}: room {k} upgrade is inside the room"
                    );
                    assert!(
                        (p.pos - room_door(k)).length() < 1.6,
                        "hotel {n}: room {k} upgrade far from its door"
                    );
                    for sp in room_spots(k, 3, 1) {
                        assert!(
                            (sp.stand - p.pos).length() > 1.6,
                            "hotel {n}: room {k} upgrade on a cleaning spot"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn rooms_open_onto_the_corridor_and_nothing_blocks_a_door() {
        for n in 1..=MAX_HOTEL {
            let lay = Layout::new(n);
            let all = lay.solids(&|_| true);
            for r in 0..lay.rooms.len() {
                let door = room_door(r);
                let inside = room_pt(r, 0.6, 0.0);
                for (s, _) in &all {
                    for k in 0..=10 {
                        let p = door.lerp(inside, k as f32 / 10.0);
                        assert!(!s.contains(p), "hotel {n} room {r} door blocked by {s:?}");
                    }
                }
                assert!(door.x.abs() < CORR, "door {r} not in the corridor");
            }
        }
    }

    #[test]
    fn later_hotels_ask_for_more() {
        let lays: Vec<Layout> = (1..=MAX_HOTEL).map(Layout::new).collect();
        for w in lays.windows(2) {
            let (a, b) = (&w[0], &w[1]);
            assert!(b.patience < a.patience);
            assert!(b.arrive_every < a.arrive_every);
            assert!(b.max_level > a.max_level);
            assert!(b.xp_scale > a.xp_scale);
            assert!(b.pads[0].cost > a.pads[0].cost);
            assert!(b.rooms.len() >= a.rooms.len());
        }
    }

    #[test]
    fn upgrading_a_room_adds_things_to_clean() {
        assert_eq!(room_spots(0, 1, 0).len(), 2);
        assert_eq!(room_spots(0, 2, 0).len(), 4);
        assert_eq!(room_spots(0, 3, 0).len(), 5);
        assert_eq!(room_spots(0, 1, 1).len(), 3);
    }
}
