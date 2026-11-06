use bevy::animation::{animated_field, AnimatedBy, AnimationTargetId};
use bevy::asset::io::Reader;
use bevy::asset::{AssetLoader, LoadContext, RenderAssetUsages};
use bevy::camera::visibility::NoFrustumCulling;
use bevy::curve::{ConstantCurve, Interval, UnevenSampleAutoCurve};
use bevy::mesh::skinning::{SkinnedMesh, SkinnedMeshInverseBindposes};
use bevy::mesh::{Indices, PrimitiveTopology, VertexAttributeValues};
use bevy::prelude::*;
use bevy::world_serialization::WorldAsset;
use serde::{Deserialize, Serialize};

pub const EXTENSION: &str = "gcad";

#[derive(Default, TypePath)]
pub struct GcadLoader;

#[derive(Serialize, Deserialize, Clone, Copy, Debug)]
pub struct GcadSettings {
    pub detail: f64,
}

impl Default for GcadSettings {
    fn default() -> Self {
        Self { detail: 1.0 }
    }
}

#[derive(Debug)]
pub struct GcadError(pub String);

impl std::fmt::Display for GcadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for GcadError {}

pub fn expand_includes(
    source: &str,
    read: &mut dyn FnMut(&str) -> Result<String, GcadError>,
) -> Result<String, GcadError> {
    let mut out = String::with_capacity(source.len());
    for line in source.lines() {
        if let Some(name) = line.trim().strip_prefix("include ") {
            let inner = read(name.trim())?;
            out.push_str(&expand_includes(&inner, read)?);
        } else {
            out.push_str(line);
            out.push('\n');
        }
    }
    Ok(out)
}

pub fn build(source: &str, detail: f64) -> Result<gcad_mesh::Model, GcadError> {
    use std::sync::{Arc, Mutex, OnceLock};
    type Built = Arc<OnceLock<Result<gcad_mesh::Model, String>>>;
    static CACHE: OnceLock<Mutex<Vec<(u64, Built)>>> = OnceLock::new();
    let key = {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        source.hash(&mut h);
        detail.to_bits().hash(&mut h);
        h.finish()
    };
    let slot: Built = {
        let mut cache = CACHE.get_or_init(Default::default).lock().unwrap();
        cache.retain(|(_, b)| Arc::strong_count(b) > 1 || b.get().is_some());
        match cache.iter().find(|(k, _)| *k == key) {
            Some((_, b)) => b.clone(),
            None => {
                let b: Built = Arc::default();
                cache.push((key, b.clone()));
                if cache.len() > 8 {
                    cache.remove(0);
                }
                b
            }
        }
    };
    slot.get_or_init(|| build_uncached(source, detail).map_err(|e| e.0))
        .clone()
        .map_err(GcadError)
}

fn build_uncached(source: &str, detail: f64) -> Result<gcad_mesh::Model, GcadError> {
    let options = gcad_mesh::Options {
        detail,
        ..Default::default()
    };
    let started = bevy::platform::time::Instant::now();
    let model = gcad_mesh::build_with(source, &options)
        .map_err(|e| GcadError(format!("line {} `{}`: {}", e.line, e.text, e.message)))?;
    let triangles: usize = model.bodies.iter().map(|b| b.indices.len() / 3).sum();
    info!(
        "built gcad model ({} bytes, {} triangles, detail {}) in {:.0} ms",
        source.len(),
        triangles,
        detail,
        started.elapsed().as_secs_f64() * 1000.0
    );
    Ok(model)
}

pub fn rigged(model: &gcad_mesh::Model) -> bool {
    model.bodies.iter().any(|b| !b.joints.is_empty())
}

pub fn fit(model: &gcad_mesh::Model) -> (f32, Vec3) {
    let (mut lo, mut hi) = (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN));
    for p in model.bodies.iter().flat_map(|b| &b.positions) {
        lo = lo.min(Vec3::from(*p));
        hi = hi.max(Vec3::from(*p));
    }
    let height = (hi.y - lo.y).max(1.0e-6);
    let base = if rigged(model) {
        Vec3::new(0.0, lo.y, 0.0)
    } else {
        Vec3::new((lo.x + hi.x) / 2.0, lo.y, (lo.z + hi.z) / 2.0)
    };
    (height, base)
}

