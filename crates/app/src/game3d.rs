use crate::paint::{Gpu, PaintCamera, Tex};
use bevy::asset::RenderAssetUsages;
use bevy::camera::ClearColorConfig;
use bevy::core_pipeline::tonemapping::{DebandDither, Tonemapping};
use bevy::ecs::system::SystemParam;
use bevy::image::{ImageAddressMode, ImageSampler, ImageSamplerDescriptor};
use bevy::light::{AmbientLight, CascadeShadowConfig, CascadeShadowConfigBuilder, NotShadowCaster};
use bevy::math::Affine2;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;
use kg_core::geom::Color as Col;
use std::collections::HashMap;
use std::f32::consts::{FRAC_PI_2, TAU};

pub const MENU_CLEAR: Color = Color::srgb(1.0, 0.7, 0.47);

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub enum World3d {
    #[default]
    Flat,
    Hotel,
    Helix,
}

#[derive(Component, Clone, Copy, PartialEq, Eq, Debug)]
pub struct Owned(pub World3d);

#[derive(SystemSet, Clone, PartialEq, Eq, Hash, Debug)]
pub struct Step;

#[derive(SystemSet, Clone, PartialEq, Eq, Hash, Debug)]
pub struct Switch;

#[derive(SystemSet, Clone, PartialEq, Eq, Hash, Debug)]
pub struct Sync3d;

pub fn is_open(world: World3d) -> impl FnMut(Res<Palette>) -> bool + Clone {
    move |pal: Res<Palette>| pal.now == world
}

pub fn opened(world: World3d) -> impl FnMut(Res<Palette>) -> bool + Clone {
    move |pal: Res<Palette>| pal.now == world && pal.was != world
}

pub struct Game3dPlugin;

impl Plugin for Game3dPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Palette>()
            .configure_sets(Update, (Switch.after(Step), Sync3d.after(Switch)))
            .add_systems(Update, switch.in_set(Switch))
            .add_plugins((
                crate::games::helix::world::HelixWorld,
                crate::games::hotel::world::HotelWorld,
            ));
    }
}

fn switch(
    mut commands: Commands,
    frame: NonSend<crate::Frame>,
    mut pal: ResMut<Palette>,
    owned: Query<(Entity, &Owned)>,
    mut paint: Query<&mut Camera, With<PaintCamera>>,
) {
    let now = frame.play().map_or(World3d::Flat, |p| p.world());
    let was = pal.now;
    if pal.was != was || now != was {
        pal.was = was;
        pal.now = now;
    }
    if now == was {
        return;
    }
    for (e, o) in &owned {
        if o.0 != now {
            commands.entity(e).despawn();
        }
    }
    for mut c in &mut paint {
        c.clear_color = match now {
            World3d::Flat => ClearColorConfig::Custom(MENU_CLEAR),
            _ => ClearColorConfig::None,
        };
    }
}

pub fn playing<T: 'static>(frame: &crate::Frame) -> Option<&T> {
    frame.play()?.any_ref().downcast_ref::<T>()
}

pub fn bevy_color(c: Col) -> Color {
    Color::srgba(c.r, c.g, c.b, c.a)
}

pub fn v3(v: glam::Vec3) -> Vec3 {
    Vec3::new(v.x, v.y, v.z)
}

#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct Rig {
    pub eye: Vec3,
    pub target: Vec3,
    pub fov: f32,
    pub near: f32,
    pub far: f32,
    pub sky: Col,
    pub shadow_reach: f32,
}

impl Default for Rig {
    fn default() -> Self {
        Self {
            eye: Vec3::new(0.0, 10.0, 10.0),
            target: Vec3::ZERO,
            fov: 0.8,
            near: 0.5,
            far: 200.0,
            sky: kg_core::geom::WHITE,
            shadow_reach: 40.0,
        }
    }
}

#[derive(Component)]
pub struct Eye;

#[derive(Component)]
pub struct Sun;

pub struct Lighting {
    pub ambient: f32,
    pub illuminance: f32,
    pub color: Col,
    pub toward: Vec3,
}

pub struct Spawned {
    pub root: Entity,
    pub eye: Entity,
}

