use bevy::animation::graph::AnimationGraphHandle;
use bevy::app::{AppExit, ScheduleRunnerPlugin};
use bevy::camera::RenderTarget;
use bevy::gltf::GltfAssetLabel;
use bevy::prelude::*;
use bevy::render::render_resource::TextureFormat;
use bevy::render::view::screenshot::{Screenshot, ScreenshotCaptured};
use bevy::window::ExitCondition;
use bevy::winit::WinitPlugin;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

const SETTLE_FRAMES: u32 = 4;
const WARMUP_FRAMES: u32 = 20;

struct Args {
    model: String,
    assets: PathBuf,
    out: PathBuf,
    size: u32,
    frames: usize,
    views: Vec<(String, f32, f32)>,
    clips: Vec<String>,
    height: f32,
}

fn view(name: &str) -> Option<(String, f32, f32)> {
    let (yaw, elev) = match name {
        "front" => (0.0, 10.0),
        "side" => (90.0, 10.0),
        "back" => (180.0, 10.0),
        "three" => (35.0, 18.0),
        "game" => (180.0, 26.0),
        _ => return None,
    };
    Some((name.to_string(), yaw, elev))
}

fn parse() -> Args {
    let mut it = std::env::args().skip(1);
    let mut a = Args {
        model: String::new(),
        assets: PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets")),
        out: PathBuf::from("/tmp/model_sheet.png"),
        size: 256,
        frames: 8,
        views: vec![],
        clips: vec![],
        height: 1.0,
    };
    while let Some(arg) = it.next() {
        let mut val = || it.next().expect("flag needs a value");
        match arg.as_str() {
            "--out" => a.out = PathBuf::from(val()),
            "--assets" => a.assets = PathBuf::from(val()),
            "--size" => a.size = val().parse().expect("size"),
            "--frames" => a.frames = val().parse().expect("frames"),
            "--height" => a.height = val().parse().expect("height"),
            "--views" => {
                a.views = val()
                    .split(',')
                    .map(|v| view(v).unwrap_or_else(|| panic!("unknown view {v}")))
                    .collect()
            }
            "--clips" => a.clips = val().split(',').map(String::from).collect(),
            m if !m.starts_with("--") => a.model = m.to_string(),
            other => panic!("unknown flag {other}"),
        }
    }
    if a.model.is_empty() {
        eprintln!(
            "usage: model_sheet <models/x.glb> [--out sheet.png] [--size 256] [--frames 8]\n  [--views front,side,back,three,game] [--clips run,jump] [--height 1.0] [--assets dir]\nRows are clip x view (or one turntable row per model without clips), columns are frames."
        );
        std::process::exit(2);
    }
    if a.views.is_empty() {
        a.views = ["front", "side", "back"]
            .iter()
            .map(|v| view(v).unwrap())
            .collect();
    }
    a
}

fn animation_names(path: &std::path::Path) -> Vec<String> {
    let Ok(bytes) = std::fs::read(path) else {
        return vec![];
    };
    if bytes.len() < 20 || &bytes[..4] != b"glTF" {
        return vec![];
    }
    let n = u32::from_le_bytes(bytes[12..16].try_into().unwrap()) as usize;
    let Ok(json) = serde_json::from_slice::<serde_json::Value>(&bytes[20..20 + n]) else {
        return vec![];
    };
    json["animations"]
        .as_array()
        .map(|a| {
            a.iter()
                .enumerate()
                .map(|(i, v)| v["name"].as_str().map_or(format!("clip{i}"), String::from))
                .collect()
        })
        .unwrap_or_default()
}

#[derive(Clone)]
struct Job {
    clip: Option<usize>,
    at: f32,
    yaw: f32,
    elev: f32,
    row: usize,
    col: usize,
}

#[derive(Resource)]
struct Plan {
    jobs: Vec<Job>,
    next: usize,
    wait: u32,
    warm: u32,
    height: f32,
}

#[derive(Resource)]
struct Rig {
    graph: Handle<AnimationGraph>,
    nodes: Vec<AnimationNodeIndex>,
    clips: Vec<Handle<AnimationClip>>,
}

#[derive(Resource)]
struct Target(Handle<Image>);

