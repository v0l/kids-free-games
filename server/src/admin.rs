use crate::db::{n, s, Db};
use crate::error::{ApiError, ApiResult};
use crate::games;
use crate::handlers::{self, order_by};
use crate::routes::Admin;
use crate::rules::{self, *};
use serde::Deserialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use worker::wasm_bindgen::JsValue;

pub const MAX_ADMIN_PAGE: i64 = 200;
pub const DEFAULT_ADMIN_PAGE: i64 = 50;

pub fn key_matches(configured: &str, presented: &str) -> bool {
    let a = Sha256::digest(configured.as_bytes());
    let b = Sha256::digest(presented.as_bytes());
    a.iter()
        .zip(b.iter())
        .fold(0u8, |acc, (x, y)| acc | (x ^ y))
        == 0
}

pub fn authorize(configured: Option<&str>, presented: Option<&str>) -> ApiResult<()> {
    let configured = configured
        .filter(|k| !k.is_empty())
        .ok_or_else(|| ApiError::not_found("admin is not enabled"))?;
    match presented {
        Some(p) if key_matches(configured, p) => Ok(()),
        _ => Err(ApiError::unauthorized()),
    }
}

#[derive(Deserialize)]
pub struct NewPlayer {
    pub name: String,
    #[serde(default)]
    pub avatar: i64,
}

#[derive(Deserialize)]
pub struct PlayerPatch {
    pub name: Option<String>,
    pub avatar: Option<i64>,
}

#[derive(Deserialize)]
pub struct SaveIn {
    pub data: String,
}

#[derive(Deserialize)]
pub struct ScoreIn {
    pub score: i64,
}

pub struct Params {
    pub q: Option<String>,
    pub limit: Option<String>,
    pub offset: Option<String>,
    pub sort: Option<String>,
}

#[derive(Deserialize)]
struct Count {
    c: i64,
}

#[derive(Deserialize)]
struct Seconds {
    seconds: i64,
}

#[derive(Deserialize)]
struct PlayerListRow {
    id: String,
    name: String,
    avatar: i64,
    created_at: i64,
    last_seen: i64,
    seconds: i64,
    scores: i64,
    saves: i64,
}

#[derive(Deserialize)]
struct GameSeconds {
    game: String,
    seconds: i64,
    sessions: i64,
    players: i64,
}

#[derive(Deserialize)]
struct DaySeconds {
    day: i64,
    seconds: i64,
}

#[derive(Deserialize)]
struct GameScores {
    game: String,
    players: i64,
    plays: i64,
}

#[derive(Deserialize)]
struct PlayerOut {
    id: String,
    name: String,
    avatar: i64,
    created_at: i64,
    last_seen: i64,
}

#[derive(Deserialize)]
struct SaveMetaRow {
    game: String,
    rev: i64,
    size: i64,
    updated_at: i64,
}

#[derive(Deserialize)]
struct PlayerScoreRow {
    game: String,
    score: i64,
    achieved_at: i64,
    plays: i64,
}

#[derive(Deserialize)]
struct DeviceRow {
    label: String,
    created_at: i64,
    last_used: i64,
}

#[derive(Deserialize)]
struct AdminBoardRow {
    player_id: String,
    name: String,
    avatar: i64,
    score: i64,
    achieved_at: i64,
    plays: i64,
}

#[derive(Deserialize)]
struct SaveRowOut {
    rev: i64,
    data: String,
    updated_at: i64,
}

fn count(row: Option<Count>) -> i64 {
    row.map(|c| c.c).unwrap_or(0)
}

fn like_pattern(q: &str) -> String {
    let escaped: String = q
        .chars()
        .flat_map(|c| match c {
            '%' | '_' | '\\' => vec!['\\', c],
            _ => vec![c],
        })
        .collect();
    format!("%{escaped}%")
}

fn sort_clause(sort: Option<&str>) -> &'static str {
    match sort {
        Some("name") => "p.name COLLATE NOCASE ASC, p.id ASC",
        Some("created") => "p.created_at DESC, p.id ASC",
        Some("playtime") => "seconds DESC, p.id ASC",
        _ => "p.last_seen DESC, p.id ASC",
    }
}

