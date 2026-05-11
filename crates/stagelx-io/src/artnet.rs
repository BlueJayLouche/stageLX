use std::{
    collections::HashMap,
    net::{IpAddr, Ipv4Addr, SocketAddr, UdpSocket},
    sync::atomic::{AtomicU64, Ordering},
    sync::Arc,
    thread::JoinHandle,
    time::Duration,
};

use bevy::prelude::*;
use crossbeam_channel::{bounded, Receiver, Sender, TrySendError};
use stagelx_dmx::engine::DmxEngineRes;
use stagelx_show::ProtocolStatus;
use crate::config::ArtNetConfig;
use crate::stats::ArtNetStats;
use crate::supervisor::{IoSink, IoSource, IoSupervisor, create_tuned_udp_socket};

pub const ARTNET_PORT: u16 = 6454;
// Cap the number of universes accepted from any single Art-Net source to
// prevent memory amplification from a malicious or misbehaving sender.
const MAX_RX_UNIVERSES: usize = 64;
const ART_POLL_INTERVAL_SECS: f32 = 3.0;

// ─── Packet helpers ───────────────────────────────────────────────────────────

pub fn build_artdmx(universe: u16, data: &[u8; 512]) -> Vec<u8> {
    let mut pkt = Vec::with_capacity(530);
    pkt.extend_from_slice(b"Art-Net\0");
    pkt.push(0x00); pkt.push(0x50); // OpCode ArtDMX = 0x5000 LE
    pkt.push(0x00); pkt.push(14);   // ProtVer 14 BE
    pkt.push(0); pkt.push(0);       // Sequence, Physical
    pkt.push((universe & 0xFF) as u8);
    pkt.push(((universe >> 8) & 0x7F) as u8);
    pkt.push(0x02); pkt.push(0x00); // Length 512 BE
    pkt.extend_from_slice(data);
    pkt
}

fn parse_artdmx(buf: &[u8]) -> Option<(u16, &[u8])> {
    if buf.len() < 18 || &buf[..8] != b"Art-Net\0" { return None; }
    let opcode = u16::from_le_bytes([buf[8], buf[9]]);
    if opcode != 0x5000 { return None; }
    let universe = (buf[14] as u16) | ((buf[15] as u16 & 0x7F) << 8);
    let length = u16::from_be_bytes([buf[16], buf[17]]) as usize;
    if length == 0 || buf.len() < 18 + length { return None; }
    Some((universe, &buf[18..18 + length]))
}

fn build_artpoll() -> Vec<u8> {
    let mut pkt = Vec::with_capacity(14);
    pkt.extend_from_slice(b"Art-Net\0");
    pkt.push(0x00); pkt.push(0x20); // OpCode ArtPoll = 0x2000 LE
    pkt.push(0x00); pkt.push(14);   // ProtVer 14 BE
    pkt.push(0x06);                 // TalkToMe: send diag + reply on change
    pkt.push(0x00);                 // Priority
    pkt
}

fn parse_artpoll_reply(buf: &[u8]) -> Option<ArtNetNode> {
    if buf.len() < 239 || &buf[..8] != b"Art-Net\0" { return None; }
    let opcode = u16::from_le_bytes([buf[8], buf[9]]);
    if opcode != 0x2100 { return None; }

    let ip = Ipv4Addr::new(buf[10], buf[11], buf[12], buf[13]);
    let status1 = buf[23];

    let short_name = cstr_trim(&buf[26..44]);
    let long_name = cstr_trim(&buf[44..108]);

    let num_ports = buf[173] as usize;
    let mut port_types = [0u8; 4];
    port_types.copy_from_slice(&buf[174..178]);

    Some(ArtNetNode {
        ip: IpAddr::V4(ip),
        short_name,
        long_name,
        status: status1,
        num_ports,
        port_types,
    })
}

fn cstr_trim(bytes: &[u8]) -> String {
    let len = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
    String::from_utf8_lossy(&bytes[..len]).into_owned()
}

// ─── Data types ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct ReceivedPacket {
    pub universe: u16,
    pub source: IpAddr,
    pub data: [u8; 512],
}

#[derive(Debug, Clone)]
pub struct ArtNetNode {
    pub ip: IpAddr,
    pub short_name: String,
    pub long_name: String,
    pub status: u8,
    pub num_ports: usize,
    pub port_types: [u8; 4],
}

// Dmx variant is large ([u8; 512]) but it is the hot path (44–60 Hz).
// Boxing it would add a heap allocation per packet — not worth it.
#[derive(Debug, Clone)]
#[allow(clippy::large_enum_variant)]
pub enum ArtNetRxMsg {
    Dmx(ReceivedPacket),
    Node(ArtNetNode),
}

