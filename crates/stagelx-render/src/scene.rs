use bevy::prelude::*;

pub fn setup_scene(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // Stage floor
    commands.spawn((
        Mesh3d(meshes.add(Plane3d::default().mesh().size(40.0, 24.0))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(0.08, 0.08, 0.08),
            perceptual_roughness: 0.9,
            metallic: 0.0,
            ..default()
        })),
        Transform::from_xyz(0.0, -0.01, 0.0),
    ));

    // Truss bar (visual only — fixtures mount to this)
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(18.0, 0.12, 0.12))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(0.4, 0.35, 0.1),
            metallic: 0.6,
            perceptual_roughness: 0.4,
            ..default()
        })),
        Transform::from_xyz(0.0, 6.0, 0.0),
    ));

    // Camera — angled view from front-of-house
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(0.0, 8.0, 22.0).looking_at(Vec3::new(0.0, 2.0, 0.0), Vec3::Y),
    ));

    // Dim ambient so beam lights are visible
    commands.insert_resource(AmbientLight {
        color: Color::srgb(0.04, 0.04, 0.07),
        brightness: 80.0,
    });
}
