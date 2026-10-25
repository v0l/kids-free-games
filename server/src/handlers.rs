use crate::db::{n, s, Db};
use crate::error::{ApiError, ApiResult};
use crate::games::{self, Better};
use crate::rules::{self, *};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

pub struct Auth {
    pub player_id: String,
}

#[derive(Serialize, Deserialize)]
pub struct PlayerRow {
    pub id: String,
    pub name: String,
    pub avatar: i64,
    pub created_at: i64,
}

#[derive(Deserialize)]
struct AuthRow {
    player_id: String,
    last_used: i64,
}

pub async fn authenticate(db: &Db, token: Option<&str>, now: i64) -> ApiResult<Auth> {
    let token = token
        .filter(|t| looks_like_token(t))
        .ok_or_else(ApiError::unauthorized)?;
    let hash = hash_token(token);
    let row: AuthRow = db
        .one(
            "SELECT player_id, last_used FROM tokens WHERE hash = ?1",
            &[s(&hash)],
        )
        .await?
        .ok_or_else(ApiError::unauthorized)?;
    if now - row.last_used > TOKEN_TOUCH_AFTER {
        db.batch(vec![
            db.stmt(
                "UPDATE tokens SET last_used = ?2 WHERE hash = ?1",
                &[s(&hash), n(now)],
            )?,
            db.stmt(
                "UPDATE players SET last_seen = ?2 WHERE id = ?1",
                &[s(&row.player_id), n(now)],
            )?,
        ])
        .await?;
    }
    Ok(Auth {
        player_id: row.player_id,
    })
}

#[derive(Deserialize)]
pub struct RegisterBody {
    pub name: String,
    #[serde(default)]
    pub avatar: i64,
    #[serde(default)]
    pub device: String,
}

fn device_label(raw: &str) -> String {
    raw.chars().filter(|c| !c.is_control()).take(40).collect()
}

async fn insert_token(db: &Db, player_id: &str, label: &str, now: i64) -> ApiResult<String> {
    let token = new_token()?;
    db.batch(vec![
        db.stmt(
            "INSERT INTO tokens (hash, player_id, label, created_at, last_used) VALUES (?1, ?2, ?3, ?4, ?4)",
            &[s(&hash_token(&token)), s(player_id), s(&device_label(label)), n(now)],
        )?,
        db.stmt(
            "DELETE FROM tokens WHERE player_id = ?1 AND hash NOT IN (SELECT hash FROM tokens WHERE player_id = ?1 ORDER BY last_used DESC, created_at DESC LIMIT ?2)",
            &[s(player_id), n(MAX_TOKENS_PER_PLAYER)],
        )?,
    ])
    .await?;
    Ok(token)
}

pub async fn register(db: &Db, body: RegisterBody, now: i64) -> ApiResult<Value> {
    let name = valid_name(&body.name)?;
    let avatar = valid_avatar(body.avatar)?;
    let id = new_player_id()?;
    db.run(
        "INSERT INTO players (id, name, avatar, created_at, last_seen) VALUES (?1, ?2, ?3, ?4, ?4)",
        &[s(&id), s(&name), n(avatar), n(now)],
    )
    .await?;
    let token = insert_token(db, &id, &body.device, now).await?;
    Ok(json!({
        "player": { "id": id, "name": name, "avatar": avatar, "created_at": now },
        "token": token,
    }))
}

async fn load_player(db: &Db, id: &str) -> ApiResult<PlayerRow> {
    db.one(
        "SELECT id, name, avatar, created_at FROM players WHERE id = ?1",
        &[s(id)],
    )
    .await?
    .ok_or_else(ApiError::unauthorized)
}

#[derive(Deserialize)]
struct TotalRow {
    seconds: i64,
    sessions: i64,
}

pub async fn get_me(db: &Db, auth: &Auth) -> ApiResult<Value> {
    let player = load_player(db, &auth.player_id).await?;
    let total: Option<TotalRow> = db
        .one(
            "SELECT COALESCE(SUM(seconds), 0) AS seconds, COUNT(*) AS sessions FROM play_sessions WHERE player_id = ?1",
            &[s(&auth.player_id)],
        )
        .await?;
    let (seconds, sessions) = total.map(|t| (t.seconds, t.sessions)).unwrap_or((0, 0));
    Ok(json!({
        "player": player,
        "playtime": { "total_seconds": seconds, "sessions": sessions },
    }))
}