fn mesh_of(body: &gcad_mesh::Body, part: &gcad_mesh::Part) -> Mesh {
    let range = part.first_index..part.first_index + part.index_count;
    let mut used: Vec<u32> = body.indices[range.clone()].to_vec();
    used.sort_unstable();
    used.dedup();
    let mut remap = vec![u32::MAX; body.positions.len()];
    for (new, old) in used.iter().enumerate() {
        remap[*old as usize] = new as u32;
    }
    let pick = |v: &[[f32; 3]]| -> Vec<[f32; 3]> { used.iter().map(|i| v[*i as usize]).collect() };
    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, pick(&body.positions));
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, pick(&body.normals));
    if !body.colours.is_empty() {
        let colours: Vec<[f32; 4]> = used
            .iter()
            .map(|i| {
                let c = Color::srgb_from_array(body.colours[*i as usize]).to_linear();
                [c.red, c.green, c.blue, 1.0]
            })
            .collect();
        mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colours);
    }
    if !body.joints.is_empty() {
        let joints: Vec<[u16; 4]> = used
            .iter()
            .map(|i| body.joint_indices[*i as usize])
            .collect();
        let weights: Vec<[f32; 4]> = used
            .iter()
            .map(|i| body.joint_weights[*i as usize])
            .collect();
        mesh.insert_attribute(
            Mesh::ATTRIBUTE_JOINT_INDEX,
            VertexAttributeValues::Uint16x4(joints),
        );
        mesh.insert_attribute(Mesh::ATTRIBUTE_JOINT_WEIGHT, weights);
    }
    mesh.insert_indices(Indices::U32(
        body.indices[range]
            .iter()
            .map(|i| remap[*i as usize])
            .collect(),
    ));
    mesh
}

fn material_of(body: &gcad_mesh::Body, part: &gcad_mesh::Part) -> StandardMaterial {
    let tint = if body.colours.is_empty() {
        Color::srgb_from_array(part.colour)
    } else {
        Color::WHITE
    };
    StandardMaterial {
        base_color: tint,
        perceptual_roughness: 1.0 - part.gloss * 0.8,
        metallic: 0.0,
        ..default()
    }
}

fn clip_of(clip: &gcad_mesh::Clip, paths: &[Vec<Name>]) -> AnimationClip {
    let mut out = AnimationClip::default();
    let target = |joint: usize| AnimationTargetId::from_names(paths[joint].iter());
    for track in &clip.tracks {
        let rotations: Vec<Quat> = track
            .rotations
            .iter()
            .map(|q| Quat::from_array(*q))
            .collect();
        if rotations.len() == 1 {
            out.add_curve_to_target(
                target(track.joint),
                AnimatableCurve::new(
                    animated_field!(Transform::rotation),
                    ConstantCurve::new(Interval::EVERYWHERE, rotations[0]),
                ),
            );
        } else if let Ok(curve) =
            UnevenSampleAutoCurve::new(track.times.iter().copied().zip(rotations))
        {
            out.add_curve_to_target(
                target(track.joint),
                AnimatableCurve::new(animated_field!(Transform::rotation), curve),
            );
        }
    }
    for track in &clip.moves {
        let offsets: Vec<Vec3> = track.offsets.iter().map(|v| Vec3::from(*v)).collect();
        if offsets.len() == 1 {
            out.add_curve_to_target(
                target(track.joint),
                AnimatableCurve::new(
                    animated_field!(Transform::translation),
                    ConstantCurve::new(Interval::EVERYWHERE, offsets[0]),
                ),
            );
        } else if let Ok(curve) =
            UnevenSampleAutoCurve::new(track.times.iter().copied().zip(offsets))
        {
            out.add_curve_to_target(
                target(track.joint),
                AnimatableCurve::new(animated_field!(Transform::translation), curve),
            );
        }
    }
    out.set_duration(clip.duration.max(1.0 / clip.fps.max(1.0)));
    out
}

fn local_offset(body: &gcad_mesh::Body, joint: usize) -> Vec3 {
    let j = &body.joints[joint];
    match j.parent {
        Some(p) => Vec3::from(j.head) - Vec3::from(body.joints[p].head),
        None => Vec3::from(j.head),
    }
}

pub fn clip_names(model: &gcad_mesh::Model) -> Vec<String> {
    let mut names: Vec<String> = model
        .bodies
        .iter()
        .flat_map(|b| b.clips.iter().map(|c| c.name.clone()))
        .collect();
    names.sort();
    names.dedup();
    names
}

