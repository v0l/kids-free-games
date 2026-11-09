use crate::games::runner::world::Models;
use bevy::platform::time::Instant;
use bevy::prelude::*;
use bevy::world_serialization::WorldAsset;
use gcad_bevy::prelude::*;
use gcad_bevy::GcadStats;
use kg_core::geom::Color as Col;
use std::collections::HashMap;
use std::sync::Arc;

pub fn settings(scale: Option<f32>) -> GcadSettings {
    GcadSettings {
        height: scale.is_none().then_some(1.0),
        scale: scale.map(f64::from),
        ..GcadSettings::fit_height(1.0)
    }
}

pub fn plugin() -> GcadPlugin {
    GcadPlugin {
        baked_dir: None,
        ..GcadPlugin::default()
    }
}

pub fn game_models() -> impl Iterator<Item = &'static str> {
    [
        crate::games::hotel::world::MODELS,
        crate::games::helix::world::MODELS,
        crate::games::paper::world::MODELS,
        crate::games::ballsort::world::MODELS,
        crate::games::trains::world::MODELS,
    ]
    .into_iter()
    .flatten()
    .copied()
}

pub fn path(name: &str) -> String {
    format!("models/{name}.gcad")
}

#[derive(Clone, Debug)]
pub struct ModelPart {
    pub name: String,
    pub mesh: Handle<Mesh>,
    pub color: Col,
    pub gloss: bool,
}

#[derive(Default)]
pub struct Shelf(HashMap<&'static str, Arc<[ModelPart]>>);

impl Shelf {
    pub fn get(&self, name: &str) -> &[ModelPart] {
        self.0.get(name).map_or(&[], |p| p)
    }
}

#[derive(Resource, Default)]
pub struct Library {
    ready: HashMap<&'static str, Arc<[ModelPart]>>,
    loading: HashMap<&'static str, Handle<WorldAsset>>,
    shelves: HashMap<usize, Arc<Shelf>>,
}

impl Library {
    pub fn start(&mut self, assets: &AssetServer, names: &[&'static str]) {
        for name in names {
            if !self.ready.contains_key(name) && !self.loading.contains_key(name) {
                self.loading
                    .insert(name, assets.load_gcad(path(name), GcadSettings::default()));
            }
        }
    }

    pub fn handle(&self, name: &str) -> Option<&Handle<WorldAsset>> {
        self.loading.get(name)
    }

    pub fn shelf(&mut self, names: &'static [&'static str], src: Sources) -> Option<Arc<Shelf>> {
        let key = names.as_ptr() as usize;
        if let Some(s) = self.shelves.get(&key) {
            return Some(s.clone());
        }
        let mut done = true;
        for name in names {
            if self.ready.contains_key(name) {
                continue;
            }
            match &src.server {
                Some(server) => {
                    self.start(server, &[name]);
                    let h = &self.loading[name];
                    let loaded = server.is_loaded_with_dependencies(h);
                    match (loaded, src.worlds.and_then(|w| w.get(h))) {
                        (true, Some(world)) => {
                            let parts = take_apart(world, src.meshes, src.mats);
                            self.ready.insert(name, parts.into());
                        }
                        _ => done = false,
                    }
                }
                None => match build_now(name, src.meshes) {
                    Some(parts) => {
                        self.ready.insert(name, parts.into());
                    }
                    None => done = false,
                },
            }
        }
        if !done {
            return None;
        }
        let shelf = Arc::new(Shelf(
            names.iter().map(|n| (*n, self.ready[n].clone())).collect(),
        ));
        self.shelves.insert(key, shelf.clone());
        Some(shelf)
    }
}

pub struct Sources<'a> {
    pub server: Option<&'a AssetServer>,
    pub worlds: Option<&'a Assets<WorldAsset>>,
    pub meshes: &'a mut Assets<Mesh>,
    pub mats: &'a Assets<StandardMaterial>,
}

