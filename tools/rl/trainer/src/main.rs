mod net;
mod popnet;

mod devices;
use candle_core::{Device, Result, Tensor, D};
use candle_nn::{AdamW, Optimizer, ParamsAdamW, VarMap};
use popnet::{Knobs, PopNet, Rollout, Trainer};
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
    groups: usize,
    device: String,
    demos: String,
    bc_epochs: usize,
    gene_set: Vec<(String, f32)>,
    critic_rounds: usize,
    holdout: bool,
    holdout_top: usize,
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
        groups: 1,
        device: devices::best().into(),
        demos: String::new(),
        bc_epochs: 4,
        gene_set: vec![],
        critic_rounds: 0,
        holdout: true,
        holdout_top: 3,
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
            "--device" => a.device = x,
            "--gene" => {
                let (k, v) = x.split_once('=').unwrap();
                a.gene_set.push((k.to_string(), v.parse().unwrap()));
            }
            "--critic-rounds" => a.critic_rounds = x.parse().unwrap(),
            "--holdout" => a.holdout = x != "0",
            "--holdout-top" => a.holdout_top = x.parse().unwrap(),
            "--demos" => a.demos = x,
            "--bc-epochs" => a.bc_epochs = x.parse().unwrap(),
            "--groups" => a.groups = x.parse().unwrap(),
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
    fn set_eval_threads(_: usize) {}
    fn set_eval_offset(_: u64) {}
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
        r.level + 5.0 * r.kills + 20.0 * r.wins + 5.0 * r.secs / GAME_CAP
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

        fn set_eval_threads(n: usize) {
            kg_core::games::clash::rl::set_eval_threads(n);
        }

        fn set_eval_offset(n: u64) {
            kg_core::games::clash::rl::set_eval_offset(n);
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
            stall: g[7],
            trip: g[8],
            wobble: g[9],
            steal: g[10],
        }
    }

    fn score(r: &Report) -> f32 {
        let per_min = r.kills / r.secs.max(1.0) * 60.0;
        100.0 * (r.wins + r.outlived) * (1.0 - r.circling) * (1.0 - r.stall)
            + 300.0 * r.peak
            + 300.0 * r.claim
            + 800.0 * r.stolen
            + 8.0 * per_min
            - 30.0 * r.circling
            - 15.0 * r.stall
            - 20.0 * r.wobble
    }

    fn describe(r: &Report) -> String {
        format!(
            "won {:.0}% outlived {:.0}% peak {:.1}% lived {:.0}s kills {:.2} inside {:.0}% circling {:.1}% stalled {:.0}% claim {:.2}% wobble {:.0}% stolen {:.2}%",
            r.wins * 100.0,
            r.outlived * 100.0,
            r.peak * 100.0,
            r.secs,
            r.kills,
            r.inside * 100.0,
            r.circling * 100.0,
            r.stall * 100.0,
            r.claim * 100.0,
            r.wobble * 100.0,
            r.stolen * 100.0
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
                ("stall", r.stall),
                ("trip", r.trip),
                ("wobble", r.wobble),
                ("steal", r.steal),
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
            kg_core::games::paper::rl::set_seats(n, 0.7);
        }

        fn set_eval_threads(n: usize) {
            kg_core::games::paper::rl::set_eval_threads(n);
        }

        fn set_eval_offset(n: u64) {
            kg_core::games::paper::rl::set_eval_offset(n);
        }
    }
}

struct Member<G: Game> {
    id: usize,
    rng: Rng,
    buf: Buf,
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
    pg_scale: f32,
}

#[derive(Default)]
struct Buf {
    obs_g: Vec<f32>,
    obs_s: Vec<f32>,
    acts: Vec<u32>,
    logp: Vec<f32>,
    vals: Vec<f32>,
    rews: Vec<f32>,
    dones: Vec<f32>,
}

struct SeatJob {
    obs: Vec<(Vec<f32>, Vec<f32>, usize)>,
    reply: Sender<Vec<Vec<f32>>>,
}

struct LeagueNet {
    net: PopNet,
    weights: popnet::Weights,
    count: usize,
}

