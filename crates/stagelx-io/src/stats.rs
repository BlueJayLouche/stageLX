//! Per-protocol telemetry Resources.
//!
//! These are written by the IO systems (often per-frame) and read by the UI.
//! Splitting them from Config eliminates exclusive `ResMut` contention between
//! the IO send/receive systems and the config UI panel.

use bevy::prelude::*;
use stagelx_show::ProtocolStatus;
use std::time::Instant;

// ─── Art-Net ──────────────────────────────────────────────────────────────────

#[derive(Resource, Default)]
pub struct ArtNetStats {
    pub tx_count: u64,
    pub rx_count: u64,
    pub status: ProtocolStatus,
    pub last_tx_at: Option<Instant>,
    pub last_rx_at: Option<Instant>,
}

// ─── sACN (E1.31) ─────────────────────────────────────────────────────────────

#[derive(Resource, Default)]
pub struct SacnStats {
    pub tx_count: u64,
    pub rx_count: u64,
    pub status: ProtocolStatus,
    pub last_tx_at: Option<Instant>,
    pub last_rx_at: Option<Instant>,
}

// ─── USB DMX (Enttec USB Pro) ─────────────────────────────────────────────────

#[derive(Resource, Default)]
pub struct UsbStats {
    pub tx_count: u64,
    pub status: ProtocolStatus,
    pub last_tx_at: Option<Instant>,
}

// ─── MIDI input ───────────────────────────────────────────────────────────────

#[derive(Resource, Default)]
pub struct MidiStats {
    pub rx_count: u64,
    pub status: ProtocolStatus,
    pub last_rx_at: Option<Instant>,
}

// ─── OSC input ────────────────────────────────────────────────────────────────

#[derive(Resource, Default)]
pub struct OscStats {
    pub rx_count: u64,
    pub status: ProtocolStatus,
    pub last_rx_at: Option<Instant>,
    /// Last N received OSC address patterns (for debugging).
    pub last_messages: Vec<String>,
}
