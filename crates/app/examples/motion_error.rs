fn source(motion: &str) -> String {
    let dir = std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets/models"));
    let text = std::fs::read_to_string(dir.join("hero.gcad")).unwrap();
    kids_games::gcad::expand_includes(&text, &mut |_| {
        std::fs::read_to_string(motion).map_err(|e| kids_games::gcad::GcadError(e.to_string()))
    })
    .unwrap()
}

fn main() {
    let mut args = std::env::args().skip(1);
    let (a, b) = (args.next().unwrap(), args.next().unwrap());
    let ma = kids_games::gcad::build(&source(&a), 1.0, None).unwrap();
    let mb = kids_games::gcad::build(&source(&b), 1.0, None).unwrap();
    let (ba, bb) = (&ma.bodies[0], &mb.bodies[0]);
    let mut worst_all: f32 = 0.0;
    for clip in &ba.clips {
        let (ca, cb) = (ba.clip_index(&clip.name), bb.clip_index(&clip.name));
        assert!(cb.is_some(), "{} missing", clip.name);
        let mut worst: f32 = 0.0;
        let mut sum = 0.0;
        let mut n = 0;
        let steps = (clip.duration * 120.0) as usize;
        for s in 0..=steps {
            let t = clip.duration as f64 * s as f64 / steps as f64;
            let pa = ba.skinned_positions(&ba.bone_matrices(ca, t));
            let pb = bb.skinned_positions(&bb.bone_matrices(cb, t));
            for (p, q) in pa.iter().zip(&pb) {
                let d =
                    ((p[0] - q[0]).powi(2) + (p[1] - q[1]).powi(2) + (p[2] - q[2]).powi(2)).sqrt();
                worst = worst.max(d);
                sum += d as f64;
                n += 1;
            }
        }
        worst_all = worst_all.max(worst);
        println!(
            "{:6} worst {:.2} cm, mean {:.3} cm over {} samples",
            clip.name,
            worst,
            sum / n as f64,
            steps + 1
        );
    }
    println!("worst vertex error {worst_all:.2} cm on a 134 cm hero");
}
