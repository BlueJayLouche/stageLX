pub mod beam;
pub mod fixture;
pub mod gobo;
pub mod scene;

use bevy::prelude::*;
use stagelx_core::{
    fixture::FixtureInstance,
    types::{DmxAddress, FixtureId},
};
use stagelx_ui::{FixtureLibraryRes, PatchRes};
use fixture::{FixtureSpawnConfig, spawn_fixture};

pub struct StageLxRenderPlugin;

impl Plugin for StageLxRenderPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, (scene::setup_scene, spawn_demo_fixtures).chain())
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

fn spawn_demo_fixtures(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut patch: ResMut<PatchRes>,
    mut _library: ResMut<FixtureLibraryRes>,
) {
    const COUNT: usize = 10;
    const SPACING: f32 = 1.8;
    let total_width = (COUNT - 1) as f32 * SPACING;

    for i in 0..COUNT {
        let x = -total_width / 2.0 + i as f32 * SPACING;

        let id = patch.0.add(FixtureInstance {
            id: FixtureId(0), // assigned by Patch::add
            name: format!("MH {}", i + 1),
            fixture_type_id: "generic-moving-head".into(),
            dmx_mode: "Standard".into(),
            address: DmxAddress::new(1, (i as u16 * 8 + 1).min(512)),
            position: [x, 6.0, 0.0],
            rotation: [0.0, 0.0, 0.0],
        });

        spawn_fixture(
            &mut commands,
            &mut meshes,
            &mut materials,
            FixtureSpawnConfig {
                id,
                position: Vec3::new(x, 6.0, 0.0),
                suspended: true,
                pan_range: 540.0,
                tilt_range: 270.0,
                beam_angle_deg: 10.0,
            },
        );
    }
}