fn plain(mut mesh: Mesh) -> (Mesh, Option<Col>) {
    let first = match mesh.attribute(Mesh::ATTRIBUTE_COLOR) {
        Some(bevy::mesh::VertexAttributeValues::Float32x4(c)) => c.first().copied(),
        _ => None,
    };
    mesh.remove_attribute(Mesh::ATTRIBUTE_COLOR);
    let color = first.map(|[r, g, b, _]| {
        let s = Color::linear_rgb(r, g, b).to_srgba();
        Col::new(s.red, s.green, s.blue, 1.0)
    });
    (mesh, color)
}

fn part_of(
    name: &str,
    mesh: Mesh,
    base: Color,
    roughness: f32,
    meshes: &mut Assets<Mesh>,
) -> ModelPart {
    let (mesh, vertex) = plain(mesh);
    let s = base.to_srgba();
    ModelPart {
        name: name.to_string(),
        mesh: meshes.add(mesh),
        color: vertex.unwrap_or(Col::new(s.red, s.green, s.blue, 1.0)),
        gloss: roughness < 0.99,
    }
}

fn take_apart(
    world: &WorldAsset,
    meshes: &mut Assets<Mesh>,
    mats: &Assets<StandardMaterial>,
) -> Vec<ModelPart> {
    let Some(mut q) = world
        .world
        .try_query::<(&GcadPart, &Mesh3d, &MeshMaterial3d<StandardMaterial>)>()
    else {
        return vec![];
    };
    let found: Vec<(String, Mesh, Color, f32)> = q
        .iter(&world.world)
        .filter(|(p, ..)| p.lod == 0)
        .filter_map(|(p, mesh, mat)| {
            let m = mats.get(&mat.0)?;
            Some((
                p.part.clone(),
                meshes.get(&mesh.0)?.clone(),
                m.base_color,
                m.perceptual_roughness,
            ))
        })
        .collect();
    found
        .into_iter()
        .map(|(name, mesh, base, rough)| part_of(&name, mesh, base, rough, meshes))
        .collect()
}

#[cfg(not(target_arch = "wasm32"))]
pub fn read_source(name: &str) -> Option<String> {
    let text = String::from_utf8(kg_core::assets::read(&path(name))?).ok()?;
    gcad_bevy::expand_includes(&text, &mut |inc| {
        kg_core::assets::read(&format!("models/{inc}"))
            .and_then(|b| String::from_utf8(b).ok())
            .ok_or_else(|| gcad_bevy::GcadError(format!("{inc} missing")))
    })
    .ok()
}

#[cfg(not(target_arch = "wasm32"))]
fn build_now(name: &str, meshes: &mut Assets<Mesh>) -> Option<Vec<ModelPart>> {
    let built = gcad_bevy::build(&read_source(name)?, &GcadSettings::default()).ok()?;
    Some(
        built
            .bodies
            .iter()
            .flat_map(|body| {
                body.parts.iter().map(|part| {
                    let mat = gcad_bevy::material_of(body, part);
                    (
                        part.name.clone(),
                        gcad_bevy::mesh_of(body, part),
                        mat.base_color,
                        mat.perceptual_roughness,
                    )
                })
            })
            .collect::<Vec<_>>()
            .into_iter()
            .map(|(n, mesh, base, rough)| part_of(&n, mesh, base, rough, meshes))
            .collect(),
    )
}

#[cfg(target_arch = "wasm32")]
fn build_now(_name: &str, _meshes: &mut Assets<Mesh>) -> Option<Vec<ModelPart>> {
    None
}

pub struct ModelsPlugin;

impl Plugin for ModelsPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(plugin())
            .init_resource::<Library>()
            .add_systems(Startup, warm_up)
            .add_systems(Update, report_ready.run_if(resource_exists::<Warming>));
    }
}

#[derive(Resource)]
struct Warming {
    started: Instant,
    pending: Vec<(String, Handle<WorldAsset>)>,
}

