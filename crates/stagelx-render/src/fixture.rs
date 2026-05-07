use bevy::prelude::*;
use stagelx_core::types::FixtureId;
use stagelx_state::Programmer;
use crate::beam::{BeamMaterial, GoboLibrary, build_beam_cone};

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

// ─── Spawning ─────────────────────────────────────────────────────────────────

pub struct FixtureSpawnConfig {
    pub id: FixtureId,
    pub position: Vec3,
    pub suspended: bool,
    pub pan_range: f32,
    pub tilt_range: f32,
    pub beam_angle_deg: f32,
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
        }
    }
}

pub fn spawn_fixture(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    beam_materials: &mut Assets<BeamMaterial>,
    cfg: FixtureSpawnConfig,
    open_gobo: Handle<Image>,
) {
    let body_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.15, 0.15, 0.15),
        metallic: 0.8,
        perceptual_roughness: 0.3,
        ..default()
    });
    let joint_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.1, 0.1, 0.1),
        metallic: 0.9,
        perceptual_roughness: 0.2,
        ..default()
    });

    let body_mesh = meshes.add(Cuboid::new(0.30, 0.25, 0.30));
    let yoke_mesh = meshes.add(Cuboid::new(0.35, 0.08, 0.08));
    let head_mesh = meshes.add(Cuboid::new(0.22, 0.28, 0.22));

    let yoke_y = if cfg.suspended { -0.18 } else { 0.18 };
    let head_y = if cfg.suspended { -0.22 } else { 0.22 };

    // ── Beam cone geometry ────────────────────────────────────────────────────
    const BEAM_HEIGHT: f32 = 18.0;
    const LENS_OFFSET: f32 = 0.14; // half of head height 0.28

    let half_angle = (cfg.beam_angle_deg * 0.5).to_radians();
    let beam_radius = BEAM_HEIGHT * half_angle.tan();

    let (cone_y, cone_rot) = if cfg.suspended {
        (-(LENS_OFFSET + BEAM_HEIGHT * 0.5), Quat::IDENTITY)
    } else {
        (
            LENS_OFFSET + BEAM_HEIGHT * 0.5,
            Quat::from_rotation_x(std::f32::consts::PI),
        )
    };

    let cone_mesh = meshes.add(build_beam_cone(beam_radius, BEAM_HEIGHT));
    let beam_mat = beam_materials.add(BeamMaterial {
        color: LinearRgba::WHITE,
        gobo_params: Vec4::ZERO,
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
    let pan_deg = (programmer.pan - 0.5) * programmer.pan_range;
    let tilt_deg = (programmer.tilt - 0.5) * programmer.tilt_range;

    for (_yoke, mut transform) in &mut yoke_q {
        transform.rotation = Quat::from_rotation_y(pan_deg.to_radians());
    }
    for (_head, mut transform) in &mut head_q {
        transform.rotation = Quat::from_rotation_x(tilt_deg.to_radians());
    }
}

pub fn articulate_beams(
    programmer: Res<Programmer>,
    time: Res<Time>,
    mut beam_q: Query<
        (&MeshMaterial3d<BeamMaterial>, &mut Transform),
        (With<BeamCone>, Without<YokeJoint>, Without<HeadJoint>),
    >,
    mut light_q: Query<&mut PointLight, With<BeamSource>>,
    mut beam_materials: ResMut<Assets<BeamMaterial>>,
    gobo_library: Res<GoboLibrary>,
) {
    // ── Strobe ────────────────────────────────────────────────────────────────
    let shutter_open = if programmer.strobe < 0.01 {
        true
    } else {
        let hz = programmer.strobe * 25.0;
        (time.elapsed_secs() * hz) % 1.0 < 0.5
    };

    // ── Colour + dimmer ───────────────────────────────────────────────────────
    const INTENSITY: f32 = 0.55;
    let d = programmer.dimmer * INTENSITY * if shutter_open { 1.0 } else { 0.0 };
    let color = LinearRgba::new(
        programmer.color[0] * d,
        programmer.color[1] * d,
        programmer.color[2] * d,
        1.0,
    );

    // ── Zoom → XZ scale ───────────────────────────────────────────────────────
    const BASE_HALF_DEG: f32 = 5.0; // matches beam_angle_deg = 10° from config
    let target_half_deg = 2.5 + programmer.zoom * 20.0;
    let scale_xz = target_half_deg.to_radians().tan()
        / BASE_HALF_DEG.to_radians().tan();

    // ── Gobo rotation ─────────────────────────────────────────────────────────
    let gobo_rotation = time.elapsed_secs() * programmer.gobo_spin * std::f32::consts::TAU;
    let gobo_params = Vec4::new(gobo_rotation, 0.0, 0.0, 0.0);

    let gobo_handle = gobo_library
        .handles
        .get(programmer.gobo_index)
        .cloned()
        .unwrap_or_else(|| gobo_library.handles[0].clone());

    for (handle, mut transform) in &mut beam_q {
        if let Some(mat) = beam_materials.get_mut(handle.id()) {
            mat.color = color;
            mat.gobo_params = gobo_params;
            mat.gobo = gobo_handle.clone();
        }
        transform.scale = Vec3::new(scale_xz, 1.0, scale_xz);
    }

    // ── Point-light (strobed, not dimmed by INTENSITY so floor stays bright) ──
    let light_intensity = programmer.dimmer * 500_000.0 * if shutter_open { 1.0 } else { 0.0 };
    let light_color = Color::srgb(programmer.color[0], programmer.color[1], programmer.color[2]);
    for mut light in &mut light_q {
        light.intensity = light_intensity;
        light.color = light_color;
    }
}

// ─── Keyboard programmer system ───────────────────────────────────────────────

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
        let dir = if keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight) {
            -1.0
        } else {
            1.0
        };
        programmer.zoom = (programmer.zoom + dir * dt * 0.6).clamp(0.0, 1.0);
    }
}
