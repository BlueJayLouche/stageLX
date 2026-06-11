use bevy::prelude::*;
use bevy::camera::visibility::RenderLayers;
use stagelx_core::types::FixtureId;
use stagelx_patch::{PatchRes, SpawnFixtureEvent, DespawnFixtureEvent};
use stagelx_show::{FixtureLibraryRes, PerfDiagnosticsRes, Programmer};
use crate::beam::{BeamMaterial, GoboLibrary, build_beam_cone};
use crate::beam_sprite::{BeamSprite, BeamSpriteTop, BeamSpriteMaterial, build_beam_sprite_quad};

const BEAM_HEIGHT: f32 = 18.0;
const LENS_OFFSET: f32 = 0.14;

// ─── Components ───────────────────────────────────────────────────────────────

#[derive(Component)]
pub struct FixtureVisual {
    pub id: FixtureId,
}

/// Marks the yoke entity — rotates around the world Y-axis for Pan.
#[derive(Component)]
pub struct YokeJoint {
    pub id: FixtureId,
    pub pan_range: f32,
}

/// Marks the head entity — rotates around its local X-axis for Tilt.
#[derive(Component)]
pub struct HeadJoint {
    pub id: FixtureId,
    pub tilt_range: f32,
}

/// Marks the beam point-light entity.
#[derive(Component)]
pub struct BeamSource {
    pub id: FixtureId,
}

/// Marks the additive-blended cone mesh that renders the visible beam shaft.
#[derive(Component)]
pub struct BeamCone {
    pub id: FixtureId,
}

// ─── Spawn config ─────────────────────────────────────────────────────────────

pub struct FixtureSpawnConfig {
    pub id: FixtureId,
    pub position: Vec3,
    pub suspended: bool,
    pub pan_range: f32,
    pub tilt_range: f32,
    pub beam_angle_deg: f32,
    /// Pre-built Bevy mesh from GDTF 3DS model data; None = use procedural cuboid.
    pub body_mesh: Option<Handle<Mesh>>,
}

impl Default for FixtureSpawnConfig {
    fn default() -> Self {
        Self {
            id: FixtureId(0),
            position: Vec3::ZERO,
            suspended: true,
            pan_range: 540.0,
            tilt_range: 270.0,
            beam_angle_deg: 10.0,
            body_mesh: None,
        }
    }
}

// ─── Observers (Bevy 0.18 event pattern) ─────────────────────────────────────

/// Observer: spawns the 3D entity tree when a fixture is added to the patch.
#[allow(clippy::too_many_arguments)]
pub fn on_fixture_spawned(
    trigger: On<SpawnFixtureEvent>,
    patch: Res<PatchRes>,
    library: Res<FixtureLibraryRes>,
    gobo_library: Res<GoboLibrary>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut beam_materials: ResMut<Assets<BeamMaterial>>,
    mut sprite_materials: ResMut<Assets<BeamSpriteMaterial>>,
    mut perf: Option<ResMut<stagelx_show::PerfDiagnosticsRes>>,
) {
    let start = std::time::Instant::now();
    let id = trigger.event().0;
    let Some(inst) = patch.0.get(id) else { return };

    let open_gobo = gobo_library.handles[0].clone();
    let position  = Vec3::from(inst.position);

    // Derive geometry parameters and optional 3DS body mesh from GDTF.
    let (pan_range, tilt_range, beam_angle_deg, body_mesh) =
        if let Some(ft) = library.library.get(&inst.fixture_type_id) {
            let pan_range = ft.find_mode(&inst.dmx_mode)
                .and_then(|m| m.channel_for("Pan"))
                .map(|ch| (ch.physical_to - ch.physical_from).abs())
                .unwrap_or(540.0);

            let tilt_range = ft.find_mode(&inst.dmx_mode)
                .and_then(|m| m.channel_for("Tilt"))
                .map(|ch| (ch.physical_to - ch.physical_from).abs())
                .unwrap_or(270.0);

            let body_mesh = crate::adapters::three_ds::mesh_from_gdtf(ft, &mut meshes);

            (pan_range, tilt_range, ft.beam_angle(), body_mesh)
        } else {
            (540.0, 270.0, 10.0, None)
        };

    spawn_fixture(
        &mut commands,
        &mut meshes,
        &mut materials,
        &mut beam_materials,
        &mut sprite_materials,
        FixtureSpawnConfig { id, position, suspended: true, pan_range, tilt_range, beam_angle_deg, body_mesh },
        open_gobo,
    );

    if let Some(ref mut p) = perf {
        p.last_fixture_spawn_ms = start.elapsed().as_secs_f32() * 1000.0;
        p.fixtures_spawned += 1;
    }
}