fn upload(data: &[f32], shape: (usize, usize), dev: &Device, dt: candle_core::DType) -> Result<Tensor> {
    if dt == candle_core::DType::BF16 {
        let v: Vec<half::bf16> = data.iter().map(|x| half::bf16::from_f32(*x)).collect();
        Tensor::from_vec(v, shape, dev)
    } else {
        Tensor::from_slice(data, shape, dev)
    }
}

fn gather(
    batch: &[SeatJob],
    l: usize,
    widest: usize,
    width: usize,
    dev: &Device,
    dt: candle_core::DType,
    part: impl Fn(&(Vec<f32>, Vec<f32>, usize)) -> &Vec<f32>,
) -> Result<Tensor> {
    let bf = dt == candle_core::DType::BF16;
    let mut f = Vec::<f32>::new();
    let mut h = Vec::<half::bf16>::new();
    if bf {
        h.resize(l * widest * width, half::bf16::ZERO);
    } else {
        f.resize(l * widest * width, 0.0);
    }
    for p in 0..l {
        let mut at = p * widest * width;
        for j in batch {
            let src = part(&j.obs[p]);
            if bf {
                for (d, s) in h[at..at + src.len()].iter_mut().zip(src) {
                    *d = half::bf16::from_f32(*s);
                }
            } else {
                f[at..at + src.len()].copy_from_slice(src);
            }
            at += src.len();
        }
    }
    if bf {
        Tensor::from_vec(h, (l, widest, width), dev)
    } else {
        Tensor::from_vec(f, (l, widest, width), dev)
    }
}

fn league_nets<G: Game>(weights: &[Vec<u8>], bf16: bool, dev: &Device) -> Result<Option<LeagueNet>> {
    if weights.is_empty() {
        return Ok(None);
    }
    let mut rows = vec![];
    let mut layout = None;
    for (k, bytes) in weights.iter().enumerate() {
        let (mut vm, _) = net::build(G::spec(), dev)?;
        let path =
            std::env::temp_dir().join(format!("kg-league-{}-{k}.safetensors", std::process::id()));
        std::fs::write(&path, bytes).unwrap();
        vm.load(&path)?;
        let _ = std::fs::remove_file(&path);
        let lay = popnet::Layout::of(&vm);
        rows.extend(lay.pack(&vm)?);
        layout = Some(lay);
    }
    let layout = layout.expect("a league model");
    let total = layout.total;
    let dtype = if bf16 {
        candle_core::DType::BF16
    } else {
        candle_core::DType::F32
    };
    let net = PopNet::new(G::spec(), layout, dtype, dev)?;
    let flat = Tensor::from_vec(rows, (weights.len(), total), dev)?;
    let weights = net.weights(&flat)?;
    Ok(Some(LeagueNet {
        net,
        weights,
        count: flat.dim(0)?,
    }))
}

