use crate::error::{ApiError, ApiResult};
use crate::games::Better;
use sha2::{Digest, Sha256};

pub const MAX_NAME: usize = 12;
pub const AVATARS: i64 = 12;
pub const MAX_SAVE_BYTES: usize = 64 * 1024;
pub const MAX_BODY_BYTES: usize = 128 * 1024;
pub const MAX_SCORE: i64 = 1_000_000_000;
pub const MAX_SESSION_SECONDS: i64 = 4 * 60 * 60;
pub const MAX_SESSIONS_PER_REQUEST: usize = 50;
pub const MIN_STARTED_AT: i64 = 1_577_836_800;
pub const FUTURE_SLACK: i64 = 300;
pub const LINK_CODE_TTL: i64 = 600;
pub const TOKEN_TOUCH_AFTER: i64 = 24 * 60 * 60;
pub const MAX_TOKENS_PER_PLAYER: i64 = 20;
pub const DEFAULT_BOARD: i64 = 10;
pub const MAX_BOARD: i64 = 50;
pub const DEFAULT_DAYS: i64 = 14;
pub const MAX_DAYS: i64 = 90;
pub const TOKEN_PREFIX: &str = "kgt_";
const CODE_ALPHABET: &[u8; 32] = b"ABCDEFGHJKLMNPQRSTUVWXYZ23456789";
const CODE_LEN: usize = 8;

pub fn random_bytes<const N: usize>() -> ApiResult<[u8; N]> {
    let mut b = [0u8; N];
    getrandom::getrandom(&mut b).map_err(ApiError::internal)?;
    Ok(b)
}

pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

pub fn new_token() -> ApiResult<String> {
    Ok(format!("{TOKEN_PREFIX}{}", hex(&random_bytes::<32>()?)))
}