/// Observer: despawns the entity tree when a fixture is removed from the patch.
pub fn on_fixture_despawned(
    trigger: On<DespawnFixtureEvent>,
    query: Query<(Entity, &FixtureVisual)>,
    mut commands: Commands,
) {
    let target = trigger.event().0;
    for (entity, vis) in &query {
        if vis.id == target {
            commands.entity(entity).despawn();
        }
    }
}

// ─── Low-level spawn ──────────────────────────────────────────────────────────

pub fn spawn_fixture(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    beam_materials: &mut Assets<BeamMaterial>,
    sprite_materials: &mut Assets<BeamSpriteMaterial>,
    cfg: FixtureSpawnConfig,
    open_gobo: Handle<Image>,
) {
    // Lowish metallic so the bodies take diffuse light from the ambient/key
    // lights. Fully-metallic surfaces with no environment map render black.
    let body_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.32, 0.33, 0.36),
        metallic: 0.25,
        perceptual_roughness: 0.5,
        ..default()
    });
    let joint_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.22, 0.22, 0.24),
        metallic: 0.3,
        perceptual_roughness: 0.45,
        ..default()
    });

    let body_mesh = cfg.body_mesh.unwrap_or_else(|| meshes.add(Cuboid::new(0.30, 0.25, 0.30)));
    let yoke_mesh = meshes.add(Cuboid::new(0.35, 0.08, 0.08));
    let head_mesh = meshes.add(Cuboid::new(0.22, 0.28, 0.22));

    let yoke_y = if cfg.suspended { -0.18 } else { 0.18 };
    let head_y = if cfg.suspended { -0.22 } else { 0.22 };

    let half_angle  = (cfg.beam_angle_deg * 0.5).to_radians();
    let beam_radius = BEAM_HEIGHT * half_angle.tan();

    let (cone_y, cone_rot) = if cfg.suspended {
        (-(LENS_OFFSET + BEAM_HEIGHT * 0.5), Quat::IDENTITY)
    } else {
        (LENS_OFFSET + BEAM_HEIGHT * 0.5, Quat::from_rotation_x(std::f32::consts::PI))
    };

    let cone_mesh = meshes.add(build_beam_cone(beam_radius, BEAM_HEIGHT));
    let beam_mat  = beam_materials.add(BeamMaterial {
        color: LinearRgba::WHITE,
        gobo_params: Vec4::ZERO,
        gobo: open_gobo.clone(),
        beam_params: Vec4::new(half_angle, BEAM_HEIGHT, 2.0, 0.8),
        world_to_cone: Mat4::IDENTITY,
        step_count: 16,
        depth_bias: 0.0,
    });

    let sprite_mesh = meshes.add(build_beam_sprite_quad(beam_radius * 2.0));
    let sprite_mat = sprite_materials.add(BeamSpriteMaterial {
        color: LinearRgba::WHITE,
        sprite_params: Vec4::new(0.0, 2.0, 0.0, 0.0),
        gobo: open_gobo.clone(),
    });
    let top_sprite_mat = sprite_materials.add(BeamSpriteMaterial {
        color: LinearRgba::WHITE,
        sprite_params: Vec4::new(0.0, 2.0, 0.0, 0.0),
        gobo: open_gobo,
    });

    commands
        .spawn((
            Mesh3d(body_mesh),
            MeshMaterial3d(body_mat.clone()),
            Transform::from_translation(cfg.position),
            FixtureVisual { id: cfg.id },
        ))
        .with_children(|body| {
            body.spawn((
                Mesh3d(yoke_mesh),
                MeshMaterial3d(joint_mat.clone()),
                Transform::from_xyz(0.0, yoke_y, 0.0),
                YokeJoint { id: cfg.id, pan_range: cfg.pan_range },
            ))
            .with_children(|yoke| {
                yoke.spawn((
                    Mesh3d(head_mesh),
                    MeshMaterial3d(joint_mat),
                    Transform::from_xyz(0.0, head_y, 0.0),
                    HeadJoint { id: cfg.id, tilt_range: cfg.tilt_range },
                ))
                .with_children(|head| {
                    head.spawn((
                        PointLight {
                            intensity: 0.0,
                            color: Color::WHITE,
                            range: 40.0,
                            shadows_enabled: false,
                            ..default()
                        },
                        Transform::from_xyz(0.0, if cfg.suspended { -0.18 } else { 0.18 }, 0.0),
                        BeamSource { id: cfg.id },
                    ));
                    head.spawn((
                        Mesh3d(cone_mesh),
                        MeshMaterial3d(beam_mat),
                        Transform::from_xyz(0.0, cone_y, 0.0).with_rotation(cone_rot),
                        BeamCone { id: cfg.id },
                    ));
                    // Main sprite — visible in FOH (Tier 0) and side view
                    head.spawn((
                        Mesh3d(sprite_mesh.clone()),
                        MeshMaterial3d(sprite_mat),
                        Transform::from_xyz(0.0, cone_y, 0.0).with_rotation(cone_rot),
                        BeamSprite { id: cfg.id },
                        Visibility::Hidden,
                    ));
                    // Top-view sprite — perpendicular to main sprite, always visible in ortho views
                    head.spawn((
                        Mesh3d(sprite_mesh),
                        MeshMaterial3d(top_sprite_mat),
                        Transform::from_xyz(0.0, cone_y, 0.0)
                            .with_rotation(cone_rot * Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)),
                        BeamSpriteTop { id: cfg.id },
                        Visibility::Visible,
                        RenderLayers::layer(2),
                    ));
                });
            });
        });
}

