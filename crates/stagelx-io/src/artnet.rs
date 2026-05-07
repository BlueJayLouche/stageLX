use std::{
    net::{IpAddr, Ipv4Addr, SocketAddr, UdpSocket},
    sync::Arc,
};

use bevy::prelude::*;
use crossbeam_channel::{bounded, Receiver, Sender};
use stagelx_dmx::engine::DmxEngine;
use stagelx_gdtf::gdtf::DmxMode;
use stagelx_state::{FixtureLibraryRes, IoConfig};

pub const ARTNET_PORT: u16 = 6454;
// Cap the number of universes accepted from any single Art-Net source to
// prevent memory amplification from a malicious or misbehaving sender.
const MAX_RX_UNIVERSES: usize = 64;

// ─── ArtDMX packet helpers ────────────────────────────────────────────────────

fn build_artdmx(universe: u16, data: &[u8; 512]) -> Vec<u8> {
    let mut pkt = Vec::with_capacity(530);
    pkt.extend_from_slice(b"Art-Net\0");
    // OpCode: ArtDMX = 0x5000, little-endian
    pkt.push(0x00);
    pkt.push(0x50);
    // Protocol version 14, big-endian
    pkt.push(0x00);
    pkt.push(14);
    // Sequence (0 = disabled), Physical
    pkt.push(0);
    pkt.push(0);
    // SubUni / Net (Art-Net 3 encoding)
    pkt.push((universe & 0xFF) as u8);
    pkt.push(((universe >> 8) & 0x7F) as u8);
    // Length (512), big-endian
    pkt.push(0x02);
    pkt.push(0x00);
    pkt.extend_from_slice(data);
    pkt
}

/// Returns `(universe, dmx_data)` for a valid ArtDMX packet, otherwise `None`.
fn parse_artdmx(buf: &[u8]) -> Option<(u16, &[u8])> {
    if buf.len() < 18 {
        return None;
    }
    if &buf[..8] != b"Art-Net\0" {
        return None;
    }
    let opcode = u16::from_le_bytes([buf[8], buf[9]]);
    if opcode != 0x5000 {
        return None;
    }
    let universe = (buf[14] as u16) | ((buf[15] as u16 & 0x7F) << 8);
    let length = u16::from_be_bytes([buf[16], buf[17]]) as usize;
    if length == 0 || buf.len() < 18 + length {
        return None;
    }
    Some((universe, &buf[18..18 + length]))
}

// ─── Received packet ──────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct ReceivedPacket {
    pub universe: u16,
    pub source: IpAddr,
    pub data: Arc<Vec<u8>>,
}

// ─── Bevy Resources ───────────────────────────────────────────────────────────

#[derive(Resource)]
pub struct DmxEngineRes(pub DmxEngine);

#[derive(Resource)]
pub struct ArtNetState {
    pub socket: Option<UdpSocket>,
    pub rx_chan: Option<Receiver<ReceivedPacket>>,
    rx_thread_tx: Option<Sender<ReceivedPacket>>,
    /// Parsed cache of IoConfig::artnet_allowed_sources. Rebuilt only when
    /// the source string changes, avoiding a Vec allocation every frame.
    pub cached_allowlist: Vec<IpAddr>,
    cached_allowlist_src: String,
}

impl Default for ArtNetState {
    fn default() -> Self {
        Self {
            socket: None,
            rx_chan: None,
            rx_thread_tx: None,
            cached_allowlist: Vec::new(),
            cached_allowlist_src: String::new(),
        }
    }
}

// ─── Systems ──────────────────────────────────────────────────────────────────

