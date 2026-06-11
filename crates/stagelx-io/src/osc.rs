//! OSC input via `rosc` over UDP.
//!
//! Message schema handled here:
//!   /fixture/{id}/{attr}   f32   — set attribute 0.0–1.0 on all patched fixtures
//!   /fixture/{id}/color    fff   — set RGB 0.0–1.0
//!
//! The socket runs non-blocking in Bevy's Update loop (same pattern as Art-Net TX).
//! A background thread is used for blocking recv so the main thread is never stalled.

use bevy::prelude::*;
use crossbeam_channel::{bounded, Receiver, Sender, TrySendError};
use rosc::{OscPacket, OscType};
use std::net::{IpAddr, Ipv4Addr, SocketAddr, UdpSocket};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::Duration;
use stagelx_patch::PatchRes;
use stagelx_show::{BackCueEvent, GoCueEvent, JumpToCueEvent, Programmer, ProtocolStatus};
use stagelx_core::types::FixtureId;
use crate::config::OscConfig;
use crate::stats::OscStats;
use crate::supervisor::{IoSource, IoSupervisor, create_tuned_udp_socket};

// ─── Incoming message ──────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct OscMsg {
    pub addr: String,
    pub args: Vec<OscType>,
}

// ─── IoSource implementation ──────────────────────────────────────────────────

pub struct OscRxSource {
    socket: UdpSocket,
    drops: Arc<AtomicU64>,
    raw_rx: Arc<AtomicU64>,
    decode_errors: Arc<AtomicU64>,
}

impl OscRxSource {
    pub fn new(
        socket: UdpSocket,
        drops: Arc<AtomicU64>,
        raw_rx: Arc<AtomicU64>,
        decode_errors: Arc<AtomicU64>,
    ) -> Self {
        Self { socket, drops, raw_rx, decode_errors }
    }
}

impl IoSource for OscRxSource {
    type Msg = OscMsg;

