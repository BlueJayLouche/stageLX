use bevy::prelude::*;
use bevy_egui::egui::{self, Color32, Pos2, RichText, Sense, Stroke, StrokeKind, Ui, Vec2};
use stagelx_gdtf::parse_mvr;
use crate::VenueLoadState;

use crate::theme::*;
use crate::widgets;
use crate::{FixtureLibraryRes, LoadVenueEvent, PatchRes, SpawnFixtureEvent};

// Legacy entry point (kept for API compat — all UI now routes through ui_root_system)
pub fn library_panel(
    mut _ctx: bevy_egui::EguiContexts,
    mut _res: ResMut<FixtureLibraryRes>,
) {
}

// ═══════════════════════════════════════════════════════════════════════════════
// Library Panel (docked / inline)
// ═══════════════════════════════════════════════════════════════════════════════

#[derive(Clone, Copy, Default, Debug, PartialEq)]
enum LibraryTab {
    #[default]
    Fixtures,
    Mvr,
    Venue,
}

pub fn library_panel_docked(
    ui: &mut Ui,
    res: &mut FixtureLibraryRes,
    patch: &mut PatchRes,
    venue_state: &mut VenueLoadState,
    commands: &mut Commands,
) {
    let available_width = ui.available_width();
    ui.set_min_width(available_width);

    let tab_id = ui.id().with("lib_tab");
    let mut tab: LibraryTab = ui.ctx().data_mut(|d| {
        d.get_temp_mut_or_insert_with(tab_id, LibraryTab::default).clone()
    });

    // ── Tabs ──────────────────────────────────────────────────────────────────
    ui.horizontal(|ui| {
        let ft_count = res.library.all().count();
        for (label, t, badge) in [
            ("Fixtures", LibraryTab::Fixtures, ft_count),
            ("MVR Scenes", LibraryTab::Mvr, 0usize),
            ("Venue", LibraryTab::Venue, 0usize),
        ] {
            let active = tab == t;
            let mut rich = RichText::new(label).size(11.0).color(if active { FG } else { FG_SECONDARY });
            if active {
                rich = rich.strong();
            }
            let galley = ui.painter().layout_no_wrap(label.to_string(), egui::TextStyle::Body.resolve(ui.style()), if active { FG } else { FG_SECONDARY });
            let width = galley.size().x + 32.0;
            let (rect, response) = ui.allocate_exact_size(Vec2::new(width, 26.0), Sense::click());
            if response.clicked() {
                tab = t;
            }
            if ui.is_rect_visible(rect) {
                let painter = ui.painter();
                if active {
                    painter.line_segment([Pos2::new(rect.min.x, rect.max.y - 1.0), Pos2::new(rect.max.x, rect.max.y - 1.0)], Stroke::new(1.0, ACCENT));
                }
                painter.text(
                    rect.center(),
                    egui::Align2::CENTER_CENTER,
                    label,
                    egui::TextStyle::Body.resolve(ui.style()),
                    if active { FG } else { FG_SECONDARY },
                );
                let badge_text = format!("{}", badge);
                painter.text(
                    Pos2::new(rect.center().x + galley.size().x * 0.5 + 8.0, rect.center().y),
                    egui::Align2::LEFT_CENTER,
                    &badge_text,
                    egui::TextStyle::Body.resolve(ui.style()),
                    if active { ACCENT } else { FG_MUTED },
                );
            }
        }
    });
    ui.add_space(10.0);

    match tab {
        LibraryTab::Fixtures => fixtures_tab(ui, res, patch),
        LibraryTab::Mvr => mvr_tab(ui, res, patch, commands),
        LibraryTab::Venue => venue_tab(ui, venue_state, commands),
    }

    ui.ctx().data_mut(|d| {
        d.insert_temp(tab_id, tab);
    });
}

