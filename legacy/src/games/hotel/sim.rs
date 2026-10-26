use macroquad::prelude::*;

pub const WORLD_W: f32 = 1200.0;
pub const WORLD_H: f32 = 800.0;
pub const ROOMS: usize = 8;
pub const ROOM_W: f32 = 185.0;
pub const ROOM_H: f32 = 250.0;
pub const CORRIDOR_Y: f32 = 300.0;
pub const EXIT_X: f32 = 385.0;
pub const DESK: Vec2 = Vec2::new(200.0, 300.0);
pub const SERVE: Vec2 = Vec2::new(200.0, 215.0);
pub const CASH: Vec2 = Vec2::new(335.0, 360.0);
pub const DOOR: Vec2 = Vec2::new(200.0, 790.0);
pub const CART: Vec2 = Vec2::new(520.0, 700.0);
pub const CLERK_SPOT: Vec2 = Vec2::new(70.0, 240.0);
pub const SOFA_SPOT: Vec2 = Vec2::new(720.0, 700.0);
pub const PALM_SPOT: Vec2 = Vec2::new(900.0, 700.0);
pub const MAID_SPOT: Vec2 = Vec2::new(1100.0, 700.0);
pub const PLAYER_SPEED: f32 = 360.0;
pub const GUEST_SPEED: f32 = 200.0;
pub const MAID_SPEED: f32 = 190.0;
pub const REACH: f32 = 75.0;
pub const SPOT_R: f32 = 60.0;
pub const CHECKIN: f32 = 0.55;
pub const CLERK_CHECKIN: f32 = 1.8;
pub const CLEAN: f32 = 0.9;
pub const MAID_CLEAN: f32 = 2.6;
pub const TOWEL_MAX: u8 = 3;
pub const QUEUE_MAX: usize = 5;
pub const QUEUE_GAP: f32 = 88.0;
pub const LOOKS: usize = 5;
pub const MAX_DAY: u32 = 20;
pub const PAY_TIME: f32 = 1.2;
pub const DECOR_BONUS: u32 = 4;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Day {
    pub number: u32,
    pub target: u32,
    pub limit: Option<f32>,
    pub open_rooms: usize,
    pub max_rooms: usize,
    pub patience: Option<f32>,
    pub gap: f32,
    pub stay: f32,
    pub price: u32,
    pub towels: bool,
    pub clerk: bool,
    pub maid: bool,
    pub decor: usize,
}

impl Day {
    pub fn for_level(number: u32, eased: bool) -> Self {
        let n = number.clamp(1, MAX_DAY);
        let k = (n - 1) as f32;
        let mut d = Self {
            number: n,
            target: match n {
                1 => 60,
                2 => 110,
                _ => (150.0 + 30.0 * (k - 2.0) + 1.3 * (k - 2.0) * (k - 2.0)) as u32,
            },
            limit: (n > 2).then_some(120.0 + 2.0 * k),
            open_rooms: if n == 1 {
                1
            } else {
                (2 + (n as usize - 2) / 5).min(4)
            },
            max_rooms: (2 + n as usize / 2).min(ROOMS),
            patience: match n {
                1 => None,
                _ => Some((42.0 - 1.4 * k).max(17.0)),
            },
            gap: (5.0 - 0.17 * k).max(1.9),
            stay: (8.0 + 0.1 * k).min(10.0),
            price: 10 + 2 * ((n - 1) / 3),
            towels: n >= 4,
            clerk: n >= 3,
            maid: n >= 5,
            decor: (n >= 6) as usize + (n >= 9) as usize,
        };
        if eased {
            d.target = d.target * 4 / 5;
            d.limit = d.limit.map(|l| l * 1.2);
            d.patience = d.patience.map(|p| p * 1.5);
            d.gap *= 1.15;
        }
        d
    }

    pub fn goal_line(&self) -> String {
        match self.limit {
            Some(l) => format!("Earn ${} in {}", self.target, clock(l)),
            None => format!("Earn ${}", self.target),
        }
    }
}

pub fn clock(secs: f32) -> String {
    let s = secs.ceil().max(0.0) as u32;
    format!("{}:{:02}", s / 60, s % 60)
}

pub fn stars_for_leavers(left: u32) -> u8 {
    match left {
        0 => 3,
        1..=2 => 2,
        _ => 1,
    }
}

pub fn day_score(day: &Day, stars: u8, seconds_left: f32) -> i64 {
    day.target as i64 + stars as i64 * 50 + seconds_left.max(0.0) as i64 * 2
}

pub fn room_rect(i: usize) -> Rect {
    let col = (i % 4) as f32;
    let y = if i < 4 { 10.0 } else { 340.0 };
    Rect::new(420.0 + col * 195.0, y, ROOM_W, ROOM_H)
}

pub fn room_door(i: usize) -> Vec2 {
    vec2(room_rect(i).center().x, CORRIDOR_Y)
}

