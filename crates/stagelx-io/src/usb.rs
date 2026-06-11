//! USB DMX output via the Enttec USB Pro protocol.
//!
//! The Enttec USB Pro presents as a virtual COM port (FTDI chip, VID 0403).
//! Protocol: UART framing at 250 000 baud.
//!
//! Output frame (label 6 — "Output Only Send DMX Packet Request"):
//!   0x7E  label  len_lsb  len_msb  start_code  dmx[0..512]  0xE7
//!
//! Total frame = 518 bytes; baud-rate gives ≈ 22 ms/frame ≈ 45 Hz max.
//!
//! TX runs in a background thread (UsbTxSink) so the 16–22 ms serial write does
//! not stall Bevy's FixedUpdate tick.  The port is opened *inside* the thread so
//! that a slow/blocking open never stalls the main thread.

use std::io::Write;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::Duration;
use bevy::prelude::*;
use crossbeam_channel::{bounded, Receiver, Sender, TryRecvError, TrySendError};
use stagelx_dmx::engine::DmxEngineRes;
use stagelx_show::ProtocolStatus;
use crate::config::UsbConfig;
use crate::stats::UsbStats;
use crate::supervisor::IoSupervisor;

pub const ENTTEC_BAUD: u32 = 250_000;
const LABEL_OUTPUT_DMX: u8 = 6;
const DMX_PAYLOAD: u16 = 513; // null start code + 512 channels

// ─── TX Command ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct UsbTxCmd {
    pub data: [u8; 512],
}

// ─── Sink ─────────────────────────────────────────────────────────────────────

pub struct UsbTxSink {
    port: String,
    baud: u32,
}

impl UsbTxSink {
    pub fn new(port: String, baud: u32) -> Self {
        Self { port, baud }
    }

    /// Spawn the TX background thread.
    ///
    /// `serialport::open()` runs **inside the thread** so a slow or blocked open
    /// never stalls Bevy's Update schedule.  The result (Ok / Err message) is
    /// sent back on `ready_tx` so the caller can update status on the next frame.
    pub fn spawn(
        self,
        rx: Receiver<UsbTxCmd>,
        shutdown: Receiver<()>,
        ready_tx: Sender<Result<(), String>>,
    ) -> JoinHandle<()> {
        std::thread::spawn(move || {
            let mut dev = match serialport::new(&self.port, self.baud)
                .timeout(Duration::from_millis(100))
                .open()
            {
                Ok(d) => {
                    let _ = ready_tx.send(Ok(()));
                    d
                }
                Err(e) => {
                    let _ = ready_tx.send(Err(e.to_string()));
                    return;
                }
            };

            loop {
                crossbeam_channel::select! {
                    recv(rx) -> cmd => {
                        match cmd {
                            Ok(cmd) => {
                                let frame = build_enttec_frame(&cmd.data);
                                if let Err(e) = dev.write_all(&frame) {
                                    warn!("USB DMX write error: {e}");
                                    // Try to re-open the device once.
                                    if let Ok(d) = serialport::new(&self.port, self.baud)
                                        .timeout(Duration::from_millis(100))
                                        .open()
                                    {
                                        dev = d;
                                    }
                                }
                            }
                            Err(_) => break,
                        }
                    }
                    recv(shutdown) -> _ => break,
                }
            }
        })
    }
}

// ─── Resource ─────────────────────────────────────────────────────────────────

#[derive(Resource)]
pub struct UsbDmxState {
    pub tx_chan: Option<Sender<UsbTxCmd>>,
    tx_shutdown: Option<Sender<()>>,
    tx_handle: Option<JoinHandle<()>>,
    /// Handle of a thread that has been asked to stop.
    /// We park it here and poll `is_finished()` each frame so we never drop it
    /// while it still holds the serial port — which would race with the next open.
    stopping: Option<JoinHandle<()>>,
    /// Receives the result of the background open attempt.
    startup_rx: Option<Receiver<Result<(), String>>>,
    pub tx_drops: Arc<AtomicU64>,
    last_port: String,
}

impl Default for UsbDmxState {
    fn default() -> Self {
        Self {
            tx_chan: None,
            tx_shutdown: None,
            tx_handle: None,
            stopping: None,
            startup_rx: None,
            tx_drops: Arc::new(AtomicU64::new(0)),
            last_port: String::new(),
        }
    }
}

// ─── Systems ──────────────────────────────────────────────────────────────────

