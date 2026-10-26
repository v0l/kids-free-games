use bevy::asset::RenderAssetUsages;
use bevy::camera::ClearColorConfig;
use bevy::light::{CascadeShadowConfigBuilder, NotShadowCaster};
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;
use kg_core::geom::Color as Col;
use std::cell::RefCell;
use std::collections::HashMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Shape {
    Cube,
    Sphere,
    Cylinder(u8),
    Cone(u8),
    Disc,
    Ring,
    Plane,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Surface {
    Matte,
    Gloss,
    Glow,
    Ghost,
    Floor(&'static str, &'static str),
}

#[derive(Clone, Copy, Debug)]
pub struct Item {
    pub shape: Shape,
    pub surface: Surface,
    pub color: Col,
    pub at: Vec3,
    pub rot: Quat,
    pub scale: Vec3,
    pub uv_repeat: Vec2,
    pub shadow: bool,
}

#[derive(Clone, Debug, Default)]
pub struct Scene3d {
    pub items: Vec<Item>,
    pub eye: Vec3,
    pub target: Vec3,
    pub fov: f32,
    pub sky: Col,
    pub shadow_reach: f32,
}

thread_local! {
    static SLOT: RefCell<Option<Scene3d>> = const { RefCell::new(None) };
}

pub fn submit(scene: Scene3d) {
    SLOT.with(|s| *s.borrow_mut() = Some(scene));
}

pub fn take() -> Option<Scene3d> {
    SLOT.with(|s| s.borrow_mut().take())
}

pub fn peek<R>(f: impl FnOnce(Option<&Scene3d>) -> R) -> R {
    SLOT.with(|s| f(s.borrow().as_ref()))
}

impl Scene3d {
    pub fn push(
        &mut self,
        shape: Shape,
        surface: Surface,
        color: Col,
        at: Vec3,
        rot: Quat,
        scale: Vec3,
    ) {
        self.items.push(Item {
            shape,
            surface,
            color,
            at,
            rot,
            scale,
            uv_repeat: Vec2::ONE,
            shadow: !matches!(surface, Surface::Ghost | Surface::Glow | Surface::Floor(..))
                && !matches!(shape, Shape::Disc | Shape::Ring | Shape::Plane),
        });
    }
}

#[derive(Component)]
pub struct World3d;

#[derive(Component)]
struct Slot;

#[derive(Component)]
struct Eye;

#[derive(Component)]
struct Sun;

#[derive(Resource, Default)]
struct Cache {
    meshes: HashMap<Shape, Handle<Mesh>>,
    materials: HashMap<(Surface, [u8; 4], [u16; 2]), Handle<StandardMaterial>>,
    textures: HashMap<(&'static str, &'static str), Handle<Image>>,
    slots: Vec<Entity>,
    active: bool,
}

pub struct World3dPlugin;

impl Plugin for World3dPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Cache>()
            .insert_resource(GlobalAmbientLight {
                color: Color::WHITE,
                brightness: 700.0,
                affects_lightmapped_meshes: true,
            })
            .add_systems(PostUpdate, sync);
    }
}

fn ring_mesh(inner: f32, segments: u32) -> Mesh {
    let mut pos = vec![];
    let mut nor = vec![];
    let mut uv = vec![];
    let mut idx = vec![];
    for i in 0..=segments {
        let a = i as f32 / segments as f32 * std::f32::consts::TAU;
        let (s, c) = a.sin_cos();
        pos.push([c * 0.5, 0.0, s * 0.5]);
        pos.push([c * 0.5 * inner, 0.0, s * 0.5 * inner]);
        nor.push([0.0, 1.0, 0.0]);
        nor.push([0.0, 1.0, 0.0]);
        uv.push([i as f32 / segments as f32, 0.0]);
        uv.push([i as f32 / segments as f32, 1.0]);
        if i < segments {
            let b = i * 2;
            idx.extend([b, b + 2, b + 1, b + 1, b + 2, b + 3]);
        }
    }
    let mut m = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::RENDER_WORLD,
    );
    m.insert_attribute(Mesh::ATTRIBUTE_POSITION, pos);
    m.insert_attribute(Mesh::ATTRIBUTE_NORMAL, nor);
    m.insert_attribute(Mesh::ATTRIBUTE_UV_0, uv);
    m.insert_indices(Indices::U32(idx));
    m
}

fn build_mesh(shape: Shape) -> Mesh {
    match shape {
        Shape::Cube => Cuboid::new(1.0, 1.0, 1.0).mesh().build(),
        Shape::Sphere => Sphere::new(0.5).mesh().uv(20, 12),
        Shape::Cylinder(n) => Cylinder::new(0.5, 1.0).mesh().resolution(n as u32).build(),
        Shape::Cone(n) => Cone::new(0.5, 1.0).mesh().resolution(n as u32).build(),
        Shape::Disc => Circle::new(0.5)
            .mesh()
            .resolution(28)
            .build()
            .rotated_by(Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2)),
        Shape::Ring => ring_mesh(0.8, 40),
        Shape::Plane => Plane3d::new(Vec3::Y, Vec2::splat(0.5)).mesh().build(),
    }
}