pub fn bed_pos(i: usize) -> Vec2 {
    let r = room_rect(i);
    if i < 4 {
        vec2(r.center().x, r.y + 150.0)
    } else {
        vec2(r.center().x, r.y + 140.0)
    }
}

pub fn clean_area(i: usize) -> Rect {
    let r = room_rect(i);
    Rect::new(r.x + 10.0, r.y + 40.0, r.w - 20.0, r.h - 50.0)
}

pub fn queue_slot(k: usize) -> Vec2 {
    vec2(DESK.x, DESK.y + 95.0 + k as f32 * QUEUE_GAP)
}

pub fn exit_path(from_room: Option<usize>, from: Vec2) -> Vec<Vec2> {
    let mut p = vec![];
    match from_room {
        Some(i) => {
            p.push(room_door(i));
            p.push(vec2(EXIT_X, CORRIDOR_Y + 40.0));
        }
        None => p.push(vec2(EXIT_X, from.y)),
    }
    p.push(vec2(EXIT_X, 750.0));
    p.push(DOOR + vec2(0.0, 30.0));
    p
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Room {
    Locked,
    Clean,
    Taken,
    Dirty { done: f32 },
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Stage {
    Waiting,
    ToRoom(usize),
    Asleep(usize, f32),
    Leaving { happy: bool },
}

#[derive(Clone, Debug)]
pub struct Guest {
    pub id: u32,
    pub look: u8,
    pub pos: Vec2,
    pub path: Vec<Vec2>,
    pub stage: Stage,
    pub patience: f32,
    pub max: f32,
    pub walk: f32,
    pub face: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Build {
    Room(usize),
    Clerk,
    Maid,
    Sofa,
    Palm,
}

impl Build {
    pub fn is_staff(self) -> bool {
        matches!(self, Build::Clerk | Build::Maid)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Spot {
    pub build: Build,
    pub price: u32,
    pub paid: u32,
    pub done: bool,
}

impl Spot {
    pub fn pos(&self) -> Vec2 {
        match self.build {
            Build::Room(i) => bed_pos(i),
            Build::Clerk => CLERK_SPOT,
            Build::Maid => MAID_SPOT,
            Build::Sofa => SOFA_SPOT,
            Build::Palm => PALM_SPOT,
        }
    }

    pub fn left(&self) -> u32 {
        self.price - self.paid
    }
}

#[derive(Clone, Debug)]
pub struct Walker {
    pub pos: Vec2,
    pub target: Option<Vec2>,
    pub steer: Option<Vec2>,
    pub walk: f32,
    pub face: f32,
    pub moving: bool,
    pub still: f32,
}

impl Walker {
    fn at(pos: Vec2) -> Self {
        Self {
            pos,
            target: None,
            steer: None,
            walk: 0.0,
            face: 1.0,
            moving: false,
            still: 0.0,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Maid {
    pub pos: Vec2,
    pub path: Vec<Vec2>,
    pub task: Option<usize>,
    pub walk: f32,
    pub face: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Task {
    Desk,
    Clean(usize),
    Towels,
    Cash,
    Build(usize),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Event {
    Arrived,
    CheckedIn(usize),
    Paid(u32),
    Collected(u32),
    Cleaned(usize),
    NeedTowels(usize),
    Towels,
    Paying(usize),
    Built(Build),
    Angry(Vec2),
    Halfway,
    Won,
    Lost,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Outcome {
    Playing,
    Won,
    Lost,
}

pub struct Sim {
    pub day: Day,
    pub rooms: [Room; ROOMS],
    pub guests: Vec<Guest>,
    pub spots: Vec<Spot>,
    pub player: Walker,
    pub towels: u8,
    pub maid: Option<Maid>,
    pub clerk: bool,
    pub decor: u32,
    pub wallet: u32,
    pub pile: u32,
    pub earned: u32,
    pub left: u32,
    pub served: u32,
    pub desk: f32,
    pub clock: Option<f32>,
    pub outcome: Outcome,
    pub events: Vec<Event>,
    pub t: f32,
    spawn_in: f32,
    next_id: u32,
    last_look: Option<u8>,
    pay_acc: f32,
    pay_fx: f32,
    towel_nag: f32,
    half_said: bool,
}

pub fn walk_along(pos: &mut Vec2, path: &mut Vec<Vec2>, speed: f32, dt: f32) -> Option<f32> {
    let mut budget = speed * dt;
    let start = *pos;
    while budget > 0.0 {
        let Some(&next) = path.first() else { break };
        let d = next - *pos;
        let len = d.length();
        if len <= budget {
            *pos = next;
            budget -= len;
            path.remove(0);
        } else {
            *pos += d / len * budget;
            budget = 0.0;
        }
    }
    let moved = *pos - start;
    (moved.length() > 0.01).then_some(moved.x)
}

impl Sim {
    pub fn new(day: Day) -> Self {
        let mut rooms = [Room::Locked; ROOMS];
        for r in rooms.iter_mut().take(day.open_rooms) {
            *r = Room::Clean;
        }
        let mut spots = vec![];
        for i in day.open_rooms..day.max_rooms {
            let k = (i - day.open_rooms) as u32;
            spots.push(Spot {
                build: Build::Room(i),
                price: 20 + 20 * k + 5 * k * k,
                paid: 0,
                done: false,
            });
        }
        let mut extra = |b: Build, price: u32| {
            spots.push(Spot {
                build: b,
                price,
                paid: 0,
                done: false,
            })
        };
        if day.clerk {
            extra(Build::Clerk, 60);
        }
        if day.maid {
            extra(Build::Maid, 90);
        }
        if day.decor >= 1 {
            extra(Build::Sofa, 50);
        }
        if day.decor >= 2 {
            extra(Build::Palm, 70);
        }
        Self {
            day,
            rooms,
            guests: vec![],
            spots,
            player: Walker::at(vec2(SERVE.x + 40.0, SERVE.y + 20.0)),
            towels: 0,
            maid: None,
            clerk: false,
            decor: 0,
            wallet: 0,
            pile: 0,
            earned: 0,
            left: 0,
            served: 0,
            desk: 0.0,
            clock: day.limit,
            outcome: Outcome::Playing,
            events: vec![],
            t: 0.0,
            spawn_in: 0.4,
            next_id: 0,
            last_look: None,
            pay_acc: 0.0,
            pay_fx: 0.0,
            towel_nag: 0.0,
            half_said: false,
        }
    }

    pub fn walk_to(&mut self, p: Vec2) {
        self.player.target = Some(clamp_world(p));
        self.player.steer = None;
    }

    pub fn steer(&mut self, dir: Option<Vec2>) {
        self.player.steer = dir;
        if dir.is_some() {
            self.player.target = None;
        }
    }

    pub fn price(&self) -> u32 {
        self.day.price + self.decor * DECOR_BONUS
    }

    pub fn waiting(&self) -> Vec<usize> {
        (0..self.guests.len())
            .filter(|i| self.guests[*i].stage == Stage::Waiting)
            .collect()
    }

    pub fn front_ready(&self) -> Option<usize> {
        let w = self.waiting();
        let g = *w.first()?;
        (self.guests[g].pos.distance(queue_slot(0)) < 4.0).then_some(g)
    }

    pub fn free_room(&self) -> Option<usize> {
        (0..ROOMS).find(|i| self.rooms[*i] == Room::Clean)
    }

    pub fn active_spot(&self, k: usize) -> bool {
        let s = &self.spots[k];
        if s.done {
            return false;
        }
        match s.build {
            Build::Room(i) => (0..i).all(|j| self.rooms[j] != Room::Locked),
            _ => true,
        }
    }

    pub fn at_desk(&self) -> bool {
        self.player.pos.distance(SERVE) < REACH
    }

    pub fn serving(&self) -> bool {
        self.front_ready().is_some() && self.free_room().is_some() && (self.at_desk() || self.clerk)
    }

    pub fn needs_towels(&self) -> bool {
        self.day.towels && self.towels == 0
    }

    fn spawn(&mut self) {
        let mut look = rand::gen_range(0, LOOKS as u8);
        if Some(look) == self.last_look {
            look = (look + 1 + rand::gen_range(0, LOOKS as u8 - 1)) % LOOKS as u8;
        }
        self.last_look = Some(look);
        let patience = self.day.patience.unwrap_or(f32::INFINITY);
        self.guests.push(Guest {
            id: self.next_id,
            look,
            pos: DOOR + vec2(0.0, 30.0),
            path: vec![],
            stage: Stage::Waiting,
            patience,
            max: patience,
            walk: 0.0,
            face: 1.0,
        });
        self.next_id += 1;
        self.events.push(Event::Arrived);
    }

    pub fn step(&mut self, dt: f32) {
        if self.outcome != Outcome::Playing {
            return;
        }
        self.t += dt;
        self.spawn_in -= dt;
        if self.spawn_in <= 0.0 {
            self.spawn_in = self.day.gap * rand::gen_range(0.8, 1.2);
            if self.waiting().len() < QUEUE_MAX {
                self.spawn();
            }
        }
        self.move_player(dt);
        self.move_guests(dt);
        self.check_in(dt);
        self.clean(dt);
        self.run_maid(dt);
        self.collect();
        self.pay(dt);
        self.finish(dt);
    }

    fn move_player(&mut self, dt: f32) {
        let p = &mut self.player;
        let before = p.pos;
        if let Some(d) = p.steer {
            let v = d.clamp_length_max(1.0);
            p.pos = clamp_world(p.pos + v * PLAYER_SPEED * dt);
        } else if let Some(t) = p.target {
            let mut path = vec![t];
            walk_along(&mut p.pos, &mut path, PLAYER_SPEED, dt);
            if path.is_empty() {
                p.target = None;
            }
        }
        let moved = p.pos - before;
        p.moving = moved.length() > 0.01;
        if p.moving {
            p.walk += dt;
            p.still = 0.0;
            if moved.x.abs() > 0.01 {
                p.face = moved.x.signum();
            }
        } else {
            p.walk = 0.0;
            p.still += dt;
        }
    }

    fn move_guests(&mut self, dt: f32) {
        let order = self.waiting();
        let ticking = self.day.patience.is_some();
        let serving = self.serving();
        for (k, gi) in order.iter().enumerate() {
            let g = &mut self.guests[*gi];
            g.path = vec![queue_slot(k)];
            let front = k == 0 && serving;
            if ticking && !front {
                g.patience -= dt;
            }
        }
        for g in &mut self.guests {
            if let Stage::Asleep(i, left) = g.stage {
                let left = left - dt;
                if left <= 0.0 {
                    g.stage = Stage::Leaving { happy: true };
                    g.path = exit_path(Some(i), g.pos);
                    self.rooms[i] = Room::Dirty { done: 0.0 };
                } else {
                    g.stage = Stage::Asleep(i, left);
                }
                continue;
            }
            if g.stage == Stage::Waiting && g.patience <= 0.0 {
                g.stage = Stage::Leaving { happy: false };
                g.path = exit_path(None, g.pos);
                self.left += 1;
                self.events.push(Event::Angry(g.pos));
            }
            match walk_along(&mut g.pos, &mut g.path, GUEST_SPEED, dt) {
                Some(dx) => {
                    g.walk += dt;
                    if dx.abs() > 0.01 {
                        g.face = dx.signum();
                    }
                }
                None => g.walk = 0.0,
            }
            if let Stage::ToRoom(i) = g.stage {
                if g.path.is_empty() {
                    g.stage = Stage::Asleep(i, self.day.stay * rand::gen_range(0.85, 1.15));
                }
            }
        }
        self.guests
            .retain(|g| !(matches!(g.stage, Stage::Leaving { .. }) && g.path.is_empty()));
    }

    fn check_in(&mut self, dt: f32) {
        if !self.serving() {
            self.desk = 0.0;
            return;
        }
        let rate = if self.at_desk() {
            1.0 / CHECKIN
        } else {
            1.0 / CLERK_CHECKIN
        };
        self.desk += dt * rate;
        if self.desk < 1.0 {
            return;
        }
        self.desk = 0.0;
        let (Some(g), Some(room)) = (self.front_ready(), self.free_room()) else {
            return;
        };
        self.rooms[room] = Room::Taken;
        let guest = &mut self.guests[g];
        guest.stage = Stage::ToRoom(room);
        guest.path = vec![room_door(room), bed_pos(room)];
        let pay = self.price();
        self.pile += pay;
        self.served += 1;
        self.events.push(Event::CheckedIn(room));
        self.events.push(Event::Paid(pay));
    }

    fn clean(&mut self, dt: f32) {
        self.towel_nag -= dt;
        if self.day.towels && self.player.pos.distance(CART) < REACH && self.towels < TOWEL_MAX {
            self.towels = TOWEL_MAX;
            self.events.push(Event::Towels);
        }
        let Some(i) = (0..ROOMS).find(|i| {
            clean_area(*i).contains(self.player.pos) && matches!(self.rooms[*i], Room::Dirty { .. })
        }) else {
            return;
        };
        if self.needs_towels() {
            if self.towel_nag <= 0.0 {
                self.towel_nag = 1.5;
                self.events.push(Event::NeedTowels(i));
            }
            return;
        }
        if let Room::Dirty { done } = self.rooms[i] {
            let done = done + dt / CLEAN;
            if done >= 1.0 {
                self.rooms[i] = Room::Clean;
                if self.day.towels {
                    self.towels -= 1;
                }
                self.events.push(Event::Cleaned(i));
            } else {
                self.rooms[i] = Room::Dirty { done };
            }
        }
    }

    fn run_maid(&mut self, dt: f32) {
        let Some(m) = &mut self.maid else { return };
        if let Some(i) = m.task {
            if !matches!(self.rooms[i], Room::Dirty { .. }) {
                m.task = None;
                m.path = vec![room_door(i), MAID_SPOT];
            }
        }
        if m.task.is_none() {
            let pick = (0..ROOMS)
                .filter(|i| matches!(self.rooms[*i], Room::Dirty { .. }))
                .min_by(|a, b| {
                    bed_pos(*a)
                        .distance(m.pos)
                        .total_cmp(&bed_pos(*b).distance(m.pos))
                });
            if let Some(i) = pick {
                m.task = Some(i);
                m.path = vec![
                    vec2(m.pos.x, CORRIDOR_Y),
                    room_door(i),
                    bed_pos(i) + vec2(40.0, 10.0),
                ];
            }
        }
        match walk_along(&mut m.pos, &mut m.path, MAID_SPEED, dt) {
            Some(dx) => {
                m.walk += dt;
                if dx.abs() > 0.01 {
                    m.face = dx.signum();
                }
            }
            None => m.walk = 0.0,
        }
        let Some(i) = m.task else { return };
        if !m.path.is_empty() {
            return;
        }
        if let Room::Dirty { done } = self.rooms[i] {
            let done = done + dt / MAID_CLEAN;
            if done >= 1.0 {
                self.rooms[i] = Room::Clean;
                m.task = None;
                self.events.push(Event::Cleaned(i));
            } else {
                self.rooms[i] = Room::Dirty { done };
            }
        }
    }

    fn collect(&mut self) {
        if self.pile > 0 && self.player.pos.distance(CASH) < REACH {
            let n = self.pile;
            self.pile = 0;
            self.wallet += n;
            self.earned += n;
            self.events.push(Event::Collected(n));
        }
    }

    pub fn spot_under_player(&self) -> Option<usize> {
        (0..self.spots.len()).find(|k| {
            self.active_spot(*k) && self.spots[*k].pos().distance(self.player.pos) < SPOT_R
        })
    }

    fn pay(&mut self, dt: f32) {
        self.pay_fx -= dt;
        let Some(k) = self.spot_under_player().filter(|_| self.wallet > 0) else {
            self.pay_acc = 0.0;
            return;
        };
        let rate = (self.spots[k].price as f32 / PAY_TIME).max(20.0);
        self.pay_acc += rate * dt;
        let n = (self.pay_acc.floor() as u32)
            .min(self.wallet)
            .min(self.spots[k].left());
        self.pay_acc -= n as f32;
        self.wallet -= n;
        self.spots[k].paid += n;
        if n > 0 && self.pay_fx <= 0.0 {
            self.pay_fx = 0.08;
            self.events.push(Event::Paying(k));
        }
        if self.spots[k].left() == 0 {
            self.spots[k].done = true;
            self.pay_acc = 0.0;
            let b = self.spots[k].build;
            match b {
                Build::Room(i) => self.rooms[i] = Room::Clean,
                Build::Clerk => self.clerk = true,
                Build::Maid => {
                    self.maid = Some(Maid {
                        pos: MAID_SPOT,
                        path: vec![],
                        task: None,
                        walk: 0.0,
                        face: -1.0,
                    })
                }
                Build::Sofa | Build::Palm => self.decor += 1,
            }
            self.events.push(Event::Built(b));
        }
    }

    fn finish(&mut self, dt: f32) {
        if !self.half_said && self.earned * 2 >= self.day.target {
            self.half_said = true;
            self.events.push(Event::Halfway);
        }
        if self.earned >= self.day.target {
            self.outcome = Outcome::Won;
            self.events.push(Event::Won);
            return;
        }
        if let Some(c) = &mut self.clock {
            *c -= dt;
            if *c <= 0.0 {
                *c = 0.0;
                self.outcome = Outcome::Lost;
                self.events.push(Event::Lost);
            }
        }
    }

    pub fn stars(&self) -> u8 {
        stars_for_leavers(self.left)
    }

    pub fn progress(&self) -> f32 {
        (self.earned as f32 / self.day.target as f32).min(1.0)
    }

    pub fn task_pos(&self, t: Task) -> Vec2 {
        match t {
            Task::Desk => SERVE,
            Task::Clean(i) => bed_pos(i) + vec2(0.0, 30.0),
            Task::Towels => CART,
            Task::Cash => CASH,
            Task::Build(k) => self.spots[k].pos(),
        }
    }

    pub fn next_task(&self) -> Option<Task> {
        let staffed = self.clerk;
        let ready = self.front_ready().is_some() && self.free_room().is_some();
        if ready && !staffed {
            return Some(Task::Desk);
        }
        let afford = (0..self.spots.len())
            .filter(|k| self.active_spot(*k))
            .filter(|k| self.wallet >= self.spots[*k].left())
            .min_by_key(|k| (!self.spots[*k].build.is_staff(), self.spots[*k].left()));
        if let Some(k) = afford {
            return Some(Task::Build(k));
        }
        let maid_room = self.maid.as_ref().and_then(|m| m.task);
        let dirty = (0..ROOMS)
            .filter(|i| matches!(self.rooms[*i], Room::Dirty { .. }) && Some(*i) != maid_room)
            .min_by(|a, b| {
                bed_pos(*a)
                    .distance(self.player.pos)
                    .total_cmp(&bed_pos(*b).distance(self.player.pos))
            });
        if let Some(i) = dirty {
            return Some(if self.needs_towels() {
                Task::Towels
            } else {
                Task::Clean(i)
            });
        }
        if self.pile > 0 {
            return Some(Task::Cash);
        }
        if self.day.towels && self.towels < TOWEL_MAX && self.free_room().is_none() {
            return Some(Task::Towels);
        }
        (!staffed && !self.waiting().is_empty()).then_some(Task::Desk)
    }

    pub fn autopilot(&mut self) {
        if self.player.target.is_none() || self.player.still > 0.2 {
            if let Some(t) = self.next_task() {
                let p = self.task_pos(t);
                if p.distance(self.player.pos) > 10.0 {
                    self.walk_to(p);
                }
            }
        }
    }
}

pub fn clamp_world(p: Vec2) -> Vec2 {
    vec2(
        p.x.clamp(30.0, WORLD_W - 30.0),
        p.y.clamp(60.0, WORLD_H - 20.0),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn strong(s: &mut Sim) {
        s.autopilot();
    }

    fn weak(s: &mut Sim, think: &mut f32, dt: f32) {
        *think -= dt;
        if *think > 0.0 {
            return;
        }
        *think = 3.5;
        if rand::gen_range(0.0, 1.0) < 0.5 {
            s.walk_to(vec2(
                rand::gen_range(0.0, WORLD_W),
                rand::gen_range(0.0, WORLD_H),
            ));
        } else if let Some(t) = s.next_task().filter(|t| !matches!(t, Task::Build(_))) {
            s.walk_to(s.task_pos(t));
        }
    }

    fn play(n: u32, bot: impl Fn(&mut Sim, &mut f32, f32), cap: f32) -> Sim {
        let mut s = Sim::new(Day::for_level(n, false));
        let mut think = 0.0;
        let dt = 1.0 / 30.0;
        while s.outcome == Outcome::Playing && s.t < cap {
            bot(&mut s, &mut think, dt);
            s.step(dt);
            s.events.clear();
        }
        s
    }

    fn strong_bot(s: &mut Sim, _: &mut f32, _: f32) {
        strong(s)
    }

    #[test]
    fn walking_follows_the_path_and_stops_at_the_end() {
        let mut p = vec2(0.0, 0.0);
        let mut path = vec![vec2(100.0, 0.0), vec2(100.0, 100.0)];
        walk_along(&mut p, &mut path, 150.0, 1.0);
        assert_eq!(p, vec2(100.0, 50.0));
        assert_eq!(path.len(), 1);
        walk_along(&mut p, &mut path, 150.0, 1.0);
        assert_eq!(p, vec2(100.0, 100.0));
        assert!(path.is_empty());
        assert_eq!(walk_along(&mut p, &mut path, 150.0, 1.0), None);
    }

    #[test]
    fn rooms_and_spots_do_not_overlap() {
        for i in 0..ROOMS {
            for j in i + 1..ROOMS {
                assert!(room_rect(i).intersect(room_rect(j)).is_none());
            }
            assert!(room_rect(i).contains(bed_pos(i)));
            assert!(clean_area(i).contains(bed_pos(i) + vec2(0.0, 30.0)));
            let r = room_rect(i);
            assert!(r.x + r.w <= WORLD_W && r.y + r.h <= WORLD_H);
            assert!(r.y + r.h < CORRIDOR_Y || r.y > CORRIDOR_Y);
        }
        let fixed = [
            SERVE, CASH, CART, CLERK_SPOT, SOFA_SPOT, PALM_SPOT, MAID_SPOT,
        ];
        for (a, p) in fixed.iter().enumerate() {
            for q in &fixed[a + 1..] {
                assert!(p.distance(*q) > 2.0 * SPOT_R - 10.0, "{p} too close to {q}");
            }
            for i in 0..ROOMS {
                assert!(!room_rect(i).contains(*p));
            }
        }
        for k in 0..QUEUE_MAX {
            assert!(queue_slot(k).y < WORLD_H - 40.0);
            assert!(queue_slot(k).distance(CASH) > REACH * 0.9);
        }
    }

    #[test]
    fn a_guest_checks_in_pays_sleeps_and_leaves_a_dirty_room() {
        let mut s = Sim::new(Day::for_level(1, false));
        s.spawn_in = 999.0;
        s.spawn();
        s.player.pos = SERVE;
        while s.served == 0 && s.t < 30.0 {
            s.step(0.05);
        }
        assert_eq!(s.served, 1);
        assert_eq!(s.guests[0].stage, Stage::ToRoom(0));
        assert_eq!(s.rooms[0], Room::Taken);
        assert_eq!(s.pile, s.price());
        assert!(s.events.contains(&Event::CheckedIn(0)));
        while s.guests[0].stage == Stage::ToRoom(0) && s.t < 30.0 {
            s.step(0.05);
        }
        assert!(matches!(s.guests[0].stage, Stage::Asleep(0, _)));
        for _ in 0..400 {
            s.step(0.05);
        }
        assert!(matches!(s.rooms[0], Room::Dirty { .. }));
        assert!(s.guests.is_empty());
    }

    #[test]
    fn nobody_checks_in_without_someone_at_the_desk() {
        let mut s = Sim::new(Day::for_level(1, false));
        s.spawn_in = 999.0;
        s.spawn();
        s.player.pos = vec2(1000.0, 700.0);
        for _ in 0..400 {
            s.step(0.05);
        }
        assert_eq!(s.served, 0);
        s.clerk = true;
        for _ in 0..100 {
            s.step(0.05);
        }
        assert_eq!(s.served, 1);
    }

    #[test]
    fn no_free_room_means_no_check_in() {
        let mut s = Sim::new(Day::for_level(1, false));
        s.spawn_in = 999.0;
        s.rooms[0] = Room::Dirty { done: 0.0 };
        s.spawn();
        s.player.pos = SERVE;
        for _ in 0..200 {
            s.step(0.05);
        }
        assert_eq!(s.served, 0);
        assert_eq!(s.pile, 0);
    }

    #[test]
    fn money_counts_only_once_it_is_picked_up() {
        let mut s = Sim::new(Day::for_level(1, false));
        s.pile = 30;
        s.player.pos = vec2(900.0, 700.0);
        s.step(0.01);
        assert_eq!(s.earned, 0);
        s.player.pos = CASH;
        s.step(0.01);
        assert_eq!((s.earned, s.wallet, s.pile), (30, 30, 0));
    }

    #[test]
    fn standing_in_a_dirty_room_cleans_it() {
        let mut s = Sim::new(Day::for_level(1, false));
        s.spawn_in = 999.0;
        s.rooms[0] = Room::Dirty { done: 0.0 };
        s.player.pos = bed_pos(0) + vec2(0.0, 30.0);
        for _ in 0..30 {
            s.step(0.05);
        }
        assert_eq!(s.rooms[0], Room::Clean);
    }

    #[test]
    fn towel_days_need_a_towel_per_room() {
        let mut s = Sim::new(Day::for_level(4, false));
        s.spawn_in = 999.0;
        s.rooms[0] = Room::Dirty { done: 0.0 };
        s.rooms[1] = Room::Dirty { done: 0.0 };
        s.player.pos = bed_pos(0) + vec2(0.0, 30.0);
        for _ in 0..40 {
            s.step(0.05);
        }
        assert_eq!(s.rooms[0], Room::Dirty { done: 0.0 });
        assert!(s.events.contains(&Event::NeedTowels(0)));
        assert_eq!(s.next_task(), Some(Task::Towels));
        s.player.pos = CART;
        s.step(0.05);
        assert_eq!(s.towels, TOWEL_MAX);
        s.player.pos = bed_pos(0) + vec2(0.0, 30.0);
        for _ in 0..40 {
            s.step(0.05);
        }
        assert_eq!(s.rooms[0], Room::Clean);
        assert_eq!(s.towels, TOWEL_MAX - 1);
    }

    #[test]
    fn standing_on_a_spot_pays_it_off_and_builds() {
        let mut s = Sim::new(Day::for_level(1, false));
        s.spawn_in = 999.0;
        let k = 0;
        assert_eq!(s.spots[k].build, Build::Room(1));
        let price = s.spots[k].price;
        s.wallet = price / 2;
        s.player.pos = s.spots[k].pos();
        for _ in 0..60 {
            s.step(0.05);
        }
        assert_eq!(s.wallet, 0);
        assert_eq!(s.spots[k].paid, price / 2);
        assert_eq!(s.rooms[1], Room::Locked);
        s.wallet = price;
        for _ in 0..60 {
            s.step(0.05);
        }
        assert_eq!(s.rooms[1], Room::Clean);
        assert_eq!(s.wallet, price - (price - price / 2));
        assert!(s.spots[k].done);
    }

    #[test]
    fn rooms_must_be_built_in_order() {
        let mut s = Sim::new(Day::for_level(8, false));
        let rooms: Vec<usize> = (0..s.spots.len())
            .filter(|k| matches!(s.spots[*k].build, Build::Room(_)))
            .collect();
        assert!(rooms.len() >= 2);
        assert!(s.active_spot(rooms[0]));
        assert!(!s.active_spot(rooms[1]));
        s.spots[rooms[0]].done = true;
        if let Build::Room(i) = s.spots[rooms[0]].build {
            s.rooms[i] = Room::Clean;
        }
        assert!(s.active_spot(rooms[1]));
    }

    #[test]
    fn a_hired_maid_cleans_rooms_by_herself() {
        let mut s = Sim::new(Day::for_level(5, false));
        s.spawn_in = 999.0;
        let k = s.spots.iter().position(|p| p.build == Build::Maid).unwrap();
        s.wallet = 500;
        s.player.pos = s.spots[k].pos();
        for _ in 0..60 {
            s.step(0.05);
        }
        assert!(s.maid.is_some());
        s.player.pos = vec2(100.0, 700.0);
        s.rooms[1] = Room::Dirty { done: 0.0 };
        for _ in 0..400 {
            s.step(0.05);
        }
        assert_eq!(s.rooms[1], Room::Clean);
    }

    #[test]
    fn decor_raises_the_room_price() {
        let mut s = Sim::new(Day::for_level(9, false));
        let base = s.price();
        s.wallet = 1000;
        for b in [Build::Sofa, Build::Palm] {
            let k = s.spots.iter().position(|p| p.build == b).unwrap();
            s.player.pos = s.spots[k].pos();
            for _ in 0..80 {
                s.step(0.05);
            }
        }
        assert_eq!(s.price(), base + 2 * DECOR_BONUS);
    }

    #[test]
    fn impatient_guests_leave_and_cost_stars() {
        let mut s = Sim::new(Day::for_level(6, false));
        s.player.pos = vec2(1000.0, 700.0);
        for _ in 0..(60.0 / 0.05) as usize {
            s.step(0.05);
        }
        assert!(s.left >= 3);
        assert_eq!(s.stars(), 1);
        assert_eq!(s.served, 0);
    }

    #[test]
    fn the_queue_never_grows_past_its_max() {
        let mut s = Sim::new(Day::for_level(20, false));
        s.player.pos = vec2(1000.0, 700.0);
        for _ in 0..2000 {
            s.step(0.05);
            assert!(s.waiting().len() <= QUEUE_MAX);
        }
    }

    #[test]
    fn guest_looks_do_not_repeat_back_to_back() {
        let mut s = Sim::new(Day::for_level(1, false));
        for _ in 0..200 {
            s.spawn();
        }
        for w in s.guests.windows(2) {
            assert_ne!(w[0].look, w[1].look);
            assert!((w[1].look as usize) < LOOKS);
        }
    }

    #[test]
    fn days_get_harder() {
        for n in 1..MAX_DAY {
            let a = Day::for_level(n, false);
            let b = Day::for_level(n + 1, false);
            assert!(b.target >= a.target);
            assert!(b.gap <= a.gap);
            assert!(b.patience.unwrap_or(f32::MAX) <= a.patience.unwrap_or(f32::MAX));
            assert!(b.max_rooms >= a.max_rooms);
        }
        let first = Day::for_level(1, false);
        assert!(first.limit.is_none() && first.patience.is_none() && !first.towels);
        let last = Day::for_level(MAX_DAY, false);
        assert!(last.towels && last.maid && last.clerk && last.decor == 2);
        let eased = Day::for_level(10, true);
        assert!(eased.target < Day::for_level(10, false).target);
    }

    #[test]
    fn a_strong_player_wins_every_day() {
        for n in 1..=MAX_DAY {
            let s = play(n, strong_bot, 600.0);
            assert_eq!(s.outcome, Outcome::Won, "day {n} earned {}", s.earned);
            assert!(s.stars() >= 2, "day {n} left {}", s.left);
        }
    }

    #[test]
    fn a_weak_player_wins_day_one_but_not_the_late_days() {
        let s = play(1, weak, 900.0);
        assert_eq!(s.outcome, Outcome::Won);
        let lost = (12..=MAX_DAY)
            .filter(|n| play(*n, weak, 900.0).outcome == Outcome::Lost)
            .count();
        assert!(lost >= 7, "weak bot lost only {lost} late days");
    }

    #[test]
    fn the_hint_points_at_the_desk_first() {
        let mut s = Sim::new(Day::for_level(1, false));
        s.spawn_in = 999.0;
        s.spawn();
        s.player.pos = vec2(1000.0, 700.0);
        for _ in 0..200 {
            s.step(0.05);
        }
        assert_eq!(s.next_task(), Some(Task::Desk));
    }

    fn potential(n: u32) -> Sim {
        let mut s = Sim::new(Day::for_level(n, false));
        s.day.target = u32::MAX;
        let dt = 1.0 / 30.0;
        while s.outcome == Outcome::Playing && s.t < s.day.limit.unwrap_or(120.0) {
            strong(&mut s);
            s.step(dt);
            s.events.clear();
        }
        s
    }

    #[test]
    #[ignore]
    fn tune() {
        for n in 1..=MAX_DAY {
            let p = potential(n);
            println!(
                "potential day {n:2} earned {} served {} left {} built {:?}",
                p.earned,
                p.served,
                p.left,
                p.spots
                    .iter()
                    .filter(|x| x.done)
                    .map(|x| x.build)
                    .collect::<Vec<_>>()
            );
            let s = play(n, strong_bot, 600.0);
            let w = play(n, weak, 600.0);
            println!(
                "day {n:2} target {:4} strong {:?} t={:5.1} left {} built {} | weak {:?} earned {}",
                s.day.target,
                s.outcome,
                s.t,
                s.left,
                s.spots.iter().filter(|p| p.done).count(),
                w.outcome,
                w.earned
            );
        }
    }
}