pub fn into_world(model: &gcad_mesh::Model, load_context: &mut LoadContext) -> WorldAsset {
    let (height, base) = fit(model);
    let scale = 1.0 / height;
    let mut world = World::default();
    let root = world
        .spawn((
            Transform::from_scale(Vec3::splat(scale)).with_translation(-base * scale),
            Visibility::default(),
            Name::new("gcad"),
        ))
        .id();
    let names = clip_names(model);
    for body in &model.bodies {
        let body_root = world
            .spawn((
                Transform::default(),
                Visibility::default(),
                Name::new(body.name.clone()),
            ))
            .id();
        world.entity_mut(root).add_child(body_root);
        let mut joints = Vec::new();
        let mut paths: Vec<Vec<Name>> = Vec::new();
        for (i, j) in body.joints.iter().enumerate() {
            let name = Name::new(j.name.clone());
            let path = match j.parent {
                Some(p) => {
                    let mut v = paths[p].clone();
                    v.push(name.clone());
                    v
                }
                None => vec![name.clone()],
            };
            let e = world
                .spawn((
                    Transform::from_translation(local_offset(body, i)),
                    Visibility::default(),
                    name,
                    AnimationTargetId::from_names(path.iter()),
                    AnimatedBy(body_root),
                ))
                .id();
            let parent = j.parent.map_or(body_root, |p| joints[p]);
            world.entity_mut(parent).add_child(e);
            joints.push(e);
            paths.push(path);
        }
        let skin = (!body.joints.is_empty()).then(|| {
            let binds: Vec<Mat4> = body
                .joints
                .iter()
                .map(|j| Mat4::from_translation(-Vec3::from(j.head)))
                .collect();
            load_context.add_labeled_asset(
                format!("{}/Skin", body.name),
                SkinnedMeshInverseBindposes::from(binds),
            )
        });
        if !body.joints.is_empty() {
            world
                .entity_mut(body_root)
                .insert(AnimationPlayer::default());
            for name in &names {
                let clip = body.clips.iter().find(|c| &c.name == name);
                let clip = clip.map(|c| clip_of(c, &paths)).unwrap_or_default();
                load_context.add_labeled_asset(format!("Animation/{name}"), clip);
            }
        }
        for part in &body.parts {
            let mesh = load_context.add_labeled_asset(
                format!("{}/{}/Mesh", body.name, part.name),
                mesh_of(body, part),
            );
            let material = load_context.add_labeled_asset(
                format!("{}/{}/Material", body.name, part.name),
                material_of(body, part),
            );
            let mut e = world.spawn((
                Mesh3d(mesh),
                MeshMaterial3d(material),
                Transform::default(),
                Visibility::default(),
                Name::new(part.name.clone()),
            ));
            if let Some(binds) = &skin {
                e.insert((
                    SkinnedMesh {
                        inverse_bindposes: binds.clone(),
                        joints: joints.clone(),
                    },
                    NoFrustumCulling,
                ));
            }
            let id = e.id();
            world.entity_mut(body_root).add_child(id);
        }
    }
    WorldAsset::new(world)
}

impl AssetLoader for GcadLoader {
    type Asset = WorldAsset;
    type Settings = GcadSettings;
    type Error = GcadError;

    async fn load(
        &self,
        reader: &mut dyn Reader,
        settings: &GcadSettings,
        load_context: &mut LoadContext<'_>,
    ) -> Result<WorldAsset, GcadError> {
        let mut bytes = Vec::new();
        reader
            .read_to_end(&mut bytes)
            .await
            .map_err(|e| GcadError(e.to_string()))?;
        let text = String::from_utf8(bytes).map_err(|e| GcadError(e.to_string()))?;
        let dir = load_context
            .path()
            .path()
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_default();
        let mut includes = Vec::new();
        for line in text.lines() {
            if let Some(name) = line.trim().strip_prefix("include ") {
                let path = dir.join(name.trim());
                let bytes = load_context
                    .read_asset_bytes(path.clone())
                    .await
                    .map_err(|e| GcadError(format!("{}: {e}", path.display())))?;
                includes.push((
                    name.trim().to_string(),
                    String::from_utf8(bytes).map_err(|e| GcadError(e.to_string()))?,
                ));
            }
        }
        let source = expand_includes(&text, &mut |name| {
            includes
                .iter()
                .find(|(n, _)| n == name)
                .map(|(_, t)| t.clone())
                .ok_or_else(|| GcadError(format!("include {name} not found")))
        })?;
        let model = build(&source, settings.detail)?;
        Ok(into_world(&model, load_context))
    }

    fn extensions(&self) -> &[&str] {
        &[EXTENSION]
    }
}

pub struct GcadPlugin;

pub const WARM: [&str; 2] = ["models/hero.gcad", "models/prop_tree.gcad"];

#[cfg(not(target_arch = "wasm32"))]
pub fn source_of(rel: &str) -> Option<String> {
    let text = String::from_utf8(kg_core::assets::read(rel)?).ok()?;
    let dir = rel.rsplit_once('/').map_or("", |(d, _)| d).to_string();
    expand_includes(&text, &mut |name| {
        kg_core::assets::read(&format!("{dir}/{name}"))
            .and_then(|b| String::from_utf8(b).ok())
            .ok_or_else(|| GcadError(format!("{dir}/{name} missing")))
    })
    .ok()
}

fn warm_up() {
    #[cfg(not(target_arch = "wasm32"))]
    bevy::tasks::AsyncComputeTaskPool::get()
        .spawn(async {
            for rel in WARM {
                if let Some(source) = source_of(rel) {
                    let started = bevy::platform::time::Instant::now();
                    if build(&source, 1.0).is_ok() {
                        info!(
                            "warmed {rel} in {:.0} ms",
                            started.elapsed().as_secs_f64() * 1000.0
                        );
                    }
                }
            }
        })
        .detach();
}

impl Plugin for GcadPlugin {
    fn build(&self, app: &mut App) {
        app.init_asset_loader::<GcadLoader>()
            .add_systems(Startup, warm_up);
    }
}
