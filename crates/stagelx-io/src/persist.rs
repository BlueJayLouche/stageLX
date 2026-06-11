//! Save and restore all I/O configs to/from `io_config.json` in the app data dir.

use std::path::PathBuf;
use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::config::{ArtNetConfig, MidiConfig, OscConfig, SacnConfig, UsbConfig};

const FILE_NAME: &str = "io_config.json";
const APP_DIR: &str = "stageLX";

fn config_path() -> PathBuf {
    dirs::data_local_dir()
        .map(|d| d.join(APP_DIR))
        .unwrap_or_else(|| PathBuf::from("."))
        .join(FILE_NAME)
}

#[derive(Serialize, Deserialize)]
struct SavedIoConfig {
    artnet: ArtNetConfig,
    sacn: SacnConfig,
    usb: UsbConfig,
    midi: MidiConfig,
    osc: OscConfig,
}

pub fn load_io_config_on_startup(
    mut artnet: ResMut<ArtNetConfig>,
    mut sacn: ResMut<SacnConfig>,
    mut usb: ResMut<UsbConfig>,
    mut midi: ResMut<MidiConfig>,
    mut osc: ResMut<OscConfig>,
) {
    let path = config_path();
    let Ok(bytes) = std::fs::read(&path) else { return };
    let Ok(saved) = serde_json::from_slice::<SavedIoConfig>(&bytes) else {
        warn!("Failed to parse io_config.json — using defaults");
        return;
    };
    *artnet = saved.artnet;
    *sacn = saved.sacn;
    *usb = saved.usb;
    *midi = saved.midi;
    *osc = saved.osc;
    info!("I/O config loaded from {}", path.display());
}

const SAVE_DEBOUNCE_SECS: f64 = 2.0;

pub fn save_io_config_system(
    artnet: Res<ArtNetConfig>,
    sacn: Res<SacnConfig>,
    usb: Res<UsbConfig>,
    midi: Res<MidiConfig>,
    osc: Res<OscConfig>,
    time: Res<Time>,
    mut last_saved: Local<f64>,
) {
    let now = time.elapsed_secs_f64();
    if now - *last_saved < SAVE_DEBOUNCE_SECS {
        return;
    }
    *last_saved = now;

    let saved = SavedIoConfig {
        artnet: artnet.clone(),
        sacn: sacn.clone(),
        usb: usb.clone(),
        midi: midi.clone(),
        osc: osc.clone(),
    };
    let path = config_path();
    if let Some(parent) = path.parent() {
        if let Err(e) = std::fs::create_dir_all(parent) {
            warn!("Failed to create config dir: {e}");
            return;
        }
    }
    match serde_json::to_string_pretty(&saved) {
        Ok(json) => {
            if let Err(e) = std::fs::write(&path, json) {
                warn!("Failed to save io_config.json: {e}");
            } else {
                info!("I/O config saved to {}", path.display());
            }
        }
        Err(e) => warn!("Failed to serialize io_config: {e}"),
    }
}
