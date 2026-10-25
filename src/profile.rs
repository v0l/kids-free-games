use crate::save;
use std::cell::RefCell;

pub const AVATARS: [&str; 12] = [
    "fox", "panda", "bunny", "lion", "frog", "penguin", "kitten", "owl", "puppy", "unicorn",
    "koala", "tiger",
];
pub const MAX_NAME: usize = 12;
pub const MAX_PROFILES: usize = 8;
pub const BOARD_SIZE: usize = 10;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Profile {
    pub id: String,
    pub name: String,
    pub avatar: usize,
    pub created: i64,
}

pub fn uuid_v4(bytes: [u8; 16]) -> String {
    let mut b = bytes;
    b[6] = (b[6] & 0x0f) | 0x40;
    b[8] = (b[8] & 0x3f) | 0x80;
    let h: String = b.iter().map(|x| format!("{x:02x}")).collect();
    format!(
        "{}-{}-{}-{}-{}",
        &h[0..8],
        &h[8..12],
        &h[12..16],
        &h[16..20],
        &h[20..32]
    )
}

pub fn is_uuid(s: &str) -> bool {
    let parts: Vec<&str> = s.split('-').collect();
    parts.iter().map(|p| p.len()).collect::<Vec<_>>() == [8, 4, 4, 4, 12]
        && s.chars().all(|c| c == '-' || c.is_ascii_hexdigit())
}

fn now_secs() -> i64 {
    #[cfg(test)]
    {
        1_700_000_000
    }
    #[cfg(not(test))]
    {
        (macroquad::miniquad::date::now()) as i64
    }
}

pub(crate) fn entropy() -> [u8; 16] {
    thread_local! {
        static SEED: RefCell<u64> = const { RefCell::new(0) };
    }
    SEED.with(|s| {
        let mut s = s.borrow_mut();
        if *s == 0 {
            let t = (macroquad::miniquad::date::now() * 1e6) as u64;
            let addr = &*s as *const u64 as u64;
            *s = t ^ addr.rotate_left(29) ^ 0x9e37_79b9_7f4a_7c15;
        }
        let mut out = [0u8; 16];
        for chunk in out.chunks_mut(8) {
            *s ^= *s << 13;
            *s ^= *s >> 7;
            *s ^= *s << 17;
            chunk.copy_from_slice(&s.to_le_bytes());
        }
        out
    })
}

pub fn clean_name(raw: &str) -> String {
    let s: String = raw
        .chars()
        .filter(|c| c.is_alphabetic() || *c == ' ' || *c == '-' || *c == '\'')
        .collect();
    let s = s.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut s: String = s.chars().take(MAX_NAME).collect();
    if let Some(f) = s.chars().next() {
        s = f.to_uppercase().collect::<String>() + &s[f.len_utf8()..];
    }
    s
}

pub fn list() -> Vec<Profile> {
    let ids = save::get_str("profiles").unwrap_or_default();
    ids.split(',')
        .filter(|id| is_uuid(id))
        .filter_map(load)
        .collect()
}

pub fn load(id: &str) -> Option<Profile> {
    let name = save::get_str(&format!("p.{id}.name"))?;
    Some(Profile {
        id: id.to_string(),
        name,
        avatar: (save::get(&format!("p.{id}.avatar")).max(0) as usize).min(AVATARS.len() - 1),
        created: save::get(&format!("p.{id}.created")),
    })
}

pub fn create(name: &str, avatar: usize) -> Option<Profile> {
    let name = clean_name(name);
    if name.is_empty() || list().len() >= MAX_PROFILES {
        return None;
    }
    let id = uuid_v4(entropy());
    save::set_str(&format!("p.{id}.name"), &name);
    save::set(
        &format!("p.{id}.avatar"),
        avatar.min(AVATARS.len() - 1) as i64,
    );
    save::set(&format!("p.{id}.created"), now_secs());
    let mut ids: Vec<String> = list().into_iter().map(|p| p.id).collect();
    ids.push(id.clone());
    save::set_str("profiles", &ids.join(","));
    select(&id);
    load(&id)
}

pub fn adopt(id: &str, name: &str, avatar: usize) -> Option<Profile> {
    let name = clean_name(name);
    if !is_uuid(id) || name.is_empty() {
        return None;
    }
    if load(id).is_none() {
        if list().len() >= MAX_PROFILES {
            return None;
        }
        save::set_str(&format!("p.{id}.name"), &name);
        save::set(&format!("p.{id}.created"), now_secs());
        let mut ids: Vec<String> = list().into_iter().map(|p| p.id).collect();
        ids.push(id.to_string());
        save::set_str("profiles", &ids.join(","));
    }
    save::set(
        &format!("p.{id}.avatar"),
        avatar.min(AVATARS.len() - 1) as i64,
    );
    select(id);
    load(id)
}