#[derive(Deserialize)]
pub struct PatchBody {
    pub name: Option<String>,
    pub avatar: Option<i64>,
}

pub async fn patch_me(db: &Db, auth: &Auth, body: PatchBody) -> ApiResult<Value> {
    let name = body.name.as_deref().map(valid_name).transpose()?;
    let avatar = body.avatar.map(valid_avatar).transpose()?;
    if name.is_none() && avatar.is_none() {
        return Err(ApiError::bad_request("nothing to change"));
    }
    db.run(
        "UPDATE players SET name = COALESCE(?2, name), avatar = COALESCE(?3, avatar) WHERE id = ?1",
        &[
            s(&auth.player_id),
            name.as_deref()
                .map(s)
                .unwrap_or(worker::wasm_bindgen::JsValue::NULL),
            avatar.map(n).unwrap_or(worker::wasm_bindgen::JsValue::NULL),
        ],
    )
    .await?;
    Ok(json!({ "player": load_player(db, &auth.player_id).await? }))
}

pub async fn delete_me(db: &Db, auth: &Auth) -> ApiResult<Value> {
    let tables = [
        "DELETE FROM saves WHERE player_id = ?1",
        "DELETE FROM scores WHERE player_id = ?1",
        "DELETE FROM play_sessions WHERE player_id = ?1",
        "DELETE FROM link_codes WHERE player_id = ?1",
        "DELETE FROM tokens WHERE player_id = ?1",
        "DELETE FROM players WHERE id = ?1",
    ];
    let stmts = tables
        .iter()
        .map(|sql| db.stmt(sql, &[s(&auth.player_id)]))
        .collect::<ApiResult<Vec<_>>>()?;
    db.batch(stmts).await?;
    Ok(json!({ "deleted": true }))
}

pub async fn create_link(db: &Db, auth: &Auth, now: i64) -> ApiResult<Value> {
    let code = new_link_code()?;
    let expires = now + LINK_CODE_TTL;
    db.batch(vec![
        db.stmt("DELETE FROM link_codes WHERE expires_at < ?1", &[n(now)])?,
        db.stmt(
            "DELETE FROM link_codes WHERE player_id = ?1",
            &[s(&auth.player_id)],
        )?,
        db.stmt(
            "INSERT INTO link_codes (code, player_id, expires_at) VALUES (?1, ?2, ?3)",
            &[s(&code), s(&auth.player_id), n(expires)],
        )?,
    ])
    .await?;
    Ok(json!({ "code": display_code(&code), "expires_at": expires, "expires_in": LINK_CODE_TTL }))
}

#[derive(Deserialize)]
pub struct RedeemBody {
    pub code: String,
    #[serde(default)]
    pub device: String,
}

#[derive(Deserialize)]
struct CodeRow {
    player_id: String,
}

pub async fn redeem_link(db: &Db, body: RedeemBody, now: i64) -> ApiResult<Value> {
    let invalid = || ApiError::not_found("that code is wrong or has expired");
    let code = normalize_code(&body.code).ok_or_else(invalid)?;
    let row: CodeRow = db
        .one(
            "SELECT player_id FROM link_codes WHERE code = ?1 AND expires_at >= ?2",
            &[s(&code), n(now)],
        )
        .await?
        .ok_or_else(invalid)?;
    let used = db
        .run("DELETE FROM link_codes WHERE code = ?1", &[s(&code)])
        .await?;
    if used == 0 {
        return Err(invalid());
    }
    let token = insert_token(db, &row.player_id, &body.device, now).await?;
    let player = load_player(db, &row.player_id).await?;
    Ok(json!({ "player": player, "token": token }))
}

#[derive(Serialize, Deserialize)]
struct SaveRow {
    game: String,
    rev: i64,
    data: String,
    updated_at: i64,
}

#[derive(Deserialize)]
struct SaveMeta {
    game: String,
    rev: i64,
    size: i64,
    updated_at: i64,
}

fn check_slot(slot: &str) -> ApiResult<()> {
    if games::is_save_slot(slot) {
        Ok(())
    } else {
        Err(ApiError::not_found(format!("unknown save slot {slot}")))
    }
}