fn warm_up(
    mut commands: Commands,
    assets: Res<AssetServer>,
    mut graphs: ResMut<Assets<AnimationGraph>>,
    mut library: ResMut<Library>,
) {
    let started = Instant::now();
    let hotel: Vec<&'static str> = game_models().collect();
    library.start(&assets, &hotel);
    let models = Models::load(&assets, &mut graphs);
    let mut pending: Vec<(String, Handle<WorldAsset>)> = hotel
        .iter()
        .filter_map(|n| library.handle(n).map(|h| (n.to_string(), h.clone())))
        .collect();
    pending.extend(
        models
            .names()
            .filter_map(|n| models.scene(n).map(|h| (n.to_string(), h.clone()))),
    );
    commands.insert_resource(Warming { started, pending });
    commands.insert_resource(models);
}

fn report_ready(
    mut commands: Commands,
    mut warming: ResMut<Warming>,
    assets: Res<AssetServer>,
    stats: Res<GcadStats>,
) {
    let started = warming.started;
    warming.pending.retain(|(name, handle)| {
        let ready = assets.is_loaded_with_dependencies(handle);
        let failed = matches!(assets.load_state(handle), bevy::asset::LoadState::Failed(_));
        if ready {
            info!(
                "model {name} ready {:.0} ms after start",
                started.elapsed().as_secs_f64() * 1000.0
            );
        }
        if failed {
            error!("model {name} failed to load");
        }
        !ready && !failed
    });
    if warming.pending.is_empty() {
        info!(
            "every model ready {:.0} ms after start, {} builds, {} from disk cache",
            started.elapsed().as_secs_f64() * 1000.0,
            stats.builds(),
            stats.hits_disk()
        );
        commands.remove_resource::<Warming>();
    }
}

#[cfg(test)]
pub mod testing {
    use super::*;

    pub fn models_dir() -> std::path::PathBuf {
        kg_core::assets::dirs()
            .into_iter()
            .find(|d| d.join("models").is_dir())
            .expect("assets/models")
            .join("models")
    }

    pub fn included(name: &str) -> bool {
        name.ends_with("_base") || name == "motion"
    }

    pub fn shipped() -> Vec<String> {
        let mut out: Vec<String> = std::fs::read_dir(models_dir())
            .unwrap()
            .filter_map(|e| {
                let p = e.ok()?.path();
                if p.extension()? != "gcad" {
                    return None;
                }
                Some(p.file_stem()?.to_string_lossy().into_owned())
            })
            .filter(|n| !included(n))
            .collect();
        out.sort();
        out
    }

    pub fn build(name: &str) -> gcad_bevy::Built {
        let source = read_source(name).unwrap_or_else(|| panic!("models/{name}.gcad missing"));
        gcad_bevy::build(&source, &GcadSettings::default())
            .unwrap_or_else(|e| panic!("{name}: {e}"))
    }

    pub fn part_names(model: &gcad_bevy::Built) -> Vec<String> {
        model
            .bodies
            .iter()
            .flat_map(|b| b.parts.iter().map(|p| p.name.clone()))
            .collect()
    }

    pub fn size(model: &gcad_bevy::Built) -> Vec3 {
        let (lo, hi) = model.bounds.expect("bounds");
        Vec3::from(hi) - Vec3::from(lo)
    }
}

#[cfg(test)]
mod tests {
    use super::testing::*;
    use super::*;

    fn budget(name: &str) -> usize {
        match name {
            "hero" => 14_000,
            "prop_tree" => 12_000,
            n if n.starts_with("hotel_person") || n == "hotel_swimmer" => 3_000,
            n if n.starts_with("hotel_") => 1_500,
            _ => 4_000,
        }
    }

