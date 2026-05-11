//! Cue system data model and playback state.
//!
//! Phase 6.4 scope: basic cue stack, instant playback (no fade engine),
//! record-from-programmer, GO/BACK navigation, JSON persistence.
//! Phase 7.1: fade engine with per-fixture interpolation.

use std::collections::HashMap;
use std::time::Instant;
use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use stagelx_core::types::FixtureId;
use crate::Programmer;
use stagelx_patch::PatchRes;

// ─── Value snapshot ───────────────────────────────────────────────────────────

/// Normalised attribute values for a single fixture in a cue.
#[derive(Default, Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CueValues {
    pub dimmer: f32,
    pub pan: f32,
    pub tilt: f32,
    pub zoom: f32,
    pub strobe: f32,
    pub color: [f32; 3],
    pub gobo_index: u8,
    pub gobo_spin: f32,
}

impl CueValues {
    /// Capture current programmer values (programmer is global, so all fixtures
    /// get the same values in this Phase-6 snapshot).
    pub fn from_programmer(prog: &Programmer) -> Self {
        Self {
            dimmer: prog.dimmer,
            pan: prog.pan,
            tilt: prog.tilt,
            zoom: prog.zoom,
            strobe: prog.strobe,
            color: prog.color,
            gobo_index: prog.gobo_index as u8,
            gobo_spin: prog.gobo_spin,
        }
    }

    /// Linear interpolation between two cue values.
    /// `t` is 0.0–1.0. Strobe snaps at the end; everything else lerps.
    pub fn lerp(&self, other: &Self, t: f32) -> Self {
        let t = t.clamp(0.0, 1.0);
        Self {
            dimmer: self.dimmer + (other.dimmer - self.dimmer) * t,
            pan: self.pan + (other.pan - self.pan) * t,
            tilt: self.tilt + (other.tilt - self.tilt) * t,
            zoom: self.zoom + (other.zoom - self.zoom) * t,
            strobe: if t >= 1.0 { other.strobe } else { self.strobe },
            color: [
                self.color[0] + (other.color[0] - self.color[0]) * t,
                self.color[1] + (other.color[1] - self.color[1]) * t,
                self.color[2] + (other.color[2] - self.color[2]) * t,
            ],
            gobo_index: if t >= 1.0 { other.gobo_index } else { self.gobo_index },
            gobo_spin: self.gobo_spin + (other.gobo_spin - self.gobo_spin) * t,
        }
    }
}

// ─── Cue ──────────────────────────────────────────────────────────────────────

/// A single cue in the stack.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Cue {
    pub id: String,
    pub label: String,
    pub fade_in_ms: u32,
    pub fade_out_ms: u32,
    pub delay_ms: u32,
    /// fixture_id → attribute values.
    pub snapshot: HashMap<FixtureId, CueValues>,
}

impl Cue {
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            fade_in_ms: 0,
            fade_out_ms: 0,
            delay_ms: 0,
            snapshot: HashMap::new(),
        }
    }
}

impl Default for Cue {
    fn default() -> Self {
        Self::new("1", "Untitled")
    }
}

// ─── CueStack ─────────────────────────────────────────────────────────────────

#[derive(Resource, Clone, Debug, Default, Serialize, Deserialize)]
pub struct CueStack {
    pub cues: Vec<Cue>,
}

impl CueStack {
    /// Append a new cue captured from the current programmer state.
    /// Every fixture currently in the patch gets the same programmer values.
    pub fn record_from_programmer(
        &mut self,
        prog: &Programmer,
        patch: &PatchRes,
    ) -> usize {
        let values = CueValues::from_programmer(prog);
        let mut snapshot = HashMap::new();
        for inst in patch.0.fixtures() {
            snapshot.insert(inst.id, values.clone());
        }

        let num = self.cues.len() + 1;
        let cue = Cue {
            id: num.to_string(),
            label: format!("Cue {num}"),
            fade_in_ms: 0,
            fade_out_ms: 0,
            delay_ms: 0,
            snapshot,
        };
        self.cues.push(cue);
        self.cues.len() - 1
    }

    /// Delete a cue by index.
    pub fn delete_cue(&mut self, index: usize) {
        if index < self.cues.len() {
            self.cues.remove(index);
            // Renumber remaining cues.
            for (i, cue) in self.cues.iter_mut().enumerate() {
                cue.id = (i + 1).to_string();
            }
        }
    }

