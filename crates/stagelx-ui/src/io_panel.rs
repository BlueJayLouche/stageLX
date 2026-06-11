use bevy::prelude::*;
use bevy_egui::egui::{self, Color32, Pos2, Rect, RichText, Sense, Stroke, StrokeKind, Ui, Vec2};

use crate::theme::*;
use crate::widgets;
use crate::{ActiveProtocol, IoPanelState};
use stagelx_io::artnet::ArtNetNodeTable;
use stagelx_io::config::{ArtNetConfig, MidiConfig, OscConfig, SacnConfig, UsbConfig};
use stagelx_io::midi::MidiTarget;
use stagelx_io::osc::OscState;
use std::sync::atomic::Ordering;
use stagelx_io::stats::{ArtNetStats, MidiStats, OscStats, SacnStats, UsbStats};
use stagelx_show::ProtocolStatus;

// ═══════════════════════════════════════════════════════════════════════════════
// DMX I/O Panel (docked / inline)
// ═══════════════════════════════════════════════════════════════════════════════

#[allow(clippy::too_many_arguments)]
pub fn io_panel_docked(
    ui: &mut Ui,
    artnet_cfg: &mut ArtNetConfig,
    artnet_stats: &ArtNetStats,
    artnet_nodes: &ArtNetNodeTable,
    sacn_cfg: &mut SacnConfig,
    sacn_stats: &SacnStats,
    usb_cfg: &mut UsbConfig,
    usb_stats: &UsbStats,
    midi_cfg: &mut MidiConfig,
    midi_stats: &MidiStats,
    midi_target: &MidiTarget,
    osc_cfg: &mut OscConfig,
    osc_stats: &OscStats,
    osc_state: &OscState,
    state: &mut IoPanelState,
) {
    // ── Protocol strip ────────────────────────────────────────────────────────
    widgets::card(ui, |ui| {
        let available_width = ui.available_width();
        ui.horizontal(|ui| {
            let protocols = [
                ("Art-Net", ActiveProtocol::ArtNet),
                ("sACN", ActiveProtocol::Sacn),
                ("USB", ActiveProtocol::Usb),
                ("MIDI", ActiveProtocol::Midi),
                ("OSC", ActiveProtocol::Osc),
            ];
            let n = protocols.len() as f32;
            let spacing = 4.0;
            let item_spacing_x = ui.spacing().item_spacing.x;
            let total_gap = (spacing + item_spacing_x) * (n - 1.0);
            let cell_width = (available_width - total_gap) / n;
            for (i, &(label, proto)) in protocols.iter().enumerate() {
                let active = state.active_protocol == proto;
                let (cell_rect, response) = ui.allocate_exact_size(Vec2::new(cell_width, 40.0), Sense::click());
                if response.clicked() {
                    state.active_protocol = proto;
                }
                if ui.is_rect_visible(cell_rect) {
                    let painter = ui.painter();
                    painter.rect_filled(cell_rect, 2.0, if active { BG_RAISED } else { Color32::TRANSPARENT });
                    painter.rect_stroke(cell_rect, 2.0, Stroke::new(1.0, if active { ACCENT_DIM } else { Color32::TRANSPARENT }), StrokeKind::Inside);

                    let status = match proto {
                        ActiveProtocol::ArtNet => artnet_stats.status,
                        ActiveProtocol::Sacn   => sacn_stats.status,
                        ActiveProtocol::Usb    => usb_stats.status,
                        ActiveProtocol::Midi   => midi_stats.status,
                        ActiveProtocol::Osc    => osc_stats.status,
                    };
                    let dot_y = cell_rect.min.y + 10.0;
                    painter.circle_filled(Pos2::new(cell_rect.center().x, dot_y), 3.0, status_to_dot(status).color());
                    if let Some(glow) = status_to_dot(status).glow() {
                        painter.circle_filled(Pos2::new(cell_rect.center().x, dot_y), 6.0, glow);
                    }

                    painter.text(
                        Pos2::new(cell_rect.center().x, cell_rect.max.y - 8.0),
                        egui::Align2::CENTER_CENTER,
                        label,
                        font_body(),
                        if active { FG } else { FG_SECONDARY },
                    );
                }
                if i + 1 < n as usize { ui.add_space(spacing); }
            }
        });
    });

    // ── Active protocol config ────────────────────────────────────────────────
    match state.active_protocol {
        ActiveProtocol::ArtNet => artnet_config(ui, artnet_cfg, artnet_nodes),
        ActiveProtocol::Sacn => sacn_config(ui, sacn_cfg),
        ActiveProtocol::Usb => usb_config(ui, usb_cfg, usb_stats, &mut state.scanned_ports),
        ActiveProtocol::Midi => midi_config(ui, midi_cfg, midi_target),
        ActiveProtocol::Osc => osc_config(ui, osc_cfg, osc_stats, osc_state),
    }

    // ── TX/RX counters ────────────────────────────────────────────────────────
    let (tx_count, rx_count) = match state.active_protocol {
        ActiveProtocol::ArtNet => (artnet_stats.tx_count, artnet_stats.rx_count),
        ActiveProtocol::Sacn   => (sacn_stats.tx_count,   sacn_stats.rx_count),
        ActiveProtocol::Usb    => (usb_stats.tx_count,    0),
        ActiveProtocol::Midi   => (0,                     midi_stats.rx_count),
        ActiveProtocol::Osc    => (0,                     osc_stats.rx_count),
    };

    widgets::card(ui, |ui| {
        let available_width = ui.available_width();
        ui.horizontal(|ui| {
            let col_width = ((available_width - 16.0) / 2.0).max(0.0);
            // TX
            ui.allocate_ui_with_layout(Vec2::new(col_width, 56.0), egui::Layout::top_down(egui::Align::LEFT), |ui| {
                ui.horizontal(|ui| {
                    widgets::status_dot(ui, widgets::DotState::Tx);
                    ui.label(RichText::new("TX").size(9.0).strong().color(FG_MUTED).monospace());
                });
                ui.label(RichText::new(format!("{}", tx_count)).size(16.0).monospace().color(FG));
                ui.label(RichText::new("packets/s").size(9.0).monospace().color(FG_FAINT));
            });

            // RX
            ui.allocate_ui_with_layout(Vec2::new(col_width, 56.0), egui::Layout::top_down(egui::Align::LEFT), |ui| {
                ui.horizontal(|ui| {
                    widgets::status_dot(ui, widgets::DotState::Live);
                    ui.label(RichText::new("RX").size(9.0).strong().color(FG_MUTED).monospace());
                });
                ui.label(RichText::new(format!("{}", rx_count)).size(16.0).monospace().color(FG));
                ui.label(RichText::new("packets/s").size(9.0).monospace().color(FG_FAINT));
            });
        });
    });
}