fn floor_image(sheet: &'static str, name: &'static str) -> Option<Image> {
    let f = crate::art::frame(sheet, name)?;
    let (w, h, mips) = crate::art::decode(sheet, f.page)?;
    let (x0, y0) = (f.src.x as u32, f.src.y as u32);
    let (cw, ch) = (f.src.w as u32, f.src.h as u32);
    let src = &mips[0];
    let mut px = Vec::with_capacity((cw * ch * 4) as usize);
    for y in y0..(y0 + ch).min(h) {
        let a = ((y * w + x0) * 4) as usize;
        px.extend_from_slice(&src[a..a + (cw.min(w - x0) * 4) as usize]);
    }
    let mut img = crate::paint::rgba_image(cw, ch, vec![px]);
    img.sampler = bevy::image::ImageSampler::Descriptor(bevy::image::ImageSamplerDescriptor {
        address_mode_u: bevy::image::ImageAddressMode::Repeat,
        address_mode_v: bevy::image::ImageAddressMode::Repeat,
        ..bevy::image::ImageSamplerDescriptor::linear()
    });
    Some(img)
}

fn to_bevy(c: Col) -> Color {
    Color::srgba(c.r, c.g, c.b, c.a)
}

fn key_of(c: Col) -> [u8; 4] {
    [
        (c.r.clamp(0.0, 1.0) * 255.0) as u8,
        (c.g.clamp(0.0, 1.0) * 255.0) as u8,
        (c.b.clamp(0.0, 1.0) * 255.0) as u8,
        (c.a.clamp(0.0, 1.0) * 255.0) as u8,
    ]
}

fn material(
    cache: &mut Cache,
    mats: &mut Assets<StandardMaterial>,
    images: &mut Assets<Image>,
    item: &Item,
) -> Handle<StandardMaterial> {
    let rep = [
        (item.uv_repeat.x * 16.0).round() as u16,
        (item.uv_repeat.y * 16.0).round() as u16,
    ];
    let key = (item.surface, key_of(item.color), rep);
    if let Some(h) = cache.materials.get(&key) {
        return h.clone();
    }
    let base = to_bevy(item.color);
    let mut m = StandardMaterial {
        base_color: base,
        perceptual_roughness: 0.75,
        reflectance: 0.2,
        ..default()
    };
    match item.surface {
        Surface::Matte => {}
        Surface::Gloss => {
            m.perceptual_roughness = 0.3;
            m.reflectance = 0.5;
        }
        Surface::Glow => {
            m.unlit = true;
            m.alpha_mode = AlphaMode::Blend;
        }
        Surface::Ghost => {
            m.alpha_mode = AlphaMode::Blend;
            m.unlit = true;
            m.cull_mode = None;
        }
        Surface::Floor(sheet, name) => {
            let tex = cache
                .textures
                .entry((sheet, name))
                .or_insert_with(|| {
                    images.add(
                        floor_image(sheet, name)
                            .unwrap_or_else(|| crate::paint::rgba_image(1, 1, vec![vec![255; 4]])),
                    )
                })
                .clone();
            m.base_color_texture = Some(tex);
            m.uv_transform = bevy::math::Affine2::from_scale(item.uv_repeat);
            m.perceptual_roughness = 0.85;
        }
    }
    let h = mats.add(m);
    cache.materials.insert(key, h.clone());
    h
}

type PaintCam = (With<crate::paint::PaintCamera>, Without<Eye>);
type SlotParts<'a> = (
    &'a mut Transform,
    &'a mut Mesh3d,
    &'a mut MeshMaterial3d<StandardMaterial>,
    &'a mut Visibility,
    Has<NotShadowCaster>,
);
type EyeParts<'a> = (&'a mut Transform, &'a mut Projection);

