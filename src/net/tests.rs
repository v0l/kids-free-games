use super::transport::{HttpRequest, HttpResponse, Transport};
use super::*;
use std::cell::RefCell;
use std::rc::Rc;

#[derive(Default)]
struct ServerState {
    players: HashMap<String, (String, i64)>,
    tokens: HashMap<String, String>,
    saves: HashMap<String, (i64, String)>,
    scores: HashMap<(String, String), i64>,
    playtime: HashMap<(String, String), i64>,
    codes: HashMap<String, String>,
    log: Vec<String>,
    down: bool,
    status_override: Option<i32>,
    next_token: u32,
}

#[derive(Clone, Default)]
struct FakeServer(Rc<RefCell<ServerState>>);

impl FakeServer {
    fn reply(&self, status: i32, body: Value) -> HttpResponse {
        HttpResponse {
            status,
            body: body.to_string(),
        }
    }

    fn handle(&self, req: &HttpRequest) -> HttpResponse {
        let mut s = self.0.borrow_mut();
        s.log.push(format!("{} {}", req.method, req.path));
        if s.down {
            return HttpResponse::network_error();
        }
        if let Some(code) = s.status_override {
            return HttpResponse {
                status: code,
                body: String::new(),
            };
        }
        drop(s);
        let body: Value = req
            .body
            .as_deref()
            .and_then(|b| serde_json::from_str(b).ok())
            .unwrap_or(Value::Null);
        let player = req
            .token
            .as_ref()
            .and_then(|t| self.0.borrow().tokens.get(t).cloned());
        let mut s = self.0.borrow_mut();
        match (req.method, req.path.as_str()) {
            ("POST", "/v1/players") => {
                let id = body["id"].as_str().unwrap().to_string();
                if s.players.contains_key(&id) {
                    return self.reply(409, json!({"error": "id_taken"}));
                }
                s.players.insert(
                    id.clone(),
                    (
                        body["name"].as_str().unwrap().to_string(),
                        body["avatar"].as_i64().unwrap(),
                    ),
                );
                let t = s.mint(&id);
                self.reply(200, json!({"token": t, "player": {"id": id}}))
            }
            ("POST", "/v1/link/redeem") => {
                let code = body["code"].as_str().unwrap().replace('-', "");
                let Some(id) = s.codes.remove(&code) else {
                    return self.reply(404, json!({"error": "not_found"}));
                };
                let t = s.mint(&id);
                let (name, avatar) = s.players[&id].clone();
                self.reply(
                    200,
                    json!({"token": t, "player": {"id": id, "name": name, "avatar": avatar}}),
                )
            }
            _ => {
                let Some(player) = player else {
                    return self.reply(401, json!({"error": "unauthorized"}));
                };
                self.authed(&mut s, req, &player, &body)
            }
        }
    }

    fn authed(
        &self,
        s: &mut ServerState,
        req: &HttpRequest,
        player: &str,
        body: &Value,
    ) -> HttpResponse {
        match (req.method, req.path.as_str()) {
            ("PATCH", "/v1/me") => {
                s.players.insert(
                    player.to_string(),
                    (
                        body["name"].as_str().unwrap().to_string(),
                        body["avatar"].as_i64().unwrap(),
                    ),
                );
                self.reply(200, json!({}))
            }
            ("GET", "/v1/saves/profile") => match s.saves.get(player) {
                Some((rev, data)) => self.reply(200, json!({"rev": rev, "data": data})),
                None => self.reply(404, json!({"error": "not_found"})),
            },
            ("PUT", "/v1/saves/profile") => {
                let base = body["base_rev"].as_i64().unwrap();
                let cur = s.saves.get(player).map(|x| x.0).unwrap_or(0);
                if base != cur {
                    return self.reply(409, json!({"error": "rev_mismatch", "rev": cur}));
                }
                s.saves.insert(
                    player.to_string(),
                    (cur + 1, body["data"].as_str().unwrap().to_string()),
                );
                self.reply(200, json!({"rev": cur + 1}))
            }
            ("POST", "/v1/link") => {
                let code = format!("CODE{:04}", s.codes.len());
                s.codes.insert(code.clone(), player.to_string());
                self.reply(
                    200,
                    json!({"code": format!("{}-{}", &code[..4], &code[4..]), "expires_in": 600}),
                )
            }
            ("POST", "/v1/playtime") => {
                let mut applied = 0;
                for x in body["sessions"].as_array().unwrap() {
                    let key = (player.to_string(), x["id"].as_str().unwrap().to_string());
                    let secs = x["seconds"].as_i64().unwrap();
                    let e = s.playtime.entry(key).or_insert(0);
                    if secs > *e {
                        *e = secs;
                        applied += 1;
                    }
                }
                self.reply(200, json!({"applied": applied}))
            }
            ("DELETE", "/v1/me") => {
                s.players.remove(player);
                s.saves.remove(player);
                s.tokens.retain(|_, p| p != player);
                self.reply(200, json!({"deleted": true}))
            }
            (m, p) if m == "POST" && p.starts_with("/v1/scores/") => {
                let game = p.rsplit('/').next().unwrap().to_string();
                let score = body["score"].as_i64().unwrap();
                let e = s.scores.entry((game, player.to_string())).or_insert(0);
                *e = (*e).max(score);
                self.reply(200, json!({"best": *e, "rank": 1}))
            }
            (m, p) if m == "GET" && p.starts_with("/v1/scores/") => {
                let game = p
                    .trim_start_matches("/v1/scores/")
                    .split('?')
                    .next()
                    .unwrap();
                let mut rows: Vec<_> = s
                    .scores
                    .iter()
                    .filter(|((g, _), _)| g == game)
                    .map(|((_, pl), sc)| (pl.clone(), *sc))
                    .collect();
                rows.sort_by_key(|r| std::cmp::Reverse(r.1));
                let entries: Vec<Value> = rows
                    .iter()
                    .enumerate()
                    .map(|(i, (pl, sc))| {
                        let (name, avatar) = s.players[pl].clone();
                        json!({"rank": i + 1, "name": name, "avatar": avatar, "score": sc, "you": pl == player})
                    })
                    .collect();
                self.reply(200, json!({"entries": entries, "me": null}))
            }
            _ => self.reply(404, json!({"error": "not_found"})),
        }
    }
}

