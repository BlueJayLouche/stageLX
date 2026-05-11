use serde::{Deserialize, Serialize};

/// Unique identifier for a patched fixture instance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct FixtureId(pub u32);

/// A DMX address: universe + 1-based channel number (1–512).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct DmxAddress {
    pub universe: u16,
    pub channel: u16,
}

impl DmxAddress {
    pub fn new(universe: u16, channel: u16) -> Self {
        debug_assert!((1..=512).contains(&channel), "DMX channel must be 1–512");
        Self { universe, channel }
    }
}
