use bevy::prelude::*;
use bevy_egui::egui::{self, Color32, FontId, Pos2, RichText, Stroke, Ui, Vec2};
use stagelx_gdtf::{parse_mvr, export_mvr};
use crate::VenueLoadState;
use std::io::Read;
use std::sync::{Arc, Mutex};

use crate::theme::*;
use crate::widgets;
use crate::{FixtureLibraryRes, LoadMvrStructureEvent, LoadVenueEvent, MvrStructureObject, PatchRes, SpawnFixtureEvent};

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
    let tab_id = ui.id().with("lib_tab");
    let mut tab: LibraryTab = ui.ctx().data_mut(|d| {
        *d.get_temp_mut_or_insert_with(tab_id, LibraryTab::default)
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
            if widgets::library_tab(ui, label, active, Some(badge)).clicked() {
                tab = t;
            }
        }
    });

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
        widgets::search_input(ui, &mut q, "Search manufacturer, model…", search_width);
        ui.ctx().data_mut(|d| d.insert_temp(search_id, q));
    });

    // List grid
    let header_height = 24.0;
    let row_height = 28.0;
    let ft_count = res.library.all().count();
    let list_height = header_height + row_height * ft_count as f32;

    // Tier 2 #13: Frame wraps scroll area
    egui::Frame::new()
        .fill(BG_INPUT)
        .stroke(Stroke::new(1.0, BORDER_SOFT))
        .corner_radius(3.0)
        .inner_margin(egui::Margin::same(0))
        .show(ui, |ui| {
            egui::ScrollArea::vertical()
                .id_salt("library_scroll")
                .max_height(list_height.min(220.0))
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    // Header
                    {
                        let (rect, _) = ui.allocate_exact_size(Vec2::new(available_width, header_height), egui::Sense::hover());
                        let painter = ui.painter();
                        let cols = compute_library_columns(available_width);
                        let mut x = rect.min.x;
                        let headers = [("Manufacturer", egui::Align2::LEFT_CENTER), ("Model", egui::Align2::LEFT_CENTER), ("Modes", egui::Align2::LEFT_CENTER), ("Used", egui::Align2::RIGHT_CENTER)];
                        for (i, (h, align)) in headers.iter().enumerate() {
                            let col_x = x + if *align == egui::Align2::RIGHT_CENTER { cols[i] - 4.0 } else { 4.0 };
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

                    for ft in res.library.all() {
                        let (rect, _) = ui.allocate_exact_size(Vec2::new(available_width, row_height), egui::Sense::hover());
                        let painter = ui.painter();
                        let cols = compute_library_columns(available_width);
                        let mut x = rect.min.x;
                        let used = patch.0.fixtures().filter(|f| f.fixture_type_id == ft.fixture_type_id).count();

                        // Manufacturer
                        painter.text(
                            Pos2::new(x + 4.0, rect.center().y),
                            egui::Align2::LEFT_CENTER,
                            truncate(&ft.manufacturer, 20),
                            font_body(),
                            FG_SECONDARY,
                        );
                        x += cols[0];

                        // Model
                        painter.text(
                            Pos2::new(x + 4.0, rect.center().y),
                            egui::Align2::LEFT_CENTER,
                            truncate(&ft.name, 24),
                            font_body(),
                            FG,
                        );
                        x += cols[1];

                        // Modes
                        let first_mode_ch = ft.dmx_modes.first().map(|m| m.channels.len()).unwrap_or(0);
                        painter.text(
                            Pos2::new(x + 4.0, rect.center().y),
                            egui::Align2::LEFT_CENTER,
                            format!("{} · {}ch", ft.dmx_modes.len(), first_mode_ch),
                            FontId::monospace(10.0),
                            FG_MUTED,
                        );
                        x += cols[2];

                        // Used
                        let used_text = if used > 0 { format!("{}", used) } else { "—".to_string() };
                        let used_color = if used > 0 { ACCENT } else { FG_FAINT };
                        painter.text(
                            Pos2::new(x + cols[3] - 4.0, rect.center().y),
                            egui::Align2::RIGHT_CENTER,
                            used_text,
                            FontId::monospace(11.0),
                            used_color,
                        );

                        painter.line_segment([Pos2::new(rect.min.x, rect.max.y), Pos2::new(rect.max.x, rect.max.y)], Stroke::new(1.0, ROW_BORDER));
                    }
                });
        });

    let gdtf_dialog_id = ui.id().with("gdtf_file_dialog");
    if widgets::dropzone(ui, "Import GDTF", ".gdtf · browse or type path below") {
        let pending = PendingDialog::new(|| rfd::FileDialog::new().add_filter("GDTF", &["gdtf"]).pick_file());
        ui.ctx().data_mut(|d| d.insert_temp(gdtf_dialog_id, pending));
    }
    if let Some(pending) = ui.ctx().data_mut(|d| d.get_temp::<PendingDialog>(gdtf_dialog_id)) {
        if let Some(result) = pending.try_take() {
            ui.ctx().data_mut(|d| d.remove_temp::<PendingDialog>(gdtf_dialog_id));
            if let Some(path) = result {
                res.import_path = path;
                load_gdtf(res);
            }
        }
    }
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
    widgets::card(ui, |ui| {
        ui.horizontal(|ui| {
            widgets::status_dot(ui, widgets::DotState::Live);
            ui.label(RichText::new("Tour 2026 — Main Stage.mvr").size(12.0).strong().color(FG));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.add_sized([60.0, 20.0], egui::Button::new(RichText::new("Re-import").color(FG_SECONDARY)).fill(Color32::TRANSPARENT).stroke(Stroke::NONE)).clicked() {
                    // TODO
                }
            });
        });
        ui.horizontal(|ui| {
            ui.label(RichText::new("Embedded GDTFs").size(10.0).monospace().color(FG_MUTED));
            ui.label(RichText::new("7").size(10.0).monospace().color(FG));
        });
        ui.horizontal(|ui| {
            ui.label(RichText::new("Fixtures imported").size(10.0).monospace().color(FG_MUTED));
            ui.label(RichText::new("11").size(10.0).monospace().color(FG));
        });
    });

    let mvr_dialog_id = ui.id().with("mvr_file_dialog");
    if widgets::dropzone(ui, "Import MVR", "loads embedded GDTFs and populates patch") {
        let pending = PendingDialog::new(|| rfd::FileDialog::new().add_filter("MVR", &["mvr"]).pick_file());
        ui.ctx().data_mut(|d| d.insert_temp(mvr_dialog_id, pending));
    }
    if let Some(pending) = ui.ctx().data_mut(|d| d.get_temp::<PendingDialog>(mvr_dialog_id)) {
        if let Some(result) = pending.try_take() {
            ui.ctx().data_mut(|d| d.remove_temp::<PendingDialog>(mvr_dialog_id));
            if let Some(path) = result {
                res.mvr_import_path = path;
                load_mvr(res, patch, commands);
            }
        }
    }
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
        ui.label(error_text(err));
    }

    ui.add_space(12.0);

    // ── Export MVR ────────────────────────────────────────────────────────────
    let export_status_id = ui.id().with("mvr_export_status");
    let mut export_status: Option<String> = ui.ctx().data_mut(|d| d.get_temp(export_status_id));

    ui.horizontal(|ui| {
        ui.label(RichText::new("Export").size(11.0).strong().color(FG_MUTED));
    });
    let mvr_save_dialog_id = ui.id().with("mvr_save_dialog");
    if ui.add_sized([available_width, 28.0], egui::Button::new(RichText::new("💾  Export current patch to MVR").size(11.0).color(FG)).fill(BG_RAISED).stroke(Stroke::new(1.0, BORDER))).clicked() {
        let pending = PendingDialog::new(|| rfd::FileDialog::new().add_filter("MVR", &["mvr"]).save_file());
        ui.ctx().data_mut(|d| d.insert_temp(mvr_save_dialog_id, pending));
    }
    if let Some(pending) = ui.ctx().data_mut(|d| d.get_temp::<PendingDialog>(mvr_save_dialog_id)) {
        if let Some(result) = pending.try_take() {
            ui.ctx().data_mut(|d| d.remove_temp::<PendingDialog>(mvr_save_dialog_id));
            if let Some(path) = result {
                let fixtures: Vec<_> = patch.0.fixtures().collect();
                if fixtures.is_empty() {
                    export_status = Some("No fixtures in patch to export.".into());
                } else {
                    let fixture_refs: Vec<_> = fixtures.to_vec();
                    match export_mvr(&fixture_refs, |id| res.library.raw_bytes(id).map(|b| b.to_vec()), "stageLX Export") {
                        Ok(bytes) => {
                            match std::fs::write(&path, &bytes) {
                                Ok(_) => export_status = Some(format!("Saved to {}", path)),
                                Err(e) => export_status = Some(format!("Write error: {e}")),
                            }
                        }
                        Err(e) => export_status = Some(format!("Export error: {e}")),
                    }
                }
            }
        }
    }
    if let Some(status) = &export_status {
        ui.label(RichText::new(status).size(10.0).color(if status.starts_with("Saved") { ACCENT } else { ERROR }));
    }
    ui.ctx().data_mut(|d| d.insert_temp(export_status_id, export_status));
}

