fn main() {
    let assets = std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets"));
    if std::env::args().any(|a| a == "--check") {
        let problems = gcad_bevy::bake::check(assets, kids_games::gcad::bake_settings);
        for p in &problems {
            eprintln!("{p}");
        }
        std::process::exit(if problems.is_empty() { 0 } else { 1 });
    }
    let baked = gcad_bevy::bake::bake(assets, kids_games::gcad::bake_settings).unwrap();
    println!(
        "baked {} built {} reused {} removed {} bytes {}",
        baked.index.entries.len(),
        baked.built.len(),
        baked.reused.len(),
        baked.removed,
        baked.bytes
    );
}