// ═══════════════════════════════════════════════════════════════════════════════
// Per-protocol configs
// ═══════════════════════════════════════════════════════════════════════════════

fn status_to_dot(s: ProtocolStatus) -> widgets::DotState {
    match s {
        ProtocolStatus::Live => widgets::DotState::Live,
        ProtocolStatus::Warn => widgets::DotState::Warn,
        ProtocolStatus::Error => widgets::DotState::Error,
        ProtocolStatus::Idle => widgets::DotState::Idle,
    }
}

fn config_row(ui: &mut Ui, label: &str, content: impl FnOnce(&mut Ui)) {
    ui.horizontal(|ui| {
        ui.add_space(0.0);
        ui.allocate_ui_with_layout(Vec2::new(76.0, 24.0), egui::Layout::left_to_right(egui::Align::Center), |ui| {
            ui.label(RichText::new(label.to_uppercase()).size(10.0).strong().color(FG_MUTED));
        });
        ui.add_space(8.0);
        content(ui);
    });
}

fn artnet_config(ui: &mut Ui, cfg: &mut ArtNetConfig, nodes: &ArtNetNodeTable) {
    config_row(ui, "Bind", |ui| {
        ui.add_sized([120.0, 24.0], egui::TextEdit::singleline(&mut cfg.ip).hint_text("0.0.0.0").text_color(FG));
    });
    config_row(ui, "Dest", |ui| {
        ui.add_sized([160.0, 24.0], egui::TextEdit::singleline(&mut cfg.dest_ip).hint_text("255.255.255.255").text_color(FG));
    });
    config_row(ui, "Mode", |ui| {
        let mut tx = cfg.tx_enabled;
        widgets::toggle(ui, &mut tx, "TX");
        cfg.tx_enabled = tx;
    });
    config_row(ui, "Universe", |ui| {
        ui.add(egui::DragValue::new(&mut cfg.out_universe).range(0_u16..=32767_u16));
        ui.label(RichText::new("0–32767").size(10.0).monospace().color(FG_FAINT));
    });
    config_row(ui, "Receive", |ui| {
        ui.horizontal(|ui| {
            let mut rx = cfg.rx_enabled;
            widgets::toggle(ui, &mut rx, "RX");
            cfg.rx_enabled = rx;
            ui.add_sized([120.0, 24.0], egui::TextEdit::singleline(&mut cfg.allowed_sources).hint_text("any").text_color(FG));
        });
    });
    config_row(ui, "Discovery", |ui| {
        let mut disc = cfg.discovery_enabled;
        widgets::toggle(ui, &mut disc, "POLL");
        cfg.discovery_enabled = disc;
    });

    // Node list
    if cfg.discovery_enabled && !nodes.nodes.is_empty() {
        widgets::card(ui, |ui| {
            widgets::eyebrow_widget(ui, &format!("Discovered Nodes ({})", nodes.nodes.len()));
            ui.add_space(4.0);
            egui::ScrollArea::vertical()
                .id_salt("artnet_nodes_scroll")
                .max_height(120.0)
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    for node in nodes.nodes.values() {
                        ui.horizontal(|ui| {
                            widgets::status_dot(ui, widgets::DotState::Live);
                            let max_name_width = (ui.available_width() - 90.0).max(40.0);
                            ui.add_sized(
                                [max_name_width, 16.0],
                                egui::Label::new(RichText::new(&node.short_name).size(11.0).color(FG).strong()).truncate(),
                            );
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                ui.label(RichText::new(node.ip.to_string()).size(9.0).monospace().color(FG_MUTED));
                            });
                        });
                    }
                });
        });
    }
}

