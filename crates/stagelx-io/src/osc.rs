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
use stagelx_show::{BackCueEvent, GoCueEvent, ProtocolStatus};
use stagelx_core::types::FixtureId;
use stagelx_dmx::engine::DmxEngineRes;
use stagelx_dmx::merge::MergeStrategy;
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
}

impl OscRxSource {
    pub fn new(socket: UdpSocket, drops: Arc<AtomicU64>) -> Self {
        Self { socket, drops }
    }
}

impl IoSource for OscRxSource {
    type Msg = OscMsg;

    fn start(&self, tx: Sender<Self::Msg>, shutdown: Receiver<()>) -> std::io::Result<JoinHandle<()>> {
        let socket = self.socket.try_clone()?;
        socket.set_nonblocking(false)?;
        socket.set_read_timeout(Some(Duration::from_millis(100)))?;
        let drops = Arc::clone(&self.drops);

        Ok(std::thread::spawn(move || {
            let mut buf = vec![0u8; 1536];
            loop {
                match socket.recv(&mut buf) {
                    Ok(n) => {
                        if let Ok(pkt) = rosc::decoder::decode(&buf[..n]) {
                            forward_packet(pkt, &tx, &drops);
                        }
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::TimedOut => {
                        if shutdown.try_recv().is_ok() {
                            break;
                        }
                    }
                    Err(_) => break,
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
    /// Clone of the socket held so we can shut it down when disabled.
    socket: Option<UdpSocket>,
    /// Shared drop counter.
    pub rx_drops: Arc<AtomicU64>,
    /// Shutdown sender for the background thread.
    shutdown: Option<Sender<()>>,
    /// Background thread handle.
    handle: Option<JoinHandle<()>>,
}

impl Default for OscState {
    fn default() -> Self {
        let (tx, rx) = bounded(256);
        Self {
            rx,
            tx,
            bound_port: None,
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
    cfg: ResMut<OscConfig>,
    mut stats: ResMut<OscStats>,
    supervisor: Res<IoSupervisor>,
) {
    let want_open = cfg.enabled;

    // ── Open socket ───────────────────────────────────────────────────────────
    if want_open && state.bound_port.is_none() {
        let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), cfg.port);
        match create_tuned_udp_socket(addr) {
            Ok(sock) => {
                let (shutdown_tx, shutdown_rx) = bounded::<()>(1);
                let source = OscRxSource::new(sock.try_clone().expect("clone"), Arc::clone(&state.rx_drops));
                match source.start(state.tx.clone(), shutdown_rx) {
                    Ok(handle) => {
                        info!("OSC listening on {}", addr);
                        stats.status = ProtocolStatus::Live;
                        state.bound_port = Some(cfg.port);
                        state.socket = Some(sock);
                        state.shutdown = Some(shutdown_tx);
                        state.handle = Some(handle);
                    }
                    Err(e) => {
                        warn!("OSC RX source start failed: {e}");
                        stats.status = ProtocolStatus::Error;
                    }
                }
            }
            Err(_e) => {
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

/// Drain received OSC messages and route per-fixture through the DMX engine
/// or emit cue trigger events.
///
/// Address schema:
///   /fixture/{id}/{attr}   f32   — set attribute 0.0–1.0
///   /fixture/{id}/color    fff   — set RGB 0.0–1.0
///   /cue/go                     — trigger cue GO
///   /cue/back                   — trigger cue BACK
pub fn osc_receive(
    state: Res<OscState>,
    patch: Res<PatchRes>,
    mut engine: ResMut<DmxEngineRes>,
    mut stats: ResMut<OscStats>,
    mut commands: Commands,
) {
    let source = engine
        .0
        .get_or_add_source("osc_in", 150, MergeStrategy::Ltp);

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
        info!("OSC RX: {}", log_line);
        stats.last_messages.push(log_line);
        if stats.last_messages.len() > 8 {
            stats.last_messages.remove(0);
        }

        let parts: Vec<&str> = msg.addr.trim_start_matches('/').split('/').collect();

        // ── Cue triggers ──────────────────────────────────────────────────────
        if parts.first() == Some(&"cue") {
            match parts.get(1).copied() {
                Some("go") => {
                    commands.trigger(GoCueEvent);
                }
                Some("back") => {
                    commands.trigger(BackCueEvent);
                }
                _ => {}
            }
            continue;
        }

        // ── Fixture control ───────────────────────────────────────────────────
        if parts.len() < 3 || parts[0] != "fixture" {
            continue;
        }
        let Ok(fixture_id) = parts[1].parse::<u32>() else { continue };
        let fixture_id = FixtureId(fixture_id);
        let attr = parts[2];

        let Some(inst) = patch.0.get(fixture_id) else { continue };
        let base = inst.address.channel;
        let universe = inst.address.universe;
        let buf = source.universes.get_or_insert(universe);

        match attr {
            "color" => {
                let floats: Vec<f32> = msg.args.iter().filter_map(osc_float).collect();
                if floats.len() >= 3 {
                    let r = (floats[0].clamp(0.0, 1.0) * 255.0) as u8;
                    let g = (floats[1].clamp(0.0, 1.0) * 255.0) as u8;
                    let b = (floats[2].clamp(0.0, 1.0) * 255.0) as u8;
                    if let Some(off) = inst.channel_map.red   { buf.set(base + off, r); }
                    if let Some(off) = inst.channel_map.green { buf.set(base + off, g); }
                    if let Some(off) = inst.channel_map.blue  { buf.set(base + off, b); }
                    count += 1;
                }
            }
            _ => {
                let Some(val) = msg.args.first().and_then(osc_float) else { continue };
                let val = val.clamp(0.0, 1.0);
                let byte = (val * 255.0) as u8;
                let u16_raw = (val * 65535.0) as u16;
                match attr {
                    "dimmer" => {
                        if let Some(off) = inst.channel_map.dimmer { buf.set(base + off, byte); }
                    }
                    "pan" => {
                        if let Some(off) = inst.channel_map.pan { buf.set(base + off, (u16_raw >> 8) as u8); }
                        if let Some(off) = inst.channel_map.pan_fine { buf.set(base + off, (u16_raw & 0xFF) as u8); }
                    }
                    "tilt" => {
                        if let Some(off) = inst.channel_map.tilt { buf.set(base + off, (u16_raw >> 8) as u8); }
                        if let Some(off) = inst.channel_map.tilt_fine { buf.set(base + off, (u16_raw & 0xFF) as u8); }
                    }
                    "red" => {
                        if let Some(off) = inst.channel_map.red { buf.set(base + off, byte); }
                    }
                    "green" => {
                        if let Some(off) = inst.channel_map.green { buf.set(base + off, byte); }
                    }
                    "blue" => {
                        if let Some(off) = inst.channel_map.blue { buf.set(base + off, byte); }
                    }
                    _ => { continue; }
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
