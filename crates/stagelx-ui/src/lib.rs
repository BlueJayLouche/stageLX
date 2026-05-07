pub mod library;
pub mod patch;
pub mod programmer;

use bevy::prelude::*;
use bevy_egui::EguiPlugin;
use stagelx_core::patch::Patch;
use stagelx_gdtf::FixtureLibrary;

// ─── Shared Bevy resources ────────────────────────────────────────────────────

/// Normalised programmer state — all values 0.0–1.0.
/// Both the render crate (articulation) and UI (sliders) share this resource.
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

/// Bevy Resource wrapping the show patch (fixture → DMX address mapping).
#[derive(Resource, Default)]
pub struct PatchRes(pub Patch);

/// Bevy Resource wrapping the loaded GDTF fixture library.
#[derive(Resource, Default)]
pub struct FixtureLibraryRes {
    pub library: FixtureLibrary,
    /// Text field state for the GDTF import path input.
    pub import_path: String,
    pub import_error: Option<String>,
}

// ─── Plugin ───────────────────────────────────────────────────────────────────

pub struct StageLxUiPlugin;

impl Plugin for StageLxUiPlugin {
    fn build(&self, app: &mut App) {
        if !app.is_plugin_added::<EguiPlugin>() {
            app.add_plugins(EguiPlugin::default());
        }

        app.init_resource::<Programmer>()
            .init_resource::<PatchRes>()
            .init_resource::<FixtureLibraryRes>()
            .add_systems(
                Update,
                (
                    programmer::programmer_panel,
                    patch::patch_panel,
                    library::library_panel,
                ),
            );
    }
}
