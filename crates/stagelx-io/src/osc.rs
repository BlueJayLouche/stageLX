//! OSC input via `rosc` over UDP.
//!
//! Message schema handled here:
//!   /fixture/{id}/{attr}   f32   — set attribute 0.0–1.0 on all patched fixtures
//!   /fixture/{id}/color    fff   — set RGB 0.0–1.0
//!
//! The socket runs non-blocking in Bevy's Update loop (same pattern as Art-Net TX).
//! A background thread is used for blocking recv so the main thread is never stalled.

use bevy::prelude::*;
use crossbeam_channel::{bounded, Receiver, Sender};
use rosc::{OscPacket, OscType};
use std::net::{IpAddr, Ipv4Addr, SocketAddr, UdpSocket};
use stagelx_state::{IoConfig, Programmer};

// ─── Incoming message ──────────────────────────────────────────────────────────

struct OscMsg {
    addr: String,
    args: Vec<OscType>,
}

// ─── Resource ─────────────────────────────────────────────────────────────────

#[derive(Resource)]
pub struct OscState {
    rx: Receiver<OscMsg>,
    tx: Sender<OscMsg>,
    pub bound_port: Option<u16>,
}

impl Default for OscState {
    fn default() -> Self {
        let (tx, rx) = bounded(256);
        Self { rx, tx, bound_port: None }
    }
}

// ─── Systems ──────────────────────────────────────────────────────────────────

/// Open / close the UDP socket based on IoConfig.
pub fn osc_manage_socket(mut state: ResMut<OscState>, mut cfg: ResMut<IoConfig>) {
    let want_open = cfg.osc_enabled;

    if want_open && state.bound_port.is_none() {
        let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), cfg.osc_port);
        match UdpSocket::bind(addr) {
            Ok(sock) => {
                // Keep blocking in the thread; Bevy Update just drains the channel.
                let tx = state.tx.clone();
                std::thread::spawn(move || {
                    let mut buf = vec![0u8; 1536];
                    loop {
                        match sock.recv(&mut buf) {
                            Ok(n) => {
                                if let Ok(pkt) = rosc::decoder::decode(&buf[..n]) {
                                    forward_packet(pkt, &tx);
                                }
                            }
                            Err(_) => break,
                        }
                    }
                });
                info!("OSC listening on {}", addr);
                cfg.osc_status = format!("Listening :{}", cfg.osc_port);
                state.bound_port = Some(cfg.osc_port);
            }
            Err(e) => {
                cfg.osc_status = format!("Bind failed: {e}");
            }
        }
    }

    if !want_open && state.bound_port.is_some() {
        // We can't easily close the background thread's socket without Arc<UdpSocket>,
        // so just mark as closed and let GC handle it when the resource is replaced.
        state.bound_port = None;
        cfg.osc_status = "Closed".into();
    }
}

fn forward_packet(pkt: OscPacket, tx: &Sender<OscMsg>) {
    match pkt {
        OscPacket::Message(m) => {
            let _ = tx.send(OscMsg { addr: m.addr, args: m.args });
        }
        OscPacket::Bundle(b) => {
            for p in b.content {
                forward_packet(p, tx);
            }
        }
    }
}

/// Drain received OSC messages and apply them to the Programmer.
pub fn osc_receive(
    state: Res<OscState>,
    mut programmer: ResMut<Programmer>,
    mut cfg: ResMut<IoConfig>,
) {
    let mut count = 0u64;
    while let Ok(msg) = state.rx.try_recv() {
        // Parse /fixture/{id}/{attr}
        let parts: Vec<&str> = msg.addr.trim_start_matches('/').split('/').collect();
        if parts.len() >= 3 && parts[0] == "fixture" {
            let attr = parts[2];
            match attr {
                "color" => {
                    // Expects three float args
                    let floats: Vec<f32> = msg.args.iter().filter_map(osc_float).collect();
                    if floats.len() >= 3 {
                        programmer.color = [
                            floats[0].clamp(0.0, 1.0),
                            floats[1].clamp(0.0, 1.0),
                            floats[2].clamp(0.0, 1.0),
                        ];
                    }
                }
                _ => {
                    if let Some(val) = msg.args.first().and_then(osc_float) {
                        let val = val.clamp(0.0, 1.0);
                        match attr {
                            "dimmer" => programmer.dimmer      = val,
                            "pan"    => programmer.pan         = val,
                            "tilt"   => programmer.tilt        = val,
                            "zoom"   => programmer.zoom        = val,
                            "strobe" => programmer.strobe      = val,
                            "red"    => programmer.color[0]    = val,
                            "green"  => programmer.color[1]    = val,
                            "blue"   => programmer.color[2]    = val,
                            _        => {}
                        }
                    }
                }
            }
            count += 1;
        }
    }
    cfg.osc_rx_count = cfg.osc_rx_count.saturating_add(count);
}

fn osc_float(t: &OscType) -> Option<f32> {
    match t {
        OscType::Float(f)  => Some(*f),
        OscType::Double(d) => Some(*d as f32),
        OscType::Int(i)    => Some(*i as f32),
        _                  => None,
    }
}
