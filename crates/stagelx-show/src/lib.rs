//! Show-level Bevy Resources and Events for stageLX.
//!
//! Contains programmer state, performance diagnostics, cue data,
//! and venue-loading events.

use std::collections::HashMap;
use bevy::prelude::*;
use stagelx_gdtf::FixtureLibrary;
use stagelx_core::types::FixtureId;

pub mod cue;
pub mod show_file;
pub use cue::*;
pub use show_file::*;

// ─── Events ───────────────────────────────────────────────────────────────────

/// Emitted by the Library UI when the user loads a venue file.
/// The render plugin observes this and calls the actual mesh loader,
/// keeping `stagelx-ui` free of any `stagelx-render` dependency.
#[derive(Event, Debug, Clone)]
pub struct LoadVenueEvent {
    pub path: String,
    /// World-space offset applied to the venue root after loading (metres).
    pub offset: [f32; 3],
}

/// One structure object to load from an MVR file (SceneObject or Truss geometry).
#[derive(Debug, Clone)]
pub struct MvrStructureObject {
    pub name: String,
    /// Absolute path to the extracted geometry file in temp storage.
    pub file_path: String,
    pub position: [f32; 3],
    pub rotation: [f32; 3],
}

/// Emitted by the Library UI after parsing an MVR file.
/// The render plugin observes this and spawns the referenced geometry.
#[derive(Event, Debug, Clone)]
pub struct LoadMvrStructureEvent {
    pub objects: Vec<MvrStructureObject>,
}

// ─── Programmer ───────────────────────────────────────────────────────────────

/// Per-fixture programmer values — normalised 0.0–1.0 unless noted.
#[derive(Clone, Debug)]
pub struct ProgrammerValues {
    pub pan: f32,
    pub tilt: f32,
    pub dimmer: f32,
    pub color: [f32; 3],
    pub zoom: f32,
    pub strobe: f32,
    pub gobo_index: usize,
    pub gobo_spin: f32,
    /// Raw DMX value (0–255) for ColorMacro1 — color preset selector on FX lights.
    pub color_macro: u8,
    /// Normalised motor rotation speed/index (0.0–1.0 → DMX 0–255).
    pub rotation: f32,
}

impl Default for ProgrammerValues {
    fn default() -> Self {
        Self {
            pan: 0.5,
            tilt: 0.62,
            dimmer: 1.0,
            color: [1.0, 1.0, 1.0],
            zoom: 0.0,
            strobe: 0.0,
            gobo_index: 0,
            gobo_spin: 0.0,
            color_macro: 0,
            rotation: 0.0,
        }
    }
}

/// Global programmer resource.
///
/// `pan/tilt/dimmer/…` are the **active editor values** — what the UI sliders
/// show and what the 3-D render uses.  They are always the values for whichever
/// fixture(s) are currently selected.
///
/// `fixture_values` is the **per-fixture store**: a fixture is "in the
/// programmer" once it has been selected and touched.  It retains its last
/// values after being deselected.  The DMX output reads from this map.
#[derive(Resource, Clone)]
pub struct Programmer {
    // ── Active editor (display) values ────────────────────────────────────────
    pub pan: f32,
    pub tilt: f32,
    pub dimmer: f32,
    pub color: [f32; 3],
    pub zoom: f32,
    pub strobe: f32,
    pub gobo_index: usize,
    pub gobo_spin: f32,
    pub color_macro: u8,
    pub rotation: f32,
    // ── Meta (not per-fixture) ────────────────────────────────────────────────
    pub pan_range: f32,
    pub tilt_range: f32,
    // ── Per-fixture store ─────────────────────────────────────────────────────
    pub fixture_values: HashMap<FixtureId, ProgrammerValues>,
    /// Set by apply_cue_to_programmer / on_load_cue_into_programmer when the
    /// display fields are changed programmatically (not by the user).
    /// programmer_update resets the baseline instead of writing back when this is true.
    pub cue_load_pending: bool,
}

