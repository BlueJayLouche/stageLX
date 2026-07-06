use bevy::{
    camera::{ScalingMode, Viewport, visibility::RenderLayers},
    core_pipeline::tonemapping::Tonemapping,
    light::GlobalAmbientLight,
    prelude::*,
};
use crate::camera::FohCameraController;
use stagelx_show::EguiViewportRect;

#[derive(Component)]
pub struct FohCamera;

#[derive(Component)]
pub struct TopCamera;

#[derive(Component)]
pub struct SideCamera;

pub fn setup_scene(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // Near-black clear so the stage reads like a dark venue, not an editor grey.
    commands.insert_resource(ClearColor(Color::srgb(0.012, 0.014, 0.018)));

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
            base_color: Color::srgb(0.45, 0.4, 0.15),
            metallic: 0.2,
            perceptual_roughness: 0.5,
            ..default()
        })),
        Transform::from_xyz(0.0, 6.0, 0.0),
    ));

    // FOH perspective camera — main view (left 75%)
    // Layer 0 = scene geometry; layer 4 = FOH-only beam visuals (Tier-2 full-res
    // cones + Tier-0 billboards), kept off the ortho cameras' layers.
    // Viewports start unset; `sync_viewports_to_ui` fits them to the egui
    // central-panel rect as soon as the UI publishes it (and on every resize
    // of the surrounding panels).
    commands.spawn((
        FohCamera,
        Camera3d::default(),
        Camera::default(),
        // Filmic tonemap for the hero view. Deliberately LDR: adding `Hdr`
        // (for Bloom) blacks out this camera — it fights the layer-4 beam
        // composite pipeline, and beam glow is already provided by the
        // ray-marched composite, so HDR bloom buys nothing here.
        Tonemapping::TonyMcMapface,
        RenderLayers::layer(0) | RenderLayers::layer(4),
        FohCameraController::default(),
    ));

    // Top-down orthographic camera — right 25%, upper half
    commands.spawn((
        TopCamera,
        Camera3d::default(),
        Camera {
            order: 1,
            ..default()
        },
        Projection::Orthographic(OrthographicProjection {
            scaling_mode: ScalingMode::FixedVertical {
                viewport_height: 30.0,
            },
            ..OrthographicProjection::default_3d()
        }),
        // Layer 0 = geometry; layer 3 = ray-marched beam cones (all tiers).
        RenderLayers::layer(0) | RenderLayers::layer(3),
        // Positioned straight above; Z is "up" in this 2-D view (stage depth)
        Transform::from_xyz(0.0, 40.0, 0.0).looking_at(Vec3::ZERO, Vec3::Z),
    ));

    // Side orthographic camera (stage right → center) — right 25%, lower half
    commands.spawn((
        SideCamera,
        Camera3d::default(),
        Camera {
            order: 2,
            ..default()
        },
        Projection::Orthographic(OrthographicProjection {
            scaling_mode: ScalingMode::FixedVertical {
                viewport_height: 20.0,
            },
            ..OrthographicProjection::default_3d()
        }),
        // Layer 0 = geometry; layer 3 = ray-marched beam cones (all tiers).
        RenderLayers::layer(0) | RenderLayers::layer(3),
        Transform::from_xyz(40.0, 3.0, 0.0).looking_at(Vec3::new(0.0, 3.0, 0.0), Vec3::Y),
    ));

    // Ambient lift so the stage geometry and fixtures read off black.
    //
    // NOTE: In Bevy 0.18 `AmbientLight` is a `#[require(Camera)]` component —
    // spawning it on its own entity does nothing (it just creates an orphan
    // camera). The scene-wide ambient is the `GlobalAmbientLight` resource.
    // The additive beam cones use an unlit custom material, so raising ambient
    // brightens the geometry without washing out the beams.
    // The camera uses Bevy's default (Blender, EV100 9.7) exposure tuned for
    // daylight, so stage-scale light needs to be in the thousands of lux to
    // register. Floor albedo is low (0.08) to keep the stage dark; fixtures use
    // higher albedo so they read several times brighter than the floor.
    commands.insert_resource(GlobalAmbientLight {
        color: Color::srgb(0.7, 0.7, 0.8),
        brightness: 1800.0,
        affects_lightmapped_meshes: true,
    });

    // Key light from front-above. Metallic fixture bodies have almost no diffuse
    // response, so ambient alone leaves them flat — a directional light gives
    // them specular shading and reads them as solid 3-D objects.
    commands.spawn((
        DirectionalLight {
            color: Color::srgb(0.95, 0.95, 1.0),
            illuminance: 5000.0,
            shadow_maps_enabled: false,
            ..default()
        },
        Transform::from_xyz(6.0, 12.0, 10.0).looking_at(Vec3::new(0.0, 4.0, 0.0), Vec3::Y),
    ));
}

/// Fit the three camera viewports to the egui central-panel rect published by
/// the UI each frame. This is what keeps the 3-D views aligned with the
/// resizable panels — whatever the user drags, the cameras follow.
#[allow(clippy::type_complexity)]
pub fn sync_viewports_to_ui(
    ui_rect: Res<EguiViewportRect>,
    windows: Query<&Window>,
    mut cameras: Query<(
        &mut Camera,
        Option<&FohCamera>,
        Option<&TopCamera>,
        Option<&SideCamera>,
    )>,
) {
    if !ui_rect.valid {
        return;
    }
    let Ok(window) = windows.single() else {
        return;
    };
    let (pw, ph) = (window.physical_width(), window.physical_height());
    if pw == 0 || ph == 0 {
        return;
    }
    let sf = window.scale_factor();

    // egui rect is in logical points; camera viewports are physical pixels.
    let x0 = ((ui_rect.min_x * sf).round().max(0.0) as u32).min(pw);
    let y0 = ((ui_rect.min_y * sf).round().max(0.0) as u32).min(ph);
    let x1 = ((ui_rect.max_x * sf).round().max(0.0) as u32).min(pw);
    let y1 = ((ui_rect.max_y * sf).round().max(0.0) as u32).min(ph);
    let vp_w = x1.saturating_sub(x0);
    let vp_h = y1.saturating_sub(y0);
    if vp_w < 16 || vp_h < 16 {
        return;
    }

    // Same splits the UI overlay draws: FOH left 75%, TOP/SIDE stacked right.
    let foh_w = (vp_w as f32 * 0.75) as u32;
    let mini_w = vp_w - foh_w;
    let mini_h = vp_h / 2;

    let foh_vp = Viewport {
        physical_position: UVec2::new(x0, y0),
        physical_size: UVec2::new(foh_w, vp_h),
        depth: 0.0..1.0,
    };
    let top_vp = Viewport {
        physical_position: UVec2::new(x0 + foh_w, y0),
        physical_size: UVec2::new(mini_w, mini_h),
        depth: 0.0..1.0,
    };
    let side_vp = Viewport {
        physical_position: UVec2::new(x0 + foh_w, y0 + mini_h),
        physical_size: UVec2::new(mini_w, vp_h - mini_h),
        depth: 0.0..1.0,
    };

    for (mut cam, foh, top, side) in &mut cameras {
        let desired = if foh.is_some() {
            &foh_vp
        } else if top.is_some() {
            &top_vp
        } else if side.is_some() {
            &side_vp
        } else {
            continue;
        };
        // Only write on change — Camera is change-detected and a write forces
        // render-graph work.
        let unchanged = cam.viewport.as_ref().is_some_and(|v| {
            v.physical_position == desired.physical_position
                && v.physical_size == desired.physical_size
        });
        if !unchanged {
            cam.viewport = Some(desired.clone());
        }
    }
}
