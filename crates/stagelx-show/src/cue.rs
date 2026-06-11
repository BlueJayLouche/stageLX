//! Cue system data model and playback state.
//!
//! Phase 6.4 scope: basic cue stack, instant playback (no fade engine),
//! record-from-programmer, GO/BACK navigation, JSON persistence.
//! Phase 7.1: fade engine with per-fixture interpolation.

use std::collections::{HashMap, HashSet};
use std::time::Instant;
use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use stagelx_core::types::FixtureId;
use crate::{Programmer, ProgrammerValues};
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
    /// Raw DMX (0–255) for the ColorMacro1 preset channel on FX fixtures.
    /// `#[serde(default)]` keeps older show files (without this field) loadable.
    #[serde(default)]
    pub color_macro: u8,
    /// Normalised motor rotation speed/index (0.0–1.0 → DMX 0–255) on FX fixtures.
    #[serde(default)]
    pub rotation: f32,
}

impl CueValues {
    /// Capture from the programmer's global display fields (used as a fallback
    /// for fixtures that were never individually selected).
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
            color_macro: prog.color_macro,
            rotation: prog.rotation,
        }
    }

    /// Capture from a per-fixture `ProgrammerValues` entry.
    pub fn from_programmer_values(pv: &ProgrammerValues) -> Self {
        Self {
            dimmer: pv.dimmer,
            pan: pv.pan,
            tilt: pv.tilt,
            zoom: pv.zoom,
            strobe: pv.strobe,
            color: pv.color,
            gobo_index: pv.gobo_index as u8,
            gobo_spin: pv.gobo_spin,
            color_macro: pv.color_macro,
            rotation: pv.rotation,
        }
    }

    /// Convert to a `ProgrammerValues` snapshot.
    pub fn to_programmer_values(&self) -> ProgrammerValues {
        ProgrammerValues {
            dimmer: self.dimmer,
            pan: self.pan,
            tilt: self.tilt,
            zoom: self.zoom,
            strobe: self.strobe,
            color: self.color,
            gobo_index: self.gobo_index as usize,
            gobo_spin: self.gobo_spin,
            color_macro: self.color_macro,
            rotation: self.rotation,
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
            // Macro presets are discrete selections — snap at the end like gobo.
            color_macro: if t >= 1.0 { other.color_macro } else { self.color_macro },
            rotation: self.rotation + (other.rotation - self.rotation) * t,
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
        let mut snapshot = HashMap::new();
        for inst in patch.0.fixtures() {
            // Use the per-fixture stored values. A fixture that was never
            // programmed records clean defaults — NOT the shared display fields,
            // which would leak the last-edited fixture's values into it.
            let values = prog.fixture_values.get(&inst.id)
                .map(CueValues::from_programmer_values)
                .unwrap_or_else(|| CueValues::from_programmer_values(&ProgrammerValues::default()));
            snapshot.insert(inst.id, values);
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

/// Jump directly to a cue by its 1-based display number (e.g. OSC /cue/2).
/// Snaps any in-progress fade, then applies the target cue with its fade_in time.
#[derive(Event, Debug, Clone, Copy)]
pub struct JumpToCueEvent(pub usize);

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

/// Push every fixture's values from a cue snapshot into the programmer's
/// per-fixture store and update the display fields from the lowest-ID fixture.
/// This ensures programmer_to_dmx (priority 200) outputs the cue, not stale values.
fn apply_cue_to_programmer(cue: &Cue, programmer: &mut Programmer) {
    for (id, vals) in &cue.snapshot {
        programmer.fixture_values.insert(*id, vals.to_programmer_values());
    }
    if let Some((_, vals)) = cue.snapshot.iter().min_by_key(|(id, _)| *id) {
        programmer.load_values(&vals.to_programmer_values());
    }
    // Signal programmer_update to reset the baseline instead of writing back.
    // Without this, programmer_update sees display ≠ old baseline and overwrites
    // all selected fixtures with the just-loaded values, destroying per-fixture data.
    programmer.cue_load_pending = true;
}

/// Build a fade `from` snapshot out of the programmer's current per-fixture
/// values, for exactly the fixtures present in `target`. This makes a GO/BACK a
/// true crossfade from whatever is live right now (including manual edits).
fn snapshot_from_programmer(
    programmer: &Programmer,
    target: &HashMap<FixtureId, CueValues>,
) -> HashMap<FixtureId, CueValues> {
    target
        .keys()
        .map(|id| (*id, CueValues::from_programmer_values(&programmer.values_for(*id))))
        .collect()
}

/// Drive an in-progress cue fade. Each frame, interpolate every fixture between
/// the fade's `from` and `to` snapshots and write the result straight into the
/// programmer's per-fixture store (priority 200), so both the DMX output and the
/// 3-D viewport animate together. Sets `cue_load_pending` so `programmer_update`
/// yields instead of overwriting the fade with the editor display.
pub fn advance_cue_fade(
    mut playhead: ResMut<CuePlayhead>,
    mut programmer: ResMut<Programmer>,
) {
    let done = {
        let PlayheadState::Fading { start, duration_ms, from, to } = &playhead.state else {
            return;
        };
        let t = (start.elapsed().as_secs_f32() * 1000.0 / (*duration_ms).max(1) as f32)
            .clamp(0.0, 1.0);
        let ids: HashSet<FixtureId> = from.keys().chain(to.keys()).copied().collect();
        for id in ids {
            let from_v = from.get(&id).cloned().unwrap_or_default();
            let to_v = to.get(&id).cloned().unwrap_or_default();
            programmer
                .fixture_values
                .insert(id, from_v.lerp(&to_v, t).to_programmer_values());
        }
        t >= 1.0
    };

    // Keep the editor from fighting the fade; updated every fade frame.
    programmer.cue_load_pending = true;
    if done {
        playhead.state = PlayheadState::Idle;
    }
}

pub fn on_go_cue(
    _trigger: On<GoCueEvent>,
    stack: Res<CueStack>,
    mut playhead: ResMut<CuePlayhead>,
    mut programmer: ResMut<Programmer>,
) {
    let next = match playhead.current_cue_index {
        Some(i) => (i + 1).min(stack.cues.len().saturating_sub(1)),
        None => 0,
    };
    if next >= stack.cues.len() {
        return;
    }

    // If already fading, snap to where we are; the new move starts from there.
    playhead.snap_fade();

    let next_cue = &stack.cues[next];
    playhead.current_cue_index = Some(next);

    if next_cue.fade_in_ms > 0 {
        // Crossfade from the current live state into the next cue. advance_cue_fade
        // writes the interpolation into the programmer (priority 200), so the DMX
        // output and the 3-D view both animate.
        let from = snapshot_from_programmer(&programmer, &next_cue.snapshot);
        playhead.state = PlayheadState::Fading {
            start: Instant::now(),
            duration_ms: next_cue.fade_in_ms,
            from,
            to: next_cue.snapshot.clone(),
        };
        programmer.cue_load_pending = true;
    } else {
        // Instant cut: push values into the programmer immediately.
        apply_cue_to_programmer(next_cue, &mut programmer);
    }
}

pub fn on_back_cue(
    _trigger: On<BackCueEvent>,
    stack: Res<CueStack>,
    mut playhead: ResMut<CuePlayhead>,
    mut programmer: ResMut<Programmer>,
) {
    let prev = match playhead.current_cue_index {
        Some(0) => None,
        Some(i) => Some(i - 1),
        None => None,
    };

    // If already fading, snap to where we are; the new move starts from there.
    playhead.snap_fade();

    // The fade-out time comes from the cue we're leaving.
    let fade_out_ms = playhead
        .current_cue_index
        .and_then(|i| stack.cues.get(i))
        .map(|c| c.fade_out_ms)
        .unwrap_or(0);

    playhead.current_cue_index = prev;

    // Backed out before the first cue — nothing to fade to.
    let Some(idx) = prev else { return };
    let Some(target) = stack.cues.get(idx) else { return };

    if fade_out_ms > 0 {
        let from = snapshot_from_programmer(&programmer, &target.snapshot);
        playhead.state = PlayheadState::Fading {
            start: Instant::now(),
            duration_ms: fade_out_ms,
            from,
            to: target.snapshot.clone(),
        };
        programmer.cue_load_pending = true;
    } else {
        apply_cue_to_programmer(target, &mut programmer);
    }
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
    programmer.cue_load_pending = true;
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

    for inst in patch.0.fixtures() {
        let values = programmer.fixture_values.get(&inst.id)
            .map(CueValues::from_programmer_values)
            .unwrap_or_else(|| CueValues::from_programmer_values(&ProgrammerValues::default()));
        cue.snapshot.insert(inst.id, values);
    }

    commands.trigger(crate::show_file::SaveShowEvent);
}

pub fn on_jump_to_cue(
    trigger: On<JumpToCueEvent>,
    stack: Res<CueStack>,
    mut playhead: ResMut<CuePlayhead>,
    mut programmer: ResMut<Programmer>,
) {
    // Event carries a 1-based display number; convert to 0-based index.
    let display_num = trigger.event().0;
    if display_num == 0 { return; }
    let idx = display_num - 1;
    let Some(target_cue) = stack.cues.get(idx) else {
        warn!("JumpToCue: cue {} not found (stack has {})", display_num, stack.cues.len());
        return;
    };

    playhead.snap_fade();

    if target_cue.fade_in_ms > 0 {
        let from = playhead.current_cue_index
            .and_then(|i| stack.cues.get(i))
            .map(|c| c.snapshot.clone())
            .unwrap_or_default();
        playhead.state = PlayheadState::Fading {
            start: std::time::Instant::now(),
            duration_ms: target_cue.fade_in_ms,
            from,
            to: target_cue.snapshot.clone(),
        };
    }

    playhead.current_cue_index = Some(idx);
    apply_cue_to_programmer(target_cue, &mut programmer);
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
            color_macro: 0,
            rotation: 0.0,
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
            color_macro: 0,
            rotation: 0.0,
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
            color_macro: 0,
            rotation: 0.0,
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
            color_macro: 0,
            rotation: 0.0,
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
            color_macro: 0,
            rotation: 0.0,
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
            color_macro: 0,
            rotation: 0.0,
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