fn league_server<G: Game>(
    jobs: &std::sync::Mutex<Receiver<SeatJob>>,
    league: &LeagueNet,
    dev: &Device,
    members: usize,
) -> Result<()> {
    let spec = G::spec();
    let (gsize, ns) = (spec.channels * spec.side * spec.side, spec.scalars);
    let na = spec.actions;
    let l = league.count;
    loop {
        let batch = {
            let jobs = jobs.lock().unwrap();
            let Ok(first) = jobs.recv() else {
                return Ok(());
            };
            let mut batch = vec![first];
            while batch.len() < members {
                match jobs.recv() {
                    Ok(j) => batch.push(j),
                    Err(_) => break,
                }
            }
            batch
        };
        let rows: Vec<usize> = (0..l)
            .map(|p| batch.iter().map(|j| j.obs[p].2).sum())
            .collect();
        let widest = rows.iter().copied().max().unwrap_or(0);
        let mut replies: Vec<Vec<Vec<f32>>> = batch.iter().map(|_| vec![vec![]; l]).collect();
        if widest > 0 {
            let dt = league.net.dtype;
            let gt = gather(&batch, l, widest, gsize, dev, dt, |o| &o.0)?;
            let st = gather(&batch, l, widest, ns, dev, dt, |o| &o.1)?;
            let (logits, _) = league.net.forward(&league.weights, &gt, &st)?;
            let probs: Vec<f32> = candle_nn::ops::softmax(&logits, D::Minus1)?
                .flatten_all()?
                .to_vec1()?;
            for p in 0..l {
                let mut at = p * widest * na;
                for (j, r) in batch.iter().zip(replies.iter_mut()) {
                    let n = j.obs[p].2 * na;
                    r[p] = probs[at..at + n].to_vec();
                    at += n;
                }
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
        let envs: Vec<G> = (0..a.envs)
            .map(|i| G::make(1000 + (id * a.envs + i) as u64 * 7919, a.secs, &genes))
            .collect();
        let batch = Batch::new(envs, (a.threads / a.pop.max(1)).max(2));
        let n = batch.agents();
        let s = G::spec();
        let mut m = Self {
            id,
            rng: Rng(0x9e3779b97f4a7c15 ^ ((id as u64 + 1) * 0x2545f4914f6cdd1d)),
            buf: Buf::default(),
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
            pg_scale: 1.0,
        };
        m.batch.write_obs(&mut m.grid, &mut m.scal);
        Ok(m)
    }

    fn begin(&mut self, steps: usize) {
        let spec = G::spec();
        let n = self.batch.agents();
        let total = steps * n;
        let b = &mut self.buf;
        b.obs_g = vec![0.0; total * spec.channels * spec.side * spec.side];
        b.obs_s = vec![0.0; total * spec.scalars];
        b.acts = vec![0; total];
        b.logp = vec![0.0; total];
        b.vals = vec![0.0; total];
        b.rews = vec![0.0; total];
        b.dones = vec![0.0; total];
    }

    fn act_and_step(
        &mut self,
        t: usize,
        probs: &[f32],
        explore: f32,
        league: Option<&Sender<SeatJob>>,
    ) -> Result<f64> {
        let spec = G::spec();
        let (gsize, ns, na) = (
            spec.channels * spec.side * spec.side,
            spec.scalars,
            spec.actions,
        );
        let n = self.batch.agents();
        let rng = &mut Rng(self.rng.0);
        let row = na + 1;
        let mut chosen = vec![0usize; n];
        for k in 0..n {
            let p = &probs[k * row..k * row + na];
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
            self.buf.acts[o] = pick as u32;
            self.buf.logp[o] = p[pick].max(1e-8).ln();
            self.buf.vals[o] = probs[k * row + na];
        }
        self.buf.obs_g[t * n * gsize..(t + 1) * n * gsize].copy_from_slice(&self.grid);
        self.buf.obs_s[t * n * ns..(t + 1) * n * ns].copy_from_slice(&self.scal);
        let t0 = Instant::now();
        self.drive_seats(league, rng)?;
        let rivals = t0.elapsed().as_secs_f64();
        self.batch.step(&chosen, &mut self.ticks);
        for k in 0..n {
            self.buf.rews[t * n + k] = self.ticks[k].reward;
            self.buf.dones[t * n + k] = if self.ticks[k].done { 1.0 } else { 0.0 };
            if self.ticks[k].done {
                self.commit[k] = Commit::default();
            }
        }
        self.batch.write_obs(&mut self.grid, &mut self.scal);
        self.rng = Rng(rng.0);
        Ok(rivals)
    }

    fn finish(&mut self, next_v: &[f32], steps: usize, dev: &Device, dt: candle_core::DType) -> Result<Rollout> {
        let n = self.batch.agents();
        let total = steps * n;
        let (gamma, lam) = (0.995f32, 0.95f32);
        let b = std::mem::take(&mut self.buf);
        let mut adv = vec![0.0f32; total];
        let mut last = vec![0.0f32; n];
        for t in (0..steps).rev() {
            for k in 0..n {
                let o = t * n + k;
                let nxt = if t == steps - 1 { next_v[k] } else { b.vals[o + n] };
                let alive = 1.0 - b.dones[o];
                let delta = b.rews[o] + gamma * nxt * alive - b.vals[o];
                last[k] = delta + gamma * lam * alive * last[k];
                adv[o] = last[k];
            }
        }
        let ret: Vec<f32> = adv.iter().zip(&b.vals).map(|(a, v)| a + v).collect();
        let mean = adv.iter().sum::<f32>() / adv.len() as f32;
        let sd = (adv.iter().map(|x| (x - mean).powi(2)).sum::<f32>() / adv.len() as f32).sqrt();
        let adv: Vec<f32> = adv.iter().map(|x| (x - mean) / (sd + 1e-8)).collect();
        let eps = self.batch.drain_episodes();
        let (r, m, c) = &mut self.episodes;
        for e in eps {
            *r += e.0;
            *m += e.1;
            *c += 1;
        }
        let spec = G::spec();
        let (gsize, ns) = (spec.channels * spec.side * spec.side, spec.scalars);
        Ok(Rollout {
            obs_g: upload(&b.obs_g, (total, gsize), dev, dt)?,
            obs_s: upload(&b.obs_s, (total, ns), dev, dt)?,
            acts: Tensor::from_vec(b.acts, total, dev)?,
            logp: Tensor::from_vec(b.logp, total, dev)?,
            adv: Tensor::from_vec(adv, total, dev)?,
            ret: Tensor::from_vec(ret, total, dev)?,
        })
    }

    fn drive_seats(&mut self, league: Option<&Sender<SeatJob>>, rng: &mut Rng) -> Result<()> {
        let Some(league) = league else {
            return Ok(());
        };
        let na = G::spec().actions;
        let by = self.batch.seated(self.league_size);
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

    fn copy_from(&mut self, other: &Member<G>) {
        self.genes = other.genes.clone();
        self.lr = other.lr;
        self.ent = other.ent;
    }

    fn mutate(&mut self, rng: &mut Rng) {
        for g in &mut self.genes {
            *g *= rng.jiggle(0.8, 1.25);
        }
        self.lr = (self.lr * rng.jiggle(0.8, 1.25) as f64).clamp(1e-6, 1e-3);
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

fn observations<G: Game>(
    members: &[Member<G>],
    dev: &Device,
    dt: candle_core::DType,
    (n, gsize, ns): (usize, usize, usize),
) -> Result<(Tensor, Tensor)> {
    let parts: Vec<Result<(Tensor, Tensor)>> = std::thread::scope(|sc| {
        let hs: Vec<_> = members
            .iter()
            .map(|m| {
                sc.spawn(move || {
                    Ok((
                        upload(&m.grid, (n, gsize), dev, dt)?,
                        upload(&m.scal, (n, ns), dev, dt)?,
                    ))
                })
            })
            .collect();
        hs.into_iter().map(|h| h.join().unwrap()).collect()
    });
    let parts = parts.into_iter().collect::<Result<Vec<_>>>()?;
    let g: Vec<&Tensor> = parts.iter().map(|p| &p.0).collect();
    let s: Vec<&Tensor> = parts.iter().map(|p| &p.1).collect();
    Ok((Tensor::stack(&g, 0)?, Tensor::stack(&s, 0)?))
}

fn policy_forward(
    tr: &Trainer,
    w: &popnet::Weights,
    g: &Tensor,
    s: &Tensor,
    with_probs: bool,
) -> Result<Vec<f32>> {
    let (logits, v) = tr.net.forward(w, g, s)?;
    let out = if with_probs {
        Tensor::cat(
            &[&candle_nn::ops::softmax(&logits, D::Minus1)?, &v.unsqueeze(2)?],
            2,
        )?
    } else {
        v
    };
    out.flatten_all()?.to_vec1()
}

fn update_population<G: Game>(
    members: &mut [Member<G>],
    tr: &mut Trainer,
    a: &Args,
    explore: f32,
    ent_boost: f32,
    league: &Sender<SeatJob>,
) -> Result<Vec<f32>> {
    let spec = G::spec();
    let (gsize, ns, na) = (
        spec.channels * spec.side * spec.side,
        spec.scalars,
        spec.actions,
    );
    let n = members[0].batch.agents();
    let row = na + 1;
    let weights = tr.acting()?;
    for m in members.iter_mut() {
        m.begin(a.steps);
    }
    let mut act = 0.0;
    let mut sim = 0.0;
    let mut rivals = 0.0;
    for t in 0..a.steps {
        let t0 = Instant::now();
        let (g, s) = observations(members, &tr.dev, tr.net.dtype, (n, gsize, ns))?;
        let probs = policy_forward(tr, &weights, &g, &s, true)?;
        act += t0.elapsed().as_secs_f64();
        let t0 = Instant::now();
        let probs = &probs;
        let rv: Vec<Result<f64>> = std::thread::scope(|sc| {
            let hs: Vec<_> = members
                .iter_mut()
                .enumerate()
                .map(|(k, m)| {
                    let tx = (m.league_size > 0).then_some(league);
                    sc.spawn(move || {
                        m.act_and_step(t, &probs[k * n * row..(k + 1) * n * row], explore, tx)
                    })
                })
                .collect();
            hs.into_iter().map(|h| h.join().unwrap()).collect()
        });
        let mut worst = 0.0f64;
        for r in rv {
            worst = worst.max(r?);
        }
        let wall = t0.elapsed().as_secs_f64();
        rivals += worst;
        sim += wall - worst;
    }
    let t0 = Instant::now();
    let (g, s) = observations(members, &tr.dev, tr.net.dtype, (n, gsize, ns))?;
    let next_v = policy_forward(tr, &weights, &g, &s, false)?;
    act += t0.elapsed().as_secs_f64();
    let steps = a.steps;
    let next_v = &next_v;
    let dev = &tr.dev;
    let dt = tr.net.dtype;
    let rolls: Vec<Result<Rollout>> = std::thread::scope(|sc| {
        let hs: Vec<_> = members
            .iter_mut()
            .enumerate()
            .map(|(k, m)| sc.spawn(move || m.finish(&next_v[k * n..(k + 1) * n], steps, dev, dt)))
            .collect();
        hs.into_iter().map(|h| h.join().unwrap()).collect()
    });
    let rolls = rolls.into_iter().collect::<Result<Vec<_>>>()?;
    let knobs: Vec<Knobs> = members
        .iter()
        .map(|m| Knobs {
            lr: m.lr,
            ent: m.ent * ent_boost,
            pg: m.pg_scale,
        })
        .collect();
    let t0 = Instant::now();
    let mut rngs: Vec<Rng> = members.iter().map(|m| Rng(m.rng.0)).collect();
    let total = a.steps * n;
    let stats = tr.learn(rolls, &knobs, a.minibatch, &mut |k| {
        let rng = &mut rngs[k];
        let mut order: Vec<u32> = (0..total as u32).collect();
        for i in (1..order.len()).rev() {
            order.swap(i, rng.below(i + 1));
        }
        order
    })?;
    let learn = t0.elapsed().as_secs_f64();
    for (m, r) in members.iter_mut().zip(rngs) {
        m.rng = r;
    }
    let t = &mut members[0].timing;
    t.0 += sim;
    t.2 += act;
    t.3 += learn;
    t.4 += rivals;
    Ok(stats.into_iter().map(|x| x.0).collect())
}

struct View {
    vm: VarMap,
    net: net::Net,
}

fn all_rows(trainers: &[Trainer]) -> Result<Vec<f32>> {
    let mut rows = vec![];
    for t in trainers {
        rows.extend(t.rows()?);
    }
    Ok(rows)
}

fn sync_views(views: &[View], trainers: &[Trainer], dev: &Device) -> Result<()> {
    let tr = &trainers[0];
    let total = tr.layout().total;
    let rows = all_rows(trainers)?;
    for (k, v) in views.iter().enumerate() {
        tr.layout()
            .unpack(&rows[k * total..(k + 1) * total], &v.vm, dev)?;
    }
    Ok(())
}

fn population<G: Game>(a: &Args) -> Result<()> {
    let dev = devices::open(&a.device, false)?;
    let mut rng = Rng(0x9e3779b97f4a7c15);
    let defaults: Vec<f32> = G::genes()
        .iter()
        .map(|g| a.gene_set.iter().find(|s| s.0 == g.0).map_or(g.1, |s| s.1))
        .collect();
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
    let mut views: Vec<View> = vec![];
    for _ in 0..a.pop {
        let (mut vm, mut net) = net::build(G::spec(), &dev)?;
        if a.bf16 {
            net.dtype = candle_core::DType::BF16;
        }
        if a.resume.ends_with(".kgb") {
            let p = kg_core::brain::conv::Policy::from_bytes(&std::fs::read(&a.resume).unwrap())
                .expect("valid kgb");
            net::import_actor(&vm, &p, &dev)?;
        } else if !a.resume.is_empty() {
            vm.load(&a.resume)?;
        }
        views.push(View { vm, net });
    }
    let layout = popnet::Layout::of(&views[0].vm);
    let mut rows = vec![];
    for v in &views {
        rows.extend(layout.pack(&v.vm)?);
    }
    let dtype = if a.bf16 {
        candle_core::DType::BF16
    } else {
        candle_core::DType::F32
    };
    let groups = a.groups.clamp(1, a.pop);
    let per_group = a.pop.div_ceil(groups);
    let total = layout.total;
    let mut trainers: Vec<Trainer> = vec![];
    for chunk in rows.chunks(per_group * total) {
        let gdev = devices::open(&a.device, true)?;
        let lay = popnet::Layout::of(&views[0].vm);
        trainers.push(Trainer::new(
            PopNet::new(G::spec(), lay, dtype, &gdev)?,
            chunk.to_vec(),
            chunk.len() / total,
            gdev,
        )?);
    }
    G::set_eval_threads((a.threads / (2 * a.pop.max(1))).max(1));
    let params = views[0].net.export()?.param_count();
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
            let dev = devices::open(&a.device, true)?;
            let nets = league_nets::<G>(&league_weights, a.bf16, &dev)?;
            servers.push((dev, nets));
        }
        sync_views(&views, &trainers, &dev)?;
        save(&views[leader].net.export()?, &live);
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
        let ent_boost = if round <= a.critic_rounds { 0.0 } else { 1.0 };
        for m in members.iter_mut() {
            m.pg_scale = if round <= a.critic_rounds { 0.0 } else { 1.0 };
        }
        println!("  explore {explore:.3}");
        let train_from = Instant::now();
        start = Instant::now();
        steps = 0;
        for u in 1..=a.gen_updates {
            let mut line = format!("round {round} upd {u:3}");
            let pop = members.len();
            let per_server = pop.div_ceil(a.league_servers.max(1));
            let results: Result<Vec<f32>> = std::thread::scope(|sc| {
                let (jobs, inbox) = std::sync::mpsc::channel::<SeatJob>();
                let inbox = std::sync::Arc::new(std::sync::Mutex::new(inbox));
                let running: Vec<_> = servers
                    .iter()
                    .filter_map(|(dev, nets)| nets.as_ref().map(|n| (dev, n)))
                    .map(|(dev, nets)| {
                        let inbox = inbox.clone();
                        sc.spawn(move || league_server::<G>(&inbox, nets, dev, per_server))
                    })
                    .collect();
                let hs: Vec<_> = members
                    .chunks_mut(per_group)
                    .zip(trainers.iter_mut())
                    .map(|(ms, tr)| {
                        let tx = jobs.clone();
                        sc.spawn(move || update_population(ms, tr, a, explore, ent_boost, &tx))
                    })
                    .collect();
                let mut out = Ok(vec![]);
                for h in hs {
                    match (h.join().unwrap(), &mut out) {
                        (Ok(v), Ok(all)) => all.extend(v),
                        (Err(e), o) => *o = Err(e),
                        _ => {}
                    }
                }
                drop(jobs);
                for s in running {
                    s.join().unwrap().unwrap();
                }
                out
            });
            let results = results?;
            for (m, ent) in members.iter_mut().zip(results) {
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
                sync_views(&views, &trainers, &dev)?;
                save(&views[leader].net.export()?, &live);
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
        sync_views(&views, &trainers, &dev)?;
        let mut policies = vec![];
        for (m, v) in members.iter().zip(&views) {
            let policy = v.net.export()?;
            let err = parity(&v.net, &policy, m.batch.envs[0].obs(0), &dev)?;
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
        let mut defending = match league.last() {
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
        let mut challenge = members[leader].score;
        if a.holdout {
            G::set_eval_offset(1_000_000 + round as u64 * 10_000);
            let mut best_fresh = f32::MIN;
            let mut picked = leader;
            for &k in order.iter().take(a.holdout_top.max(1)) {
                let p = views[k].net.export()?;
                let bots = G::eval(&p, &[], seeds).0;
                let vs = (!league.is_empty()).then(|| G::eval(&p, &league, seeds).0);
                let s = (bots + vs.unwrap_or(bots)) / 2.0;
                if s > best_fresh {
                    best_fresh = s;
                    picked = k;
                }
            }
            if let Some(p) = league.last() {
                let bots = G::eval(p, &[], seeds).0;
                let vs = G::eval(p, &league, seeds).0;
                defending = (bots + vs) / 2.0;
            }
            println!("  held-out seeds: m{picked} {best_fresh:.2} vs champion {defending:.2}");
            leader = picked;
            challenge = best_fresh;
            G::set_eval_offset(0);
        }
        let best = &members[leader];
        let champion = defending;
        if challenge > champion + a.min_gain {
            stale = 0;
            let policy = views[leader].net.export()?;
            save(&policy, &a.out);
            views[leader].vm.save(format!("{}.safetensors", a.out))?;
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
            let from = {
                let t = &trainers[d.id / per_group];
                let rows = t.rows()?;
                let k = d.id % per_group;
                rows[k * total..(k + 1) * total].to_vec()
            };
            trainers[l.id / per_group].set_row(l.id % per_group, &from)?;
            l.copy_from(d);
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

fn clone_demos<G: Game>(a: &Args, out: &str) -> Result<()> {
    let spec = G::spec();
    let (gsize, ns) = (spec.channels * spec.side * spec.side, spec.scalars);
    let row = gsize + ns * 4 + 1;
    let bytes = std::fs::read(&a.demos).unwrap();
    let n = bytes.len() / row;
    let dev = devices::open(&a.device, false)?;
    let (vm, net) = net::build(spec, &dev)?;
    let mut opt = adam(&vm, 1e-3)?;
    let mut rng = Rng(0x2545f4914f6cdd1d);
    let mut counts = vec![0usize; G::spec().actions];
    for i in 0..n {
        counts[bytes[i * row + row - 1] as usize] += 1;
    }
    println!("cloning {n} demos, action counts {counts:?}");
    let mb = 512;
    for epoch in 0..a.bc_epochs {
        let mut order: Vec<usize> = (0..n).collect();
        for i in (1..n).rev() {
            order.swap(i, rng.below(i + 1));
        }
        let (mut loss_sum, mut hits, mut seen) = (0.0f32, 0usize, 0usize);
        for chunk in order.chunks(mb) {
            let mut g = Vec::with_capacity(chunk.len() * gsize);
            let mut sc = Vec::with_capacity(chunk.len() * ns);
            let mut act = Vec::with_capacity(chunk.len());
            for &i in chunk {
                let r = &bytes[i * row..(i + 1) * row];
                g.extend(r[..gsize].iter().map(|v| *v as f32 / 255.0));
                for k in 0..ns {
                    let o = gsize + k * 4;
                    sc.push(f32::from_le_bytes([r[o], r[o + 1], r[o + 2], r[o + 3]]));
                }
                act.push(r[row - 1] as u32);
            }
            let gt = Tensor::from_slice(&g, (chunk.len(), gsize), &dev)?;
            let st = Tensor::from_slice(&sc, (chunk.len(), ns), &dev)?;
            let at = Tensor::from_slice(&act, chunk.len(), &dev)?;
            let (logits, _) = net.forward(&gt, &st)?;
            let lsm = candle_nn::ops::log_softmax(&logits, D::Minus1)?;
            let nll = lsm.gather(&at.unsqueeze(1)?, 1)?.squeeze(1)?.neg()?.mean_all()?;
            opt.backward_step(&nll)?;
            loss_sum += nll.to_scalar::<f32>()? * chunk.len() as f32;
            let pred: Vec<u32> = logits.argmax(D::Minus1)?.to_vec1()?;
            hits += pred.iter().zip(&act).filter(|(p, q)| p == q).count();
            seen += chunk.len();
        }
        println!(
            "clone epoch {epoch}: loss {:.4} accuracy {:.1}%",
            loss_sum / seen as f32,
            hits as f32 * 100.0 / seen as f32
        );
    }
    vm.save(out)?;
    Ok(())
}

fn main() -> Result<()> {
    let a = args();
    kg_core::save::use_memory_only();
    devices::setup();
    let mut a = a;
    if a.bf16 && !devices::bf16_works(&a.device) {
        println!("bf16 does not work on {}, falling back to f32", a.device);
        a.bf16 = false;
    }
    if !a.demos.is_empty() && a.resume.is_empty() {
        let start = format!("{}.bc.safetensors", a.out);
        match a.game.as_str() {
            "clash" => clone_demos::<kg_core::games::clash::rl::Arena>(&a, &start)?,
            "paper" => clone_demos::<kg_core::games::paper::rl::Arena>(&a, &start)?,
            g => panic!("unknown game {g}"),
        }
        a.resume = start;
    }
    let a = a;
    match a.game.as_str() {
        "clash" => population::<kg_core::games::clash::rl::Arena>(&a),
        "paper" => population::<kg_core::games::paper::rl::Arena>(&a),
        g => panic!("unknown game {g}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use candle_core::DType;

    fn job(seed: f32, rows: &[usize]) -> SeatJob {
        let (reply, _) = std::sync::mpsc::channel();
        SeatJob {
            obs: rows
                .iter()
                .map(|n| {
                    (
                        (0..n * 6).map(|i| seed + i as f32 * 0.37).collect(),
                        (0..n * 2).map(|i| seed - i as f32 * 0.11).collect(),
                        *n,
                    )
                })
                .collect(),
            reply,
        }
    }

    fn three_pass(batch: &[SeatJob], l: usize, widest: usize, width: usize, dt: DType, pick: usize) -> Tensor {
        let mut flat = vec![0.0f32; l * widest * width];
        for p in 0..l {
            let mut at = p * widest * width;
            for j in batch {
                let src = if pick == 0 { &j.obs[p].0 } else { &j.obs[p].1 };
                flat[at..at + src.len()].copy_from_slice(src);
                at += src.len();
            }
        }
        upload(&flat, (l * widest, width), &Device::Cpu, dt).unwrap().reshape((l, widest, width)).unwrap()
    }

    #[test]
    fn league_gather_matches_the_three_pass_buffer() {
        let batch = vec![job(0.5, &[3, 1]), job(-1.25, &[2, 4])];
        for dt in [DType::BF16, DType::F32] {
            let g = gather(&batch, 2, 6, 6, &Device::Cpu, dt, |o| &o.0).unwrap();
            let s = gather(&batch, 2, 6, 2, &Device::Cpu, dt, |o| &o.1).unwrap();
            let wg = three_pass(&batch, 2, 6, 6, dt, 0);
            let ws = three_pass(&batch, 2, 6, 2, dt, 1);
            assert_eq!(g.dtype(), dt);
            let bits = |t: Tensor| t.to_dtype(DType::F32).unwrap().flatten_all().unwrap().to_vec1::<f32>().unwrap().iter().map(|x| x.to_bits()).collect::<Vec<_>>();
            assert_eq!(bits(g), bits(wg));
            assert_eq!(bits(s), bits(ws));
        }
    }
}
