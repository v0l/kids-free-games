use super::sim::{xp_for_level, Event, World};
use crate::geom::Color;
use crate::rng as rand;
use glam::Vec2;

pub const KINDS: usize = 4;
pub const SPEED_SECS: f32 = 5.0;
pub const MAGNET_SECS: f32 = 8.0;
pub const SHIELD_SECS: f32 = 5.0;
pub const SPEED_GAIN: f32 = 1.45;
pub const MAGNET_REACH: f32 = 3.0;
pub const PICKUP_R: f32 = 34.0;
pub const SPAWN_EVERY: f32 = 4.0;
const PER_AREA: f32 = 4.0 / (3000.0 * 3000.0);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Kind {
    Speed,
    Magnet,
    Shield,
    Grow,
}

impl Kind {
    pub const ALL: [Kind; KINDS] = [Kind::Speed, Kind::Magnet, Kind::Shield, Kind::Grow];

    pub fn index(self) -> usize {
        self as usize
    }
}

pub fn color(k: Kind) -> Color {
    match k {
        Kind::Speed => Color::from_rgba(255, 196, 40, 255),
        Kind::Magnet => Color::from_rgba(235, 70, 90, 255),
        Kind::Shield => Color::from_rgba(90, 190, 255, 255),
        Kind::Grow => Color::from_rgba(110, 215, 90, 255),
    }
}

