pub mod beam;
pub mod fixture;
pub mod gobo;
pub mod scene;

use bevy::prelude::*;
use fixture::{FixtureSpawnConfig, spawn_fixture};
use stagelx_core::types::FixtureId;

// ─── Programmer resource ──────────────────────────────────────────────────────

/// Simple direct-value programmer for Phase 1.
/// All values are normalised 0.0–1.0. Phase 3 will replace this with the
/// full DmxEngine + per-fixture DMX addressing.
#[derive(Resource)]
pub struct Programmer {
    pub pan: f32,
    pub tilt: f32,
    pub dimmer: f32,
    pub color: [f32; 3],
    pub pan_range: f32,
    pub tilt_range: f32,
}

impl Default for Programmer {
    fn default() -> Self {
        Self {
            pan: 0.5,
            tilt: 0.5,
            dimmer: 1.0,
            color: [1.0, 1.0, 1.0],
            pan_range: 540.0,
            tilt_range: 270.0,
        }
    }
}

// ─── Plugin ───────────────────────────────────────────────────────────────────

pub struct StageLxRenderPlugin;

impl Plugin for StageLxRenderPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Programmer>()
            .add_systems(Startup, (scene::setup_scene, spawn_demo_fixtures).chain())
            .add_systems(
                Update,
                (
                    fixture::keyboard_programmer,
                    fixture::articulate_fixtures,
                )
                    .chain(),
            );
    }
}

// ─── Demo fixture startup ─────────────────────────────────────────────────────

/// Spawns 10 generic moving heads on the truss for the Phase 1 demo.
/// Replace this with patch-driven spawning in Phase 1 UI work.
fn spawn_demo_fixtures(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    const COUNT: usize = 10;
    const SPACING: f32 = 1.8;
    let total_width = (COUNT - 1) as f32 * SPACING;

    for i in 0..COUNT {
        let x = -total_width / 2.0 + i as f32 * SPACING;
        spawn_fixture(
            &mut commands,
            &mut meshes,
            &mut materials,
            FixtureSpawnConfig {
                id: FixtureId(i as u32),
                position: Vec3::new(x, 6.0, 0.0),
                suspended: true,
                pan_range: 540.0,
                tilt_range: 270.0,
                beam_angle_deg: 10.0,
            },
        );
    }
}