    #[test]
    fn every_shipped_model_builds_mesh_only_inside_its_budget_with_named_parts() {
        let names = shipped();
        assert!(names.len() > 40, "{names:?}");
        for name in &names {
            let model = build(name);
            let tris = model.triangles();
            assert!(
                (1..=budget(name)).contains(&tris),
                "{name}: {tris} triangles, budget {}",
                budget(name)
            );
            for body in &model.bodies {
                assert!(!body.indices.is_empty(), "{name}: empty body {}", body.name);
                for part in &body.parts {
                    assert!(!part.name.is_empty(), "{name}: unnamed part");
                    assert!(part.index_count > 0, "{name}: empty part {}", part.name);
                }
            }
            let s = size(&model);
            assert!(s.min_element() > 0.0, "{name}: flat {s}");
        }
    }

    #[test]
    fn every_hotel_model_is_warmed_and_on_disk() {
        let names = shipped();
        for m in crate::games::hotel::world::MODELS {
            assert!(names.iter().any(|n| n == m), "{m} is not in assets/models");
        }
        for n in names.iter().filter(|n| n.starts_with("hotel_")) {
            assert!(
                crate::games::hotel::world::MODELS.contains(&n.as_str()),
                "{n} ships but the hotel never loads it"
            );
        }
    }

    #[test]
    fn every_ball_sort_model_is_loaded_and_on_disk() {
        let names = shipped();
        for m in crate::games::ballsort::world::MODELS {
            assert!(names.iter().any(|n| n == m), "{m} is not in assets/models");
        }
        for n in names.iter().filter(|n| n.starts_with("ballsort_")) {
            assert!(
                crate::games::ballsort::world::MODELS.contains(&n.as_str()),
                "{n} ships but ball sort never loads it"
            );
        }
    }

    #[test]
    fn every_trains_model_is_warmed_and_on_disk() {
        let names = shipped();
        for m in crate::games::trains::world::MODELS {
            assert!(names.iter().any(|n| n == m), "{m} is not in assets/models");
        }
        for n in names.iter().filter(|n| n.starts_with("trains_")) {
            assert!(
                crate::games::trains::world::MODELS.contains(&n.as_str()),
                "{n} ships but trains never loads it"
            );
        }
    }

    #[test]
    fn the_ball_sort_tube_has_the_parts_the_game_paints_and_the_size_it_had() {
        let tube = build("ballsort_tube");
        let parts = part_names(&tube);
        for want in ["glass", "foot", "rim", "glint"] {
            assert!(parts.iter().any(|p| p == want), "{parts:?}");
        }
        let s = size(&tube);
        let h = kg_core::games::ballsort::TUBE_H;
        assert!((s.y - (h + 0.045)).abs() < 0.02, "{s}");
        assert!(
            (s.x - 1.17).abs() < 0.05 && (s.z - 1.17).abs() < 0.05,
            "{s}"
        );
    }

    #[test]
    fn the_ball_sort_ball_is_one_part_as_wide_as_the_old_sphere() {
        let ball = build("ballsort_ball");
        assert_eq!(part_names(&ball), ["ball"]);
        let s = size(&ball);
        let d = kg_core::games::ballsort::BALL_D;
        assert!((s.x - d).abs() < 0.02 && (s.y - d).abs() < 0.02, "{s}");
    }

    #[test]
    fn every_ball_sort_model_builds_fast() {
        let started = std::time::Instant::now();
        for m in crate::games::ballsort::world::MODELS {
            build(m);
        }
        let ms = started.elapsed().as_secs_f64() * 1000.0;
        assert!(ms < 150.0, "ball sort models took {ms:.0} ms");
    }

    #[test]
    fn every_hotel_model_builds_fast() {
        let started = std::time::Instant::now();
        for m in crate::games::hotel::world::MODELS {
            build(m);
        }
        let ms = started.elapsed().as_secs_f64() * 1000.0;
        assert!(ms < 1500.0, "all hotel models took {ms:.0} ms");
    }

