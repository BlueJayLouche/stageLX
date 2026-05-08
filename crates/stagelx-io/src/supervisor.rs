use std::thread::JoinHandle;

use crossbeam_channel::{Receiver, Sender};
use bevy::prelude::*;

/// Contract for a blocking IO receive thread.
///
/// Implementations spawn a thread that reads from a hardware or network source
/// and forwards typed messages to Bevy via `tx`. The thread should exit when
/// `tx` is dropped or when `shutdown` receives any value.
///
/// Channel depth must be ≤ 8 per universe — callers should use
/// `crossbeam_channel::bounded(8)` and carry post-merge snapshots, not raw packets.
pub trait IoSource: Send + 'static {
    type Msg: Send + 'static;
    fn start(tx: Sender<Self::Msg>, shutdown: Receiver<()>) -> JoinHandle<()>;
}

/// Contract for a blocking IO send thread.
///
/// Implementations spawn a thread that drains `rx` and writes to hardware or
/// the network. The thread should exit when `rx` is disconnected or when
/// `shutdown` receives any value.
pub trait IoSink: Send + 'static {
    type Cmd: Send + 'static;
    fn start(rx: Receiver<Self::Cmd>, shutdown: Receiver<()>) -> JoinHandle<()>;
}

/// Owns IO thread handles and aggregates telemetry across all transports.
///
/// New transports (MIDI, OSC, future) must register with this supervisor
/// rather than managing threads ad-hoc. Existing transports (Art-Net, sACN,
/// USB) will migrate to this in Phase 6.
#[derive(Resource, Default)]
pub struct IoSupervisor {
    /// Total messages dropped due to full channels across all sources.
    pub rx_drops: u64,
    /// Total messages dropped due to full channels across all sinks.
    pub tx_drops: u64,
}
