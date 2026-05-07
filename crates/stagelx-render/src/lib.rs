pub mod beam;
pub mod fixture;
pub mod gobo;
pub mod scene;

use bevy::prelude::*;
use stagelx_core::{
    fixture::FixtureInstance,
    types::{DmxAddress, FixtureId},
};
use stagelx_state::{FixtureLibraryRes, PatchRes};
use beam::{BeamMaterial, GoboLibrary, setup_gobos};
use fixture::{FixtureSpawnConfig, spawn_fixture};

pub struct StageLxRenderPlugin;

impl Plugin for StageLxRenderPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(MaterialPlugin::<BeamMaterial>::default())
            .add_systems(
                Startup,
                (scene::setup_scene, setup_gobos, spawn_demo_fixtures).chain(),
            )
            .add_systems(
                Update,
                (
                    fixture::keyboard_programmer,
                    fixture::articulate_fixtures,
                    fixture::articulate_beams,
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
    mut beam_materials: ResMut<Assets<BeamMaterial>>,
    mut patch: ResMut<PatchRes>,
    _library: Res<FixtureLibraryRes>,
    gobo_library: Res<GoboLibrary>,
) {
    const COUNT: usize = 10;
    const SPACING: f32 = 1.8;
    let total_width = (COUNT - 1) as f32 * SPACING;
    let open_gobo = gobo_library.handles[0].clone();

    for i in 0..COUNT {
        let x = -total_width / 2.0 + i as f32 * SPACING;

        let id = patch.0.add(FixtureInstance {
            id: FixtureId(0),
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
            &mut beam_materials,
            FixtureSpawnConfig {
                id,
                position: Vec3::new(x, 6.0, 0.0),
                suspended: true,
                pan_range: 540.0,
                tilt_range: 270.0,
                beam_angle_deg: 10.0,
            },
            open_gobo.clone(),
        );
    }
}