#[derive(Resource, Default)]
pub struct ArtNetNodeTable {
    pub nodes: HashMap<IpAddr, ArtNetNode>,
    /// Incremented whenever the table changes (for UI change detection).
    pub version: u64,
}

#[derive(Debug, Clone)]
pub struct ArtNetTxCmd {
    pub universe: u16,
    pub data: [u8; 512],
}

// ─── IoSource / IoSink implementations ────────────────────────────────────────

pub struct ArtNetRxSource {
    socket: UdpSocket,
    drops: Arc<AtomicU64>,
}

impl ArtNetRxSource {
    pub fn new(socket: UdpSocket, drops: Arc<AtomicU64>) -> Self {
        Self { socket, drops }
    }
}

impl IoSource for ArtNetRxSource {
    type Msg = ArtNetRxMsg;

    fn start(&self, tx: Sender<Self::Msg>, shutdown: Receiver<()>) -> std::io::Result<JoinHandle<()>> {
        let socket = self.socket.try_clone()?;
        socket.set_nonblocking(false)?;
        socket.set_read_timeout(Some(Duration::from_millis(100)))?;
        let drops = Arc::clone(&self.drops);

        Ok(std::thread::spawn(move || {
            let mut buf = [0u8; 600];
            loop {
                match socket.recv_from(&mut buf) {
                    Ok((n, src)) => {
                        // Try ArtDMX first (most common)
                        if let Some((universe, data)) = parse_artdmx(&buf[..n]) {
                            let mut pkt_data = [0u8; 512];
                            let len = data.len().min(512);
                            pkt_data[..len].copy_from_slice(&data[..len]);
                            let pkt = ReceivedPacket {
                                universe,
                                source: src.ip(),
                                data: pkt_data,
                            };
                            if let Err(TrySendError::Full(_)) = tx.try_send(ArtNetRxMsg::Dmx(pkt)) {
                                drops.fetch_add(1, Ordering::Relaxed);
                            }
                            continue;
                        }
                        // Try ArtPollReply
                        if let Some(node) = parse_artpoll_reply(&buf[..n]) {
                            let _ = tx.try_send(ArtNetRxMsg::Node(node));
                            continue;
                        }
                    }
                    Err(e) if matches!(e.kind(), std::io::ErrorKind::TimedOut | std::io::ErrorKind::WouldBlock | std::io::ErrorKind::Interrupted) => {
                        if shutdown.try_recv().is_ok() {
                            break;
                        }
                    }
                    Err(e) => {
                        warn!("Art-Net RX error: {e}");
                        std::thread::sleep(Duration::from_millis(10));
                    }
                }
            }
        }))
    }
}

pub struct ArtNetTxSink {
    socket: UdpSocket,
    dest: SocketAddr,
}

impl ArtNetTxSink {
    pub fn new(socket: UdpSocket, dest: SocketAddr) -> Self {
        Self { socket, dest }
    }
}

impl IoSink for ArtNetTxSink {
    type Cmd = ArtNetTxCmd;

    fn start(&self, rx: Receiver<Self::Cmd>, shutdown: Receiver<()>) -> std::io::Result<JoinHandle<()>> {
        let socket = self.socket.try_clone()?;
        let dest = self.dest;

        Ok(std::thread::spawn(move || {
            loop {
                crossbeam_channel::select! {
                    recv(rx) -> cmd => {
                        match cmd {
                            Ok(cmd) => {
                                let pkt = build_artdmx(cmd.universe, &cmd.data);
                                let _ = socket.send_to(&pkt, dest);
                            }
                            Err(_) => break,
                        }
                    }
                    recv(shutdown) -> _ => break,
                }
            }
        }))
    }
}

// ─── Bevy Resources ───────────────────────────────────────────────────────────

#[derive(Resource)]
pub struct ArtNetState {
    pub socket: Option<UdpSocket>,
    pub rx_chan: Option<Receiver<ArtNetRxMsg>>,
    pub tx_chan: Option<Sender<ArtNetTxCmd>>,
    pub rx_drops: Arc<AtomicU64>,
    pub cached_allowlist: Vec<IpAddr>,
    cached_allowlist_src: String,
    rx_shutdown: Option<Sender<()>>,
    tx_shutdown: Option<Sender<()>>,
    rx_handle: Option<JoinHandle<()>>,
    tx_handle: Option<JoinHandle<()>>,
    last_bind_attempt: Option<std::time::Instant>,
    last_poll_sent: Option<std::time::Instant>,
}