    /// Load from JSON file path. Returns Ok(()) even if file missing.
    pub fn load_from_file(path: &str) -> Option<Self> {
        let bytes = std::fs::read(path).ok()?;
        serde_json::from_slice(&bytes).ok()
    }

    /// Save to JSON file path.
    pub fn save_to_file(&self, path: &str) -> std::io::Result<()> {
        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(path, json)
    }
}

// ─── Playhead State ───────────────────────────────────────────────────────────

/// Fade state machine for cue playback.
#[derive(Clone, Debug, Default)]
pub enum PlayheadState {
    #[default]
    Idle,
    Fading {
        start: Instant,
        duration_ms: u32,
        /// Snapshot of the cue we're leaving.
        from: HashMap<FixtureId, CueValues>,
        /// Snapshot of the cue we're entering.
        to: HashMap<FixtureId, CueValues>,
    },
}

// ─── Playhead ─────────────────────────────────────────────────────────────────

#[derive(Resource, Clone, Debug)]
pub struct CuePlayhead {
    pub current_cue_index: Option<usize>,
    pub state: PlayheadState,
}

impl Default for CuePlayhead {
    fn default() -> Self {
        Self {
            current_cue_index: None,
            state: PlayheadState::Idle,
        }
    }
}

impl CuePlayhead {
    /// Advance to next cue. Returns the new index.
    pub fn go(&mut self, stack: &CueStack) -> Option<usize> {
        let next = match self.current_cue_index {
            Some(i) => (i + 1).min(stack.cues.len().saturating_sub(1)),
            None => 0,
        };
        if next < stack.cues.len() {
            self.current_cue_index = Some(next);
        }
        self.current_cue_index
    }

    /// Retreat to previous cue. Returns the new index.
    pub fn back(&mut self) -> Option<usize> {
        self.current_cue_index = match self.current_cue_index {
            Some(0) => None,
            Some(i) => Some(i - 1),
            None => None,
        };
        self.current_cue_index
    }

    /// Snap any active fade to its target immediately.
    pub fn snap_fade(&mut self) {
        if matches!(self.state, PlayheadState::Fading { .. }) {
            self.state = PlayheadState::Idle;
        }
    }
}

// ─── Events ───────────────────────────────────────────────────────────────────

/// How the RECORD button should capture values.
#[derive(Resource, Default, Clone, Copy, PartialEq, Eq, Debug)]
pub enum CaptureMode {
    #[default]
    Programmer,
    Stage,
}

/// Triggered by UI when user presses RECORD (programmer mode).
#[derive(Event, Debug, Clone)]
pub struct RecordCueEvent;

/// Triggered by UI when user presses RECORD (stage-capture mode).
/// Captures the current merged DMX output per fixture.
#[derive(Event, Debug, Clone)]
pub struct RecordStageCueEvent;

/// Triggered by UI when user presses GO.
#[derive(Event, Debug, Clone)]
pub struct GoCueEvent;

/// Triggered by UI when user presses BACK.
#[derive(Event, Debug, Clone)]
pub struct BackCueEvent;

/// Triggered by UI when user deletes a cue.
#[derive(Event, Debug, Clone, Copy)]
pub struct DeleteCueEvent(pub usize);

/// Triggered by UI when user clicks a cue row to load it into the programmer.
#[derive(Event, Debug, Clone, Copy)]
pub struct LoadCueIntoProgrammerEvent(pub usize);

/// Triggered by UI when user presses UPDATE to overwrite the active cue.
#[derive(Event, Debug, Clone)]
pub struct UpdateCueEvent;

// ─── Observer handlers ────────────────────────────────────────────────────────

pub fn on_record_cue(
    _trigger: On<RecordCueEvent>,
    mut stack: ResMut<CueStack>,
    programmer: Res<Programmer>,
    patch: Res<PatchRes>,
    mut commands: Commands,
) {
    stack.record_from_programmer(&programmer, &patch);
    commands.trigger(crate::show_file::SaveShowEvent);
}

pub fn on_go_cue(
    _trigger: On<GoCueEvent>,
    stack: Res<CueStack>,
    mut playhead: ResMut<CuePlayhead>,
) {
    let next = match playhead.current_cue_index {
        Some(i) => (i + 1).min(stack.cues.len().saturating_sub(1)),
        None => 0,
    };
    if next >= stack.cues.len() {
        return;
    }

    // If already fading, snap to target first.
    playhead.snap_fade();

    let current_idx = playhead.current_cue_index;
    let next_cue = &stack.cues[next];

    if next_cue.fade_in_ms > 0 {
        let from = current_idx
            .and_then(|i| stack.cues.get(i))
            .map(|c| c.snapshot.clone())
            .unwrap_or_default();
        playhead.state = PlayheadState::Fading {
            start: Instant::now(),
            duration_ms: next_cue.fade_in_ms,
            from,
            to: next_cue.snapshot.clone(),
        };
    }

    playhead.current_cue_index = Some(next);
}

