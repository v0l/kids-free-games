use crate::brain::Effort;

pub const START: i64 = 1000;
pub const FLOOR: i64 = 800;
pub const TOP: i64 = 2000;
const STEP: f32 = 40.0;

pub fn expected(rating: i64) -> f32 {
    let r = rating.clamp(FLOOR, TOP) as f32;
    0.35 + 0.4 * (r - FLOOR as f32) / (TOP - FLOOR) as f32
}

pub fn placement(outlived: usize, field: usize) -> f32 {
    if field <= 1 {
        return 1.0;
    }
    (outlived as f32 / (field - 1) as f32).clamp(0.0, 1.0)
}

pub fn update(rating: i64, placement: f32, won: bool) -> i64 {
    let score = if won { 1.0 } else { placement };
    let gain = STEP * (score - expected(rating)) + if won { STEP * 0.5 } else { 0.0 };
    (rating + gain.round() as i64).clamp(FLOOR, TOP)
}

pub fn effort(rating: i64) -> Effort {
    let k = (rating.clamp(FLOOR, TOP) - FLOOR) as f32 / (TOP - FLOOR) as f32;
    Effort::new(0.15 + 0.85 * k)
}

pub fn tier(rating: i64) -> u32 {
    ((rating.clamp(FLOOR, TOP) - FLOOR) / 100) as u32 + 1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn winning_raises_the_rating_and_an_early_exit_lowers_it() {
        assert!(update(START, 1.0, true) > START);
        assert!(update(START, 0.1, false) < START);
        assert_eq!(update(FLOOR, 0.0, false), FLOOR);
        assert_eq!(update(TOP, 1.0, true), TOP);
    }

    #[test]
    fn a_player_who_keeps_winning_climbs_until_wins_get_rarer() {
        let mut r = START;
        for _ in 0..20 {
            r = update(r, 1.0, true);
        }
        assert!(r > 1500, "{r}");
        let mid = update(1600, 0.6, false);
        assert!(
            mid < 1600,
            "an average finish at a high rating drifts down: {mid}"
        );
    }

    #[test]
    fn bots_get_harder_as_the_rating_climbs() {
        assert!(effort(FLOOR).get() < 0.2);
        assert!(effort(START).get() < effort(1500).get());
        assert_eq!(effort(TOP).get(), 1.0);
        assert_eq!(tier(START), 3);
    }

    #[test]
    fn placement_is_the_share_of_the_field_outlived() {
        assert_eq!(placement(0, 12), 0.0);
        assert_eq!(placement(11, 12), 1.0);
        assert!((placement(6, 13) - 0.5).abs() < 1e-6);
    }
}