fn fixtures_tab(
    ui: &mut Ui,
    res: &mut FixtureLibraryRes,
    patch: &PatchRes,
) {
    let available_width = ui.available_width();

    // Search — stored in egui temp data, independent of the GDTF import path
    ui.horizontal(|ui| {
        let search_width = available_width;
        let search_id = ui.id().with("lib_search_query");
        let mut q: String = ui.ctx().data_mut(|d| {
            d.get_temp_mut_or_insert_with(search_id, String::new).clone()
        });
        ui.add_sized([(search_width - 24.0).max(0.0), 24.0], egui::TextEdit::singleline(&mut q).hint_text("Search manufacturer, model…"));
        ui.ctx().data_mut(|d| d.insert_temp(search_id, q));
    });
    ui.add_space(8.0);

    // List grid
    let header_height = 24.0;
    let row_height = 28.0;
    let ft_count = res.library.all().count();
    let list_height = header_height + row_height * ft_count as f32;

    let (rect, _response) = ui.allocate_exact_size(Vec2::new(available_width, list_height.min(220.0)), Sense::hover());
    if ui.is_rect_visible(rect) {
        let painter = ui.painter();
        painter.rect_filled(rect, 3.0, BG_INPUT);
        painter.rect_stroke(rect, 3.0, Stroke::new(1.0, BORDER_SOFT), StrokeKind::Middle);
    }

    egui::ScrollArea::vertical()
        .max_height(list_height.min(220.0))
        .auto_shrink([false, false])
        .show(ui, |ui| {
            // Header
            ui.horizontal(|ui| {
                ui.set_min_size(Vec2::new(available_width, header_height));
                let cols = [available_width * 0.30, available_width * 0.35, 100.0, 60.0];
                let headers = ["Manufacturer", "Model", "Modes", "Used"];
                for (i, h) in headers.iter().enumerate() {
                    ui.label(RichText::new(*h).size(9.0).strong().color(FG_MUTED));
                    ui.add_space((cols[i] - 40.0).max(0.0));
                }
            });

            for ft in res.library.all() {
                ui.horizontal(|ui| {
                    ui.set_min_size(Vec2::new(available_width, row_height));
                    let used = patch.0.fixtures().filter(|f| f.fixture_type_id == ft.fixture_type_id).count();
                    ui.label(body_row_secondary(&ft.manufacturer));
                    ui.add_space((available_width * 0.30 - 60.0).max(0.0));
                    ui.label(body_row(&ft.name));
                    ui.add_space((available_width * 0.35 - 60.0).max(0.0));
                    let first_mode_ch = ft.dmx_modes.first().map(|m| m.channels.len()).unwrap_or(0);
                    ui.label(RichText::new(format!("{} · {}ch", ft.dmx_modes.len(), first_mode_ch)).size(10.0).monospace().color(FG_MUTED));
                    ui.add_space(60.0);
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if used > 0 {
                            ui.label(RichText::new(format!("{}", used)).size(11.0).monospace().color(ACCENT));
                        } else {
                            ui.label(RichText::new("—").size(11.0).monospace().color(FG_FAINT));
                        }
                    });
                });
                ui.painter().line_segment([Pos2::new(ui.min_rect().min.x, ui.cursor().min.y), Pos2::new(ui.min_rect().max.x, ui.cursor().min.y)], Stroke::new(1.0, ROW_BORDER));
            }
        });

    ui.add_space(10.0);

    if widgets::dropzone(ui, "Import GDTF", ".gdtf · browse or type path below") {
        if let Some(path) = rfd::FileDialog::new().add_filter("GDTF", &["gdtf"]).pick_file() {
            res.import_path = path.to_string_lossy().to_string();
            load_gdtf(res);
        }
    }
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        ui.add_sized(
            [(available_width - 70.0).max(0.0), 24.0],
            egui::TextEdit::singleline(&mut res.import_path).hint_text("Path to .gdtf file…"),
        );
        if ui.add_sized([60.0, 24.0], egui::Button::new("Load").fill(BG_RAISED).stroke(Stroke::new(1.0, BORDER))).clicked() {
            load_gdtf(res);
        }
    });

    if let Some(ref err) = res.import_error.clone() {
        ui.add_space(4.0);
        ui.label(error_text(err));
    }
}

