mod net;

use candle_core::{Device, Result, Tensor, D};
use candle_nn::{AdamW, Optimizer, ParamsAdamW, VarMap};
use kg_core::brain::conv::{Obs, Policy};
use kg_core::brain::learn::{Batch, Learner, Tick};
use kg_core::brain::Effort;
use std::sync::mpsc::{Receiver, Sender};
use std::time::Instant;

#[derive(Clone)]
struct Args {
    game: String,
    pop: usize,
    envs: usize,
    steps: usize,
    gen_updates: usize,
    generations: usize,
    lr: f64,
    ent: f32,
    explore: f32,
    out: String,
    resume: String,
    threads: usize,
    secs: f32,
    patience: usize,
    min_gain: f32,
    eval_seeds: u64,
    league: usize,
    warmup: usize,
    minibatch: usize,
    bf16: bool,
    league_servers: usize,
}

fn args() -> Args {
    let mut a = Args {
        game: "clash".into(),
        pop: 6,
        envs: 128,
        steps: 32,
        gen_updates: 40,
        generations: 60,
        lr: 2.5e-4,
        ent: 0.02,
        explore: 0.15,
        out: "/tmp/kgrl/clash.kgb".into(),
        resume: String::new(),
        threads: 24,
        secs: 600.0,
        patience: 8,
        min_gain: 0.25,
        eval_seeds: 16,
        league: 6,
        warmup: 20,
        minibatch: 4096,
        bf16: false,
        league_servers: 4,
    };
    let v: Vec<String> = std::env::args().collect();
    let mut i = 1;
    while i + 1 < v.len() {
        let x = v[i + 1].clone();
        match v[i].as_str() {
            "--game" => a.game = x,
            "--pop" => a.pop = x.parse().unwrap(),
            "--envs" => a.envs = x.parse().unwrap(),
            "--steps" => a.steps = x.parse().unwrap(),
            "--gen-updates" => a.gen_updates = x.parse().unwrap(),
            "--generations" => a.generations = x.parse().unwrap(),
            "--lr" => a.lr = x.parse().unwrap(),
            "--ent" => a.ent = x.parse().unwrap(),
            "--explore" => a.explore = x.parse().unwrap(),
            "--out" => a.out = x,
            "--resume" => a.resume = x,
            "--threads" => a.threads = x.parse().unwrap(),
            "--secs" => a.secs = x.parse().unwrap(),
            "--patience" => a.patience = x.parse().unwrap(),
            "--min-gain" => a.min_gain = x.parse().unwrap(),
            "--eval-seeds" => a.eval_seeds = x.parse().unwrap(),
            "--league" => a.league = x.parse().unwrap(),
            "--warmup" => a.warmup = x.parse().unwrap(),
            "--minibatch" => a.minibatch = x.parse().unwrap(),
            "--bf16" => a.bf16 = x == "1" || x == "true",
            "--league-servers" => a.league_servers = x.parse().unwrap(),
            other => panic!("unknown flag {other}"),
        }
        i += 2;
    }
    a
}

struct Rng(u64);

impl Rng {
    fn uniform(&mut self) -> f32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 40) as f32 / (1u64 << 24) as f32
    }

    fn below(&mut self, n: usize) -> usize {
        (self.uniform() * n as f32) as usize % n.max(1)
    }

    fn jiggle(&mut self, lo: f32, hi: f32) -> f32 {
        lo * (hi / lo).powf(self.uniform())
    }
}

#[derive(Clone, Copy, Default)]
struct Commit {
    action: usize,
    left: u32,
}

impl Commit {
    fn pick(&mut self, sampled: usize, chance: f32, actions: usize, rng: &mut Rng) -> usize {
        if self.left == 0 && rng.uniform() < chance {
            self.action = rng.below(actions);
            self.left = 5 + (rng.uniform() * 11.0) as u32;
        }
        if self.left == 0 {
            return sampled;
        }
        self.left -= 1;
        self.action
    }
}

fn save(policy: &Policy, path: &str) {
    if let Some(d) = std::path::Path::new(path).parent() {
        let _ = std::fs::create_dir_all(d);
    }
    let tmp = format!("{path}.tmp");
    std::fs::write(&tmp, policy.to_bytes()).unwrap();
    std::fs::rename(&tmp, path).unwrap();
}

