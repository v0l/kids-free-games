use super::sim::{xp_for_level, Spec, World};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Goal {
    Level(u32),
    Eat(u32),
    Survive(f32),
    Boss(u32),
}

#[derive(Clone, Debug)]
pub struct Floor {
    pub n: u32,
    pub goal: Goal,
    pub spec: Spec,
    pub boss_skin: usize,
    pub theme: usize,
}

pub const THEMES: usize = 5;

pub fn is_boss(n: u32) -> bool {
    n > 0 && n.is_multiple_of(5)
}

pub fn floor(n: u32) -> Floor {
    let n = n.max(1);
    let start = (1 + n / 6).min(12);
    let spec = Spec {
        size: 1500.0 + (n as f32 * 25.0).min(900.0),
        bots: (6 + n as usize / 4).min(12),
        eggs: 230 + if is_boss(n) { 120 } else { 0 },
        rocks: (n as usize / 3).min(8),
        aggression: (0.08 + n as f32 * 0.022).min(0.85),
        skill: (0.35 + n as f32 * 0.015).min(0.9),
        big_share: (0.04 + n as f32 * 0.014).min(0.45),
        lo: start.saturating_sub(1).max(1),
        hi: start + 1 + n / 4,
        adaptive: false,
        start,
        powerups: true,
    };
    let goal = if is_boss(n) {
        Goal::Boss((n / 5 + 5).clamp(start + 2, 16))
    } else {
        match n % 5 {
            1 => Goal::Level((start + 2 + n / 4).min(14)),
            2 => Goal::Eat(2 + n / 3),
            3 => Goal::Survive((18.0 + n as f32 * 1.5).min(75.0)),
            _ => Goal::Level((start + 3 + n / 3).min(14)),
        }
    };
    Floor {
        n,
        goal,
        spec,
        boss_skin: (n as usize / 5) % super::skins::SKINS.len(),
        theme: (n as usize - 1) % THEMES,
    }
}

impl Floor {
    pub fn build(&self, name: &str, skin: usize) -> World {
        let mut w = World::new(self.spec.clone(), name, skin);
        if let Goal::Boss(level) = self.goal {
            w.spawn_boss(level, self.boss_skin);
        }
        w
    }

    pub fn progress(&self, w: &World) -> f32 {
        let me = w.player();
        match self.goal {
            Goal::Level(l) => {
                let have = (me.xp as f32).min(xp_for_level(l) as f32);
                have / xp_for_level(l) as f32
            }
            Goal::Eat(k) => (w.player_eats as f32 / k as f32).min(1.0),
            Goal::Survive(t) => (me.age / t).min(1.0),
            Goal::Boss(_) => {
                if w.boss_down {
                    1.0
                } else {
                    0.0
                }
            }
        }
    }

    pub fn done(&self, w: &World) -> bool {
        w.player_alive()
            && match self.goal {
                Goal::Level(l) => w.player().level() >= l,
                Goal::Eat(k) => w.player_eats >= k,
                Goal::Survive(t) => w.player().age >= t,
                Goal::Boss(_) => w.boss_down,
            }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn play(f: &Floor, strong: bool, seed: u64, limit: f32) -> bool {
        crate::rng::srand(seed);
        let mut w = f.build("Me", 0);
        let dt = 1.0 / 30.0;
        let mut t = 0.0;
        while t < limit {
            let steer = if strong {
                w.autopilot()
            } else {
                w.player().angle + 0.01
            };
            w.step(dt, Some(steer));
            if f.done(&w) {
                return true;
            }
            if !w.player_alive() {
                return false;
            }
            t += dt;
        }
        false
    }

    #[test]
    fn floors_never_repeat_a_goal_or_a_setup() {
        let fl: Vec<Floor> = (1..=30).map(floor).collect();
        for pair in fl.windows(2) {
            let (a, b) = (&pair[0], &pair[1]);
            assert_ne!(a.goal, b.goal, "floors {} and {}", a.n, b.n);
        }
        for (i, a) in fl.iter().enumerate() {
            for b in &fl[i + 1..] {
                let same = a.goal == b.goal
                    && a.spec.size == b.spec.size
                    && a.spec.aggression == b.spec.aggression;
                assert!(!same, "floors {} and {} are the same", a.n, b.n);
            }
        }
    }

    #[test]
    fn every_fifth_floor_has_a_boss_and_the_boss_can_be_out_levelled() {
        for n in 1..=60 {
            let f = floor(n);
            match f.goal {
                Goal::Boss(l) => {
                    assert!(is_boss(n));
                    let eggs = f.spec.eggs as u32;
                    let reachable = xp_for_level(f.spec.start) + eggs;
                    assert!(
                        reachable > xp_for_level(l + 1) + 20,
                        "floor {n}: boss {l} needs {} xp, eggs give {reachable}",
                        xp_for_level(l + 1)
                    );
                }
                _ => assert!(!is_boss(n)),
            }
        }
    }

    #[test]
    fn difficulty_rises_floor_over_floor() {
        let key = |n: u32| {
            let f = floor(n);
            f.spec.aggression + f.spec.skill + f.spec.size / 1000.0 + f.spec.rocks as f32 * 0.1
        };
        for n in (1..30).step_by(6) {
            assert!(key(n + 6) > key(n), "floor {n}");
        }
    }

    #[test]
    fn goals_are_reachable_within_the_world() {
        for n in 1..=60 {
            let f = floor(n);
            if let Goal::Level(l) = f.goal {
                assert!(
                    xp_for_level(l) < f.spec.eggs as u32 + 40,
                    "floor {n} level {l}"
                );
            }
            if let Goal::Eat(k) = f.goal {
                assert!(k <= f.spec.bots as u32 * 2, "floor {n} eat {k}");
            }
        }
    }

    #[test]
    fn a_strong_player_clears_early_floors() {
        let mut cleared = 0;
        for n in 1..=5 {
            for seed in 0..3 {
                if play(&floor(n), true, seed + n as u64 * 10, 180.0) {
                    cleared += 1;
                }
            }
        }
        assert!(cleared >= 10, "strong player cleared {cleared}/15");
    }

    #[test]
    fn a_weak_player_going_in_circles_fails_the_harder_floors() {
        let mut weak = 0;
        let mut strong = 0;
        for n in [3, 8, 13, 18] {
            for seed in 0..3 {
                if play(&floor(n), false, seed + 100, 120.0) {
                    weak += 1;
                }
                if play(&floor(n), true, seed + 100, 180.0) {
                    strong += 1;
                }
            }
        }
        assert!(strong > weak, "strong {strong} weak {weak}");
    }
}
