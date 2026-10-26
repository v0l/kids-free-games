use bevy::asset::RenderAssetUsages;
use bevy::camera::visibility::NoFrustumCulling;
use bevy::camera::{ClearColorConfig, ScalingMode};
use bevy::core_pipeline::tonemapping::{DebandDither, Tonemapping};
use bevy::image::{ImageSampler, ImageSamplerDescriptor};
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use bevy::sprite_render::AlphaMode2d;
use glam::Vec2;
use kg_core::engine::View;
use kg_core::geom::Color as Col;
use std::collections::HashMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Tex {
    White,
    Glyphs,
    Page(&'static str, u16),
}

#[derive(Default)]
pub struct Batch {
    pub tex: Option<Tex>,
    pub pos: Vec<[f32; 3]>,
    pub uv: Vec<[f32; 2]>,
    pub color: Vec<[f32; 4]>,
    pub idx: Vec<u32>,
}

impl Batch {
    fn clear(&mut self) {
        self.tex = None;
        self.pos.clear();
        self.uv.clear();
        self.color.clear();
        self.idx.clear();
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Vert {
    pub pos: Vec2,
    pub uv: Vec2,
    pub color: Col,
}

#[derive(Resource, Default)]
pub struct Pen {
    pub batches: Vec<Batch>,
    used: usize,
    pub offset: Vec2,
    pub detail: f32,
    pub view: Option<View>,
}

fn srgb_to_linear(c: f32) -> f32 {
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

pub fn linear(c: Col) -> [f32; 4] {
    [
        srgb_to_linear(c.r.clamp(0.0, 1.0)),
        srgb_to_linear(c.g.clamp(0.0, 1.0)),
        srgb_to_linear(c.b.clamp(0.0, 1.0)),
        c.a.clamp(0.0, 1.0),
    ]
}

impl Pen {
    pub fn begin(&mut self, view: View, detail: f32) {
        for b in &mut self.batches[..self.used] {
            b.clear();
        }
        self.used = 0;
        self.offset = Vec2::ZERO;
        self.detail = detail.max(0.01);
        kg_core::font::set_text_scale(self.detail);
        self.view = Some(view);
    }

    pub fn used(&self) -> &[Batch] {
        &self.batches[..self.used]
    }

    pub fn vertex_count(&self) -> usize {
        self.used().iter().map(|b| b.pos.len()).sum()
    }

    fn batch(&mut self, tex: Tex) -> &mut Batch {
        let reuse = self.used > 0
            && self.batches[self.used - 1].tex == Some(tex)
            && self.batches[self.used - 1].pos.len() < 60_000;
        if !reuse {
            if self.used == self.batches.len() {
                self.batches.push(Batch::default());
            }
            self.batches[self.used].tex = Some(tex);
            self.used += 1;
        }
        &mut self.batches[self.used - 1]
    }

    pub fn mesh(&mut self, tex: Tex, verts: &[Vert], idx: &[u32]) {
        let off = self.offset;
        let b = self.batch(tex);
        let base = b.pos.len() as u32;
        for v in verts {
            let p = v.pos + off;
            b.pos.push([p.x, -p.y, 0.0]);
            b.uv.push([v.uv.x, v.uv.y]);
            b.color.push(linear(v.color));
        }
        b.idx.extend(idx.iter().map(|i| base + i));
    }

    pub fn quad(&mut self, tex: Tex, corners: [Vec2; 4], uvs: [Vec2; 4], color: Col) {
        let v = |i: usize| Vert {
            pos: corners[i],
            uv: uvs[i],
            color,
        };
        self.mesh(tex, &[v(0), v(1), v(2), v(3)], &[0, 1, 2, 0, 2, 3]);
    }

    pub fn tri(&mut self, a: Vec2, b: Vec2, c: Vec2, color: Col) {
        let half = Vec2::splat(0.5);
        let v = |p: Vec2| Vert {
            pos: p,
            uv: half,
            color,
        };
        self.mesh(Tex::White, &[v(a), v(b), v(c)], &[0, 1, 2]);
    }

    pub fn sides(&self, radius: f32) -> usize {
        (radius * self.detail * 0.6).clamp(20.0, 96.0) as usize
    }
}

#[derive(Component)]
struct Layer;

#[derive(Resource, Default)]
pub struct Gpu {
    pages: HashMap<Tex, Handle<Image>>,
    materials: HashMap<Tex, Handle<ColorMaterial>>,
    layers: Vec<(Entity, Handle<Mesh>)>,
    glyph_generation: u64,
}

#[derive(Component)]
pub struct PaintCamera;

pub struct PaintPlugin;

impl Plugin for PaintPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Pen>()
            .init_resource::<Gpu>()
            .add_systems(Startup, spawn_camera)
            .add_systems(PostUpdate, sync);
    }
}

fn spawn_camera(mut commands: Commands) {
    commands.spawn((
        Camera2d,
        Camera {
            order: 10,
            clear_color: ClearColorConfig::Custom(Color::srgb(1.0, 0.7, 0.47)),
            ..default()
        },
        Tonemapping::None,
        DebandDither::Disabled,
        PaintCamera,
        Projection::Orthographic(OrthographicProjection {
            scaling_mode: ScalingMode::Fixed {
                width: 960.0,
                height: 720.0,
            },
            ..OrthographicProjection::default_2d()
        }),
    ));
}

pub fn rgba_image(w: u32, h: u32, mips: Vec<Vec<u8>>) -> Image {
    let levels = mips.len().max(1) as u32;
    let mut img = Image::new(
        Extent3d {
            width: w,
            height: h,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        mips.concat(),
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    );
    img.texture_descriptor.mip_level_count = levels;
    img.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor::linear());
    img
}

fn page_image(tex: Tex) -> Image {
    match tex {
        Tex::White => rgba_image(1, 1, vec![vec![255; 4]]),
        Tex::Glyphs => {
            let (size, alpha) = crate::text::atlas_alpha();
            glyph_image(size, &alpha)
        }
        Tex::Page(sheet, page) => match crate::art::decode(sheet, page as usize) {
            Some((w, h, mips)) => rgba_image(w, h, mips),
            None => rgba_image(1, 1, vec![vec![255, 0, 255, 255]]),
        },
    }
}

fn glyph_image(size: usize, alpha: &[u8]) -> Image {
    let mut px = Vec::with_capacity(size * size * 4);
    for a in alpha {
        px.extend_from_slice(&[255, 255, 255, *a]);
    }
    let mut img = rgba_image(size as u32, size as u32, vec![px]);
    img.asset_usage = RenderAssetUsages::RENDER_WORLD | RenderAssetUsages::MAIN_WORLD;
    img
}

type CameraParts = (&'static mut Projection, &'static mut Transform);
type CameraOnly = (With<PaintCamera>, Without<Layer>);

#[allow(clippy::too_many_arguments)]
fn sync(
    mut commands: Commands,
    pen: Res<Pen>,
    mut gpu: ResMut<Gpu>,
    mut images: ResMut<Assets<Image>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    mut layers: Query<(&mut Visibility, &mut MeshMaterial2d<ColorMaterial>, &mut Transform), With<Layer>>,
    mut camera: Query<CameraParts, CameraOnly>,
    window: Query<&Window>,
) {
    let Some(view) = pen.view else { return };
    let f = view.full();
    if let Ok((mut proj, mut t)) = camera.single_mut() {
        if let Projection::Orthographic(o) = proj.as_mut() {
            let same = matches!(o.scaling_mode, ScalingMode::Fixed { width, height } if width == f.w && height == f.h);
            if !same {
                o.scaling_mode = ScalingMode::Fixed {
                    width: f.w,
                    height: f.h,
                };
            }
        }
        let at = Vec3::new(f.x + f.w / 2.0, -(f.y + f.h / 2.0), 0.0);
        if t.translation != at {
            t.translation = at;
        }
    }
    let _ = window;
    let generation = crate::text::atlas_generation();
    if generation != gpu.glyph_generation {
        gpu.glyph_generation = generation;
        if let Some(h) = gpu.pages.get(&Tex::Glyphs).cloned() {
            let (size, alpha) = crate::text::atlas_alpha();
            let _ = images.insert(h.id(), glyph_image(size, &alpha));
        }
    }
    let used = pen.used();
    for (i, b) in used.iter().enumerate() {
        let tex = b.tex.unwrap_or(Tex::White);
        if !gpu.materials.contains_key(&tex) {
            let img = images.add(page_image(tex));
            gpu.pages.insert(tex, img.clone());
            let mat = materials.add(ColorMaterial {
                color: Color::WHITE,
                texture: Some(img),
                alpha_mode: AlphaMode2d::Blend,
                ..default()
            });
            gpu.materials.insert(tex, mat);
        }
        let mat = gpu.materials[&tex].clone();
        let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::RENDER_WORLD);
        mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, b.pos.clone());
        mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, b.uv.clone());
        mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, b.color.clone());
        mesh.insert_indices(Indices::U32(b.idx.clone()));
        let z = i as f32 * 0.01;
        if i < gpu.layers.len() {
            let (e, h) = gpu.layers[i].clone();
            let _ = meshes.insert(h.id(), mesh);
            if let Ok((mut vis, mut m, mut t)) = layers.get_mut(e) {
                if *vis != Visibility::Inherited {
                    *vis = Visibility::Inherited;
                }
                if m.0 != mat {
                    m.0 = mat;
                }
                t.translation.z = z;
            }
        } else {
            let h = meshes.add(mesh);
            let e = commands
                .spawn((
                    Mesh2d(h.clone()),
                    MeshMaterial2d(mat),
                    Transform::from_xyz(0.0, 0.0, z),
                    NoFrustumCulling,
                    Layer,
                ))
                .id();
            gpu.layers.push((e, h));
        }
    }
    for (e, _) in gpu.layers.iter().skip(used.len()) {
        if let Ok((mut vis, _, _)) = layers.get_mut(*e) {
            if *vis != Visibility::Hidden {
                *vis = Visibility::Hidden;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::vec2;
    use kg_core::geom::WHITE;

    #[test]
    fn same_texture_draws_share_a_batch_and_a_switch_starts_a_new_one() {
        let mut p = Pen::default();
        p.begin(View::ipad_landscape(), 1.0);
        p.tri(vec2(0.0, 0.0), vec2(1.0, 0.0), vec2(0.0, 1.0), WHITE);
        p.tri(vec2(0.0, 0.0), vec2(1.0, 0.0), vec2(0.0, 1.0), WHITE);
        p.quad(Tex::Glyphs, [Vec2::ZERO; 4], [Vec2::ZERO; 4], WHITE);
        p.tri(vec2(0.0, 0.0), vec2(1.0, 0.0), vec2(0.0, 1.0), WHITE);
        assert_eq!(p.used().len(), 3);
        assert_eq!(p.used()[0].idx, vec![0, 1, 2, 3, 4, 5]);
        p.begin(View::ipad_landscape(), 1.0);
        assert!(p.used().is_empty());
        assert_eq!(p.vertex_count(), 0);
    }

    #[test]
    fn view_down_is_world_down_and_the_offset_shifts_everything() {
        let mut p = Pen::default();
        p.begin(View::ipad_landscape(), 1.0);
        p.offset = vec2(5.0, 7.0);
        p.tri(vec2(10.0, 20.0), vec2(1.0, 0.0), vec2(0.0, 1.0), WHITE);
        assert_eq!(p.used()[0].pos[0], [15.0, -27.0, 0.0]);
    }

    #[test]
    fn colours_are_handed_to_the_gpu_in_linear_light() {
        let c = linear(Col::new(0.5, 1.0, 0.0, 0.5));
        assert!((c[0] - 0.214).abs() < 0.01);
        assert_eq!(c[1], 1.0);
        assert_eq!(c[3], 0.5);
    }
}