fn parity(net: &net::Net, policy: &Policy, sample: &Obs, dev: &Device) -> Result<f32> {
    let s = &net.spec;
    let g = Tensor::from_slice(&sample.grid, (1, s.channels * s.side * s.side), dev)?;
    let sc = Tensor::from_slice(&sample.scalars, (1, s.scalars), dev)?;
    let (logits, _) = net.forward(&g, &sc)?;
    let want: Vec<f32> = logits.squeeze(0)?.to_vec1()?;
    let got = policy.logits(sample);
    let top = want.iter().fold(0.0f32, |m, v| m.max(v.abs()));
    Ok(want
        .iter()
        .zip(&got)
        .map(|(a, b)| (a - b).abs())
        .fold(0.0, f32::max)
        / (1.0 + top))
}

trait Game: Learner + Sized {
    fn genes() -> Vec<(&'static str, f32)>;
    fn make(seed: u64, secs: f32, genes: &[f32]) -> Self;
    fn set_genes(&mut self, genes: &[f32]);
    fn baseline(seeds: u64) -> (f32, String);
    fn eval(policy: &Policy, rivals: &[Policy], seeds: u64) -> (f32, String);
    fn set_seats(_: usize) {}
}

mod clash {
    use super::*;
    use kg_core::games::clash::rl::{
        evaluate, evaluate_against, Arena, Pilot, Report, Rewards, GAME_CAP,
    };

    fn rewards(g: &[f32]) -> Rewards {
        Rewards {
            xp: g[0],
            level: g[1],
            kill: g[2],
            cut: g[3],
            death: g[4],
            stall: g[5],
            wall: g[6],
            bump: g[7],
            win: g[8],
        }
    }

    fn score(r: &Report) -> f32 {
        r.level + 2.0 * r.kills + 20.0 * r.wins + 5.0 * r.secs / GAME_CAP
            - 5.0 * r.circling
            - 3.0 * r.stall
    }

    fn describe(r: &Report) -> String {
        format!(
            "won {:.0}% level {:.1} lived {:.0}s died {:.0}% kills {:.2} stalled {:.0}% circling {:.1}%",
            r.wins * 100.0,
            r.level,
            r.secs,
            r.deaths * 100.0,
            r.kills,
            r.stall * 100.0,
            r.circling * 100.0
        )
    }

    impl Game for Arena {
        fn genes() -> Vec<(&'static str, f32)> {
            let r = Rewards::default();
            vec![
                ("xp", r.xp),
                ("level", r.level),
                ("kill", r.kill),
                ("cut", r.cut),
                ("death", r.death),
                ("stall", r.stall),
                ("wall", r.wall),
                ("bump", r.bump),
                ("win", r.win),
            ]
        }

        fn make(seed: u64, secs: f32, genes: &[f32]) -> Self {
            Arena::with(seed, secs, rewards(genes))
        }

        fn set_genes(&mut self, genes: &[f32]) {
            self.rewards = rewards(genes);
        }

        fn baseline(seeds: u64) -> (f32, String) {
            let r = evaluate(&Pilot::Bot, seeds, GAME_CAP);
            (score(&r), describe(&r))
        }

        fn eval(policy: &Policy, rivals: &[Policy], seeds: u64) -> (f32, String) {
            let r = evaluate_against(
                &Pilot::Policy(policy, Effort::BEST),
                seeds,
                GAME_CAP,
                rivals,
                0.5,
            );
            (score(&r), describe(&r))
        }

        fn set_seats(n: usize) {
            kg_core::games::clash::rl::set_seats(n, 0.25);
        }
    }
}

mod paper {
    use super::*;
    use kg_core::games::paper::rl::{evaluate_against, Arena, Pilot, Report, Rewards};

    fn rewards(g: &[f32]) -> Rewards {
        Rewards {
            land: g[0],
            kill: g[1],
            death: g[2],
            idle: g[3],
            linger: g[4],
            win: g[5],
            outlive: g[6],
        }
    }

    fn score(r: &Report) -> f32 {
        (20.0 * r.wins + 10.0 * r.outlived) * (1.0 - r.circling) + 200.0 * r.peak + r.kills
            - 30.0 * r.circling
    }

    fn describe(r: &Report) -> String {
        format!(
            "won {:.0}% outlived {:.0}% peak {:.1}% lived {:.0}s kills {:.2} inside {:.0}% circling {:.1}%",
            r.wins * 100.0,
            r.outlived * 100.0,
            r.peak * 100.0,
            r.secs,
            r.kills,
            r.inside * 100.0,
            r.circling * 100.0
        )
    }