fn venue_tab(
    ui: &mut Ui,
    venue_state: &mut VenueLoadState,
    commands: &mut Commands,
) {
    let available_width = ui.available_width();

    // Loaded venue card
    widgets::card(ui, |ui| {
        ui.horizontal(|ui| {
            widgets::status_dot(ui, widgets::DotState::Tx);
            ui.label(RichText::new("arena-mainstage.glb").size(12.0).strong().color(FG));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.add_sized([60.0, 20.0], egui::Button::new(RichText::new("Reload").color(FG_SECONDARY)).fill(Color32::TRANSPARENT).stroke(Stroke::NONE)).clicked() {
                    // TODO
                }
            });
        });
        ui.horizontal(|ui| {
            ui.label(RichText::new("Format").size(10.0).monospace().color(FG_MUTED));
            ui.label(RichText::new("glTF Binary").size(10.0).monospace().color(FG));
        });
        ui.horizontal(|ui| {
            ui.label(RichText::new("Tris").size(10.0).monospace().color(FG_MUTED));
            ui.label(RichText::new("184,302").size(10.0).monospace().color(FG));
        });
    });

    // World offset controls
    ui.horizontal(|ui| {
        ui.label(RichText::new("World Offset").size(10.0).monospace().color(FG_MUTED));
    });
    ui.horizontal(|ui| {
        ui.label(RichText::new("X").size(10.0).monospace().color(FG_MUTED));
        ui.add(egui::DragValue::new(&mut venue_state.offset[0]).suffix(" m").speed(0.1));
        ui.label(RichText::new("Y").size(10.0).monospace().color(FG_MUTED));
        ui.add(egui::DragValue::new(&mut venue_state.offset[1]).suffix(" m").speed(0.1));
        ui.label(RichText::new("Z").size(10.0).monospace().color(FG_MUTED));
        ui.add(egui::DragValue::new(&mut venue_state.offset[2]).suffix(" m").speed(0.1));
    });

    ui.add_space(8.0);

    let venue_dialog_id = ui.id().with("venue_file_dialog");
    if widgets::dropzone(ui, "Replace Venue", "OBJ · GLB · glTF · FBX") {
        let pending = PendingDialog::new(|| rfd::FileDialog::new().add_filter("Venue", &["obj", "glb", "gltf", "fbx"]).pick_file());
        ui.ctx().data_mut(|d| d.insert_temp(venue_dialog_id, pending));
    }
    if let Some(pending) = ui.ctx().data_mut(|d| d.get_temp::<PendingDialog>(venue_dialog_id)) {
        if let Some(result) = pending.try_take() {
            ui.ctx().data_mut(|d| d.remove_temp::<PendingDialog>(venue_dialog_id));
            if let Some(path) = result {
                let offset = venue_state.offset;
                commands.trigger(LoadVenueEvent { path, offset });
            }
        }
    }
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
                let offset = venue_state.offset;
                commands.trigger(LoadVenueEvent { path, offset });
            }
        }
    });

    if let Some(ref err) = venue_state.import_error.clone() {
        ui.label(error_text(err));
    }
}

