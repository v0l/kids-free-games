use crate::save::{self, Map};

pub const LOWER_IS_BETTER: [&str; 1] = ["memory"];

const NOT_SYNCED: [&str; 3] = ["name", "avatar", "created"];

const RAISE_ONLY: [&str; 9] = [
    ".best",
    ".unlocked",
    ".cleared",
    ".played",
    ".runs",
    ".taught",
    ".milestone",
    ".bestlines",
    ".learned",
];

#[derive(Debug, PartialEq, Eq)]
enum Rule {
    Higher,
    Lower,
    Local,
}

fn rule(key: &str) -> Rule {
    if let Some(game) = key.strip_suffix(".best") {
        if LOWER_IS_BETTER.contains(&game) {
            return Rule::Lower;
        }
    }
    if key.contains(".stars.") || RAISE_ONLY.iter().any(|s| key.ends_with(s)) {
        Rule::Higher
    } else {
        Rule::Local
    }
}

pub fn prefix(player: &str) -> String {
    format!("p.{player}.")
}

pub fn collect(player: &str) -> Map {
    let pre = prefix(player);
    save::keys_with_prefix(&pre)
        .into_iter()
        .filter_map(|k| {
            let rel = k[pre.len()..].to_string();
            if NOT_SYNCED.contains(&rel.as_str()) {
                return None;
            }
            let v = save::get_str(&k)?;
            Some((rel, v))
        })
        .collect()
}

pub fn apply(player: &str, merged: &Map) -> usize {
    let pre = prefix(player);
    let mut changed = 0;
    for (rel, v) in merged {
        if NOT_SYNCED.contains(&rel.as_str()) {
            continue;
        }
        let key = format!("{pre}{rel}");
        if save::get_str(&key).as_deref() != Some(v) {
            save::set_str(&key, v);
            changed += 1;
        }
    }
    changed
}

pub fn merge(local: &Map, remote: &Map, base: &Map) -> Map {
    let mut out = remote.clone();
    for (k, mine) in local {
        let Some(theirs) = remote.get(k) else {
            out.insert(k.clone(), mine.clone());
            continue;
        };
        let pick = match (rule(k), mine.parse::<i64>(), theirs.parse::<i64>()) {
            (Rule::Higher, Ok(a), Ok(b)) => a.max(b).to_string(),
            (Rule::Lower, Ok(a), Ok(b)) => a.min(b).to_string(),
            _ if base.get(k) == Some(mine) => theirs.clone(),
            _ => mine.clone(),
        };
        out.insert(k.clone(), pick);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn map(pairs: &[(&str, &str)]) -> Map {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    fn none() -> Map {
        Map::new()
    }

    #[test]
    fn keys_only_one_side_has_are_kept() {
        let m = merge(
            &map(&[("snake.unlocked", "3")]),
            &map(&[("hotel.unlocked", "5")]),
            &none(),
        );
        assert_eq!(m, map(&[("snake.unlocked", "3"), ("hotel.unlocked", "5")]));
    }

    #[test]
    fn progress_never_goes_backwards_on_a_conflict() {
        let m = merge(
            &map(&[
                ("snake.stars.2", "1"),
                ("snake.best", "40"),
                ("fruit.unlocked", "9"),
            ]),
            &map(&[
                ("snake.stars.2", "3"),
                ("snake.best", "25"),
                ("fruit.unlocked", "4"),
            ]),
            &map(&[
                ("snake.stars.2", "1"),
                ("snake.best", "40"),
                ("fruit.unlocked", "9"),
            ]),
        );
        assert_eq!(m["snake.stars.2"], "3");
        assert_eq!(m["snake.best"], "40");
        assert_eq!(m["fruit.unlocked"], "9");
    }

    #[test]
    fn memory_best_keeps_the_fewest_moves() {
        let m = merge(
            &map(&[("memory.best", "14")]),
            &map(&[("memory.best", "11")]),
            &none(),
        );
        assert_eq!(m["memory.best"], "11");
        let m = merge(
            &map(&[("memory.best", "9")]),
            &map(&[("memory.best", "11")]),
            &none(),
        );
        assert_eq!(m["memory.best"], "9");
    }

    #[test]
    fn a_device_that_did_not_play_takes_the_other_devices_levels() {
        let base = map(&[("count.level", "2"), ("spell.ease", "1")]);
        let m = merge(
            &base,
            &map(&[("count.level", "6"), ("spell.ease", "4")]),
            &base,
        );
        assert_eq!(m["count.level"], "6");
        assert_eq!(m["spell.ease"], "4");
    }

    #[test]
    fn a_device_that_played_offline_keeps_its_own_levels() {
        let base = map(&[("count.level", "2"), ("spell.ease", "1")]);
        let m = merge(
            &map(&[("count.level", "3"), ("spell.ease", "1")]),
            &map(&[("count.level", "6"), ("spell.ease", "4")]),
            &base,
        );
        assert_eq!(m["count.level"], "3");
        assert_eq!(m["spell.ease"], "4");
    }

    #[test]
    fn text_values_do_not_crash_the_numeric_rules() {
        let m = merge(
            &map(&[("snake.best", "x")]),
            &map(&[("snake.best", "7")]),
            &none(),
        );
        assert_eq!(m["snake.best"], "x");
    }

    #[test]
    fn merging_is_stable_when_both_sides_agree() {
        let a = map(&[("snake.best", "7"), ("hotel.level", "3")]);
        assert_eq!(merge(&a, &a, &a), a);
        assert_eq!(merge(&a, &a, &none()), a);
    }

    #[test]
    fn name_avatar_and_created_stay_out_of_the_save() {
        save::reset_for_test();
        let id = "11111111-2222-4333-8444-555555555555";
        save::set_str(&format!("p.{id}.name"), "Mia");
        save::set(&format!("p.{id}.avatar"), 2);
        save::set(&format!("p.{id}.created"), 5);
        save::set(&format!("p.{id}.snake.unlocked"), 4);
        save::set("p.other.snake.unlocked", 9);
        assert_eq!(collect(id), map(&[("snake.unlocked", "4")]));
    }

    #[test]
    fn apply_writes_only_what_differs() {
        save::reset_for_test();
        let id = "11111111-2222-4333-8444-555555555555";
        save::set(&format!("p.{id}.snake.unlocked"), 4);
        let n = apply(id, &map(&[("snake.unlocked", "4"), ("hotel.level", "2")]));
        assert_eq!(n, 1);
        assert_eq!(save::get(&format!("p.{id}.hotel.level")), 2);
    }

    #[test]
    fn rules_cover_the_keys_the_games_write() {
        assert_eq!(rule("snake.stars.7"), Rule::Higher);
        assert_eq!(rule("memory.best"), Rule::Lower);
        assert_eq!(rule("snake.best"), Rule::Higher);
        assert_eq!(rule("count.level"), Rule::Local);
        assert_eq!(rule("spell.fails"), Rule::Local);
    }
}