impl Default for ArtNetState {
    fn default() -> Self {
        Self {
            socket: None,
            rx_chan: None,
            tx_chan: None,
            rx_drops: Arc::new(AtomicU64::new(0)),
            cached_allowlist: Vec::new(),
            cached_allowlist_src: String::new(),
            rx_shutdown: None,
            tx_shutdown: None,
            rx_handle: None,
            tx_handle: None,
            last_bind_attempt: None,
            last_poll_sent: None,
        }
    }
}

// ─── Systems ──────────────────────────────────────────────────────────────────

pub fn artnet_manage_socket(
    mut state: ResMut<ArtNetState>,
    cfg: Res<ArtNetConfig>,
    supervisor: Res<IoSupervisor>,
    mut stats: ResMut<ArtNetStats>,
) {
    let wants_io = cfg.tx_enabled || cfg.rx_enabled || cfg.discovery_enabled;

    // ── Release socket when nothing is needed ─────────────────────────────────
    if !wants_io && state.socket.is_some() {
        if let Some(shutdown) = state.rx_shutdown.take() {
            let _ = shutdown.try_send(());
        }
        if let Some(shutdown) = state.tx_shutdown.take() {
            let _ = shutdown.try_send(());
        }
        state.socket = None;
        state.rx_chan = None;
        state.tx_chan = None;
        state.rx_handle = None;
        state.tx_handle = None;
        stats.status = ProtocolStatus::Idle;
        info!("Art-Net socket released (all IO disabled)");
        return;
    }

    // ── Create socket if missing ──────────────────────────────────────────────
    if wants_io && state.socket.is_none() {
        let now = std::time::Instant::now();
        let should_try = state.last_bind_attempt.is_none_or(|t| {
            now.duration_since(t).as_secs_f32() >= 1.0
        });
        if should_try {
            state.last_bind_attempt = Some(now);
            let bind_ip: IpAddr = cfg
                .ip
                .trim()
                .parse()
                .unwrap_or(IpAddr::V4(Ipv4Addr::UNSPECIFIED));
            let bind_addr = SocketAddr::new(bind_ip, ARTNET_PORT);
            match create_tuned_udp_socket(bind_addr) {
                Ok(sock) => {
                    info!("Art-Net socket bound to {}", bind_addr);
                    state.socket = Some(sock);
                }
                Err(e) => warn!("Art-Net bind failed: {e}"),
            }
        }
    }

    // ── Start RX thread when enabled ──────────────────────────────────────────
    if cfg.rx_enabled && state.rx_chan.is_none() {
        if let Some(ref sock) = state.socket {
            let (tx, rx) = bounded::<ArtNetRxMsg>(8);
            let (shutdown_tx, shutdown_rx) = bounded::<()>(1);
            let source = ArtNetRxSource::new(sock.try_clone().expect("clone"), Arc::clone(&state.rx_drops));
            match source.start(tx.clone(), shutdown_rx) {
                Ok(handle) => {
                    state.rx_chan = Some(rx);
                    state.rx_shutdown = Some(shutdown_tx);
                    state.rx_handle = Some(handle);
                }
                Err(e) => warn!("Art-Net RX source start failed: {e}"),
            }
        }
    }

    // ── Stop RX thread when disabled ──────────────────────────────────────────
    if !cfg.rx_enabled && state.rx_chan.is_some() {
        if let Some(shutdown) = state.rx_shutdown.take() {
            let _ = shutdown.try_send(());
        }
        state.rx_chan = None;
        state.rx_handle = None;
        if state.tx_chan.is_none() {
            stats.status = ProtocolStatus::Idle;
        }
    }

    // ── Start TX thread when enabled ──────────────────────────────────────────
    if cfg.tx_enabled && state.socket.is_some() && state.tx_chan.is_none() {
        let dest_ip: IpAddr = cfg
            .dest_ip
            .trim()
            .parse()
            .unwrap_or(IpAddr::V4(Ipv4Addr::BROADCAST));
        let dest = SocketAddr::new(dest_ip, ARTNET_PORT);
        let (tx, rx) = bounded::<ArtNetTxCmd>(1);
        let (shutdown_tx, shutdown_rx) = bounded::<()>(1);
        let sink = ArtNetTxSink::new(state.socket.as_ref().unwrap().try_clone().expect("clone"), dest);
        match sink.start(rx, shutdown_rx) {
            Ok(handle) => {
                state.tx_chan = Some(tx);
                state.tx_shutdown = Some(shutdown_tx);
                state.tx_handle = Some(handle);
            }
            Err(e) => warn!("Art-Net TX sink start failed: {e}"),
        }
    }

    // ── Stop TX thread when disabled ───────────────────────────────────────────
    if !cfg.tx_enabled && state.tx_chan.is_some() {
        if let Some(shutdown) = state.tx_shutdown.take() {
            let _ = shutdown.try_send(());
        }
        state.tx_chan = None;
        state.tx_handle = None;
        if state.rx_chan.is_none() {
            stats.status = ProtocolStatus::Idle;
        }
    }

    // ── Send ArtPoll every 3 s when discovery enabled ─────────────────────────
    if cfg.discovery_enabled {
        let now = std::time::Instant::now();
        let should_poll = state.last_poll_sent.is_none_or(|t| {
            now.duration_since(t).as_secs_f32() >= ART_POLL_INTERVAL_SECS
        });
        if should_poll {
            state.last_poll_sent = Some(now);
            if let Some(ref sock) = state.socket {
                let poll_pkt = build_artpoll();
                let dest = SocketAddr::new(IpAddr::V4(Ipv4Addr::BROADCAST), ARTNET_PORT);
                let _ = sock.send_to(&poll_pkt, dest);
            }
        }
    }

    // ── Rebuild allowlist cache ───────────────────────────────────────────────
    if cfg.allowed_sources != state.cached_allowlist_src {
        state.cached_allowlist = cfg
            .allowed_sources
            .split(',')
            .filter_map(|s| s.trim().parse().ok())
            .collect();
        state.cached_allowlist_src.clone_from(&cfg.allowed_sources);
    }

    // ── Sync thread-local drops into the supervisor ───────────────────────────
    let local_drops = state.rx_drops.load(Ordering::Relaxed);
    let global = supervisor.rx_drops.load(Ordering::Relaxed);
    if local_drops > global {
        supervisor.rx_drops.store(local_drops, Ordering::Relaxed);
    }
}

