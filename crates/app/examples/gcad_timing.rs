fn main() {
    let dir = std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets/models"));
    let detail: f64 = std::env::args()
        .nth(1)
        .and_then(|d| d.parse().ok())
        .unwrap_or(1.0);
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .unwrap()
        .filter_map(|e| e.ok()?.file_name().into_string().ok())
        .filter_map(|f| f.strip_suffix(".gcad").map(str::to_string))
        .filter(|n| n == "hero" || n.starts_with("prop_"))
        .collect();
    names.sort();
    let mut total = 0.0;
    for name in names {
        let text = std::fs::read_to_string(dir.join(format!("{name}.gcad"))).unwrap();
        let source = kids_games::gcad::expand_includes(&text, &mut |inc| {
            std::fs::read_to_string(dir.join(inc))
                .map_err(|e| kids_games::gcad::GcadError(e.to_string()))
        })
        .unwrap();
        let mut best = f64::MAX;
        let mut triangles = 0;
        for _ in 0..3 {
            let t = std::time::Instant::now();
            let options = gcad_mesh::Options {
                detail,
                ..Default::default()
            };
            let model = gcad_mesh::build_with(&source, &options).unwrap();
            best = best.min(t.elapsed().as_secs_f64() * 1000.0);
            triangles = model.bodies.iter().map(|b| b.indices.len() / 3).sum();
        }
        total += best;
        println!(
            "{name}: {} bytes, {triangles} triangles, {best:.1} ms",
            source.len()
        );
    }
    println!("total {total:.0} ms at detail {detail}");
}
