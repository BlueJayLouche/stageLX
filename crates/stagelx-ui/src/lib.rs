pub mod io_panel;
pub mod library;
pub mod patch;
pub mod programmer;
pub mod theme;
pub mod widgets;

use bevy::prelude::*;
use bevy_egui::{EguiPlugin, EguiPrimaryContextPass, egui};
use bevy_egui::egui::{Color32, Pos2, Rect, RichText, Stroke, StrokeKind, TextStyle, Vec2};
use std::collections::HashSet;

pub use stagelx_state::{
    DespawnFixtureEvent, FixtureLibraryRes, IoConfig, LoadVenueEvent, PatchEditState, PatchRes,
    Programmer, SpawnFixtureEvent, VenueLoadState,
};
use stagelx_core::types::FixtureId;

use crate::theme::*;

// ═══════════════════════════════════════════════════════════════════════════════
// Panel kind enum (for detach/minimize tracking)
// ═══════════════════════════════════════════════════════════════════════════════

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PanelKind {
    Programmer,
    Patch,
    Library,
    Io,
}

// ═══════════════════════════════════════════════════════════════════════════════
// New resources
// ═══════════════════════════════════════════════════════════════════════════════

#[derive(Resource)]
pub struct UiLayoutState {
    pub detached: HashSet<PanelKind>,
    pub minimized: HashSet<PanelKind>,
    pub show_status_bar: bool,
}

impl Default for UiLayoutState {
    fn default() -> Self {
        Self {
            detached: HashSet::new(),
            minimized: HashSet::new(),
            show_status_bar: true,
        }
    }
}