fn mvr_tab(
    ui: &mut Ui,
    res: &mut FixtureLibraryRes,
    patch: &mut PatchRes,
    commands: &mut Commands,
) {
    let available_width = ui.available_width();

    // Loaded asset card (placeholder)
    let card_height = 80.0;
    let (rect, _response) = ui.allocate_exact_size(Vec2::new(available_width, card_height), Sense::hover());
    if ui.is_rect_visible(rect) {
        let painter = ui.painter();
        painter.rect_filled(rect, 3.0, BG_INPUT);
        painter.rect_stroke(rect, 3.0, Stroke::new(1.0, BORDER_SOFT), StrokeKind::Middle);
    }
    ui.allocate_new_ui(egui::UiBuilder::new().max_rect(rect), |ui| {
        ui.add_space(12.0);
        ui.horizontal(|ui| {
            widgets::status_dot(ui, widgets::DotState::Live);
            ui.label(RichText::new("Tour 2026 — Main Stage.mvr").size(12.0).strong().color(FG));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.add_sized([60.0, 20.0], egui::Button::new(RichText::new("Re-import").color(FG_SECONDARY)).fill(Color32::TRANSPARENT).stroke(Stroke::NONE)).clicked() {
                    // TODO
                }
            });
        });
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.label(RichText::new("Embedded GDTFs").size(10.0).monospace().color(FG_MUTED));
            ui.label(RichText::new("7").size(10.0).monospace().color(FG));
        });
        ui.horizontal(|ui| {
            ui.label(RichText::new("Fixtures imported").size(10.0).monospace().color(FG_MUTED));
            ui.label(RichText::new("11").size(10.0).monospace().color(FG));
        });
    });

    ui.add_space(8.0);

    if widgets::dropzone(ui, "Import MVR", "loads embedded GDTFs and populates patch") {
        if let Some(path) = rfd::FileDialog::new().add_filter("MVR", &["mvr"]).pick_file() {
            res.mvr_import_path = path.to_string_lossy().to_string();
            load_mvr(res, patch, commands);
        }
    }
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        ui.add_sized(
            [(available_width - 70.0).max(0.0), 24.0],
            egui::TextEdit::singleline(&mut res.mvr_import_path).hint_text("Path to .mvr file…"),
        );
        if ui.add_sized([60.0, 24.0], egui::Button::new("Load").fill(BG_RAISED).stroke(Stroke::new(1.0, BORDER))).clicked() {
            load_mvr(res, patch, commands);
        }
    });

    if let Some(ref err) = res.mvr_import_error.clone() {
        ui.add_space(4.0);
        ui.label(error_text(err));
    }
}