    fn start(&self, tx: Sender<Self::Msg>, shutdown: Receiver<()>) -> std::io::Result<JoinHandle<()>> {
        let socket = self.socket.try_clone()?;
        socket.set_nonblocking(false)?;
        socket.set_read_timeout(Some(Duration::from_millis(100)))?;
        let drops = Arc::clone(&self.drops);
        let raw_rx = Arc::clone(&self.raw_rx);
        let decode_errors = Arc::clone(&self.decode_errors);

        Ok(std::thread::spawn(move || {
            let mut buf = vec![0u8; 1536];
            loop {
                match socket.recv_from(&mut buf) {
                    Ok((n, src)) => {
                        raw_rx.fetch_add(1, Ordering::Relaxed);
                        match rosc::decoder::decode(&buf[..n]) {
                            Ok(pkt) => forward_packet(pkt, &tx, &drops),
                            Err(e) => {
                                decode_errors.fetch_add(1, Ordering::Relaxed);
                                warn!("OSC decode error from {src}: {e}  ({n} bytes: {:02x?})", &buf[..n.min(16)]);
                            }
                        }
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::TimedOut
                           || e.kind() == std::io::ErrorKind::WouldBlock => {
                        if shutdown.try_recv().is_ok() {
                            break;
                        }
                    }
                    Err(e) => {
                        warn!("OSC socket error: {e}");
                        break;
                    }
                }
            }
        }))
    }
}

fn forward_packet(pkt: OscPacket, tx: &Sender<OscMsg>, drops: &Arc<AtomicU64>) {
    match pkt {
        OscPacket::Message(m) => {
            if let Err(TrySendError::Full(_)) = tx.try_send(OscMsg { addr: m.addr, args: m.args }) {
                drops.fetch_add(1, Ordering::Relaxed);
            }
        }
        OscPacket::Bundle(b) => {
            for p in b.content {
                forward_packet(p, tx, drops);
            }
        }
    }
}

// ─── Resource ─────────────────────────────────────────────────────────────────

#[derive(Resource)]
pub struct OscState {
    pub rx: Receiver<OscMsg>,
    tx: Sender<OscMsg>,
    pub bound_port: Option<u16>,
    /// Cached local LAN IP shown in the UI (e.g. "192.168.1.42").
    pub local_ip: Option<String>,
    /// Raw UDP datagrams received (incremented before decode, so non-zero means
    /// packets are reaching the socket even if rosc rejects them).
    pub raw_rx_count: Arc<AtomicU64>,
    /// UDP datagrams that arrived but failed rosc decode.
    pub decode_errors: Arc<AtomicU64>,
    /// Clone of the socket held so we can shut it down when disabled.
    socket: Option<UdpSocket>,
    /// Shared drop counter.
    pub rx_drops: Arc<AtomicU64>,
    /// Shutdown sender for the background thread.
    shutdown: Option<Sender<()>>,
    /// Background thread handle.
    handle: Option<JoinHandle<()>>,
}

/// Sniff the local LAN IP by connecting a UDP socket to an external address
/// (no packets sent). Returns None on failure.
fn probe_local_ip() -> Option<String> {
    let probe = UdpSocket::bind("0.0.0.0:0").ok()?;
    probe.connect("8.8.8.8:80").ok()?;
    let addr = probe.local_addr().ok()?;
    Some(addr.ip().to_string())
}

impl Default for OscState {
    fn default() -> Self {
        let (tx, rx) = bounded(256);
        Self {
            rx,
            tx,
            bound_port: None,
            local_ip: None,
            raw_rx_count: Arc::new(AtomicU64::new(0)),
            decode_errors: Arc::new(AtomicU64::new(0)),
            socket: None,
            rx_drops: Arc::new(AtomicU64::new(0)),
            shutdown: None,
            handle: None,
        }
    }
}

// ─── Systems ──────────────────────────────────────────────────────────────────

/// Open / close the UDP socket based on IoConfig.
pub fn osc_manage_socket(
    mut state: ResMut<OscState>,
    cfg: Res<OscConfig>,
    mut stats: ResMut<OscStats>,
    supervisor: Res<IoSupervisor>,
) {
    let want_open = cfg.enabled;

    // ── Close if port changed while running ───────────────────────────────────
    let port_changed = want_open
        && state.bound_port.is_some()
        && state.bound_port != Some(cfg.port);
    if port_changed {
        if let Some(shutdown) = state.shutdown.take() {
            let _ = shutdown.try_send(());
        }
        state.socket = None;
        state.bound_port = None;
        state.handle = None;
        info!("OSC port changed — reopening on {}", cfg.port);
    }

    // ── Open socket ───────────────────────────────────────────────────────────
    if want_open && state.bound_port.is_none() {
        let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), cfg.port);
        match create_tuned_udp_socket(addr) {
            Ok(sock) => {
                let (shutdown_tx, shutdown_rx) = bounded::<()>(1);
                let source = OscRxSource::new(
                    sock.try_clone().expect("clone"),
                    Arc::clone(&state.rx_drops),
                    Arc::clone(&state.raw_rx_count),
                    Arc::clone(&state.decode_errors),
                );
                match source.start(state.tx.clone(), shutdown_rx) {
                    Ok(handle) => {
                        state.local_ip = probe_local_ip();
                        let display_ip = state.local_ip.as_deref().unwrap_or("0.0.0.0");
                        info!("OSC listening on 0.0.0.0:{} (send to {}:{})", cfg.port, display_ip, cfg.port);
                        stats.status = ProtocolStatus::Live;
                        state.bound_port = Some(cfg.port);
                        state.socket = Some(sock);
                        state.shutdown = Some(shutdown_tx);
                        state.handle = Some(handle);
                    }
                    Err(e) => {
                        warn!("OSC RX thread failed to start: {e}");
                        stats.status = ProtocolStatus::Error;
                    }
                }
            }
            Err(e) => {
                warn!("OSC bind failed on port {}: {e}", cfg.port);
                stats.status = ProtocolStatus::Error;
            }
        }
    }

    // ── Close socket ──────────────────────────────────────────────────────────
    if !want_open && state.bound_port.is_some() {
        if let Some(shutdown) = state.shutdown.take() {
            let _ = shutdown.try_send(());
        }
        state.socket = None;
        state.bound_port = None;
        state.local_ip = None;
        state.handle = None;
        stats.status = ProtocolStatus::Idle;
    }

    // ── Sync drops into supervisor ────────────────────────────────────────────
    let local_drops = state.rx_drops.load(Ordering::Relaxed);
    let global = supervisor.rx_drops.load(Ordering::Relaxed);
    if local_drops > global {
        supervisor.rx_drops.store(local_drops, Ordering::Relaxed);
    }
}

