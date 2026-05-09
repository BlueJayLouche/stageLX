//! Venue / stage geometry loading.
//!
//! Supports:
//!   - OBJ  — parsed with `tobj`, converted to Bevy meshes manually
//!   - GLB / glTF — parsed with the `gltf` crate, triangles extracted manually
//!   - FBX — parsed with `ufbx`, triangulated and converted to Bevy meshes
//!
//! Loaded meshes are spawned as children of a root `VenueRoot` entity so that
//! the entire venue can be despawned by removing that entity.

use bevy::{
    asset::RenderAssetUsages,
    mesh::{Indices, PrimitiveTopology},
    prelude::*,
};

// ─── Component ────────────────────────────────────────────────────────────────

/// Marks the root entity of the loaded venue geometry.
#[derive(Component)]
pub struct VenueRoot;

// ─── Public load function ─────────────────────────────────────────────────────

/// Read a venue file from `path` and spawn it.
/// Replaces any previously loaded venue.
pub fn load_venue(
    path: &str,
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    existing: &Query<Entity, With<VenueRoot>>,
) -> Result<(), String> {
    // Despawn existing venue before loading the new one.
    for entity in existing.iter() {
        commands.entity(entity).despawn();
    }

    let lower = path.to_lowercase();
    if lower.ends_with(".obj") {
        load_obj(path, commands, meshes, materials)
    } else if lower.ends_with(".glb") || lower.ends_with(".gltf") {
        load_glb(path, commands, meshes, materials)
    } else if lower.ends_with(".fbx") {
        load_fbx(path, commands, meshes, materials)
    } else {
        Err(format!("Unsupported format — use .obj, .glb/.gltf, or .fbx (got '{}')", path))
    }
}

// ─── OBJ loader ──────────────────────────────────────────────────────────────

fn load_obj(
    path: &str,
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
) -> Result<(), String> {
    let (models, _mats) = tobj::load_obj(path, &tobj::GPU_LOAD_OPTIONS)
        .map_err(|e| format!("OBJ load error: {e}"))?;

    let venue = commands.spawn((
        Transform::default(),
        Visibility::default(),
        VenueRoot,
    )).id();

    let mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.5, 0.5, 0.5),
        perceptual_roughness: 0.8,
        metallic: 0.0,
        ..default()
    });

    for model in &models {
        let m = &model.mesh;
        if m.indices.is_empty() { continue; }

        let positions: Vec<[f32; 3]> = m.positions.chunks(3)
            .map(|c| [c[0], c[1], c[2]])
            .collect();

        let normals: Vec<[f32; 3]> = if m.normals.len() == m.positions.len() {
            m.normals.chunks(3).map(|c| [c[0], c[1], c[2]]).collect()
        } else {
            compute_flat_normals(&positions, &m.indices)
        };

        let uvs: Vec<[f32; 2]> = if !m.texcoords.is_empty() {
            m.texcoords.chunks(2).map(|c| [c[0], 1.0 - c[1]]).collect()
        } else {
            vec![[0.0; 2]; positions.len()]
        };

        let mut bevy_mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::RENDER_WORLD);
        bevy_mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
        bevy_mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL,   normals);
        bevy_mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0,     uvs);
        bevy_mesh.insert_indices(Indices::U32(m.indices.clone()));

        let child = commands.spawn((
            Mesh3d(meshes.add(bevy_mesh)),
            MeshMaterial3d(mat.clone()),
            Transform::default(),
        )).id();
        commands.entity(venue).add_child(child);
    }

    info!("Venue OBJ loaded: {} mesh(es) from '{}'", models.len(), path);
    Ok(())
}

// ─── GLB / glTF loader ────────────────────────────────────────────────────────

fn load_glb(
    path: &str,
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
) -> Result<(), String> {
    let (doc, buffers, _images) = gltf::import(path)
        .map_err(|e| format!("glTF load error: {e}"))?;

    let venue = commands.spawn((
        Transform::default(),
        Visibility::default(),
        VenueRoot,
    )).id();

    let mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.5, 0.5, 0.5),
        perceptual_roughness: 0.8,
        metallic: 0.0,
        ..default()
    });

    let mut mesh_count = 0usize;
    for mesh in doc.meshes() {
        for prim in mesh.primitives() {
            let reader = prim.reader(|buf| buffers.get(buf.index()).map(|b| b.0.as_slice()));

            let positions: Vec<[f32; 3]> = match reader.read_positions() {
                Some(iter) => iter.collect(),
                None => continue,
            };

            let indices: Vec<u32> = match reader.read_indices() {
                Some(iter) => iter.into_u32().collect(),
                None => continue,
            };

            let normals: Vec<[f32; 3]> = reader.read_normals()
                .map(|iter| iter.collect())
                .unwrap_or_else(|| compute_flat_normals(&positions, &indices));

            let uvs: Vec<[f32; 2]> = reader.read_tex_coords(0)
                .map(|iter| iter.into_f32().collect())
                .unwrap_or_else(|| vec![[0.0; 2]; positions.len()]);

            let mut bevy_mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::RENDER_WORLD);
            bevy_mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
            bevy_mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL,   normals);
            bevy_mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0,     uvs);
            bevy_mesh.insert_indices(Indices::U32(indices));

            let child = commands.spawn((
                Mesh3d(meshes.add(bevy_mesh)),
                MeshMaterial3d(mat.clone()),
                Transform::default(),
            )).id();
            commands.entity(venue).add_child(child);
            mesh_count += 1;
        }
    }

    info!("Venue glTF loaded: {} primitive(s) from '{}'", mesh_count, path);
    Ok(())
}

