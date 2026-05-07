use bevy::prelude::*;
use stagelx_core::types::FixtureId;

// ─── Components ───────────────────────────────────────────────────────────────

/// Root entity for a rendered fixture. Holds the fixture's ID for DMX lookup.
#[derive(Component)]
pub struct FixtureVisual {
    pub id: FixtureId,
}

/// Marks the yoke entity — rotates around the world Y-axis for Pan.
#[derive(Component)]
pub struct YokeJoint {
    pub id: FixtureId,
    /// Full pan range in degrees, symmetric (e.g. 540.0 for ±270°).
    pub pan_range: f32,
}

/// Marks the head entity — rotates around its local X-axis for Tilt.
#[derive(Component)]
pub struct HeadJoint {
    pub id: FixtureId,
    /// Full tilt range in degrees, symmetric (e.g. 270.0 for ±135°).
    pub tilt_range: f32,
}

/// Marks the beam point-light entity.
#[derive(Component)]
pub struct BeamSource {
    pub id: FixtureId,
}

// ─── Spawning ─────────────────────────────────────────────────────────────────

pub struct FixtureSpawnConfig {
    pub id: FixtureId,
    pub position: Vec3,
    /// Suspended from above (truss) when true; sits on floor when false.
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
    cfg: FixtureSpawnConfig,
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

    // Yoke offset: slightly below body centre
    let yoke_y = if cfg.suspended { -0.18 } else { 0.18 };
    // Head hangs below (or sits above) yoke
    let head_y = if cfg.suspended { -0.22 } else { 0.22 };

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
                });
            });
        });
}

// ─── Articulation system ──────────────────────────────────────────────────────

/// Reads the Programmer resource and updates fixture joint transforms each frame.
pub fn articulate_fixtures(
    programmer: Res<crate::Programmer>,
    mut yoke_q: Query<(&YokeJoint, &mut Transform)>,
    mut head_q: Query<(&HeadJoint, &mut Transform)>,
    mut beam_q: Query<(&BeamSource, &mut PointLight)>,
) {
    // For Phase 1 all fixtures share the same programmer values.
    // Phase 3 will dispatch per-fixture DMX values from the engine.
    let pan_deg = (programmer.pan - 0.5) * programmer.pan_range;
    let tilt_deg = (programmer.tilt - 0.5) * programmer.tilt_range;

    for (_yoke, mut transform) in &mut yoke_q {
        transform.rotation = Quat::from_rotation_y(pan_deg.to_radians());
    }

    for (_head, mut transform) in &mut head_q {
        transform.rotation = Quat::from_rotation_x(tilt_deg.to_radians());
    }

    for (_beam, mut light) in &mut beam_q {
        light.intensity = programmer.dimmer * 500_000.0;
        light.color = Color::srgb(programmer.color[0], programmer.color[1], programmer.color[2]);
    }
}

// ─── Keyboard programmer system ───────────────────────────────────────────────

pub fn keyboard_programmer(
    keys: Res<ButtonInput<KeyCode>>,
    mut programmer: ResMut<crate::Programmer>,
    time: Res<Time>,
) {
    let dt = time.delta_secs();
    let pan_speed = dt * 0.4;
    let tilt_speed = dt * 0.4;
    let dim_speed = dt * 0.8;

    if keys.pressed(KeyCode::ArrowLeft)  { programmer.pan  = (programmer.pan  - pan_speed).max(0.0); }
    if keys.pressed(KeyCode::ArrowRight) { programmer.pan  = (programmer.pan  + pan_speed).min(1.0); }
    if keys.pressed(KeyCode::ArrowUp)    { programmer.tilt = (programmer.tilt + tilt_speed).min(1.0); }
    if keys.pressed(KeyCode::ArrowDown)  { programmer.tilt = (programmer.tilt - tilt_speed).max(0.0); }

    if keys.pressed(KeyCode::Equal) || keys.pressed(KeyCode::NumpadAdd) {
        programmer.dimmer = (programmer.dimmer + dim_speed).min(1.0);
    }
    if keys.pressed(KeyCode::Minus) || keys.pressed(KeyCode::NumpadSubtract) {
        programmer.dimmer = (programmer.dimmer - dim_speed).max(0.0);
    }

    // R/G/B colour nudges
    if keys.pressed(KeyCode::KeyR) { programmer.color[0] = (programmer.color[0] + dt).min(1.0); }
    if keys.pressed(KeyCode::KeyG) { programmer.color[1] = (programmer.color[1] + dt).min(1.0); }
    if keys.pressed(KeyCode::KeyB) { programmer.color[2] = (programmer.color[2] + dt).min(1.0); }
    if keys.pressed(KeyCode::KeyW) { programmer.color = [1.0, 1.0, 1.0]; }
    if keys.pressed(KeyCode::KeyX) { programmer.color = [1.0, 0.0, 0.0]; }
    if keys.pressed(KeyCode::KeyC) { programmer.color = [0.0, 0.5, 1.0]; }
}