fn sacn_config(ui: &mut Ui, cfg: &mut SacnConfig) {
    config_row(ui, "Mode", |ui| {
        let mut tx = cfg.tx_enabled;
        let mut rx = cfg.rx_enabled;
        widgets::toggle(ui, &mut tx, "TX");
        cfg.tx_enabled = tx;
        ui.add_space(4.0);
        widgets::toggle(ui, &mut rx, "RX");
        cfg.rx_enabled = rx;
    });
    config_row(ui, "Universe", |ui| {
        ui.add(egui::DragValue::new(&mut cfg.out_universe).range(1_u16..=63999_u16));
        ui.label(RichText::new("1–63999").size(10.0).monospace().color(FG_FAINT));
    });
    config_row(ui, "Priority", |ui| {
        ui.add(egui::DragValue::new(&mut cfg.priority).range(1_u8..=200_u8));
    });
    config_row(ui, "Dest", |ui| {
        ui.add_sized([160.0, 24.0], egui::TextEdit::singleline(&mut cfg.dest_ip).hint_text("239.255.X.X").text_color(FG));
        ui.label(RichText::new("multicast").size(10.0).monospace().color(FG_FAINT));
    });
    config_row(ui, "RX Univ", |ui| {
        ui.add(egui::DragValue::new(&mut cfg.rx_universe).range(0_u16..=63999_u16));
        ui.label(RichText::new("0 = same as out").size(10.0).monospace().color(FG_FAINT));
    });
}