// ─── Articulation systems ─────────────────────────────────────────────────────

pub fn articulate_fixtures(
    programmer: Res<Programmer>,
    mut yoke_q: Query<(&YokeJoint, &mut Transform), Without<HeadJoint>>,
    mut head_q: Query<(&HeadJoint, &mut Transform), Without<YokeJoint>>,
) {
    for (yoke, mut transform) in &mut yoke_q {
        let pan = programmer.values_for(yoke.id).pan;
        let pan_deg = (pan - 0.5) * yoke.pan_range;
        transform.rotation = Quat::from_rotation_y(pan_deg.to_radians());
    }
    for (head, mut transform) in &mut head_q {
        let tilt = programmer.values_for(head.id).tilt;
        let tilt_deg = (tilt - 0.5) * head.tilt_range;
        transform.rotation = Quat::from_rotation_x(tilt_deg.to_radians());
    }
}

#[allow(clippy::too_many_arguments)]
#[allow(clippy::type_complexity)]
pub fn articulate_beams(
    programmer: Res<Programmer>,
    time: Res<Time>,
    mut beam_q: Query<
        (&BeamCone, &MeshMaterial3d<BeamMaterial>, &mut Transform, Ref<GlobalTransform>),
        (Without<YokeJoint>, Without<HeadJoint>, Without<BeamSprite>, Without<BeamSpriteTop>),
    >,
    mut sprite_q: Query<(&BeamSprite, &MeshMaterial3d<BeamSpriteMaterial>, &mut Transform, &GlobalTransform), (Without<BeamCone>, Without<BeamSpriteTop>)>,
    mut top_sprite_q: Query<(&BeamSpriteTop, &MeshMaterial3d<BeamSpriteMaterial>, &mut Transform, &GlobalTransform), (Without<BeamCone>, Without<BeamSprite>)>,
    mut light_q: Query<(&BeamSource, &mut PointLight)>,
    mut beam_materials: ResMut<Assets<BeamMaterial>>,
    mut sprite_materials: ResMut<Assets<BeamSpriteMaterial>>,
    gobo_library: Res<GoboLibrary>,
    mut perf: ResMut<PerfDiagnosticsRes>,
) {
    let t0 = std::time::Instant::now();

    const INTENSITY: f32 = 0.55;
    const BASE_HALF_DEG: f32 = 5.0;
    let elapsed = time.elapsed_secs();

    // Whether the programmer changed this frame (per-fixture writes mark it),
    // plus a time-driven flag if *any* fixture is strobing or spinning a gobo.
    let programmer_changed = programmer.is_changed();
    let time_driven = programmer
        .fixture_values
        .values()
        .any(|v| v.strobe > 0.01 || v.gobo_spin > 0.0);
    let needs_full_update = programmer_changed || time_driven;

    // Derive a single fixture's beam appearance from its own programmer values.
    let derive = |id: FixtureId| {
        let v = programmer.values_for(id);
        let shutter_open = if v.strobe < 0.01 {
            true
        } else {
            let hz = v.strobe * 25.0;
            (elapsed * hz) % 1.0 < 0.5
        };
        let d = v.dimmer * INTENSITY * if shutter_open { 1.0 } else { 0.0 };
        let color = LinearRgba::new(v.color[0] * d, v.color[1] * d, v.color[2] * d, 1.0);
        let target_half_deg = 2.5 + v.zoom * 20.0;
        let scale_xz = target_half_deg.to_radians().tan() / BASE_HALF_DEG.to_radians().tan();
        let gobo_rotation = elapsed * v.gobo_spin * std::f32::consts::TAU;
        let gobo_handle = gobo_library
            .handles
            .get(v.gobo_index)
            .cloned()
            .unwrap_or_else(|| gobo_library.handles[0].clone());
        let light_intensity = v.dimmer * 500_000.0 * if shutter_open { 1.0 } else { 0.0 };
        let light_color = Color::srgb(v.color[0], v.color[1], v.color[2]);
        (color, scale_xz, gobo_rotation, gobo_handle, light_intensity, light_color)
    };

    for (cone, handle, mut transform, global_tf) in &mut beam_q {
        let transform_changed = global_tf.is_changed();
        if !needs_full_update && !transform_changed {
            continue;
        }

        if let Some(mat) = beam_materials.get_mut(handle.id()) {
            if needs_full_update {
                let (color, scale_xz, gobo_rotation, gobo_handle, _, _) = derive(cone.id);
                mat.color         = color;
                mat.gobo_params   = Vec4::new(gobo_rotation, 0.0, 0.0, 0.0);
                mat.gobo          = gobo_handle;
                // Use the *base* half-angle here: the shader operates in scaled cone-local
                // space (world_to_cone includes the XZ zoom scale), so the cone geometry
                // in that space is always the original base cone.
                mat.beam_params   = Vec4::new(
                    BASE_HALF_DEG.to_radians(),
                    BEAM_HEIGHT,
                    2.0,
                    0.8,
                );
                transform.scale = Vec3::new(scale_xz, 1.0, scale_xz);
            }
            if transform_changed {
                mat.world_to_cone = global_tf.to_matrix().inverse();
            }
        }
    }

    // Sprite materials and point lights only need updating when programmer changes.
    if needs_full_update {
        for (sprite, sprite_handle, mut sprite_transform, _global_tf) in &mut sprite_q {
            let (color, scale_xz, gobo_rotation, gobo_handle, _, _) = derive(sprite.id);
            if let Some(mat) = sprite_materials.get_mut(sprite_handle.id()) {
                mat.color = color;
                mat.sprite_params = Vec4::new(gobo_rotation, 2.0, 0.0, 0.0);
                mat.gobo = gobo_handle;
            }
            sprite_transform.scale = Vec3::new(scale_xz, scale_xz, scale_xz);
        }
        for (top, top_sprite_handle, mut top_sprite_transform, _global_tf) in &mut top_sprite_q {
            let (color, scale_xz, gobo_rotation, gobo_handle, _, _) = derive(top.id);
            if let Some(mat) = sprite_materials.get_mut(top_sprite_handle.id()) {
                mat.color = color;
                mat.sprite_params = Vec4::new(gobo_rotation, 2.0, 0.0, 0.0);
                mat.gobo = gobo_handle;
            }
            top_sprite_transform.scale = Vec3::new(scale_xz, scale_xz, scale_xz);
        }

        for (source, mut light) in &mut light_q {
            let (_, _, _, _, light_intensity, light_color) = derive(source.id);
            light.intensity = light_intensity;
            light.color     = light_color;
        }
    }

    perf.beam_articulate_ms = t0.elapsed().as_secs_f32() * 1000.0;
}

