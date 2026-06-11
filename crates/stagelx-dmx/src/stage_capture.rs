//! Stage capture — record the current merged DMX output into a cue snapshot.
//!
//! Reads the DmxEngine output buffer per fixture and inverse-projects
//! channel values back to normalised `CueValues`.

use std::collections::HashMap;
use bevy::prelude::*;
use stagelx_patch::PatchRes;
use stagelx_show::{CueStack, CueValues, RecordStageCueEvent, SaveShowEvent};

use crate::engine::DmxEngineRes;

/// Observer: captures the current merged DMX output per fixture and appends
/// a new cue to the stack.
pub fn on_record_stage_cue(
    _trigger: On<RecordStageCueEvent>,
    engine: Res<DmxEngineRes>,
    patch: Res<PatchRes>,
    mut stack: ResMut<CueStack>,
    mut commands: Commands,
) {
    let mut snapshot = HashMap::new();

    for inst in patch.0.fixtures() {
        let base = inst.address.channel;
        let universe = inst.address.universe;

        let Some(buf) = engine.0.output_buffer(universe) else {
            // No output for this universe yet — use defaults.
            snapshot.insert(inst.id, CueValues::default());
            continue;
        };

        let has_map = inst.channel_map.dimmer.is_some()
            || inst.channel_map.pan.is_some()
            || inst.channel_map.tilt.is_some();

        let values = if has_map {
            // Inverse-project using the pre-computed channel map.
            let dimmer = inst
                .channel_map
                .dimmer
                .map(|off| buf.get(base + off) as f32 / 255.0)
                .unwrap_or(0.0);

            let pan = read_16bit_normalized(buf, base, inst.channel_map.pan, inst.channel_map.pan_fine);
            let tilt = read_16bit_normalized(buf, base, inst.channel_map.tilt, inst.channel_map.tilt_fine);

            let red = inst
                .channel_map
                .red
                .map(|off| buf.get(base + off) as f32 / 255.0)
                .unwrap_or(1.0);
            let green = inst
                .channel_map
                .green
                .map(|off| buf.get(base + off) as f32 / 255.0)
                .unwrap_or(1.0);
            let blue = inst
                .channel_map
                .blue
                .map(|off| buf.get(base + off) as f32 / 255.0)
                .unwrap_or(1.0);

            let gobo_index = inst
                .channel_map
                .gobo
                .map(|off| (buf.get(base + off) as f32 / 32.0) as u8)
                .unwrap_or(0);
            let gobo_spin = inst
                .channel_map
                .gobo_rotation
                .map(|off| buf.get(base + off) as f32 / 255.0)
                .unwrap_or(0.0);

            let color_macro = inst
                .channel_map
                .color_macro
                .map(|off| buf.get(base + off))
                .unwrap_or(0);
            let rotation = inst
                .channel_map
                .rotation
                .map(|off| buf.get(base + off) as f32 / 255.0)
                .unwrap_or(0.0);

            CueValues {
                dimmer,
                pan,
                tilt,
                zoom: 0.0,
                strobe: 0.0,
                color: [red, green, blue],
                gobo_index,
                gobo_spin,
                color_macro,
                rotation,
            }
        } else {
            // Generic 8-ch inverse: Dimmer | Pan MSB | Pan Fine | Tilt MSB | Tilt Fine | R | G | B
            let dimmer = buf.get(base) as f32 / 255.0;
            let pan = read_16bit_raw_normalized(buf, base + 1, base + 2);
            let tilt = read_16bit_raw_normalized(buf, base + 3, base + 4);
            let red = buf.get(base + 5) as f32 / 255.0;
            let green = buf.get(base + 6) as f32 / 255.0;
            let blue = buf.get(base + 7) as f32 / 255.0;

            CueValues {
                dimmer,
                pan,
                tilt,
                zoom: 0.0,
                strobe: 0.0,
                color: [red, green, blue],
                gobo_index: 0,
                gobo_spin: 0.0,
                color_macro: 0,
                rotation: 0.0,
            }
        };

        snapshot.insert(inst.id, values);
    }

    let num = stack.cues.len() + 1;
    let cue = stagelx_show::Cue {
        id: num.to_string(),
        label: format!("Stage {num}"),
        fade_in_ms: 2000, // Default 2s fade for stage-captured cues.
        fade_out_ms: 2000,
        delay_ms: 0,
        snapshot,
    };
    stack.cues.push(cue);
    commands.trigger(SaveShowEvent);
}

// ─── Helpers ──────────────────────────────────────────────────────────────────

/// Read a 16-bit value (MSB + optional fine) from a DMX buffer and normalise
/// to 0.0–1.0.
fn read_16bit_normalized(
    buf: &stagelx_core::universe::DmxBuffer,
    base: u16,
    msb_offset: Option<u16>,
    fine_offset: Option<u16>,
) -> f32 {
    let msb = msb_offset.map(|off| buf.get(base + off)).unwrap_or(128);
    let fine = fine_offset.map(|off| buf.get(base + off)).unwrap_or(0);
    let raw = ((msb as u16) << 8) | (fine as u16);
    (raw as f32 / 65535.0).clamp(0.0, 1.0)
}

/// Read a 16-bit value from raw MSB/fine channels and normalise.
fn read_16bit_raw_normalized(
    buf: &stagelx_core::universe::DmxBuffer,
    msb_ch: u16,
    fine_ch: u16,
) -> f32 {
    let msb = buf.get(msb_ch);
    let fine = buf.get(fine_ch);
    let raw = ((msb as u16) << 8) | (fine as u16);
    (raw as f32 / 65535.0).clamp(0.0, 1.0)
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use stagelx_core::universe::DmxBuffer;

    #[test]
    fn round_trip_8bit() {
        let mut buf = DmxBuffer::default();
        buf.set(1, 128);
        assert_eq!(buf.get(1), 128);
    }

    #[test]
    fn round_trip_16bit_pan_tilt() {
        // Forward: pan = 0.5 → raw = 32767 → msb = 127, fine = 255
        let pan = 0.5_f32;
        let raw = (pan * 65535.0) as u16;
        let msb = (raw >> 8) as u8;
        let fine = (raw & 0xFF) as u8;

        let mut buf = DmxBuffer::default();
        buf.set(1, msb);
        buf.set(2, fine);

        // Inverse
        let recovered = read_16bit_raw_normalized(&buf, 1, 2);
        assert!((recovered - 0.5).abs() < 0.001);
    }

    #[test]
    fn inverse_dimmer_full() {
        let mut buf = DmxBuffer::default();
        buf.set(1, 255);
        let dimmer = buf.get(1) as f32 / 255.0;
        assert!((dimmer - 1.0).abs() < 0.001);
    }

    #[test]
    fn inverse_dimmer_zero() {
        let mut buf = DmxBuffer::default();
        buf.set(1, 0);
        let dimmer = buf.get(1) as f32 / 255.0;
        assert!((dimmer - 0.0).abs() < 0.001);
    }

    #[test]
    fn inverse_color_channels() {
        let mut buf = DmxBuffer::default();
        buf.set(5, 128);
        buf.set(6, 64);
        buf.set(7, 255);
        let r = buf.get(5) as f32 / 255.0;
        let g = buf.get(6) as f32 / 255.0;
        let b = buf.get(7) as f32 / 255.0;
        assert!((r - 128.0 / 255.0).abs() < 0.001);
        assert!((g - 64.0 / 255.0).abs() < 0.001);
        assert!((b - 1.0).abs() < 0.001);
    }
}