impl ServerState {
    fn mint(&mut self, player: &str) -> String {
        self.next_token += 1;
        let t = format!("kgt_{:064}", self.next_token);
        self.tokens.insert(t.clone(), player.to_string());
        t
    }
}

struct Wire {
    server: FakeServer,
    done: HashMap<u32, HttpResponse>,
    next: u32,
}

impl Transport for Wire {
    fn start(&mut self, req: HttpRequest) -> u32 {
        self.next += 1;
        let r = self.server.handle(&req);
        self.done.insert(self.next, r);
        self.next
    }

    fn poll(&mut self, id: u32) -> Option<HttpResponse> {
        self.done.remove(&id)
    }
}

struct Device {
    net: Net,
    store: Map,
    now: i64,
}

impl Device {
    fn new(server: &FakeServer) -> Self {
        let net = Net::new(Box::new(Wire {
            server: server.clone(),
            done: HashMap::new(),
            next: 0,
        }));
        Self {
            net,
            store: Map::new(),
            now: 1_760_000_000,
        }
    }

    fn run<R>(&mut self, f: impl FnOnce(&mut Net) -> R) -> R {
        let prev = save::swap_for_test(std::mem::take(&mut self.store));
        let out = f(&mut self.net);
        self.store = save::swap_for_test(prev);
        out
    }

    fn step(&mut self, seconds: u32, active: bool) {
        for _ in 0..seconds * 2 {
            self.now += 1;
            let now = self.now;
            self.run(|n| n.update(0.5, now, active));
        }
    }

    fn play<R>(&mut self, f: impl FnOnce() -> R) -> R {
        let prev = save::swap_for_test(std::mem::take(&mut self.store));
        let out = f();
        self.store = save::swap_for_test(prev);
        out
    }

    fn get(&mut self, key: &str) -> i64 {
        self.play(|| save::player_get(key))
    }
}

fn mia(d: &mut Device) -> String {
    d.play(|| crate::profile::create("Mia", 2).unwrap().id)
}

#[test]
fn a_new_player_registers_with_their_own_id_and_pulls_an_empty_save() {
    let server = FakeServer::default();
    let mut a = Device::new(&server);
    let id = mia(&mut a);
    a.step(10, true);
    assert!(server.0.borrow().players.contains_key(&id));
    assert_eq!(server.0.borrow().players[&id], ("Mia".to_string(), 2));
    assert!(a.run(|n| n.is_online()));
}

#[test]
fn progress_is_pushed_after_a_short_pause_and_not_before() {
    let server = FakeServer::default();
    let mut a = Device::new(&server);
    let id = mia(&mut a);
    a.step(10, true);
    a.play(|| crate::profile::unlock("snake", 4));
    a.step(2, true);
    assert!(!server.0.borrow().saves.contains_key(&id));
    a.step(10, true);
    let saved = server.0.borrow().saves[&id].1.clone();
    assert!(saved.contains("snake.unlocked=4"), "{saved}");
}

