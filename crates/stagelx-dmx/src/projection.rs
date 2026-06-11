//! Programmer → DMX channel projection.
//!
//! Uses pre-computed `DmxChannelMap` on each `FixtureInstance` to avoid
//! per-tick GDTF string lookups.

use bevy::prelude::*;
use stagelx_patch::PatchRes;
use stagelx_show::{FixtureLibraryRes, Programmer};

/// Write normalised programmer state into the DMX engine's "programmer" source.
/// Only fixtures present in `programmer.fixture_values` are written; others are
/// left at silence (0) so they are not affected by the programmer source.
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

    // Rebuild from scratch each tick: only fixtures currently in the programmer
    // assert channels, so a removed/released fixture stops shadowing lower-priority
    // sources (e.g. OSC) instead of latching its last value forever.
    source.universes.clear_all();

    for inst in patch.0.fixtures() {
        let Some(vals) = programmer.fixture_values.get(&inst.id) else {
            continue;
        };

        let base = inst.address.channel;
        let universe = inst.address.universe;
        let buf = source.universes.get_or_insert(universe);

        let dimmer_byte    = (vals.dimmer.clamp(0.0, 1.0) * 255.0) as u8;
        let pan_raw        = (vals.pan.clamp(0.0, 1.0) * 65535.0) as u16;
        let tilt_raw       = (vals.tilt.clamp(0.0, 1.0) * 65535.0) as u16;
        let r              = (vals.color[0].clamp(0.0, 1.0) * 255.0) as u8;
        let g              = (vals.color[1].clamp(0.0, 1.0) * 255.0) as u8;
        let b              = (vals.color[2].clamp(0.0, 1.0) * 255.0) as u8;
        let gobo_byte      = (vals.gobo_index as f32 * 32.0).clamp(0.0, 255.0) as u8;
        let gobo_spin_byte = (vals.gobo_spin.clamp(0.0, 1.0) * 255.0) as u8;
        let rotation_byte  = (vals.rotation.clamp(0.0, 1.0) * 255.0) as u8;
        let strobe_byte    = (vals.strobe.clamp(0.0, 1.0) * 255.0) as u8;

        let has_map = inst.channel_map.dimmer.is_some()
            || inst.channel_map.pan.is_some()
            || inst.channel_map.tilt.is_some()
            || inst.channel_map.red.is_some()
            || inst.channel_map.color_macro.is_some()
            || inst.channel_map.rotation.is_some();

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
            if let Some(off) = inst.channel_map.color_macro {
                buf.set(base + off, vals.color_macro);
            }
            if let Some(off) = inst.channel_map.rotation {
                buf.set(base + off, rotation_byte);
            }
            if let Some(off) = inst.channel_map.strobe {
                buf.set(base + off, strobe_byte);
            }
        } else {
            // Generic 8-ch: Dimmer | Pan MSB | Pan Fine | Tilt MSB | Tilt Fine | R | G | B
            buf.set(base,     dimmer_byte);
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
