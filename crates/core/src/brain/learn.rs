use super::conv::Obs;

pub struct Spec {
    pub channels: usize,
    pub side: usize,
    pub scalars: usize,
    pub actions: usize,
}

pub struct Tick {
    pub reward: f32,
    pub done: bool,
}

pub trait Learner: Send + Sync {
    fn spec() -> Spec
    where
        Self: Sized;
    fn agents(&self) -> usize;
    fn obs(&self, agent: usize) -> &Obs;
    fn step(&mut self, actions: &[usize], out: &mut [Tick]);
    fn drain_episodes(&mut self) -> Vec<(f32, f32)>;
    fn seats(&self) -> &[Seat] {
        &[]
    }
    fn seat_actions(&mut self, _: &[usize]) {}
}

pub struct Seat {
    pub policy: usize,
    pub obs: Obs,
}

pub struct Batch<E: Learner> {
    pub envs: Vec<E>,
    threads: usize,
}

impl<E: Learner> Batch<E> {
    pub fn new(envs: Vec<E>, threads: usize) -> Self {
        Self {
            envs,
            threads: threads.max(1),
        }
    }

    pub fn agents(&self) -> usize {
        self.envs.iter().map(|e| e.agents()).sum()
    }

    pub fn write_obs(&self, grid: &mut [f32], scalars: &mut [f32]) {
        let s = E::spec();
        let g = s.channels * s.side * s.side;
        let chunk = self.envs.len().div_ceil(self.threads).max(1);
        let per = self.envs.first().map_or(0, |e| e.agents());
        std::thread::scope(|sc| {
            for ((envs, gs), ss) in self
                .envs
                .chunks(chunk)
                .zip(grid.chunks_mut(chunk * per * g))
                .zip(scalars.chunks_mut(chunk * per * s.scalars))
            {
                sc.spawn(move || {
                    let mut k = 0;
                    for e in envs {
                        for a in 0..e.agents() {
                            let o = e.obs(a);
                            gs[k * g..(k + 1) * g].copy_from_slice(&o.grid);
                            ss[k * s.scalars..(k + 1) * s.scalars].copy_from_slice(&o.scalars);
                            k += 1;
                        }
                    }
                });
            }
        });
    }

    pub fn step(&mut self, actions: &[usize], out: &mut [Tick]) {
        let chunk = self.envs.len().div_ceil(self.threads).max(1);
        let per = self.envs.first().map_or(0, |e| e.agents());
        std::thread::scope(|sc| {
            for ((envs, acts), ticks) in self
                .envs
                .chunks_mut(chunk)
                .zip(actions.chunks(chunk * per))
                .zip(out.chunks_mut(chunk * per))
            {
                sc.spawn(move || {
                    let mut k = 0;
                    for e in envs {
                        let n = e.agents();
                        e.step(&acts[k..k + n], &mut ticks[k..k + n]);
                        k += n;
                    }
                });
            }
        });
    }

    pub fn seated(&self, policies: usize) -> Vec<Vec<(usize, usize)>> {
        let mut by = vec![vec![]; policies];
        for (e, env) in self.envs.iter().enumerate() {
            for (k, seat) in env.seats().iter().enumerate() {
                if let Some(list) = by.get_mut(seat.policy) {
                    list.push((e, k));
                }
            }
        }
        by
    }

    pub fn seat_obs(&self, list: &[(usize, usize)], grid: &mut Vec<f32>, scalars: &mut Vec<f32>) {
        grid.clear();
        scalars.clear();
        for &(e, k) in list {
            let o = &self.envs[e].seats()[k].obs;
            grid.extend_from_slice(&o.grid);
            scalars.extend_from_slice(&o.scalars);
        }
    }

    pub fn give_seat_actions(&mut self, picks: &[Vec<usize>]) {
        for (env, a) in self.envs.iter_mut().zip(picks) {
            if !a.is_empty() {
                env.seat_actions(a);
            }
        }
    }

    pub fn drain_episodes(&mut self) -> Vec<(f32, f32)> {
        self.envs
            .iter_mut()
            .flat_map(|e| e.drain_episodes())
            .collect()
    }
}