/// Open or close the USB serial device based on `UsbConfig`.
/// Runs in `Update` so it can respond quickly to config changes.
pub fn usb_manage_device(
    mut state: ResMut<UsbDmxState>,
    cfg: Res<UsbConfig>,
    mut stats: ResMut<UsbStats>,
    supervisor: Res<IoSupervisor>,
) {
    let port = cfg.port.trim().to_string();

    // ── Poll async open result ────────────────────────────────────────────────
    if let Some(ready_rx) = &state.startup_rx {
        match ready_rx.try_recv() {
            Ok(Ok(())) => {
                info!("USB DMX TX opened: {}", state.last_port);
                stats.status = ProtocolStatus::Live;
                state.startup_rx = None;
            }
            Ok(Err(e)) => {
                warn!("USB DMX TX open failed: {e}");
                stats.status = ProtocolStatus::Error;
                state.startup_rx = None;
                state.tx_chan = None;
                // Thread already exited after sending Err; move handle to stopping
                // so we still wait for OS-level cleanup before any retry.
                state.stopping = state.tx_handle.take();
            }
            Err(TryRecvError::Empty) => {
                stats.status = ProtocolStatus::Warn; // connecting…
            }
            Err(TryRecvError::Disconnected) => {
                // Thread died before sending a result.
                stats.status = ProtocolStatus::Error;
                state.startup_rx = None;
                state.tx_chan = None;
                state.stopping = state.tx_handle.take();
            }
        }
    }

    // ── Wait for the stopping thread to release the port ─────────────────────
    if let Some(handle) = &state.stopping {
        if handle.is_finished() {
            state.stopping = None;
        } else {
            // Port still held by the old thread — sync drops and bail.
            // usb_manage_device will run again next frame.
            let local = state.tx_drops.load(Ordering::Relaxed);
            let global = supervisor.tx_drops.load(Ordering::Relaxed);
            if local > global {
                supervisor.tx_drops.store(local, Ordering::Relaxed);
            }
            return;
        }
    }

    // ── Start TX thread when enabled ──────────────────────────────────────────
    if cfg.tx_enabled && state.tx_chan.is_none() && state.startup_rx.is_none() {
        if port.is_empty() {
            stats.status = ProtocolStatus::Warn;
        } else {
            let (tx, rx) = bounded::<UsbTxCmd>(1);
            let (shutdown_tx, shutdown_rx) = bounded::<()>(1);
            let (ready_tx, ready_rx) = bounded::<Result<(), String>>(1);
            let handle = UsbTxSink::new(port.clone(), ENTTEC_BAUD)
                .spawn(rx, shutdown_rx, ready_tx);
            stats.status = ProtocolStatus::Warn; // connecting…
            state.tx_chan = Some(tx);
            state.tx_shutdown = Some(shutdown_tx);
            state.tx_handle = Some(handle);
            state.startup_rx = Some(ready_rx);
            state.last_port = port;
        }
    }

    // ── Stop TX thread when disabled ──────────────────────────────────────────
    if !cfg.tx_enabled && (state.tx_chan.is_some() || state.startup_rx.is_some()) {
        if let Some(shutdown) = state.tx_shutdown.take() {
            let _ = shutdown.try_send(());
        }
        state.tx_chan = None;
        state.startup_rx = None;
        // Park the handle instead of dropping it — the thread still holds the
        // serial port fd until it exits.  usb_manage_device will poll
        // is_finished() next frame and clear it when the OS releases the port.
        state.stopping = state.tx_handle.take();
        stats.status = ProtocolStatus::Idle;
        info!("USB DMX TX thread stopping");
    }

    // ── Sync tx_drops into supervisor ─────────────────────────────────────────
    let local = state.tx_drops.load(Ordering::Relaxed);
    let global = supervisor.tx_drops.load(Ordering::Relaxed);
    if local > global {
        supervisor.tx_drops.store(local, Ordering::Relaxed);
    }
}

/// Send a DMX frame over the Enttec USB Pro device.
/// Runs in `FixedUpdate` at 44 Hz — queues a command for the TX background thread.
pub fn usb_send(
    state: Res<UsbDmxState>,
    engine: Res<DmxEngineRes>,
    cfg: Res<UsbConfig>,
    mut stats: ResMut<UsbStats>,
) {
    if !cfg.tx_enabled {
        return;
    }
    let Some(tx) = &state.tx_chan else { return };

    let universe = cfg.universe;
    if let Some(dmx_buf) = engine.0.output_buffer(universe) {
        let cmd = UsbTxCmd {
            data: *dmx_buf.as_bytes(),
        };
        match tx.try_send(cmd) {
            Ok(_) => {
                stats.tx_count = stats.tx_count.saturating_add(1);
                stats.last_tx_at = Some(std::time::Instant::now());
                // Don't clobber Live with Warn during the connecting window.
                if stats.status != ProtocolStatus::Warn {
                    stats.status = ProtocolStatus::Live;
                }
            }
            Err(TrySendError::Full(_)) => {}
            Err(TrySendError::Disconnected(_)) => {
                stats.status = ProtocolStatus::Error;
            }
        }
    }
}

// ─── Frame builder ────────────────────────────────────────────────────────────

fn build_enttec_frame(data: &[u8; 512]) -> [u8; 518] {
    let mut frame = [0u8; 518];
    frame[0] = 0x7E;                              // Start delimiter
    frame[1] = LABEL_OUTPUT_DMX;
    frame[2] = (DMX_PAYLOAD & 0xFF) as u8;        // Length LSB (513 = 0x01)
    frame[3] = ((DMX_PAYLOAD >> 8) & 0xFF) as u8; // Length MSB (513 >> 8 = 0x02)
    frame[4] = 0x00;                              // DMX null start code
    frame[5..517].copy_from_slice(data);
    frame[517] = 0xE7;                            // End delimiter
    frame
}