pub fn looks_like_token(t: &str) -> bool {
    t.len() == TOKEN_PREFIX.len() + 64
        && t.starts_with(TOKEN_PREFIX)
        && t[TOKEN_PREFIX.len()..]
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

pub fn hash_token(token: &str) -> String {
    hex(&Sha256::digest(token.as_bytes()))
}

#[cfg(test)]
pub fn is_uuid(s: &str) -> bool {
    let parts: Vec<&str> = s.split('-').collect();
    parts.iter().map(|p| p.len()).collect::<Vec<_>>() == [8, 4, 4, 4, 12]
        && s.bytes().all(|c| c == b'-' || c.is_ascii_hexdigit())
        && s == s.to_ascii_lowercase()
}

pub fn new_player_id() -> ApiResult<String> {
    let mut b = random_bytes::<16>()?;
    b[6] = (b[6] & 0x0f) | 0x40;
    b[8] = (b[8] & 0x3f) | 0x80;
    let h = hex(&b);
    Ok(format!(
        "{}-{}-{}-{}-{}",
        &h[0..8],
        &h[8..12],
        &h[12..16],
        &h[16..20],
        &h[20..32]
    ))
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

pub fn valid_name(raw: &str) -> ApiResult<String> {
    let name = clean_name(raw);
    if name.is_empty() {
        return Err(ApiError::bad_request("name needs at least one letter"));
    }
    Ok(name)
}

pub fn valid_avatar(avatar: i64) -> ApiResult<i64> {
    if (0..AVATARS).contains(&avatar) {
        Ok(avatar)
    } else {
        Err(ApiError::bad_request(format!(
            "avatar must be 0 to {}",
            AVATARS - 1
        )))
    }
}

pub fn new_link_code() -> ApiResult<String> {
    let b = random_bytes::<5>()?;
    let bits = b.iter().fold(0u64, |acc, x| (acc << 8) | *x as u64);
    let raw: String = (0..CODE_LEN)
        .map(|i| CODE_ALPHABET[((bits >> (5 * (CODE_LEN - 1 - i))) & 31) as usize] as char)
        .collect();
    Ok(raw)
}

pub fn display_code(code: &str) -> String {
    format!("{}-{}", &code[..4], &code[4..])
}

pub fn normalize_code(input: &str) -> Option<String> {
    let code: String = input
        .chars()
        .filter(|c| *c != '-' && !c.is_whitespace())
        .map(|c| c.to_ascii_uppercase())
        .collect();
    (code.len() == CODE_LEN && code.bytes().all(|b| CODE_ALPHABET.contains(&b))).then_some(code)
}

pub fn valid_score(better: Better, score: i64) -> ApiResult<i64> {
    if (1..=MAX_SCORE).contains(&score) {
        Ok(score)
    } else {
        Err(ApiError::bad_request(format!(
            "score must be 1 to {MAX_SCORE} ({} is better)",
            better.as_str()
        )))
    }
}

pub fn valid_save_data(data: &str) -> ApiResult<()> {
    if data.len() > MAX_SAVE_BYTES {
        return Err(ApiError::too_large(format!(
            "save is over {MAX_SAVE_BYTES} bytes"
        )));
    }
    Ok(())
}

pub fn valid_session_id(id: &str) -> bool {
    (8..=64).contains(&id.len())
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

pub fn valid_play_session(
    id: &str,
    game: &str,
    started_at: i64,
    seconds: i64,
    now: i64,
) -> ApiResult<()> {
    if !valid_session_id(id) {
        return Err(ApiError::bad_request(
            "session id must be 8 to 64 letters, digits, - or _",
        ));
    }
    if !crate::games::is_game(game) {
        return Err(ApiError::bad_request(format!("unknown game {game}")));
    }
    if !(0..=MAX_SESSION_SECONDS).contains(&seconds) {
        return Err(ApiError::bad_request(format!(
            "seconds must be 0 to {MAX_SESSION_SECONDS}"
        )));
    }
    if started_at < MIN_STARTED_AT || started_at + seconds > now + FUTURE_SLACK {
        return Err(ApiError::bad_request(
            "session time is outside the allowed range",
        ));
    }
    Ok(())
}

pub fn clamp_limit(raw: Option<&str>, default: i64, max: i64) -> i64 {
    raw.and_then(|s| s.parse::<i64>().ok())
        .map(|v| v.clamp(1, max))
        .unwrap_or(default)
}

pub fn day_string(unix_day: i64) -> String {
    let z = unix_day + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y:04}-{m:02}-{d:02}")
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOW: i64 = 1_760_000_000;

    #[test]
    fn tokens_are_random_prefixed_and_hash_stably() {
        let a = new_token().unwrap();
        let b = new_token().unwrap();
        assert_ne!(a, b);
        assert!(looks_like_token(&a));
        assert_eq!(hash_token(&a), hash_token(&a));
        assert_ne!(hash_token(&a), hash_token(&b));
        assert_eq!(hash_token(&a).len(), 64);
        assert_eq!(
            hash_token("abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn malformed_tokens_are_rejected_before_hitting_the_database() {
        assert!(!looks_like_token(""));
        assert!(!looks_like_token("kgt_short"));
        assert!(!looks_like_token(&format!("kgt_{}", "g".repeat(64))));
        assert!(!looks_like_token(&format!("xxx_{}", "a".repeat(64))));
        assert!(looks_like_token(&format!("kgt_{}", "a".repeat(64))));
    }

    #[test]
    fn player_ids_are_lowercase_v4_uuids() {
        let id = new_player_id().unwrap();
        assert!(is_uuid(&id), "{id}");
        assert_eq!(&id[14..15], "4");
        assert!("89ab".contains(&id[19..20]));
        assert_ne!(id, new_player_id().unwrap());
        assert!(!is_uuid("guest"));
        assert!(!is_uuid(&id.to_uppercase()));
        assert!(!is_uuid("zzzzzzzz-zzzz-zzzz-zzzz-zzzzzzzzzzzz"));
    }

    #[test]
    fn names_match_the_app_rules() {
        assert_eq!(clean_name("  mia  "), "Mia");
        assert_eq!(clean_name("x<script>"), "Xscript");
        assert_eq!(clean_name("élodie"), "Élodie");
        assert_eq!(
            clean_name("abcdefghijklmnopqrstuvwxyz").chars().count(),
            MAX_NAME
        );
        assert_eq!(clean_name("123"), "");
        assert!(valid_name("123").is_err());
        assert_eq!(valid_name("leo").unwrap(), "Leo");
    }

    #[test]
    fn avatars_are_bounded_to_the_app_set() {
        assert!(valid_avatar(0).is_ok());
        assert!(valid_avatar(11).is_ok());
        assert!(valid_avatar(12).is_err());
        assert!(valid_avatar(-1).is_err());
    }

    #[test]
    fn link_codes_use_the_unambiguous_alphabet_and_normalise() {
        for _ in 0..200 {
            let c = new_link_code().unwrap();
            assert_eq!(c.len(), 8);
            assert!(c.bytes().all(|b| CODE_ALPHABET.contains(&b)), "{c}");
            assert_eq!(normalize_code(&display_code(&c)), Some(c.clone()));
            assert_eq!(normalize_code(&c.to_lowercase()), Some(c));
        }
        assert_eq!(normalize_code("abcd efgh"), Some("ABCDEFGH".into()));
        assert_eq!(normalize_code("ABCD-EFG"), None);
        assert_eq!(normalize_code("ABCD-EFG0"), None);
        assert_eq!(normalize_code("ABCD-EFGI"), None);
        assert_eq!(normalize_code(""), None);
    }

    #[test]
    fn link_codes_cover_every_alphabet_symbol() {
        let mut seen = std::collections::HashSet::new();
        for _ in 0..2000 {
            seen.extend(new_link_code().unwrap().bytes());
        }
        assert_eq!(seen.len(), 32);
    }

    #[test]
    fn scores_must_be_positive_and_bounded() {
        assert!(valid_score(Better::Higher, 1).is_ok());
        assert!(valid_score(Better::Lower, MAX_SCORE).is_ok());
        assert!(valid_score(Better::Higher, 0).is_err());
        assert!(valid_score(Better::Higher, -5).is_err());
        assert!(valid_score(Better::Higher, MAX_SCORE + 1).is_err());
    }

    #[test]
    fn saves_over_the_cap_are_too_large() {
        assert!(valid_save_data(&"a".repeat(MAX_SAVE_BYTES)).is_ok());
        let e = valid_save_data(&"a".repeat(MAX_SAVE_BYTES + 1)).unwrap_err();
        assert_eq!(e.status, 413);
    }

    #[test]
    fn play_sessions_cannot_claim_the_future_or_absurd_lengths() {
        let ok = |s, secs| valid_play_session("abcd1234", "snake", s, secs, NOW).is_ok();
        assert!(ok(NOW - 600, 600));
        assert!(ok(NOW - 60, 0));
        assert!(ok(NOW - 10, 100));
        assert!(!ok(NOW - 10, 1000));
        assert!(!ok(NOW + 4000, 10));
        assert!(!ok(NOW - 100, -1));
        assert!(!ok(NOW - 100_000, MAX_SESSION_SECONDS + 1));
        assert!(!ok(100, 10));
        assert!(ok(NOW - 100_000, MAX_SESSION_SECONDS));
        assert!(valid_play_session("short", "snake", NOW - 10, 5, NOW).is_err());
        assert!(valid_play_session("bad id with spaces", "snake", NOW - 10, 5, NOW).is_err());
        assert!(valid_play_session("abcd1234", "nope", NOW - 10, 5, NOW).is_err());
    }

    #[test]
    fn limits_fall_back_and_clamp() {
        assert_eq!(clamp_limit(None, 10, 50), 10);
        assert_eq!(clamp_limit(Some("abc"), 10, 50), 10);
        assert_eq!(clamp_limit(Some("0"), 10, 50), 1);
        assert_eq!(clamp_limit(Some("500"), 10, 50), 50);
        assert_eq!(clamp_limit(Some("25"), 10, 50), 25);
    }

    #[test]
    fn days_format_as_utc_dates() {
        assert_eq!(day_string(0), "1970-01-01");
        assert_eq!(day_string(19_723), "2024-01-01");
        assert_eq!(day_string(19_782), "2024-02-29");
        assert_eq!(day_string(19_783), "2024-03-01");
        assert_eq!(day_string(NOW / 86_400), "2025-10-09");
        assert_eq!(day_string(-1), "1969-12-31");
    }
}