    impl Game for Arena {
        fn genes() -> Vec<(&'static str, f32)> {
            let r = Rewards::default();
            vec![
                ("land", r.land),
                ("kill", r.kill),
                ("death", r.death),
                ("idle", r.idle),
                ("linger", r.linger),
                ("win", r.win),
                ("outlive", r.outlive),
            ]
        }

        fn make(seed: u64, secs: f32, genes: &[f32]) -> Self {
            Arena::with(seed, secs, rewards(genes))
        }

        fn set_genes(&mut self, genes: &[f32]) {
            self.rewards = rewards(genes);
        }

        fn baseline(seeds: u64) -> (f32, String) {
            let r = evaluate_against(&Pilot::Bot, seeds, None, &[], 0.0);
            (score(&r), describe(&r))
        }

        fn eval(policy: &Policy, rivals: &[Policy], seeds: u64) -> (f32, String) {
            let r = evaluate_against(
                &Pilot::Policy(policy, Effort::BEST),
                seeds,
                None,
                rivals,
                0.5,
            );
            (score(&r), describe(&r))
        }

        fn set_seats(n: usize) {
            kg_core::games::paper::rl::set_seats(n, 0.3);
        }
    }
}

struct Member<G: Game> {
    id: usize,
    dev: Device,
    rng: Rng,
    vm: VarMap,
    net: net::Net,
    opt: AdamW,
    batch: Batch<G>,
    genes: Vec<f32>,
    lr: f64,
    ent: f32,
    grid: Vec<f32>,
    scal: Vec<f32>,
    ticks: Vec<Tick>,
    commit: Vec<Commit>,
    score: f32,
    vs_bots: f32,
    vs_league: f32,
    text: String,
    episodes: (f32, f32, usize),
    timing: (f64, f64, f64, f64, f64),
    league_size: usize,
}

struct SeatJob {
    obs: Vec<(Vec<f32>, Vec<f32>, usize)>,
    reply: Sender<Vec<Vec<f32>>>,
}

fn league_nets<G: Game>(weights: &[Vec<u8>], bf16: bool, dev: &Device) -> Result<Vec<net::Net>> {
    let mut out = vec![];
    for (k, bytes) in weights.iter().enumerate() {
        let (mut vm, mut net) = net::build(G::spec(), dev)?;
        if bf16 {
            net.dtype = candle_core::DType::BF16;
        }
        let path =
            std::env::temp_dir().join(format!("kg-league-{}-{k}.safetensors", std::process::id()));
        std::fs::write(&path, bytes).unwrap();
        vm.load(&path)?;
        let _ = std::fs::remove_file(&path);
        out.push(net.frozen());
    }
    Ok(out)
}

fn league_server<G: Game>(
    jobs: &std::sync::Mutex<Receiver<SeatJob>>,
    nets: &[net::Net],
    dev: &Device,
    members: usize,
) -> Result<()> {
    let spec = G::spec();
    let (gsize, ns) = (spec.channels * spec.side * spec.side, spec.scalars);
    let na = spec.actions;
    loop {
        let batch = {
            let jobs = jobs.lock().unwrap();
            let Ok(first) = jobs.recv() else {
                return Ok(());
            };
            let mut batch = vec![first];
            while batch.len() < members {
                match jobs.try_recv() {
                    Ok(j) => batch.push(j),
                    Err(_) => break,
                }
            }
            batch
        };
        let mut replies: Vec<Vec<Vec<f32>>> = batch.iter().map(|_| vec![]).collect();
        for (p, net) in nets.iter().enumerate() {
            let rows: usize = batch.iter().map(|j| j.obs[p].2).sum();
            if rows == 0 {
                for r in replies.iter_mut() {
                    r.push(vec![]);
                }
                continue;
            }
            let mut g = Vec::with_capacity(rows * gsize);
            let mut sc = Vec::with_capacity(rows * ns);
            for j in &batch {
                g.extend_from_slice(&j.obs[p].0);
                sc.extend_from_slice(&j.obs[p].1);
            }
            let gt = Tensor::from_slice(&g, (rows, gsize), dev)?;
            let st = Tensor::from_slice(&sc, (rows, ns), dev)?;
            let probs: Vec<f32> = candle_nn::ops::softmax(&net.act(&gt, &st)?, D::Minus1)?
                .flatten_all()?
                .to_vec1()?;
            let mut at = 0;
            for (j, r) in batch.iter().zip(replies.iter_mut()) {
                let n = j.obs[p].2 * na;
                r.push(probs[at..at + n].to_vec());
                at += n;
            }
        }
        for (j, r) in batch.into_iter().zip(replies) {
            let _ = j.reply.send(r);
        }
    }
}

fn adam(vm: &VarMap, lr: f64) -> Result<AdamW> {
    AdamW::new(
        vm.all_vars(),
        ParamsAdamW {
            lr,
            weight_decay: 0.0,
            eps: 1e-5,
            ..Default::default()
        },
    )
}

impl<G: Game> Member<G> {
    fn new(id: usize, a: &Args, genes: Vec<f32>, lr: f64, ent: f32) -> Result<Self> {
        let dev = Device::new_cuda_with_stream(0)?;
        if let Device::Cuda(c) = &dev {
            unsafe { c.cuda_stream().context().disable_event_tracking() };
        }
        let (mut vm, mut net) = net::build(G::spec(), &dev)?;
        if a.bf16 {
            net.dtype = candle_core::DType::BF16;
        }
        if !a.resume.is_empty() {
            vm.load(&a.resume)?;
        }
        let opt = adam(&vm, lr)?;
        let envs: Vec<G> = (0..a.envs)
            .map(|i| G::make(1000 + (id * a.envs + i) as u64 * 7919, a.secs, &genes))
            .collect();
        let batch = Batch::new(envs, (a.threads / a.pop.max(1)).max(2));
        let n = batch.agents();
        let s = G::spec();
        let mut m = Self {
            id,
            dev,
            rng: Rng(0x9e3779b97f4a7c15 ^ ((id as u64 + 1) * 0x2545f4914f6cdd1d)),
            vm,
            net,
            opt,
            batch,
            genes,
            lr,
            ent,
            grid: vec![0.0; n * s.channels * s.side * s.side],
            scal: vec![0.0; n * s.scalars],
            ticks: (0..n)
                .map(|_| Tick {
                    reward: 0.0,
                    done: false,
                })
                .collect(),
            commit: vec![Commit::default(); n],
            score: f32::MIN,
            vs_bots: 0.0,
            vs_league: 0.0,
            text: String::new(),
            episodes: (0.0, 0.0, 0),
            timing: (0.0, 0.0, 0.0, 0.0, 0.0),
            league_size: 0,
        };
        m.batch.write_obs(&mut m.grid, &mut m.scal);
        Ok(m)
    }

