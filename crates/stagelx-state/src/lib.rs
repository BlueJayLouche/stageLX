//! Shared Bevy Resources and Events for stageLX.
//!
//! Lives in its own crate so that stagelx-render, stagelx-io, and stagelx-ui
//! can all depend on the resource *types* without any of them depending on
//! each other.

use bevy::prelude::*;
use stagelx_core::{patch::Patch, types::FixtureId};
use stagelx_gdtf::FixtureLibrary;

// ─── Events ───────────────────────────────────────────────────────────────────

/// Emitted by the patch UI or MVR importer when a fixture is added to the patch.
/// The render plugin responds by spawning the 3D scene entity.
#[derive(Event, Debug, Clone, Copy)]
pub struct SpawnFixtureEvent(pub FixtureId);

/// Emitted when a fixture is removed from the patch.
/// The render plugin responds by despawning the corresponding scene entity.
#[derive(Event, Debug, Clone, Copy)]
pub struct DespawnFixtureEvent(pub FixtureId);

// ─── Programmer ───────────────────────────────────────────────────────────────

/// Normalised programmer state — all channel values 0.0–1.0.
/// Written by the UI and keyboard handler; read by render and DMX output.
#[derive(Resource)]
pub struct Programmer {
    pub pan: f32,
    pub tilt: f32,
    pub dimmer: f32,
    pub color: [f32; 3],
    pub pan_range: f32,
    pub tilt_range: f32,
    /// 0.0 = narrowest beam (5°), 1.0 = widest beam (45°).
    pub zoom: f32,
    /// 0.0 = shutter open (no strobe), 1.0 = fastest strobe (~25 Hz).
    pub strobe: f32,
    /// Index into GoboLibrary (0 = open beam).
    pub gobo_index: usize,
    /// Gobo spin speed in rotations per second (0.0 = static).
    pub gobo_spin: f32,
}

impl Default for Programmer {
    fn default() -> Self {
        Self {
            pan: 0.5,
            tilt: 0.5,
            dimmer: 1.0,
            color: [1.0, 1.0, 1.0],
            pan_range: 540.0,
            tilt_range: 270.0,
            zoom: 0.0,
            strobe: 0.0,
            gobo_index: 0,
            gobo_spin: 0.0,
        }
    }
}

// ─── PatchRes ─────────────────────────────────────────────────────────────────

/// Bevy Resource wrapping the show patch (fixture → DMX address mapping).
#[derive(Resource, Default)]
pub struct PatchRes(pub Patch);

// ─── PatchEditState ───────────────────────────────────────────────────────────

/// Transient state for the patch panel "Add Fixture" form.
#[derive(Resource, Default)]
pub struct PatchEditState {
    pub selected_type_id: String,
    pub selected_mode:    String,
    pub new_name:         String,
    pub universe_str:     String,
    pub channel_str:      String,
    pub add_error:        Option<String>,
}

// ─── FixtureLibraryRes ────────────────────────────────────────────────────────

/// Bevy Resource wrapping the loaded GDTF fixture library.
#[derive(Resource, Default)]
pub struct FixtureLibraryRes {
    pub library: FixtureLibrary,
    /// Text field state for the GDTF import path input.
    pub import_path: String,
    pub import_error: Option<String>,
    /// MVR import state.
    pub mvr_import_path: String,
    pub mvr_import_error: Option<String>,
}

// ─── IoConfig ─────────────────────────────────────────────────────────────────

/// I/O configuration shared between the UI panels and the IO crate.
#[derive(Resource)]
pub struct IoConfig {
    // ── Art-Net ──────────────────────────────────────────────────────────────
    /// Local IP to bind (empty = 0.0.0.0).
    pub artnet_ip: String,
    /// Universe to output (0-based Art-Net universe number).
    pub artnet_out_universe: u16,
    /// Destination IP for TX. Empty = 255.255.255.255 (limited broadcast).
    pub artnet_dest_ip: String,
    /// Enable incoming Art-Net listener.
    pub artnet_rx_enabled: bool,
    /// Comma-separated source IPs to accept for RX. Empty = accept all.
    pub artnet_allowed_sources: String,
    pub artnet_tx_count: u64,
    pub artnet_rx_count: u64,
    pub artnet_status: String,

    // ── sACN (E1.31) ─────────────────────────────────────────────────────────
    /// Enable sACN output.
    pub sacn_tx_enabled: bool,
    /// Enable sACN input listener.
    pub sacn_rx_enabled: bool,
    /// Universe to TX on (1–63999).
    pub sacn_out_universe: u16,
    /// sACN source priority (1–200; 100 is the standard default).
    pub sacn_priority: u8,
    /// Unicast destination IP for sACN TX. Empty = use multicast 239.255.X.X.
    pub sacn_dest_ip: String,
    pub sacn_tx_count: u64,
    pub sacn_rx_count: u64,
    pub sacn_status: String,

    // ── USB DMX (Enttec USB Pro) ──────────────────────────────────────────────
    /// Enable USB DMX output.
    pub usb_tx_enabled: bool,
    /// Serial port path, e.g. "/dev/tty.usbserial-ABC123" or "COM3".
    pub usb_port: String,
    /// Universe to send on the USB device.
    pub usb_universe: u16,
    pub usb_tx_count: u64,
    pub usb_status: String,
}

impl Default for IoConfig {
    fn default() -> Self {
        Self {
            artnet_ip: String::new(),
            artnet_out_universe: 0,
            artnet_dest_ip: String::new(),
            artnet_rx_enabled: false,
            artnet_allowed_sources: String::new(),
            artnet_tx_count: 0,
            artnet_rx_count: 0,
            artnet_status: "Idle".into(),

            sacn_tx_enabled: false,
            sacn_rx_enabled: false,
            sacn_out_universe: 1,
            sacn_priority: 100,
            sacn_dest_ip: String::new(),
            sacn_tx_count: 0,
            sacn_rx_count: 0,
            sacn_status: "Idle".into(),

            usb_tx_enabled: false,
            usb_port: String::new(),
            usb_universe: 1,
            usb_tx_count: 0,
            usb_status: "Idle".into(),
        }
    }
}
