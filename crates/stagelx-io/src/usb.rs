//! USB DMX output via the Enttec USB Pro protocol.
//!
//! The Enttec USB Pro presents as a virtual COM port (FTDI chip, VID 0403).
//! Protocol: UART framing at 250 000 baud.
//!
//! Output frame (label 6 — "Output Only Send DMX Packet Request"):
//!   0x7E  label  len_lsb  len_msb  start_code  dmx[0..512]  0xE7
//!
//! Total frame = 518 bytes; baud-rate gives ≈ 22 ms/frame ≈ 45 Hz max.

use std::io::Write;
use bevy::prelude::*;
use serialport::SerialPort;
use stagelx_state::IoConfig;

pub const ENTTEC_BAUD: u32 = 250_000;
const LABEL_OUTPUT_DMX: u8 = 6;
const DMX_PAYLOAD: u16 = 513; // null start code + 512 channels

// ─── Non-send resource ────────────────────────────────────────────────────────
// SerialPort is Send but not Sync, so we register UsbDmxState as a NonSend
// resource (main-thread only) rather than a thread-safe Resource.

pub struct UsbDmxState {
    pub device: Option<Box<dyn SerialPort>>,
}

impl Default for UsbDmxState {
    fn default() -> Self {
        Self { device: None }
    }
}

// ─── Systems ──────────────────────────────────────────────────────────────────

/// Open or close the USB serial device based on `IoConfig.usb_tx_enabled`.
/// Runs in `Update` so it can respond quickly to config changes.
pub fn usb_manage_device(mut state: NonSendMut<UsbDmxState>, mut cfg: ResMut<IoConfig>) {
    let port = cfg.usb_port.trim();

    if cfg.usb_tx_enabled && state.device.is_none() {
        if port.is_empty() {
            cfg.usb_status = "No port configured".into();
            return;
        }
        match serialport::new(port, ENTTEC_BAUD)
            .timeout(std::time::Duration::from_millis(10))
            .open()
        {
            Ok(dev) => {
                info!("USB DMX opened: {}", port);
                cfg.usb_status = format!("Open: {port}");
                state.device = Some(dev);
            }
            Err(e) => {
                cfg.usb_status = format!("Open failed: {e}");
            }
        }
    }

    if !cfg.usb_tx_enabled && state.device.is_some() {
        state.device = None;
        cfg.usb_status = "Closed".into();
        info!("USB DMX closed");
    }
}

/// Send a DMX frame over the Enttec USB Pro device.
/// Runs in `FixedUpdate` at 44 Hz alongside Art-Net / sACN output.
pub fn usb_send(
    mut state: NonSendMut<UsbDmxState>,
    engine: Res<crate::artnet::DmxEngineRes>,
    mut cfg: ResMut<IoConfig>,
) {
    if !cfg.usb_tx_enabled {
        return;
    }
    let Some(ref mut dev) = state.device else { return };

    let universe = cfg.usb_universe;
    if let Some(dmx_buf) = engine.0.output_buffer(universe) {
        let frame = build_enttec_frame(dmx_buf.as_bytes());
        match dev.write_all(&frame) {
            Ok(()) => {
                cfg.usb_tx_count = cfg.usb_tx_count.saturating_add(1);
                cfg.usb_status = format!("TX u{universe}");
            }
            Err(e) => {
                cfg.usb_status = format!("TX error: {e}");
                // Drop the device; usb_manage_device will try to re-open it.
                state.device = None;
            }
        }
    }
}

// ─── Frame builder ────────────────────────────────────────────────────────────

fn build_enttec_frame(data: &[u8; 512]) -> [u8; 518] {
    let mut frame = [0u8; 518];
    frame[0] = 0x7E;                            // Start delimiter
    frame[1] = LABEL_OUTPUT_DMX;
    frame[2] = (DMX_PAYLOAD & 0xFF) as u8;      // Length LSB (513 = 0x01)
    frame[3] = ((DMX_PAYLOAD >> 8) & 0xFF) as u8; // Length MSB (513 >> 8 = 0x02)
    frame[4] = 0x00;                            // DMX null start code
    frame[5..517].copy_from_slice(data);
    frame[517] = 0xE7;                          // End delimiter
    frame
}
