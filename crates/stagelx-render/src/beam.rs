use bevy::{
    asset::RenderAssetUsages,
    mesh::{Indices, MeshVertexBufferLayoutRef, PrimitiveTopology},
    pbr::{MaterialPipeline, MaterialPipelineKey},
    prelude::*,
    render::render_resource::{
        AsBindGroup, Extent3d, RenderPipelineDescriptor, SpecializedMeshPipelineError,
        TextureDimension, TextureFormat,
    },
    shader::ShaderRef,
};

// ─── BeamMaterial ─────────────────────────────────────────────────────────────

/// Custom material for beam cones.
/// - `color`       — rgba premultiplied by dimmer/strobe, maps to vec4<f32> binding 0
/// - `gobo_params` — x = rotation (radians), binding 1
/// - `gobo`        — gobo mask texture, binding 2/3
#[derive(Asset, TypePath, AsBindGroup, Clone)]
pub struct BeamMaterial {
    #[uniform(0)]
    pub color: LinearRgba,
    #[uniform(1)]
    pub gobo_params: Vec4, // x = rotation_radians
    #[texture(2)]
    #[sampler(3)]
    pub gobo: Handle<Image>,
}

impl Material for BeamMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/beam.wgsl".into()
    }
    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Add
    }
    fn specialize(
        _pipeline: &MaterialPipeline,
        descriptor: &mut RenderPipelineDescriptor,
        _layout: &MeshVertexBufferLayoutRef,
        _key: MaterialPipelineKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        descriptor.primitive.cull_mode = None;
        Ok(())
    }
}

// ─── GoboLibrary resource ─────────────────────────────────────────────────────

pub const GOBO_NAMES: &[&str] = &["Open", "Dots", "Breakup", "Star"];

#[derive(Resource)]
pub struct GoboLibrary {
    pub handles: Vec<Handle<Image>>,
}

pub fn setup_gobos(mut images: ResMut<Assets<Image>>, mut commands: Commands) {
    const SIZE: u32 = 128;

    let mut add = |px: Vec<u8>| -> Handle<Image> {
        images.add(Image::new(
            Extent3d { width: SIZE, height: SIZE, depth_or_array_layers: 1 },
            TextureDimension::D2,
            px,
            TextureFormat::Rgba8UnormSrgb,
            RenderAssetUsages::RENDER_WORLD,
        ))
    };

    let handles = vec![
        add(gen_open(SIZE)),
        add(gen_dots(SIZE)),
        add(gen_breakup(SIZE)),
        add(gen_star(SIZE)),
    ];

    commands.insert_resource(GoboLibrary { handles });
}

// ─── Procedural gobo generators ───────────────────────────────────────────────

fn in_disc(x: f32, y: f32, r: f32) -> bool {
    x * x + y * y < r * r
}

fn rgba(on: bool) -> [u8; 4] {
    let v = if on { 255 } else { 0 };
    [v, v, v, 255]
}

fn coords(col: u32, row: u32, size: u32) -> (f32, f32) {
    let x = (col as f32 + 0.5) / size as f32 * 2.0 - 1.0;
    let y = (row as f32 + 0.5) / size as f32 * 2.0 - 1.0;
    (x, y)
}

fn gen_open(size: u32) -> Vec<u8> {
    let n = size as usize;
    let mut data = vec![0u8; n * n * 4];
    for row in 0..size {
        for col in 0..size {
            let (x, y) = coords(col, row, size);
            let p = rgba(in_disc(x, y, 0.95));
            let i = (row as usize * n + col as usize) * 4;
            data[i..i + 4].copy_from_slice(&p);
        }
    }
    data
}

fn gen_dots(size: u32) -> Vec<u8> {
    let n = size as usize;
    let mut data = vec![0u8; n * n * 4];
    // Centre dot + 6 surrounding dots in a hex layout
    let centers: &[(f32, f32)] = &[
        (0.0, 0.0),
        (0.5, 0.0), (-0.5, 0.0),
        (0.25, 0.43), (-0.25, 0.43),
        (0.25, -0.43), (-0.25, -0.43),
    ];
    const DR: f32 = 0.21;
    for row in 0..size {
        for col in 0..size {
            let (x, y) = coords(col, row, size);
            let outer = in_disc(x, y, 0.95);
            let dot = centers.iter().any(|&(cx, cy)| in_disc(x - cx, y - cy, DR));
            let p = rgba(outer && dot);
            let i = (row as usize * n + col as usize) * 4;
            data[i..i + 4].copy_from_slice(&p);
        }
    }
    data
}

fn gen_breakup(size: u32) -> Vec<u8> {
    let n = size as usize;
    let mut data = vec![0u8; n * n * 4];
    for row in 0..size {
        for col in 0..size {
            let (x, y) = coords(col, row, size);
            let outer = in_disc(x, y, 0.95);
            let v = (x * 6.3 + y * 4.1).sin()
                + (x * 3.7 - y * 5.9).sin()
                + ((x * x + y * y) * 9.0).cos();
            let p = rgba(outer && v > 0.2);
            let i = (row as usize * n + col as usize) * 4;
            data[i..i + 4].copy_from_slice(&p);
        }
    }
    data
}

fn gen_star(size: u32) -> Vec<u8> {
    let n = size as usize;
    let mut data = vec![0u8; n * n * 4];
    for row in 0..size {
        for col in 0..size {
            let (x, y) = coords(col, row, size);
            let r = (x * x + y * y).sqrt();
            let angle = y.atan2(x);
            let spoke = ((angle * 5.0).sin() * 0.5 + 0.5).powi(2);
            let p = rgba(in_disc(x, y, 0.95) && r < 0.22 + spoke * 0.68);
            let i = (row as usize * n + col as usize) * 4;
            data[i..i + 4].copy_from_slice(&p);
        }
    }
    data
}

// ─── Projection-UV cone mesh ──────────────────────────────────────────────────

/// Cone with apex at (0, +height/2, 0) and base circle of `radius` at
/// (0, -height/2, 0).  UV coordinates use a top-down XZ projection:
///   apex  → (0.5, 0.5)
///   base  → (cos θ · 0.5 + 0.5,  sin θ · 0.5 + 0.5)
/// This makes gobo textures appear as if projected from the light source.
pub fn build_beam_cone(radius: f32, height: f32) -> Mesh {
    const SEGMENTS: u32 = 48;

    let mut positions: Vec<[f32; 3]> = Vec::new();
    let mut normals: Vec<[f32; 3]> = Vec::new();
    let mut uvs: Vec<[f32; 2]> = Vec::new();
    let mut indices: Vec<u32> = Vec::new();

    // Apex
    positions.push([0.0, height * 0.5, 0.0]);
    normals.push([0.0, 1.0, 0.0]);
    uvs.push([0.5, 0.5]);

    // Base ring (SEGMENTS + 1 so the last vertex wraps back to angle 0)
    for i in 0..=SEGMENTS {
        let angle = (i as f32 / SEGMENTS as f32) * std::f32::consts::TAU;
        let (ca, sa) = (angle.cos(), angle.sin());
        positions.push([ca * radius, -height * 0.5, sa * radius]);
        let n = Vec3::new(ca, radius / height, sa).normalize();
        normals.push(n.to_array());
        uvs.push([ca * 0.5 + 0.5, sa * 0.5 + 0.5]);
    }

    // Triangles (apex = 0, base ring = 1..=SEGMENTS+1)
    for i in 0..SEGMENTS {
        indices.push(0);
        indices.push(i + 1);
        indices.push(i + 2);
    }

    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::RENDER_WORLD);
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_indices(Indices::U32(indices));
    mesh
}
