use bevy::prelude::*;
use bevy_egui::egui::{self, Color32, Pos2, RichText, Sense, Stroke, StrokeKind, Ui, Vec2};
use stagelx_core::{fixture::{DmxChannelMap, FixtureInstance}, types::{DmxAddress, FixtureId}};

use crate::theme::*;
use crate::widgets;
use crate::{FixtureLibraryRes, PatchEditState, PatchRes, PatchSelection, SpawnFixtureEvent};

// Legacy entry point
pub fn patch_panel(
    mut _ctx: bevy_egui::EguiContexts,
    mut _patch: ResMut<PatchRes>,
    _library: Res<FixtureLibraryRes>,
    mut _edit: ResMut<PatchEditState>,
    mut _commands: Commands,
) {
}

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
    ui.set_min_width(available_width);

    let filter_id = ui.id().with("patch_filter");
    let mut filter: PatchFilterState = ui.ctx().data_mut(|d| {
        d.get_temp_mut_or_insert_with(filter_id, PatchFilterState::default).clone()
    });

    // ── Toolbar ───────────────────────────────────────────────────────────────
    ui.horizontal(|ui| {
        // Search input — single widget, no overlapping painted rect
        let search_width = available_width - 180.0;
        ui.add_sized([search_width, 24.0], egui::TextEdit::singleline(&mut filter.query).hint_text("🔍  Filter by name, type, address…"));

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
    ui.add_space(8.0);

    // ── Header row ────────────────────────────────────────────────────────────
    {
        let header_height = 20.0;
        let (rect, _response) = ui.allocate_exact_size(Vec2::new(available_width, header_height), Sense::hover());
        if ui.is_rect_visible(rect) {
            let painter = ui.painter();
            let cols = [32.0, available_width * 0.25, available_width * 0.30, available_width * 0.18, 78.0, 32.0];
            let mut x = rect.min.x + 8.0;
            let headers = ["#", "Name", "Fixture Type", "Mode", "Address", ""];
            for (i, h) in headers.iter().enumerate() {
                let align = if i == 0 || i == 4 { egui::Align2::RIGHT_CENTER } else { egui::Align2::LEFT_CENTER };
                painter.text(
                    Pos2::new(x + if i == 0 || i == 4 { cols[i] - 8.0 } else { 0.0 }, rect.center().y),
                    align,
                    *h,
                    egui::TextStyle::Body.resolve(ui.style()),
                    FG_MUTED,
                );
                x += cols[i] + 8.0;
            }
            painter.line_segment([Pos2::new(rect.min.x, rect.min.y), Pos2::new(rect.max.x, rect.min.y)], Stroke::new(1.0, BORDER_SOFT));
        }
    }

    // ── Fixture rows ──────────────────────────────────────────────────────────
    let mut fixtures: Vec<_> = patch.0.fixtures().collect();
    fixtures.sort_by_key(|f| f.id.0);

    let row_height = 24.0;
    let list_height = (fixtures.len() as f32 * row_height).min(180.0);
    let (list_rect, _response) = ui.allocate_exact_size(Vec2::new(available_width, list_height), Sense::hover());

    if ui.is_rect_visible(list_rect) {
        let painter = ui.painter();
        painter.rect_filled(list_rect, 3.0, BG_INPUT);
        painter.rect_stroke(list_rect, 3.0, Stroke::new(1.0, BORDER_SOFT), StrokeKind::Middle);

        // Use egui's scroll area for the list
        egui::ScrollArea::vertical()
            .max_height(list_height)
            .auto_shrink([false, false])
            .show(ui, |ui| {
                for (i, f) in fixtures.iter().enumerate() {
                    let selected = patch_sel.selected_ids.contains(&f.id);
                    let row_rect = ui.available_rect_before_wrap();
                    let full_width = row_rect.width();

                    let response = ui.allocate_ui_with_layout(
                        Vec2::new(full_width, row_height),
                        egui::Layout::left_to_right(egui::Align::Center),
                        |ui| {
                            let (rect, response) = ui.allocate_exact_size(Vec2::new(full_width, row_height), Sense::click());
                            if response.clicked() {
                                if ui.input(|inp| inp.modifiers.command || inp.modifiers.ctrl) {
                                    if patch_sel.selected_ids.contains(&f.id) {
                                        patch_sel.selected_ids.remove(&f.id);
                                    } else {
                                        patch_sel.selected_ids.insert(f.id);
                                        filter.anchor_id = Some(f.id);
                                    }
                                } else if ui.input(|inp| inp.modifiers.shift) {
                                    // Range select: extend from anchor to current row
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

                                let mut x = rect.min.x + 8.0;
                                // Index
                                let idx_text = format!("{:03}", f.id.0 + 1);
                                painter.text(
                                    Pos2::new(x + 24.0, rect.center().y),
                                    egui::Align2::RIGHT_CENTER,
                                    &idx_text,
                                    egui::TextStyle::Body.resolve(ui.style()),
                                    if selected { ACCENT } else { FG_MUTED },
                                );
                                x += 40.0;

                                // Live/idle dot + name (dot is Idle placeholder — live tracking is future work)
                                painter.circle_filled(Pos2::new(x + 4.0, rect.center().y), 3.0, widgets::DotState::Idle.color());
                                let name_x = x + 14.0;
                                painter.text(
                                    Pos2::new(name_x, rect.center().y),
                                    egui::Align2::LEFT_CENTER,
                                    &f.name,
                                    egui::TextStyle::Body.resolve(ui.style()),
                                    FG,
                                );
                                x += full_width * 0.25 + 8.0;

                                // Type
                                painter.text(
                                    Pos2::new(x, rect.center().y),
                                    egui::Align2::LEFT_CENTER,
                                    truncate(&f.fixture_type_id, 30),
                                    egui::TextStyle::Body.resolve(ui.style()),
                                    FG_SECONDARY,
                                );
                                x += full_width * 0.30 + 8.0;

                                // Mode
                                painter.text(
                                    Pos2::new(x, rect.center().y),
                                    egui::Align2::LEFT_CENTER,
                                    truncate(&f.dmx_mode, 12),
                                    egui::TextStyle::Body.resolve(ui.style()),
                                    FG_MUTED,
                                );
                                x += full_width * 0.18 + 8.0;

                                // Address
                                let addr_text = format!("{}.{:03}", f.address.universe, f.address.channel);
                                painter.text(
                                    Pos2::new(x + 70.0, rect.center().y),
                                    egui::Align2::RIGHT_CENTER,
                                    &addr_text,
                                    egui::TextStyle::Body.resolve(ui.style()),
                                    if selected { FG } else { FG_SECONDARY },
                                );
                                x += 86.0;

                                // Status
                                painter.text(
                                    Pos2::new(rect.max.x - 8.0, rect.center().y),
                                    egui::Align2::RIGHT_CENTER,
                                    "OK",
                                    egui::TextStyle::Body.resolve(ui.style()),
                                    FG_FAINT,
                                );
                            }
                            response
                        },
                    );
                }
            });
    }

    // ── Footer ────────────────────────────────────────────────────────────────
    ui.add_space(8.0);
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
    ui.add_space(12.0);
    {
        let form_height = 90.0;
        let (rect, _response) = ui.allocate_exact_size(Vec2::new(available_width, form_height), Sense::hover());
        if ui.is_rect_visible(rect) {
            let painter = ui.painter();
            painter.rect_filled(rect, 3.0, BG_CHROME);
            painter.rect_stroke(rect, 3.0, Stroke::new(1.0, BORDER_SOFT), StrokeKind::Middle);
        }

        ui.allocate_ui_at_rect(rect, |ui| {
            ui.add_space(10.0);
            ui.horizontal(|ui| {
                widgets::eyebrow_widget(ui, "Add Fixture");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(hint("NEXT FREE: 2.078"));
                });
            });
            ui.add_space(8.0);

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

                    egui::ComboBox::from_label("")
                        .selected_text(type_label)
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
                            .selected_text(&edit.selected_mode)
                            .show_ui(ui, |ui| {
                                for m in &modes {
                                    ui.selectable_value(&mut edit.selected_mode, m.clone(), m);
                                }
                            });
                    }

                    ui.add_sized([available_width * 0.15, 24.0], egui::TextEdit::singleline(&mut edit.new_name).hint_text("Fixture name"));
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
    }

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

fn truncate(s: &str, max: usize) -> &str {
    match s.char_indices().nth(max) {
        Some((idx, _)) => &s[..idx],
        None => s,
    }
}
