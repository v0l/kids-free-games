fn main() {
    let dir = std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets/models"));
    let detail: f64 = std::env::args()
        .nth(1)
        .and_then(|d| d.parse().ok())
        .unwrap_or(1.0);
    let mut total = 0.0;
    for name in ["hero", "prop_tree"] {
        let text = std::fs::read_to_string(dir.join(format!("{name}.gcad"))).unwrap();
        let source = kids_games::gcad::expand_includes(&text, &mut |inc| {
            std::fs::read_to_string(dir.join(inc))
                .map_err(|e| kids_games::gcad::GcadError(e.to_string()))
        })
        .unwrap();
        let mut best = f64::MAX;
        for _ in 0..3 {
            let t = std::time::Instant::now();
            let options = gcad_mesh::Options {
                detail,
                ..Default::default()
            };
            let model = gcad_mesh::build_with(&source, &options).unwrap();
            best = best.min(t.elapsed().as_secs_f64() * 1000.0);
            std::hint::black_box(model);
        }
        total += best;
        println!("{name}: {} bytes, {best:.0} ms", source.len());
    }
    println!("total {total:.0} ms at detail {detail}");
}