async fn player_exists(db: &Db, id: &str) -> ApiResult<PlayerOut> {
    db.one(
        "SELECT id, name, avatar, created_at, last_seen FROM players WHERE id = ?1",
        &[s(id)],
    )
    .await?
    .ok_or_else(|| ApiError::not_found("no such player"))
}

fn check_game(game: &str) -> ApiResult<games::Better> {
    games::direction(game).ok_or_else(|| ApiError::not_found(format!("unknown game {game}")))
}

fn check_slot(slot: &str) -> ApiResult<()> {
    if games::is_save_slot(slot) {
        Ok(())
    } else {
        Err(ApiError::not_found(format!("unknown save slot {slot}")))
    }
}

async fn stats(db: &Db, now: i64) -> ApiResult<Value> {
    let players = count(db.one("SELECT COUNT(*) AS c FROM players", &[]).await?);
    let active_day = count(
        db.one(
            "SELECT COUNT(*) AS c FROM players WHERE last_seen >= ?1",
            &[n(now - 86_400)],
        )
        .await?,
    );
    let active_week = count(
        db.one(
            "SELECT COUNT(*) AS c FROM players WHERE last_seen >= ?1",
            &[n(now - 7 * 86_400)],
        )
        .await?,
    );
    let saves = count(db.one("SELECT COUNT(*) AS c FROM saves", &[]).await?);
    let scores = count(db.one("SELECT COUNT(*) AS c FROM scores", &[]).await?);
    let sessions = count(
        db.one("SELECT COUNT(*) AS c FROM play_sessions", &[])
            .await?,
    );
    let seconds: Option<Seconds> = db
        .one(
            "SELECT COALESCE(SUM(seconds), 0) AS seconds FROM play_sessions",
            &[],
        )
        .await?;
    let by_game: Vec<GameSeconds> = db
        .all(
            "SELECT game, SUM(seconds) AS seconds, COUNT(*) AS sessions, COUNT(DISTINCT player_id) AS players FROM play_sessions GROUP BY game ORDER BY seconds DESC, game ASC",
            &[],
        )
        .await?;
    let score_rows: Vec<GameScores> = db
        .all(
            "SELECT game, COUNT(*) AS players, SUM(plays) AS plays FROM scores GROUP BY game",
            &[],
        )
        .await?;
    let today = now.div_euclid(86_400);
    let since = (today - 13) * 86_400;
    let by_day: Vec<DaySeconds> = db
        .all(
            "SELECT started_at / 86400 AS day, SUM(seconds) AS seconds FROM play_sessions WHERE started_at >= ?1 GROUP BY day ORDER BY day ASC",
            &[n(since)],
        )
        .await?;
    let games_out: Vec<Value> = games::GAMES
        .iter()
        .map(|(g, better)| {
            let time = by_game.iter().find(|x| x.game == *g);
            let sc = score_rows.iter().find(|x| x.game == *g);
            json!({
                "id": g,
                "better": better.as_str(),
                "seconds": time.map(|t| t.seconds).unwrap_or(0),
                "sessions": time.map(|t| t.sessions).unwrap_or(0),
                "players": time.map(|t| t.players).unwrap_or(0),
                "scored_players": sc.map(|x| x.players).unwrap_or(0),
                "plays": sc.map(|x| x.plays).unwrap_or(0),
            })
        })
        .collect();
    Ok(json!({
        "now": now,
        "players": players,
        "active_day": active_day,
        "active_week": active_week,
        "saves": saves,
        "scores": scores,
        "sessions": sessions,
        "seconds": seconds.map(|x| x.seconds).unwrap_or(0),
        "games": games_out,
        "by_day": (0..14)
            .map(|i| {
                let day = today - 13 + i;
                let secs = by_day.iter().find(|d| d.day == day).map(|d| d.seconds).unwrap_or(0);
                json!({ "day": rules::day_string(day), "seconds": secs })
            })
            .collect::<Vec<_>>(),
    }))
}

