use crate::games::{Kind, CATALOG};
use crate::{mastery, save};

pub const UNLOCK_COST: i64 = 2;

fn flag(game: &str) -> String {
    format!("gate.open.{game}")
}

static FORCED_OFF: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

pub fn force_off(off: bool) {
    FORCED_OFF.store(off, std::sync::atomic::Ordering::Relaxed);
}

pub fn enabled() -> bool {
    !FORCED_OFF.load(std::sync::atomic::Ordering::Relaxed) && save::player_get("gate.off") == 0
}

pub fn set_enabled(on: bool) {
    save::player_set("gate.off", i64::from(!on));
}

pub fn enabled_for(id: &str) -> bool {
    save::get(&format!("p.{id}.gate.off")) == 0
}

pub fn set_enabled_for(id: &str, on: bool) {
    save::set(&format!("p.{id}.gate.off"), i64::from(!on));
}

pub const HOLD_SECS: f32 = 1.5;

pub fn first_learning_game() -> &'static str {
    CATALOG
        .iter()
        .find(|i| i.kind == Kind::Learn)
        .map(|i| i.key)
        .unwrap_or("mathpop")
}

pub fn spent() -> i64 {
    save::player_get("gate.spent")
}

pub fn available() -> i64 {
    (mastery::points() - spent()).max(0)
}

fn has_history(game: &str) -> bool {
    let prefix = save::player_key(&format!("{game}."));
    !save::keys_with_prefix(&prefix).is_empty()
}

fn is_arcade(game: &str) -> bool {
    CATALOG
        .iter()
        .any(|i| i.key == game && i.kind == Kind::Arcade)
}

pub fn open(game: &str) -> bool {
    !is_arcade(game) || !enabled() || save::player_get(&flag(game)) != 0 || has_history(game)
}

pub fn opened_count() -> usize {
    CATALOG
        .iter()
        .filter(|i| i.kind == Kind::Arcade && open(i.key))
        .count()
}

pub fn cost() -> i64 {
    if opened_count() == 0 {
        0
    } else {
        UNLOCK_COST
    }
}

pub fn can_afford() -> bool {
    available() >= cost()
}

pub fn unlock(game: &str) -> bool {
    if open(game) {
        return true;
    }
    if !can_afford() {
        return false;
    }
    save::player_set("gate.spent", spent() + cost());
    save::player_set(&flag(game), 1);
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mastery::{record, Answer};
    use crate::profile;

    fn earn(points: i64) {
        save::player_set("learn.points", mastery::points() + points);
    }

    fn arcade(n: usize) -> &'static str {
        CATALOG
            .iter()
            .filter(|i| i.kind == Kind::Arcade)
            .nth(n)
            .unwrap()
            .key
    }

    fn fresh_player(name: &str) {
        profile::create(name, 0);
    }

    #[test]
    fn forcing_the_gate_off_opens_every_game() {
        save::reset_for_test();
        fresh_player("Mia");
        assert!(!open(arcade(0)));
        force_off(true);
        let opened = open(arcade(0));
        force_off(false);
        assert!(opened);
    }

    #[test]
    fn learning_games_are_never_locked() {
        save::reset_for_test();
        fresh_player("Mia");
        let learn = CATALOG.iter().find(|i| i.kind == Kind::Learn).unwrap();
        assert!(open(learn.key));
    }

    #[test]
    fn the_first_unlock_is_free_then_each_costs_points() {
        save::reset_for_test();
        fresh_player("Mia");
        assert!(!open(arcade(0)));
        assert!(unlock(arcade(0)));
        assert_eq!(available(), 0);
        assert!(!unlock(arcade(1)));
        assert!(!open(arcade(1)));
        earn(UNLOCK_COST);
        assert!(unlock(arcade(1)));
        assert!(open(arcade(1)));
        assert_eq!(available(), 0);
    }

    #[test]
    fn spent_points_stay_spent_and_earning_only_adds() {
        save::reset_for_test();
        fresh_player("Mia");
        unlock(arcade(0));
        earn(5);
        unlock(arcade(1));
        assert_eq!(spent(), UNLOCK_COST);
        assert_eq!(mastery::points(), 5);
        assert_eq!(available(), 5 - UNLOCK_COST);
        assert!(unlock(arcade(1)));
        assert_eq!(spent(), UNLOCK_COST);
    }

    #[test]
    fn real_learning_earns_the_points_that_unlock_games() {
        save::reset_for_test();
        fresh_player("Mia");
        unlock(arcade(0));
        for _ in 0..20 {
            record(
                "mathpop",
                0,
                Answer {
                    first_try: true,
                    secs: 1.0,
                    helped: false,
                },
                4.0,
            );
        }
        assert!(mastery::points() >= UNLOCK_COST);
        assert!(unlock(arcade(1)));
    }

    #[test]
    fn unlocks_are_per_player() {
        save::reset_for_test();
        fresh_player("Mia");
        unlock(arcade(0));
        earn(10);
        unlock(arcade(1));
        fresh_player("Leo");
        assert!(!open(arcade(0)));
        assert!(!open(arcade(1)));
        assert_eq!(available(), 0);
    }

    #[test]
    fn a_player_who_already_played_a_game_keeps_it() {
        save::reset_for_test();
        fresh_player("Mia");
        profile::set(arcade(2), "best", 40);
        assert!(open(arcade(2)));
        assert_eq!(opened_count(), 1);
        assert_eq!(cost(), UNLOCK_COST);
        assert_eq!(spent(), 0);
    }

    #[test]
    fn the_parent_switch_opens_everything_for_that_player_only() {
        save::reset_for_test();
        fresh_player("Mia");
        set_enabled(false);
        assert!(open(arcade(3)));
        fresh_player("Leo");
        assert!(!open(arcade(3)));
        set_enabled(true);
        assert!(!open(arcade(3)));
    }

    #[test]
    fn the_parent_switch_can_target_a_player_by_id() {
        save::reset_for_test();
        let mia = profile::create("Mia", 0).unwrap();
        let leo = profile::create("Leo", 0).unwrap();
        set_enabled_for(&mia.id, false);
        assert!(!enabled_for(&mia.id));
        assert!(enabled_for(&leo.id));
        profile::select(&mia.id);
        assert!(open(arcade(3)));
        profile::select(&leo.id);
        assert!(!open(arcade(3)));
    }

    #[test]
    fn switching_gating_back_on_keeps_earlier_unlocks() {
        save::reset_for_test();
        fresh_player("Mia");
        unlock(arcade(0));
        set_enabled(false);
        set_enabled(true);
        assert!(open(arcade(0)));
        assert!(!open(arcade(1)));
    }
}