#[test]
fn a_second_device_gets_the_first_devices_progress_by_link_code() {
    let server = FakeServer::default();
    let mut a = Device::new(&server);
    let id = mia(&mut a);
    a.play(|| {
        crate::profile::unlock("snake", 5);
        crate::profile::award_stars("snake", 2, 3);
    });
    a.step(15, true);
    a.run(|n| n.request_code());
    a.step(5, true);
    let code = match a.run(|n| n.code.clone()) {
        Code::Ready { code, .. } => code,
        other => panic!("{other:?}"),
    };

    let mut b = Device::new(&server);
    b.run(|n| n.redeem_code(&code));
    b.step(10, true);
    assert_eq!(b.run(|n| n.redeem.clone()), Redeem::Done);
    assert_eq!(b.play(crate::profile::current_id), Some(id));
    assert_eq!(b.play(|| crate::profile::current().unwrap().name), "Mia");
    assert_eq!(b.get("snake.unlocked"), 5);
    assert_eq!(b.get("snake.stars.2"), 3);
}

#[test]
fn a_wrong_code_reports_an_error_and_makes_no_player() {
    let server = FakeServer::default();
    let mut b = Device::new(&server);
    b.run(|n| n.redeem_code("ZZZZ-ZZZZ"));
    b.step(3, true);
    assert_eq!(
        b.run(|n| n.redeem.clone()),
        Redeem::Failed("That code did not work")
    );
    assert!(b.play(|| crate::profile::list().is_empty()));
}

#[test]
fn two_devices_playing_apart_merge_without_losing_progress() {
    let server = FakeServer::default();
    let mut a = Device::new(&server);
    mia(&mut a);
    a.step(10, true);
    a.run(|n| n.request_code());
    a.step(3, true);
    let Code::Ready { code, .. } = a.run(|n| n.code.clone()) else {
        panic!()
    };
    let mut b = Device::new(&server);
    b.run(|n| n.redeem_code(&code));
    b.step(10, true);

    a.play(|| {
        crate::profile::unlock("snake", 3);
        crate::profile::award_stars("hotel", 1, 2);
    });
    b.play(|| {
        crate::profile::unlock("snake", 6);
        crate::profile::award_stars("hotel", 2, 3);
    });
    a.step(20, true);
    b.step(20, true);
    a.step(320, true);
    b.step(320, true);
    assert_eq!(a.get("snake.unlocked"), 6);
    assert_eq!(b.get("snake.unlocked"), 6);
    assert_eq!(a.get("hotel.stars.1"), 2);
    assert_eq!(a.get("hotel.stars.2"), 3);
    assert_eq!(b.get("hotel.stars.1"), 2);
    assert_eq!(b.get("hotel.stars.2"), 3);
}

#[test]
fn scores_are_queued_offline_and_sent_when_the_network_returns() {
    let server = FakeServer::default();
    let mut a = Device::new(&server);
    let id = mia(&mut a);
    a.step(10, true);
    server.0.borrow_mut().down = true;
    a.play(|| crate::profile::submit("snake", 42, crate::profile::Better::Higher));
    a.step(60, true);
    assert!(server.0.borrow().scores.is_empty());
    assert_eq!(a.play(|| outbox().len()), 1);
    server.0.borrow_mut().down = false;
    a.step(120, true);
    assert_eq!(server.0.borrow().scores[&("snake".to_string(), id)], 42);
    assert!(a.play(|| outbox().is_empty()));
}

#[test]
fn a_score_is_never_sent_for_the_wrong_player_after_switching() {
    let server = FakeServer::default();
    let mut a = Device::new(&server);
    let mia_id = mia(&mut a);
    a.step(10, true);
    server.0.borrow_mut().down = true;
    a.play(|| crate::profile::submit("snake", 42, crate::profile::Better::Higher));
    let leo = a.play(|| crate::profile::create("Leo", 1).unwrap().id);
    server.0.borrow_mut().down = false;
    a.step(120, true);
    let s = server.0.borrow();
    assert_eq!(s.scores.get(&("snake".to_string(), mia_id)), None);
    assert_eq!(s.scores.get(&("snake".to_string(), leo)), None);
    drop(s);
    assert_eq!(a.play(|| outbox().len()), 1);
}

#[test]
fn the_results_board_is_fetched_after_a_score_lands() {
    let server = FakeServer::default();
    let mut a = Device::new(&server);
    mia(&mut a);
    a.step(10, true);
    a.play(|| crate::profile::submit("snake", 42, crate::profile::Better::Higher));
    a.run(|n| n.request_board("snake"));
    a.step(30, true);
    let rows = a.run(|n| n.board("snake").map(|b| b.to_vec())).unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].name, "Mia");
    assert_eq!(rows[0].score, 42);
    assert!(rows[0].mine);
}

