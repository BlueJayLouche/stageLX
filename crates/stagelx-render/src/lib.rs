pub mod adapters;
pub mod beam;
pub mod beam_sprite;
pub mod camera;
pub mod fixture;
pub mod gobo;
pub mod lod;
pub mod scene;
pub mod venue;

use bevy::prelude::*;
use stagelx_core::{
    fixture::FixtureInstance,
    types::{DmxAddress, FixtureId},
};
use stagelx_state::{FixtureLibraryRes, LoadVenueEvent, PatchRes, VenueLoadState};
use beam::{BeamMaterial, GoboLibrary, setup_gobos};
use beam_sprite::BeamSpriteMaterial;
use camera::{foh_camera_input, foh_camera_update};
use fixture::{FixtureSpawnConfig, spawn_fixture};
use lod::{
    BeamCompositeMaterial, setup_beam_lod,
    sync_beam_camera_to_foh, evaluate_beam_lod, apply_beam_lod,
    resize_beam_render_target,
};
pub use venue::VenueRoot;

pub struct StageLxRenderPlugin;

impl Plugin for StageLxRenderPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<VenueLoadState>()
            .add_plugins(MaterialPlugin::<BeamMaterial>::default())
            .add_plugins(MaterialPlugin::<BeamSpriteMaterial>::default())
            .add_plugins(MaterialPlugin::<BeamCompositeMaterial>::default())
            .add_observer(fixture::on_fixture_spawned)
            .add_observer(fixture::on_fixture_despawned)
            .add_observer(on_load_venue)
            .add_systems(
                Startup,
                (scene::setup_scene, setup_gobos, setup_beam_lod, spawn_demo_fixtures).chain(),
            )
            .add_systems(
                Update,
                (
                    scene::update_viewports_on_resize,
                    resize_beam_render_target,
                    foh_camera_input,
                    foh_camera_update,
                    sync_beam_camera_to_foh,
                    fixture::keyboard_programmer,
                    fixture::articulate_fixtures,
                    fixture::articulate_beams,
                    evaluate_beam_lod,
                    apply_beam_lod,
                )
                    .chain(),
            );
    }
}

// ─── LoadVenueEvent observer ──────────────────────────────────────────────────

fn on_load_venue(
    trigger: On<LoadVenueEvent>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut venue_state: ResMut<VenueLoadState>,
    existing: Query<Entity, With<VenueRoot>>,
) {
    let path = trigger.event().path.clone();
    match venue::load_venue(&path, &mut commands, &mut meshes, &mut materials, &existing) {
        Ok(()) => {
            venue_state.import_error = None;
            venue_state.import_path.clear();
        }
        Err(e) => venue_state.import_error = Some(e),
    }
}

// ─── Demo fixture startup ─────────────────────────────────────────────────────

fn spawn_demo_fixtures(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut beam_materials: ResMut<Assets<BeamMaterial>>,
    mut sprite_materials: ResMut<Assets<BeamSpriteMaterial>>,
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

        let channel_map = _library
            .library
            .get("generic-moving-head")
            .map(|ft| ft.channel_map("Standard"))
            .unwrap_or_default();

        let id = patch.0.add(FixtureInstance {
            id: FixtureId(0),
            name: format!("MH {}", i + 1),
            fixture_type_id: "generic-moving-head".into(),
            dmx_mode: "Standard".into(),
            address: DmxAddress::new(1, (i as u16 * 8 + 1).min(512)),
            position: [x, 6.0, 0.0],
            rotation: [0.0, 0.0, 0.0],
            channel_map,
        });

        // Demo fixtures spawn directly to avoid the 1-frame event delay.
        spawn_fixture(
            &mut commands,
            &mut meshes,
            &mut materials,
            &mut beam_materials,
            &mut sprite_materials,
            FixtureSpawnConfig {
                id,
                position: Vec3::new(x, 6.0, 0.0),
                suspended: true,
                pan_range: 540.0,
                tilt_range: 270.0,
                beam_angle_deg: 10.0,
                body_mesh: None,
            },
            open_gobo.clone(),
        );
    }
}