#[derive(Resource, Default)]
pub struct PatchSelection {
    pub selected_ids: HashSet<FixtureId>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum ActiveProtocol {
    #[default]
    ArtNet,
    Sacn,
    Usb,
    Midi,
    Osc,
}

#[derive(Resource, Default)]
pub struct IoPanelState {
    pub active_protocol: ActiveProtocol,
}

// ═══════════════════════════════════════════════════════════════════════════════
// Plugin
// ═══════════════════════════════════════════════════════════════════════════════

pub struct StageLxUiPlugin;

impl Plugin for StageLxUiPlugin {
    fn build(&self, app: &mut App) {
        if !app.is_plugin_added::<EguiPlugin>() {
            app.add_plugins(EguiPlugin::default());
        }

        app.init_resource::<Programmer>()
            .init_resource::<PatchRes>()
            .init_resource::<PatchEditState>()
            .init_resource::<FixtureLibraryRes>()
            .init_resource::<IoConfig>()
            .init_resource::<VenueLoadState>()
            .init_resource::<UiLayoutState>()
            .init_resource::<PatchSelection>()
            .init_resource::<IoPanelState>()
            .add_systems(
                EguiPrimaryContextPass,
                ui_root_system,
            );
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Root UI system — layout shell
// ═══════════════════════════════════════════════════════════════════════════════

fn ui_root_system(
    mut ctx: bevy_egui::EguiContexts,
    mut layout: ResMut<UiLayoutState>,
    mut patch_sel: ResMut<PatchSelection>,
    mut prog: ResMut<Programmer>,
    mut patch: ResMut<PatchRes>,
    mut patch_edit: ResMut<PatchEditState>,
    mut library: ResMut<FixtureLibraryRes>,
    mut io_cfg: ResMut<IoConfig>,
    mut io_state: ResMut<IoPanelState>,
    mut venue_state: ResMut<VenueLoadState>,
    mut commands: Commands,
    windows: Query<&Window>,
) {
    let Ok(_window) = windows.single() else { return };
    let egui_ctx = ctx.ctx_mut().expect("egui context");

    // Apply global dark style
    let mut style = (*egui_ctx.style()).clone();
    style.visuals.dark_mode = true;
    style.visuals.panel_fill = BG_APP;
    style.visuals.window_fill = BG_PANEL;
    style.visuals.window_stroke = Stroke::new(1.0, BORDER);
    style.visuals.widgets.noninteractive.bg_fill = BG_PANEL;
    style.visuals.widgets.inactive.bg_fill = BG_RAISED;
    style.visuals.widgets.hovered.bg_fill = BG_HOVER;
    style.visuals.widgets.active.bg_fill = BG_INPUT;
    style.visuals.selection.bg_fill = ACCENT_BG;
    style.visuals.selection.stroke = Stroke::new(1.0, ACCENT);
    style.visuals.window_shadow = egui::epaint::Shadow {
        offset: [0, 6],
        blur: 20,
        spread: 2,
        color: Color32::from_black_alpha(90),
    };
    egui_ctx.set_style(style);

    // ── Top bar ───────────────────────────────────────────────────────────────
    egui::TopBottomPanel::top("top_bar")
        .exact_height(36.0)
        .frame(egui::Frame::new().fill(BG_CHROME).inner_margin(egui::Margin::same(0)))
        .show(egui_ctx, |ui| {
            ui.horizontal(|ui| {
                ui.set_min_size(Vec2::new(ui.available_width(), 36.0));
                ui.add_space(10.0);

                // Wordmark
                ui.label(wordmark("stage"));
                ui.label(wordmark_accent("LX"));
                ui.label(
                    RichText::new("0.1.0")
                        .size(9.0)
                        .monospace()
                        .color(FG_FAINT),
                );
                ui.add_space(14.0);

                // Divider
                ui.painter().line_segment([Pos2::new(ui.cursor().min.x, ui.cursor().min.y), Pos2::new(ui.cursor().min.x, ui.cursor().min.y + ui.available_height())], Stroke::new(1.0, BORDER_SOFT));
                ui.add_space(14.0);

                // Show name
                ui.label(RichText::new("Show").size(11.0).color(FG_MUTED));
                ui.label(RichText::new("tour-2026-mainstage").size(12.0).color(FG).strong());
                widgets::status_dot(ui, widgets::DotState::Live);
                ui.label(RichText::new("SAVED 12s ago").size(9.0).monospace().color(FG_MUTED));
                ui.add_space(14.0);
                ui.painter().line_segment([Pos2::new(ui.cursor().min.x, ui.cursor().min.y), Pos2::new(ui.cursor().min.x, ui.cursor().min.y + ui.available_height())], Stroke::new(1.0, BORDER_SOFT));
                ui.add_space(8.0);

                // Mode tabs
                let modes = ["Setup", "Patch", "Program", "Run"];
                for (i, m) in modes.iter().enumerate() {
                    let active = i == 2; // Program default
                    let btn = egui::Button::new(mode_tab(*m, active))
                        .fill(if active { BG_PANEL } else { Color32::TRANSPARENT })
                        .stroke(if active { Stroke::new(1.0, BORDER) } else { Stroke::NONE })
                        .corner_radius(3.0)
                        .min_size(Vec2::new(0.0, 26.0));
                    ui.add(btn);
                    ui.add_space(2.0);
                }

                ui.add_space((ui.available_width() - 280.0).max(0.0)); // spacer

                // Protocol pills
                widgets::pill(ui, "Art-Net", Some(widgets::DotState::Live));
                ui.add_space(6.0);
                widgets::pill(ui, "sACN", Some(widgets::DotState::Live));
                ui.add_space(6.0);
                widgets::pill(ui, "USB", Some(widgets::DotState::Warn));
                ui.add_space(6.0);
                widgets::pill(ui, "MIDI", Some(widgets::DotState::Idle));
                ui.add_space(6.0);
                widgets::pill(ui, "OSC", Some(widgets::DotState::Live));
                ui.add_space(14.0);

                // FPS / CPU (placeholder)
                ui.label(RichText::new("FPS 60.0").size(10.0).monospace().color(FG_MUTED));
                ui.add_space(10.0);
                ui.label(RichText::new("CPU 14%").size(10.0).monospace().color(FG_MUTED));
                ui.add_space(12.0);

                // Settings gear
                if ui.add_sized([24.0, 24.0], egui::Button::new("⚙").fill(Color32::TRANSPARENT).stroke(Stroke::NONE)).clicked() {
                    // TODO: settings
                }
            });
        });

    // ── Status bar ────────────────────────────────────────────────────────────
    if layout.show_status_bar {
        egui::TopBottomPanel::bottom("status_bar")
            .exact_height(22.0)
            .frame(egui::Frame::new().fill(BG_CHROME).inner_margin(egui::Margin::same(0)))
            .show(egui_ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.set_min_size(Vec2::new(ui.available_width(), 22.0));
                    ui.add_space(12.0);
                    ui.label(status_bar_text(format!("{} selected", patch_sel.selected_ids.len()).replace("0 selected", "— selected")));
                    ui.label(status_bar_text("·"));
                    let count = patch.0.len();
                    ui.label(status_bar_text(format!("{} patched", count)));
                    ui.label(status_bar_text("·"));
                    ui.label(status_bar_text("U1 81/512"));
                    ui.label(status_bar_text("·"));
                    ui.label(status_bar_text("U2 65/512"));
                    ui.add_space((ui.available_width() - 200.0).max(0.0));
                    ui.label(status_bar_text("arena-mainstage.glb"));
                    ui.label(status_bar_text("·"));
                    ui.label(RichText::new("● record armed").size(10.0).monospace().color(RX));
                    ui.label(status_bar_text("·"));
                    ui.label(status_bar_text("BPM 128.0"));
                });
            });
    }

