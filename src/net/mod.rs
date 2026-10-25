pub mod merge;
pub mod playtime;
pub mod transport;

use crate::save::{self, Map};
use serde_json::{json, Value};
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use transport::{HttpRequest, HttpResponse, Transport};

const CHECK_EVERY: f32 = 3.0;
const PUSH_DELAY: f32 = 5.0;
const OPEN_SESSION_EVERY: f32 = 60.0;
const BACKOFF_BASE: f32 = 15.0;
const BACKOFF_MAX: f32 = 600.0;
const OUTBOX_MAX: usize = 100;
const REPULL_EVERY: f32 = 300.0;
const BOARD_ROWS: usize = 5;
const CONFLICT_LIMIT: u32 = 5;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BoardRow {
    pub place: usize,
    pub name: String,
    pub avatar: usize,
    pub score: i64,
    pub mine: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Redeem {
    Idle,
    Working,
    Failed(&'static str),
    Done,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Code {
    Idle,
    Working,
    Ready { code: String, until: i64 },
    Failed,
}

struct Board {
    rows: Vec<BoardRow>,
    fresh: bool,
}

#[derive(Clone, Debug)]
enum Job {
    Register(String),
    PatchMe(String),
    Pull(String),
    Push {
        player: String,
        snapshot: Map,
    },
    Score {
        player: String,
        game: String,
        score: i64,
    },
    Playtime {
        player: String,
        sent: Vec<(String, i64)>,
    },
    Board(String),
    Delete(String),
    MakeCode(String),
    Redeem,
}

struct Flight {
    id: u32,
    job: Job,
}

pub struct Net {
    transport: Box<dyn Transport>,
    flight: Option<Flight>,
    clock: f32,
    retry_at: f32,
    fails: u32,
    check_in: f32,
    dirty_since: HashMap<String, f32>,
    pulled: HashMap<String, f32>,
    blocked: HashSet<String>,
    conflicts: u32,
    boards: HashMap<String, Board>,
    wanted_boards: Vec<String>,
    tracker: playtime::Tracker,
    open_sent_at: f32,
    push_now: bool,
    redeem_code: Option<String>,
    pub redeem: Redeem,
    make_code: bool,
    pub code: Code,
    now: i64,
}

fn token_key(player: &str) -> String {
    format!("net.{player}.token")
}

fn rev_key(player: &str) -> String {
    format!("net.{player}.rev")
}

fn base_key(player: &str) -> String {
    format!("net.{player}.base")
}

fn base(player: &str) -> Map {
    save::decode(&save::get_str(&base_key(player)).unwrap_or_default())
}

fn set_base(player: &str, map: &Map) {
    save::set_str(&base_key(player), &save::encode(map));
}

fn label_key(player: &str) -> String {
    format!("net.{player}.label")
}

fn token(player: &str) -> Option<String> {
    save::get_str(&token_key(player)).filter(|t| !t.is_empty())
}

fn label(player: &str) -> Option<String> {
    let p = crate::profile::load(player)?;
    Some(format!("{}|{}", p.name, p.avatar))
}

fn device() -> &'static str {
    if cfg!(target_arch = "wasm32") {
        "Web"
    } else if cfg!(target_os = "android") {
        "Android"
    } else if cfg!(target_os = "ios") {
        "iPhone or iPad"
    } else {
        "Computer"
    }
}

pub fn parse_outbox(text: &str) -> Vec<(String, String, i64)> {
    text.split(';')
        .filter_map(|e| {
            let mut f = e.split(',');
            let player = f.next().filter(|p| crate::profile::is_uuid(p))?;
            let game = f.next().filter(|g| !g.is_empty())?;
            Some((
                player.to_string(),
                game.to_string(),
                f.next()?.parse().ok()?,
            ))
        })
        .collect()
}

pub fn format_outbox(items: &[(String, String, i64)]) -> String {
    items
        .iter()
        .map(|(p, g, s)| format!("{p},{g},{s}"))
        .collect::<Vec<_>>()
        .join(";")
}

fn outbox() -> Vec<(String, String, i64)> {
    parse_outbox(&save::get_str("net.outbox").unwrap_or_default())
}

fn set_outbox(items: &[(String, String, i64)]) {
    save::set_str("net.outbox", &format_outbox(items));
}

pub fn queue_score(player: &str, game: &str, score: i64) {
    let mut items = outbox();
    items.retain(|(pl, g, s)| !(pl == player && g == game && *s == score));
    items.push((player.to_string(), game.to_string(), score));
    let drop = items.len().saturating_sub(OUTBOX_MAX);
    items.drain(..drop);
    set_outbox(&items);
}

fn deletions() -> Vec<String> {
    save::get_str("net.deletions")
        .unwrap_or_default()
        .split(',')
        .filter(|t| !t.is_empty())
        .map(String::from)
        .collect()
}

fn set_deletions(tokens: &[String]) {
    save::set_str("net.deletions", &tokens.join(","));
}

pub fn board_rows(v: &Value, avatars: usize) -> Vec<BoardRow> {
    let row = |e: &Value, mine: bool| -> Option<BoardRow> {
        Some(BoardRow {
            place: e["rank"].as_u64()? as usize,
            name: e["name"].as_str()?.to_string(),
            avatar: (e["avatar"].as_u64()? as usize).min(avatars.saturating_sub(1)),
            score: e["score"].as_i64()?,
            mine: mine || e["you"].as_bool().unwrap_or(false),
        })
    };
    let mut rows: Vec<BoardRow> = v["entries"]
        .as_array()
        .map(|a| a.iter().filter_map(|e| row(e, false)).collect())
        .unwrap_or_default();
    rows.truncate(BOARD_ROWS);
    if rows.iter().any(|r| r.mine) {
        return rows;
    }
    if let (Some(rank), Some(score)) = (v["me"]["rank"].as_u64(), v["me"]["score"].as_i64()) {
        if let Some(p) = crate::profile::current() {
            rows.pop();
            rows.push(BoardRow {
                place: rank as usize,
                name: p.name,
                avatar: p.avatar,
                score,
                mine: true,
            });
        }
    }
    rows
}

fn parse(resp: &HttpResponse) -> Value {
    serde_json::from_str(&resp.body).unwrap_or(Value::Null)
}

impl Net {
    pub fn new(transport: Box<dyn Transport>) -> Self {
        Self {
            transport,
            flight: None,
            clock: 0.0,
            retry_at: 0.0,
            fails: 0,
            check_in: 0.0,
            dirty_since: HashMap::new(),
            pulled: HashMap::new(),
            blocked: HashSet::new(),
            conflicts: 0,
            boards: HashMap::new(),
            wanted_boards: vec![],
            tracker: playtime::Tracker::load(&save::get_str("net.playtime").unwrap_or_default()),
            open_sent_at: 0.0,
            push_now: false,
            redeem_code: None,
            redeem: Redeem::Idle,
            make_code: false,
            code: Code::Idle,
            now: 0,
        }
    }

    fn player(&self) -> Option<String> {
        crate::profile::current_id()
    }

    pub fn begin_play(&mut self, game: &str, now: i64) {
        if let Some(p) = self.player() {
            let id = crate::profile::uuid_v4(crate::profile::entropy());
            self.tracker.begin(&p, game, id, now);
            self.persist_playtime();
        }
    }

    pub fn end_play(&mut self) {
        self.tracker.end();
        self.push_now = true;
        self.open_sent_at = f32::MIN;
        self.persist_playtime();
    }

    fn persist_playtime(&mut self) {
        if self.tracker.take_dirty() {
            save::set_str("net.playtime", &self.tracker.dump());
        }
    }

    pub fn request_board(&mut self, game: &str) {
        if let Some(b) = self.boards.get_mut(game) {
            b.fresh = false;
        }
        if !self.wanted_boards.iter().any(|g| g == game) {
            self.wanted_boards.push(game.to_string());
        }
    }

    pub fn board(&self, game: &str) -> Option<&[BoardRow]> {
        self.boards
            .get(game)
            .filter(|b| b.fresh)
            .map(|b| b.rows.as_slice())
    }

    pub fn forget_profile(&mut self, id: &str) {
        if let Some(t) = token(id) {
            let mut d = deletions();
            d.push(t);
            set_deletions(&d);
        }
        save::remove_prefix(&format!("net.{id}."));
        let rest: Vec<_> = outbox().into_iter().filter(|(p, _, _)| p != id).collect();
        set_outbox(&rest);
        self.tracker.forget(id);
        self.persist_playtime();
        self.dirty_since.remove(id);
        self.pulled.remove(id);
        self.boards.clear();
    }

    pub fn is_online(&self) -> bool {
        self.player()
            .map(|p| token(&p).is_some() && self.pulled.contains_key(&p))
            .unwrap_or(false)
    }

    pub fn request_code(&mut self) {
        self.make_code = true;
        self.code = Code::Working;
    }

    pub fn redeem_code(&mut self, code: &str) {
        self.redeem_code = Some(code.to_string());
        self.redeem = Redeem::Working;
    }

    pub fn update(&mut self, dt: f32, now: i64, active: bool) {
        self.clock += dt;
        self.now = now;
        self.tracker.tick(dt, active);
        self.persist_playtime();
        if let Some(f) = &self.flight {
            if let Some(resp) = self.transport.poll(f.id) {
                let job = self.flight.take().map(|f| f.job);
                if let Some(job) = job {
                    self.finish(job, resp);
                }
            }
            return;
        }
        self.watch_local_changes(dt);
        if let Some(job) = self.next_job() {
            self.launch(job);
        }
    }

    fn watch_local_changes(&mut self, dt: f32) {
        self.check_in -= dt;
        if self.check_in > 0.0 {
            return;
        }
        self.check_in = CHECK_EVERY;
        let Some(p) = self.player() else { return };
        if !self.pulled.contains_key(&p) {
            return;
        }
        if merge::collect(&p) != base(&p) {
            self.dirty_since.entry(p).or_insert(self.clock);
        } else {
            self.dirty_since.remove(&p);
        }
    }

    fn backed_off(&self) -> bool {
        self.clock < self.retry_at
    }

    fn next_job(&mut self) -> Option<Job> {
        if self.redeem_code.is_some() {
            return Some(Job::Redeem);
        }
        let player = self.player();
        if self.make_code {
            if let Some(p) = player.clone().filter(|p| token(p).is_some()) {
                self.make_code = false;
                return Some(Job::MakeCode(p));
            }
        }
        if self.backed_off() {
            return None;
        }
        if let Some(t) = deletions().into_iter().next() {
            return Some(Job::Delete(t));
        }
        let p = player?;
        if self.blocked.contains(&p) {
            return None;
        }
        if token(&p).is_none() {
            return Some(Job::Register(p));
        }
        if label(&p) != save::get_str(&label_key(&p)) {
            return Some(Job::PatchMe(p));
        }
        let pulled_at = self.pulled.get(&p).copied();
        let stale = pulled_at.is_some_and(|at| self.clock - at >= REPULL_EVERY);
        if pulled_at.is_none() || (stale && !self.dirty_since.contains_key(&p)) {
            return Some(Job::Pull(p));
        }
        let due = self
            .dirty_since
            .get(&p)
            .map(|since| self.push_now || self.clock - since >= PUSH_DELAY)
            .unwrap_or(false);
        if due {
            let snapshot = merge::collect(&p);
            return Some(Job::Push {
                player: p,
                snapshot,
            });
        }
        if let Some((pl, game, score)) = outbox().into_iter().find(|(pl, _, _)| pl == &p) {
            return Some(Job::Score {
                player: pl,
                game,
                score,
            });
        }
        let include_open = self.clock - self.open_sent_at >= OPEN_SESSION_EVERY;
        let pending = self.tracker.pending(&p, include_open);
        if !pending.is_empty() {
            return Some(Job::Playtime {
                player: p,
                sent: pending
                    .into_iter()
                    .map(|(id, _, _, secs)| (id, secs))
                    .collect(),
            });
        }
        if let Some(game) = self.wanted_boards.first().cloned() {
            return Some(Job::Board(game));
        }
        None
    }

    fn launch(&mut self, job: Job) {
        let req = match &job {
            Job::Register(p) => {
                let prof = crate::profile::load(p);
                let (name, avatar) = prof.map(|x| (x.name, x.avatar)).unwrap_or_default();
                post(
                    "/v1/players",
                    None,
                    json!({ "id": p, "name": name, "avatar": avatar, "device": device() }),
                )
            }
            Job::PatchMe(p) => {
                let prof = crate::profile::load(p);
                let (name, avatar) = prof.map(|x| (x.name, x.avatar)).unwrap_or_default();
                request(
                    "PATCH",
                    "/v1/me".into(),
                    token(p),
                    Some(json!({ "name": name, "avatar": avatar })),
                )
            }
            Job::Pull(p) => request("GET", "/v1/saves/profile".into(), token(p), None),
            Job::Push { player, snapshot } => {
                let data = save::encode(snapshot);
                let rev = save::get(&rev_key(player));
                request(
                    "PUT",
                    "/v1/saves/profile".into(),
                    token(player),
                    Some(json!({ "data": data, "base_rev": rev })),
                )
            }
            Job::Score {
                player,
                game,
                score,
            } => post(
                &format!("/v1/scores/{game}"),
                token(player),
                json!({ "score": score }),
            ),
            Job::Playtime { player, sent } => {
                let sessions: Vec<Value> = self
                    .tracker
                    .pending(player, true)
                    .into_iter()
                    .filter(|(id, _, _, _)| sent.iter().any(|(s, _)| s == id))
                    .map(|(id, game, started, secs)| {
                        json!({ "id": id, "game": game, "started_at": started, "seconds": secs })
                    })
                    .collect();
                post(
                    "/v1/playtime",
                    token(player),
                    json!({ "sessions": sessions }),
                )
            }
            Job::Board(game) => request(
                "GET",
                format!("/v1/scores/{game}?limit=10"),
                self.player().and_then(|p| token(&p)),
                None,
            ),
            Job::Delete(t) => request("DELETE", "/v1/me".into(), Some(t.clone()), None),
            Job::MakeCode(p) => request("POST", "/v1/link".into(), token(p), None),
            Job::Redeem => post(
                "/v1/link/redeem",
                None,
                json!({ "code": self.redeem_code.clone().unwrap_or_default(), "device": device() }),
            ),
        };
        if matches!(job, Job::Push { .. }) {
            self.push_now = false;
        }
        if matches!(job, Job::Redeem) {
            self.redeem_code = None;
        }
        let id = self.transport.start(req);
        self.flight = Some(Flight { id, job });
    }

    fn back_off(&mut self) {
        self.fails += 1;
        let wait = (BACKOFF_BASE * 2f32.powi(self.fails.min(6) as i32 - 1)).min(BACKOFF_MAX);
        self.retry_at = self.clock + wait;
    }

    fn succeeded(&mut self) {
        self.fails = 0;
        self.retry_at = 0.0;
    }

    fn retryable(status: i32) -> bool {
        status == 0 || status == 429 || status >= 500
    }

    fn lost_account(&mut self, player: &str) {
        save::set_str(&token_key(player), "");
        save::set(&rev_key(player), 0);
        set_base(player, &Map::new());
        self.pulled.remove(player);
        self.dirty_since.remove(player);
    }

    fn move_to_new_id(&mut self, old: &str) {
        save::remove_prefix(&format!("net.{old}."));
        let Some(new) = crate::profile::reassign(old) else {
            self.blocked.insert(old.to_string());
            return;
        };
        let moved: Vec<_> = outbox()
            .into_iter()
            .map(|(p, g, sc)| (if p == old { new.clone() } else { p }, g, sc))
            .collect();
        set_outbox(&moved);
        self.tracker.reassign(old, &new);
        self.persist_playtime();
        self.pulled.remove(old);
        self.dirty_since.remove(old);
    }

    fn finish(&mut self, job: Job, resp: HttpResponse) {
        let body = parse(&resp);
        let status = resp.status;
        match job {
            Job::Register(p) => match status {
                200 => {
                    if let Some(t) = body["token"].as_str() {
                        save::set_str(&token_key(&p), t);
                        save::set_str(&label_key(&p), &label(&p).unwrap_or_default());
                    }
                    self.succeeded();
                }
                409 if body["error"] == "id_taken" => self.move_to_new_id(&p),
                s if Self::retryable(s) => self.back_off(),
                _ => {
                    self.blocked.insert(p);
                }
            },
            Job::PatchMe(p) => match status {
                200 => {
                    save::set_str(&label_key(&p), &label(&p).unwrap_or_default());
                    self.succeeded();
                }
                401 => self.lost_account(&p),
                s if Self::retryable(s) => self.back_off(),
                _ => save::set_str(&label_key(&p), &label(&p).unwrap_or_default()),
            },
            Job::Pull(p) => match status {
                200 => {
                    let remote = save::decode(body["data"].as_str().unwrap_or_default());
                    let local = merge::collect(&p);
                    let merged = merge::merge(&local, &remote, &base(&p));
                    merge::apply(&p, &merged);
                    save::set(&rev_key(&p), body["rev"].as_i64().unwrap_or(0));
                    set_base(&p, &remote);
                    self.dirty_since.remove(&p);
                    self.pulled.insert(p, self.clock);
                    self.succeeded();
                }
                404 => {
                    save::set(&rev_key(&p), 0);
                    set_base(&p, &Map::new());
                    self.dirty_since.remove(&p);
                    self.pulled.insert(p, self.clock);
                    self.succeeded();
                }
                401 => self.lost_account(&p),
                _ => self.back_off(),
            },
            Job::Push { player, snapshot } => match status {
                200 => {
                    save::set(&rev_key(&player), body["rev"].as_i64().unwrap_or(0));
                    set_base(&player, &snapshot);
                    self.dirty_since.remove(&player);
                    self.conflicts = 0;
                    self.succeeded();
                }
                409 => {
                    self.conflicts += 1;
                    if self.conflicts > CONFLICT_LIMIT {
                        self.conflicts = 0;
                        self.back_off();
                    }
                    self.pulled.remove(&player);
                }
                401 => self.lost_account(&player),
                413 => {
                    set_base(&player, &snapshot);
                    self.dirty_since.remove(&player);
                }
                _ => self.back_off(),
            },
            Job::Score {
                player,
                game,
                score,
            } => match status {
                200 => {
                    let mut items = outbox();
                    if let Some(i) = items
                        .iter()
                        .position(|(p, g, s)| p == &player && g == &game && *s == score)
                    {
                        items.remove(i);
                    }
                    set_outbox(&items);
                    self.succeeded();
                    if let Some(b) = self.boards.get_mut(&game) {
                        b.fresh = false;
                    }
                    self.request_board(&game);
                }
                401 => self.lost_account(&player),
                s if Self::retryable(s) => self.back_off(),
                _ => {
                    let mut items = outbox();
                    items.retain(|(p, g, s)| !(p == &player && g == &game && *s == score));
                    set_outbox(&items);
                }
            },
            Job::Playtime { player, sent } => match status {
                200 => {
                    for (id, secs) in sent {
                        self.tracker.mark_sent(&id, secs);
                    }
                    self.open_sent_at = self.clock;
                    self.persist_playtime();
                    self.succeeded();
                }
                401 => self.lost_account(&player),
                s if Self::retryable(s) => self.back_off(),
                _ => {
                    for (id, secs) in sent {
                        self.tracker.mark_sent(&id, secs);
                    }
                    self.persist_playtime();
                }
            },
            Job::Board(game) => {
                match status {
                    200 => {
                        let rows = board_rows(&body, crate::profile::AVATARS.len());
                        self.boards
                            .insert(game.clone(), Board { rows, fresh: true });
                        self.succeeded();
                    }
                    s if Self::retryable(s) => {
                        self.back_off();
                        return;
                    }
                    _ => {}
                }
                self.wanted_boards.retain(|g| g != &game);
            }
            Job::Delete(t) => {
                if status == 200 || status == 401 {
                    let rest: Vec<String> = deletions().into_iter().filter(|x| x != &t).collect();
                    set_deletions(&rest);
                    self.succeeded();
                } else if Self::retryable(status) {
                    self.back_off();
                }
            }
            Job::MakeCode(_) => {
                if status == 200 {
                    let secs = body["expires_in"].as_i64().unwrap_or(600);
                    self.code = Code::Ready {
                        code: body["code"].as_str().unwrap_or_default().to_string(),
                        until: self.now + secs,
                    };
                } else {
                    self.code = Code::Failed;
                }
            }
            Job::Redeem => match status {
                200 => {
                    let player = &body["player"];
                    let adopted = crate::profile::adopt(
                        player["id"].as_str().unwrap_or_default(),
                        player["name"].as_str().unwrap_or_default(),
                        player["avatar"].as_u64().unwrap_or(0) as usize,
                    );
                    match (adopted, body["token"].as_str()) {
                        (Some(prof), Some(t)) => {
                            save::set_str(&token_key(&prof.id), t);
                            save::set_str(
                                &label_key(&prof.id),
                                &format!("{}|{}", prof.name, prof.avatar),
                            );
                            self.pulled.remove(&prof.id);
                            self.boards.clear();
                            self.redeem = Redeem::Done;
                        }
                        _ => self.redeem = Redeem::Failed("There are too many players here"),
                    }
                }
                404 => self.redeem = Redeem::Failed("That code did not work"),
                429 => self.redeem = Redeem::Failed("Wait a minute and try again"),
                0 => self.redeem = Redeem::Failed("No internet"),
                _ => self.redeem = Redeem::Failed("Something went wrong"),
            },
        }
    }
}

fn request(
    method: &'static str,
    path: String,
    token: Option<String>,
    body: Option<Value>,
) -> HttpRequest {
    HttpRequest {
        method,
        path,
        token,
        body: body.map(|b| b.to_string()),
    }
}

fn post(path: &str, token: Option<String>, body: Value) -> HttpRequest {
    request("POST", path.to_string(), token, Some(body))
}

thread_local! {
    static NET: RefCell<Option<Net>> = const { RefCell::new(None) };
}

fn with<R>(f: impl FnOnce(&mut Net) -> R) -> Option<R> {
    if cfg!(test) {
        return None;
    }
    NET.with(|n| {
        let mut n = n.borrow_mut();
        let net = n.get_or_insert_with(|| Net::new(transport::default_transport()));
        Some(f(net))
    })
}

fn unix_now() -> i64 {
    macroquad::miniquad::date::now() as i64
}

pub fn update(dt: f32, active: bool) {
    with(|n| n.update(dt, unix_now(), active));
}

pub fn begin_play(game: &str) {
    with(|n| n.begin_play(game, unix_now()));
}

pub fn end_play() {
    with(|n| n.end_play());
}

pub fn on_score(game: &str, score: i64) {
    if let Some(p) = crate::profile::current_id() {
        queue_score(&p, game, score);
    }
}

pub fn forget_profile(id: &str) {
    with(|n| n.forget_profile(id));
}

pub fn online_board(game: &str) -> Option<Vec<BoardRow>> {
    with(|n| n.board(game).map(|b| b.to_vec())).flatten()
}

pub fn is_online() -> bool {
    with(|n| n.is_online()).unwrap_or(false)
}

pub fn request_code() {
    with(|n| n.request_code());
}

pub fn code() -> Code {
    with(|n| n.code.clone()).unwrap_or(Code::Idle)
}

pub fn clear_code() {
    with(|n| n.code = Code::Idle);
}

pub fn redeem(code: &str) {
    with(|n| n.redeem_code(code));
}

pub fn redeem_state() -> Redeem {
    with(|n| n.redeem.clone()).unwrap_or(Redeem::Idle)
}

pub fn clear_redeem() {
    with(|n| n.redeem = Redeem::Idle);
}

pub fn active(input: &crate::engine::Input) -> bool {
    input.down
        || input.pressed
        || input.released
        || input.swipe.is_some()
        || input.key_dir.is_some()
        || input.left
        || input.right
        || input.action
}

pub fn request_board(game: &str) {
    with(|n| n.request_board(game));
}

#[cfg(test)]
mod tests;