/// Drain received OSC messages and write into the programmer's per-fixture store.
///
/// Routing through the programmer (priority 200) means:
///   • The normal `programmer_to_dmx` path handles DMX projection
///   • The 3D render sees the changes immediately
///   • Values can be recorded into cues with RECORD
///
/// Address schema:
///   /fixture/{id}/dimmer   f32        — 0.0–1.0
///   /fixture/{id}/pan      f32        — 0.0–1.0
///   /fixture/{id}/tilt     f32        — 0.0–1.0
///   /fixture/{id}/zoom     f32        — 0.0–1.0
///   /fixture/{id}/strobe   f32        — 0.0–1.0
///   /fixture/{id}/red      f32        — 0.0–1.0
///   /fixture/{id}/green    f32        — 0.0–1.0
///   /fixture/{id}/blue     f32        — 0.0–1.0
///   /fixture/{id}/color    f32 f32 f32 — RGB 0.0–1.0
///   /cue/go                            — trigger cue GO
///   /cue/back                          — trigger cue BACK
///   /cue/{n}                           — jump directly to cue n (1-based)
pub fn osc_receive(
    state: Res<OscState>,
    patch: Res<PatchRes>,
    mut programmer: ResMut<Programmer>,
    mut stats: ResMut<OscStats>,
    mut commands: Commands,
) {
    let mut count = 0u64;
    while let Ok(msg) = state.rx.try_recv() {
        let args_summary: String = msg
            .args
            .iter()
            .map(|a| match a {
                OscType::Float(f)  => format!("{:.3}", f),
                OscType::Double(d) => format!("{:.3}", d),
                OscType::Int(i)    => i.to_string(),
                _                  => format!("{:?}", a),
            })
            .collect::<Vec<_>>()
            .join(", ");
        let log_line = format!("{}  [{}]", msg.addr, args_summary);
        warn!("OSC RX: {}", log_line);
        stats.last_messages.push(log_line);
        if stats.last_messages.len() > 8 {
            stats.last_messages.remove(0);
        }

        let parts: Vec<&str> = msg.addr.trim_start_matches('/').split('/').collect();

        // ── Cue triggers ──────────────────────────────────────────────────────
        if parts.first() == Some(&"cue") {
            match parts.get(1).copied() {
                Some("go")   => commands.trigger(GoCueEvent),
                Some("back") => commands.trigger(BackCueEvent),
                Some(n) => {
                    if let Ok(num) = n.parse::<usize>() {
                        commands.trigger(JumpToCueEvent(num));
                    }
                }
                None => {}
            }
            continue;
        }

        // ── Fixture control ───────────────────────────────────────────────────
        if parts.len() < 3 || parts[0] != "fixture" {
            continue;
        }
        // The patch UI displays fixture IDs 1-based (id.0 + 1), so /fixture/006
        // means internal FixtureId(5). Clamp to prevent underflow on /fixture/0.
        let Ok(display_num) = parts[1].parse::<u32>() else { continue };
        if display_num == 0 {
            warn!("OSC: fixture numbers are 1-based, got 0 in {}", msg.addr);
            continue;
        }
        let fixture_id = FixtureId(display_num - 1);
        let attr = parts[2];

        if patch.0.get(fixture_id).is_none() {
            warn!("OSC: no fixture {} (internal id {}) in patch ({})", display_num, fixture_id.0, msg.addr);
            continue;
        }

        // Seed the programmer entry from the fixture's current effective values
        // so we only overwrite the one attribute that arrived, not everything.
        let current = programmer.values_for(fixture_id);
        let pv = programmer.fixture_values.entry(fixture_id).or_insert(current);

        match attr {
            "color" => {
                let floats: Vec<f32> = msg.args.iter().filter_map(osc_float).collect();
                if floats.len() >= 3 {
                    pv.color = [
                        floats[0].clamp(0.0, 1.0),
                        floats[1].clamp(0.0, 1.0),
                        floats[2].clamp(0.0, 1.0),
                    ];
                    count += 1;
                }
            }
            _ => {
                let Some(val) = msg.args.first().and_then(osc_float) else { continue };
                let val = val.clamp(0.0, 1.0);
                match attr {
                    "dimmer" => pv.dimmer = val,
                    "pan"    => pv.pan    = val,
                    "tilt"   => pv.tilt   = val,
                    "zoom"   => pv.zoom   = val,
                    "strobe" => pv.strobe = val,
                    "red"    => pv.color[0] = val,
                    "green"  => pv.color[1] = val,
                    "blue"   => pv.color[2] = val,
                    _ => {
                        warn!("OSC: unknown attribute '{}' in {}", attr, msg.addr);
                        continue;
                    }
                }
                count += 1;
            }
        }
    }
    if count > 0 {
        stats.rx_count = stats.rx_count.saturating_add(count);
        stats.last_rx_at = Some(std::time::Instant::now());
    }
}

fn osc_float(t: &OscType) -> Option<f32> {
    match t {
        OscType::Float(f)  => Some(*f),
        OscType::Double(d) => Some(*d as f32),
        OscType::Int(i)    => Some(*i as f32),
        _                  => None,
    }
}