    // ── Left rail (Programmer) ────────────────────────────────────────────────
    if !layout.detached.contains(&PanelKind::Programmer) {
        egui::SidePanel::left("left_rail")
            .exact_width(300.0)
            .frame(egui::Frame::new().fill(BG_CHROME).inner_margin(egui::Margin::same(0)))
            .show(egui_ctx, |ui| {
                // Rail header
                ui.horizontal(|ui| {
                    ui.set_min_size(Vec2::new(ui.available_width(), 28.0));
                    ui.add_space(10.0);
                    ui.label(RichText::new("Programmer").size(10.0).strong().color(FG_SECONDARY));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if widgets::icon_btn_detach(ui).on_hover_text("Detach").clicked() {
                            layout.detached.insert(PanelKind::Programmer);
                        }
                        let is_min = layout.minimized.contains(&PanelKind::Programmer);
                        if widgets::icon_btn_minimize(ui).on_hover_text(if is_min { "Restore" } else { "Minimize" }).clicked() {
                            if is_min { layout.minimized.remove(&PanelKind::Programmer); } else { layout.minimized.insert(PanelKind::Programmer); }
                        }
                    });
                });
                ui.painter().line_segment([Pos2::new(ui.min_rect().min.x, ui.cursor().min.y), Pos2::new(ui.min_rect().max.x, ui.cursor().min.y)], Stroke::new(1.0, BORDER));

                if !layout.minimized.contains(&PanelKind::Programmer) {
                    ui.add_space(8.0);
                    programmer::programmer_panel_docked(ui, &mut prog, &patch_sel, &patch);
                }
            });
    }

    // ── Right rail (DMX I/O) ──────────────────────────────────────────────────
    if !layout.detached.contains(&PanelKind::Io) {
        egui::SidePanel::right("right_rail")
            .exact_width(320.0)
            .frame(egui::Frame::new().fill(BG_CHROME).inner_margin(egui::Margin::same(0)))
            .show(egui_ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.set_min_size(Vec2::new(ui.available_width(), 28.0));
                    ui.add_space(10.0);
                    ui.label(RichText::new("DMX I/O").size(10.0).strong().color(FG_SECONDARY));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if widgets::icon_btn_detach(ui).on_hover_text("Detach").clicked() {
                            layout.detached.insert(PanelKind::Io);
                        }
                        let is_min = layout.minimized.contains(&PanelKind::Io);
                        if widgets::icon_btn_minimize(ui).on_hover_text(if is_min { "Restore" } else { "Minimize" }).clicked() {
                            if is_min { layout.minimized.remove(&PanelKind::Io); } else { layout.minimized.insert(PanelKind::Io); }
                        }
                    });
                });
                ui.painter().line_segment([Pos2::new(ui.min_rect().min.x, ui.cursor().min.y), Pos2::new(ui.min_rect().max.x, ui.cursor().min.y)], Stroke::new(1.0, BORDER));

                if !layout.minimized.contains(&PanelKind::Io) {
                    ui.add_space(8.0);
                    io_panel::io_panel_docked(ui, &mut io_cfg, &mut io_state);
                }
            });
    }

    // ── Central panel (viewports + bottom strip) ──────────────────────────────
    egui::CentralPanel::default()
        .frame(egui::Frame::new().fill(BG_APP))
        .show(egui_ctx, |ui| {
            let full_rect = ui.available_rect_before_wrap();
            let bottom_height = 248.0f32;
            let viewport_rect = Rect::from_min_max(
                full_rect.min,
                Pos2::new(full_rect.max.x, full_rect.max.y - bottom_height),
            );
            let bottom_rect = Rect::from_min_max(
                Pos2::new(full_rect.min.x, full_rect.max.y - bottom_height),
                full_rect.max,
            );

            // Viewport region (rendered as layout placeholders — actual 3D is underneath)
            ui.allocate_new_ui(egui::UiBuilder::new().max_rect(viewport_rect), |ui| {
                let avail = ui.available_size();
                let split_x = avail.x * 0.75;
                let split_y = avail.y * 0.5;

                // FOH viewport background
                let foh_rect = Rect::from_min_size(full_rect.min, Vec2::new(split_x, avail.y));
                ui.painter().rect_filled(foh_rect, 0.0, BG_APP);
                ui.painter().text(
                    Pos2::new(foh_rect.min.x + 12.0, foh_rect.min.y + 10.0),
                    egui::Align2::LEFT_TOP,
                    "FOH",
                    TextStyle::Body.resolve(ui.style()),
                    ACCENT,
                );
                ui.painter().text(
                    Pos2::new(foh_rect.min.x + 12.0 + 36.0, foh_rect.min.y + 10.0),
                    egui::Align2::LEFT_TOP,
                    "35mm · persp",
                    TextStyle::Body.resolve(ui.style()),
                    FG_MUTED,
                );

                // Viewport toolbar
                let toolbar_rect = Rect::from_min_size(
                    Pos2::new(foh_rect.max.x - 140.0, foh_rect.min.y + 10.0),
                    Vec2::new(128.0, 24.0),
                );
                ui.painter().rect_filled(toolbar_rect, 3.0, Color32::from_rgba_premultiplied(13, 15, 16, 217));
                ui.painter().rect_stroke(toolbar_rect, 3.0, Stroke::new(1.0, BORDER_SOFT), StrokeKind::Middle);

                // Hint
                ui.painter().text(
                    Pos2::new(foh_rect.max.x - 12.0, foh_rect.max.y - 10.0),
                    egui::Align2::RIGHT_BOTTOM,
                    "SHIFT-drag orbit · scroll zoom",
                    TextStyle::Body.resolve(ui.style()),
                    FG_FAINT,
                );

                // TOP viewport
                let top_rect = Rect::from_min_size(
                    Pos2::new(full_rect.min.x + split_x, full_rect.min.y),
                    Vec2::new(avail.x - split_x, split_y),
                );
                ui.painter().rect_filled(top_rect, 0.0, BG_APP);
                ui.painter().line_segment(
                    [top_rect.min, Pos2::new(top_rect.min.x, top_rect.max.y)],
                    Stroke::new(1.0, BORDER),
                );
                ui.painter().text(
                    Pos2::new(top_rect.min.x + 12.0, top_rect.min.y + 10.0),
                    egui::Align2::LEFT_TOP,
                    "TOP",
                    TextStyle::Body.resolve(ui.style()),
                    ACCENT,
                );
                ui.painter().text(
                    Pos2::new(top_rect.min.x + 12.0 + 32.0, top_rect.min.y + 10.0),
                    egui::Align2::LEFT_TOP,
                    "ortho",
                    TextStyle::Body.resolve(ui.style()),
                    FG_MUTED,
                );

                // SIDE viewport
                let side_rect = Rect::from_min_size(
                    Pos2::new(full_rect.min.x + split_x, full_rect.min.y + split_y),
                    Vec2::new(avail.x - split_x, avail.y - split_y),
                );
                ui.painter().rect_filled(side_rect, 0.0, BG_APP);
                ui.painter().line_segment(
                    [side_rect.min, Pos2::new(side_rect.max.x, side_rect.min.y)],
                    Stroke::new(1.0, BORDER),
                );
                ui.painter().line_segment(
                    [Pos2::new(side_rect.min.x, side_rect.min.y), Pos2::new(side_rect.min.x, side_rect.max.y)],
                    Stroke::new(1.0, BORDER),
                );
                ui.painter().text(
                    Pos2::new(side_rect.min.x + 12.0, side_rect.min.y + 10.0),
                    egui::Align2::LEFT_TOP,
                    "SIDE",
                    TextStyle::Body.resolve(ui.style()),
                    ACCENT,
                );
                ui.painter().text(
                    Pos2::new(side_rect.min.x + 12.0 + 36.0, side_rect.min.y + 10.0),
                    egui::Align2::LEFT_TOP,
                    "ortho",
                    TextStyle::Body.resolve(ui.style()),
                    FG_MUTED,
                );
            });

            // Bottom strip: Patch + Library
            ui.allocate_new_ui(egui::UiBuilder::new().max_rect(bottom_rect), |ui| {
                ui.painter().line_segment([Pos2::new(bottom_rect.min.x, bottom_rect.min.y), Pos2::new(bottom_rect.max.x, bottom_rect.min.y)], Stroke::new(1.0, BORDER));
                let avail = ui.available_size();
                let patch_width = avail.x * 1.4 / 2.4;

                // Patch panel
                let patch_rect = Rect::from_min_size(bottom_rect.min, Vec2::new(patch_width, avail.y));
                ui.allocate_new_ui(egui::UiBuilder::new().max_rect(patch_rect), |ui| {
                    ui.horizontal(|ui| {
                        ui.set_min_size(Vec2::new(ui.available_width(), 28.0));
                        ui.add_space(10.0);
                        ui.label(RichText::new("Patch").size(10.0).strong().color(FG_SECONDARY));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if widgets::icon_btn_detach(ui).on_hover_text("Detach").clicked() {
                                layout.detached.insert(PanelKind::Patch);
                            }
                            let is_min = layout.minimized.contains(&PanelKind::Patch);
                            if widgets::icon_btn_minimize(ui).on_hover_text(if is_min { "Restore" } else { "Minimize" }).clicked() {
                                if is_min { layout.minimized.remove(&PanelKind::Patch); } else { layout.minimized.insert(PanelKind::Patch); }
                            }
                        });
                    });
                    ui.painter().line_segment([Pos2::new(patch_rect.min.x, ui.cursor().min.y), Pos2::new(patch_rect.max.x, ui.cursor().min.y)], Stroke::new(1.0, BORDER));
                    if !layout.minimized.contains(&PanelKind::Patch) {
                        ui.add_space(8.0);
                        patch::patch_panel_docked(
                            ui,
                            &mut patch,
                            &library,
                            &mut patch_edit,
                            &mut patch_sel,
                            &mut commands,
                        );
                    }
                });

                // Library panel
                let lib_rect = Rect::from_min_size(
                    Pos2::new(bottom_rect.min.x + patch_width, bottom_rect.min.y),
                    Vec2::new(avail.x - patch_width, avail.y),
                );
                ui.allocate_new_ui(egui::UiBuilder::new().max_rect(lib_rect), |ui| {
                    ui.painter().line_segment(
                        [Pos2::new(lib_rect.min.x, lib_rect.min.y), Pos2::new(lib_rect.min.x, lib_rect.max.y)],
                        Stroke::new(1.0, BORDER),
                    );
                    ui.horizontal(|ui| {
                        ui.set_min_size(Vec2::new(ui.available_width(), 28.0));
                        ui.add_space(10.0);
                        ui.label(RichText::new("Library").size(10.0).strong().color(FG_SECONDARY));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if widgets::icon_btn_detach(ui).on_hover_text("Detach").clicked() {
                                layout.detached.insert(PanelKind::Library);
                            }
                            let is_min = layout.minimized.contains(&PanelKind::Library);
                            if widgets::icon_btn_minimize(ui).on_hover_text(if is_min { "Restore" } else { "Minimize" }).clicked() {
                                if is_min { layout.minimized.remove(&PanelKind::Library); } else { layout.minimized.insert(PanelKind::Library); }
                            }
                        });
                    });
                    ui.painter().line_segment([Pos2::new(lib_rect.min.x, ui.cursor().min.y), Pos2::new(lib_rect.max.x, ui.cursor().min.y)], Stroke::new(1.0, BORDER));
                    if !layout.minimized.contains(&PanelKind::Library) {
                        ui.add_space(8.0);
                        library::library_panel_docked(
                            ui,
                            &mut library,
                            &mut patch,
                            &mut venue_state,
                            &mut commands,
                        );
                    }
                });
            });
        });

    // ── Detached floating windows ─────────────────────────────────────────────
    let float_frame = egui::Frame::window(&egui_ctx.style())
        .fill(BG_PANEL)
        .stroke(Stroke::new(1.0, BORDER))
        .shadow(egui::epaint::Shadow {
            offset: [0, 8],
            blur: 24,
            spread: 2,
            color: Color32::from_black_alpha(102),
        });

    if layout.detached.contains(&PanelKind::Programmer) {
        egui::Window::new("Programmer")
            .default_pos([100.0, 100.0])
            .default_width(360.0)
            .resizable(true)
            .frame(float_frame)
            .show(egui_ctx, |ui| {
                programmer::programmer_panel_docked(ui, &mut prog, &patch_sel, &patch);
                if ui.button("Re-dock").clicked() {
                    layout.detached.remove(&PanelKind::Programmer);
                }
            });
    }
    if layout.detached.contains(&PanelKind::Patch) {
        egui::Window::new("Patch")
            .default_pos([400.0, 100.0])
            .default_width(580.0)
            .default_height(400.0)
            .resizable(true)
            .frame(float_frame)
            .show(egui_ctx, |ui| {
                patch::patch_panel_docked(ui, &mut patch, &library, &mut patch_edit, &mut patch_sel, &mut commands);
                if ui.button("Re-dock").clicked() {
                    layout.detached.remove(&PanelKind::Patch);
                }
            });
    }
    if layout.detached.contains(&PanelKind::Library) {
        egui::Window::new("Library")
            .default_pos([700.0, 100.0])
            .default_width(420.0)
            .default_height(400.0)
            .resizable(true)
            .frame(float_frame)
            .show(egui_ctx, |ui| {
                library::library_panel_docked(ui, &mut library, &mut patch, &mut venue_state, &mut commands);
                if ui.button("Re-dock").clicked() {
                    layout.detached.remove(&PanelKind::Library);
                }
            });
    }
    if layout.detached.contains(&PanelKind::Io) {
        egui::Window::new("DMX I/O")
            .default_pos([1000.0, 100.0])
            .default_width(360.0)
            .resizable(true)
            .frame(float_frame)
            .show(egui_ctx, |ui| {
                io_panel::io_panel_docked(ui, &mut io_cfg, &mut io_state);
                if ui.button("Re-dock").clicked() {
                    layout.detached.remove(&PanelKind::Io);
                }
            });
    }
}