/// Manage the UDP socket lifetime based on IoConfig.
pub fn artnet_manage_socket(mut state: ResMut<ArtNetState>, cfg: Res<IoConfig>) {
    if state.socket.is_none() {
        let bind_ip: IpAddr = cfg
            .artnet_ip
            .trim()
            .parse()
            .unwrap_or(IpAddr::V4(Ipv4Addr::UNSPECIFIED));
        let bind_addr = SocketAddr::new(bind_ip, ARTNET_PORT);
        match UdpSocket::bind(bind_addr) {
            Ok(sock) => {
                sock.set_nonblocking(true).ok();
                sock.set_broadcast(true).ok();
                info!("Art-Net socket bound to {}", bind_addr);
                state.socket = Some(sock);
            }
            Err(e) => warn!("Art-Net bind failed: {e}"),
        }
    }

    if cfg.artnet_rx_enabled && state.rx_chan.is_none() {
        // Clone the bound TX socket so both sides share the same port binding.
        // A second bind() to the same port would fail with EADDRINUSE.
        if let Some(ref sock) = state.socket {
            match sock.try_clone() {
                Ok(rx_sock) => {
                    // TX socket is nonblocking (send_to must not stall the frame).
                    // The clone inherits that mode — reset it so the RX thread
                    // blocks in the kernel until a packet arrives instead of
                    // busy-polling with sleep(1ms).
                    rx_sock.set_nonblocking(false).ok();

                    let (tx, rx) = bounded::<ReceivedPacket>(256);
                    state.rx_chan = Some(rx);
                    state.rx_thread_tx = Some(tx.clone());

                    std::thread::spawn(move || {
                        let mut buf = [0u8; 600];
                        loop {
                            match rx_sock.recv_from(&mut buf) {
                                Ok((n, src)) => {
                                    if let Some((universe, data)) = parse_artdmx(&buf[..n]) {
                                        let pkt = ReceivedPacket {
                                            universe,
                                            source: src.ip(),
                                            data: Arc::new(data.to_vec()),
                                        };
                                        if tx.send(pkt).is_err() {
                                            break;
                                        }
                                    }
                                }
                                Err(e) => {
                                    warn!("Art-Net RX error: {e}");
                                    std::thread::sleep(std::time::Duration::from_millis(10));
                                }
                            }
                        }
                    });
                }
                Err(e) => warn!("Art-Net RX socket clone failed: {e}"),
            }
        }
    }

    if !cfg.artnet_rx_enabled {
        state.rx_chan = None;
        state.rx_thread_tx = None;
    }

    // Rebuild the allowlist cache only when the config string actually changed.
    if cfg.artnet_allowed_sources != state.cached_allowlist_src {
        state.cached_allowlist = cfg
            .artnet_allowed_sources
            .split(',')
            .filter_map(|s| s.trim().parse().ok())
            .collect();
        state.cached_allowlist_src.clone_from(&cfg.artnet_allowed_sources);
    }
}

/// Drain incoming Art-Net packets into the DMX engine "artnet_in" source.
///
/// Security mitigations applied here:
/// - Source IP allowlist: packets from unlisted hosts are silently dropped.
/// - Universe cap: at most MAX_RX_UNIVERSES unique universes are created per source
///   to prevent the memory-amplification attack from a malicious sender.
pub fn artnet_receive(
    state: Res<ArtNetState>,
    mut engine: ResMut<DmxEngineRes>,
    mut cfg: ResMut<IoConfig>,
) {
    let Some(rx) = &state.rx_chan else { return };

    // Use the pre-parsed allowlist from ArtNetState (rebuilt only on config change).
    let allowlist = &state.cached_allowlist;

    let source = engine
        .0
        .get_or_add_source("artnet_in", 100, stagelx_dmx::merge::MergeStrategy::Htp);

    let current_universe_count = source.universes.universes().count();
    let mut new_universe_count = current_universe_count;
    let mut count = 0u64;

    while let Ok(pkt) = rx.try_recv() {
        if !allowlist.is_empty() && !allowlist.contains(&pkt.source) {
            continue;
        }

        // #2 — Universe cap
        let already_exists = source.universes.get(pkt.universe).is_some();
        if !already_exists && new_universe_count >= MAX_RX_UNIVERSES {
            continue;
        }
        if !already_exists {
            new_universe_count += 1;
        }

        let buf = source.universes.get_or_insert(pkt.universe);
        buf.copy_from_slice(&pkt.data);
        count += 1;
    }
    cfg.artnet_rx_count = cfg.artnet_rx_count.saturating_add(count);
}

