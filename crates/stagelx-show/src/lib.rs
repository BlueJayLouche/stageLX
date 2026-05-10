//! Show-level Bevy Resources and Events for stageLX.
//!
//! Contains programmer state, performance diagnostics, cue data,
//! and venue-loading events.

use bevy::prelude::*;
use stagelx_gdtf::FixtureLibrary;

pub mod cue;
pub use cue::*;

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

/// Normalised programmer state — all channel values 0.0–1.0.
/// Written by the UI and keyboard handler; read by render and DMX output.
#[derive(Resource, Clone, PartialEq)]
pub struct Programmer {
    pub pan: f32,
    pub tilt: f32,
    pub dimmer: f32,
    pub color: [f32; 3],
    pub pan_range: f32,
    pub tilt_range: f32,
    /// 0.0 = narrowest beam (5°), 1.0 = widest beam (45°).
    pub zoom: f32,
    /// 0.0 = shutter open (no strobe), 1.0 = fastest strobe (~25 Hz).
    pub strobe: f32,
    /// Index into GoboLibrary (0 = open beam).
    pub gobo_index: usize,
    /// Gobo spin speed in rotations per second (0.0 = static).
    pub gobo_spin: f32,
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
            zoom: 0.0,
            strobe: 0.0,
            gobo_index: 0,
            gobo_spin: 0.0,
        }
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
