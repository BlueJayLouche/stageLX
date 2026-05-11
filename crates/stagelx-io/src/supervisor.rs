use std::net::{SocketAddr, UdpSocket};
use std::sync::atomic::AtomicU64;
use std::sync::Arc;
use std::thread::JoinHandle;

use crossbeam_channel::{Receiver, Sender};
use bevy::prelude::*;

// ─── Traits ───────────────────────────────────────────────────────────────────

/// Contract for a blocking IO receive thread.
///
/// Implementations spawn a thread that reads from a hardware or network source
/// and forwards typed messages to Bevy via `tx`. The thread should exit when
/// `shutdown` receives a value or is dropped.
///
/// Channel depth must be ≤ 8 per universe — callers should use
/// `crossbeam_channel::bounded(8)` and carry post-merge snapshots, not raw packets.
pub trait IoSource: Send + 'static {
    type Msg: Send + 'static;
    fn start(&self, tx: Sender<Self::Msg>, shutdown: Receiver<()>) -> std::io::Result<JoinHandle<()>>;
}

/// Contract for a blocking IO send thread.
///
/// Implementations spawn a thread that drains `rx` and writes to hardware or
/// the network. The thread should exit when `rx` is disconnected or when
/// `shutdown` receives a value.
pub trait IoSink: Send + 'static {
    type Cmd: Send + 'static;
    fn start(&self, rx: Receiver<Self::Cmd>, shutdown: Receiver<()>) -> std::io::Result<JoinHandle<()>>;
}

// ─── Supervisor ───────────────────────────────────────────────────────────────

/// Owns IO thread handles and aggregates telemetry across all transports.
///
/// New transports (MIDI, OSC, future) must register with this supervisor
/// rather than managing threads ad-hoc. Existing transports (Art-Net, sACN,
/// USB) will migrate to this in Phase 6.
#[derive(Resource)]
pub struct IoSupervisor {
    /// Total messages dropped due to full channels across all sources.
    pub rx_drops: Arc<AtomicU64>,
    /// Total messages dropped due to full channels across all sinks.
    pub tx_drops: Arc<AtomicU64>,
    /// Snapshot of rx_drops from the last frame (for change detection).
    last_rx_drops: u64,
}

impl Default for IoSupervisor {
    fn default() -> Self {
        Self {
            rx_drops: Arc::new(AtomicU64::new(0)),
            tx_drops: Arc::new(AtomicU64::new(0)),
            last_rx_drops: 0,
        }
    }
}

/// Emitted when `IoSupervisor::rx_drops` increments.
/// UI should flash a warning toast.
#[derive(Event, Debug, Clone)]
pub struct IoOverflowWarning {
    pub protocol: String,
    pub rx_drops: u64,
}

/// System: watch `IoSupervisor::rx_drops` and emit `IoOverflowWarning` when it changes.
pub fn io_supervisor_tick(
    mut supervisor: ResMut<IoSupervisor>,
    mut commands: Commands,
) {
    let current = supervisor.rx_drops.load(std::sync::atomic::Ordering::Relaxed);
    if current > supervisor.last_rx_drops {
        commands.trigger(IoOverflowWarning {
            protocol: "IO".into(),
            rx_drops: current,
        });
        supervisor.last_rx_drops = current;
    }
}

// ─── Socket helpers ───────────────────────────────────────────────────────────

/// Create a UDP socket with production tuning:
/// - `SO_REUSEADDR` (and `SO_REUSEPORT` on macOS) to prevent `EADDRINUSE` on rapid rebind
/// - `SO_RCVBUF` = 4 MB to absorb burst traffic without kernel drops
/// - Non-blocking + broadcast enabled
pub fn create_tuned_udp_socket(addr: SocketAddr) -> std::io::Result<UdpSocket> {
    let domain = if addr.is_ipv4() {
        socket2::Domain::IPV4
    } else {
        socket2::Domain::IPV6
    };
    let socket = socket2::Socket::new(domain, socket2::Type::DGRAM, None)?;
    socket.set_nonblocking(true)?;
    socket.set_broadcast(true)?;
    socket.set_reuse_address(true)?;
    #[cfg(target_os = "macos")]
    socket.set_reuse_port(true)?;
    socket.set_recv_buffer_size(4 * 1024 * 1024)?;
    socket.bind(&addr.into())?;
    Ok(socket.into())
}

/// Tune an existing `std::net::UdpSocket` (for sockets created elsewhere, e.g. cloned).
pub fn tune_udp_socket(socket: &UdpSocket) -> std::io::Result<()> {
    let s2 = socket2::Socket::from(socket.try_clone()?);
    s2.set_recv_buffer_size(4 * 1024 * 1024)?;
    Ok(())
}