async fn list_players(db: &Db, p: &Params) -> ApiResult<Value> {
    let limit = clamp_limit(p.limit.as_deref(), DEFAULT_ADMIN_PAGE, MAX_ADMIN_PAGE);
    let offset = p
        .offset
        .as_deref()
        .and_then(|v| v.parse::<i64>().ok())
        .unwrap_or(0)
        .max(0);
    let q = p.q.as_deref().map(str::trim).filter(|q| !q.is_empty());
    let (filter, mut args): (&str, Vec<JsValue>) = match q {
        Some(q) => (
            "WHERE p.name LIKE ?1 ESCAPE '\\' OR p.id LIKE ?1 ESCAPE '\\'",
            vec![s(&like_pattern(q))],
        ),
        None => ("", vec![]),
    };
    let total = count(
        db.one(
            &format!("SELECT COUNT(*) AS c FROM players p {filter}"),
            &args,
        )
        .await?,
    );
    let (lim, off) = (args.len() + 1, args.len() + 2);
    args.push(n(limit));
    args.push(n(offset));
    let sql = format!(
        "SELECT p.id, p.name, p.avatar, p.created_at, p.last_seen, \
         COALESCE((SELECT SUM(seconds) FROM play_sessions x WHERE x.player_id = p.id), 0) AS seconds, \
         (SELECT COUNT(*) FROM scores x WHERE x.player_id = p.id) AS scores, \
         (SELECT COUNT(*) FROM saves x WHERE x.player_id = p.id) AS saves \
         FROM players p {filter} ORDER BY {} LIMIT ?{lim} OFFSET ?{off}",
        sort_clause(p.sort.as_deref())
    );
    let rows: Vec<PlayerListRow> = db.all(&sql, &args).await?;
    Ok(json!({
        "total": total,
        "limit": limit,
        "offset": offset,
        "players": rows.iter().map(|r| json!({
            "id": r.id, "name": r.name, "avatar": r.avatar,
            "created_at": r.created_at, "last_seen": r.last_seen,
            "seconds": r.seconds, "scores": r.scores, "saves": r.saves,
        })).collect::<Vec<_>>(),
    }))
}

async fn create_player(db: &Db, body: NewPlayer, now: i64) -> ApiResult<Value> {
    let name = valid_name(&body.name)?;
    let avatar = valid_avatar(body.avatar)?;
    let id = new_player_id()?;
    db.run(
        "INSERT INTO players (id, name, avatar, created_at, last_seen) VALUES (?1, ?2, ?3, ?4, ?4)",
        &[s(&id), s(&name), n(avatar), n(now)],
    )
    .await?;
    let link = handlers::issue_link_code(db, &id, now).await?;
    Ok(json!({
        "player": { "id": id, "name": name, "avatar": avatar, "created_at": now, "last_seen": now },
        "link": link,
    }))
}

async fn get_player(db: &Db, id: &str) -> ApiResult<Value> {
    let player = player_exists(db, id).await?;
    let saves: Vec<SaveMetaRow> = db
        .all(
            "SELECT game, rev, LENGTH(data) AS size, updated_at FROM saves WHERE player_id = ?1 ORDER BY game",
            &[s(id)],
        )
        .await?;
    let scores: Vec<PlayerScoreRow> = db
        .all(
            "SELECT game, score, achieved_at, plays FROM scores WHERE player_id = ?1 ORDER BY game",
            &[s(id)],
        )
        .await?;
    let time: Vec<GameSeconds> = db
        .all(
            "SELECT game, SUM(seconds) AS seconds, COUNT(*) AS sessions, 1 AS players FROM play_sessions WHERE player_id = ?1 GROUP BY game ORDER BY seconds DESC, game ASC",
            &[s(id)],
        )
        .await?;
    let devices: Vec<DeviceRow> = db
        .all(
            "SELECT label, created_at, last_used FROM tokens WHERE player_id = ?1 ORDER BY last_used DESC",
            &[s(id)],
        )
        .await?;
    Ok(json!({
        "player": { "id": player.id, "name": player.name, "avatar": player.avatar, "created_at": player.created_at, "last_seen": player.last_seen },
        "saves": saves.iter().map(|r| json!({ "slot": r.game, "rev": r.rev, "size": r.size, "updated_at": r.updated_at })).collect::<Vec<_>>(),
        "scores": scores.iter().map(|r| json!({ "game": r.game, "score": r.score, "achieved_at": r.achieved_at, "plays": r.plays })).collect::<Vec<_>>(),
        "playtime": time.iter().map(|r| json!({ "game": r.game, "seconds": r.seconds, "sessions": r.sessions })).collect::<Vec<_>>(),
        "devices": devices.iter().map(|r| json!({ "label": r.label, "created_at": r.created_at, "last_used": r.last_used })).collect::<Vec<_>>(),
    }))
}