pub fn spawn_world(
    commands: &mut Commands,
    root: impl Bundle,
    world: World3d,
    light: Lighting,
) -> Spawned {
    let mut eye = None;
    let root = commands
        .spawn((
            root,
            Owned(world),
            Transform::default(),
            Visibility::default(),
        ))
        .with_children(|w| {
            let e = w.spawn((
                Camera3d::default(),
                Camera {
                    order: 0,
                    ..default()
                },
                Projection::Perspective(PerspectiveProjection::default()),
                Tonemapping::None,
                DebandDither::Disabled,
                AmbientLight {
                    color: Color::WHITE,
                    brightness: light.ambient,
                    affects_lightmapped_meshes: true,
                },
                Rig::default(),
                Eye,
            ));
            eye = Some(e.id());
            w.spawn((
                DirectionalLight {
                    illuminance: light.illuminance,
                    shadow_maps_enabled: true,
                    color: bevy_color(light.color),
                    ..default()
                },
                Transform::default().looking_to(light.toward.normalize(), Vec3::Y),
                shadow_reach(40.0),
                Sun,
            ));
        })
        .id();
    Spawned {
        root,
        eye: eye.unwrap_or(root),
    }
}

pub fn shadow_reach(reach: f32) -> CascadeShadowConfig {
    CascadeShadowConfigBuilder {
        num_cascades: 1,
        minimum_distance: 0.1,
        maximum_distance: reach,
        first_cascade_far_bound: reach,
        ..default()
    }
    .build()
}

pub type EyeParts<'a> = (
    &'a mut Rig,
    &'a mut Camera,
    &'a mut Transform,
    &'a mut Projection,
);

#[derive(SystemParam)]
pub struct Lens<'w, 's> {
    eye: Query<'w, 's, EyeParts<'static>, With<Eye>>,
    sun: Query<'w, 's, &'static mut CascadeShadowConfig, With<Sun>>,
}

