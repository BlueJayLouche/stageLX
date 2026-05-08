use bevy::{prelude::*, camera::{ScalingMode, Viewport}, window::WindowResized};
use crate::camera::FohCameraController;

#[derive(Component)]
pub struct FohCamera;

#[derive(Component)]
pub struct TopCamera;

#[derive(Component)]
pub struct SideCamera;

fn foh_viewport(w: u32, h: u32) -> Viewport {
    Viewport {
        physical_position: UVec2::ZERO,
        physical_size: UVec2::new(w * 3 / 4, h),
        depth: 0.0..1.0,
    }
}

fn top_viewport(w: u32, h: u32) -> Viewport {
    Viewport {
        physical_position: UVec2::new(w * 3 / 4, 0),
        physical_size: UVec2::new(w / 4, h / 2),
        depth: 0.0..1.0,
    }
}

fn side_viewport(w: u32, h: u32) -> Viewport {
    Viewport {
        physical_position: UVec2::new(w * 3 / 4, h / 2),
        physical_size: UVec2::new(w / 4, h / 2),
        depth: 0.0..1.0,
    }
}

pub fn setup_scene(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    windows: Query<&Window>,
) {
    let (w, h) = windows
        .single()
        .map(|win| (win.physical_width(), win.physical_height()))
        .unwrap_or((1600, 900));

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

    // FOH perspective camera — main view (left 75%)
    commands.spawn((
        FohCamera,
        Camera3d::default(),
        Camera {
            viewport: Some(foh_viewport(w, h)),
            ..default()
        },
        FohCameraController::default(),
    ));

    // Top-down orthographic camera — right 25%, upper half
    commands.spawn((
        TopCamera,
        Camera3d::default(),
        Camera {
            viewport: Some(top_viewport(w, h)),
            order: 1,
            ..default()
        },
        Projection::Orthographic(OrthographicProjection {
            scaling_mode: ScalingMode::FixedVertical {
                viewport_height: 30.0,
            },
            ..OrthographicProjection::default_3d()
        }),
        // Positioned straight above; Z is "up" in this 2-D view (stage depth)
        Transform::from_xyz(0.0, 40.0, 0.0).looking_at(Vec3::ZERO, Vec3::Z),
    ));

    // Side orthographic camera (stage right → center) — right 25%, lower half
    commands.spawn((
        SideCamera,
        Camera3d::default(),
        Camera {
            viewport: Some(side_viewport(w, h)),
            order: 2,
            ..default()
        },
        Projection::Orthographic(OrthographicProjection {
            scaling_mode: ScalingMode::FixedVertical {
                viewport_height: 20.0,
            },
            ..OrthographicProjection::default_3d()
        }),
        Transform::from_xyz(40.0, 3.0, 0.0).looking_at(Vec3::new(0.0, 3.0, 0.0), Vec3::Y),
    ));

    // Dim ambient so beam lights are visible (Bevy 0.18: component, not Resource)
    commands.spawn(AmbientLight {
        color: Color::srgb(0.04, 0.04, 0.07),
        brightness: 80.0,
        affects_lightmapped_meshes: true,
    });
}

pub fn update_viewports_on_resize(
    mut resize_events: MessageReader<WindowResized>,
    windows: Query<&Window>,
    mut cameras: Query<(
        &mut Camera,
        Option<&FohCamera>,
        Option<&TopCamera>,
        Option<&SideCamera>,
    )>,
) {
    if resize_events.is_empty() {
        return;
    }
    resize_events.clear();

    let Ok(window) = windows.single() else {
        return;
    };
    let w = window.physical_width();
    let h = window.physical_height();
    if w == 0 || h == 0 {
        return;
    }

    for (mut cam, foh, top, side) in &mut cameras {
        cam.viewport = Some(if foh.is_some() {
            foh_viewport(w, h)
        } else if top.is_some() {
            top_viewport(w, h)
        } else if side.is_some() {
            side_viewport(w, h)
        } else {
            continue;
        });
    }
}
