use bevy::prelude::*;
use bevy_egui::egui::{self, Color32, Pos2, RichText, Sense, Stroke, Ui, Vec2};
use stagelx_core::{fixture::FixtureInstance, types::{DmxAddress, FixtureId}};

use crate::theme::*;
use crate::widgets;
use crate::{FixtureLibraryRes, PatchEditState, PatchRes, PatchSelection, SpawnFixtureEvent};

// ═══════════════════════════════════════════════════════════════════════════════
// Patch Panel (docked / inline)
// ═══════════════════════════════════════════════════════════════════════════════

#[derive(Default, Clone)]
struct PatchFilterState {
    query: String,
    chip: PatchChip,
    /// Last fixture clicked without shift — anchor for range selection.
    anchor_id: Option<FixtureId>,
}

#[derive(Clone, Copy, Default, PartialEq)]
enum PatchChip {
    #[default]
    All,
    Live,
    U1,
    U2,
}

pub fn patch_panel_docked(
    ui: &mut Ui,
    patch: &mut PatchRes,
    library: &FixtureLibraryRes,
    edit: &mut PatchEditState,
    patch_sel: &mut PatchSelection,
    commands: &mut Commands,
) {
    let available_width = ui.available_width();

    let filter_id = ui.id().with("patch_filter");
    let mut filter: PatchFilterState = ui.ctx().data_mut(|d| {
        d.get_temp_mut_or_insert_with(filter_id, PatchFilterState::default).clone()
    });

    // ── Toolbar ───────────────────────────────────────────────────────────────
    ui.horizontal(|ui| {
        // Search input with drawn magnifier (Tier 1 #9)
        let search_width = (available_width - 180.0).max(0.0);
        widgets::search_input(ui, &mut filter.query, "Filter by name, type, address…", search_width);

        // Quick chips
        let chips = [("All", PatchChip::All), ("Live", PatchChip::Live), ("U1", PatchChip::U1), ("U2", PatchChip::U2)];
        for (label, chip) in chips {
            let active = filter.chip == chip;
            let btn = egui::Button::new(RichText::new(label).size(10.0).color(if active { FG } else { FG_SECONDARY }))
                .fill(if active { BG_RAISED } else { Color32::TRANSPARENT })
                .stroke(if active { Stroke::new(1.0, BORDER) } else { Stroke::NONE })
                .min_size(Vec2::new(0.0, 20.0));
            if ui.add(btn).clicked() {
                filter.chip = chip;
            }
            ui.add_space(4.0);
        }
    });

    // ── Fixture rows (Tier 2 #12: TableBuilder) ───────────────────────────────
    let mut fixtures: Vec<_> = patch.0.fixtures().collect();
    fixtures.sort_by_key(|f| f.id.0);

    let row_height = 24.0;
    let list_height = (fixtures.len() as f32 * row_height).min(180.0);

    // Tier 2 #13: Frame wraps scroll area instead of allocating rect first
    egui::Frame::new()
        .fill(BG_INPUT)
        .stroke(Stroke::new(1.0, BORDER_SOFT))
        .corner_radius(3.0)
        .inner_margin(egui::Margin::same(0))
        .show(ui, |ui| {
            egui::ScrollArea::vertical()
                .id_salt("patch_scroll")
                .max_height(list_height)
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    let full_width = ui.available_width();
                    let mods = ui.ctx().input(|i| i.modifiers);

                    // Header
                    {
                        let header_height = 20.0;
                        let (rect, _) = ui.allocate_exact_size(Vec2::new(full_width, header_height), Sense::hover());
                        let painter = ui.painter();
                        let cols = compute_columns(full_width);
                        let mut x = rect.min.x;
                        let headers = [("#", egui::Align2::RIGHT_CENTER), ("Name", egui::Align2::LEFT_CENTER), ("Fixture Type", egui::Align2::LEFT_CENTER), ("Mode", egui::Align2::LEFT_CENTER), ("Address", egui::Align2::RIGHT_CENTER), ("", egui::Align2::CENTER_CENTER)];
                        for (i, (h, align)) in headers.iter().enumerate() {
                            let col_x = x + if *align == egui::Align2::RIGHT_CENTER { cols[i] - 4.0 } else { 0.0 };
                            painter.text(
                                Pos2::new(col_x, rect.center().y),
                                *align,
                                *h,
                                font_body(),
                                FG_MUTED,
                            );
                            x += cols[i];
                        }
                    }

                    // Rows
                    for (i, f) in fixtures.iter().enumerate() {
                        let selected = patch_sel.selected_ids.contains(&f.id);
                        let (rect, response) = ui.allocate_exact_size(Vec2::new(full_width, row_height), Sense::click());
                        if response.clicked() {
                            if mods.command || mods.ctrl {
                                if patch_sel.selected_ids.contains(&f.id) {
                                    patch_sel.selected_ids.remove(&f.id);
                                } else {
                                    patch_sel.selected_ids.insert(f.id);
                                    filter.anchor_id = Some(f.id);
                                }
                            } else if mods.shift {
                                if let Some(anchor) = filter.anchor_id {
                                    if let Some(a_idx) = fixtures.iter().position(|x| x.id == anchor) {
                                        let (lo, hi) = if a_idx <= i { (a_idx, i) } else { (i, a_idx) };
                                        for fx in &fixtures[lo..=hi] {
                                            patch_sel.selected_ids.insert(fx.id);
                                        }
                                    } else {
                                        patch_sel.selected_ids.insert(f.id);
                                    }
                                } else {
                                    patch_sel.selected_ids.insert(f.id);
                                }
                            } else {
                                patch_sel.selected_ids.clear();
                                patch_sel.selected_ids.insert(f.id);
                                filter.anchor_id = Some(f.id);
                            }
                        }

                        if ui.is_rect_visible(rect) {
                            let painter = ui.painter();
                            let bg = if selected {
                                ROW_SELECTED
                            } else if i % 2 == 1 {
                                STRIPE_ODD
                            } else {
                                Color32::TRANSPARENT
                            };
                            painter.rect_filled(rect, 0.0, bg);
                            if selected {
                                painter.line_segment([Pos2::new(rect.min.x, rect.min.y), Pos2::new(rect.min.x, rect.max.y)], Stroke::new(2.0, ACCENT));
                            }
                            painter.line_segment([Pos2::new(rect.min.x, rect.min.y), Pos2::new(rect.max.x, rect.min.y)], Stroke::new(1.0, ROW_BORDER));

                            let cols = compute_columns(full_width);
                            let mut x = rect.min.x;

                            // Index
                            let idx_text = format!("{:03}", f.id.0 + 1);
                            painter.text(
                                Pos2::new(x + cols[0] - 4.0, rect.center().y),
                                egui::Align2::RIGHT_CENTER,
                                &idx_text,
                                font_body(),
                                if selected { ACCENT } else { FG_MUTED },
                            );
                            x += cols[0];

                            // Name with dot
                            painter.circle_filled(Pos2::new(x + 4.0, rect.center().y), 3.0, widgets::DotState::Idle.color());
                            painter.text(
                                Pos2::new(x + 14.0, rect.center().y),
                                egui::Align2::LEFT_CENTER,
                                &f.name,
                                font_body(),
                                FG,
                            );
                            x += cols[1];

                            // Type
                            painter.text(
                                Pos2::new(x, rect.center().y),
                                egui::Align2::LEFT_CENTER,
                                truncate(&f.fixture_type_id, 30),
                                font_body(),
                                FG_SECONDARY,
                            );
                            x += cols[2];

                            // Mode
                            painter.text(
                                Pos2::new(x, rect.center().y),
                                egui::Align2::LEFT_CENTER,
                                truncate(&f.dmx_mode, 12),
                                font_body(),
                                FG_MUTED,
                            );
                            x += cols[3];

                            // Address
                            let addr_text = format!("{}.{:03}", f.address.universe, f.address.channel);
                            painter.text(
                                Pos2::new(x + cols[4] - 4.0, rect.center().y),
                                egui::Align2::RIGHT_CENTER,
                                &addr_text,
                                font_address(),
                                if selected { FG } else { FG_SECONDARY },
                            );
                            x += cols[4];

                            // Status
                            painter.text(
                                Pos2::new(x + cols[5] - 4.0, rect.center().y),
                                egui::Align2::RIGHT_CENTER,
                                "OK",
                                font_body(),
                                FG_FAINT,
                            );
                        }
                    }
                });
        });

    // ── Footer ────────────────────────────────────────────────────────────────
    ui.horizontal(|ui| {
        let live_count = fixtures.len(); // placeholder
        ui.label(RichText::new(format!("{} selected", patch_sel.selected_ids.len())).size(10.0).color(ACCENT).monospace());
        ui.label(RichText::new("·").size(10.0).color(FG_MUTED));
        ui.label(RichText::new(format!("{} patched", fixtures.len())).size(10.0).color(FG).monospace());
        ui.label(RichText::new("·").size(10.0).color(FG_MUTED));
        ui.label(RichText::new(format!("{} live", live_count)).size(10.0).color(RX).monospace());
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(RichText::new("U1 81/512  ·  U2 65/512").size(10.0).monospace().color(FG_FAINT));
        });
    });

    // ── Add Fixture form ──────────────────────────────────────────────────────
    widgets::card(ui, |ui| {
        ui.horizontal(|ui| {
            widgets::eyebrow_widget(ui, "Add Fixture");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(hint("NEXT FREE: 2.078"));
            });
        });

        ui.horizontal(|ui| {
            // Type selector
            let type_ids: Vec<String> = library.library.all()
                .map(|ft| ft.fixture_type_id.clone())
                .collect();
            let type_labels: Vec<String> = library.library.all()
                .map(|ft| format!("{} · {}", ft.manufacturer, ft.name))
                .collect();

            if type_ids.is_empty() {
                ui.colored_label(FG_MUTED, "Load a GDTF fixture first.");
            } else {
                if !type_ids.contains(&edit.selected_type_id) {
                    edit.selected_type_id = type_ids[0].clone();
                }

                let type_label = type_labels
                    .get(type_ids.iter().position(|id| *id == edit.selected_type_id).unwrap_or(0))
                    .cloned()
                    .unwrap_or_default();
                let type_label_short = truncate(&type_label, 20).to_string();

                egui::ComboBox::from_label("")
                    .width(120.0)
                    .selected_text(type_label_short)
                    .show_ui(ui, |ui| {
                        for (id, label) in type_ids.iter().zip(type_labels.iter()) {
                            ui.selectable_value(&mut edit.selected_type_id, id.clone(), label);
                        }
                    });

                // Mode selector
                let modes: Vec<String> = library
                    .library
                    .get(&edit.selected_type_id)
                    .map(|ft| ft.dmx_modes.iter().map(|m| m.name.clone()).collect())
                    .unwrap_or_default();
                if !modes.is_empty() {
                    if !modes.contains(&edit.selected_mode) {
                        edit.selected_mode = modes[0].clone();
                    }
                    egui::ComboBox::from_id_salt("mode_combo")
                        .width(80.0)
                        .selected_text(truncate(&edit.selected_mode, 12))
                        .show_ui(ui, |ui| {
                            for m in &modes {
                                ui.selectable_value(&mut edit.selected_mode, m.clone(), m);
                            }
                        });
                }

                ui.add_sized([(available_width * 0.15).min(100.0), 24.0], egui::TextEdit::singleline(&mut edit.new_name).hint_text("Fixture name"));
                ui.add_sized([60.0, 24.0], egui::TextEdit::singleline(&mut edit.universe_str).hint_text("Univ"));
                ui.add_sized([60.0, 24.0], egui::TextEdit::singleline(&mut edit.channel_str).hint_text("Ch"));

                if ui.add_sized([80.0, 24.0], egui::Button::new(RichText::new("+ Patch").color(ACCENT)).fill(ACCENT_BG).stroke(Stroke::new(1.0, ACCENT_DIM))).clicked() {
                    match add_fixture(patch, edit, library, commands) {
                        Ok(()) => {}
                        Err(e) => edit.add_error = Some(e),
                    }
                }
            }
        });

        if let Some(ref err) = edit.add_error.clone() {
            ui.add_space(4.0);
            ui.label(error_text(err));
        }
    });

    // Save filter state
    ui.ctx().data_mut(|d| {
        d.insert_temp(filter_id, filter);
    });
}