#[derive(Resource, Clone)]
struct Shots {
    done: Arc<AtomicUsize>,
    images: Arc<Mutex<Vec<(usize, usize, image::RgbaImage)>>>,
}

#[derive(Resource)]
struct Output {
    path: PathBuf,
    size: u32,
    rows: usize,
    cols: usize,
}

fn main() {
    let args = parse();
    let model_path = args.assets.join(&args.model);
    assert!(model_path.exists(), "{} not found", model_path.display());
    let all = animation_names(&model_path);
    let picked: Vec<usize> = if args.clips.is_empty() {
        (0..all.len()).collect()
    } else {
        args.clips
            .iter()
            .map(|c| {
                all.iter()
                    .position(|n| n == c)
                    .unwrap_or_else(|| panic!("no clip {c}; has {all:?}"))
            })
            .collect()
    };

    let mut jobs = vec![];
    let mut names = vec![];
    if picked.is_empty() {
        for (v, (name, yaw0, elev)) in args.views.iter().enumerate() {
            names.push(name.clone());
            for c in 0..args.frames {
                jobs.push(Job {
                    clip: None,
                    at: 0.0,
                    yaw: yaw0 + 360.0 * c as f32 / args.frames as f32,
                    elev: *elev,
                    row: v,
                    col: c,
                });
            }
        }
    } else {
        for (ci, clip) in picked.iter().enumerate() {
            for (vi, (name, yaw, elev)) in args.views.iter().enumerate() {
                let row = ci * args.views.len() + vi;
                names.push(format!("{} {}", all[*clip], name));
                for c in 0..args.frames {
                    jobs.push(Job {
                        clip: Some(ci),
                        at: c as f32 / args.frames as f32,
                        yaw: *yaw,
                        elev: *elev,
                        row,
                        col: c,
                    });
                }
            }
        }
    }
    let rows = names.len();
    for (i, n) in names.iter().enumerate() {
        println!("row {i}: {n}");
    }

    let shots = Shots {
        done: Arc::new(AtomicUsize::new(0)),
        images: Arc::new(Mutex::new(vec![])),
    };
    let asset_root = args.assets.canonicalize().unwrap();
    let size = args.size;
    let model = args.model.clone();
    let picked2 = picked.clone();

    App::new()
        .add_plugins(
            DefaultPlugins
                .set(AssetPlugin {
                    file_path: asset_root.to_string_lossy().into_owned(),
                    ..default()
                })
                .set(WindowPlugin {
                    primary_window: None,
                    exit_condition: ExitCondition::DontExit,
                    ..default()
                })
                .disable::<WinitPlugin>(),
        )
        .add_plugins(ScheduleRunnerPlugin::run_loop(Duration::from_millis(5)))
        .insert_resource(ClearColor(Color::srgb(0.78, 0.78, 0.8)))
        .insert_resource(GlobalAmbientLight {
            color: Color::WHITE,
            brightness: 700.0,
            ..default()
        })
        .insert_resource(Plan {
            jobs,
            next: 0,
            wait: 0,
            warm: 0,
            height: args.height,
        })
        .insert_resource(Output {
            path: args.out.clone(),
            size,
            rows,
            cols: args.frames,
        })
        .insert_resource(shots)
        .add_systems(
            Startup,
            move |mut commands: Commands,
                  assets: Res<AssetServer>,
                  mut graphs: ResMut<Assets<AnimationGraph>>,
                  mut images: ResMut<Assets<Image>>| {
                let target = images.add(Image::new_target_texture(
                    size,
                    size,
                    TextureFormat::Rgba8Unorm,
                    Some(TextureFormat::Rgba8UnormSrgb),
                ));
                commands.spawn((
                    Camera3d::default(),
                    bevy::core_pipeline::tonemapping::Tonemapping::None,
                    Camera {
                        clear_color: ClearColorConfig::Default,
                        ..default()
                    },
                    RenderTarget::Image(target.clone().into()),
                    Transform::default(),
                ));
                commands.spawn((
                    DirectionalLight {
                        illuminance: 9000.0,
                        shadow_maps_enabled: false,
                        ..default()
                    },
                    Transform::from_xyz(2.0, 4.0, 3.0).looking_at(Vec3::ZERO, Vec3::Y),
                ));
                commands.spawn(WorldAssetRoot(
                    assets.load(GltfAssetLabel::Scene(0).from_asset(model.clone())),
                ));
                let clips: Vec<Handle<AnimationClip>> = picked2
                    .iter()
                    .map(|i| assets.load(GltfAssetLabel::Animation(*i).from_asset(model.clone())))
                    .collect();
                let (graph, nodes) = AnimationGraph::from_clips(clips.clone());
                commands.insert_resource(Rig {
                    graph: graphs.add(graph),
                    nodes,
                    clips,
                });
                commands.insert_resource(Target(target));
            },
        )
        .add_systems(Update, drive)
        .run();
}

