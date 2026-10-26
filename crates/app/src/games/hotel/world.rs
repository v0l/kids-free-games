use super::build::{Scene, Xf};
use super::models::{
    bed, bin, burger, car, coin_stack, darker, desk, dust, guest_look, lamp, locked_door, person, plant,
    player_look, rgb, roll, rug, shadow, shelf, sofa, staff_look, swimmer, toilet, towel, tv, umbrella,
    wake, wardrobe, Pose,
};
use super::SHEET;
use bevy::math::Vec3;
use glam::{vec2, vec3, Vec2};
use kg_core::games::hotel::layout::*;
use kg_core::games::hotel::sim::*;
use kg_core::games::hotel::Hotel;
use kg_core::geom::{Color, Rect, WHITE};

const GOLD: Color = Color::new(1.0, 0.82, 0.24, 1.0);
const PLUM: Color = Color::new(0.24, 0.12, 0.3, 1.0);
const SHOE_COLORS: [Color; 2] = [
    Color::new(0.2, 0.6, 1.0, 1.0),
    Color::new(1.0, 0.75, 0.1, 1.0),
];

pub struct World<'a> {
    pub h: &'a Hotel,
}

pub fn stage_sits(st: Stage) -> bool {
    matches!(st, Stage::WaitSnack(_) | Stage::Eat(..) | Stage::Toilet(..))
}