#[allow(clippy::too_many_arguments)]
fn sync(
    mut commands: Commands,
    mut cache: ResMut<Cache>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut mats: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
    mut slots: Query<SlotParts, (With<Slot>, Without<Eye>)>,
    mut eye: Query<EyeParts, (With<Eye>, Without<Slot>)>,
    owned: Query<Entity, With<World3d>>,
    mut paint: Query<&mut Camera, PaintCam>,
) {
    let Some(scene) = take() else {
        if cache.active {
            for e in &owned {
                commands.entity(e).despawn();
            }
            cache.slots.clear();
            cache.active = false;
            if let Ok(mut c) = paint.single_mut() {
                c.clear_color = ClearColorConfig::Custom(Color::srgb(1.0, 0.7, 0.47));
            }
        }
        return;
    };
    if !cache.active {
        cache.active = true;
        if let Ok(mut c) = paint.single_mut() {
            c.clear_color = ClearColorConfig::None;
        }
        commands.spawn((
            Camera3d::default(),
            Camera {
                order: 0,
                clear_color: ClearColorConfig::Custom(to_bevy(scene.sky)),
                ..default()
            },
            Projection::Perspective(PerspectiveProjection {
                fov: scene.fov,
                near: 0.5,
                far: 200.0,
                ..default()
            }),
            Transform::from_translation(scene.eye).looking_at(scene.target, Vec3::Y),
            bevy::core_pipeline::tonemapping::Tonemapping::None,
            Eye,
            World3d,
        ));
        commands.spawn((
            DirectionalLight {
                illuminance: 9000.0,
                shadow_maps_enabled: true,
                color: Color::srgb(1.0, 0.97, 0.9),
                ..default()
            },
            Transform::from_xyz(-6.0, 14.0, 8.0).looking_at(Vec3::ZERO, Vec3::Y),
            CascadeShadowConfigBuilder {
                num_cascades: 1,
                maximum_distance: scene.shadow_reach.max(20.0),
                ..default()
            }
            .build(),
            Sun,
            World3d,
        ));
    }
    if let Ok((mut t, mut p)) = eye.single_mut() {
        let want = Transform::from_translation(scene.eye).looking_at(scene.target, Vec3::Y);
        if *t != want {
            *t = want;
        }
        if let Projection::Perspective(pp) = p.as_mut() {
            if (pp.fov - scene.fov).abs() > 1e-4 {
                pp.fov = scene.fov;
            }
        }
    }
    for (i, item) in scene.items.iter().enumerate() {
        let mesh = cache
            .meshes
            .entry(item.shape)
            .or_insert_with(|| meshes.add(build_mesh(item.shape)))
            .clone();
        let mat = material(&mut cache, &mut mats, &mut images, item);
        let tf = Transform {
            translation: item.at,
            rotation: item.rot,
            scale: item.scale,
        };
        if let Some(&e) = cache.slots.get(i) {
            if let Ok((mut t, mut m, mut mm, mut vis, no_shadow)) = slots.get_mut(e) {
                if *t != tf {
                    *t = tf;
                }
                if m.0 != mesh {
                    m.0 = mesh;
                }
                if mm.0 != mat {
                    mm.0 = mat;
                }
                if *vis != Visibility::Inherited {
                    *vis = Visibility::Inherited;
                }
                if no_shadow == item.shadow {
                    if item.shadow {
                        commands.entity(e).remove::<NotShadowCaster>();
                    } else {
                        commands.entity(e).insert(NotShadowCaster);
                    }
                }
            }
        } else {
            let mut ec = commands.spawn((Mesh3d(mesh), MeshMaterial3d(mat), tf, Slot, World3d));
            if !item.shadow {
                ec.insert(NotShadowCaster);
            }
            cache.slots.push(ec.id());
        }
    }
    for &e in cache.slots.iter().skip(scene.items.len()) {
        if let Ok((_, _, _, mut vis, _)) = slots.get_mut(e) {
            if *vis != Visibility::Hidden {
                *vis = Visibility::Hidden;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_slot_hands_over_one_scene_per_frame() {
        assert!(take().is_none());
        submit(Scene3d {
            fov: 0.8,
            ..Default::default()
        });
        assert!(peek(|s| s.is_some()));
        assert_eq!(take().unwrap().fov, 0.8);
        assert!(take().is_none());
    }

    #[test]
    fn flat_and_see_through_things_cast_no_shadow() {
        let mut s = Scene3d::default();
        s.push(
            Shape::Cube,
            Surface::Matte,
            kg_core::geom::WHITE,
            Vec3::ZERO,
            Quat::IDENTITY,
            Vec3::ONE,
        );
        s.push(
            Shape::Disc,
            Surface::Matte,
            kg_core::geom::WHITE,
            Vec3::ZERO,
            Quat::IDENTITY,
            Vec3::ONE,
        );
        s.push(
            Shape::Cube,
            Surface::Ghost,
            kg_core::geom::WHITE,
            Vec3::ZERO,
            Quat::IDENTITY,
            Vec3::ONE,
        );
        assert_eq!(
            s.items.iter().map(|i| i.shadow).collect::<Vec<_>>(),
            [true, false, false]
        );
    }

    #[test]
    fn every_shape_builds_a_mesh() {
        for s in [
            Shape::Cube,
            Shape::Sphere,
            Shape::Cylinder(10),
            Shape::Cone(10),
            Shape::Disc,
            Shape::Ring,
            Shape::Plane,
        ] {
            assert!(build_mesh(s).count_vertices() > 3, "{s:?}");
        }
    }
}