async fn patch_player(db: &Db, id: &str, body: PlayerPatch) -> ApiResult<Value> {
    let name = body.name.as_deref().map(valid_name).transpose()?;
    let avatar = body.avatar.map(valid_avatar).transpose()?;
    if name.is_none() && avatar.is_none() {
        return Err(ApiError::bad_request("nothing to change"));
    }
    player_exists(db, id).await?;
    db.run(
        "UPDATE players SET name = COALESCE(?2, name), avatar = COALESCE(?3, avatar) WHERE id = ?1",
        &[
            s(id),
            name.as_deref().map(s).unwrap_or(JsValue::NULL),
            avatar.map(n).unwrap_or(JsValue::NULL),
        ],
    )
    .await?;
    let p = player_exists(db, id).await?;
    Ok(
        json!({ "player": { "id": p.id, "name": p.name, "avatar": p.avatar, "created_at": p.created_at, "last_seen": p.last_seen } }),
    )
}

async fn delete_player(db: &Db, id: &str) -> ApiResult<Value> {
    player_exists(db, id).await?;
    handlers::delete_player(db, id).await?;
    Ok(json!({ "deleted": true }))
}

async fn get_save(db: &Db, id: &str, slot: &str) -> ApiResult<Value> {
    check_slot(slot)?;
    let row: SaveRowOut = db
        .one(
            "SELECT rev, data, updated_at FROM saves WHERE player_id = ?1 AND game = ?2",
            &[s(id), s(slot)],
        )
        .await?
        .ok_or_else(|| ApiError::not_found("no save in that slot"))?;
    Ok(json!({ "slot": slot, "rev": row.rev, "data": row.data, "updated_at": row.updated_at }))
}

async fn put_save(db: &Db, id: &str, slot: &str, body: SaveIn, now: i64) -> ApiResult<Value> {
    check_slot(slot)?;
    valid_save_data(&body.data)?;
    player_exists(db, id).await?;
    db.run(
        "INSERT INTO saves (player_id, game, rev, data, updated_at) VALUES (?1, ?2, 1, ?3, ?4) ON CONFLICT (player_id, game) DO UPDATE SET data = excluded.data, rev = rev + 1, updated_at = excluded.updated_at",
        &[s(id), s(slot), s(&body.data), n(now)],
    )
    .await?;
    get_save(db, id, slot).await
}

async fn delete_save(db: &Db, id: &str, slot: &str) -> ApiResult<Value> {
    check_slot(slot)?;
    let removed = db
        .run(
            "DELETE FROM saves WHERE player_id = ?1 AND game = ?2",
            &[s(id), s(slot)],
        )
        .await?;
    if removed == 0 {
        return Err(ApiError::not_found("no save in that slot"));
    }
    Ok(json!({ "deleted": true }))
}

async fn board(db: &Db, game: &str, limit: Option<&str>) -> ApiResult<Value> {
    let better = check_game(game)?;
    let limit = clamp_limit(limit, DEFAULT_ADMIN_PAGE, MAX_ADMIN_PAGE);
    let sql = format!(
        "SELECT s.player_id, p.name, p.avatar, s.score, s.achieved_at, s.plays FROM scores s JOIN players p ON p.id = s.player_id WHERE s.game = ?1 ORDER BY {} LIMIT ?2",
        order_by(better)
    );
    let rows: Vec<AdminBoardRow> = db.all(&sql, &[s(game), n(limit)]).await?;
    let total = count(
        db.one(
            "SELECT COUNT(*) AS c FROM scores WHERE game = ?1",
            &[s(game)],
        )
        .await?,
    );
    Ok(json!({
        "game": game,
        "better": better.as_str(),
        "total": total,
        "entries": rows.iter().enumerate().map(|(i, r)| json!({
            "rank": i + 1, "player_id": r.player_id, "name": r.name, "avatar": r.avatar,
            "score": r.score, "achieved_at": r.achieved_at, "plays": r.plays,
        })).collect::<Vec<_>>(),
    }))
}

