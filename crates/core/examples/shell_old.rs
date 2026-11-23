use kg_core::games::hotel::layout::{Build, Layout, Solid};
use kg_core::games::hotel::shell::plan;
use std::io::Write;
use std::time::Instant;

fn main() {
    let number: u32 = std::env::args().nth(1).and_then(|a| a.parse().ok()).unwrap_or(3);
    let out = std::env::args().nth(2).unwrap();
    let lay = Layout::new(number);
    let built = |b: Build| b != Build::Next;
    let mut best = std::time::Duration::MAX;
    for _ in 0..50 {
        let t = Instant::now();
        let s = lay.solids(&built);
        let n = s.iter().filter(|(_, k)| *k != Solid::Thing).count();
        std::hint::black_box(n);
        best = best.min(t.elapsed());
    }
    let solids = lay.solids(&built);
    let boxes = solids.iter().filter(|(_, k)| *k != Solid::Thing).count();
    println!("old shell rects {boxes} pieces {} tris {} rects_time {best:?}", boxes * 2, boxes * 24);
    let mut f = std::fs::File::create(out).unwrap();
    for (r, k) in solids.iter().filter(|(_, k)| *k != Solid::Thing) {
        writeln!(f, "{} {} {} {} {}", r.x, r.y, r.w, r.h, matches!(k, Solid::Locked) as u8).unwrap();
    }
    let mut pbest = std::time::Duration::MAX;
    for _ in 0..50 {
        let t = Instant::now();
        std::hint::black_box(plan(&lay, &built));
        pbest = pbest.min(t.elapsed());
    }
    println!("plan time {pbest:?}");
}