fn usb_config(ui: &mut Ui, cfg: &mut UsbConfig, stats: &UsbStats, scanned_ports: &mut Vec<String>) {
    config_row(ui, "State", |ui| {
        let mut en = cfg.tx_enabled;
        widgets::toggle(ui, &mut en, "TX ENABLED");
        cfg.tx_enabled = en;
    });
    config_row(ui, "Port", |ui| {
        ui.add_sized([130.0, 24.0], egui::TextEdit::singleline(&mut cfg.port).hint_text("/dev/tty.usbserial-…").text_color(FG));
        let popup_id = ui.make_persistent_id("usb_port_popup");
        let btn = ui.add_sized([24.0, 24.0], egui::Button::new("▾").fill(BG_RAISED).stroke(Stroke::new(1.0, BORDER))).on_hover_text("Enumerate serial ports");
        if btn.clicked() {
            *scanned_ports = serialport::available_ports()
                .unwrap_or_default()
                .into_iter()
                .map(|p| p.port_name)
                .collect();
            #[allow(deprecated)]
            ui.memory_mut(|m| m.toggle_popup(popup_id));
        }
        #[allow(deprecated)]
        egui::popup_below_widget(ui, popup_id, &btn, egui::PopupCloseBehavior::CloseOnClickOutside, |ui| {
            ui.set_min_width(180.0);
            if scanned_ports.is_empty() {
                ui.label(egui::RichText::new("No serial ports found").size(11.0).color(FG_MUTED));
            } else {
                for port in scanned_ports.iter() {
                    if ui.selectable_label(cfg.port == *port, port).clicked() {
                        cfg.port.clone_from(port);
                        #[allow(deprecated)]
                        ui.memory_mut(|m| m.close_popup(popup_id));
                    }
                }
            }
        });
    });
    config_row(ui, "Universe", |ui| {
        ui.add(egui::DragValue::new(&mut cfg.universe).range(1_u16..=32767_u16));
    });

    // Warning banner if error
    if stats.status == ProtocolStatus::Error {
        widgets::banner(ui, widgets::DotState::Warn, "port busy — close other apps using this device");
    }
}

fn midi_config(ui: &mut Ui, cfg: &mut MidiConfig, target: &MidiTarget) {
    config_row(ui, "State", |ui| {
        let mut en = cfg.enabled;
        widgets::toggle(ui, &mut en, "ENABLE");
        cfg.enabled = en;
    });
    config_row(ui, "Port", |ui| {
        ui.add_sized([160.0, 24.0], egui::TextEdit::singleline(&mut cfg.port).hint_text("select MIDI input…").text_color(FG));
    });

    config_row(ui, "Target", |ui| {
        let mut selected = cfg.target_selected_fixtures;
        let label = if selected { "SELECTED FIXTURES" } else { "GLOBAL" };
        widgets::toggle(ui, &mut selected, label);
        cfg.target_selected_fixtures = selected;
        if selected {
            ui.label(RichText::new(format!("{} fixtures", target.fixture_ids.len())).size(10.0).monospace().color(FG_FAINT));
        }
    });

    ui.horizontal(|ui| {
        widgets::eyebrow_widget(ui, "CC Mapping");
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.add_space(6.0);
            if ui.add_sized([60.0, 20.0], egui::Button::new(RichText::new("Learn").color(FG_SECONDARY)).fill(Color32::TRANSPARENT).stroke(Stroke::NONE)).clicked() {
                // TODO: MIDI learn
            }
        });
    });

    let mut ccs = [
        ("Dimmer", &mut cfg.cc_dimmer),
        ("Pan", &mut cfg.cc_pan),
        ("Tilt", &mut cfg.cc_tilt),
        ("Zoom", &mut cfg.cc_zoom),
        ("Red", &mut cfg.cc_red),
        ("Green", &mut cfg.cc_green),
        ("Blue", &mut cfg.cc_blue),
        ("Strobe", &mut cfg.cc_strobe),
    ];

    ui.columns(2, |cols| {
        for (i, (label, val)) in ccs.iter_mut().enumerate() {
            let col = if i % 2 == 0 { &mut cols[0] } else { &mut cols[1] };
            let row_width = col.available_width();
            let (rect, _) = col.allocate_exact_size(Vec2::new(row_width, 24.0), Sense::hover());
            if col.is_rect_visible(rect) {
                let painter = col.painter();
                painter.rect_filled(rect, 2.0, BG_INPUT);
                painter.rect_stroke(rect, 2.0, Stroke::new(1.0, BORDER_SOFT), StrokeKind::Inside);
                painter.text(
                    Pos2::new(rect.min.x + 6.0, rect.center().y),
                    egui::Align2::LEFT_CENTER,
                    *label,
                    font_status(),
                    FG_SECONDARY,
                );
            }
            // DragValue in right sub-rect — explicit size avoids negative-width in tight columns
            let dv_w = (row_width - 50.0).clamp(28.0, 54.0);
            let dv_rect = Rect::from_min_size(
                Pos2::new(rect.max.x - dv_w - 4.0, rect.min.y + 2.0),
                Vec2::new(dv_w, 20.0),
            );
            col.scope_builder(egui::UiBuilder::new().max_rect(dv_rect), |ui| {
                ui.add(egui::DragValue::new(*val).range(0_u8..=127_u8));
            });
            col.add_space(4.0);
        }
    });

    ui.add_space(4.0);
    widgets::eyebrow_widget(ui, "Note Triggers");

    ui.horizontal(|ui| {
        ui.label(RichText::new("GO").size(11.0).color(FG_SECONDARY));
        let mut go_enabled = cfg.note_go.is_some();
        ui.checkbox(&mut go_enabled, "");
        if go_enabled {
            let mut note = cfg.note_go.unwrap_or(60);
            ui.add(egui::DragValue::new(&mut note).range(0_u8..=127_u8));
            cfg.note_go = Some(note);
        } else {
            cfg.note_go = None;
            ui.label(RichText::new("—").size(11.0).color(FG_FAINT));
        }

        ui.add_space(12.0);

        ui.label(RichText::new("BACK").size(11.0).color(FG_SECONDARY));
        let mut back_enabled = cfg.note_back.is_some();
        ui.checkbox(&mut back_enabled, "");
        if back_enabled {
            let mut note = cfg.note_back.unwrap_or(61);
            ui.add(egui::DragValue::new(&mut note).range(0_u8..=127_u8));
            cfg.note_back = Some(note);
        } else {
            cfg.note_back = None;
            ui.label(RichText::new("—").size(11.0).color(FG_FAINT));
        }
    });
}