// ─── FBX loader ───────────────────────────────────────────────────────────────

fn load_fbx(
    path: &str,
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
) -> Result<(), String> {
    let mut opts = ufbx::LoadOpts::default();
    opts.generate_missing_normals = true;

    let scene = ufbx::load_file(path, opts)
        .map_err(|e| format!("FBX load error: {} — {}", e.description, e.info()))?;

    let venue = commands.spawn((
        Transform::default(),
        Visibility::default(),
        VenueRoot,
    )).id();

    let mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.5, 0.5, 0.5),
        perceptual_roughness: 0.8,
        metallic: 0.0,
        ..default()
    });

    let mut mesh_count = 0usize;
    for mesh in &scene.meshes {

        if mesh.num_indices == 0 || mesh.num_faces == 0 {
            continue;
        }

        // ufbx gives per-face-corner vertex attributes (num_indices total).
        // Each face corner is already a unique vertex with its own normal/UV.
        let mut positions = Vec::with_capacity(mesh.num_indices);
        let mut normals = Vec::with_capacity(mesh.num_indices);
        let mut uvs = Vec::with_capacity(mesh.num_indices);

        for i in 0..mesh.num_indices {
            let pos = mesh.vertex_position[i];
            positions.push([pos.x as f32, pos.y as f32, pos.z as f32]);

            let normal = if mesh.vertex_normal.exists {
                mesh.vertex_normal[i]
            } else {
                ufbx::Vec3 { x: 0.0, y: 1.0, z: 0.0 }
            };
            normals.push([normal.x as f32, normal.y as f32, normal.z as f32]);

            let uv = if mesh.vertex_uv.exists {
                let uv = mesh.vertex_uv[i];
                [uv.x as f32, uv.y as f32]
            } else {
                [0.0, 0.0]
            };
            uvs.push(uv);
        }

        // Triangulate faces (fan triangulation for n-gons).
        let mut indices = Vec::new();
        for face in mesh.faces.as_ref() {
            let n = face.num_indices as usize;
            if n < 3 {
                continue;
            }
            let base = face.index_begin as usize;
            for i in 1..(n - 1) {
                indices.push(base as u32);
                indices.push((base + i) as u32);
                indices.push((base + i + 1) as u32);
            }
        }

        let mut bevy_mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::RENDER_WORLD);
        bevy_mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
        bevy_mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
        bevy_mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
        bevy_mesh.insert_indices(Indices::U32(indices));

        let child = commands.spawn((
            Mesh3d(meshes.add(bevy_mesh)),
            MeshMaterial3d(mat.clone()),
            Transform::default(),
        )).id();
        commands.entity(venue).add_child(child);
        mesh_count += 1;
    }

    info!("Venue FBX loaded: {} mesh(es) from '{}'", mesh_count, path);
    Ok(())
}

// ─── Normal computation ───────────────────────────────────────────────────────

fn compute_flat_normals(positions: &[[f32; 3]], indices: &[u32]) -> Vec<[f32; 3]> {
    let mut normals = vec![[0.0f32; 3]; positions.len()];
    for tri in indices.chunks(3) {
        if tri.len() < 3 { continue; }
        let (a, b, c) = (
            positions[tri[0] as usize],
            positions[tri[1] as usize],
            positions[tri[2] as usize],
        );
        let ab = [b[0]-a[0], b[1]-a[1], b[2]-a[2]];
        let ac = [c[0]-a[0], c[1]-a[1], c[2]-a[2]];
        let n = normalize([
            ab[1]*ac[2] - ab[2]*ac[1],
            ab[2]*ac[0] - ab[0]*ac[2],
            ab[0]*ac[1] - ab[1]*ac[0],
        ]);
        for &vi in tri { normals[vi as usize] = n; }
    }
    normals
}

fn normalize(v: [f32; 3]) -> [f32; 3] {
    let len = (v[0]*v[0] + v[1]*v[1] + v[2]*v[2]).sqrt();
    if len < 1e-10 { return [0.0, 1.0, 0.0]; }
    [v[0]/len, v[1]/len, v[2]/len]
}
