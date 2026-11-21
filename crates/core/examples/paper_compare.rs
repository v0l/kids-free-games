use kg_core::brain::conv::Policy;
use kg_core::brain::Effort;
use kg_core::games::paper::arena::rl_effort;
use kg_core::games::paper::rl::{play_seed, skill_of, Pilot, Report};

fn load(path: &str) -> Policy {
    Policy::from_bytes(&std::fs::read(path).expect("read")).expect("valid kgb")
}

fn run(p: &Policy, games: u64, in_game: bool) -> Vec<Report> {
    let next = std::sync::atomic::AtomicU64::new(0);
    let mut all: Vec<(u64, Report)> = std::thread::scope(|s| {
        let hs: Vec<_> = (0..40)
            .map(|_| {
                let next = &next;
                s.spawn(move || {
                    let mut out = vec![];
                    loop {
                        let seed = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        if seed >= games {
                            return out;
                        }
                        let seed = seed + 900_000;
                        let skill = skill_of(seed);
                        let e = if in_game { rl_effort(skill) } else { 1.0 };
                        out.push((seed, play_seed(&Pilot::Policy(p, Effort::new(e)), seed, skill)));
                    }
                })
            })
            .collect();
        hs.into_iter().flat_map(|h| h.join().unwrap()).collect()
    });
    all.sort_by_key(|x| x.0);
    all.into_iter().map(|x| x.1).collect()
}

fn ci(a: &[f32], b: &[f32]) -> (f32, f32, f32) {
    let n = a.len() as f32;
    let d: Vec<f32> = a.iter().zip(b).map(|(x, y)| x - y).collect();
    let m = d.iter().sum::<f32>() / n;
    let v = d.iter().map(|x| (x - m).powi(2)).sum::<f32>() / (n - 1.0);
    (a.iter().sum::<f32>() / n, b.iter().sum::<f32>() / n, 1.96 * (v / n).sqrt())
}

fn main() {
    kg_core::save::use_memory_only();
    let mut args = std::env::args().skip(1);
    let new = load(&args.next().expect("new.kgb"));
    let old = load(&args.next().expect("old.kgb"));
    let games: u64 = args.next().map_or(1000, |g| g.parse().unwrap());
    for (label, in_game) in [("effort sqrt(skill)", true), ("effort 1.0", false)] {
        let (n, o) = (run(&new, games, in_game), run(&old, games, in_game));
        println!("{label}, {games} paired games (new, old, diff +/- 95% CI)");
        let fields: [(&str, fn(&Report) -> f32); 4] = [
            ("outlived", |r| r.outlived),
            ("peak", |r| r.peak),
            ("wins", |r| r.wins),
            ("secs", |r| r.secs),
        ];
        for (name, f) in fields {
            let a: Vec<f32> = n.iter().map(f).collect();
            let b: Vec<f32> = o.iter().map(f).collect();
            let (ma, mb, h) = ci(&a, &b);
            println!("  {name:9} {ma:.4} {mb:.4} diff {:+.4} +/- {h:.4}", ma - mb);
        }
    }
}
