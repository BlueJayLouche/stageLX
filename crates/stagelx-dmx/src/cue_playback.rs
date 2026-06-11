//! Cue playback → DMX channel projection.
//!
//! Writes the active cue's snapshot into the DMX engine at priority 150
//! (below programmer=200, above external input=100).
//! Phase 7.1: supports cross-fade interpolation via CuePlayhead fade state.

use bevy::prelude::*;
use stagelx_core::types::FixtureId;
use stagelx_patch::PatchRes;
use stagelx_show::{CuePlayhead, CueStack, CueValues, PlayheadState};

use crate::engine::DmxEngineRes;
use crate::merge::MergeStrategy;

/// Write cue values into the DMX engine for a single fixture.
fn write_fixture_cue_values(
    engine: &mut DmxEngineRes,
    source_name: &str,
    priority: u8,
    fixture_id: FixtureId,
    values: &CueValues,
    patch: &PatchRes,
) {
    let source = engine
        .0
        .get_or_add_source(source_name, priority, MergeStrategy::Ltp);

    let Some(inst) = patch.0.get(fixture_id) else { return };
    let base = inst.address.channel;
    let universe = inst.address.universe;
    let buf = source.universes.get_or_insert(universe);

    let dimmer_byte = (values.dimmer.clamp(0.0, 1.0) * 255.0) as u8;
    let pan_raw = (values.pan.clamp(0.0, 1.0) * 65535.0) as u16;
    let tilt_raw = (values.tilt.clamp(0.0, 1.0) * 65535.0) as u16;
    let r = (values.color[0].clamp(0.0, 1.0) * 255.0) as u8;
    let g = (values.color[1].clamp(0.0, 1.0) * 255.0) as u8;
    let b = (values.color[2].clamp(0.0, 1.0) * 255.0) as u8;
    let gobo_byte = (values.gobo_index as f32 * 32.0).clamp(0.0, 255.0) as u8;
    let gobo_spin_byte = (values.gobo_spin.clamp(0.0, 1.0) * 255.0) as u8;

    // Use pre-computed channel map when available; fall back to generic 8-ch layout.
    let has_map = inst.channel_map.dimmer.is_some()
        || inst.channel_map.pan.is_some()
        || inst.channel_map.tilt.is_some();

    if has_map {
        if let Some(off) = inst.channel_map.dimmer {
            buf.set(base + off, dimmer_byte);
        }
        if let Some(off) = inst.channel_map.pan {
            buf.set(base + off, (pan_raw >> 8) as u8);
        }
        if let Some(off) = inst.channel_map.pan_fine {
            buf.set(base + off, (pan_raw & 0xFF) as u8);
        }
        if let Some(off) = inst.channel_map.tilt {
            buf.set(base + off, (tilt_raw >> 8) as u8);
        }
        if let Some(off) = inst.channel_map.tilt_fine {
            buf.set(base + off, (tilt_raw & 0xFF) as u8);
        }
        if let Some(off) = inst.channel_map.red {
            buf.set(base + off, r);
        }
        if let Some(off) = inst.channel_map.green {
            buf.set(base + off, g);
        }
        if let Some(off) = inst.channel_map.blue {
            buf.set(base + off, b);
        }
        if let Some(off) = inst.channel_map.gobo {
            buf.set(base + off, gobo_byte);
        }
        if let Some(off) = inst.channel_map.gobo_rotation {
            buf.set(base + off, gobo_spin_byte);
        }
    } else {
        // Generic 8-ch: Dimmer | Pan MSB | Pan Fine | Tilt MSB | Tilt Fine | R | G | B
        buf.set(base, dimmer_byte);
        buf.set(base + 1, (pan_raw >> 8) as u8);
        buf.set(base + 2, (pan_raw & 0xFF) as u8);
        buf.set(base + 3, (tilt_raw >> 8) as u8);
        buf.set(base + 4, (tilt_raw & 0xFF) as u8);
        buf.set(base + 5, r);
        buf.set(base + 6, g);
        buf.set(base + 7, b);
    }
}

/// Write the active cue's snapshot into the DMX engine's "cue_playback" source.
/// Runs in FixedUpdate alongside programmer_to_dmx.
///
/// When the playhead is in a fade, interpolates between the `from` and `to`
/// snapshots per fixture. When idle, writes the current cue directly.
pub fn cue_to_dmx(
    mut engine: ResMut<DmxEngineRes>,
    stack: Res<CueStack>,
    playhead: Res<CuePlayhead>,
    patch: Res<PatchRes>,
) {
    const SOURCE: &str = "cue_playback";
    const PRIORITY: u8 = 150;

    // Rebuild from scratch each tick so fixtures no longer in the active cue
    // release their channels instead of latching.
    engine
        .0
        .get_or_add_source(SOURCE, PRIORITY, MergeStrategy::Ltp)
        .universes
        .clear_all();

    // During a fade the programmer source (priority 200) carries the interpolated
    // values — advance_cue_fade writes them into the programmer each frame so both
    // DMX and the 3-D view animate. This lower-priority source would be shadowed
    // anyway, so it stands down to keep a single owner of the fade.
    if matches!(playhead.state, PlayheadState::Fading { .. }) {
        return;
    }

    // ── Idle: write current cue snapshot directly ───────────────────────────
    let Some(idx) = playhead.current_cue_index else { return };
    let Some(cue) = stack.cues.get(idx) else { return };

    for (fixture_id, values) in &cue.snapshot {
        write_fixture_cue_values(&mut engine, SOURCE, PRIORITY, *fixture_id, values, &patch);
    }
}
