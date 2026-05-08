use bevy::prelude::*;
use bevy_egui::egui::{self, Color32, Pos2, Rect, RichText, Sense, Stroke, StrokeKind, Ui, Vec2};

use crate::theme::*;
use crate::widgets;
use crate::{ActiveProtocol, IoConfig, IoPanelState};

// Legacy entry point
pub fn io_panel(mut _ctx: bevy_egui::EguiContexts, mut _cfg: ResMut<IoConfig>) {}

// ═══════════════════════════════════════════════════════════════════════════════
// DMX I/O Panel (docked / inline)
// ═══════════════════════════════════════════════════════════════════════════════

pub fn io_panel_docked(
    ui: &mut Ui,
    cfg: &mut IoConfig,
    state: &mut IoPanelState,
) {
    let available_width = ui.available_width();
    ui.set_min_width(available_width);

    // ── Protocol strip ────────────────────────────────────────────────────────
    {
        let strip_height = 48.0;
        let (rect, _response) = ui.allocate_exact_size(Vec2::new(available_width, strip_height), Sense::hover());
        if ui.is_rect_visible(rect) {
            let painter = ui.painter();
            painter.rect_filled(rect, 3.0, BG_INPUT);
            painter.rect_stroke(rect, 3.0, Stroke::new(1.0, BORDER_SOFT), StrokeKind::Middle);
        }

        ui.allocate_ui_at_rect(rect, |ui| {
            ui.horizontal(|ui| {
                let protocols = [
                    ("Art-Net", ActiveProtocol::ArtNet),
                    ("sACN", ActiveProtocol::Sacn),
                    ("USB", ActiveProtocol::Usb),
                    ("MIDI", ActiveProtocol::Midi),
                    ("OSC", ActiveProtocol::Osc),
                ];
                let cell_width = available_width / 5.0 - 4.0;
                for (label, proto) in protocols {
                    let active = state.active_protocol == proto;
                    let (cell_rect, response) = ui.allocate_exact_size(Vec2::new(cell_width, 40.0), Sense::click());
                    if response.clicked() {
                        state.active_protocol = proto;
                    }
                    if ui.is_rect_visible(cell_rect) {
                        let painter = ui.painter();
                        painter.rect_filled(cell_rect, 2.0, if active { BG_RAISED } else { Color32::TRANSPARENT });
                        painter.rect_stroke(cell_rect, 2.0, Stroke::new(1.0, if active { ACCENT_DIM } else { Color32::TRANSPARENT }), StrokeKind::Middle);

                        let status = match proto {
                            ActiveProtocol::ArtNet => status_to_dot(&cfg.artnet_status),
                            ActiveProtocol::Sacn   => status_to_dot(&cfg.sacn_status),
                            ActiveProtocol::Usb    => status_to_dot(&cfg.usb_status),
                            ActiveProtocol::Midi   => status_to_dot(&cfg.midi_status),
                            ActiveProtocol::Osc    => status_to_dot(&cfg.osc_status),
                        };
                        let dot_y = cell_rect.min.y + 10.0;
                        painter.circle_filled(Pos2::new(cell_rect.center().x, dot_y), 3.0, status.color());
                        if let Some(glow) = status.glow() {
                            painter.circle_filled(Pos2::new(cell_rect.center().x, dot_y), 6.0, glow);
                        }

                        painter.text(
                            Pos2::new(cell_rect.center().x, cell_rect.max.y - 8.0),
                            egui::Align2::CENTER_CENTER,
                            label,
                            egui::TextStyle::Body.resolve(ui.style()),
                            if active { FG } else { FG_SECONDARY },
                        );
                    }
                    ui.add_space(4.0);
                }
            });
        });
    }
    ui.add_space(10.0);

    // ── Active protocol config ────────────────────────────────────────────────
    match state.active_protocol {
        ActiveProtocol::ArtNet => artnet_config(ui, cfg),
        ActiveProtocol::Sacn => sacn_config(ui, cfg),
        ActiveProtocol::Usb => usb_config(ui, cfg),
        ActiveProtocol::Midi => midi_config(ui, cfg),
        ActiveProtocol::Osc => osc_config(ui, cfg),
    }

    // ── TX/RX counters ────────────────────────────────────────────────────────
    let (tx_count, rx_count) = match state.active_protocol {
        ActiveProtocol::ArtNet => (cfg.artnet_tx_count, cfg.artnet_rx_count),
        ActiveProtocol::Sacn   => (cfg.sacn_tx_count,   cfg.sacn_rx_count),
        ActiveProtocol::Usb    => (cfg.usb_tx_count,    0),
        ActiveProtocol::Midi   => (0,                   cfg.midi_rx_count),
        ActiveProtocol::Osc    => (0,                   cfg.osc_rx_count),
    };

    ui.add_space(12.0);
    {
        let card_height = 72.0;
        let (rect, _response) = ui.allocate_exact_size(Vec2::new(available_width, card_height), Sense::hover());
        if ui.is_rect_visible(rect) {
            let painter = ui.painter();
            painter.rect_filled(rect, 3.0, BG_CHROME);
            painter.rect_stroke(rect, 3.0, Stroke::new(1.0, BORDER_SOFT), StrokeKind::Middle);
        }

        ui.allocate_ui_at_rect(rect, |ui| {
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                let col_width = (available_width - 16.0) / 2.0;
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
}

// ═══════════════════════════════════════════════════════════════════════════════
// Per-protocol configs
// ═══════════════════════════════════════════════════════════════════════════════

fn status_to_dot(s: &str) -> widgets::DotState {
    if s.contains("bound") || s.contains("TX") || s.contains("listening") {
        widgets::DotState::Live
    } else if s.contains("busy") || s.contains("error") || s.contains("warn") {
        widgets::DotState::Warn
    } else {
        widgets::DotState::Idle
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
    ui.add_space(6.0);
}

fn artnet_config(ui: &mut Ui, cfg: &mut IoConfig) {
    config_row(ui, "Bind", |ui| {
        ui.add_sized([120.0, 24.0], egui::TextEdit::singleline(&mut cfg.artnet_ip).hint_text("0.0.0.0").text_color(FG));
    });
    config_row(ui, "Dest", |ui| {
        ui.add_sized([160.0, 24.0], egui::TextEdit::singleline(&mut cfg.artnet_dest_ip).hint_text("255.255.255.255").text_color(FG));
    });
    config_row(ui, "Universe", |ui| {
        ui.add(egui::DragValue::new(&mut cfg.artnet_out_universe).range(0_u16..=32767_u16));
        ui.label(RichText::new("0–32767").size(10.0).monospace().color(FG_FAINT));
    });
    config_row(ui, "Receive", |ui| {
        ui.horizontal(|ui| {
            let mut rx = cfg.artnet_rx_enabled;
            widgets::toggle(ui, &mut rx, "RX");
            cfg.artnet_rx_enabled = rx;
            ui.add_sized([120.0, 24.0], egui::TextEdit::singleline(&mut cfg.artnet_allowed_sources).hint_text("any").text_color(FG));
        });
    });

    // Live banner
    widgets::banner(ui, widgets::DotState::Live, &format!("bound {}:6454 · 2 nodes seen", cfg.artnet_ip));
}

fn sacn_config(ui: &mut Ui, cfg: &mut IoConfig) {
    config_row(ui, "Mode", |ui| {
        let mut tx = cfg.sacn_tx_enabled;
        let mut rx = cfg.sacn_rx_enabled;
        widgets::toggle(ui, &mut tx, "TX");
        cfg.sacn_tx_enabled = tx;
        ui.add_space(4.0);
        widgets::toggle(ui, &mut rx, "RX");
        cfg.sacn_rx_enabled = rx;
    });
    config_row(ui, "Universe", |ui| {
        ui.add(egui::DragValue::new(&mut cfg.sacn_out_universe).range(1_u16..=63999_u16));
        ui.label(RichText::new("1–63999").size(10.0).monospace().color(FG_FAINT));
    });
    config_row(ui, "Priority", |ui| {
        ui.add(egui::DragValue::new(&mut cfg.sacn_priority).range(1_u8..=200_u8));
    });
    config_row(ui, "Dest", |ui| {
        ui.add_sized([160.0, 24.0], egui::TextEdit::singleline(&mut cfg.sacn_dest_ip).hint_text("239.255.X.X").text_color(FG));
        ui.label(RichText::new("multicast").size(10.0).monospace().color(FG_FAINT));
    });
}

fn usb_config(ui: &mut Ui, cfg: &mut IoConfig) {
    config_row(ui, "State", |ui| {
        let mut en = cfg.usb_tx_enabled;
        widgets::toggle(ui, &mut en, "TX ENABLED");
        cfg.usb_tx_enabled = en;
    });
    config_row(ui, "Port", |ui| {
        ui.add_sized([130.0, 24.0], egui::TextEdit::singleline(&mut cfg.usb_port).hint_text("/dev/tty.usbserial-…").text_color(FG));
        if ui.add_sized([24.0, 24.0], egui::Button::new("▾").fill(BG_RAISED).stroke(Stroke::new(1.0, BORDER))).on_hover_text("Enumerate serial ports").clicked() {
            // TODO: populate usb_port from serialport::available_ports() via IoSupervisor
        }
    });
    config_row(ui, "Universe", |ui| {
        ui.add(egui::DragValue::new(&mut cfg.usb_universe).range(1_u16..=32767_u16));
    });

    // Warning banner (placeholder)
    widgets::banner(ui, widgets::DotState::Warn, "port busy — close other apps using this device");
}

fn midi_config(ui: &mut Ui, cfg: &mut IoConfig) {
    config_row(ui, "State", |ui| {
        let mut en = cfg.midi_enabled;
        widgets::toggle(ui, &mut en, "ENABLE");
        cfg.midi_enabled = en;
    });
    config_row(ui, "Port", |ui| {
        ui.add_sized([160.0, 24.0], egui::TextEdit::singleline(&mut cfg.midi_port).hint_text("select MIDI input…").text_color(FG));
    });

    ui.add_space(10.0);
    ui.horizontal(|ui| {
        widgets::eyebrow_widget(ui, "CC Mapping");
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.add_sized([60.0, 20.0], egui::Button::new(RichText::new("Learn").color(FG_SECONDARY)).fill(Color32::TRANSPARENT).stroke(Stroke::NONE)).clicked() {
                // TODO: MIDI learn
            }
        });
    });
    ui.add_space(6.0);

    let mut ccs = [
        ("Dimmer", &mut cfg.midi_cc_dimmer),
        ("Pan", &mut cfg.midi_cc_pan),
        ("Tilt", &mut cfg.midi_cc_tilt),
        ("Zoom", &mut cfg.midi_cc_zoom),
        ("Red", &mut cfg.midi_cc_red),
        ("Green", &mut cfg.midi_cc_green),
        ("Blue", &mut cfg.midi_cc_blue),
        ("Strobe", &mut cfg.midi_cc_strobe),
    ];

    ui.columns(2, |cols| {
        for (i, (label, val)) in ccs.iter_mut().enumerate() {
            let col = if i % 2 == 0 { &mut cols[0] } else { &mut cols[1] };
            let row_width = col.available_width();
            let (rect, _) = col.allocate_exact_size(Vec2::new(row_width, 24.0), Sense::hover());
            if col.is_rect_visible(rect) {
                let painter = col.painter();
                painter.rect_filled(rect, 2.0, BG_INPUT);
                painter.rect_stroke(rect, 2.0, Stroke::new(1.0, BORDER_SOFT), StrokeKind::Middle);
                painter.text(
                    Pos2::new(rect.min.x + 6.0, rect.center().y),
                    egui::Align2::LEFT_CENTER,
                    *label,
                    egui::FontId::monospace(10.0),
                    FG_SECONDARY,
                );
            }
            // DragValue in right sub-rect — explicit size avoids negative-width in tight columns
            let dv_w = (row_width - 50.0).clamp(28.0, 54.0);
            let dv_rect = Rect::from_min_size(
                Pos2::new(rect.max.x - dv_w - 4.0, rect.min.y + 2.0),
                Vec2::new(dv_w, 20.0),
            );
            col.allocate_ui_at_rect(dv_rect, |ui| {
                ui.add(egui::DragValue::new(*val).range(0_u8..=127_u8));
            });
            col.add_space(4.0);
        }
    });
}

fn osc_config(ui: &mut Ui, cfg: &mut IoConfig) {
    config_row(ui, "State", |ui| {
        let mut en = cfg.osc_enabled;
        widgets::toggle(ui, &mut en, "LISTENING");
        cfg.osc_enabled = en;
    });
    config_row(ui, "Port", |ui| {
        ui.add(egui::DragValue::new(&mut cfg.osc_port).range(1024_u16..=65535_u16));
    });

    ui.add_space(8.0);
    {
        let card_height = 60.0;
        let available_width = ui.available_width();
        let (rect, _response) = ui.allocate_exact_size(Vec2::new(available_width, card_height), Sense::hover());
        if ui.is_rect_visible(rect) {
            let painter = ui.painter();
            painter.rect_filled(rect, 3.0, BG_INPUT);
            painter.rect_stroke(rect, 3.0, Stroke::new(1.0, BORDER_SOFT), StrokeKind::Middle);
        }
        ui.allocate_ui_at_rect(rect, |ui| {
            ui.add_space(8.0);
            widgets::eyebrow_widget(ui, "Address Pattern");
            ui.add_space(4.0);
            ui.label(RichText::new("/fixture/{id}/{attr}").size(11.0).monospace().color(ACCENT));
            ui.label(RichText::new("f32 · 0.0–1.0 normalised").size(9.0).monospace().color(FG_MUTED));
        });
    }
}