pub async fn list_saves(db: &Db, auth: &Auth) -> ApiResult<Value> {
    let rows: Vec<SaveMeta> = db
        .all(
            "SELECT game, rev, LENGTH(data) AS size, updated_at FROM saves WHERE player_id = ?1 ORDER BY game",
            &[s(&auth.player_id)],
        )
        .await?;
    let saves: Vec<Value> = rows
        .into_iter()
        .map(
            |r| json!({ "slot": r.game, "rev": r.rev, "size": r.size, "updated_at": r.updated_at }),
        )
        .collect();
    Ok(json!({ "saves": saves }))
}

pub async fn get_save(db: &Db, auth: &Auth, slot: &str) -> ApiResult<Value> {
    check_slot(slot)?;
    let row: SaveRow = db
        .one(
            "SELECT game, rev, data, updated_at FROM saves WHERE player_id = ?1 AND game = ?2",
            &[s(&auth.player_id), s(slot)],
        )
        .await?
        .ok_or_else(|| ApiError::not_found("no save in that slot yet"))?;
    Ok(json!({ "slot": row.game, "rev": row.rev, "data": row.data, "updated_at": row.updated_at }))
}

#[derive(Deserialize)]
pub struct PutSaveBody {
    pub data: String,
    #[serde(default)]
    pub base_rev: i64,
}

pub async fn put_save(
    db: &Db,
    auth: &Auth,
    slot: &str,
    body: PutSaveBody,
    now: i64,
) -> ApiResult<Value> {
    check_slot(slot)?;
    valid_save_data(&body.data)?;
    if body.base_rev < 0 {
        return Err(ApiError::bad_request("base_rev must be 0 or more"));
    }
    let changed = if body.base_rev == 0 {
        db.run(
            "INSERT INTO saves (player_id, game, rev, data, updated_at) VALUES (?1, ?2, 1, ?3, ?4) ON CONFLICT (player_id, game) DO NOTHING",
            &[s(&auth.player_id), s(slot), s(&body.data), n(now)],
        )
        .await?
    } else {
        db.run(
            "UPDATE saves SET data = ?3, rev = rev + 1, updated_at = ?4 WHERE player_id = ?1 AND game = ?2 AND rev = ?5",
            &[s(&auth.player_id), s(slot), s(&body.data), n(now), n(body.base_rev)],
        )
        .await?
    };
    if changed == 0 {
        let current: Option<SaveMeta> = db
            .one(
                "SELECT game, rev, LENGTH(data) AS size, updated_at FROM saves WHERE player_id = ?1 AND game = ?2",
                &[s(&auth.player_id), s(slot)],
            )
            .await?;
        let rev = current.as_ref().map(|c| c.rev).unwrap_or(0);
        let updated = current.as_ref().map(|c| c.updated_at).unwrap_or(0);
        return Err(ApiError::conflict(
            "rev_mismatch",
            "the save changed on another device, fetch it and merge",
        )
        .with_extra(json!({ "rev": rev, "updated_at": updated })));
    }
    Ok(json!({ "slot": slot, "rev": body.base_rev + 1, "updated_at": now }))
}

#[derive(Deserialize)]
pub struct ScoreBody {
    pub score: i64,
}

#[derive(Deserialize)]
struct ScoreRow {
    score: i64,
    achieved_at: i64,
}

#[derive(Deserialize)]
struct CountRow {
    c: i64,
}

fn order_by(better: Better) -> &'static str {
    match better {
        Better::Higher => "s.score DESC, s.achieved_at ASC, s.player_id ASC",
        Better::Lower => "s.score ASC, s.achieved_at ASC, s.player_id ASC",
    }
}

fn upsert_sql(better: Better) -> &'static str {
    match better {
        Better::Higher => "INSERT INTO scores (game, player_id, score, achieved_at, plays) VALUES (?1, ?2, ?3, ?4, 1) ON CONFLICT (game, player_id) DO UPDATE SET plays = plays + 1, achieved_at = CASE WHEN excluded.score > score THEN excluded.achieved_at ELSE achieved_at END, score = MAX(score, excluded.score)",
        Better::Lower => "INSERT INTO scores (game, player_id, score, achieved_at, plays) VALUES (?1, ?2, ?3, ?4, 1) ON CONFLICT (game, player_id) DO UPDATE SET plays = plays + 1, achieved_at = CASE WHEN excluded.score < score THEN excluded.achieved_at ELSE achieved_at END, score = MIN(score, excluded.score)",
    }
}