/// Async file dialog helper. Spawned on a background thread so the UI doesn't freeze.
#[derive(Clone, Default)]
struct PendingDialog {
    result: Arc<Mutex<Option<Option<String>>>>,
}

impl PendingDialog {
    fn new(pick_fn: impl FnOnce() -> Option<std::path::PathBuf> + Send + 'static) -> Self {
        let result = Arc::new(Mutex::new(None));
        let result_clone = result.clone();
        std::thread::spawn(move || {
            *result_clone.lock().unwrap() = Some(pick_fn().map(|p| p.to_string_lossy().to_string()));
        });
        Self { result }
    }

    /// Returns `None` if still pending, `Some(None)` if cancelled, `Some(Some(path))` if selected.
    fn try_take(&self) -> Option<Option<String>> {
        self.result.lock().unwrap().take()
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

    // ── Extract structure geometry (SceneObject / Truss) ─────────────────────
    let has_structure = !scene.scene_objects.is_empty() || !scene.trusses.is_empty();
    if has_structure {
        let temp_dir = std::env::temp_dir().join("stagelx_mvr_geometry");
        let _ = std::fs::create_dir_all(&temp_dir);

        let cursor = std::io::Cursor::new(&data);
        if let Ok(mut archive) = zip::ZipArchive::new(cursor) {
            // Pre-collect names for case-insensitive lookup.
            let names: Vec<String> = (0..archive.len())
                .filter_map(|i| archive.by_index(i).ok().map(|f| f.name().to_string()))
                .collect();

            let mut structure_objects = Vec::new();

            let mut extract_geometry = |name: &str, position: [f32; 3], rotation: [f32; 3], geo: &stagelx_gdtf::MvrGeometry3D| {
                let target_lower = geo.file_name.to_ascii_lowercase();
                let zip_name = names.iter().find(|n| n.to_ascii_lowercase() == target_lower)?;
                let mut file = archive.by_name(zip_name).ok()?;
                let mut bytes = Vec::new();
                file.read_to_end(&mut bytes).ok()?;
                let temp_path = temp_dir.join(&geo.file_name);
                std::fs::write(&temp_path, &bytes).ok()?;
                let path_str = temp_path.to_str()?.to_string();
                Some(MvrStructureObject {
                    name: name.to_string(),
                    file_path: path_str,
                    position,
                    rotation,
                })
            };

            for obj in &scene.scene_objects {
                for geo in &obj.geometries {
                    if let Some(so) = extract_geometry(&obj.name, obj.position, obj.rotation, geo) {
                        structure_objects.push(so);
                    }
                }
            }
            for truss in &scene.trusses {
                for geo in &truss.geometries {
                    if let Some(so) = extract_geometry(&truss.name, truss.position, truss.rotation, geo) {
                        structure_objects.push(so);
                    }
                }
            }

            if !structure_objects.is_empty() {
                let count = structure_objects.len();
                commands.trigger(LoadMvrStructureEvent { objects: structure_objects });
                bevy::log::info!("MVR structure: {} geometry object(s) extracted", count);
            }
        }
    }

    // ── Load embedded GDTF files ─────────────────────────────────────────────
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

    // ── Spawn fixtures ───────────────────────────────────────────────────────
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

fn compute_library_columns(full_width: f32) -> [f32; 4] {
    let fixed = 70.0 + 40.0; // Modes + Used
    let remainder = (full_width - fixed).max(0.0);
    [
        remainder * 0.45,  // Manufacturer
        remainder * 0.55,  // Model
        70.0,              // Modes
        40.0,              // Used
    ]
}

fn truncate(s: &str, max: usize) -> &str {
    match s.char_indices().nth(max) {
        Some((idx, _)) => &s[..idx],
        None => s,
    }
}
