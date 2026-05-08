//! Programmer → DMX channel projection.
//!
//! Uses pre-computed `DmxChannelMap` on each `FixtureInstance` to avoid
//! per-tick GDTF string lookups.

use bevy::prelude::*;
// FixtureInstance is accessed via PatchRes, no direct import needed.
use stagelx_state::{FixtureLibraryRes, PatchRes, Programmer};

// DmxEngine is accessed via DmxEngineRes, no direct import needed.

/// Write normalised programmer state into the DMX engine's "programmer" source.
/// Runs in FixedUpdate so it fires at the same rate as the protocol sends.
pub fn programmer_to_dmx(
    mut engine: ResMut<crate::engine::DmxEngineRes>,
    programmer: Res<Programmer>,
    patch: Res<PatchRes>,
    _library: Res<FixtureLibraryRes>,
) {
    let source = engine
        .0
        .get_or_add_source("programmer", 200, crate::merge::MergeStrategy::Ltp);

    let dimmer_byte = (programmer.dimmer * 255.0) as u8;
    let pan_raw = (programmer.pan * 65535.0) as u16;
    let tilt_raw = (programmer.tilt * 65535.0) as u16;
    let r = (programmer.color[0] * 255.0) as u8;
    let g = (programmer.color[1] * 255.0) as u8;
    let b = (programmer.color[2] * 255.0) as u8;

    for inst in patch.0.fixtures() {
        let base = inst.address.channel;
        let universe = inst.address.universe;
        let buf = source.universes.get_or_insert(universe);

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
}