async fn put_score(db: &Db, game: &str, id: &str, body: ScoreIn, now: i64) -> ApiResult<Value> {
    let better = check_game(game)?;
    let score = valid_score(better, body.score)?;
    player_exists(db, id).await?;
    db.run(
        "INSERT INTO scores (game, player_id, score, achieved_at, plays) VALUES (?1, ?2, ?3, ?4, 1) ON CONFLICT (game, player_id) DO UPDATE SET score = excluded.score, achieved_at = excluded.achieved_at",
        &[s(game), s(id), n(score), n(now)],
    )
    .await?;
    Ok(json!({ "game": game, "player_id": id, "score": score }))
}

async fn delete_score(db: &Db, game: &str, id: &str) -> ApiResult<Value> {
    check_game(game)?;
    let removed = db
        .run(
            "DELETE FROM scores WHERE game = ?1 AND player_id = ?2",
            &[s(game), s(id)],
        )
        .await?;
    if removed == 0 {
        return Err(ApiError::not_found("no such score"));
    }
    Ok(json!({ "deleted": true }))
}

pub async fn handle(
    db: &Db,
    endpoint: Admin,
    params: Params,
    body: Option<String>,
    now: i64,
) -> ApiResult<Value> {
    let json_body = |b: Option<String>| -> ApiResult<String> {
        b.ok_or_else(|| ApiError::bad_request("a json body is required"))
    };
    match endpoint {
        Admin::Stats => stats(db, now).await,
        Admin::ListPlayers => list_players(db, &params).await,
        Admin::CreatePlayer => {
            create_player(db, serde_json::from_str(&json_body(body)?)?, now).await
        }
        Admin::GetPlayer(id) => get_player(db, &id).await,
        Admin::PatchPlayer(id) => {
            patch_player(db, &id, serde_json::from_str(&json_body(body)?)?).await
        }
        Admin::DeletePlayer(id) => delete_player(db, &id).await,
        Admin::CreateLink(id) => {
            player_exists(db, &id).await?;
            handlers::issue_link_code(db, &id, now).await
        }
        Admin::GetSave(id, slot) => get_save(db, &id, &slot).await,
        Admin::PutSave(id, slot) => {
            put_save(
                db,
                &id,
                &slot,
                serde_json::from_str(&json_body(body)?)?,
                now,
            )
            .await
        }
        Admin::DeleteSave(id, slot) => delete_save(db, &id, &slot).await,
        Admin::Board(game) => board(db, &game, params.limit.as_deref()).await,
        Admin::PutScore(game, id) => {
            put_score(
                db,
                &game,
                &id,
                serde_json::from_str(&json_body(body)?)?,
                now,
            )
            .await
        }
        Admin::DeleteScore(game, id) => delete_score(db, &game, &id).await,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_configured_key_is_accepted() {
        assert!(authorize(Some("s3cret"), Some("s3cret")).is_ok());
        assert_eq!(
            authorize(Some("s3cret"), Some("s3cre")).unwrap_err().status,
            401
        );
        assert_eq!(authorize(Some("s3cret"), Some("")).unwrap_err().status, 401);
        assert_eq!(authorize(Some("s3cret"), None).unwrap_err().status, 401);
    }

    #[test]
    fn admin_is_off_until_a_key_is_configured() {
        assert_eq!(authorize(None, Some("x")).unwrap_err().status, 404);
        assert_eq!(authorize(Some(""), Some("")).unwrap_err().status, 404);
    }

    #[test]
    fn search_text_cannot_inject_wildcards() {
        assert_eq!(like_pattern("mia"), "%mia%");
        assert_eq!(like_pattern("50%_\\"), "%50\\%\\_\\\\%");
    }

    #[test]
    fn unknown_sort_falls_back_to_recent() {
        assert!(sort_clause(Some("; drop")).starts_with("p.last_seen"));
        assert!(sort_clause(Some("playtime")).starts_with("seconds"));
        assert!(sort_clause(None).starts_with("p.last_seen"));
    }
}