    fn update(
        &mut self,
        a: &Args,
        explore: f32,
        ent_boost: f32,
        league: Option<&Sender<SeatJob>>,
    ) -> Result<(f32, f32)> {
        let dev = &self.dev.clone();
        let rng = &mut Rng(self.rng.0);
        let out = self.rollout_and_learn(a, dev, rng, explore, ent_boost, league);
        self.rng = Rng(rng.0);
        out
    }

    fn rollout_and_learn(
        &mut self,
        a: &Args,
        dev: &Device,
        rng: &mut Rng,
        explore: f32,
        ent_boost: f32,
        league: Option<&Sender<SeatJob>>,
    ) -> Result<(f32, f32)> {
        let spec = G::spec();
        let (gsize, ns, na) = (
            spec.channels * spec.side * spec.side,
            spec.scalars,
            spec.actions,
        );
        let n = self.batch.agents();
        let t_steps = a.steps;
        let (gamma, lam) = (0.995f32, 0.95f32);
        let total = t_steps * n;
        let mut obs_g = vec![0.0f32; total * gsize];
        let mut obs_s = vec![0.0f32; total * ns];
        let mut acts = vec![0u32; total];
        let mut logp = vec![0.0f32; total];
        let mut vals = vec![0.0f32; total];
        let mut rews = vec![0.0f32; total];
        let mut dones = vec![0.0f32; total];
        let clock = Instant::now();
        let mut sim = 0.0f64;
        let mut look = 0.0f64;
        let mut rivals = 0.0f64;
        let actor = self.net.frozen();
        for t in 0..t_steps {
            let g = Tensor::from_slice(&self.grid, (n, gsize), dev)?;
            let s = Tensor::from_slice(&self.scal, (n, ns), dev)?;
            let (logits, v) = actor.forward(&g, &s)?;
            let both: Vec<f32> = Tensor::cat(
                &[
                    &candle_nn::ops::softmax(&logits, D::Minus1)?,
                    &v.unsqueeze(1)?,
                ],
                1,
            )?
            .flatten_all()?
            .to_vec1()?;
            let row = na + 1;
            let v: Vec<f32> = (0..n).map(|k| both[k * row + na]).collect();
            let mut chosen = vec![0usize; n];
            for k in 0..n {
                let p = &both[k * row..k * row + na];
                let mut r = rng.uniform();
                let mut pick = na - 1;
                for (j, q) in p.iter().enumerate() {
                    if r < *q {
                        pick = j;
                        break;
                    }
                    r -= q;
                }
                let pick = self.commit[k].pick(pick, explore, na, rng);
                chosen[k] = pick;
                let o = t * n + k;
                acts[o] = pick as u32;
                logp[o] = p[pick].max(1e-8).ln();
                vals[o] = v[k];
            }
            obs_g[t * n * gsize..(t + 1) * n * gsize].copy_from_slice(&self.grid);
            obs_s[t * n * ns..(t + 1) * n * ns].copy_from_slice(&self.scal);
            let t0 = Instant::now();
            self.drive_seats(league, rng)?;
            rivals += t0.elapsed().as_secs_f64();
            let t0 = Instant::now();
            self.batch.step(&chosen, &mut self.ticks);
            sim += t0.elapsed().as_secs_f64();
            for k in 0..n {
                rews[t * n + k] = self.ticks[k].reward;
                dones[t * n + k] = if self.ticks[k].done { 1.0 } else { 0.0 };
                if self.ticks[k].done {
                    self.commit[k] = Commit::default();
                }
            }
            let t0 = Instant::now();
            self.batch.write_obs(&mut self.grid, &mut self.scal);
            look += t0.elapsed().as_secs_f64();
        }
        let rollout = clock.elapsed().as_secs_f64();
        let g = Tensor::from_slice(&self.grid, (n, gsize), dev)?;
        let s = Tensor::from_slice(&self.scal, (n, ns), dev)?;
        let next_v: Vec<f32> = actor.forward(&g, &s)?.1.to_vec1()?;
        let mut adv = vec![0.0f32; total];
        let mut last = vec![0.0f32; n];
        for t in (0..t_steps).rev() {
            for k in 0..n {
                let o = t * n + k;
                let nxt = if t == t_steps - 1 {
                    next_v[k]
                } else {
                    vals[o + n]
                };
                let alive = 1.0 - dones[o];
                let delta = rews[o] + gamma * nxt * alive - vals[o];
                last[k] = delta + gamma * lam * alive * last[k];
                adv[o] = last[k];
            }
        }
        let ret: Vec<f32> = adv.iter().zip(&vals).map(|(a, v)| a + v).collect();
        let mean = adv.iter().sum::<f32>() / adv.len() as f32;
        let sd = (adv.iter().map(|x| (x - mean).powi(2)).sum::<f32>() / adv.len() as f32).sqrt();
        let nadv: Vec<f32> = adv.iter().map(|x| (x - mean) / (sd + 1e-8)).collect();
        let bg = Tensor::from_slice(&obs_g, (total, gsize), dev)?;
        let bs = Tensor::from_slice(&obs_s, (total, ns), dev)?;
        let ba = Tensor::from_slice(&acts, total, dev)?;
        let blp = Tensor::from_slice(&logp, total, dev)?;
        let badv = Tensor::from_slice(&nadv, total, dev)?;
        let bret = Tensor::from_slice(&ret, total, dev)?;
        let mb = a.minibatch.min(total);
        let mut stats = (0.0f32, 0.0f32);
        let learn_from = Instant::now();
        self.opt.set_learning_rate(self.lr);
        for _ in 0..3 {
            let mut order: Vec<u32> = (0..total as u32).collect();
            for i in (1..order.len()).rev() {
                order.swap(i, rng.below(i + 1));
            }
            for chunk in order.chunks(mb) {
                let idx = Tensor::from_slice(chunk, chunk.len(), dev)?;
                let (logits, v) = self
                    .net
                    .forward(&bg.index_select(&idx, 0)?, &bs.index_select(&idx, 0)?)?;
                let lsm = candle_nn::ops::log_softmax(&logits, D::Minus1)?;
                let a_ = ba.index_select(&idx, 0)?;
                let new_lp = lsm.gather(&a_.unsqueeze(1)?, 1)?.squeeze(1)?;
                let ratio = (new_lp - blp.index_select(&idx, 0)?)?.exp()?;
                let ad = badv.index_select(&idx, 0)?;
                let s1 = (&ratio * &ad)?;
                let s2 = (ratio.clamp(0.8f32, 1.2f32)? * &ad)?;
                let pg = s1.minimum(&s2)?.mean_all()?.neg()?;
                let vl = (v - bret.index_select(&idx, 0)?)?.sqr()?.mean_all()?;
                let ent = (lsm.exp()? * &lsm)?.sum(D::Minus1)?.mean_all()?.neg()?;
                let loss = ((&pg + (&vl * 0.5)?)? - (&ent * (self.ent * ent_boost) as f64)?)?;
                self.opt.backward_step(&loss)?;
                stats = (ent.to_scalar::<f32>()?, vl.to_scalar::<f32>()?);
            }
        }
        let learn = learn_from.elapsed().as_secs_f64();
        let t = &mut self.timing;
        t.0 += sim;
        t.1 += look;
        t.2 += rollout - sim - look - rivals;
        t.4 += rivals;
        t.3 += learn;
        let eps = self.batch.drain_episodes();
        let (r, m, c) = &mut self.episodes;
        for e in eps {
            *r += e.0;
            *m += e.1;
            *c += 1;
        }
        Ok(stats)
    }

