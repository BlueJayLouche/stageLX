use bevy::{
    camera::{RenderTarget, Viewport, visibility::RenderLayers},
    prelude::*,
    render::{camera::CameraRenderGraph, render_resource::{AsBindGroup, TextureFormat}},
    shader::ShaderRef,
    window::WindowResized,
};
use bevy::core_pipeline::Core3d;

use std::collections::HashMap;

use stagelx_core::types::FixtureId;
use stagelx_show::PerfDiagnosticsRes;

use crate::{
    beam::BeamMaterial,
    beam_sprite::BeamSprite,
    fixture::BeamCone,
    scene::FohCamera,
};

// ─── Constants ────────────────────────────────────────────────────────────────

/// Screen-space radius threshold (px) between Tier 0 and Tier 1.
const TIER_1_THRESHOLD_PX: f32 = 50.0;
/// Screen-space radius threshold (px) between Tier 1 and Tier 2.
const TIER_2_THRESHOLD_PX: f32 = 200.0;
/// Hysteresis band (px) to prevent per-frame tier flicker.
const HYSTERESIS_PX: f32 = 10.0;
/// Maximum number of beams that may ray-march (Tier 1 + Tier 2).
const RAY_MARCH_HARD_CAP: usize = 64;

// ─── Components ───────────────────────────────────────────────────────────────

#[derive(Component, Clone, Copy, PartialEq, Eq, Debug)]
pub enum BeamLodTier {
    /// Billboard sprite — zero ray-march cost.
    Tier0,
    /// Half-res offscreen ray march (16 steps).
    Tier1,
    /// Full-res ray march (32 steps).
    Tier2,
}

/// Marker for the half-resolution beam camera.
#[derive(Component)]
pub struct BeamHalfResCamera;

/// Marker for the fullscreen composite quad that adds the half-res beam pass.
#[derive(Component)]
pub struct BeamCompositeQuad;

// ─── Resources ────────────────────────────────────────────────────────────────

/// Owns the half-res render target handle so systems can reference it.
#[derive(Resource)]
pub struct BeamRenderTarget {
    pub half_res: Handle<Image>,
}

// ─── Composite material ───────────────────────────────────────────────────────

/// Fullscreen quad material that samples the half-res beam texture with additive blending.
#[derive(Asset, TypePath, AsBindGroup, Clone)]
pub struct BeamCompositeMaterial {
    #[texture(0)]
    #[sampler(1)]
    pub beam_texture: Handle<Image>,
}

impl Material for BeamCompositeMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/beam_composite.wgsl".into()
    }
    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Add
    }
    fn specialize(
        _pipeline: &bevy::pbr::MaterialPipeline,
        descriptor: &mut bevy::render::render_resource::RenderPipelineDescriptor,
        _layout: &bevy::mesh::MeshVertexBufferLayoutRef,
        _key: bevy::pbr::MaterialPipelineKey<Self>,
    ) -> Result<(), bevy::render::render_resource::SpecializedMeshPipelineError> {
        descriptor.primitive.cull_mode = None;
        Ok(())
    }
}

// ─── Startup: create render target, beam camera, composite quad ───────────────

pub fn setup_beam_lod(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<BeamCompositeMaterial>>,
    windows: Query<&Window>,
) {
    let window = windows.single().expect("primary window");
    let (w, h) = (window.physical_width(), window.physical_height());
    let (half_w, half_h) = ((w / 2).max(1), (h / 2).max(1));

    let image = Image::new_target_texture(
        half_w,
        half_h,
        TextureFormat::Rgba16Float,
        None,
    );

    let half_res_handle: Handle<Image> = images.add(image);

    commands.insert_resource(BeamRenderTarget {
        half_res: half_res_handle.clone(),
    });

    // Half-res beam camera (layer 1 only).
    // Spawn CameraRenderGraph first to avoid the on_add hook warning on Camera.
    let beam_cam = commands.spawn((
        BeamHalfResCamera,
        CameraRenderGraph::new(Core3d),
    )).id();
    commands.entity(beam_cam).insert((
        Camera3d::default(),
        Camera {
            order: -1,
            clear_color: ClearColorConfig::Custom(Color::NONE),
            viewport: Some(Viewport {
                physical_position: UVec2::ZERO,
                physical_size: UVec2::new(half_w, half_h),
                depth: 0.0..1.0,
            }),
            ..default()
        },
        RenderTarget::Image(half_res_handle.clone().into()),
        RenderLayers::layer(1),
        Transform::default(),
    ));

    // Fullscreen composite quad (layer 0, main camera only).
    // Size is arbitrary — it will be rescaled each frame to fill the viewport.
    let quad_mesh = meshes.add(bevy::math::primitives::Rectangle::new(1.0, 1.0));
    let composite_mat = materials.add(BeamCompositeMaterial {
        beam_texture: half_res_handle,
    });

    commands.spawn((
        BeamCompositeQuad,
        Mesh3d(quad_mesh),
        MeshMaterial3d(composite_mat),
        Visibility::default(),
        RenderLayers::layer(0),
    ));
}

