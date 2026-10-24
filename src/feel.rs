use crate::engine::*;
use macroquad::prelude::*;
use std::cell::RefCell;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Impact {
    Small,
    Medium,
    Large,
}

impl Impact {
    pub fn hit_stop(self) -> f32 {
        match self {
            Impact::Small => 0.0,
            Impact::Medium => 0.05,
            Impact::Large => 0.12,
        }
    }

    pub fn flash(self) -> f32 {
        match self {
            Impact::Small => 0.0,
            Impact::Medium => 0.0,
            Impact::Large => 0.35,
        }
    }
}

pub const SLOW: f32 = 0.05;
pub const FLASH_TIME: f32 = 0.12;
pub const MAX_FLASHES_PER_SEC: usize = 3;

#[derive(Default)]
pub struct Feel {
    stop: f32,
    flash: f32,
    flash_color: Option<Color>,
    flashes: Vec<f32>,
    clock: f32,
}

impl Feel {
    pub fn hit(&mut self, i: Impact, color: Color) {
        self.stop = self.stop.max(i.hit_stop());
        if i.flash() > 0.0 {
            self.flashes.retain(|t| self.clock - *t < 1.0);
            if self.flashes.len() < MAX_FLASHES_PER_SEC {
                self.flashes.push(self.clock);
                self.flash = i.flash();
                self.flash_color = Some(color);
            }
        }
    }

    pub fn game_dt(&mut self, real_dt: f32) -> f32 {
        self.clock += real_dt;
        self.flash = (self.flash - real_dt * (0.35 / FLASH_TIME)).max(0.0);
        if self.stop > 0.0 {
            self.stop = (self.stop - real_dt).max(0.0);
            real_dt * SLOW
        } else {
            real_dt
        }
    }

    pub fn frozen(&self) -> bool {
        self.stop > 0.0
    }

    pub fn flash_alpha(&self) -> f32 {
        self.flash
    }

    pub fn draw(&self, view: &View) {
        if self.flash > 0.0 {
            let c = self.flash_color.unwrap_or(WHITE);
            fill_screen(view, with_alpha(c, self.flash));
        }
    }
}

thread_local! {
    static PENDING: RefCell<Vec<(Impact, Color)>> = const { RefCell::new(Vec::new()) };
}

pub fn impact(i: Impact) {
    impact_tinted(i, WHITE);
}

pub fn impact_tinted(i: Impact, color: Color) {
    PENDING.with(|p| p.borrow_mut().push((i, color)));
}

pub fn take() -> Vec<(Impact, Color)> {
    PENDING.with(|p| std::mem::take(&mut *p.borrow_mut()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hit_stop_slows_then_returns_to_real_time() {
        let mut f = Feel::default();
        f.hit(Impact::Large, WHITE);
        assert!(f.frozen());
        let dt = 1.0 / 60.0;
        assert!((f.game_dt(dt) - dt * SLOW).abs() < 1e-6);
        let mut t = dt;
        while f.frozen() {
            f.game_dt(dt);
            t += dt;
            assert!(t < 0.2, "hit-stop never ended");
        }
        assert_eq!(f.game_dt(dt), dt);
    }

    #[test]
    fn overlapping_stops_keep_the_longest_not_the_sum() {
        let mut f = Feel::default();
        f.hit(Impact::Large, WHITE);
        f.game_dt(0.05);
        f.hit(Impact::Medium, WHITE);
        let mut t = 0.0;
        while f.frozen() {
            f.game_dt(0.01);
            t += 0.01;
        }
        assert!(t <= 0.08, "medium hit extended a large stop to {t}");
    }

    #[test]
    fn small_impacts_never_freeze_or_flash() {
        let mut f = Feel::default();
        for _ in 0..100 {
            f.hit(Impact::Small, WHITE);
        }
        assert!(!f.frozen());
        assert_eq!(f.flash_alpha(), 0.0);
    }

    #[test]
    fn flashes_stay_under_three_a_second() {
        let mut f = Feel::default();
        let mut started = 0;
        for _ in 0..60 {
            let before = f.flash_alpha();
            f.hit(Impact::Large, WHITE);
            if f.flash_alpha() > before {
                started += 1;
            }
            f.game_dt(1.0 / 60.0);
        }
        assert!(
            started <= MAX_FLASHES_PER_SEC,
            "{started} flashes in a second"
        );
    }

    #[test]
    fn flash_fades_out() {
        let mut f = Feel::default();
        f.hit(Impact::Large, WHITE);
        for _ in 0..20 {
            f.game_dt(1.0 / 60.0);
        }
        assert_eq!(f.flash_alpha(), 0.0);
    }
}