impl World<'_> {
    fn zone_open(&self, b: Build) -> bool {
        self.h.sim.has(b)
    }

    fn draw_floors(&self, s: &mut Scene) {
        let lay = &self.h.sim.lay;
        let th = lay.theme;
        let hall = Rect::new(-HALL_W, lay.end_z(), HALL_W * 2.0, LOBBY_S - lay.end_z());
        s.floor(hall, 2.5, SHEET, "parquet", th.floor_tint);
        if lay.has_toilets && self.zone_open(Build::Toilets) {
            s.floor(lay.toilet_room, 2.5, SHEET, "parquet", rgb(210, 235, 245));
        }
        if lay.has_cafe && self.zone_open(Build::Cafe) {
            s.floor(lay.cafe, 2.5, SHEET, "parquet", rgb(255, 225, 190));
        }
        for (k, r) in lay.rooms.iter().enumerate() {
            let tint = if self.h.sim.room_open(k) {
                th.room_floor
            } else {
                Color::new(0.5, 0.5, 0.55, 1.0)
            };
            s.floor(*r, 2.0, SHEET, "roomfloor", tint);
        }
    }

    pub fn draw_world(&self, s: &mut Scene) {
        let sim = &self.h.sim;
        let lay = &sim.lay;
        let th = lay.theme;
        s.flat_rect(
            vec2(0.0, (LOBBY_S + OUTSIDE_S) / 2.0 + 1.0),
            vec2(4.0, OUTSIDE_S - LOBBY_S + 2.0),
            0.0,
            th.path,
        );
        s.flat_rect(
            vec2(0.0, (LOBBY_S + lay.end_z()) / 2.0),
            vec2(CORR * 2.0 - 0.3, LOBBY_S - lay.end_z()),
            0.004,
            th.carpet,
        );
        let locked_zone = Color::new(0.3, 0.3, 0.35, 0.35);
        if lay.has_toilets && !self.zone_open(Build::Toilets) {
            let r = lay.toilet_room;
            s.flat_rect(
                vec2(r.x + r.w / 2.0, r.y + r.h / 2.0),
                vec2(r.w, r.h),
                0.003,
                locked_zone,
            );
        }
        if lay.has_cafe && !self.zone_open(Build::Cafe) {
            let r = lay.cafe;
            s.flat_rect(
                vec2(r.x + r.w / 2.0, r.y + r.h / 2.0),
                vec2(r.w, r.h),
                0.003,
                locked_zone,
            );
        }
        if lay.has_pool {
            let g = lay.garden;
            s.flat_rect(
                vec2(g.x + g.w / 2.0, g.y + g.h / 2.0),
                vec2(g.w, g.h),
                0.002,
                if self.zone_open(Build::Pool) {
                    rgb(235, 225, 200)
                } else {
                    darker(th.outside, 0.85)
                },
            );
        }
        for k in 0..3 {
            let x = [-4.6, 4.6, -4.6][k];
            let z = [OUTSIDE_S - 1.5, OUTSIDE_S - 1.5, OUTSIDE_S - 3.6][k];
            plant(s, vec2(x, z), 1.6);
        }
        for (i, c) in [rgb(240, 80, 80), rgb(80, 150, 240)].iter().enumerate() {
            car(s, vec2(-4.5 + i as f32 * 9.0, OUTSIDE_S - 0.2 + 1.6), *c);
        }
        self.draw_floors(s);
        self.draw_walls(s);
        desk(s, lay.desk, &th);
        sofa(
            s,
            vec2(-HALL_W + 0.7, 6.0),
            th.accent,
            std::f32::consts::FRAC_PI_2,
        );
        sofa(
            s,
            vec2(HALL_W - 0.7, 6.0),
            th.accent,
            -std::f32::consts::FRAC_PI_2,
        );
        plant(s, vec2(-CORR - 0.5, LOBBY_S - 0.5), 1.2);
        plant(s, vec2(CORR + 0.5, LOBBY_S - 0.5), 1.2);
        for r in 0..lay.rooms.len() {
            if sim.room_open(r) {
                self.draw_room(s, r);
            }
        }
        if lay.has_toilets && self.zone_open(Build::Toilets) {
            self.draw_toilets(s);
        }
        if lay.has_cafe && self.zone_open(Build::Cafe) {
            self.draw_cafe(s);
        }
        if lay.has_pool && self.zone_open(Build::Pool) {
            self.draw_pool(s);
        }
        for p in PILES {
            let amount = sim.pile(p);
            if amount > 0 {
                coin_stack(s, sim.pile_pos(p), amount, self.h.t);
            }
        }
        for (k, room) in sim.rooms.iter().enumerate() {
            if room.tip > 0 {
                coin_stack(s, room_pt(k, 1.6, 1.2), room.tip, self.h.t);
            }
        }
        self.draw_pads(s);
        self.draw_people(s);
    }

    fn draw_room(&self, s: &mut Scene, r: usize) {
        let sim = &self.h.sim;
        let th = sim.lay.theme;
        let decor = sim.decor(r);
        let pop = self.pop_of(Build::Room(r as u8)).min(
            self.pop_of(Build::Decor(r as u8, 2))
                .min(self.pop_of(Build::Decor(r as u8, 3))),
        );
        let outer = s.xf;
        let c = room_center(r);
        s.xf = Xf {
            origin: vec3(c.x, 0.0, c.y) * (1.0 - pop),
            yaw: 0.0,
            scale: Vec3::splat(pop),
        };
        let room = &sim.rooms[r];
        let spots = sim.spots(r);
        let dirty = |kind: SpotKind| {
            spots
                .iter()
                .position(|sp| sp.kind == kind)
                .map(|i| room.dirty[i])
                .unwrap_or(false)
        };
        let sheet = match decor {
            1 => rgb(150, 180, 230),
            2 => rgb(255, 255, 255),
            _ => th.accent,
        };
        let wood = match decor {
            1 => rgb(180, 120, 80),
            2 => th.trim,
            _ => rgb(250, 205, 80),
        };
        let bx = s.xf;
        s.xf = bx.then(Xf::at(
            vec3(room_bed(r).x, 0.0, room_bed(r).y),
            bed_yaw(r),
            if decor >= 3 { 1.12 } else { 1.0 },
        ));
        bed(
            s,
            Vec2::ZERO,
            if dirty(SpotKind::Bed) {
                rgb(200, 190, 175)
            } else {
                sheet
            },
            wood,
            decor >= 3,
        );
        s.xf = bx;
        if dirty(SpotKind::Bed) {
            let b = room_bed(r);
            s.sphere(
                vec3(b.x, 0.68, b.y + 0.3),
                vec3(0.5, 0.15, 0.4),
                4,
                rgb(220, 215, 205),
            );
        }
        let bin_at = spots
            .iter()
            .find(|sp| sp.kind == SpotKind::Bin)
            .map(|sp| sp.obj)
            .unwrap_or_default();
        bin(s, bin_at, dirty(SpotKind::Bin));
        plant(s, room_pt(r, 4.0, 2.2), 1.0);
        if decor >= 2 || sim.lay.extra_spots > 0 {
            let at = room_pt(r, 1.5, 1.0);
            rug(
                s,
                at,
                if decor >= 3 {
                    th.accent
                } else {
                    rgb(240, 200, 120)
                },
            );
            if dirty(SpotKind::Rug) {
                dust(s, at, self.h.t);
            }
        }
        if decor >= 2 {
            let at = room_pt(r, 4.0, -2.15);
            wardrobe(s, at, wood, 0.0);
            lamp(s, room_pt(r, 4.2, 1.4), th.accent);
            if dirty(SpotKind::Wardrobe) {
                dust(s, room_pt(r, 3.4, -1.45), self.h.t);
            }
        }
        if decor >= 3 {
            tv(s, room_pt(r, 0.3, 2.1), std::f32::consts::PI);
            if dirty(SpotKind::Tv) {
                dust(s, room_pt(r, 0.95, 1.6), self.h.t);
            }
        }
        s.xf = outer;
    }

    fn draw_toilets(&self, s: &mut Scene) {
        let sim = &self.h.sim;
        let lay = &sim.lay;
        let r = lay.toilet_room;
        for (k, st) in lay.stalls.iter().enumerate() {
            let outer = s.xf;
            s.xf = outer.then(Xf::at(
                vec3(st.x, 0.0, st.y),
                std::f32::consts::FRAC_PI_2,
                1.0,
            ));
            toilet(s, Vec2::ZERO);
            s.xf = outer;
            for q in 0..sim.stalls[k].paper {
                roll(s, vec3(r.x + 0.2, 0.55 + q as f32 * 0.22, st.y - 0.45));
            }
        }
        for k in 0..=lay.stalls.len() {
            let z = SIDE_N + 0.3 + STALL_H * k as f32;
            s.cuboid(
                vec3(r.x + 0.8, 0.0, z),
                vec3(1.6, 1.3, 0.1),
                rgb(120, 200, 235),
            );
        }
        shelf(s, lay.paper_shelf, WHITE);
        for q in 0..3 {
            roll(
                s,
                vec3(
                    lay.paper_shelf.x - 0.15 + q as f32 * 0.15,
                    0.95,
                    lay.paper_shelf.y,
                ),
            );
        }
        s.cuboid2(
            vec3(r.x + r.w - 0.5, 0.0, r.y + r.h - 0.9),
            vec3(0.6, 0.8, 1.2),
            WHITE,
            rgb(220, 240, 250),
        );
    }

    fn draw_cafe(&self, s: &mut Scene) {
        let lay = &self.h.sim.lay;
        let th = lay.theme;
        let c = lay.counter;
        s.cuboid2(
            vec3(c.x + c.w / 2.0, 0.0, c.y + c.h / 2.0),
            vec3(c.w, 1.0, c.h),
            rgb(240, 140, 60),
            rgb(255, 215, 130),
        );
        for k in 0..3 {
            burger(s, vec3(c.x + c.w / 2.0, 1.0, c.y + 0.5 + k as f32 * 1.0));
        }
        for tb in &lay.tables {
            s.cylinder(vec3(tb.x, 0.0, tb.y), 0.08, 0.7, 6, rgb(90, 90, 100), WHITE);
            s.cylinder(
                vec3(tb.x, 0.7, tb.y),
                0.5,
                0.06,
                14,
                rgb(230, 230, 235),
                WHITE,
            );
            for p in seat_spots(*tb) {
                s.cylinder(vec3(p.x, 0.0, p.y), 0.25, 0.42, 8, th.accent, th.accent);
            }
        }
    }

    fn draw_pool(&self, s: &mut Scene) {
        let lay = &self.h.sim.lay;
        let th = lay.theme;
        let p = lay.pool;
        let c = vec2(p.x + p.w / 2.0, p.y + p.h / 2.0);
        s.flat_rect(c, vec2(p.w + 0.9, p.h + 0.9), 0.004, rgb(245, 245, 252));
        s.flat_rect(c, vec2(p.w, p.h), 0.008, rgb(40, 160, 230));
        for k in 0..6 {
            let x = p.x + 0.6 + k as f32 * (p.w - 1.2) / 5.0;
            let z = p.y + p.h / 2.0 + (self.h.t * 1.3 + k as f32).sin() * (p.h * 0.3);
            s.disc(
                vec2(x, z),
                vec2(0.5, 0.12),
                0.01,
                Color::new(0.6, 0.88, 1.0, 0.6),
            );
        }
        for k in 1..3 {
            let z = p.y + k as f32 * p.h / 3.0;
            s.flat_rect(
                vec2(c.x, z),
                vec2(p.w - 0.4, 0.05),
                0.011,
                rgb(255, 240, 120),
            );
        }
        let rim = rgb(250, 250, 255);
        let e = 0.22;
        s.cuboid(
            vec3(c.x, 0.0, p.y - e / 2.0),
            vec3(p.w + 2.0 * e, 0.16, e),
            rim,
        );
        s.cuboid(
            vec3(c.x, 0.0, p.y + p.h + e / 2.0),
            vec3(p.w + 2.0 * e, 0.16, e),
            rim,
        );
        s.cuboid(vec3(p.x - e / 2.0, 0.0, c.y), vec3(e, 0.16, p.h), rim);
        s.cuboid(vec3(p.x + p.w + e / 2.0, 0.0, c.y), vec3(e, 0.16, p.h), rim);
        for (k, sb) in lay.sunbeds.iter().enumerate() {
            s.cuboid2(
                vec3(sb.x, 0.0, sb.y),
                vec3(0.7, 0.3, 1.5),
                WHITE,
                if k % 2 == 0 {
                    th.accent
                } else {
                    rgb(255, 210, 80)
                },
            );
            if k % 2 == 0 {
                umbrella(s, *sb + vec2(0.8, -0.3), th.trim);
            }
        }
        shelf(s, lay.towel_shelf, rgb(120, 190, 250));
        for sp in &self.h.splashes {
            let k = sp.age / 0.8;
            for i in 0..6 {
                let a = i as f32 / 6.0 * std::f32::consts::TAU;
                let r = 0.2 + k * 0.6;
                let up = (k * std::f32::consts::PI).sin() * 0.6;
                s.sphere(
                    vec3(sp.at.x + a.cos() * r, 0.1 + up, sp.at.y + a.sin() * r),
                    Vec3::splat(0.08 * (1.0 - k) + 0.02),
                    2,
                    Color::new(0.85, 0.95, 1.0, 1.0 - k),
                );
            }
            s.ring_flat(
                sp.at,
                0.3 + k * 0.9,
                0.08,
                0.012,
                1.0,
                Color::new(1.0, 1.0, 1.0, 0.7 * (1.0 - k)),
            );
        }
    }

    fn draw_walls(&self, s: &mut Scene) {
        let th = self.h.sim.lay.theme;
        let wall_h = 1.2;
        let things: Vec<Rect> = {
            let built: Vec<Build> = self
                .h
                .sim
                .lay
                .pads
                .iter()
                .zip(&self.h.sim.built)
                .filter(|(_, b)| **b)
                .map(|(p, _)| p.build)
                .collect();
            self.h
                .sim
                .lay
                .solids(&|b| built.contains(&b))
                .into_iter()
                .filter(|(_, k)| *k != Solid::Wall)
                .map(|(r, _)| r)
                .collect()
        };
        for w in self.h.sim.walls() {
            if things.contains(w) {
                continue;
            }
            let c = vec3(w.x + w.w / 2.0, 0.0, w.y + w.h / 2.0);
            s.cuboid2(c, vec3(w.w, wall_h, w.h), th.wall, th.trim);
        }
        for w in self.h.sim.locked() {
            locked_door(s, *w, &th);
        }
        for side in [-1.0, 1.0] {
            s.cuboid2(
                vec3(side * (CORR + 0.15), 0.0, LOBBY_S),
                vec3(0.3, 2.0, 0.34),
                th.trim,
                darker(th.trim, 1.1),
            );
        }
        s.cuboid2(
            vec3(0.0, 2.0, LOBBY_S),
            vec3(CORR * 2.0 + 0.6, 0.35, 0.34),
            th.trim,
            darker(th.trim, 1.1),
        );
    }

    fn pop_of(&self, b: Build) -> f32 {
        self.h.pop_of(b)
    }

    fn draw_pads(&self, s: &mut Scene) {
        let sim = &self.h.sim;
        for i in sim.visible_pads() {
            let p = &sim.lay.pads[i];
            let k = sim.paid[i] as f32 / p.cost as f32;
            let pulse = 1.0 + (self.h.t * 3.0).sin() * 0.03;
            let afford = sim.wallet + sim.paid[i] >= p.cost;
            let base = if afford {
                rgb(255, 255, 255)
            } else {
                Color::new(1.0, 1.0, 1.0, 0.55)
            };
            s.flat_rect(p.pos, vec2(1.5, 1.5) * pulse, 0.02, PLUM.with_alpha(0.4));
            s.ring_flat(p.pos, 0.75 * pulse, 0.1, 0.03, 1.0, base);
            if k > 0.0 {
                s.ring_flat(p.pos, 0.75 * pulse, 0.1, 0.035, k, rgb(120, 230, 90));
            }
            if sim.pad_under_player() == Some(i) && sim.pad_arm < PAD_ARM && afford {
                s.ring_flat(p.pos, 0.6, 0.12, 0.04, sim.pad_arm / PAD_ARM, GOLD);
            }
        }
    }

    fn draw_people(&self, s: &mut Scene) {
        let sim = &self.h.sim;
        let water = 0.02;
        for g in &sim.guests {
            let look = guest_look(g.look, g.vip);
            if let Stage::Swim(_, t) = g.stage {
                let at = swim_pos(&sim.lay.pool, t, g.id);
                let face = swim_dir(t);
                wake(s, at, water, face, self.h.t);
                swimmer(s, at, water, face, self.h.t + g.id as f32, &look);
                continue;
            }
            let lie = matches!(g.stage, Stage::Sleep(..));
            let sit = stage_sits(g.stage);
            let mut pos = g.pos;
            let mut face = g.face;
            if let Stage::Sleep(k, _) = g.stage {
                pos = room_bed(k);
                face = bed_yaw(k) + std::f32::consts::PI;
            }
            if let Stage::WaitTowel(b) = g.stage {
                pos = sim.lay.sunbeds[b] + vec2(0.0, 0.7);
            }
            if !lie {
                shadow(s, pos, 0.36);
            }
            let hop = if g.happy && !g.path.is_empty() {
                (self.h.t * 9.0 + g.id as f32).sin().abs() * 0.05
            } else {
                0.0
            };
            person(
                s,
                pos,
                face,
                0.85,
                &look,
                Pose {
                    walk: if g.path.is_empty() { 0.0 } else { g.walk },
                    sit,
                    lie,
                    arms_up: matches!(g.stage, Stage::Eat(..)),
                    hop,
                    ..Default::default()
                },
            );
            if let Stage::Eat(..) = g.stage {
                burger(
                    s,
                    vec3(pos.x, 1.15, pos.y) + vec3(face.sin(), 0.0, face.cos()) * 0.2,
                );
            }
        }
        for st in &sim.staff {
            let kind = match st.role {
                Build::Clerk => 0,
                Build::Maid => 1,
                Build::Janitor => 2,
                Build::Waiter => 3,
                _ => 4,
            };
            shadow(s, st.pos, 0.36);
            person(
                s,
                st.pos,
                st.face,
                0.85,
                &staff_look(kind),
                Pose {
                    walk: if st.path.is_empty() { 0.0 } else { st.walk },
                    sweep: if st.role == Build::Maid && st.path.is_empty() {
                        st.work
                    } else {
                        0.0
                    },
                    ..Default::default()
                },
            );
            if let Some(item) = st.carry {
                let hand = st.pos + vec2(st.face.sin(), st.face.cos()) * 0.32;
                carried(s, item, vec3(hand.x, 0.95, hand.y), 1);
            }
        }
        let p = &sim.player;
        shadow(s, p.pos, 0.4);
        let squash = if p.moving {
            0.0
        } else {
            (self.h.t * 2.4).sin() * 0.015
        };
        let shoes = match sim.shoes() {
            0 => None,
            l => Some(SHOE_COLORS[l - 1]),
        };
        person(
            s,
            p.pos,
            p.face,
            1.0,
            &player_look(),
            Pose {
                walk: if p.moving { p.walk } else { 0.0 },
                arms_up: p.carry.is_some(),
                squash,
                shoes,
                sweep: p.work,
                ..Default::default()
            },
        );
        if p.moving && sim.shoes() > 0 {
            let back = p.pos - vec2(p.face.sin(), p.face.cos()) * 0.45;
            s.disc(
                back,
                vec2(0.18, 0.14),
                0.02,
                SHOE_COLORS[sim.shoes() - 1].with_alpha(0.45),
            );
        }
        if let Some(item) = p.carry {
            let hand = p.pos + vec2(p.face.sin(), p.face.cos()) * 0.3;
            carried(s, item, vec3(hand.x, 1.2, hand.y), p.count);
        }
    }
}


fn carried(s: &mut Scene, item: Item, at: Vec3, n: u8) {
    for k in 0..n.max(1) {
        let p = at + vec3(0.0, k as f32 * 0.2, 0.0);
        match item {
            Item::Paper => roll(s, p),
            Item::Snack => burger(s, p),
            Item::Towel => towel(s, p),
        }
    }
}