fn add_fixture(
    patch: &mut PatchRes,
    edit: &mut PatchEditState,
    library: &FixtureLibraryRes,
    commands: &mut Commands,
) -> Result<(), String> {
    let universe: u16 = edit.universe_str.trim().parse()
        .map_err(|_| "Universe must be a number (1–32767)".to_string())?;
    let channel: u16 = edit.channel_str.trim().parse()
        .map_err(|_| "Channel must be 1–512".to_string())?;

    if universe == 0 {
        return Err("Universe must be ≥ 1".into());
    }
    if channel == 0 || channel > 512 {
        return Err("Channel must be 1–512".into());
    }
    if edit.selected_type_id.is_empty() {
        return Err("No fixture type selected".into());
    }

    let name = if edit.new_name.trim().is_empty() {
        format!("Fixture {}", patch.0.len() + 1)
    } else {
        edit.new_name.trim().to_string()
    };

    let channel_map = library
        .library
        .get(&edit.selected_type_id)
        .map(|ft| ft.channel_map(&edit.selected_mode))
        .unwrap_or_default();

    let id = patch.0.add(FixtureInstance {
        id: FixtureId(0),
        name,
        fixture_type_id: edit.selected_type_id.clone(),
        dmx_mode: edit.selected_mode.clone(),
        address: DmxAddress::new(universe, channel),
        position: [0.0, 6.0, 0.0],
        rotation: [0.0, 0.0, 0.0],
        channel_map,
    });

    commands.trigger(SpawnFixtureEvent(id));

    edit.new_name.clear();
    edit.universe_str.clear();
    edit.channel_str.clear();
    edit.add_error = None;
    Ok(())
}

fn compute_columns(full_width: f32) -> [f32; 6] {
    let fixed = 32.0 + 78.0 + 32.0 + 24.0; // # + Address + Status + gaps
    let remainder = (full_width - fixed).max(0.0);
    [
        32.0,
        remainder * 0.30,
        remainder * 0.35,
        remainder * 0.35,
        78.0,
        32.0,
    ]
}

fn truncate(s: &str, max: usize) -> &str {
    match s.char_indices().nth(max) {
        Some((idx, _)) => &s[..idx],
        None => s,
    }
}