fn osc_config(ui: &mut Ui, cfg: &mut OscConfig, stats: &OscStats, state: &OscState) {
    config_row(ui, "State", |ui| {
        let mut en = cfg.enabled;
        widgets::toggle(ui, &mut en, "LISTENING");
        cfg.enabled = en;
    });
    config_row(ui, "Port", |ui| {
        ui.add(egui::DragValue::new(&mut cfg.port).range(1024_u16..=65535_u16));
    });

    // ── TouchOSC connection guide ──────────────────────────────────────────────
    if cfg.enabled {
        if let (Some(ip), Some(port)) = (&state.local_ip, state.bound_port) {
            let raw = state.raw_rx_count.load(Ordering::Relaxed);
            let errs = state.decode_errors.load(Ordering::Relaxed);
            widgets::card(ui, |ui| {
                widgets::eyebrow_widget(ui, "TouchOSC Setup");
                ui.add_space(2.0);
                ui.label(RichText::new("Send to:").size(10.0).color(FG_MUTED));
                ui.horizontal(|ui| {
                    let addr = format!("{}:{}", ip, port);
                    ui.label(RichText::new(&addr).size(12.0).monospace().strong().color(ACCENT));
                });
                ui.add_space(4.0);
                ui.label(RichText::new("In TouchOSC: Connection → OSC").size(9.0).monospace().color(FG_FAINT));
                ui.label(RichText::new(format!("Host: {}  Send Port: {}", ip, port)).size(9.0).monospace().color(FG_FAINT));
                ui.add_space(4.0);
                // Diagnostic counters — tell user whether UDP is arriving at all
                let raw_color = if raw > 0 { ACCENT } else { FG_MUTED };
                ui.label(RichText::new(format!("UDP rx: {}  decode errors: {}", raw, errs)).size(9.0).monospace().color(raw_color));
            });
        }
    }

    widgets::card(ui, |ui| {
        widgets::eyebrow_widget(ui, "Address Patterns");
        ui.label(RichText::new("/fixture/{id}/{attr}").size(11.0).monospace().color(ACCENT));
        ui.label(RichText::new("f32 · 0.0–1.0 normalised").size(9.0).monospace().color(FG_MUTED));
        ui.add_space(4.0);
        ui.label(RichText::new("/cue/go  /cue/back").size(11.0).monospace().color(ACCENT));
        ui.label(RichText::new("trigger cue playback").size(9.0).monospace().color(FG_MUTED));
    });

    if !stats.last_messages.is_empty() {
        widgets::card(ui, |ui| {
            widgets::eyebrow_widget(ui, "Last Received");
            ui.add_space(2.0);
            for line in stats.last_messages.iter().rev() {
                ui.label(RichText::new(line).size(10.0).monospace().color(FG_SECONDARY));
            }
        });
    }
}
