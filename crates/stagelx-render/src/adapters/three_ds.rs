use bevy::{
    asset::RenderAssetUsages,
    mesh::{Indices, PrimitiveTopology},
    prelude::*,
};

/// Build a Bevy mesh from the first 3DS model embedded in a GDTF fixture type.
///
/// Returns `None` if the fixture type has no embedded models or if parsing fails.
/// Uses flat shading (one normal per face corner) — adequate for fixture bodies.
pub fn mesh_from_gdtf(
    ft: &stagelx_gdtf::gdtf::GdtfFixtureType,
    meshes: &mut Assets<Mesh>,
) -> Option<Handle<Mesh>> {
    let (_, bytes) = ft.models.first()?;
    let scene = ds3::parse(bytes).ok()?;
    let mesh3d = scene.meshes.into_iter().next()?;
    let buf = mesh3d.to_flat_buffers();

    let mut m = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::RENDER_WORLD);
    m.insert_attribute(Mesh::ATTRIBUTE_POSITION, buf.positions);
    m.insert_attribute(Mesh::ATTRIBUTE_NORMAL,   buf.normals);
    m.insert_attribute(Mesh::ATTRIBUTE_UV_0,     buf.uvs);
    m.insert_indices(Indices::U32(buf.indices));
    Some(meshes.add(m))
}