// ─── System: sync beam camera + composite quad to FOH camera ──────────────────

#[allow(clippy::type_complexity)]
pub fn sync_beam_camera_to_foh(
    foh_q: Query<&Transform, With<FohCamera>>,
    mut beam_cam_q: Query<&mut Transform, (With<BeamHalfResCamera>, Without<FohCamera>)>,
    mut composite_q: Query<
        &mut Transform,
        (With<BeamCompositeQuad>, Without<FohCamera>, Without<BeamHalfResCamera>),
    >,
    windows: Query<&Window>,
    projection_q: Query<&Projection, With<FohCamera>>,
) {
    let Ok(foh_tf) = foh_q.single() else { return };
    let Ok(window) = windows.single() else { return };

    // Sync beam camera transform.
    for mut tf in &mut beam_cam_q {
        *tf = *foh_tf;
    }

    // Position composite quad just in front of the near plane.
    let near = 0.11; // slightly beyond default near plane
    let forward = foh_tf.forward();
    let pos = foh_tf.translation + forward * near;

    // Scale quad to exactly fill viewport at `near` distance.
    let aspect = if window.height() > 0.0 {
        window.width() / window.height()
    } else {
        16.0 / 9.0
    };

    // Extract FOV from projection.
    let fov_y = match projection_q.single() {
        Ok(Projection::Perspective(p)) => p.fov,
        _ => std::f32::consts::FRAC_PI_3,
    };

    let half_h = near * (fov_y * 0.5).tan();
    let half_w = half_h * aspect;

    for mut tf in &mut composite_q {
        tf.translation = pos;
        tf.rotation = foh_tf.rotation;
        tf.scale = Vec3::new(half_w * 2.0, half_h * 2.0, 1.0);
    }
}

// ─── System: evaluate LOD tiers per frame ─────────────────────────────────────

pub fn evaluate_beam_lod(
    windows: Query<&Window>,
    foh_q: Query<(&Transform, &Projection), With<FohCamera>>,
    beam_q: Query<(Entity, &GlobalTransform, &BeamCone, Option<&BeamLodTier>)>,
    mut commands: Commands,
    mut scored: Local<Vec<(Entity, f32, BeamLodTier)>>,
    mut perf: ResMut<PerfDiagnosticsRes>,
) {
    let t0 = std::time::Instant::now();

    let Ok(window) = windows.single() else {
        perf.beam_lod_eval_ms = t0.elapsed().as_secs_f32() * 1000.0;
        return;
    };
    let Ok((foh_tf, foh_proj)) = foh_q.single() else {
        perf.beam_lod_eval_ms = t0.elapsed().as_secs_f32() * 1000.0;
        return;
    };

    let fov_y = match foh_proj {
        Projection::Perspective(p) => p.fov,
        _ => std::f32::consts::FRAC_PI_3,
    };
    let vp_h = window.physical_height() as f32;

    // 1. Compute screen-space radius for every beam.
    scored.clear();
    scored.reserve(beam_q.iter().len());
    for (entity, global_tf, _beam, current_tier) in &beam_q {
        let pos = global_tf.translation();
        let dist = pos.distance(foh_tf.translation);
        if dist < 1e-3 {
            continue;
        }

        // Approximate base radius from beam cone scale.
        // The cone mesh has base radius proportional to its X/Z scale.
        let scale = global_tf.scale();
        let approx_base_radius = scale.x.max(scale.z) * 0.5;

        let angular_radius = (approx_base_radius / dist).atan();
        let pixel_radius = angular_radius / (fov_y * 0.5).tan() * (vp_h * 0.5);

        // Apply hysteresis to avoid per-frame tier churn.
        let tier = match current_tier {
            Some(BeamLodTier::Tier0) => {
                if pixel_radius > TIER_1_THRESHOLD_PX + HYSTERESIS_PX {
                    if pixel_radius > TIER_2_THRESHOLD_PX + HYSTERESIS_PX {
                        BeamLodTier::Tier2
                    } else {
                        BeamLodTier::Tier1
                    }
                } else {
                    BeamLodTier::Tier0
                }
            }
            Some(BeamLodTier::Tier1) => {
                if pixel_radius < TIER_1_THRESHOLD_PX - HYSTERESIS_PX {
                    BeamLodTier::Tier0
                } else if pixel_radius > TIER_2_THRESHOLD_PX + HYSTERESIS_PX {
                    BeamLodTier::Tier2
                } else {
                    BeamLodTier::Tier1
                }
            }
            Some(BeamLodTier::Tier2) => {
                if pixel_radius < TIER_2_THRESHOLD_PX - HYSTERESIS_PX {
                    if pixel_radius < TIER_1_THRESHOLD_PX - HYSTERESIS_PX {
                        BeamLodTier::Tier0
                    } else {
                        BeamLodTier::Tier1
                    }
                } else {
                    BeamLodTier::Tier2
                }
            }
            None => {
                if pixel_radius < TIER_1_THRESHOLD_PX {
                    BeamLodTier::Tier0
                } else if pixel_radius < TIER_2_THRESHOLD_PX {
                    BeamLodTier::Tier1
                } else {
                    BeamLodTier::Tier2
                }
            }
        };

        scored.push((entity, pixel_radius, tier));
    }

    // 2. Enforce hard cap: only top 64 by pixel radius may be Tier 1/2.
    scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    let mut ray_march_count = 0usize;
    for (_entity, _radius, tier) in &mut scored {
        if *tier != BeamLodTier::Tier0 {
            if ray_march_count >= RAY_MARCH_HARD_CAP {
                *tier = BeamLodTier::Tier0;
            } else {
                ray_march_count += 1;
            }
        }
    }

    // 3. Write tier components.
    for (entity, _radius, tier) in scored.drain(..) {
        commands.entity(entity).insert(tier);
    }

    perf.beam_lod_eval_ms = t0.elapsed().as_secs_f32() * 1000.0;
}