pub fn label(k: Kind) -> &'static str {
    match k {
        Kind::Speed => "Zoom!",
        Kind::Magnet => "Magnet!",
        Kind::Shield => "Shield!",
        Kind::Grow => "Grow!",
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Power {
    pub pos: Vec2,
    pub kind: Kind,
    pub age: f32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Boosts {
    pub speed: f32,
    pub magnet: f32,
}

impl Boosts {
    pub fn tick(&mut self, dt: f32) {
        self.speed = (self.speed - dt).max(0.0);
        self.magnet = (self.magnet - dt).max(0.0);
    }
}

impl World {
    pub fn power_cap(&self) -> usize {
        ((self.size * self.size * PER_AREA).round() as usize).max(1)
    }

    pub fn spawn_power(&mut self, kind: Kind) {
        for _ in 0..20 {
            let pos = self.random_point(140.0);
            if self.near_rock(pos, 60.0) {
                continue;
            }
            let crowded = self
                .snakes
                .iter()
                .filter(|s| s.alive)
                .any(|s| (s.head() - pos).length() < 250.0);
            if !crowded {
                self.powers.push(Power {
                    pos,
                    kind,
                    age: 0.0,
                });
                return;
            }
        }
    }

    pub(super) fn tick_powers(&mut self, dt: f32) {
        for p in &mut self.powers {
            p.age += dt;
        }
        for s in self.snakes.iter_mut().filter(|s| s.alive) {
            s.boosts.tick(dt);
        }
        if !self.spec.powerups {
            return;
        }
        self.power_clock -= dt;
        if self.power_clock <= 0.0 {
            self.power_clock = SPAWN_EVERY;
            if self.powers.len() < self.power_cap() {
                let kind = Kind::ALL[rand::gen_range(0usize, KINDS)];
                self.spawn_power(kind);
            }
        }
        for i in 0..self.snakes.len() {
            if !self.snakes[i].alive {
                continue;
            }
            let head = self.snakes[i].head();
            let reach = self.snakes[i].radius() + PICKUP_R;
            let Some(k) = self
                .powers
                .iter()
                .position(|p| (p.pos - head).length() < reach)
            else {
                continue;
            };
            let p = self.powers.swap_remove(k);
            self.apply_power(i, p.kind);
            self.events.push(Event::Power {
                id: self.snakes[i].id,
                kind: p.kind,
                at: p.pos,
                player: self.snakes[i].player,
            });
        }
    }

    pub fn apply_power(&mut self, i: usize, kind: Kind) {
        match kind {
            Kind::Speed => self.snakes[i].boosts.speed = SPEED_SECS,
            Kind::Magnet => self.snakes[i].boosts.magnet = MAGNET_SECS,
            Kind::Shield => {
                let s = &mut self.snakes[i];
                s.shield = s.shield.max(SHIELD_SECS);
            }
            Kind::Grow => {
                let next = xp_for_level(self.snakes[i].level() + 1);
                let need = next.saturating_sub(self.snakes[i].xp);
                self.gain(i, need);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::sim::{level_of, Spec, World};
    use super::*;
    use glam::vec2;

    fn arena() -> World {
        crate::rng::srand(3);
        let mut w = World::new(
            Spec {
                bots: 0,
                eggs: 0,
                ..Spec::arena(1, true)
            },
            "Me",
            0,
        );
        w.snakes[0].shield = 0.0;
        w
    }

    fn grab(w: &mut World, kind: Kind) {
        let head = w.snakes[0].head();
        w.powers.push(Power {
            pos: head + w.snakes[0].dir() * 20.0,
            kind,
            age: 0.0,
        });
        w.step(1.0 / 30.0, None);
    }

    #[test]
    fn running_over_a_powerup_picks_it_up_once() {
        let mut w = arena();
        grab(&mut w, Kind::Speed);
        assert!(w.powers.is_empty());
        assert!(w.events.iter().any(|e| matches!(
            e,
            Event::Power {
                kind: Kind::Speed,
                player: true,
                ..
            }
        )));
    }

    #[test]
    fn speed_makes_the_snake_faster_for_five_seconds() {
        let mut w = arena();
        let base = w.snakes[0].speed();
        grab(&mut w, Kind::Speed);
        assert!((w.snakes[0].speed() / base - SPEED_GAIN).abs() < 1e-3);
        for _ in 0..(SPEED_SECS * 30.0) as usize + 2 {
            w.step(1.0 / 30.0, None);
        }
        assert!((w.snakes[0].speed() - base).abs() < 1e-3);
    }

    #[test]
    fn a_magnet_pulls_in_eggs_from_three_times_as_far() {
        let mut w = arena();
        grab(&mut w, Kind::Magnet);
        let r = w.snakes[0].radius();
        let reach = 55.0 + r * 1.6;
        let head = w.snakes[0].head();
        let side = vec2(-w.snakes[0].dir().y, w.snakes[0].dir().x);
        let start = head + side * reach * 2.2;
        w.eggs.push(super::super::sim::Egg {
            pos: start,
            vel: Vec2::ZERO,
            value: 1,
            kind: 0,
            age: 0.0,
        });
        let before = (start - w.snakes[0].head()).length();
        w.step(1.0 / 30.0, None);
        let after = w
            .eggs
            .first()
            .map_or(0.0, |e| (e.pos - w.snakes[0].head()).length());
        assert!(after < before - 5.0, "{before} -> {after}");
    }

    #[test]
    fn a_shield_turns_being_eaten_into_a_bump() {
        let mut w = arena();
        grab(&mut w, Kind::Shield);
        assert!(w.snakes[0].shield >= SHIELD_SECS - 0.1);
        let head = w.snakes[0].head();
        let i = w.add_snake(head + w.snakes[0].dir() * 22.0, 9, "Big".into(), 1);
        w.snakes[i].xp = xp_for_level(9);
        w.snakes[i].angle = std::f32::consts::FRAC_PI_2;
        w.layout_tail(i);
        w.step(1.0 / 60.0, None);
        assert!(w.player_alive());
    }

    #[test]
    fn grow_adds_exactly_one_level() {
        let mut w = arena();
        let before = w.snakes[0].level();
        grab(&mut w, Kind::Grow);
        assert_eq!(level_of(w.snakes[0].xp), before + 1);
    }

    #[test]
    fn a_bot_steers_for_a_powerup_it_can_reach() {
        let mut w = arena();
        let i = w.add_snake(vec2(1500.0, 1500.0), 3, "Bot".into(), 1);
        w.snakes[i].angle = 0.0;
        w.snakes[i].want = 0.0;
        w.layout_tail(i);
        w.snakes[0].segs[0] = vec2(400.0, 400.0);
        w.layout_tail(0);
        let target = vec2(1500.0, 1500.0) + vec2(160.0, 120.0);
        w.powers.push(Power {
            pos: target,
            kind: Kind::Grow,
            age: 0.0,
        });
        let mut brain = super::super::bots::Brain::new(0.8, 0.0);
        let want = w.think(i, &mut brain);
        let to = (target - w.snakes[i].head()).normalize();
        assert!(vec2(want.cos(), want.sin()).dot(to) > 0.7);
    }

    #[test]
    fn powerups_spawn_up_to_the_cap_and_can_be_turned_off() {
        crate::rng::srand(4);
        let mut w = World::new(Spec::arena(1, false), "Me", 0);
        let mut most = 0;
        for _ in 0..30 * 60 {
            w.step(1.0 / 30.0, None);
            assert!(w.powers.len() <= w.power_cap());
            most = most.max(w.powers.len());
        }
        assert!(most > 0);
        let mut off = World::new(
            Spec {
                powerups: false,
                ..Spec::arena(1, false)
            },
            "Me",
            0,
        );
        for _ in 0..30 * 30 {
            off.step(1.0 / 30.0, None);
        }
        assert!(off.powers.is_empty());
    }
}
