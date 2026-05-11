//! Per-protocol configuration Resources.
//!
//! These live in `stagelx-io` (not `stagelx-state`) so that each protocol system
//! can borrow its config independently, eliminating scheduler contention.

use bevy::prelude::*;

// ─── Art-Net ──────────────────────────────────────────────────────────────────

#[derive(Resource)]
pub struct ArtNetConfig {
    /// Local IP to bind (empty = 0.0.0.0).
    pub ip: String,
    /// Universe to output (0-based Art-Net universe number).
    pub out_universe: u16,
    /// Destination IP for TX. Empty = 255.255.255.255 (limited broadcast).
    pub dest_ip: String,
    /// Enable Art-Net output.
    pub tx_enabled: bool,
    /// Enable incoming Art-Net listener.
    pub rx_enabled: bool,
    /// Comma-separated source IPs to accept for RX. Empty = accept all.
    pub allowed_sources: String,
    /// Enable Art-Net node discovery (ArtPoll/ArtPollReply).
    pub discovery_enabled: bool,
}

impl Default for ArtNetConfig {
    fn default() -> Self {
        Self {
            ip: String::new(),
            out_universe: 0,
            dest_ip: String::new(),
            tx_enabled: true,
            rx_enabled: false,
            allowed_sources: String::new(),
            discovery_enabled: false,
        }
    }
}

// ─── sACN (E1.31) ─────────────────────────────────────────────────────────────

#[derive(Resource)]
pub struct SacnConfig {
    /// Enable sACN output.
    pub tx_enabled: bool,
    /// Enable sACN input listener.
    pub rx_enabled: bool,
    /// Universe to TX on (1–63999).
    pub out_universe: u16,
    /// sACN source priority (1–200; 100 is the standard default).
    pub priority: u8,
    /// Unicast destination IP for sACN TX. Empty = use multicast 239.255.X.X.
    pub dest_ip: String,
    /// Universe to listen on for RX (for multicast join). 0 = same as out_universe.
    pub rx_universe: u16,
}

impl Default for SacnConfig {
    fn default() -> Self {
        Self {
            tx_enabled: false,
            rx_enabled: false,
            out_universe: 1,
            priority: 100,
            dest_ip: String::new(),
            rx_universe: 0,
        }
    }
}

// ─── USB DMX (Enttec USB Pro) ─────────────────────────────────────────────────

#[derive(Resource)]
pub struct UsbConfig {
    /// Enable USB DMX output.
    pub tx_enabled: bool,
    /// Serial port path, e.g. "/dev/tty.usbserial-ABC123" or "COM3".
    pub port: String,
    /// Universe to send on the USB device.
    pub universe: u16,
}

impl Default for UsbConfig {
    fn default() -> Self {
        Self {
            tx_enabled: false,
            port: String::new(),
            universe: 1,
        }
    }
}

// ─── MIDI input ───────────────────────────────────────────────────────────────

#[derive(Resource)]
pub struct MidiConfig {
    /// Enable MIDI input.
    pub enabled: bool,
    /// MIDI port name (exact string from midir port listing).
    pub port: String,
    /// When true, MIDI CCs control the fixtures currently selected in the patch
    /// panel instead of the global Programmer.
    pub target_selected_fixtures: bool,
    /// MIDI CC numbers mapped to programmer attributes.
    pub cc_dimmer: u8,
    pub cc_pan: u8,
    pub cc_tilt: u8,
    pub cc_red: u8,
    pub cc_green: u8,
    pub cc_blue: u8,
    pub cc_zoom: u8,
    pub cc_strobe: u8,
    /// MIDI note numbers for cue triggers (None = disabled).
    pub note_go: Option<u8>,
    pub note_back: Option<u8>,
}

impl Default for MidiConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            port: String::new(),
            target_selected_fixtures: false,
            cc_dimmer: 7,
            cc_pan: 10,
            cc_tilt: 11,
            cc_red: 12,
            cc_green: 13,
            cc_blue: 14,
            cc_zoom: 15,
            cc_strobe: 16,
            note_go: None,
            note_back: None,
        }
    }
}

// ─── OSC input ────────────────────────────────────────────────────────────────

#[derive(Resource)]
pub struct OscConfig {
    /// Enable OSC input listener.
    pub enabled: bool,
    /// UDP port for OSC listener (default 8000).
    pub port: u16,
}

impl Default for OscConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            port: 8000,
        }
    }
}