// ─── System: apply LOD tier (visibility, render layers, step count) ───────────

#[allow(clippy::too_many_arguments)]
#[allow(clippy::type_complexity)]
pub fn apply_beam_lod(
    mut beam_cones: Query<
        (Entity, &BeamCone, &BeamLodTier, &mut Visibility, &MeshMaterial3d<BeamMaterial>),
        Without<BeamSprite>,
    >,
    beam_sprites: Query<(Entity, &BeamSprite, &mut Visibility), Without<BeamCone>>,
    mut half_res_cam: Query<&mut Camera, With<BeamHalfResCamera>>,
    mut composite_vis: Query<
        &mut Visibility,
        (With<BeamCompositeQuad>, Without<BeamCone>, Without<BeamSprite>),
    >,
    mut beam_materials: ResMut<Assets<BeamMaterial>>,
    mut commands: Commands,
    mut sprite_by_id: Local<HashMap<FixtureId, Entity>>,
    mut rebuild_lookup: Local<bool>,
    added_sprites: Query<Entity, Added<BeamSprite>>,
    mut removed_sprites: RemovedComponents<BeamSprite>,
    mut perf: ResMut<PerfDiagnosticsRes>,
) {
    let t0 = std::time::Instant::now();
    // Rebuild sprite lookup only when sprites are added or removed.
    let has_removed = removed_sprites.read().next().is_some();
    if !added_sprites.is_empty() || has_removed {
        *rebuild_lookup = true;
    }
    if *rebuild_lookup {
        sprite_by_id.clear();
        for (entity, sprite, _vis) in &beam_sprites {
            sprite_by_id.insert(sprite.id, entity);
        }
        *rebuild_lookup = false;
    }

    let mut any_tier1 = false;

    for (entity, cone, tier, mut vis, mat_handle) in &mut beam_cones {
        any_tier1 |= *tier == BeamLodTier::Tier1;
        // Cones are always visible; which cameras draw them is controlled purely
        // by render layers:
        //   layer 1 — FOH half-res target (Tier 1)
        //   layer 4 — FOH full-res, main camera only (Tier 2)
        //   layer 3 — ortho TOP/SIDE previews (all tiers, ray-marched)
        *vis = Visibility::Visible;

        let cone_layers = match tier {
            // Tier 0: FOH shows a cheap billboard, so the cone is ortho-only.
            BeamLodTier::Tier0 => RenderLayers::layer(3),
            BeamLodTier::Tier1 => RenderLayers::layer(1) | RenderLayers::layer(3),
            BeamLodTier::Tier2 => RenderLayers::layer(4) | RenderLayers::layer(3),
        };
        commands.entity(entity).insert(cone_layers);

        // Set step count on material.
        if let Some(mut mat) = beam_materials.get_mut(mat_handle) {
            mat.step_count = match tier {
                BeamLodTier::Tier1 => 16,
                BeamLodTier::Tier2 => 32,
                BeamLodTier::Tier0 => 16,
            };
        }

        // The FOH billboard is only the Tier-0 visual (Tier 1/2 use the cone in
        // FOH too). It lives on layer 4 so it never leaks into the ortho views,
        // which now render the ray-marched cone instead of a sprite.
        if let Some(&sprite_entity) = sprite_by_id.get(&cone.id) {
            match tier {
                BeamLodTier::Tier0 => {
                    commands.entity(sprite_entity).insert(Visibility::Visible);
                    commands.entity(sprite_entity).insert(RenderLayers::layer(4));
                }
                BeamLodTier::Tier1 | BeamLodTier::Tier2 => {
                    commands.entity(sprite_entity).insert(Visibility::Hidden);
                }
            }
        }
    }

    // With no Tier-1 beams the half-res pass has nothing to draw — deactivate
    // the camera (skips its whole Core3d graph) and hide the composite quad
    // (its texture holds a stale frame once the camera stops rendering).
    // Guarded writes: Camera is change-detected and a write forces render work.
    for mut cam in &mut half_res_cam {
        if cam.is_active != any_tier1 {
            cam.is_active = any_tier1;
        }
    }
    let quad_vis = if any_tier1 { Visibility::Visible } else { Visibility::Hidden };
    for mut vis in &mut composite_vis {
        if *vis != quad_vis {
            *vis = quad_vis;
        }
    }

    perf.beam_lod_apply_ms = t0.elapsed().as_secs_f32() * 1000.0;
}