#[allow(clippy::too_many_arguments)]
fn drive(
    mut commands: Commands,
    mut plan: ResMut<Plan>,
    rig: Res<Rig>,
    clips: Res<Assets<AnimationClip>>,
    target: Res<Target>,
    shots: Res<Shots>,
    out: Res<Output>,
    fresh: Query<Entity, (With<AnimationPlayer>, Without<AnimationGraphHandle>)>,
    mut players: Query<&mut AnimationPlayer>,
    meshes: Query<(), With<Mesh3d>>,
    mut cam: Query<&mut Transform, With<Camera3d>>,
    mut exit: MessageWriter<AppExit>,
) {
    for e in &fresh {
        commands
            .entity(e)
            .insert(AnimationGraphHandle(rig.graph.clone()));
    }
    let wants_anim = !rig.clips.is_empty();
    let ready = !meshes.is_empty()
        && (!wants_anim
            || (players.iter().len() > 0
                && rig.clips.iter().all(|c| clips.get(c).is_some())
                && fresh.is_empty()));
    if !ready {
        return;
    }
    if plan.warm < WARMUP_FRAMES {
        plan.warm += 1;
        return;
    }

    let done = shots.done.load(Ordering::SeqCst);
    if plan.next >= plan.jobs.len() {
        if done >= plan.jobs.len() {
            write_sheet(&shots, &out);
            exit.write(AppExit::Success);
        }
        return;
    }
    if done < plan.next {
        return;
    }

    let job = plan.jobs[plan.next].clone();
    if plan.wait == 0 {
        if let Some(c) = job.clip {
            let dur = clips
                .get(&rig.clips[c])
                .map_or(1.0, |c| c.duration().max(0.001));
            for mut p in &mut players {
                p.stop_all();
                p.play(rig.nodes[c]).seek_to(job.at * dur).pause();
            }
        }
        let centre = Vec3::new(0.0, 0.5 * plan.height, 0.0);
        let dist = 1.9 * plan.height;
        let (yaw, elev) = (job.yaw.to_radians(), job.elev.to_radians());
        let eye = centre
            + Vec3::new(
                yaw.sin() * elev.cos(),
                elev.sin(),
                yaw.cos() * elev.cos(),
            ) * dist;
        for mut t in &mut cam {
            *t = Transform::from_translation(eye).looking_at(centre, Vec3::Y);
        }
    }
    plan.wait += 1;
    if plan.wait == SETTLE_FRAMES {
        let sink = shots.clone();
        let (row, col) = (job.row, job.col);
        commands.spawn(Screenshot::image(target.0.clone())).observe(
            move |cap: On<ScreenshotCaptured>| {
                if let Ok(img) = cap.image.clone().try_into_dynamic() {
                    sink.images
                        .lock()
                        .unwrap()
                        .push((row, col, img.to_rgba8()));
                }
                sink.done.fetch_add(1, Ordering::SeqCst);
            },
        );
        plan.next += 1;
        plan.wait = 0;
    }
}

fn write_sheet(shots: &Shots, out: &Output) {
    let s = out.size;
    let mut sheet = image::RgbaImage::new(s * out.cols as u32, s * out.rows as u32);
    for (row, col, img) in shots.images.lock().unwrap().iter() {
        image::imageops::replace(&mut sheet, img, (*col as u32 * s) as i64, (*row as u32 * s) as i64);
    }
    if let Some(dir) = out.path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    sheet.save(&out.path).expect("write sheet");
    println!("{}", out.path.display());
}
