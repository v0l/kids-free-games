use super::{gauss, Brain, Effort, Net};
use crate::rng as rand;

pub trait Env {
    fn episode(&mut self, seed: u64, policy: &mut dyn FnMut(&[Vec<f32>]) -> usize) -> f32;
}

#[derive(Clone, Debug)]
pub struct Config {
    pub hidden: Vec<usize>,
    pub generations: usize,
    pub population: usize,
    pub seeds: usize,
    pub sigma: f32,
    pub rate: f32,
    pub threads: usize,
    pub seed: u64,
    pub log: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            hidden: vec![16],
            generations: 60,
            population: 48,
            seeds: 4,
            sigma: 0.06,
            rate: 0.03,
            threads: std::thread::available_parallelism().map_or(4, |n| n.get()),
            seed: 1,
            log: false,
        }
    }
}

impl Config {
    pub fn from_env() -> Self {
        let get = |k: &str| std::env::var(k).ok().and_then(|v| v.parse::<f64>().ok());
        let mut c = Self {
            log: true,
            ..Self::default()
        };
        if let Some(v) = get("GENS") {
            c.generations = v as usize;
        }
        if let Some(v) = get("POP") {
            c.population = v as usize;
        }
        if let Some(v) = get("SEEDS") {
            c.seeds = v as usize;
        }
        if let Some(v) = get("SIGMA") {
            c.sigma = v as f32;
        }
        if let Some(v) = get("RATE") {
            c.rate = v as f32;
        }
        if let Some(v) = get("THREADS") {
            c.threads = v as usize;
        }
        if let Some(v) = get("SEED") {
            c.seed = v as u64;
        }
        if let Ok(h) = std::env::var("HIDDEN") {
            c.hidden = h.split(',').filter_map(|x| x.parse().ok()).collect();
        }
        c
    }
}

pub fn evaluate<E: Env>(env: &mut E, net: &Net, effort: Effort, seeds: &[u64]) -> f32 {
    let mut total = 0.0;
    for &s in seeds {
        rand::srand(s ^ 0x9e37_79b9_7f4a_7c15);
        let mut brain = Brain::new(net.clone(), effort);
        total += env.episode(s, &mut |opts| brain.choose(opts));
    }
    total / seeds.len().max(1) as f32
}

fn parallel<E, F, T>(
    make: &F,
    threads: usize,
    jobs: Vec<T>,
    run: impl Fn(&mut E, T) -> f32 + Sync,
) -> Vec<f32>
where
    E: Env,
    F: Fn() -> E + Sync,
    T: Send,
{
    let n = jobs.len();
    let threads = threads.clamp(1, n.max(1));
    let mut slots: Vec<Vec<(usize, T)>> = (0..threads).map(|_| vec![]).collect();
    for (k, j) in jobs.into_iter().enumerate() {
        slots[k % threads].push((k, j));
    }
    let mut out = vec![0.0; n];
    std::thread::scope(|s| {
        let handles: Vec<_> = slots
            .into_iter()
            .map(|slot| {
                let run = &run;
                s.spawn(move || {
                    let mut env = make();
                    slot.into_iter()
                        .map(|(k, j)| (k, run(&mut env, j)))
                        .collect::<Vec<_>>()
                })
            })
            .collect();
        for h in handles {
            for (k, v) in h.join().expect("trainer thread panicked") {
                out[k] = v;
            }
        }
    });
    out
}

pub struct Report {
    pub net: Net,
    pub history: Vec<f32>,
    pub by_effort: Vec<(f32, f32)>,
}