// ─── Keyboard programmer ──────────────────────────────────────────────────────

pub fn keyboard_programmer(
    keys: Res<ButtonInput<KeyCode>>,
    mut programmer: ResMut<Programmer>,
    time: Res<Time>,
) {
    let dt = time.delta_secs();

    if keys.pressed(KeyCode::ArrowLeft)  { programmer.pan  = (programmer.pan  - dt * 0.4).max(0.0); }
    if keys.pressed(KeyCode::ArrowRight) { programmer.pan  = (programmer.pan  + dt * 0.4).min(1.0); }
    if keys.pressed(KeyCode::ArrowUp)    { programmer.tilt = (programmer.tilt + dt * 0.4).min(1.0); }
    if keys.pressed(KeyCode::ArrowDown)  { programmer.tilt = (programmer.tilt - dt * 0.4).max(0.0); }

    if keys.pressed(KeyCode::Equal) || keys.pressed(KeyCode::NumpadAdd) {
        programmer.dimmer = (programmer.dimmer + dt * 0.8).min(1.0);
    }
    if keys.pressed(KeyCode::Minus) || keys.pressed(KeyCode::NumpadSubtract) {
        programmer.dimmer = (programmer.dimmer - dt * 0.8).max(0.0);
    }

    if keys.pressed(KeyCode::KeyR) { programmer.color[0] = (programmer.color[0] + dt).min(1.0); }
    if keys.pressed(KeyCode::KeyG) { programmer.color[1] = (programmer.color[1] + dt).min(1.0); }
    if keys.pressed(KeyCode::KeyB) { programmer.color[2] = (programmer.color[2] + dt).min(1.0); }
    if keys.pressed(KeyCode::KeyW) { programmer.color = [1.0, 1.0, 1.0]; }
    if keys.pressed(KeyCode::KeyX) { programmer.color = [1.0, 0.0, 0.0]; }
    if keys.pressed(KeyCode::KeyC) { programmer.color = [0.0, 0.5, 1.0]; }

    if keys.pressed(KeyCode::KeyZ) {
        let dir = if keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight) { -1.0 } else { 1.0 };
        programmer.zoom = (programmer.zoom + dir * dt * 0.6).clamp(0.0, 1.0);
    }
}

