pub const MIN_SECONDS: i64 = 3;
pub const IDLE_AFTER: f32 = 90.0;
pub const MAX_SESSION: f64 = 4.0 * 3600.0;
pub const MAX_PER_UPLOAD: usize = 50;
const DUMP_EVERY: f32 = 20.0;
const KEEP: usize = 200;
pub const HOLD_FOR: f32 = 600.0;

#[derive(Clone, Debug, PartialEq)]
pub struct Session {
    pub id: String,
    pub player: String,
    pub game: String,
    pub started: i64,
    pub seconds: f64,
    pub sent: i64,
    pub open: bool,
}

impl Session {
    pub fn whole(&self) -> i64 {
        self.seconds as i64
    }

    fn worth_sending(&self) -> bool {
        self.whole() >= MIN_SECONDS && self.whole() > self.sent
    }
}

#[derive(Default)]
pub struct Tracker {
    sessions: Vec<Session>,
    held: Vec<(String, f32)>,
    single_until: f32,
    idle: f32,
    since_dump: f32,
    dirty: bool,
}

impl Tracker {
    pub fn load(text: &str) -> Self {
        let sessions = text
            .split(';')
            .filter_map(|s| {
                let mut f = s.split(',');
                Some(Session {
                    id: f.next().filter(|i| !i.is_empty())?.to_string(),
                    player: f.next().filter(|i| !i.is_empty())?.to_string(),
                    game: f.next()?.to_string(),
                    started: f.next()?.parse().ok()?,
                    seconds: f.next()?.parse().ok()?,
                    sent: f.next()?.parse().ok()?,
                    open: false,
                })
            })
            .collect();
        let mut t = Self {
            sessions,
            ..Default::default()
        };
        t.prune();
        t
    }

    pub fn dump(&self) -> String {
        self.sessions
            .iter()
            .map(|s| {
                format!(
                    "{},{},{},{},{:.1},{}",
                    s.id, s.player, s.game, s.started, s.seconds, s.sent
                )
            })
            .collect::<Vec<_>>()
            .join(";")
    }

    pub fn reassign(&mut self, old: &str, new: &str) {
        for s in self.sessions.iter_mut().filter(|s| s.player == old) {
            s.player = new.to_string();
            self.dirty = true;
        }
    }

    pub fn take_dirty(&mut self) -> bool {
        std::mem::take(&mut self.dirty)
    }

    pub fn begin(&mut self, player: &str, game: &str, id: String, now: i64) {
        self.end();
        self.sessions.push(Session {
            id,
            player: player.to_string(),
            game: game.to_string(),
            started: now,
            seconds: 0.0,
            sent: 0,
            open: true,
        });
        self.idle = 0.0;
        self.dirty = true;
    }

    pub fn end(&mut self) {
        let mut any = false;
        for s in self.sessions.iter_mut().filter(|s| s.open) {
            s.open = false;
            any = true;
        }
        if any {
            self.dirty = true;
            self.prune();
        }
    }

    pub fn is_open(&self) -> bool {
        self.sessions.iter().any(|s| s.open)
    }

    pub fn tick(&mut self, dt: f32, active: bool) {
        if !self.is_open() {
            return;
        }
        if active {
            self.idle = 0.0;
        } else {
            self.idle += dt;
        }
        if self.idle <= IDLE_AFTER {
            for s in self.sessions.iter_mut().filter(|s| s.open) {
                s.seconds = (s.seconds + dt as f64).min(MAX_SESSION);
            }
        }
        self.since_dump += dt;
        if self.since_dump >= DUMP_EVERY {
            self.since_dump = 0.0;
            self.dirty = true;
        }
    }

