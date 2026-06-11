/// Number of channels in a single DMX universe.
pub const DMX_CHANNELS: usize = 512;

/// A single 512-byte DMX universe buffer.
///
/// Tracks which channels have been explicitly written so that merge operations
/// only affect touched channels.  This prevents a high-priority LTP source from
/// zeroing out channels it never set (e.g. the Programmer stomping on OSC data).
#[derive(Clone)]
pub struct DmxBuffer {
    data: [u8; DMX_CHANNELS],
    /// True for every channel that was explicitly set since the last `clear()`.
    written: [bool; DMX_CHANNELS],
}

impl std::fmt::Debug for DmxBuffer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let active = self.written.iter().filter(|&&b| b).count();
        f.debug_struct("DmxBuffer")
            .field("active_channels", &active)
            .finish()
    }
}

impl Default for DmxBuffer {
    fn default() -> Self {
        Self {
            data: [0; DMX_CHANNELS],
            written: [false; DMX_CHANNELS],
        }
    }
}

impl DmxBuffer {
    /// Set channel `ch` (1-based) to `value`.
    pub fn set(&mut self, ch: u16, value: u8) {
        if let Some(i) = ch.checked_sub(1).map(|i| i as usize) {
            if let Some(slot) = self.data.get_mut(i) {
                *slot = value;
                self.written[i] = true;
            }
        }
    }

    /// Get channel `ch` (1-based).
    pub fn get(&self, ch: u16) -> u8 {
        ch.checked_sub(1)
            .and_then(|i| self.data.get(i as usize).copied())
            .unwrap_or(0)
    }

    pub fn as_bytes(&self) -> &[u8; DMX_CHANNELS] {
        &self.data
    }

    /// HTP merge: keep the higher value for each channel that `other` has written.
    pub fn merge_htp(&mut self, other: &DmxBuffer) {
        for i in 0..DMX_CHANNELS {
            if other.written[i] {
                self.data[i] = self.data[i].max(other.data[i]);
            }
        }
    }

    /// LTP merge: overwrite only the channels that `other` has explicitly written.
    /// Untouched channels in `other` are left alone, solving the "whole-universe
    /// stomp" problem when multiple fixtures share a universe.
    pub fn merge_ltp(&mut self, other: &DmxBuffer) {
        for i in 0..DMX_CHANNELS {
            if other.written[i] {
                self.data[i] = other.data[i];
            }
        }
    }

    pub fn clear(&mut self) {
        self.data.fill(0);
        self.written.fill(false);
    }

    /// Overwrite channels from a raw slice; zeros any channels beyond the slice length.
    /// All channels in the copied range are marked written.
    pub fn copy_from_slice(&mut self, src: &[u8]) {
        let n = src.len().min(DMX_CHANNELS);
        self.data[..n].copy_from_slice(&src[..n]);
        self.data[n..].fill(0);
        self.written[..n].fill(true);
        self.written[n..].fill(false);
    }
}

/// Manages multiple DMX universes indexed by universe number.
#[derive(Debug, Default)]
pub struct UniverseSet {
    buffers: std::collections::HashMap<u16, DmxBuffer>,
}

impl UniverseSet {
    pub fn get_or_insert(&mut self, universe: u16) -> &mut DmxBuffer {
        self.buffers.entry(universe).or_insert_with(DmxBuffer::default)
    }

    pub fn get(&self, universe: u16) -> Option<&DmxBuffer> {
        self.buffers.get(&universe)
    }

    /// Reset every buffer to silence (data + written mask), keeping the
    /// allocations. Used by rebuilt-each-tick sources (programmer, cue playback)
    /// so a channel they stop writing is released instead of latching forever.
    pub fn clear_all(&mut self) {
        for buf in self.buffers.values_mut() {
            buf.clear();
        }
    }

    pub fn universes(&self) -> impl Iterator<Item = (u16, &DmxBuffer)> {
        self.buffers.iter().map(|(id, buf)| (*id, buf))
    }
}
