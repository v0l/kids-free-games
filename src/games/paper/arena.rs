pub const MAX_LEVEL: u32 = 12;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Arena {
    pub level: u32,
    pub target: f32,
    pub bots: usize,
    pub pace: f32,
    pub tempo: f32,
    pub aggro: f32,
}

impl Arena {
    pub fn for_level(level: u32, fails: u32) -> Self {
        let level = level.clamp(1, MAX_LEVEL);
        let steps = (level - 1) as f32;
        let mut target = (8.0 + 3.0 * steps).min(40.0);
        let mut bots = (1 + (level as usize - 1) / 2).min(4);
        let mut pace = (0.4 + 0.075 * steps).min(1.0);
        let mut tempo = (0.75 + 0.0625 * steps).min(1.0);
        let mut aggro = if level < 2 {
            0.0
        } else {
            (0.06 * (steps - 1.0)).clamp(0.0, 0.6)
        };
        if fails >= 2 {
            target *= 0.8;
            bots = bots.saturating_sub(1).max(1);
            pace *= 0.85;
            tempo *= 0.92;
            aggro *= 0.5;
        }
        if fails >= 4 {
            target *= 0.8;
            bots = 1;
            pace *= 0.85;
            tempo *= 0.92;
            aggro = 0.0;
        }
        Self {
            level,
            target: target.round().max(6.0) / 100.0,
            bots,
            pace,
            tempo,
            aggro,
        }
    }

    pub fn par(&self) -> (f32, f32) {
        let pct = self.target * 100.0;
        (15.0 + pct * 2.5, 30.0 + pct * 5.0)
    }

    pub fn stars(&self, secs: f32) -> u8 {
        let (fast, slow) = self.par();
        if secs <= fast {
            3
        } else if secs <= slow {
            2
        } else {
            1
        }
    }
}

pub fn score(level: u32, share: f32, won: bool) -> i64 {
    let cleared = if won { level } else { level.saturating_sub(1) };
    cleared as i64 * 100 + ((share * 100.0).floor() as i64).clamp(0, 99)
}

pub fn score_label(s: i64) -> String {
    format!("{} won  {}%", s / 100, s % 100)
}

pub fn next_level(level: u32) -> u32 {
    (level + 1).min(MAX_LEVEL)
}

pub fn clock_label(secs: f32) -> String {
    let s = secs as u32;
    format!("{}:{:02}", s / 60, s % 60)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arenas_get_harder_with_level() {
        let all: Vec<Arena> = (1..=MAX_LEVEL).map(|l| Arena::for_level(l, 0)).collect();
        for w in all.windows(2) {
            assert!(w[1].target >= w[0].target);
            assert!(w[1].bots >= w[0].bots);
            assert!(w[1].pace >= w[0].pace);
            assert!(w[1].tempo >= w[0].tempo);
            assert!(w[1].aggro >= w[0].aggro);
        }
        let (first, last) = (all[0], all[all.len() - 1]);
        assert!(last.target > first.target * 3.0);
        assert!(first.bots == 1 && last.bots == 4);
        assert!(first.pace < 0.5 && last.pace >= 1.0);
        assert!(first.tempo < 0.8 && last.tempo >= 1.0);
        assert_eq!(first.aggro, 0.0);
        assert!(last.aggro > 0.3);
    }

    #[test]
    fn the_first_arena_is_small_and_slow() {
        let a = Arena::for_level(1, 0);
        assert!(a.target <= 0.1);
        assert_eq!(a.bots, 1);
        assert!(a.pace <= 0.4);
    }

    #[test]
    fn struggling_makes_an_arena_easier() {
        for level in 2..=MAX_LEVEL {
            let base = Arena::for_level(level, 0);
            let help = Arena::for_level(level, 2);
            let more = Arena::for_level(level, 4);
            assert_eq!(Arena::for_level(level, 1), base);
            assert!(help.target <= base.target && more.target <= help.target);
            assert!(help.pace < base.pace && more.pace < help.pace);
            assert!(help.tempo < base.tempo && more.tempo < help.tempo);
            assert!(help.aggro <= base.aggro && more.aggro == 0.0);
            assert!(more.bots == 1);
        }
        assert!(Arena::for_level(1, 2).target < Arena::for_level(1, 0).target);
    }

    #[test]
    fn faster_wins_earn_more_stars() {
        let a = Arena::for_level(3, 0);
        let (fast, slow) = a.par();
        assert!(fast < slow);
        assert_eq!(a.stars(fast - 1.0), 3);
        assert_eq!(a.stars(slow - 1.0), 2);
        assert_eq!(a.stars(slow + 500.0), 1);
    }

    #[test]
    fn score_orders_by_arenas_won_then_land() {
        assert!(score(3, 0.14, true) > score(3, 0.39, false));
        assert!(score(3, 0.30, false) > score(2, 0.14, true));
        assert!(score(3, 0.14, false) > score(2, 0.20, false));
        assert_eq!(score_label(score(3, 0.14, true)), "3 won  14%");
    }

    #[test]
    fn the_last_arena_repeats() {
        assert_eq!(next_level(1), 2);
        assert_eq!(next_level(MAX_LEVEL), MAX_LEVEL);
    }

    #[test]
    fn clock_reads_minutes_and_seconds() {
        assert_eq!(clock_label(65.4), "1:05");
    }
}