    #[test]
    fn trains_models_keep_their_parts_and_size() {
        let wheel = part_names(&build("trains_wheel"));
        for p in ["rim", "hub", "knob"] {
            assert!(wheel.iter().any(|n| n == p), "wheel has no {p}");
        }
        let engine = part_names(&build("trains_engine"));
        for p in ["boiler", "funnel", "cab", "headlamp"] {
            assert!(engine.iter().any(|n| n == p), "engine has no {p}");
        }
        let carriage = part_names(&build("trains_carriage"));
        for p in ["side_l", "side_r", "end_l", "end_r"] {
            assert!(carriage.iter().any(|n| n == p), "carriage has no {p}");
        }
        let crown = |n: &str| part_names(&build(n)).iter().any(|p| p == "crown");
        assert!(crown("trains_tree_round") && crown("trains_tree_pine"));
        for (name, want) in [
            ("trains_carriage", Vec3::new(2.945, 0.895, 1.22)),
            ("trains_station", Vec3::new(6.6, 5.19, 6.506)),
            ("trains_platform", Vec3::new(60.0, 0.425, 6.3)),
        ] {
            let got = size(&build(name));
            assert!((got - want).abs().max_element() < 0.15, "{name}: {got} not {want}");
        }
    }

    #[test]
    fn a_person_has_every_part_the_hotel_recolours_and_animates() {
        for name in crate::games::hotel::models::PEOPLE {
            let parts = part_names(&build(name));
            let want: &[&str] = if name.ends_with("lie") {
                &["head", "hair"]
            } else {
                &[
                    "torso", "head", "hair", "hat", "brim", "arm_l", "arm_r", "hand_l", "hand_r",
                ]
            };
            for p in want {
                assert!(parts.iter().any(|n| n == p), "{name} has no {p}: {parts:?}");
            }
        }
        let standing = part_names(&build("hotel_person"));
        for p in ["leg_l", "leg_r", "shoe_l", "shoe_r", "boot_l", "boot_r"] {
            assert!(standing.iter().any(|n| n == p), "no {p}");
        }
        let swimmer = part_names(&build("hotel_swimmer"));
        for p in ["arm_l", "arm_r", "leg_l", "leg_r", "head", "cap"] {
            assert!(swimmer.iter().any(|n| n == p), "swimmer has no {p}");
        }
    }

    #[test]
    fn hotel_props_keep_the_size_of_the_primitives_they_replaced() {
        let want = [
            ("hotel_person", Vec3::new(0.80, 1.62, 0.66)),
            ("hotel_bed", Vec3::new(1.6, 1.0, 2.1)),
            ("hotel_desk", Vec3::new(3.2, 1.37, 0.92)),
            ("hotel_sofa", Vec3::new(2.1, 0.95, 0.8)),
            ("hotel_car", Vec3::new(1.44, 1.0, 2.4)),
            ("hotel_wardrobe", Vec3::new(1.1, 1.7, 0.63)),
            ("hotel_umbrella", Vec3::new(1.5, 1.8, 1.43)),
        ];
        let mut wrong = vec![];
        for (name, size_was) in want {
            let s = size(&build(name));
            if (s - size_was).abs().max_element() > 0.03 {
                wrong.push(format!("{name} is {s}, was {size_was}"));
            }
        }
        assert!(wrong.is_empty(), "{wrong:#?}");
    }

    #[test]
    fn helix_and_paper_models_have_the_parts_and_sizes_the_games_scale_from() {
        for (name, size_is) in [
            ("helix_ball", Vec3::ONE),
            ("helix_pole", Vec3::ONE),
            ("paper_cube", Vec3::ONE),
        ] {
            let s = size(&build(name));
            assert!((s - size_is).abs().max_element() < 0.03, "{name} is {s}");
        }
        let head = build("paper_head");
        assert_eq!(part_names(&head), ["body", "crown", "eye_l", "eye_r"]);
        let s = size(&head);
        assert!((s.x - 1.76).abs() < 0.03 && (s.z - 1.7).abs() < 0.03, "{s}");
        assert!(s.y > 1.39, "{s}");
    }
}
