use crate::paint::{Gpu, PaintCamera, Tex};
use bevy::camera::visibility::NoFrustumCulling;
use bevy::camera::{ClearColorConfig, Exposure};
use bevy::core_pipeline::tonemapping::{DebandDither, Tonemapping};
use bevy::light::{
    AmbientLight, CascadeShadowConfigBuilder, DirectionalLight, DirectionalLightShadowMap,
    NotShadowCaster,
};
use bevy::math::{Affine2, Vec2 as BVec2};
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;
use bevy::render::render_resource::Face;
use std::collections::HashMap;
use std::f32::consts::TAU;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Shape3 {
    Sector,
    Cylinder,
    Sphere,
    Quad,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Skin {
    pub sheet: &'static str,
    pub name: &'static str,
    pub crop: [f32; 4],
}

#[derive(Clone, Copy, Debug)]
pub struct Item {
    pub shape: Shape3,
    pub skin: Option<Skin>,
    pub color: [f32; 4],
    pub pos: [f32; 3],
    pub rot: [f32; 4],
    pub scale: [f32; 3],
    pub unlit: bool,
    pub blend: bool,
    pub cast: bool,
}

impl Item {
    pub fn new(shape: Shape3) -> Self {
        Self {
            shape,
            skin: None,
            color: [1.0; 4],
            pos: [0.0; 3],
            rot: [0.0, 0.0, 0.0, 1.0],
            scale: [1.0; 3],
            unlit: false,
            blend: false,
            cast: true,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Scene3 {
    pub eye: [f32; 3],
    pub target: [f32; 3],
    pub fov: f32,
    pub near: f32,
    pub far: f32,
    pub light: [f32; 3],
    pub illuminance: f32,
    pub ambient: f32,
    pub sky: [f32; 3],
    pub items: Vec<Item>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct MatKey {
    skin: Option<(&'static str, &'static str, [i32; 4])>,
    color: [u8; 4],
    unlit: bool,
    blend: bool,
}

#[derive(Component)]
pub struct Cam3;

#[derive(Component)]
pub struct Sun3;

#[derive(Component)]
pub struct Piece3;

#[derive(Resource, Default)]
pub struct Gpu3 {
    active: bool,
    meshes: HashMap<Shape3, Handle<Mesh>>,
    mats: HashMap<MatKey, Handle<StandardMaterial>>,
    casters: Vec<Entity>,
    plain: Vec<Entity>,
}

pub fn sector_mesh(outer: f32, inner: f32, thick: f32, span: f32, steps: usize) -> Mesh {
    let mut pos: Vec<[f32; 3]> = vec![];
    let mut nor: Vec<[f32; 3]> = vec![];
    let mut uv: Vec<[f32; 2]> = vec![];
    let mut idx: Vec<u32> = vec![];
    let mut quad = |p: [[f32; 3]; 4], n: [f32; 3], t: [[f32; 2]; 4]| {
        let b = pos.len() as u32;
        pos.extend_from_slice(&p);
        nor.extend_from_slice(&[n; 4]);
        uv.extend_from_slice(&t);
        idx.extend_from_slice(&[b, b + 1, b + 2, b, b + 2, b + 3]);
    };
    let at = |a: f32, r: f32, y: f32| [a.cos() * r, y, a.sin() * r];
    for s in 0..steps {
        let k0 = s as f32 / steps as f32;
        let k1 = (s + 1) as f32 / steps as f32;
        let (a0, a1) = (k0 * span, k1 * span);
        let mid = (a0 + a1) / 2.0;
        quad(
            [
                at(a0, inner, 0.0),
                at(a0, outer, 0.0),
                at(a1, outer, 0.0),
                at(a1, inner, 0.0),
            ],
            [0.0, 1.0, 0.0],
            [[k0, 0.0], [k0, 0.8], [k1, 0.8], [k1, 0.0]],
        );
        quad(
            [
                at(a0, outer, 0.0),
                at(a0, outer, -thick),
                at(a1, outer, -thick),
                at(a1, outer, 0.0),
            ],
            [mid.cos(), 0.0, mid.sin()],
            [[k0, 0.8], [k0, 1.0], [k1, 1.0], [k1, 0.8]],
        );
        quad(
            [
                at(a0, inner, 0.0),
                at(a0, inner, -thick),
                at(a1, inner, -thick),
                at(a1, inner, 0.0),
            ],
            [-mid.cos(), 0.0, -mid.sin()],
            [[k0, 0.8], [k0, 1.0], [k1, 1.0], [k1, 0.8]],
        );
    }
    for (a, n) in [
        (0.0, [0.0, 0.0, -1.0]),
        (span, [-span.sin(), 0.0, span.cos()]),
    ] {
        quad(
            [
                at(a, inner, 0.0),
                at(a, outer, 0.0),
                at(a, outer, -thick),
                at(a, inner, -thick),
            ],
            n,
            [[0.5, 0.9]; 4],
        );
    }
    let mut m = Mesh::new(
        PrimitiveTopology::TriangleList,
        bevy::asset::RenderAssetUsages::RENDER_WORLD,
    );
    m.insert_attribute(Mesh::ATTRIBUTE_POSITION, pos);
    m.insert_attribute(Mesh::ATTRIBUTE_NORMAL, nor);
    m.insert_attribute(Mesh::ATTRIBUTE_UV_0, uv);
    m.insert_indices(Indices::U32(idx));
    m
}

fn mesh_for(shape: Shape3) -> Mesh {
    use kg_core::games::helix::{RING_INNER, RING_OUTER, RING_THICK, SEGMENTS};
    match shape {
        Shape3::Sector => sector_mesh(RING_OUTER, RING_INNER, RING_THICK, TAU / SEGMENTS as f32, 6),
        Shape3::Cylinder => Mesh::from(Cylinder::new(1.0, 1.0)),
        Shape3::Sphere => Sphere::new(1.0).mesh().uv(32, 18),
        Shape3::Quad => Mesh::from(Rectangle::new(1.0, 1.0)),
    }
}

fn key_of(it: &Item) -> MatKey {
    let q = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    MatKey {
        skin: it
            .skin
            .map(|s| (s.sheet, s.name, s.crop.map(|c| (c * 1000.0).round() as i32))),
        color: it.color.map(q),
        unlit: it.unlit,
        blend: it.blend,
    }
}

fn material_for(it: &Item, gpu: &mut Gpu, images: &mut Assets<Image>) -> StandardMaterial {
    let mut m = StandardMaterial {
        base_color: Color::srgba(it.color[0], it.color[1], it.color[2], it.color[3]),
        perceptual_roughness: 0.55,
        reflectance: 0.3,
        unlit: it.unlit,
        cull_mode: None,
        double_sided: true,
        alpha_mode: if it.blend {
            AlphaMode::Blend
        } else {
            AlphaMode::Opaque
        },
        ..default()
    };
    if let Some(s) = it.skin {
        if let Some(f) = crate::art::frame(s.sheet, s.name) {
            let (pw, ph) = crate::art::page_size(s.sheet, f.page);
            let tex = Tex::Page(s.sheet, f.page as u16);
            m.base_color_texture = Some(gpu.page_handle(images, tex));
            let [cx, cy, cw, ch] = s.crop;
            let (u0, v0) = ((f.src.x + f.src.w * cx) / pw, (f.src.y + f.src.h * cy) / ph);
            let (su, sv) = (f.src.w * cw / pw, f.src.h * ch / ph);
            m.uv_transform =
                Affine2::from_scale_angle_translation(BVec2::new(su, sv), 0.0, BVec2::new(u0, v0));
        }
    }
    let _ = Face::Back;
    m
}

type CamParts = (
    &'static mut Camera,
    &'static mut Transform,
    &'static mut Projection,
);
type CamOnly = (With<Cam3>, Without<Sun3>, Without<Piece3>);
type SunParts = (
    &'static mut DirectionalLight,
    &'static mut Transform,
    &'static mut Visibility,
);
type SunOnly = (With<Sun3>, Without<Cam3>, Without<Piece3>);
type PieceParts = (
    &'static mut Visibility,
    &'static mut Transform,
    &'static mut Mesh3d,
    &'static mut MeshMaterial3d<StandardMaterial>,
);
type PieceOnly = (With<Piece3>, Without<Cam3>, Without<Sun3>);

pub fn hand_back(active: &mut bool, has_scene: bool) -> bool {
    let release = *active && !has_scene;
    *active = has_scene;
    release
}

#[allow(clippy::too_many_arguments)]
pub fn sync3(
    mut commands: Commands,
    pen: Res<crate::paint::Pen>,
    mut gpu: ResMut<Gpu>,
    mut g3: ResMut<Gpu3>,
    mut images: ResMut<Assets<Image>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut cam: Query<CamParts, CamOnly>,
    mut sun: Query<SunParts, SunOnly>,
    mut pieces: Query<PieceParts, PieceOnly>,
    mut paint: Query<&mut Camera, (With<PaintCamera>, Without<Cam3>)>,
) {
    let Some(scene) = pen.scene.as_ref() else {
        if !hand_back(&mut g3.active, false) {
            return;
        }
        for (mut c, _, _) in cam.iter_mut() {
            if c.is_active {
                c.is_active = false;
            }
        }
        for (_, _, mut v) in sun.iter_mut() {
            *v = Visibility::Hidden;
        }
        for e in g3.casters.iter().chain(g3.plain.iter()) {
            if let Ok((mut v, _, _, _)) = pieces.get_mut(*e) {
                if *v != Visibility::Hidden {
                    *v = Visibility::Hidden;
                }
            }
        }
        for mut p in paint.iter_mut() {
            p.clear_color = ClearColorConfig::Custom(Color::srgb(1.0, 0.7, 0.47));
        }
        return;
    };
    hand_back(&mut g3.active, true);
    for mut p in paint.iter_mut() {
        p.clear_color = ClearColorConfig::None;
    }
    let eye = Vec3::from(scene.eye);
    let target = Vec3::from(scene.target);
    let tf = Transform::from_translation(eye).looking_at(target, Vec3::Y);
    let projection = Projection::Perspective(PerspectiveProjection {
        fov: scene.fov,
        near: scene.near,
        far: scene.far,
        ..default()
    });
    let ambient = AmbientLight {
        color: Color::WHITE,
        brightness: scene.ambient,
        ..default()
    };
    if let Ok((mut c, mut t, mut p)) = cam.single_mut() {
        c.is_active = true;
        c.clear_color =
            ClearColorConfig::Custom(Color::srgb(scene.sky[0], scene.sky[1], scene.sky[2]));
        *t = tf;
        *p = projection;
    } else {
        commands.spawn((
            Camera3d::default(),
            Camera {
                order: 0,
                clear_color: ClearColorConfig::Custom(Color::srgb(
                    scene.sky[0],
                    scene.sky[1],
                    scene.sky[2],
                )),
                ..default()
            },
            projection,
            tf,
            Tonemapping::None,
            DebandDither::Disabled,
            Exposure::default(),
            ambient,
            Cam3,
        ));
    }
    let dir = Vec3::from(scene.light).normalize();
    let sun_tf = Transform::default().looking_to(dir, Vec3::Y);
    if let Ok((mut l, mut t, mut v)) = sun.single_mut() {
        l.illuminance = scene.illuminance;
        *t = sun_tf;
        *v = Visibility::Inherited;
    } else {
        let reach = (eye - target).length();
        commands.spawn((
            DirectionalLight {
                illuminance: scene.illuminance,
                shadow_maps_enabled: true,
                ..default()
            },
            CascadeShadowConfigBuilder {
                num_cascades: 1,
                minimum_distance: 1.0,
                maximum_distance: reach * 2.0,
                first_cascade_far_bound: reach * 2.0,
                ..default()
            }
            .build(),
            sun_tf,
            Sun3,
        ));
        commands.insert_resource(DirectionalLightShadowMap { size: 2048 });
    }
    let mut next_cast = 0;
    let mut next_plain = 0;
    for it in &scene.items {
        let h = g3
            .meshes
            .entry(it.shape)
            .or_insert_with(|| meshes.add(mesh_for(it.shape)))
            .clone();
        let key = key_of(it);
        let mat = match g3.mats.get(&key) {
            Some(m) => m.clone(),
            None => {
                let m = materials.add(material_for(it, &mut gpu, &mut images));
                g3.mats.insert(key, m.clone());
                m
            }
        };
        let t = Transform {
            translation: Vec3::from(it.pos),
            rotation: Quat::from_array(it.rot),
            scale: Vec3::from(it.scale),
        };
        let (pool, slot) = if it.cast && !it.blend {
            (&mut g3.casters, &mut next_cast)
        } else {
            (&mut g3.plain, &mut next_plain)
        };
        if *slot < pool.len() {
            if let Ok((mut v, mut pt, mut m, mut mm)) = pieces.get_mut(pool[*slot]) {
                *v = Visibility::Inherited;
                *pt = t;
                if m.0 != h {
                    m.0 = h;
                }
                if mm.0 != mat {
                    mm.0 = mat;
                }
            }
        } else {
            let mut e = commands.spawn((
                Mesh3d(h),
                MeshMaterial3d(mat),
                t,
                Visibility::Inherited,
                NoFrustumCulling,
                Piece3,
            ));
            if !(it.cast && !it.blend) {
                e.insert(NotShadowCaster);
            }
            let id = e.id();
            pool.push(id);
        }
        *slot += 1;
    }
    for (pool, used) in [(&g3.casters, next_cast), (&g3.plain, next_plain)] {
        for e in pool.iter().skip(used) {
            if let Ok((mut v, _, _, _)) = pieces.get_mut(*e) {
                if *v != Visibility::Hidden {
                    *v = Visibility::Hidden;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_idle_layer_leaves_the_paint_camera_alone_for_other_3d_games() {
        let mut active = false;
        for _ in 0..3 {
            assert!(!hand_back(&mut active, false));
        }
        assert!(!hand_back(&mut active, true));
        assert!(hand_back(&mut active, false));
        assert!(!hand_back(&mut active, false));
    }

    #[test]
    fn a_sector_mesh_has_top_sides_and_caps() {
        let m = sector_mesh(150.0, 60.0, 18.0, TAU / 12.0, 6);
        assert_eq!(m.count_vertices(), (6 * 3 + 2) * 4);
    }

    #[test]
    fn material_keys_ignore_sub_level_colour_noise() {
        let a = Item::new(Shape3::Quad);
        let mut b = a;
        b.color[0] = 0.9999;
        assert_eq!(key_of(&a), key_of(&b));
    }
}
