pub mod artnet;
pub mod config;
pub mod error;
pub mod midi;
pub mod osc;
pub mod persist;
pub mod sacn;
pub mod stats;
pub mod supervisor;
pub mod usb;

pub use artnet::dmx_engine_tick;

use bevy::prelude::*;
use artnet::{
    ArtNetState, ArtNetNodeTable,
    artnet_manage_socket, artnet_receive, artnet_send,
};
use config::{
    ArtNetConfig, SacnConfig, UsbConfig, MidiConfig, OscConfig,
};
use sacn::{SacnState, sacn_manage_socket, sacn_receive, sacn_send};
use stats::{
    ArtNetStats, SacnStats, UsbStats, MidiStats, OscStats,
};
use usb::{UsbDmxState, usb_manage_device, usb_send};
use midi::{MidiState, MidiTarget, midi_manage_connection, midi_receive};
use osc::{OscState, osc_manage_socket, osc_receive};
use supervisor::{IoSupervisor, io_supervisor_tick};
use persist::{load_io_config_on_startup, save_io_config_system};
use stagelx_dmx::engine::DmxEngineRes;
use stagelx_dmx::programmer_to_dmx;

/// Art-Net, sACN, and USB DMX output rate.  E1.31 §6.6 recommends ≥ 44 Hz.
const DMX_OUTPUT_HZ: f64 = 44.0;

pub struct IoPlugin;

impl Plugin for IoPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(DmxEngineRes::default())
            // Per-protocol config (written rarely by UI)
            .init_resource::<ArtNetConfig>()
            .init_resource::<SacnConfig>()
            .init_resource::<UsbConfig>()
            .init_resource::<MidiConfig>()
            .init_resource::<OscConfig>()
            // Per-protocol stats (written per-frame by IO systems)
            .init_resource::<ArtNetStats>()
            .init_resource::<SacnStats>()
            .init_resource::<UsbStats>()
            .init_resource::<MidiStats>()
            .init_resource::<OscStats>()
            // Internal transport state
            .init_resource::<ArtNetState>()
            .init_resource::<ArtNetNodeTable>()
            .init_resource::<SacnState>()
            .init_resource::<OscState>()
            .init_resource::<UsbDmxState>()
            .init_resource::<IoSupervisor>()
            .insert_non_send_resource(MidiState::default())
            .init_resource::<MidiTarget>()
            .insert_resource(Time::<Fixed>::from_hz(DMX_OUTPUT_HZ))
            .add_systems(Startup, load_io_config_on_startup)
            // Every render frame: manage sockets/devices, drain incoming packets.
            .add_systems(
                Update,
                (
                    artnet_manage_socket,
                    sacn_manage_socket,
                    usb_manage_device,
                    osc_manage_socket,
                    midi_manage_connection,
                    artnet_receive,
                    sacn_receive,
                    osc_receive,
                    midi_receive,
                    io_supervisor_tick,
                    save_io_config_system.run_if(
                        resource_changed::<ArtNetConfig>
                            .or(resource_changed::<SacnConfig>)
                            .or(resource_changed::<UsbConfig>)
                            .or(resource_changed::<MidiConfig>)
                            .or(resource_changed::<OscConfig>),
                    ),
                )
                    .chain(),
            )
            // Exactly 44 times/sec: write programmer → DMX, merge, send all outputs.
            .add_systems(FixedUpdate, programmer_to_dmx)
            .add_systems(FixedUpdate, dmx_engine_tick.after(programmer_to_dmx))
            .add_systems(FixedUpdate, artnet_send.after(dmx_engine_tick))
            .add_systems(FixedUpdate, sacn_send.after(artnet_send))
            .add_systems(FixedUpdate, usb_send.after(sacn_send));
    }
}