    pub fn reject<'a>(&mut self, ids: impl Iterator<Item = &'a str>, clock: f32) {
        let ids: Vec<&str> = ids.collect();
        self.single_until = clock + HOLD_FOR;
        if let [one] = ids[..] {
            self.held.retain(|(h, _)| h != one);
            self.held.push((one.to_string(), clock + HOLD_FOR));
        }
    }

    pub fn pending(
        &self,
        player: &str,
        include_open: bool,
        clock: f32,
    ) -> Vec<(String, String, i64, i64)> {
        let held = |id: &str| self.held.iter().any(|(h, until)| h == id && clock < *until);
        let single = clock < self.single_until;
        let picked = self
            .sessions
            .iter()
            .filter(|s| s.player == player && (include_open || !s.open) && s.worth_sending())
            .filter(|s| !held(&s.id));
        let size = if single { 1 } else { MAX_PER_UPLOAD };
        picked
            .take(size)
            .map(|s| (s.id.clone(), s.game.clone(), s.started, s.whole()))
            .collect()
    }

    pub fn lookup<'a>(
        &self,
        player: &str,
        ids: impl Iterator<Item = &'a str>,
    ) -> Vec<(String, String, i64, i64)> {
        ids.filter_map(|id| {
            self.sessions
                .iter()
                .find(|s| s.id == id && s.player == player)
                .map(|s| (s.id.clone(), s.game.clone(), s.started, s.whole()))
        })
        .collect()
    }

    pub fn mark_sent(&mut self, id: &str, seconds: i64) {
        if let Some(s) = self.sessions.iter_mut().find(|s| s.id == id) {
            s.sent = s.sent.max(seconds);
        }
        self.dirty = true;
        self.prune();
    }

    fn prune(&mut self) {
        self.sessions.retain(|s| s.open || s.worth_sending());
        if self.sessions.len() > KEEP {
            let drop = self.sessions.len() - KEEP;
            self.sessions.drain(..drop);
        }
    }

    pub fn forget(&mut self, player: &str) {
        self.sessions.retain(|s| s.player != player);
        self.dirty = true;
    }

    pub fn total_unsent(&self) -> i64 {
        self.sessions.iter().map(|s| s.whole() - s.sent).sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const P: &str = "11111111-2222-4333-8444-555555555555";

    fn run(t: &mut Tracker, secs: u32, active: bool) {
        for _ in 0..secs {
            t.tick(1.0, active);
        }
    }

    #[test]
    fn counts_only_while_a_game_is_open() {
        let mut t = Tracker::default();
        run(&mut t, 30, true);
        assert!(t.pending(P, true, 0.0).is_empty());
        t.begin(P, "snake", "abcdef01".into(), 1000);
        run(&mut t, 30, true);
        t.end();
        run(&mut t, 30, true);
        assert_eq!(
            t.pending(P, true, 0.0),
            vec![("abcdef01".into(), "snake".into(), 1000, 30)]
        );
    }

    #[test]
    fn a_walk_away_stops_the_clock_after_ninety_seconds() {
        let mut t = Tracker::default();
        t.begin(P, "snake", "abcdef01".into(), 0);
        run(&mut t, 20, true);
        run(&mut t, 600, false);
        t.end();
        assert_eq!(t.pending(P, true, 0.0)[0].3, 20 + IDLE_AFTER as i64);
    }

    #[test]
    fn touching_again_after_a_pause_resumes_counting() {
        let mut t = Tracker::default();
        t.begin(P, "snake", "abcdef01".into(), 0);
        run(&mut t, 200, false);
        let before = t.pending(P, true, 0.0)[0].3;
        run(&mut t, 10, true);
        assert_eq!(t.pending(P, true, 0.0)[0].3, before + 10);
    }

    #[test]
    fn blips_under_three_seconds_are_dropped() {
        let mut t = Tracker::default();
        t.begin(P, "snake", "abcdef01".into(), 0);
        run(&mut t, 2, true);
        t.end();
        assert!(t.pending(P, true, 0.0).is_empty());
        assert_eq!(t.dump(), "");
    }

    #[test]
    fn starting_a_new_game_closes_the_old_session() {
        let mut t = Tracker::default();
        t.begin(P, "snake", "aaaaaaaa".into(), 0);
        run(&mut t, 10, true);
        t.begin(P, "hotel", "bbbbbbbb".into(), 10);
        run(&mut t, 5, true);
        let p = t.pending(P, true, 0.0);
        assert_eq!(p.len(), 2);
        assert_eq!((p[0].1.as_str(), p[0].3), ("snake", 10));
        assert_eq!((p[1].1.as_str(), p[1].3), ("hotel", 5));
    }

    #[test]
    fn an_open_session_is_resent_only_with_new_seconds() {
        let mut t = Tracker::default();
        t.begin(P, "snake", "aaaaaaaa".into(), 0);
        run(&mut t, 10, true);
        t.mark_sent("aaaaaaaa", 10);
        assert!(t.pending(P, true, 0.0).is_empty());
        run(&mut t, 5, true);
        assert_eq!(t.pending(P, true, 0.0)[0].3, 15);
        t.end();
        t.mark_sent("aaaaaaaa", 15);
        assert!(t.pending(P, true, 0.0).is_empty());
        assert_eq!(t.dump(), "");
    }

    #[test]
    fn unsent_sessions_survive_a_restart_as_closed() {
        let mut t = Tracker::default();
        t.begin(P, "snake", "aaaaaaaa".into(), 500);
        run(&mut t, 40, true);
        let saved = t.dump();
        let back = Tracker::load(&saved);
        assert!(!back.is_open());
        assert_eq!(back.pending(P, true, 0.0), t.pending(P, true, 0.0));
        assert_eq!(back.total_unsent(), 40);
    }

    #[test]
    fn garbage_in_the_save_is_ignored() {
        let t = Tracker::load(
            "junk;;a,b;,x,snake,1,2,3;ok123456,11111111-2222-4333-8444-555555555555,snake,5,30.0,0",
        );
        assert_eq!(t.pending(P, true, 0.0).len(), 1);
    }

    #[test]
    fn sessions_belong_to_the_player_who_played_them() {
        let mut t = Tracker::default();
        t.begin(P, "snake", "aaaaaaaa".into(), 0);
        run(&mut t, 10, true);
        t.end();
        assert!(t.pending("someone-else", true, 0.0).is_empty());
        t.forget(P);
        assert!(t.pending(P, true, 0.0).is_empty());
    }

    #[test]
    fn open_sessions_can_be_held_back_from_an_upload() {
        let mut t = Tracker::default();
        t.begin(P, "snake", "aaaaaaaa".into(), 0);
        run(&mut t, 10, true);
        assert!(t.pending(P, false, 0.0).is_empty());
        assert_eq!(t.pending(P, true, 0.0).len(), 1);
        t.end();
        assert_eq!(t.pending(P, false, 0.0).len(), 1);
    }

    #[test]
    fn one_session_never_claims_more_than_four_hours() {
        let mut t = Tracker::default();
        t.begin(P, "snake", "aaaaaaaa".into(), 0);
        for _ in 0..200 {
            t.tick(100.0, true);
        }
        assert_eq!(t.pending(P, true, 0.0)[0].3, MAX_SESSION as i64);
    }

    #[test]
    fn uploads_are_batched_to_the_server_limit() {
        let mut t = Tracker::default();
        for i in 0..60 {
            t.begin(P, "snake", format!("id{i:06}"), i);
            run(&mut t, 5, true);
        }
        t.end();
        assert_eq!(t.pending(P, true, 0.0).len(), MAX_PER_UPLOAD);
    }
}
