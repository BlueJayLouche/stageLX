pub mod adapters;
pub mod beam;
pub mod beam_sprite;
pub mod camera;
pub mod diagnostics;
pub mod fixture;
pub mod gobo;
pub mod lod;
pub mod scene;
pub mod venue;

use bevy::prelude::*;
use stagelx_show::{LoadMvrStructureEvent, LoadVenueEvent, PerfDiagnosticsRes, VenueLoadState};
use beam::{BeamMaterial, setup_gobos};
use beam_sprite::BeamSpriteMaterial;
use camera::{foh_camera_input, foh_camera_update};
use lod::{
    BeamCompositeMaterial, setup_beam_lod,
    sync_beam_camera_to_foh, evaluate_beam_lod, apply_beam_lod,
    sort_beams_front_to_back, resize_beam_render_target,
};
use diagnostics::{estimate_gpu_memory, track_beam_counts, track_frame_time};
pub use venue::VenueRoot;

pub struct StageLxRenderPlugin;

impl Plugin for StageLxRenderPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<VenueLoadState>()
            .init_resource::<PerfDiagnosticsRes>()
            .add_plugins(MaterialPlugin::<BeamMaterial>::default())
            .add_plugins(MaterialPlugin::<BeamSpriteMaterial>::default())
            .add_plugins(MaterialPlugin::<BeamCompositeMaterial>::default())
            .add_observer(fixture::on_fixture_spawned)
            .add_observer(fixture::on_fixture_despawned)
            .add_observer(on_load_venue)
            .add_observer(on_load_mvr_structure)
            .add_systems(
                Startup,
                (scene::setup_scene, setup_gobos, setup_beam_lod).chain(),
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
                    sort_beams_front_to_back,
                    track_frame_time,
                    track_beam_counts,
                )
                    .chain(),
            )
            .add_systems(
                Last,
                estimate_gpu_memory,
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
    let event = trigger.event();
    let path = event.path.clone();
    let offset = event.offset;
    match venue::load_venue(&path, offset, &mut commands, &mut meshes, &mut materials, &existing) {
        Ok(()) => {
            venue_state.import_error = None;
            venue_state.import_path.clear();
        }
        Err(e) => venue_state.import_error = Some(e),
    }
}

// ─── LoadMvrStructureEvent observer ───────────────────────────────────────────

fn on_load_mvr_structure(
    trigger: On<LoadMvrStructureEvent>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    existing: Query<Entity, With<VenueRoot>>,
) {
    let event = trigger.event();
    match venue::load_mvr_structure(&event.objects, &mut commands, &mut meshes, &mut materials, &existing) {
        Ok(()) => {}
        Err(e) => bevy::log::error!("MVR structure load error: {e}"),
    }
}



// Demo fixtures were previously spawned here at startup (10 generic moving heads
// on universe 1). Removed: they overlapped user-patched addresses. Fixtures now
// come only from the patch UI or an auto-loaded show.