#[test]
fn play_time_is_uploaded_when_a_game_closes_and_never_twice() {
    let server = FakeServer::default();
    let mut a = Device::new(&server);
    let id = mia(&mut a);
    a.step(10, true);
    let now = a.now;
    a.play(|| ());
    a.run(|n| n.begin_play("snake", now));
    a.step(45, true);
    a.run(|n| n.end_play());
    a.step(10, true);
    let before = server
        .0
        .borrow()
        .log
        .iter()
        .filter(|l| l.contains("playtime"))
        .count();
    a.step(120, true);
    let after = server
        .0
        .borrow()
        .log
        .iter()
        .filter(|l| l.contains("playtime"))
        .count();
    assert_eq!(before, after);
    let total: i64 = server
        .0
        .borrow()
        .playtime
        .iter()
        .filter(|((p, _), _)| p == &id)
        .map(|(_, s)| *s)
        .sum();
    assert!((44..=46).contains(&total), "{total}");
}

#[test]
fn a_long_session_reports_progress_while_still_open() {
    let server = FakeServer::default();
    let mut a = Device::new(&server);
    mia(&mut a);
    a.step(10, true);
    let now = a.now;
    a.run(|n| n.begin_play("snake", now));
    a.step(150, true);
    let total: i64 = server.0.borrow().playtime.values().sum();
    assert!(total >= 60, "{total}");
}

#[test]
fn server_errors_back_off_instead_of_hammering() {
    let server = FakeServer::default();
    let mut a = Device::new(&server);
    mia(&mut a);
    server.0.borrow_mut().status_override = Some(503);
    a.step(300, true);
    let calls = server.0.borrow().log.len();
    assert!(calls <= 8, "{calls} calls in 5 minutes");
    server.0.borrow_mut().status_override = None;
    a.step(900, true);
    assert!(a.run(|n| n.is_online()));
}

#[test]
fn deleting_a_player_deletes_their_server_account() {
    let server = FakeServer::default();
    let mut a = Device::new(&server);
    let id = mia(&mut a);
    a.step(10, true);
    assert!(server.0.borrow().players.contains_key(&id));
    a.play(|| crate::profile::delete(&id));
    a.run(|n| n.forget_profile(&id));
    a.step(10, true);
    assert!(!server.0.borrow().players.contains_key(&id));
}

#[test]
fn a_guest_never_touches_the_network() {
    let server = FakeServer::default();
    let mut a = Device::new(&server);
    a.step(60, true);
    assert!(server.0.borrow().log.is_empty());
}

#[test]
fn renaming_a_player_updates_the_server() {
    let server = FakeServer::default();
    let mut a = Device::new(&server);
    let id = mia(&mut a);
    a.step(10, true);
    a.play(|| crate::profile::rename(&id, "Mimi", 5));
    a.step(10, true);
    assert_eq!(server.0.borrow().players[&id], ("Mimi".to_string(), 5));
}

#[test]
fn a_revoked_token_is_dropped_and_the_player_re_registers_or_blocks() {
    let server = FakeServer::default();
    let mut a = Device::new(&server);
    let id = mia(&mut a);
    a.step(10, true);
    server.0.borrow_mut().tokens.clear();
    a.play(|| crate::profile::unlock("snake", 2));
    a.step(60, true);
    assert!(!a.run(|n| n.is_online()));
    assert!(server.0.borrow().players.contains_key(&id));
}

#[test]
fn board_rows_keep_my_row_when_i_am_below_the_cut() {
    crate::save::reset_for_test();
    crate::profile::create("Mia", 2).unwrap();
    let v = json!({
        "entries": [
            {"rank":1,"name":"A","avatar":0,"score":9,"you":false},
            {"rank":2,"name":"B","avatar":1,"score":8,"you":false},
            {"rank":3,"name":"C","avatar":1,"score":7,"you":false},
            {"rank":4,"name":"D","avatar":1,"score":6,"you":false},
            {"rank":5,"name":"E","avatar":1,"score":5,"you":false},
            {"rank":6,"name":"F","avatar":1,"score":4,"you":false}
        ],
        "me": {"rank": 14, "score": 2}
    });
    let rows = board_rows(&v, 12);
    assert_eq!(rows.len(), 5);
    assert_eq!(rows[4].place, 14);
    assert_eq!(rows[4].name, "Mia");
    assert!(rows[4].mine);
    assert_eq!(rows[0].place, 1);
}

#[test]
fn board_rows_ignore_garbage() {
    assert!(board_rows(&Value::Null, 12).is_empty());
    assert!(board_rows(&json!({"entries": [{"rank": "x"}]}), 12).is_empty());
}

#[test]
fn the_score_outbox_round_trips_and_ignores_junk() {
    let items = vec![
        (
            "11111111-2222-4333-8444-555555555555".to_string(),
            "snake".to_string(),
            42,
        ),
        (
            "11111111-2222-4333-8444-555555555555".to_string(),
            "memory".to_string(),
            12,
        ),
    ];
    assert_eq!(parse_outbox(&format_outbox(&items)), items);
    assert!(parse_outbox("junk;a,b,c;,,1").is_empty());
}