pub fn artnet_receive(
    state: Res<ArtNetState>,
    mut engine: ResMut<DmxEngineRes>,
    mut stats: ResMut<ArtNetStats>,
    mut node_table: ResMut<ArtNetNodeTable>,
) {
    let Some(rx) = &state.rx_chan else { return };

    let allowlist = &state.cached_allowlist;

    let source = engine
        .0
        .get_or_add_source("artnet_in", 100, stagelx_dmx::merge::MergeStrategy::Htp);

    let current_universe_count = source.universes.universes().count();
    let mut new_universe_count = current_universe_count;
    let mut count = 0u64;

    while let Ok(msg) = rx.try_recv() {
        match msg {
            ArtNetRxMsg::Dmx(pkt) => {
                if !allowlist.is_empty() && !allowlist.contains(&pkt.source) {
                    continue;
                }
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
            ArtNetRxMsg::Node(node) => {
                node_table.nodes.insert(node.ip, node);
                node_table.version += 1;
            }
        }
    }
    if count > 0 {
        stats.rx_count = stats.rx_count.saturating_add(count);
        stats.last_rx_at = Some(std::time::Instant::now());
    }
}

pub fn dmx_engine_tick(
    mut engine: ResMut<DmxEngineRes>,
    mut perf: Option<ResMut<stagelx_show::PerfDiagnosticsRes>>,
) {
    let start = std::time::Instant::now();
    engine.0.tick();
    if let Some(ref mut p) = perf {
        p.record_dmx_tick(start.elapsed().as_secs_f32() * 1000.0);
    }
}

pub fn artnet_send(
    state: Res<ArtNetState>,
    engine: Res<DmxEngineRes>,
    cfg: Res<ArtNetConfig>,
    mut stats: ResMut<ArtNetStats>,
) {
    let Some(tx) = &state.tx_chan else { return };

    let out_universe = cfg.out_universe;
    if let Some(dmx_buf) = engine.0.output_buffer(out_universe) {
        let cmd = ArtNetTxCmd {
            universe: out_universe,
            data: *dmx_buf.as_bytes(),
        };
        match tx.try_send(cmd) {
            Ok(_) => {
                stats.tx_count = stats.tx_count.saturating_add(1);
                stats.last_tx_at = Some(std::time::Instant::now());
                stats.status = ProtocolStatus::Live;
            }
            Err(TrySendError::Full(_)) => {}
            Err(TrySendError::Disconnected(_)) => {
                stats.status = ProtocolStatus::Error;
            }
        }
    }
}