/// `PartialEq` compares only the active display fields so that the render
/// system's change-detection (`*last != *current`) is not triggered by writes
/// to `fixture_values` that happen every frame.
impl PartialEq for Programmer {
    fn eq(&self, other: &Self) -> bool {
        self.pan         == other.pan
            && self.tilt         == other.tilt
            && self.dimmer       == other.dimmer
            && self.color        == other.color
            && self.zoom         == other.zoom
            && self.strobe       == other.strobe
            && self.gobo_index   == other.gobo_index
            && self.gobo_spin    == other.gobo_spin
    }
}

impl Default for Programmer {
    fn default() -> Self {
        Self {
            pan: 0.5,
            tilt: 0.62,
            dimmer: 1.0,
            color: [1.0, 1.0, 1.0],
            pan_range: 540.0,
            tilt_range: 270.0,
            zoom: 0.0,
            strobe: 0.0,
            gobo_index: 0,
            gobo_spin: 0.0,
            color_macro: 0,
            rotation: 0.0,
            fixture_values: HashMap::new(),
            cue_load_pending: false,
        }
    }
}

impl Programmer {
    /// Snapshot the active display fields into a `ProgrammerValues`.
    pub fn active_values(&self) -> ProgrammerValues {
        ProgrammerValues {
            pan: self.pan,
            tilt: self.tilt,
            dimmer: self.dimmer,
            color: self.color,
            zoom: self.zoom,
            strobe: self.strobe,
            gobo_index: self.gobo_index,
            gobo_spin: self.gobo_spin,
            color_macro: self.color_macro,
            rotation: self.rotation,
        }
    }

    /// Effective values for a single fixture, for systems that must render or
    /// output each fixture independently (DMX projection, 3-D articulation).
    ///
    /// Returns the fixture's stored per-fixture entry; falls back to the active
    /// display fields for fixtures that have never been individually programmed.
    pub fn values_for(&self, id: FixtureId) -> ProgrammerValues {
        self.fixture_values
            .get(&id)
            .cloned()
            .unwrap_or_else(|| self.active_values())
    }

    /// Copy a `ProgrammerValues` snapshot into the active display fields.
    pub fn load_values(&mut self, v: &ProgrammerValues) {
        self.pan         = v.pan;
        self.tilt        = v.tilt;
        self.dimmer      = v.dimmer;
        self.color       = v.color;
        self.zoom        = v.zoom;
        self.strobe      = v.strobe;
        self.gobo_index  = v.gobo_index;
        self.gobo_spin   = v.gobo_spin;
        self.color_macro = v.color_macro;
        self.rotation    = v.rotation;
    }
}

// ─── EguiViewportRect ─────────────────────────────────────────────────────────

/// The egui logical-pixel rect occupied by the central 3D viewport.
/// Written each frame by `ui_root_system`; read by `foh_camera_input` to
/// decide whether mouse input should be routed to the camera or ignored.
#[derive(Resource, Default, Clone, Copy)]
pub struct EguiViewportRect {
    pub min_x: f32,
    pub min_y: f32,
    pub max_x: f32,
    pub max_y: f32,
    pub valid: bool,
}

impl EguiViewportRect {
    pub fn contains(&self, x: f32, y: f32) -> bool {
        self.valid && x >= self.min_x && x <= self.max_x && y >= self.min_y && y <= self.max_y
    }
}

// ─── FixtureLibraryRes ────────────────────────────────────────────────────────

/// Bevy Resource wrapping the loaded GDTF fixture library.
#[derive(Resource, Default)]
pub struct FixtureLibraryRes {
    pub library: FixtureLibrary,
    /// Text field state for the GDTF import path input.
    pub import_path: String,
    pub import_error: Option<String>,
    /// MVR import state.
    pub mvr_import_path: String,
    pub mvr_import_error: Option<String>,
}

// ─── ShowName ─────────────────────────────────────────────────────────────────

/// The name of the currently open show, displayed in the title bar.
#[derive(Resource, Debug, Clone)]
pub struct ShowName(pub String);

