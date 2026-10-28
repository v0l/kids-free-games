use kg_core::games::paper::world::{spread_spots, World};
use kg_core::rng as rand;

fn run(brains: bool, seeds: u64) {
    let (mut home, mut out, mut total, mut peak) = (0u64, 0u64, 0u64, 0.0f32);
    for seed in 0..seeds {
        rand::srand(seed + 1);
        let mut w = World::new(1.0);
        w.brains = brains;
        w.effort = Some(1.0);
        for s in spread_spots(10) {
            w.add_player(s, rand::gen_range(-3.1, 3.1), true);
        }
        for _ in 0..60 * 90 {
            w.advance(1.0 / 60.0);
            w.respawn_bots(1.0 / 60.0);
            w.deaths.clear();
            w.claims.clear();
            for (id, p) in w.players.iter().enumerate() {
                if !p.alive {
                    continue;
                }
                total += 1;
                if p.out() {
                    out += 1;
                } else if w.owner_at(p.pos) == Some(id as u8) {
                    home += 1;
                }
            }
        }
        peak += w.players.iter().map(|p| p.peak).sum::<f32>() / w.players.len() as f32;
    }
    println!(
        "{}: inside own land {:.0}%, out drawing {:.0}%, mean peak land {:.2}%",
        if brains { "brain" } else { "old bot" },
        home as f32 / total as f32 * 100.0,
        out as f32 / total as f32 * 100.0,
        peak / seeds as f32 * 100.0
    );
}

fn big(brains: bool, seeds: u64) {
    use kg_core::games::paper::{cell_center, idx, N, SIZE};
    let (mut home, mut out, mut total, mut trips) = (0u64, 0u64, 0u64, Vec::new());
    for seed in 0..seeds {
        rand::srand(seed + 1);
        let mut w = World::new(1.0);
        w.brains = brains;
        w.effort = Some(1.0);
        let spots = spread_spots(10);
        for s in &spots {
            w.add_player(*s, rand::gen_range(-3.1, 3.1), true);
        }
        let c = glam::Vec2::splat(SIZE * 0.5);
        for y in 0..N {
            for x in 0..N {
                let q = cell_center(glam::ivec2(x, y));
                let i = idx(glam::ivec2(x, y));
                if q.distance(c) < SIZE * 0.3 && w.owner[i].is_none() {
                    w.claim(i, Some(0));
                }
            }
        }
        w.players[0].pos = c;
        let mut inside_since: Option<u32> = None;
        for f in 0..60 * 90u32 {
            w.advance(1.0 / 60.0);
            w.respawn_bots(1.0 / 60.0);
            w.deaths.clear();
            w.claims.clear();
            let p = &w.players[0];
            if !p.alive {
                break;
            }
            total += 1;
            if p.out() {
                out += 1;
                if let Some(s) = inside_since.take() {
                    trips.push((f - s) as f32 / 60.0);
                }
            } else if w.owner_at(p.pos) == Some(0) {
                home += 1;
                inside_since.get_or_insert(f);
            }
        }
    }
    trips.sort_by(f32::total_cmp);
    let med = trips.get(trips.len() / 2).copied().unwrap_or(0.0);
    let long: f32 = trips.iter().filter(|t| **t > 3.0).sum();
    let all: f32 = trips.iter().sum();
    println!(
        "   trips over 3 s: {} of {}, {:.0}% of inside time, longest {:.1}s",
        trips.iter().filter(|t| **t > 3.0).count(),
        trips.len(),
        long / all.max(0.01) * 100.0,
        trips.last().copied().unwrap_or(0.0)
    );
    println!(
        "{} owning 28% of the map: inside own land {:.0}%, out drawing {:.0}%, {} crossings, median {:.1}s inside before leaving",
        if brains { "brain" } else { "old bot" },
        home as f32 / total.max(1) as f32 * 100.0,
        out as f32 / total.max(1) as f32 * 100.0,
        trips.len(),
        med
    );
}

fn main() {
    run(false, 6);
    run(true, 6);
    big(false, 6);
    big(true, 6);
}