fn rank_sql(better: Better) -> String {
    let cmp = match better {
        Better::Higher => ">",
        Better::Lower => "<",
    };
    format!(
        "SELECT COUNT(*) AS c FROM scores WHERE game = ?1 AND (score {cmp} ?2 OR (score = ?2 AND (achieved_at < ?3 OR (achieved_at = ?3 AND player_id < ?4))))"
    )
}

async fn rank_of(db: &Db, game: &str, better: Better, me: &str, mine: &ScoreRow) -> ApiResult<i64> {
    let c: Option<CountRow> = db
        .one(
            &rank_sql(better),
            &[s(game), n(mine.score), n(mine.achieved_at), s(me)],
        )
        .await?;
    Ok(c.map(|c| c.c).unwrap_or(0) + 1)
}

fn game_direction(game: &str) -> ApiResult<Better> {
    games::direction(game).ok_or_else(|| ApiError::not_found(format!("unknown game {game}")))
}

pub async fn post_score(
    db: &Db,
    auth: &Auth,
    game: &str,
    body: ScoreBody,
    now: i64,
) -> ApiResult<Value> {
    let better = game_direction(game)?;
    let score = valid_score(better, body.score)?;
    let before: Option<ScoreRow> = db
        .one(
            "SELECT score, achieved_at FROM scores WHERE game = ?1 AND player_id = ?2",
            &[s(game), s(&auth.player_id)],
        )
        .await?;
    db.run(
        upsert_sql(better),
        &[s(game), s(&auth.player_id), n(score), n(now)],
    )
    .await?;
    let mine: ScoreRow = db
        .one(
            "SELECT score, achieved_at FROM scores WHERE game = ?1 AND player_id = ?2",
            &[s(game), s(&auth.player_id)],
        )
        .await?
        .ok_or_else(|| ApiError::internal("score row missing after upsert"))?;
    let rank = rank_of(db, game, better, &auth.player_id, &mine).await?;
    let improved = before.map(|b| better.beats(score, b.score)).unwrap_or(true);
    Ok(json!({ "game": game, "best": mine.score, "improved": improved, "rank": rank }))
}

#[derive(Deserialize)]
struct BoardRow {
    player_id: String,
    name: String,
    avatar: i64,
    score: i64,
    achieved_at: i64,
}

pub async fn get_board(db: &Db, auth: &Auth, game: &str, limit: Option<&str>) -> ApiResult<Value> {
    let better = game_direction(game)?;
    let limit = clamp_limit(limit, DEFAULT_BOARD, MAX_BOARD);
    let sql = format!(
        "SELECT s.player_id, p.name, p.avatar, s.score, s.achieved_at FROM scores s JOIN players p ON p.id = s.player_id WHERE s.game = ?1 ORDER BY {} LIMIT ?2",
        order_by(better)
    );
    let rows: Vec<BoardRow> = db.all(&sql, &[s(game), n(limit)]).await?;
    let entries: Vec<Value> = rows
        .iter()
        .enumerate()
        .map(|(i, r)| {
            json!({
                "rank": i + 1,
                "player_id": r.player_id,
                "name": r.name,
                "avatar": r.avatar,
                "score": r.score,
                "achieved_at": r.achieved_at,
                "you": r.player_id == auth.player_id,
            })
        })
        .collect();
    let mine: Option<ScoreRow> = db
        .one(
            "SELECT score, achieved_at FROM scores WHERE game = ?1 AND player_id = ?2",
            &[s(game), s(&auth.player_id)],
        )
        .await?;
    let me = match mine {
        Some(m) => {
            let rank = rank_of(db, game, better, &auth.player_id, &m).await?;
            json!({ "rank": rank, "score": m.score, "achieved_at": m.achieved_at })
        }
        None => Value::Null,
    };
    Ok(json!({
        "game": game,
        "better": better.as_str(),
        "entries": entries,
        "me": me,
    }))
}

#[derive(Deserialize)]
pub struct PlaytimeBody {
    pub sessions: Vec<SessionIn>,
}

#[derive(Deserialize)]
pub struct SessionIn {
    pub id: String,
    pub game: String,
    pub started_at: i64,
    pub seconds: i64,
}