impl Default for ShowName {
    fn default() -> Self {
        Self("Untitled Show".into())
    }
}

// ─── VenueLoadState ───────────────────────────────────────────────────────────

/// UI state for the venue loader (moved from stagelx-render per Rule 21).
#[derive(Resource, Default)]
pub struct VenueLoadState {
    pub import_path: String,
    pub import_error: Option<String>,
    /// World-space offset applied to the loaded venue (metres, Bevy coords).
    pub offset: [f32; 3],
}

// ─── ProtocolStatus ───────────────────────────────────────────────────────────

#[derive(Default, PartialEq, Eq, Clone, Copy, Debug)]
pub enum ProtocolStatus {
    #[default]
    Idle,
    Live,
    Warn,
    Error,
}

// ─── Performance Diagnostics ──────────────────────────────────────────────────

/// Running performance metrics collected across subsystems.
/// Written by render / IO / DMX crates; read by the UI for the performance HUD.
#[derive(Resource, Debug)]
pub struct PerfDiagnosticsRes {
    /// DMX tick: number of ticks sampled.
    pub dmx_tick_count: u64,
    /// DMX tick: running mean (ms).
    pub dmx_tick_mean_ms: f32,
    /// DMX tick: M2 accumulator for Welford's algorithm.
    dmx_tick_m2: f64,
    /// DMX tick: sample standard deviation (ms).
    pub dmx_tick_std_dev_ms: f32,
    /// DMX tick: duration of the last tick (ms).
    pub dmx_tick_last_ms: f32,
    /// Number of beams in the scene.
    pub beam_count: usize,
    /// Number of beams in Tier 1 + Tier 2 (ray-marched).
    pub beam_raymarch_count: usize,
    /// Estimated GPU memory for all venue + fixture geometry (MB).
    pub estimated_gpu_memory_mb: f32,
    /// CPU frame time from Bevy diagnostics (ms).
    pub frame_time_ms: f32,
    /// Duration of the most recent fixture spawn (ms).
    pub last_fixture_spawn_ms: f32,
    /// Total fixtures spawned since app start.
    pub fixtures_spawned: u64,
    /// Per-system CPU timings (ms).
    pub beam_articulate_ms: f32,
    pub beam_lod_eval_ms: f32,
    pub beam_lod_apply_ms: f32,
    pub beam_sort_ms: f32,
}

impl Default for PerfDiagnosticsRes {
    fn default() -> Self {
        Self {
            dmx_tick_count: 0,
            dmx_tick_mean_ms: 0.0,
            dmx_tick_m2: 0.0,
            dmx_tick_std_dev_ms: 0.0,
            dmx_tick_last_ms: 0.0,
            beam_count: 0,
            beam_raymarch_count: 0,
            estimated_gpu_memory_mb: 0.0,
            frame_time_ms: 0.0,
            last_fixture_spawn_ms: 0.0,
            fixtures_spawned: 0,
            beam_articulate_ms: 0.0,
            beam_lod_eval_ms: 0.0,
            beam_lod_apply_ms: 0.0,
            beam_sort_ms: 0.0,
        }
    }
}

impl PerfDiagnosticsRes {
    /// Record a new DMX tick duration using Welford's online algorithm.
    pub fn record_dmx_tick(&mut self, duration_ms: f32) {
        self.dmx_tick_count += 1;
        self.dmx_tick_last_ms = duration_ms;
        let n = self.dmx_tick_count as f64;
        let x = duration_ms as f64;
        let delta = x - self.dmx_tick_mean_ms as f64;
        self.dmx_tick_mean_ms = (self.dmx_tick_mean_ms as f64 + delta / n) as f32;
        let delta2 = x - self.dmx_tick_mean_ms as f64;
        self.dmx_tick_m2 += delta * delta2;
        if self.dmx_tick_count > 1 {
            self.dmx_tick_std_dev_ms = (self.dmx_tick_m2 / (n - 1.0)).sqrt() as f32;
        }
    }
}