pub fn reassign(old: &str) -> Option<String> {
    load(old)?;
    let new = uuid_v4(entropy());
    for (from, to) in [
        (format!("p.{old}."), format!("p.{new}.")),
        (format!("net.{old}."), format!("net.{new}.")),
    ] {
        for k in save::keys_with_prefix(&from) {
            if let Some(v) = save::get_str(&k) {
                save::set_str(&format!("{to}{}", &k[from.len()..]), &v);
            }
        }
        save::remove_prefix(&from);
    }
    let ids: Vec<String> = list_ids()
        .into_iter()
        .map(|id| if id == old { new.clone() } else { id })
        .collect();
    save::set_str("profiles", &ids.join(","));
    for k in save::keys_with_prefix("board.") {
        let mut b = parse_board(&save::get_str(&k).unwrap_or_default());
        let mut hit = false;
        for e in b.iter_mut().filter(|e| e.player == old) {
            e.player = new.clone();
            hit = true;
        }
        if hit {
            save::set_str(&k, &format_board(&b));
        }
    }
    if save::get_str("current").as_deref() == Some(old) {
        select(&new);
    }
    Some(new)
}

fn list_ids() -> Vec<String> {
    save::get_str("profiles")
        .unwrap_or_default()
        .split(',')
        .filter(|id| is_uuid(id))
        .map(String::from)
        .collect()
}

pub fn rename(id: &str, name: &str, avatar: usize) {
    let name = clean_name(name);
    if !name.is_empty() {
        save::set_str(&format!("p.{id}.name"), &name);
    }
    save::set(
        &format!("p.{id}.avatar"),
        avatar.min(AVATARS.len() - 1) as i64,
    );
}

pub fn delete(id: &str) {
    let ids: Vec<String> = list()
        .into_iter()
        .map(|p| p.id)
        .filter(|p| p != id)
        .collect();
    save::set_str("profiles", &ids.join(","));
    save::remove_prefix(&format!("p.{id}."));
    crate::net::forget_profile(id);
    if current_id().as_deref() == Some(id) {
        save::set_str("current", "");
    }
}

pub fn select(id: &str) {
    save::set_str("current", id);
}

pub fn current_id() -> Option<String> {
    save::get_str("current").filter(|id| is_uuid(id) && load(id).is_some())
}