pub async fn post_playtime(db: &Db, auth: &Auth, body: PlaytimeBody, now: i64) -> ApiResult<Value> {
    if body.sessions.is_empty() || body.sessions.len() > MAX_SESSIONS_PER_REQUEST {
        return Err(ApiError::bad_request(format!(
            "send 1 to {MAX_SESSIONS_PER_REQUEST} sessions at a time"
        )));
    }
    for x in &body.sessions {
        valid_play_session(&x.id, &x.game, x.started_at, x.seconds, now)?;
    }
    let stmts = body
        .sessions
        .iter()
        .map(|x| {
            db.stmt(
                "INSERT INTO play_sessions (player_id, id, game, started_at, seconds) VALUES (?1, ?2, ?3, ?4, ?5) ON CONFLICT (player_id, id) DO UPDATE SET seconds = MAX(seconds, excluded.seconds) WHERE excluded.game = game AND excluded.started_at = started_at AND excluded.seconds > seconds",
                &[s(&auth.player_id), s(&x.id), s(&x.game), n(x.started_at), n(x.seconds)],
            )
        })
        .collect::<ApiResult<Vec<_>>>()?;
    let changes = db.batch(stmts).await?;
    let applied = changes.iter().filter(|c| **c > 0).count();
    Ok(json!({ "received": body.sessions.len(), "applied": applied }))
}

#[derive(Deserialize)]
struct GameTime {
    game: String,
    seconds: i64,
    sessions: i64,
}

#[derive(Deserialize)]
struct DayTime {
    day: i64,
    seconds: i64,
}

pub async fn get_playtime(db: &Db, auth: &Auth, days: Option<&str>, now: i64) -> ApiResult<Value> {
    let days = clamp_limit(days, DEFAULT_DAYS, MAX_DAYS);
    let today = now.div_euclid(86_400);
    let since = (today - days + 1) * 86_400;
    let by_game: Vec<GameTime> = db
        .all(
            "SELECT game, SUM(seconds) AS seconds, COUNT(*) AS sessions FROM play_sessions WHERE player_id = ?1 GROUP BY game ORDER BY seconds DESC, game ASC",
            &[s(&auth.player_id)],
        )
        .await?;
    let by_day: Vec<DayTime> = db
        .all(
            "SELECT started_at / 86400 AS day, SUM(seconds) AS seconds FROM play_sessions WHERE player_id = ?1 AND started_at >= ?2 GROUP BY day ORDER BY day ASC",
            &[s(&auth.player_id), n(since)],
        )
        .await?;
    let total: i64 = by_game.iter().map(|g| g.seconds).sum();
    let sessions: i64 = by_game.iter().map(|g| g.sessions).sum();
    Ok(json!({
        "total_seconds": total,
        "sessions": sessions,
        "by_game": by_game.iter().map(|g| json!({ "game": g.game, "seconds": g.seconds, "sessions": g.sessions })).collect::<Vec<_>>(),
        "days": days,
        "by_day": by_day.iter().map(|d| json!({ "day": rules::day_string(d.day), "seconds": d.seconds })).collect::<Vec<_>>(),
    }))
}

pub fn games_list() -> Value {
    json!({
        "games": games::GAMES
            .iter()
            .map(|(g, b)| json!({ "id": g, "better": b.as_str() }))
            .collect::<Vec<_>>(),
        "save_slots": std::iter::once(games::PROFILE_SAVE)
            .chain(games::GAMES.iter().map(|(g, _)| *g))
            .collect::<Vec<_>>(),
        "limits": {
            "max_save_bytes": MAX_SAVE_BYTES,
            "max_session_seconds": MAX_SESSION_SECONDS,
            "max_sessions_per_request": MAX_SESSIONS_PER_REQUEST,
            "max_board": MAX_BOARD,
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rank_sql_matches_board_order_for_both_directions() {
        assert!(rank_sql(Better::Higher).contains("score > ?2"));
        assert!(rank_sql(Better::Lower).contains("score < ?2"));
        assert!(order_by(Better::Higher).starts_with("s.score DESC"));
        assert!(order_by(Better::Lower).starts_with("s.score ASC"));
    }

    #[test]
    fn games_list_names_every_game_and_the_profile_slot() {
        let v = games_list();
        assert_eq!(v["games"].as_array().unwrap().len(), games::GAMES.len());
        let slots = v["save_slots"].as_array().unwrap();
        assert_eq!(slots[0], "profile");
        assert_eq!(slots.len(), games::GAMES.len() + 1);
    }

    #[test]
    fn device_labels_are_trimmed_and_stripped() {
        assert_eq!(device_label("Pixel\n7"), "Pixel7");
        assert_eq!(device_label(&"x".repeat(100)).len(), 40);
    }
}