fn venue_tab(
    ui: &mut Ui,
    venue_state: &mut VenueLoadState,
    commands: &mut Commands,
) {
    let available_width = ui.available_width();

    // Loaded venue card
    let card_height = 80.0;
    let (rect, _response) = ui.allocate_exact_size(Vec2::new(available_width, card_height), Sense::hover());
    if ui.is_rect_visible(rect) {
        let painter = ui.painter();
        painter.rect_filled(rect, 3.0, BG_INPUT);
        painter.rect_stroke(rect, 3.0, Stroke::new(1.0, BORDER_SOFT), StrokeKind::Middle);
    }
    ui.allocate_new_ui(egui::UiBuilder::new().max_rect(rect), |ui| {
        ui.add_space(12.0);
        ui.horizontal(|ui| {
            widgets::status_dot(ui, widgets::DotState::Tx);
            ui.label(RichText::new("arena-mainstage.glb").size(12.0).strong().color(FG));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.add_sized([60.0, 20.0], egui::Button::new(RichText::new("Reload").color(FG_SECONDARY)).fill(Color32::TRANSPARENT).stroke(Stroke::NONE)).clicked() {
                    // TODO
                }
            });
        });
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.label(RichText::new("Format").size(10.0).monospace().color(FG_MUTED));
            ui.label(RichText::new("glTF Binary").size(10.0).monospace().color(FG));
        });
        ui.horizontal(|ui| {
            ui.label(RichText::new("Tris").size(10.0).monospace().color(FG_MUTED));
            ui.label(RichText::new("184,302").size(10.0).monospace().color(FG));
        });
    });

    ui.add_space(8.0);

    if widgets::dropzone(ui, "Replace Venue", "OBJ · GLB · glTF") {
        if let Some(path) = rfd::FileDialog::new().add_filter("Venue", &["obj", "glb", "gltf"]).pick_file() {
            commands.trigger(LoadVenueEvent { path: path.to_string_lossy().to_string() });
        }
    }
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        ui.add_sized(
            [(available_width - 70.0).max(0.0), 24.0],
            egui::TextEdit::singleline(&mut venue_state.import_path).hint_text("Path to .obj or .glb file…"),
        );
        if ui.add_sized([60.0, 24.0], egui::Button::new("Load").fill(BG_RAISED).stroke(Stroke::new(1.0, BORDER))).clicked() {
            let path = venue_state.import_path.trim().to_string();
            if path.is_empty() {
                venue_state.import_error = Some("Please enter a file path.".into());
            } else {
                commands.trigger(LoadVenueEvent { path });
            }
        }
    });

    if let Some(ref err) = venue_state.import_error.clone() {
        ui.add_space(4.0);
        ui.label(error_text(err));
    }
}

fn load_gdtf(res: &mut FixtureLibraryRes) {
    let path = res.import_path.trim().to_string();
    if path.is_empty() {
        res.import_error = Some("Please enter a file path.".into());
        return;
    }
    match std::fs::read(&path) {
        Ok(data) => match res.library.load(&data) {
            Ok(_) => {
                res.import_error = None;
                res.import_path.clear();
            }
            Err(e) => res.import_error = Some(format!("Parse error: {e}")),
        },
        Err(e) => res.import_error = Some(format!("Cannot read file: {e}")),
    }
}

fn load_mvr(
    res: &mut FixtureLibraryRes,
    patch: &mut PatchRes,
    commands: &mut Commands,
) {
    let path = res.mvr_import_path.trim().to_string();
    if path.is_empty() {
        res.mvr_import_error = Some("Please enter an MVR file path.".into());
        return;
    }

    let data = match std::fs::read(&path) {
        Ok(d) => d,
        Err(e) => {
            res.mvr_import_error = Some(format!("Cannot read file: {e}"));
            return;
        }
    };

    let scene = match parse_mvr(&data) {
        Ok(s) => s,
        Err(e) => {
            res.mvr_import_error = Some(format!("MVR parse error: {e}"));
            return;
        }
    };

    let mut name_to_id: std::collections::HashMap<String, String> = Default::default();
    for (filename, bytes) in &scene.gdtf_files {
        match res.library.load(bytes) {
            Ok(type_id) => {
                let key = filename.rsplit('/').next().unwrap_or(filename).to_string();
                name_to_id.insert(key, type_id);
            }
            Err(e) => {
                bevy::log::warn!("MVR: failed to load embedded GDTF '{}': {e}", filename);
            }
        }
    }

    let mut count = 0usize;
    for mut inst in scene.fixture_instances {
        if let Some(real_id) = name_to_id.get(&inst.fixture_type_id) {
            inst.fixture_type_id = real_id.clone();
        }
        if let Some(ft) = res.library.get(&inst.fixture_type_id) {
            inst.channel_map = ft.channel_map(&inst.dmx_mode);
            let id = patch.0.add(inst);
            commands.trigger(SpawnFixtureEvent(id));
            count += 1;
        } else {
            bevy::log::warn!("MVR: fixture '{}' references unknown type '{}'", inst.name, inst.fixture_type_id);
        }
    }

    res.mvr_import_error = None;
    res.mvr_import_path.clear();
    bevy::log::info!("MVR import complete: {} fixtures added from '{}'", count, path);
}