// ─── System: sort beams front-to-back for additive-phase early-Z ──────────────

/// Updates each beam material's `depth_bias` so that Bevy's transparent-phase
/// ascending sort renders closer beams first (front-to-back).
///
/// Bevy defaults to back-to-front for all transparent meshes. For additive
/// blending this is sub-optimal because it maximizes overdraw. Front-to-back
/// lets the GPU's early-z reject beam fragments occluded by opaque venue
/// geometry sooner.
///
/// The bias is only written when it changes by more than `BIAS_EPSILON` to
/// avoid marking material assets dirty every frame.
const BIAS_EPSILON: f32 = 0.5;

pub fn sort_beams_front_to_back(
    foh_q: Query<&Transform, With<FohCamera>>,
    beam_q: Query<(&MeshMaterial3d<BeamMaterial>, &GlobalTransform), With<BeamCone>>,
    mut beam_materials: ResMut<Assets<BeamMaterial>>,
    mut last_bias: Local<HashMap<AssetId<BeamMaterial>, f32>>,
    mut perf: ResMut<PerfDiagnosticsRes>,
) {
    let t0 = std::time::Instant::now();

    let Ok(foh_tf) = foh_q.single() else {
        perf.beam_sort_ms = t0.elapsed().as_secs_f32() * 1000.0;
        return;
    };
    let cam_pos = foh_tf.translation;
    let cam_forward: Vec3 = foh_tf.forward().into();

    for (mat_handle, global_tf) in &beam_q {
        let beam_pos = global_tf.translation();
        // view_z matches the sign convention of Bevy's ViewRangefinder3d:
        // negative = in front of camera, positive = behind.
        let view_z = (beam_pos - cam_pos).dot(cam_forward);
        let desired_bias = -2.0 * view_z;

        let id = mat_handle.id();
        let should_update = last_bias
            .get(&id)
            .map(|&last| (desired_bias - last).abs() > BIAS_EPSILON)
            .unwrap_or(true);

        if should_update {
            if let Some(mut mat) = beam_materials.get_mut(id) {
                mat.depth_bias = desired_bias;
            }
            last_bias.insert(id, desired_bias);
        }
    }

    perf.beam_sort_ms = t0.elapsed().as_secs_f32() * 1000.0;
}

// ─── System: resize beam render target on window resize ───────────────────────

pub fn resize_beam_render_target(
    mut events: MessageReader<WindowResized>,
    mut images: ResMut<Assets<Image>>,
    mut beam_render_target: ResMut<BeamRenderTarget>,
    mut beam_cam_q: Query<(&mut Camera, &mut RenderTarget), With<BeamHalfResCamera>>,
    mut composite_q: Query<&mut MeshMaterial3d<BeamCompositeMaterial>, With<BeamCompositeQuad>>,
    mut materials: ResMut<Assets<BeamCompositeMaterial>>,
) {
    for event in events.read() {
        let (w, h) = (event.width as u32, event.height as u32);
        let (half_w, half_h) = ((w / 2).max(1), (h / 2).max(1));

        let new_image = Image::new_target_texture(
            half_w,
            half_h,
            TextureFormat::Rgba16Float,
            None,
        );
        let new_handle = images.add(new_image);

        // Update beam camera render target and viewport.
        for (mut cam, mut render_target) in &mut beam_cam_q {
            *render_target = RenderTarget::Image(new_handle.clone().into());
            cam.viewport = Some(Viewport {
                physical_position: UVec2::ZERO,
                physical_size: UVec2::new(half_w, half_h),
                depth: 0.0..1.0,
            });
        }

        // Update composite quad material.
        for mat_handle in &mut composite_q {
            if let Some(mut mat) = materials.get_mut(&*mat_handle) {
                mat.beam_texture = new_handle.clone();
            }
        }

        // Replace the old handle in the resource (old image will be GC'd by Bevy).
        beam_render_target.half_res = new_handle;
    }
}
