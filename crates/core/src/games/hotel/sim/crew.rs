use super::*;

pub const CREW_FEATURES: usize = 18;
pub const THINK_FASTEST: f32 = 1.0;

pub type Chooser<'a> = &'a mut dyn FnMut(Effort, &[Vec<f32>]) -> Option<usize>;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Pick {
    Room(usize),
    Stall(usize),
    Guest(u32),
}

pub fn shipped() -> Option<Arc<Net>> {
    static NET: OnceLock<Option<Arc<Net>>> = OnceLock::new();
    NET.get_or_init(|| {
        Net::from_bytes(include_bytes!("../../../../../../assets/brains/hotel.kgb")).map(Arc::new)
    })
    .clone()
}

pub fn hire_effort(role: Build) -> Effort {
    Effort::new(match role {
        Build::Maid => 0.7,
        Build::Janitor => 0.6,
        Build::Waiter => 0.8,
        Build::Lifeguard => 0.9,
        _ => 1.0,
    })
}

fn role_slot(role: Build) -> usize {
    match role {
        Build::Maid => 0,
        Build::Janitor => 1,
        Build::Waiter => 2,
        _ => 3,
    }
}

struct Job1 {
    pick: Pick,
    at: Vec2,
    waited: f32,
    left: f32,
    money: f32,
    size: f32,
    vip: bool,
}

impl Sim {
    pub fn effort_of(&self, s: usize) -> Effort {
        self.effort
            .unwrap_or_else(|| hire_effort(self.staff[s].role))
    }

    fn path_len(&self, from: Vec2, to: Vec2) -> f32 {
        let path = self.route(from, to);
        let mut at = from;
        let mut total = 0.0;
        for p in path {
            total += (p - at).length();
            at = p;
        }
        total
    }

    fn front_wait(&self) -> (f32, f32) {
        match self.waiting_line().first() {
            Some(&g) => {
                let g = &self.guests[g];
                (1.0 - g.patience / g.max, (g.patience / 20.0).clamp(0.0, 1.0))
            }
            None => (0.0, 1.0),
        }
    }

    fn open_jobs(&self, s: usize) -> Vec<Job1> {
        let role = self.staff[s].role;
        let mut v = vec![];
        match role {
            Build::Maid => {
                let (waited, left) = self.front_wait();
                for k in 0..self.rooms.len() {
                    if !self.rooms[k].is_dirty() || self.claimed_room(k) {
                        continue;
                    }
                    let spots = self.spots(k);
                    let Some(i) = (0..spots.len()).find(|&i| self.rooms[k].dirty[i]) else {
                        continue;
                    };
                    let dirty = self.rooms[k].dirty.iter().filter(|d| **d).count();
                    v.push(Job1 {
                        pick: Pick::Room(k),
                        at: spots[i].stand,
                        waited,
                        left,
                        money: (self.rooms[k].tip + self.room_price(k, false)) as f32,
                        size: dirty as f32 / 8.0,
                        vip: false,
                    });
                }
            }
            Build::Janitor => {
                for k in 0..self.stalls.len() {
                    let paper = self.stalls[k].paper;
                    let taken = self
                        .staff
                        .iter()
                        .enumerate()
                        .any(|(o, st)| o != s && st.role == role && st.job != Job::Idle && st.pick == Some(k));
                    if paper >= PAPER_MAX || taken {
                        continue;
                    }
                    let wait = self
                        .guests
                        .iter()
                        .filter(|g| matches!(g.stage, Stage::WaitStall))
                        .map(|g| g.patience / g.max)
                        .fold(f32::MAX, f32::min);
                    let waiting = wait < f32::MAX;
                    v.push(Job1 {
                        pick: Pick::Stall(k),
                        at: self.lay.stall_front[k],
                        waited: if waiting { 1.0 - wait } else { 0.0 },
                        left: if waiting { wait } else { 1.0 },
                        money: self.extra_price(Want::Toilet) as f32,
                        size: (PAPER_MAX - paper) as f32 / PAPER_MAX as f32,
                        vip: false,
                    });
                }
            }
            _ => {
                let snack = role == Build::Waiter;
                for g in &self.guests {
                    let want = match g.stage {
                        Stage::WaitSnack(_) if snack => Want::Snack,
                        Stage::WaitTowel(_) if !snack => Want::Pool,
                        _ => continue,
                    };
                    if self.claimed_guest(g.id) {
                        continue;
                    }
                    v.push(Job1 {
                        pick: Pick::Guest(g.id),
                        at: g.pos,
                        waited: 1.0 - g.patience / g.max,
                        left: (g.patience / 20.0).clamp(0.0, 1.0),
                        money: self.extra_price(want) as f32,
                        size: 0.125,
                        vip: g.vip,
                    });
                }
            }
        }
        v
    }

    pub fn job_open_for(&self, s: usize) -> bool {
        self.staff[s].role != Build::Clerk && !self.open_jobs(s).is_empty()
    }

    pub(super) fn crew_pick(&mut self, s: usize, choose: Chooser) -> Option<Pick> {
        if self.staff[s].think > 0.0 {
            return None;
        }
        let jobs = self.open_jobs(s);
        if jobs.is_empty() {
            return None;
        }
        let role = self.staff[s].role;
        let from = self.staff[s].pos;
        let open = (jobs.len() as f32 / 6.0).min(1.0);
        let queue = self.waiting_line().len() as f32 / MAX_QUEUE as f32;
        let spare = (0..self.rooms.len())
            .filter(|&k| self.room_open(k) && !self.rooms[k].is_dirty() && self.rooms[k].guest.is_none())
            .count() as f32
            / 4.0;
        let blocked = (self.front_ready().is_some() && self.free_room().is_none()) as u8 as f32;
        let mut slot = [0.0f32; 4];
        slot[role_slot(role)] = 1.0;
        let base = |idle: f32| {
            let mut f = vec![0.0; CREW_FEATURES];
            f[..4].copy_from_slice(&slot);
            f[4] = idle;
            f[11] = open;
            f
        };
        let mut options: Vec<Vec<f32>> = jobs
            .iter()
            .enumerate()
            .map(|(order, j)| {
                let mut f = base(0.0);
                f[5] = (self.path_len(from, j.at) / 20.0).min(1.5);
                f[6] = j.waited.clamp(0.0, 1.0);
                f[7] = j.left;
                f[8] = (j.money / 20.0).min(1.5);
                f[9] = ((self.player.pos - j.at).length() < 3.0) as u8 as f32;
                f[10] = j.size;
                f[12] = j.vip as u8 as f32;
                f[13] = 1.0 - order as f32 / 8.0;
                f[14] = blocked;
                f[15] = queue;
                f[16] = spare.min(1.0);
                f[17] = (self.path_len(self.lay.desk_front, j.at) / 30.0).min(1.5);
                f
            })
            .collect();
        let may_rest = !self.staff[s].rested;
        if may_rest {
            options.push(base(1.0));
        }
        let effort = self.effort_of(s);
        let choice = match choose(effort, &options) {
            Some(k) => {
                self.staff[s].think = effort.think_every(THINK_FASTEST);
                self.staff[s].rested = k >= jobs.len();
                k
            }
            None => {
                let first = jobs.iter().position(|j| match j.pick {
                    Pick::Stall(k) => self.stalls[k].paper == 0,
                    _ => true,
                });
                first.unwrap_or(jobs.len())
            }
        };
        jobs.get(choice).map(|j| j.pick)
    }
}
