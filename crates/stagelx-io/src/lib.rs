pub mod artnet;
pub mod error;
pub mod midi;
pub mod osc;
pub mod sacn;
pub mod usb;

use bevy::prelude::*;
use artnet::{
    ArtNetState, DmxEngineRes,
    artnet_manage_socket, artnet_receive, artnet_send,
    dmx_engine_tick, programmer_to_dmx,
};
use sacn::{SacnState, sacn_manage_socket, sacn_receive, sacn_send};
use usb::{UsbDmxState, usb_manage_device, usb_send};
use stagelx_dmx::engine::DmxEngine;

/// Art-Net, sACN, and USB DMX output rate.  E1.31 §6.6 recommends ≥ 44 Hz.
const DMX_OUTPUT_HZ: f64 = 44.0;

pub struct IoPlugin;

impl Plugin for IoPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(DmxEngineRes(DmxEngine::default()))
            .init_resource::<ArtNetState>()
            .init_resource::<SacnState>()
            .insert_non_send_resource(UsbDmxState::default())
            .insert_resource(Time::<Fixed>::from_hz(DMX_OUTPUT_HZ))
            // Every render frame: manage sockets/devices, drain incoming packets.
            .add_systems(
                Update,
                (
                    artnet_manage_socket,
                    sacn_manage_socket,
                    usb_manage_device,
                    artnet_receive,
                    sacn_receive,
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