    fn drive_seats(&mut self, league: Option<&Sender<SeatJob>>, rng: &mut Rng) -> Result<()> {
        let Some(league) = league else {
            return Ok(());
        };
        let na = G::spec().actions;
        let by = self.batch.seated(self.league_size);
        if by.iter().all(|l| l.is_empty()) {
            return Ok(());
        }
        let mut obs = vec![];
        for list in &by {
            let (mut g, mut sc) = (vec![], vec![]);
            self.batch.seat_obs(list, &mut g, &mut sc);
            obs.push((g, sc, list.len()));
        }
        let (reply, back) = std::sync::mpsc::channel();
        league.send(SeatJob { obs, reply }).unwrap();
        let probs: Vec<Vec<f32>> = back.recv().unwrap();
        let mut picks: Vec<Vec<usize>> = self
            .batch
            .envs
            .iter()
            .map(|e| vec![0; e.seats().len()])
            .collect();
        for (list, probs) in by.iter().zip(&probs) {
            for (k, &(e, seat)) in list.iter().enumerate() {
                let p = &probs[k * na..(k + 1) * na];
                let mut r = rng.uniform();
                let mut pick = na - 1;
                for (j, q) in p.iter().enumerate() {
                    if r < *q {
                        pick = j;
                        break;
                    }
                    r -= q;
                }
                picks[e][seat] = pick;
            }
        }
        self.batch.give_seat_actions(&picks);
        Ok(())
    }