pub fn on_back_cue(
    _trigger: On<BackCueEvent>,
    stack: Res<CueStack>,
    mut playhead: ResMut<CuePlayhead>,
) {
    let prev = match playhead.current_cue_index {
        Some(0) => None,
        Some(i) => Some(i - 1),
        None => None,
    };

    // If already fading, snap to target first.
    playhead.snap_fade();

    if let Some(current) = playhead.current_cue_index {
        let current_cue = &stack.cues[current];
        if current_cue.fade_out_ms > 0 && prev.is_some() {
            let from = current_cue.snapshot.clone();
            let to = prev
                .and_then(|i| stack.cues.get(i))
                .map(|c| c.snapshot.clone())
                .unwrap_or_default();
            playhead.state = PlayheadState::Fading {
                start: Instant::now(),
                duration_ms: current_cue.fade_out_ms,
                from,
                to,
            };
        }
    }

    playhead.current_cue_index = prev;
}

pub fn on_delete_cue(
    trigger: On<DeleteCueEvent>,
    mut stack: ResMut<CueStack>,
    mut playhead: ResMut<CuePlayhead>,
    mut commands: Commands,
) {
    let index = trigger.event().0;
    stack.delete_cue(index);
    // Adjust playhead if it pointed at or past the deleted cue.
    if let Some(idx) = playhead.current_cue_index {
        if idx >= stack.cues.len() {
            playhead.current_cue_index = stack.cues.len().checked_sub(1);
        } else if idx == index {
            playhead.current_cue_index = idx.checked_sub(1);
        }
    }
    commands.trigger(crate::show_file::SaveShowEvent);
}

pub fn on_load_cue_into_programmer(
    trigger: On<LoadCueIntoProgrammerEvent>,
    stack: Res<CueStack>,
    mut programmer: ResMut<Programmer>,
) {
    let idx = trigger.event().0;
    let Some(cue) = stack.cues.get(idx) else { return };

    // For programmer-recorded cues all fixtures share the same values.
    // For stage-captured cues, load the lowest-ID fixture as a stable representative.
    let values = cue.snapshot.iter()
        .min_by_key(|(id, _)| *id)
        .map(|(_, v)| v.clone())
        .unwrap_or_default();

    programmer.dimmer = values.dimmer;
    programmer.pan = values.pan;
    programmer.tilt = values.tilt;
    programmer.zoom = values.zoom;
    programmer.strobe = values.strobe;
    programmer.color = values.color;
    programmer.gobo_index = values.gobo_index as usize;
    programmer.gobo_spin = values.gobo_spin;
}