/// Write normalised programmer state into the DMX engine's "programmer" source.
/// Runs in FixedUpdate so it fires at the same rate as the protocol sends.
pub fn programmer_to_dmx(
    mut engine: ResMut<DmxEngineRes>,
    programmer: Res<stagelx_state::Programmer>,
    patch: Res<stagelx_state::PatchRes>,
    library: Res<FixtureLibraryRes>,
) {
    let source = engine
        .0
        .get_or_add_source("programmer", 200, stagelx_dmx::merge::MergeStrategy::Ltp);

    let dimmer_byte = (programmer.dimmer * 255.0) as u8;
    let pan_raw     = (programmer.pan    * 65535.0) as u16;
    let tilt_raw    = (programmer.tilt   * 65535.0) as u16;
    let r = (programmer.color[0] * 255.0) as u8;
    let g = (programmer.color[1] * 255.0) as u8;
    let b = (programmer.color[2] * 255.0) as u8;

    for inst in patch.0.fixtures() {
        let base     = inst.address.channel;
        let universe = inst.address.universe;
        let buf      = source.universes.get_or_insert(universe);

        // Use GDTF channel offsets when the fixture type is loaded; fall back to
        // the generic 8-channel layout for unloaded or procedural fixtures.
        let mode = library
            .library
            .get(&inst.fixture_type_id)
            .and_then(|ft| ft.find_mode(&inst.dmx_mode));

        if let Some(mode) = mode {
            write_attr8(buf, base, mode, "Dimmer",     dimmer_byte);
            write_attr16(buf, base, mode, "Pan",       pan_raw);
            write_attr16(buf, base, mode, "Tilt",      tilt_raw);
            write_attr8(buf, base, mode, "ColorAdd_R", r);
            write_attr8(buf, base, mode, "ColorAdd_G", g);
            write_attr8(buf, base, mode, "ColorAdd_B", b);
        } else {
            // Generic 8-ch: Dimmer | Pan MSB | Pan Fine | Tilt MSB | Tilt Fine | R | G | B
            buf.set(base,     dimmer_byte);
            buf.set(base + 1, (pan_raw >> 8) as u8);
            buf.set(base + 2, (pan_raw & 0xFF) as u8);
            buf.set(base + 3, (tilt_raw >> 8) as u8);
            buf.set(base + 4, (tilt_raw & 0xFF) as u8);
            buf.set(base + 5, r);
            buf.set(base + 6, g);
            buf.set(base + 7, b);
        }
    }
}

/// Write a single 8-bit attribute to the DMX buffer using the GDTF channel offset.
fn write_attr8(
    buf: &mut stagelx_core::universe::DmxBuffer,
    base: u16,
    mode: &DmxMode,
    attr: &str,
    value: u8,
) {
    if let Some(ch) = mode.channel_for(attr) {
        buf.set(base + ch.offset - 1, value);
    }
}

/// Write a 16-bit attribute (MSB + optional fine byte) using the GDTF channel offset.
fn write_attr16(
    buf: &mut stagelx_core::universe::DmxBuffer,
    base: u16,
    mode: &DmxMode,
    attr: &str,
    value: u16,
) {
    if let Some(ch) = mode.channel_for(attr) {
        let idx = base + ch.offset - 1;
        buf.set(idx, (value >> 8) as u8);
        if ch.resolution >= 2 {
            buf.set(idx + 1, (value & 0xFF) as u8);
        }
    }
}

/// Merge all DMX sources into the output universe set.
/// Must run after programmer_to_dmx and artnet_receive, before the send systems.
pub fn dmx_engine_tick(mut engine: ResMut<DmxEngineRes>) {
    engine.0.tick();
}

/// Send Art-Net output for the configured universe.
/// Runs in FixedUpdate — no Instant throttle needed.
pub fn artnet_send(
    state: Res<ArtNetState>,
    engine: Res<DmxEngineRes>,
    mut cfg: ResMut<IoConfig>,
) {

    let Some(sock) = &state.socket else { return };

    // #4 — Configurable TX destination (empty = limited broadcast)
    let dest_ip: IpAddr = cfg
        .artnet_dest_ip
        .trim()
        .parse()
        .unwrap_or(IpAddr::V4(Ipv4Addr::BROADCAST));
    let dest = SocketAddr::new(dest_ip, ARTNET_PORT);

    let out_universe = cfg.artnet_out_universe;
    if let Some(dmx_buf) = engine.0.output_buffer(out_universe) {
        let pkt = build_artdmx(out_universe, dmx_buf.as_bytes());
        match sock.send_to(&pkt, dest) {
            Ok(_) => {
                cfg.artnet_tx_count = cfg.artnet_tx_count.saturating_add(1);
                cfg.artnet_status = format!("TX u{} → {}", out_universe, dest_ip);
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {}
            Err(e) => {
                cfg.artnet_status = format!("TX error: {e}");
            }
        }
    }
}