impl Lens<'_, '_> {
    pub fn aim(&mut self, want: Rig) {
        for (mut rig, mut cam, mut t, mut p) in &mut self.eye {
            if *rig == want {
                continue;
            }
            *rig = want;
            cam.clear_color = ClearColorConfig::Custom(bevy_color(want.sky));
            *t = Transform::from_translation(want.eye).looking_at(want.target, Vec3::Y);
            *p = Projection::Perspective(PerspectiveProjection {
                fov: want.fov,
                near: want.near,
                far: want.far,
                ..default()
            });
            for mut c in &mut self.sun {
                if c.bounds.first() != Some(&want.shadow_reach) {
                    *c = shadow_reach(want.shadow_reach);
                }
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Shape {
    Cube,
    Sphere,
    Ball,
    Cylinder(u8),
    Cone(u8),
    Disc,
    Ring,
    Plane,
    Quad,
    Sector,
}

impl Shape {
    pub fn flat(self) -> bool {
        matches!(self, Shape::Disc | Shape::Ring | Shape::Plane | Shape::Quad)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Finish {
    Matte,
    Gloss,
    Glow,
    Ghost,
    Floor,
    Satin,
    Sticker,
    Backdrop,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Skin {
    Tile(&'static str, &'static str, Vec2),
    Sprite(&'static str, &'static str, [f32; 4]),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Paint {
    pub color: Col,
    pub finish: Finish,
    pub skin: Option<Skin>,
}

impl Paint {
    pub fn new(finish: Finish, color: Col) -> Self {
        Self {
            color,
            finish,
            skin: None,
        }
    }

    pub fn skinned(finish: Finish, color: Col, skin: Skin) -> Self {
        Self {
            color,
            finish,
            skin: Some(skin),
        }
    }

    pub fn solid(&self) -> bool {
        self.color.a >= 0.99 && matches!(self.finish, Finish::Matte | Finish::Gloss | Finish::Satin)
    }
}

#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct Part {
    pub shape: Shape,
    pub paint: Paint,
}

impl Part {
    pub fn casts(&self) -> bool {
        self.paint.solid() && !self.shape.flat()
    }
}

#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
#[component(on_add = shadow_hook)]
pub enum Shadow {
    Cast,
    Skip,
}

impl From<bool> for Shadow {
    fn from(cast: bool) -> Self {
        if cast {
            Shadow::Cast
        } else {
            Shadow::Skip
        }
    }
}

fn shadow_hook(mut world: bevy::ecs::world::DeferredWorld, ctx: bevy::ecs::lifecycle::HookContext) {
    if world.get::<Shadow>(ctx.entity) == Some(&Shadow::Skip) {
        world.commands().entity(ctx.entity).insert(NotShadowCaster);
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum SkinKey {
    Tile(&'static str, &'static str, [u16; 2]),
    Sprite(&'static str, &'static str, [i32; 4]),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct MatKey {
    color: [u8; 4],
    finish: Finish,
    skin: Option<SkinKey>,
}

fn byte(v: f32) -> u8 {
    (v.clamp(0.0, 1.0) * 255.0).round() as u8
}

fn key_of(p: &Paint) -> MatKey {
    MatKey {
        color: [
            byte(p.color.r),
            byte(p.color.g),
            byte(p.color.b),
            byte(p.color.a),
        ],
        finish: p.finish,
        skin: p.skin.map(|s| match s {
            Skin::Tile(a, b, r) => SkinKey::Tile(
                a,
                b,
                [(r.x * 16.0).round() as u16, (r.y * 16.0).round() as u16],
            ),
            Skin::Sprite(a, b, c) => SkinKey::Sprite(a, b, c.map(|v| (v * 1000.0).round() as i32)),
        }),
    }
}

#[derive(Resource, Default)]
pub struct Palette {
    pub now: World3d,
    pub was: World3d,
    meshes: HashMap<Shape, Handle<Mesh>>,
    mats: HashMap<MatKey, Handle<StandardMaterial>>,
    tiles: HashMap<(&'static str, &'static str), Handle<Image>>,
}

#[derive(SystemParam)]
pub struct Kit<'w> {
    pal: ResMut<'w, Palette>,
    meshes: ResMut<'w, Assets<Mesh>>,
    mats: ResMut<'w, Assets<StandardMaterial>>,
    images: ResMut<'w, Assets<Image>>,
    gpu: ResMut<'w, Gpu>,
}

impl Kit<'_> {
    pub fn mesh(&mut self, shape: Shape) -> Handle<Mesh> {
        if let Some(h) = self.pal.meshes.get(&shape) {
            return h.clone();
        }
        let h = self.meshes.add(build_mesh(shape));
        self.pal.meshes.insert(shape, h.clone());
        h
    }

    pub fn mat(&mut self, paint: Paint) -> Handle<StandardMaterial> {
        let key = key_of(&paint);
        if let Some(h) = self.pal.mats.get(&key) {
            return h.clone();
        }
        let m = self.material(&paint);
        let h = self.mats.add(m);
        self.pal.mats.insert(key, h.clone());
        h
    }

    fn material(&mut self, p: &Paint) -> StandardMaterial {
        let blend = p.color.a < 0.99;
        let mut m = StandardMaterial {
            base_color: bevy_color(p.color),
            perceptual_roughness: 0.75,
            reflectance: 0.2,
            alpha_mode: if blend {
                AlphaMode::Blend
            } else {
                AlphaMode::Opaque
            },
            ..default()
        };
        match p.finish {
            Finish::Matte => {}
            Finish::Gloss => {
                m.perceptual_roughness = 0.3;
                m.reflectance = 0.5;
            }
            Finish::Glow => {
                m.unlit = true;
                m.alpha_mode = AlphaMode::Blend;
            }
            Finish::Ghost => {
                m.unlit = true;
                m.alpha_mode = AlphaMode::Blend;
                m.cull_mode = None;
            }
            Finish::Floor => m.perceptual_roughness = 0.85,
            Finish::Satin => {
                m.perceptual_roughness = 0.55;
                m.reflectance = 0.3;
                m.cull_mode = None;
                m.double_sided = true;
            }
            Finish::Sticker => {
                m.unlit = true;
                m.alpha_mode = AlphaMode::Blend;
                m.cull_mode = None;
                m.double_sided = true;
            }
            Finish::Backdrop => {
                m.unlit = true;
                m.cull_mode = None;
            }
        }
        match p.skin {
            Some(Skin::Tile(sheet, name, repeat)) => {
                let images = &mut self.images;
                let tex =
                    self.pal
                        .tiles
                        .entry((sheet, name))
                        .or_insert_with(|| {
                            images.add(tile_image(sheet, name).unwrap_or_else(|| {
                                crate::paint::rgba_image(1, 1, vec![vec![255; 4]])
                            }))
                        })
                        .clone();
                m.base_color_texture = Some(tex);
                m.uv_transform = Affine2::from_scale(repeat);
            }
            Some(Skin::Sprite(sheet, name, [cx, cy, cw, ch])) => {
                if let Some(f) = crate::art::frame(sheet, name) {
                    let (pw, ph) = crate::art::page_size(sheet, f.page);
                    let tex = Tex::Page(sheet, f.page as u16);
                    m.base_color_texture = Some(self.gpu.page_handle(&mut self.images, tex));
                    let at =
                        Vec2::new((f.src.x + f.src.w * cx) / pw, (f.src.y + f.src.h * cy) / ph);
                    let size = Vec2::new(f.src.w * cw / pw, f.src.h * ch / ph);
                    m.uv_transform = Affine2::from_scale_angle_translation(size, 0.0, at);
                }
            }
            None => {}
        }
        m
    }

    pub fn part(&mut self, shape: Shape, paint: Paint, at: Transform) -> impl Bundle {
        let part = Part { shape, paint };
        let solid = (
            Mesh3d(self.mesh(shape)),
            MeshMaterial3d(self.mat(paint)),
            at,
            part,
        );
        (solid, Visibility::default(), Shadow::from(part.casts()))
    }

    pub fn repaint(
        &mut self,
        part: &mut Part,
        mat: &mut MeshMaterial3d<StandardMaterial>,
        paint: Paint,
    ) {
        if part.paint == paint {
            return;
        }
        part.paint = paint;
        let h = self.mat(paint);
        if mat.0 != h {
            mat.0 = h;
        }
    }
}

fn tile_image(sheet: &'static str, name: &'static str) -> Option<Image> {
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
    img.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
        address_mode_u: ImageAddressMode::Repeat,
        address_mode_v: ImageAddressMode::Repeat,
        ..ImageSamplerDescriptor::linear()
    });
    Some(img)
}

fn quads(pos: Vec<[f32; 3]>, nor: Vec<[f32; 3]>, uv: Vec<[f32; 2]>, idx: Vec<u32>) -> Mesh {
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

pub fn ring_mesh(inner: f32, segments: u32) -> Mesh {
    let (mut pos, mut nor, mut uv, mut idx) = (vec![], vec![], vec![], vec![]);
    for i in 0..=segments {
        let k = i as f32 / segments as f32;
        let (s, c) = (k * TAU).sin_cos();
        pos.push([c * 0.5, 0.0, s * 0.5]);
        pos.push([c * 0.5 * inner, 0.0, s * 0.5 * inner]);
        nor.extend([[0.0, 1.0, 0.0]; 2]);
        uv.extend([[k, 0.0], [k, 1.0]]);
        if i < segments {
            let b = i * 2;
            idx.extend([b, b + 2, b + 1, b + 1, b + 2, b + 3]);
        }
    }
    quads(pos, nor, uv, idx)
}

pub fn sector_mesh(outer: f32, inner: f32, thick: f32, span: f32, steps: usize) -> Mesh {
    let (mut pos, mut nor, mut uv, mut idx) = (vec![], vec![], vec![], vec![]);
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
    quads(pos, nor, uv, idx)
}

pub fn build_mesh(shape: Shape) -> Mesh {
    use kg_core::games::helix::{RING_INNER, RING_OUTER, RING_THICK, SEGMENTS};
    match shape {
        Shape::Cube => Cuboid::new(1.0, 1.0, 1.0).mesh().build(),
        Shape::Sphere => Sphere::new(0.5).mesh().uv(20, 12),
        Shape::Ball => Sphere::new(0.5).mesh().uv(32, 18),
        Shape::Cylinder(n) => Cylinder::new(0.5, 1.0).mesh().resolution(n as u32).build(),
        Shape::Cone(n) => Cone::new(0.5, 1.0).mesh().resolution(n as u32).build(),
        Shape::Disc => Circle::new(0.5)
            .mesh()
            .resolution(28)
            .build()
            .rotated_by(Quat::from_rotation_x(-FRAC_PI_2)),
        Shape::Ring => ring_mesh(0.8, 40),
        Shape::Plane => Plane3d::new(Vec3::Y, Vec2::splat(0.5)).mesh().build(),
        Shape::Quad => Rectangle::new(1.0, 1.0).mesh().build(),
        Shape::Sector => sector_mesh(RING_OUTER, RING_INNER, RING_THICK, TAU / SEGMENTS as f32, 6),
    }
}

pub fn visible(world: &World, e: Entity) -> bool {
    let mut at = Some(e);
    while let Some(x) = at {
        if world.get::<Visibility>(x) == Some(&Visibility::Hidden) {
            return false;
        }
        at = world.get::<ChildOf>(x).map(|c| c.parent());
    }
    true
}

#[cfg(test)]
pub mod testing {
    use super::*;
    use kg_core::engine::View;

    pub fn app(scene: crate::Scene, view: View) -> App {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, bevy::transform::TransformPlugin))
            .init_resource::<Assets<Mesh>>()
            .init_resource::<Assets<StandardMaterial>>()
            .init_resource::<Assets<Image>>()
            .init_resource::<Gpu>()
            .insert_resource(pen_for(view))
            .insert_non_send(crate::Frame::new(scene))
            .add_systems(Update, (|| {}).in_set(Step))
            .add_plugins(Game3dPlugin);
        app
    }

    fn pen_for(view: View) -> crate::paint::Pen {
        let mut pen = crate::paint::Pen::default();
        pen.begin(view, 1.0);
        pen
    }

    pub fn with_game(play: Box<dyn crate::games::Play>, view: View) -> App {
        let mut menu = crate::menu::App::new();
        menu.current = Some((0, play));
        app(crate::Scene::Menu(Box::new(menu)), view)
    }

    pub fn shot(n: u32, view: View) -> App {
        let mut shot = crate::shots::Shot::new(n);
        let mut pen = crate::paint::Pen::default();
        pen.begin(view, 1.0);
        shot.frame(&mut pen, &view, 1.0 / 60.0);
        app(crate::Scene::Shot(shot), view)
    }

    pub fn game<T: 'static>(app: &mut App) -> &mut T {
        let frame = app.world_mut().non_send_mut::<crate::Frame>().into_inner();
        frame
            .play_mut()
            .and_then(|p| p.as_any().downcast_mut::<T>())
            .expect("that game is open")
    }

    pub fn close(app: &mut App) {
        if let crate::Scene::Menu(m) = &mut frame(app).scene {
            m.current = None;
        }
    }

    pub fn open(app: &App) -> World3d {
        app.world().resource::<Palette>().now
    }

    pub fn count(app: &mut App) -> usize {
        let w = app.world_mut();
        w.query_filtered::<Entity, Without<bevy::ecs::resource::IsResource>>()
            .iter(w)
            .count()
    }

    pub fn parts(app: &mut App) -> Vec<(Part, GlobalTransform)> {
        let w = app.world_mut();
        let all: Vec<(Entity, Part, GlobalTransform)> = w
            .query::<(Entity, &Part, &GlobalTransform)>()
            .iter(w)
            .map(|(e, p, g)| (e, *p, *g))
            .collect();
        all.into_iter()
            .filter(|(e, _, _)| visible(w, *e))
            .map(|(_, p, g)| (p, g))
            .collect()
    }

    pub fn frame(app: &mut App) -> Mut<'_, crate::Frame> {
        app.world_mut().non_send_mut::<crate::Frame>()
    }

    pub fn step(app: &mut App, n: usize) {
        for _ in 0..n {
            app.update();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_shape_builds_a_mesh() {
        for s in [
            Shape::Cube,
            Shape::Sphere,
            Shape::Ball,
            Shape::Cylinder(10),
            Shape::Cone(10),
            Shape::Disc,
            Shape::Ring,
            Shape::Plane,
            Shape::Quad,
            Shape::Sector,
        ] {
            assert!(build_mesh(s).count_vertices() > 3, "{s:?}");
        }
    }

    #[test]
    fn a_sector_mesh_has_top_sides_and_caps() {
        let m = sector_mesh(150.0, 60.0, 18.0, TAU / 12.0, 6);
        assert_eq!(m.count_vertices(), (6 * 3 + 2) * 4);
    }

    #[test]
    fn material_keys_ignore_sub_level_colour_noise() {
        let a = Paint::new(Finish::Matte, Col::new(1.0, 0.5, 0.0, 1.0));
        let mut b = a;
        b.color.r = 0.9999;
        assert_eq!(key_of(&a), key_of(&b));
    }

    #[test]
    fn flat_and_see_through_parts_cast_no_shadow() {
        let white = kg_core::geom::WHITE;
        let part = |shape, paint| Part { shape, paint }.casts();
        assert!(part(Shape::Cube, Paint::new(Finish::Matte, white)));
        assert!(!part(Shape::Disc, Paint::new(Finish::Matte, white)));
        assert!(!part(Shape::Cube, Paint::new(Finish::Ghost, white)));
        assert!(!part(
            Shape::Sector,
            Paint::new(Finish::Satin, white.with_alpha(0.5))
        ));
    }

    #[test]
    fn the_menu_has_no_3d_world_and_leaves_the_paint_camera_clearing() {
        let mut app = testing::app(
            crate::Scene::Menu(Box::default()),
            kg_core::engine::View::ipad_landscape(),
        );
        testing::step(&mut app, 3);
        assert_eq!(app.world().resource::<Palette>().now, World3d::Flat);
        assert!(testing::parts(&mut app).is_empty());
    }
}