pub fn on_update_cue(
    _trigger: On<UpdateCueEvent>,
    mut stack: ResMut<CueStack>,
    programmer: Res<Programmer>,
    patch: Res<PatchRes>,
    playhead: Res<CuePlayhead>,
    mut commands: Commands,
) {
    let Some(idx) = playhead.current_cue_index else { return };
    let Some(cue) = stack.cues.get_mut(idx) else { return };

    let values = CueValues::from_programmer(&programmer);
    for inst in patch.0.fixtures() {
        cue.snapshot.insert(inst.id, values.clone());
    }

    commands.trigger(crate::show_file::SaveShowEvent);
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cue_values_lerp_midpoint() {
        let a = CueValues {
            dimmer: 0.0,
            pan: 0.0,
            tilt: 0.0,
            zoom: 0.0,
            strobe: 0.0,
            color: [0.0, 0.0, 0.0],
            gobo_index: 0,
            gobo_spin: 0.0,
        };
        let b = CueValues {
            dimmer: 1.0,
            pan: 1.0,
            tilt: 1.0,
            zoom: 1.0,
            strobe: 1.0,
            color: [1.0, 1.0, 1.0],
            gobo_index: 3,
            gobo_spin: 1.0,
        };
        let mid = a.lerp(&b, 0.5);
        assert!((mid.dimmer - 0.5).abs() < 0.001);
        assert!((mid.pan - 0.5).abs() < 0.001);
        assert!((mid.tilt - 0.5).abs() < 0.001);
        assert!((mid.zoom - 0.5).abs() < 0.001);
        assert!((mid.color[0] - 0.5).abs() < 0.001);
        assert!((mid.color[1] - 0.5).abs() < 0.001);
        assert!((mid.color[2] - 0.5).abs() < 0.001);
        // Strobe should hold 'from' value until t >= 1.0
        assert!((mid.strobe - 0.0).abs() < 0.001);
        // Gobo snaps like strobe
        assert_eq!(mid.gobo_index, 0);
        // Gobo spin lerps
        assert!((mid.gobo_spin - 0.5).abs() < 0.001);
    }

    #[test]
    fn cue_values_lerp_clamped() {
        let a = CueValues {
            dimmer: 0.5,
            pan: 0.5,
            tilt: 0.5,
            zoom: 0.5,
            strobe: 0.5,
            color: [0.5, 0.5, 0.5],
            gobo_index: 0,
            gobo_spin: 0.0,
        };
        let b = CueValues {
            dimmer: 1.0,
            pan: 1.0,
            tilt: 1.0,
            zoom: 1.0,
            strobe: 1.0,
            color: [1.0, 1.0, 1.0],
            gobo_index: 3,
            gobo_spin: 1.0,
        };
        // t > 1.0 should be clamped
        let over = a.lerp(&b, 2.0);
        assert!((over.dimmer - 1.0).abs() < 0.001);
        // strobe snaps at t >= 1.0
        assert!((over.strobe - 1.0).abs() < 0.001);
        // gobo snaps at t >= 1.0
        assert_eq!(over.gobo_index, 3);
        // gobo spin clamps
        assert!((over.gobo_spin - 1.0).abs() < 0.001);

        // t < 0.0 should be clamped
        let under = a.lerp(&b, -1.0);
        assert!((under.dimmer - 0.5).abs() < 0.001);
    }

    #[test]
    fn cue_values_lerp_strobe_snap() {
        let a = CueValues {
            dimmer: 0.0,
            pan: 0.0,
            tilt: 0.0,
            zoom: 0.0,
            strobe: 0.0,
            color: [0.0, 0.0, 0.0],
            gobo_index: 0,
            gobo_spin: 0.0,
        };
        let b = CueValues {
            dimmer: 1.0,
            pan: 1.0,
            tilt: 1.0,
            zoom: 1.0,
            strobe: 1.0,
            color: [1.0, 1.0, 1.0],
            gobo_index: 3,
            gobo_spin: 1.0,
        };
        // At t = 0.999, strobe and gobo should still be 'from'
        let almost = a.lerp(&b, 0.999);
        assert!((almost.strobe - 0.0).abs() < 0.001);
        // At t = 1.0, strobe snaps to 'to'
        let done = a.lerp(&b, 1.0);
        assert!((done.strobe - 1.0).abs() < 0.001);
    }

    #[test]
    fn playhead_go_advances() {
        let mut stack = CueStack::default();
        stack.cues.push(Cue::new("1", "A"));
        stack.cues.push(Cue::new("2", "B"));

        let mut ph = CuePlayhead::default();
        assert_eq!(ph.go(&stack), Some(0));
        assert_eq!(ph.go(&stack), Some(1));
        // Stops at last cue
        assert_eq!(ph.go(&stack), Some(1));
    }

    #[test]
    fn playhead_back_retreats() {
        let mut stack = CueStack::default();
        stack.cues.push(Cue::new("1", "A"));
        stack.cues.push(Cue::new("2", "B"));

        let mut ph = CuePlayhead::default();
        ph.go(&stack);
        ph.go(&stack);
        assert_eq!(ph.current_cue_index, Some(1));
        assert_eq!(ph.back(), Some(0));
        assert_eq!(ph.back(), None);
        // Stays None
        assert_eq!(ph.back(), None);
    }

    #[test]
    fn playhead_snap_fade() {
        let mut stack = CueStack::default();
        stack.cues.push(Cue::new("1", "A"));
        stack.cues[0].fade_in_ms = 1000;
        stack.cues.push(Cue::new("2", "B"));
        stack.cues[1].fade_in_ms = 1000;

        let mut ph = CuePlayhead::default();
        // Simulate a GO that starts a fade
        ph.state = PlayheadState::Fading {
            start: Instant::now(),
            duration_ms: 1000,
            from: HashMap::new(),
            to: HashMap::new(),
        };
        assert!(matches!(ph.state, PlayheadState::Fading { .. }));
        ph.snap_fade();
        assert!(matches!(ph.state, PlayheadState::Idle));
    }
}