pub fn train<E: Env, F: Fn() -> E + Sync>(make: F, features: usize, cfg: &Config) -> Report {
    rand::srand(cfg.seed);
    let mut sizes = vec![features];
    sizes.extend(&cfg.hidden);
    sizes.push(1);
    let mut net = Net::random(&sizes, 1.0);
    let n = net.weights.len();
    let (mut m, mut v) = (vec![0.0f32; n], vec![0.0f32; n]);
    let mut history = vec![];
    let half = (cfg.population / 2).max(1);
    for round in 0..cfg.generations {
        let seeds: Vec<u64> = (0..cfg.seeds)
            .map(|k| cfg.seed * 1_000_003 + round as u64 * 1009 + k as u64)
            .collect();
        let noise: Vec<Vec<f32>> = (0..half)
            .map(|_| (0..n).map(|_| gauss()).collect())
            .collect();
        let mut jobs = vec![];
        for eps in &noise {
            for sign in [1.0f32, -1.0] {
                let mut cand = net.clone();
                for (w, e) in cand.weights.iter_mut().zip(eps) {
                    *w += sign * cfg.sigma * e;
                }
                jobs.push(cand);
            }
        }
        let fit = parallel(&make, cfg.threads, jobs, |env, cand| {
            evaluate(env, &cand, Effort::BEST, &seeds)
        });
        let mut order: Vec<usize> = (0..fit.len()).collect();
        order.sort_by(|a, b| fit[*a].total_cmp(&fit[*b]));
        let mut rank = vec![0.0f32; fit.len()];
        for (r, k) in order.iter().enumerate() {
            rank[*k] = r as f32 / (fit.len() - 1).max(1) as f32 - 0.5;
        }
        let mut grad = vec![0.0f32; n];
        for (i, eps) in noise.iter().enumerate() {
            let d = rank[2 * i] - rank[2 * i + 1];
            for (g, e) in grad.iter_mut().zip(eps) {
                *g += d * e;
            }
        }
        let scale = 1.0 / (half as f32 * cfg.sigma);
        let t = (round + 1) as i32;
        for k in 0..n {
            let g = grad[k] * scale - 0.005 * net.weights[k];
            m[k] = 0.9 * m[k] + 0.1 * g;
            v[k] = 0.999 * v[k] + 0.001 * g * g;
            let mh = m[k] / (1.0 - 0.9f32.powi(t));
            let vh = v[k] / (1.0 - 0.999f32.powi(t));
            net.weights[k] += cfg.rate * mh / (vh.sqrt() + 1e-8);
        }
        let mean = fit.iter().sum::<f32>() / fit.len() as f32;
        history.push(mean);
        if cfg.log {
            let top = fit.iter().copied().fold(f32::MIN, f32::max);
            eprintln!("gen {round:3}  mean {mean:10.3}  best {top:10.3}");
        }
    }
    let by_effort = calibrate(&make, &net, cfg, 16);
    Report {
        net,
        history,
        by_effort,
    }
}

pub fn calibrate<E: Env, F: Fn() -> E + Sync>(
    make: &F,
    net: &Net,
    cfg: &Config,
    runs: usize,
) -> Vec<(f32, f32)> {
    let efforts = [0.0f32, 0.25, 0.5, 0.75, 1.0];
    let seeds: Vec<u64> = (0..runs as u64).map(|k| 7_777_777 + k).collect();
    let jobs: Vec<(f32, u64)> = efforts
        .iter()
        .flat_map(|e| seeds.iter().map(move |s| (*e, *s)))
        .collect();
    let fit = parallel(make, cfg.threads, jobs, |env, (e, s)| {
        evaluate(env, net, Effort::new(e), &[s])
    });
    efforts
        .iter()
        .enumerate()
        .map(|(i, e)| {
            (
                *e,
                fit[i * runs..(i + 1) * runs].iter().sum::<f32>() / runs as f32,
            )
        })
        .collect()
}

pub fn save(report: &Report, path: &str) {
    if let Some(dir) = std::path::Path::new(path).parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    std::fs::write(path, report.net.to_bytes()).expect("write brain");
    eprintln!("saved {path} ({} weights)", report.net.weights.len());
    for (e, f) in &report.by_effort {
        eprintln!("effort {e:.2}: fitness {f:.3}");
    }
}
