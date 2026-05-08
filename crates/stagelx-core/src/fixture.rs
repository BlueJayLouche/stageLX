use serde::{Deserialize, Serialize};
use crate::types::{DmxAddress, FixtureId};

/// Pre-computed GDTF channel offsets for the 8 most common attributes.
/// Eliminates per-tick string lookups in the DMX projection path.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default)]
pub struct DmxChannelMap {
    pub dimmer: Option<u16>,
    pub pan: Option<u16>,
    pub pan_fine: Option<u16>,
    pub tilt: Option<u16>,
    pub tilt_fine: Option<u16>,
    pub red: Option<u16>,
    pub green: Option<u16>,
    pub blue: Option<u16>,
}

/// A patched fixture instance in the show.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FixtureInstance {
    pub id: FixtureId,
    /// Human-readable label shown in the UI.
    pub name: String,
    /// GDTF FixtureTypeId GUID, links to a loaded GdtfFixtureType.
    pub fixture_type_id: String,
    /// Active DMX mode name, must match a mode in the GDTF definition.
    pub dmx_mode: String,
    /// Starting DMX address for this instance.
    pub address: DmxAddress,
    /// 3D position in the scene (x, y, z) in metres.
    pub position: [f32; 3],
    /// Euler rotation in degrees (rx, ry, rz).
    pub rotation: [f32; 3],
    /// Pre-computed channel offsets for fast DMX projection.
    pub channel_map: DmxChannelMap,
}