    fn take_episodes(&mut self) -> Option<(f32, f32, usize)> {
        let (r, m, c) = std::mem::take(&mut self.episodes);
        (c > 0).then(|| (r / c as f32, m / c as f32, c))
    }

    fn copy_from(&mut self, other: &Member<G>) -> Result<()> {
        let src = other.vm.data().lock().unwrap();
        for (k, v) in self.vm.data().lock().unwrap().iter() {
            v.set(src[k].as_tensor())?;
        }
        drop(src);
        self.opt = adam(&self.vm, other.lr)?;
        self.genes = other.genes.clone();
        self.lr = other.lr;
        self.ent = other.ent;
        Ok(())
    }

    fn mutate(&mut self, rng: &mut Rng) {
        for g in &mut self.genes {
            *g *= rng.jiggle(0.8, 1.25);
        }
        self.lr = (self.lr * rng.jiggle(0.8, 1.25) as f64).clamp(2e-5, 1e-3);
        self.ent = (self.ent * rng.jiggle(0.8, 1.25)).clamp(0.002, 0.05);
        let genes = self.genes.clone();
        for e in &mut self.batch.envs {
            e.set_genes(&genes);
        }
    }

    fn describe_genes(&self) -> String {
        let names = G::genes();
        let mut s = format!("lr {:.1e} ent {:.3}", self.lr, self.ent);
        for ((name, _), v) in names.iter().zip(&self.genes) {
            s += &format!(" {name} {v:.3}");
        }
        s
    }
}

fn population<G: Game>(a: &Args) -> Result<()> {
    let dev = Device::new_cuda(0)?;
    let mut rng = Rng(0x9e3779b97f4a7c15);
    let defaults: Vec<f32> = G::genes().iter().map(|g| g.1).collect();
    let mut members: Vec<Member<G>> = vec![];
    for id in 0..a.pop {
        let (genes, lr, ent) = if id == 0 {
            (defaults.clone(), a.lr, a.ent)
        } else {
            (
                defaults.iter().map(|g| g * rng.jiggle(0.5, 2.0)).collect(),
                a.lr * rng.jiggle(0.5, 2.0) as f64,
                a.ent * rng.jiggle(0.5, 2.0),
            )
        };
        members.push(Member::new(id, a, genes, lr, ent)?);
    }
    let params = members[0].net.export()?.param_count();
    let (base, base_text) = G::baseline(a.eval_seeds);
    println!(
        "game {} on {dev:?}: population {} x {} agents, policy {params} weights; hand-written bot: {base_text} (score {base:.2})",
        a.game,
        a.pop,
        members[0].batch.agents()
    );
    for m in &members {
        println!("  member {}: {}", m.id, m.describe_genes());
    }
    let live = format!("{}.live", a.out);
    let mut league: Vec<Policy> = vec![];
    let mut league_weights: Vec<Vec<u8>> = vec![];
    let mut stale = 0;
    let mut leader = 0;
    let mut start;
    let mut steps;
    for round in 1..=a.generations {
        G::set_seats(league.len());
        for m in members.iter_mut() {
            m.league_size = league.len();
        }
        let mut servers = vec![];
        for _ in 0..a.league_servers {
            let dev = Device::new_cuda_with_stream(0)?;
            let nets = league_nets::<G>(&league_weights, a.bf16, &dev)?;
            servers.push((dev, nets));
        }
        save(&members[leader].net.export()?, &live);
        std::fs::write(format!("{live}.round"), round.to_string()).unwrap();
        println!(
            "=== generation {round}: opponents are hand-written bots{}",
            if league.is_empty() {
                String::new()
            } else {
                format!(" and {} league models", league.len())
            }
        );
        let fresh = 1.0 - ((round - 1) as f32 / a.warmup.max(1) as f32).min(1.0);
        let explore = 0.01 + (a.explore - 0.01) * fresh;
        let ent_boost = 1.0;
        println!("  explore {explore:.3}");
        let train_from = Instant::now();
        start = Instant::now();
        steps = 0;
        for u in 1..=a.gen_updates {
            let mut line = format!("round {round} upd {u:3}");
            let pop = members.len();
            let results: Vec<Result<(f32, f32)>> = std::thread::scope(|sc| {
                let (jobs, inbox) = std::sync::mpsc::channel::<SeatJob>();
                let inbox = std::sync::Arc::new(std::sync::Mutex::new(inbox));
                let running: Vec<_> = servers
                    .iter()
                    .filter(|(_, nets)| !nets.is_empty())
                    .map(|(dev, nets)| {
                        let inbox = inbox.clone();
                        sc.spawn(move || league_server::<G>(&inbox, nets, dev, pop))
                    })
                    .collect();
                let hs: Vec<_> = members
                    .iter_mut()
                    .map(|m| {
                        let tx = jobs.clone();
                        sc.spawn(move || {
                            let league = (m.league_size > 0).then_some(&tx);
                            m.update(a, explore, ent_boost, league)
                        })
                    })
                    .collect();
                drop(jobs);
                let out = hs.into_iter().map(|h| h.join().unwrap()).collect();
                for s in running {
                    s.join().unwrap().unwrap();
                }
                out
            });
            for (m, r) in members.iter_mut().zip(results) {
                let (ent, _) = r?;
                steps += a.steps * m.batch.agents();
                if u % 10 == 0 {
                    match m.take_episodes() {
                        Some((r, lvl, _)) => {
                            line += &format!(" | m{} ret {r:+.2} lvl {lvl:.1} ent {ent:.2}", m.id)
                        }
                        None => line += &format!(" | m{} -", m.id),
                    }
                }
            }
            if u % 5 == 0 {
                save(&members[leader].net.export()?, &live);
            }
            if u % 10 == 0 {
                let sps = steps as f64 / start.elapsed().as_secs_f64();
                steps = 0;
                start = Instant::now();
                let t = members.iter().fold((0.0, 0.0, 0.0, 0.0, 0.0), |a, m| {
                    let x = m.timing;
                    (a.0 + x.0, a.1 + x.1, a.2 + x.2, a.3 + x.3, a.4 + x.4)
                });
                for m in members.iter_mut() {
                    m.timing = (0.0, 0.0, 0.0, 0.0, 0.0);
                }
                let all = (t.0 + t.1 + t.2 + t.3 + t.4).max(1e-9);
                println!(
                    "{line} | sps {sps:.0} | time: sim {:.0}% observe {:.0}% act {:.0}% rivals {:.0}% learn {:.0}%",
                    100.0 * t.0 / all,
                    100.0 * t.1 / all,
                    100.0 * t.2 / all,
                    100.0 * t.4 / all,
                    100.0 * t.3 / all
                );
            }
        }
        let eval_from = Instant::now();
        let mut policies = vec![];
        for m in members.iter() {
            let policy = m.net.export()?;
            let err = parity(&m.net, &policy, m.batch.envs[0].obs(0), &m.dev)?;
            let limit = if a.bf16 { 0.15 } else { 1e-4 };
            if err > limit {
                panic!("candle and the game's inference disagree: {err}");
            }
            policies.push(policy);
        }
        let league_ref = &league;
        let seeds = a.eval_seeds;
        let results: Vec<((f32, String), Option<f32>)> = std::thread::scope(|sc| {
            let hs: Vec<_> = policies
                .iter()
                .map(|p| {
                    let bots = sc.spawn(move || G::eval(p, &[], seeds));
                    let vs = (!league_ref.is_empty())
                        .then(|| sc.spawn(move || G::eval(p, league_ref, seeds).0));
                    (bots, vs)
                })
                .collect();
            hs.into_iter()
                .map(|(b, v)| (b.join().unwrap(), v.map(|h| h.join().unwrap())))
                .collect()
        });
        for (m, ((sb, tb), sl)) in members.iter_mut().zip(results) {
            m.vs_bots = sb;
            m.vs_league = sl.unwrap_or(sb);
            m.score = (m.vs_bots + m.vs_league) / 2.0;
            m.text = tb;
        }
        let mut order: Vec<usize> = (0..members.len()).collect();
        order.sort_by(|x, y| members[*y].score.total_cmp(&members[*x].score));
        println!(
            "EVAL round {round} (bot {base:.2}): train {:.0}s, eval {:.0}s",
            eval_from.duration_since(train_from).as_secs_f32(),
            eval_from.elapsed().as_secs_f32()
        );
        for &k in &order {
            let m = &members[k];
            println!(
                "  m{} score {:.2} (vs bots {:.2}, vs league {:.2}) {} | {}",
                m.id,
                m.score,
                m.vs_bots,
                m.vs_league,
                m.text,
                m.describe_genes()
            );
        }
        leader = order[0];
        let best = &members[leader];
        let defending = match league.last() {
            Some(p) if !league.is_empty() => {
                let bots = G::eval(p, &[], seeds).0;
                let vs = G::eval(p, &league, seeds).0;
                (bots + vs) / 2.0
            }
            _ => f32::MIN,
        };
        if league.len() > 0 {
            println!("  champion replayed on the same games: score {defending:.2}");
        }
        let champion = defending;
        if best.score > champion + a.min_gain {
            stale = 0;
            let policy = best.net.export()?;
            save(&policy, &a.out);
            best.vm.save(format!("{}.safetensors", a.out))?;
            save(&policy, &format!("{}.round{round}", a.out));
            league.push(policy);
            league_weights.push(std::fs::read(format!("{}.safetensors", a.out)).unwrap());
            if league.len() > a.league {
                league.remove(0);
                league_weights.remove(0);
            }
            println!(
                "  new champion m{} -> {} (league {})",
                best.id,
                a.out,
                league.len()
            );
        } else {
            stale += 1;
            println!(
                "  no gain over champion {champion:.2} ({stale}/{})",
                a.patience
            );
            if stale >= a.patience && champion > base {
                println!(
                    "stopping: the champion beats the bot and {stale} generations brought no gain"
                );
                break;
            }
        }
        let cut = (a.pop / 4).max(usize::from(a.pop > 1));
        for i in 0..cut {
            let loser = order[order.len() - 1 - i];
            let donor = order[rng.below(cut)];
            if loser == donor {
                continue;
            }
            let (l, d) = if loser < donor {
                let (lo, hi) = members.split_at_mut(donor);
                (&mut lo[loser], &hi[0])
            } else {
                let (lo, hi) = members.split_at_mut(loser);
                (&mut hi[0], &lo[donor])
            };
            l.copy_from(d)?;
            l.mutate(&mut rng);
            println!(
                "  m{} copies m{} and mutates: {}",
                l.id,
                d.id,
                l.describe_genes()
            );
        }
    }
    Ok(())
}

fn main() -> Result<()> {
    let a = args();
    kg_core::save::use_memory_only();
    candle_core::cuda::set_gemm_reduced_precision_f32(std::env::var("KG_TF32").is_ok());
    match a.game.as_str() {
        "clash" => population::<kg_core::games::clash::rl::Arena>(&a),
        "paper" => population::<kg_core::games::paper::rl::Arena>(&a),
        g => panic!("unknown game {g}"),
    }
}