pub fn current() -> Option<Profile> {
    current_id().and_then(|id| load(&id))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Better {
    Higher,
    Lower,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub score: i64,
    pub player: String,
    pub when: i64,
}

fn board_key(game: &str) -> String {
    format!("board.{game}")
}

pub fn parse_board(s: &str) -> Vec<Entry> {
    s.split(';')
        .filter_map(|e| {
            let mut f = e.split(',');
            Some(Entry {
                score: f.next()?.parse().ok()?,
                player: f
                    .next()
                    .filter(|p| is_uuid(p) || *p == "guest")?
                    .to_string(),
                when: f.next()?.parse().ok()?,
            })
        })
        .collect()
}

pub fn format_board(b: &[Entry]) -> String {
    b.iter()
        .map(|e| format!("{},{},{}", e.score, e.player, e.when))
        .collect::<Vec<_>>()
        .join(";")
}

pub fn insert_ranked(board: &mut Vec<Entry>, e: Entry, better: Better) -> Option<usize> {
    let beats = |a: i64, b: i64| match better {
        Better::Higher => a > b,
        Better::Lower => a < b,
    };
    let at = board
        .iter()
        .position(|x| beats(e.score, x.score))
        .unwrap_or(board.len());
    if at >= BOARD_SIZE {
        return None;
    }
    board.insert(at, e);
    board.truncate(BOARD_SIZE);
    Some(at)
}

pub fn board(game: &str) -> Vec<Entry> {
    parse_board(&save::get_str(&board_key(game)).unwrap_or_default())
}

pub fn submit(game: &str, score: i64, better: Better) -> Option<usize> {
    if better == Better::Higher && score <= 0 {
        return None;
    }
    crate::net::on_score(game, score);
    let player = current_id().unwrap_or_else(|| "guest".into());
    let mut b = board(game);
    let rank = insert_ranked(
        &mut b,
        Entry {
            score,
            player,
            when: now_secs(),
        },
        better,
    )?;
    save::set_str(&board_key(game), &format_board(&b));
    let best_key = format!("{game}.best");
    let prev = save::player_get_str(&best_key).and_then(|s| s.parse::<i64>().ok());
    let improved = match (prev, better) {
        (None, _) => true,
        (Some(p), Better::Higher) => score > p,
        (Some(p), Better::Lower) => score < p,
    };
    if improved {
        save::player_set(&best_key, score);
    }
    Some(rank)
}

pub fn personal_best(game: &str) -> Option<i64> {
    save::player_get_str(&format!("{game}.best")).and_then(|s| s.parse().ok())
}

pub fn player_name(id: &str) -> String {
    load(id).map(|p| p.name).unwrap_or_else(|| "Guest".into())
}

pub fn player_avatar(id: &str) -> usize {
    load(id).map(|p| p.avatar).unwrap_or(0)
}

pub fn level(game: &str) -> u32 {
    save::player_get(&format!("{game}.level")).max(0) as u32
}

pub fn set_level(game: &str, level: u32) {
    save::player_set(&format!("{game}.level"), level as i64);
}

pub fn unlocked(game: &str) -> u32 {
    save::player_get(&format!("{game}.unlocked")).max(0) as u32
}

pub fn unlock(game: &str, level: u32) -> bool {
    save::player_raise(&format!("{game}.unlocked"), level as i64)
}

pub fn stars(game: &str, level: u32) -> u8 {
    save::player_get(&format!("{game}.stars.{level}")).clamp(0, 3) as u8
}

pub fn award_stars(game: &str, level: u32, stars: u8) -> bool {
    save::player_raise(&format!("{game}.stars.{level}"), stars.min(3) as i64)
}

pub fn total_stars(game: &str) -> u32 {
    let prefix = save::player_key(&format!("{game}.stars."));
    save::keys_with_prefix(&prefix)
        .iter()
        .map(|k| save::get(k).clamp(0, 3) as u32)
        .sum()
}

pub fn get(game: &str, key: &str) -> i64 {
    save::player_get(&format!("{game}.{key}"))
}

pub fn set(game: &str, key: &str, value: i64) {
    save::player_set(&format!("{game}.{key}"), value);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fresh() {
        save::reset_for_test();
    }

    #[test]
    fn uuids_are_version_4_and_well_formed() {
        let u = uuid_v4([0xab; 16]);
        assert!(is_uuid(&u), "{u}");
        assert_eq!(&u[14..15], "4");
        assert!("89ab".contains(&u[19..20]));
        assert_ne!(uuid_v4(entropy()), uuid_v4(entropy()));
        assert!(!is_uuid("guest"));
    }

    #[test]
    fn names_are_cleaned_and_capped() {
        assert_eq!(clean_name("  mia  "), "Mia");
        assert_eq!(clean_name("x<script>"), "Xscript");
        assert_eq!(clean_name("élodie"), "Élodie");
        assert_eq!(
            clean_name("abcdefghijklmnopqrstuvwxyz").chars().count(),
            MAX_NAME
        );
        assert_eq!(clean_name("123"), "");
    }

    #[test]
    fn profiles_keep_their_own_progress() {
        fresh();
        let a = create("mia", 2).unwrap();
        set_level("spell", 4);
        award_stars("spell", 1, 3);
        let b = create("leo", 5).unwrap();
        assert_eq!(current_id(), Some(b.id.clone()));
        assert_eq!(level("spell"), 0);
        set_level("spell", 1);
        select(&a.id);
        assert_eq!(level("spell"), 4);
        assert_eq!(total_stars("spell"), 3);
        assert_eq!(list().len(), 2);
        assert_eq!(list()[0].name, "Mia");
    }

    #[test]
    fn deleting_a_profile_wipes_its_data_only() {
        fresh();
        let a = create("mia", 0).unwrap();
        set_level("crossy", 3);
        let b = create("leo", 1).unwrap();
        set_level("crossy", 7);
        delete(&a.id);
        assert!(load(&a.id).is_none());
        assert_eq!(list().len(), 1);
        select(&b.id);
        assert_eq!(level("crossy"), 7);
        assert!(save::keys_with_prefix(&format!("p.{}.", a.id)).is_empty());
    }

    #[test]
    fn empty_names_and_too_many_profiles_are_refused() {
        fresh();
        assert!(create("  ", 0).is_none());
        for i in 0..MAX_PROFILES {
            assert!(create(&format!("kid{}", "abcdefgh".chars().nth(i).unwrap()), 0).is_some());
        }
        assert!(create("extra", 0).is_none());
    }

    #[test]
    fn leaderboard_keeps_the_top_ten_in_order() {
        let mut b = vec![];
        for s in [5, 9, 1, 7, 3, 8, 2, 6, 4, 10, 11, 0] {
            insert_ranked(
                &mut b,
                Entry {
                    score: s,
                    player: "guest".into(),
                    when: 0,
                },
                Better::Higher,
            );
        }
        let scores: Vec<i64> = b.iter().map(|e| e.score).collect();
        assert_eq!(scores, vec![11, 10, 9, 8, 7, 6, 5, 4, 3, 2]);
        assert_eq!(parse_board(&format_board(&b)), b);
    }

    #[test]
    fn lower_is_better_boards_rank_fewest_moves_first() {
        fresh();
        create("mia", 0).unwrap();
        assert_eq!(submit("memory", 20, Better::Lower), Some(0));
        assert_eq!(submit("memory", 14, Better::Lower), Some(0));
        assert_eq!(submit("memory", 30, Better::Lower), Some(2));
        assert_eq!(personal_best("memory"), Some(14));
        assert_eq!(board("memory")[0].score, 14);
    }

    #[test]
    fn siblings_share_one_board_but_keep_their_own_bests() {
        fresh();
        let a = create("mia", 0).unwrap();
        submit("flappy", 12, Better::Higher);
        let b = create("leo", 1).unwrap();
        assert_eq!(submit("flappy", 20, Better::Higher), Some(0));
        assert_eq!(personal_best("flappy"), Some(20));
        select(&a.id);
        assert_eq!(personal_best("flappy"), Some(12));
        let board = board("flappy");
        assert_eq!(board[0].player, b.id);
        assert_eq!(player_name(&board[1].player), "Mia");
        assert_eq!(submit("flappy", 0, Better::Higher), None);
    }
}
