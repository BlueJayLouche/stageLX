pub mod cue;
pub mod io_panel;
pub mod library;
pub mod patch;
pub mod programmer;
pub mod theme;
pub mod widgets;

use bevy::prelude::*;
use bevy_egui::{EguiPlugin, EguiPrimaryContextPass, egui};
use bevy_egui::egui::{Color32, Pos2, Rect, RichText, Stroke, StrokeKind, Vec2};
use std::collections::HashSet;

pub use stagelx_patch::{DespawnFixtureEvent, PatchEditState, PatchRes, SpawnFixtureEvent};
pub use stagelx_show::{
    BackCueEvent, CuePlayhead, CueStack, DeleteCueEvent, FixtureLibraryRes,
    GoCueEvent, LoadMvrStructureEvent, LoadShowEvent, LoadVenueEvent,
    MvrStructureObject, PerfDiagnosticsRes, Programmer, RecordCueEvent,
    SaveShowEvent, VenueLoadState,
};
use stagelx_io::artnet::ArtNetNodeTable;
use stagelx_io::config::{ArtNetConfig, MidiConfig, OscConfig, SacnConfig, UsbConfig};
use stagelx_io::midi::MidiTarget;
use stagelx_io::stats::{ArtNetStats, MidiStats, OscStats, SacnStats, UsbStats};
use stagelx_show::ProtocolStatus;

#[derive(bevy::ecs::system::SystemParam)]
struct IoStats<'w> {
    artnet: Res<'w, ArtNetStats>,
    sacn: Res<'w, SacnStats>,
    usb: Res<'w, UsbStats>,
    midi: Res<'w, MidiStats>,
    osc: Res<'w, OscStats>,
}

#[derive(bevy::ecs::system::SystemParam)]
struct UiMutableState<'w> {
    layout: ResMut<'w, UiLayoutState>,
    patch_sel: ResMut<'w, PatchSelection>,
    programmer: ResMut<'w, Programmer>,
    patch: ResMut<'w, PatchRes>,
    patch_edit: ResMut<'w, PatchEditState>,
    library: ResMut<'w, FixtureLibraryRes>,
    venue: ResMut<'w, VenueLoadState>,
    app_mode: ResMut<'w, AppMode>,
    fonts: ResMut<'w, FontsInstalled>,
}
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
    Cue,
}

// ═══════════════════════════════════════════════════════════════════════════════
// App mode
// ═══════════════════════════════════════════════════════════════════════════════

#[derive(Resource, Default, PartialEq, Eq, Clone, Copy)]
pub enum AppMode {
    Setup,
    Patch,
    #[default]
    Program,
    Run,
}

impl AppMode {
    pub fn as_str(self) -> &'static str {
        match self {
            AppMode::Setup => "Setup",
            AppMode::Patch => "Patch",
            AppMode::Program => "Program",
            AppMode::Run => "Run",
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Stub resources for placeholder data
// ═══════════════════════════════════════════════════════════════════════════════

#[derive(Resource)]
pub struct ShowMeta {
    pub name: String,
    pub last_saved: std::time::Instant,
}

impl Default for ShowMeta {
    fn default() -> Self {
        Self {
            name: "tour-2026-mainstage".into(),
            last_saved: std::time::Instant::now(),
        }
    }
}

/// Tracks an async file dialog for Open Show (Ctrl+O).
#[derive(Resource, Default)]
struct OpenShowDialog {
    pending: Option<std::thread::JoinHandle<Option<String>>>,
}

impl OpenShowDialog {
    fn spawn() -> Self {
        let handle = std::thread::spawn(|| {
            rfd::FileDialog::new()
                .add_filter("Show", &["slx"])
                .pick_file()
                .map(|p| p.to_string_lossy().to_string())
        });
        Self { pending: Some(handle) }
    }

    fn try_take(&mut self) -> Option<String> {
        let handle = self.pending.take()?;
        if handle.is_finished() {
            handle.join().ok().flatten()
        } else {
            self.pending = Some(handle);
            None
        }
    }
}

#[derive(Resource, Default)]
struct FontsInstalled(bool);

// ═══════════════════════════════════════════════════════════════════════════════
// Existing resources
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
            .init_resource::<CueStack>()
            .init_resource::<CuePlayhead>()
            .init_resource::<stagelx_show::CaptureMode>()
            // IO config/stats are initialised by stagelx_io::IoPlugin
            .init_resource::<VenueLoadState>()
            .init_resource::<UiLayoutState>()
            .init_resource::<PatchSelection>()
            .init_resource::<IoPanelState>()
            .init_resource::<AppMode>()
            .init_resource::<ShowMeta>()
            .init_resource::<OpenShowDialog>()
            .init_resource::<FontsInstalled>()
            .add_systems(EguiPrimaryContextPass, (io_panel_system, ui_root_system).chain())
            .add_systems(Update, sync_midi_target);
    }
}

#[allow(clippy::too_many_arguments)]
fn io_panel_system(
    mut ctx: bevy_egui::EguiContexts,
    mut layout: ResMut<UiLayoutState>,
    mut io_state: ResMut<IoPanelState>,
    mut artnet_cfg: ResMut<ArtNetConfig>,
    artnet_stats: Res<ArtNetStats>,
    artnet_nodes: Res<ArtNetNodeTable>,
    mut sacn_cfg: ResMut<SacnConfig>,
    sacn_stats: Res<SacnStats>,
    mut usb_cfg: ResMut<UsbConfig>,
    usb_stats: Res<UsbStats>,
    mut midi_cfg: ResMut<MidiConfig>,
    midi_stats: Res<MidiStats>,
    midi_target: Res<MidiTarget>,
    mut osc_cfg: ResMut<OscConfig>,
    osc_stats: Res<OscStats>,
    app_mode: Res<AppMode>,
) {
    let show_io = matches!(*app_mode, AppMode::Program | AppMode::Setup);
    let egui_ctx = ctx.ctx_mut().expect("egui context");

    let float_frame = egui::Frame::window(&egui_ctx.style())
        .fill(BG_PANEL)
        .stroke(Stroke::new(1.0, BORDER))
        .shadow(egui::epaint::Shadow {
            offset: [0, 8],
            blur: 24,
            spread: 2,
            color: Color32::from_black_alpha(102),
        });

    // ── Right rail (DMX I/O) ──────────────────────────────────────────────────
    if show_io && !layout.detached.contains(&PanelKind::Io) {
        egui::SidePanel::right("right_rail")
            .exact_width(320.0)
            .frame(egui::Frame::new().fill(BG_CHROME).inner_margin(egui::Margin::same(0)))
            .show(egui_ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.set_min_size(Vec2::new(0.0, 28.0));
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
                let p = ui.available_rect_before_wrap();
                ui.painter().line_segment(
                    [Pos2::new(p.min.x, p.min.y), Pos2::new(p.max.x, p.min.y)],
                    Stroke::new(1.0, BORDER),
                );

                if !layout.minimized.contains(&PanelKind::Io) {
                    io_panel::io_panel_docked(
                        ui,
                        &mut artnet_cfg,
                        &artnet_stats,
                        &artnet_nodes,
                        &mut sacn_cfg,
                        &sacn_stats,
                        &mut usb_cfg,
                        &usb_stats,
                        &mut midi_cfg,
                        &midi_stats,
                        &midi_target,
                        &mut osc_cfg,
                        &osc_stats,
                        &mut io_state,
                    );
                }
            });
    }

    // ── Detached floating IO window ───────────────────────────────────────────
    if show_io && layout.detached.contains(&PanelKind::Io) {
        egui::Window::new("DMX I/O")
            .default_pos([1000.0, 100.0])
            .default_width(360.0)
            .resizable(true)
            .frame(float_frame)
            .show(egui_ctx, |ui| {
                io_panel::io_panel_docked(
                    ui,
                    &mut artnet_cfg,
                    &artnet_stats,
                    &artnet_nodes,
                    &mut sacn_cfg,
                    &sacn_stats,
                    &mut usb_cfg,
                    &usb_stats,
                    &mut midi_cfg,
                    &midi_stats,
                    &midi_target,
                    &mut osc_cfg,
                    &osc_stats,
                    &mut io_state,
                );
                if ui.button("Re-dock").clicked() {
                    layout.detached.remove(&PanelKind::Io);
                }
            });
    }
}

fn sync_midi_target(
    cfg: Res<MidiConfig>,
    patch_sel: Res<PatchSelection>,
    mut midi_target: ResMut<MidiTarget>,
) {
    if cfg.target_selected_fixtures {
        if midi_target.fixture_ids != patch_sel.selected_ids {
            midi_target.fixture_ids.clone_from(&patch_sel.selected_ids);
        }
    } else if !midi_target.fixture_ids.is_empty() {
        midi_target.fixture_ids.clear();
    }
}

fn install_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    fonts.font_data.insert(
        "plex_sans".into(),
        egui::FontData::from_static(include_bytes!("../assets/IBMPlexSans-Regular.ttf")).into(),
    );
    fonts.font_data.insert(
        "plex_mono".into(),
        egui::FontData::from_static(include_bytes!("../assets/IBMPlexMono-Regular.ttf")).into(),
    );
    fonts.families
        .entry(egui::FontFamily::Proportional)
        .or_default()
        .insert(0, "plex_sans".into());
    fonts.families
        .entry(egui::FontFamily::Monospace)
        .or_default()
        .insert(0, "plex_mono".into());
    ctx.set_fonts(fonts);
}

// ═══════════════════════════════════════════════════════════════════════════════
// Root UI system — layout shell
// ═══════════════════════════════════════════════════════════════════════════════

#[allow(clippy::too_many_arguments)]
fn ui_root_system(
    mut ctx: bevy_egui::EguiContexts,
    mut state: UiMutableState,
    mut stack: ResMut<CueStack>,
    playhead: Res<CuePlayhead>,
    mut capture_mode: ResMut<stagelx_show::CaptureMode>,
    show_meta: Res<ShowMeta>,
    perf: Res<PerfDiagnosticsRes>,
    io_stats: IoStats,
    mut open_dialog: ResMut<OpenShowDialog>,
    mut commands: Commands,
    windows: Query<&Window>,
) {
    let layout = &mut *state.layout;
    let patch_sel = &mut *state.patch_sel;
    let prog = &mut *state.programmer;
    let patch = &mut *state.patch;
    let patch_edit = &mut *state.patch_edit;
    let library = &mut *state.library;
    let venue_state = &mut *state.venue;
    let app_mode = &mut *state.app_mode;
    let fonts_installed = &mut *state.fonts;

    // Mode-based panel visibility.
    let show_left_rail = matches!(*app_mode, AppMode::Program | AppMode::Run);
    let show_bottom_strip = matches!(*app_mode, AppMode::Program | AppMode::Patch | AppMode::Setup);
    let run_mode = *app_mode == AppMode::Run;
    let Ok(window) = windows.single() else { return };
    let scale_factor = window.scale_factor();
    let egui_ctx = ctx.ctx_mut().expect("egui context");
    egui_ctx.set_pixels_per_point(scale_factor);

    if !fonts_installed.0 {
        install_fonts(egui_ctx);
        fonts_installed.0 = true;
    }

    // ── Global keyboard shortcuts ─────────────────────────────────────────────
    if egui_ctx.input(|i| i.key_pressed(egui::Key::S) && i.modifiers.command) {
        commands.trigger(SaveShowEvent);
    }
    if egui_ctx.input(|i| i.key_pressed(egui::Key::O) && i.modifiers.command) {
        *open_dialog = OpenShowDialog::spawn();
    }
    if let Some(path) = open_dialog.try_take() {
        commands.trigger(LoadShowEvent { path });
    }

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
    // Global spacing override (Tier 1 #2)
    style.spacing.item_spacing = Vec2::new(6.0, 4.0);
    style.spacing.button_padding = Vec2::new(8.0, 4.0);
    style.spacing.interact_size = Vec2::new(0.0, 24.0);
    style.spacing.icon_width = 12.0;
    style.spacing.menu_margin = egui::Margin::same(6);
    style.spacing.window_margin = egui::Margin::same(0);
    egui_ctx.set_style(style);

    // ── Top bar ───────────────────────────────────────────────────────────────
    egui::TopBottomPanel::top("top_bar")
        .exact_height(36.0)
        .frame(egui::Frame::new().fill(BG_CHROME).inner_margin(egui::Margin::same(0)))
        .show(egui_ctx, |ui| {
            ui.horizontal(|ui| {
                ui.set_min_size(Vec2::new(ui.available_width(), 36.0));
                ui.add_space(10.0);

                // Wordmark as single LayoutJob (Tier 1 #5)
                let mut job = egui::text::LayoutJob::default();
                job.append("stage", 0.0, egui::TextFormat {
                    font_id: font_wordmark(),
                    color: FG,
                    ..Default::default()
                });
                job.append("LX", 0.0, egui::TextFormat {
                    font_id: font_wordmark(),
                    color: ACCENT,
                    ..Default::default()
                });
                ui.label(job);

                ui.label(
                    RichText::new("0.1.0")
                        .size(9.0)
                        .monospace()
                        .color(FG_FAINT),
                );
                ui.add_space(14.0);

                // Divider (Tier 1 #6)
                widgets::vertical_divider(ui, 24.0);
                ui.add_space(14.0);

                // Show name
                ui.label(RichText::new("Show").size(11.0).color(FG_MUTED));
                ui.label(RichText::new(&show_meta.name).size(12.0).color(FG).strong());
                widgets::status_dot(ui, widgets::DotState::Live);
                let saved_ago = format!("SAVED {}s ago", show_meta.last_saved.elapsed().as_secs());
                ui.label(RichText::new(saved_ago).size(9.0).monospace().color(FG_MUTED));
                ui.add_space(14.0);
                widgets::vertical_divider(ui, 24.0);
                ui.add_space(8.0);

                // Mode tabs (Tier 2 #17)
                let modes = [AppMode::Setup, AppMode::Patch, AppMode::Program, AppMode::Run];
                for m in modes {
                    let active = *app_mode == m;
                    let label = m.as_str();
                    let btn = egui::Button::new(mode_tab(label, active))
                        .fill(if active { BG_PANEL } else { Color32::TRANSPARENT })
                        .stroke(if active { Stroke::new(1.0, BORDER) } else { Stroke::NONE })
                        .corner_radius(3.0)
                        .min_size(Vec2::new(0.0, 26.0));
                    if ui.add(btn).clicked() {
                        *app_mode = m;
                    }
                    ui.add_space(2.0);
                }

                // Right-aligned section — placed from right edge inward
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    // Settings gear — toggles status bar visibility
                    if ui.add_sized([24.0, 24.0], egui::Button::new("⚙").fill(Color32::TRANSPARENT).stroke(Stroke::NONE)).clicked() {
                        layout.show_status_bar = !layout.show_status_bar;
                    }
                    ui.add_space(12.0);

                    // FPS / CPU — computed from perf diagnostics
                    let fps = if perf.frame_time_ms > 0.0 {
                        1000.0 / perf.frame_time_ms
                    } else {
                        0.0
                    };
                    let cpu_ms = perf.beam_articulate_ms
                        + perf.beam_lod_eval_ms
                        + perf.beam_lod_apply_ms
                        + perf.beam_sort_ms
                        + perf.dmx_tick_last_ms;
                    let cpu_pct = (cpu_ms / 16.667).clamp(0.0, 99.9);
                    ui.label(RichText::new(format!("CPU {:.0}%", cpu_pct)).size(10.0).monospace().color(FG_MUTED));
                    ui.add_space(10.0);
                    ui.label(RichText::new(format!("FPS {:.1}", fps)).size(10.0).monospace().color(FG_MUTED));
                    ui.add_space(14.0);

                    // Protocol pills — linked to real I/O stats
                    let artnet_dot = status_to_dot(io_stats.artnet.status);
                    let sacn_dot   = status_to_dot(io_stats.sacn.status);
                    let usb_dot    = status_to_dot(io_stats.usb.status);
                    let midi_dot   = status_to_dot(io_stats.midi.status);
                    let osc_dot    = status_to_dot(io_stats.osc.status);

                    widgets::pill(ui, "OSC", Some(osc_dot));
                    ui.add_space(6.0);
                    widgets::pill(ui, "MIDI", Some(midi_dot));
                    ui.add_space(6.0);
                    widgets::pill(ui, "USB", Some(usb_dot));
                    ui.add_space(6.0);
                    widgets::pill(ui, "sACN", Some(sacn_dot));
                    ui.add_space(6.0);
                    widgets::pill(ui, "Art-Net", Some(artnet_dot));
                });
            });
        });

    // ── Status bar ────────────────────────────────────────────────────────────
    if layout.show_status_bar {
        // Compute live metrics.
        let fps = if perf.frame_time_ms > 0.0 {
            1000.0 / perf.frame_time_ms
        } else {
            0.0
        };
        let cpu_ms = perf.beam_articulate_ms
            + perf.beam_lod_eval_ms
            + perf.beam_lod_apply_ms
            + perf.beam_sort_ms
            + perf.dmx_tick_last_ms;
        let cpu_pct = (cpu_ms / 16.667).clamp(0.0, 99.9);

        // Universe channel counts.
        let mut universe_max_ch: std::collections::HashMap<u16, u16> = std::collections::HashMap::new();
        for inst in patch.0.fixtures() {
            let addr = inst.address.channel;
            let u = inst.address.universe;
            let max = universe_max_ch.entry(u).or_insert(0);
            *max = (*max).max(addr + 8); // rough: 8 ch per fixture
        }
        let u1_text = universe_max_ch.get(&1).map(|m| format!("U1 {}/512", m.min(&512))).unwrap_or_else(|| "U1 —".into());
        let u2_text = universe_max_ch.get(&2).map(|m| format!("U2 {}/512", m.min(&512))).unwrap_or_else(|| "U2 —".into());

        let venue_name = std::path::Path::new(&venue_state.import_path)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("—");

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
                    ui.label(status_bar_text(u1_text));
                    ui.label(status_bar_text("·"));
                    ui.label(status_bar_text(u2_text));

                    // Right-aligned section
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(status_bar_text(format!("{:.1} FPS", fps)));
                        ui.label(status_bar_text("·"));
                        ui.label(status_bar_text(format!("CPU {:.0}%", cpu_pct)));
                        ui.label(status_bar_text("·"));
                        ui.label(RichText::new("● record armed").size(10.0).monospace().color(RX));
                        ui.label(status_bar_text("·"));
                        ui.label(status_bar_text(venue_name));
                    });
                });
            });
    }

    // ── Left rail (Cue + Programmer) ──────────────────────────────────────────
    if show_left_rail && (!layout.detached.contains(&PanelKind::Programmer) || !layout.detached.contains(&PanelKind::Cue)) {
        egui::SidePanel::left("left_rail")
            .exact_width(300.0)
            .frame(egui::Frame::new().fill(BG_CHROME).inner_margin(egui::Margin::same(0)))
            .show(egui_ctx, |ui| {
                // ── Cue panel ─────────────────────────────────────────────────────
                if !layout.detached.contains(&PanelKind::Cue) {
                    ui.horizontal(|ui| {
                        ui.set_min_size(Vec2::new(0.0, 28.0));
                        ui.add_space(10.0);
                        ui.label(RichText::new("Cue").size(10.0).strong().color(FG_SECONDARY));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if widgets::icon_btn_detach(ui).on_hover_text("Detach").clicked() {
                                layout.detached.insert(PanelKind::Cue);
                            }
                            let is_min = layout.minimized.contains(&PanelKind::Cue);
                            if widgets::icon_btn_minimize(ui).on_hover_text(if is_min { "Restore" } else { "Minimize" }).clicked() {
                                if is_min { layout.minimized.remove(&PanelKind::Cue); } else { layout.minimized.insert(PanelKind::Cue); }
                            }
                        });
                    });
                    let p = ui.available_rect_before_wrap();
                    ui.painter().line_segment(
                        [Pos2::new(p.min.x, p.min.y), Pos2::new(p.max.x, p.min.y)],
                        Stroke::new(1.0, BORDER),
                    );
                    if !layout.minimized.contains(&PanelKind::Cue) {
                        cue::cue_panel_docked(ui, &mut stack, &playhead, &mut capture_mode, &mut commands);
                    }
                    // Divider before Programmer
                    if !layout.detached.contains(&PanelKind::Programmer) {
                        let p = ui.available_rect_before_wrap();
                        ui.painter().line_segment(
                            [Pos2::new(p.min.x, p.min.y), Pos2::new(p.max.x, p.min.y)],
                            Stroke::new(1.0, BORDER),
                        );
                    }
                }

                // ── Programmer panel ──────────────────────────────────────────────
                if !layout.detached.contains(&PanelKind::Programmer) {
                    ui.horizontal(|ui| {
                        ui.set_min_size(Vec2::new(0.0, 28.0));
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
                    let p = ui.available_rect_before_wrap();
                    ui.painter().line_segment(
                        [Pos2::new(p.min.x, p.min.y), Pos2::new(p.max.x, p.min.y)],
                        Stroke::new(1.0, BORDER),
                    );
                    if !layout.minimized.contains(&PanelKind::Programmer) {
                        programmer::programmer_panel_docked(ui, prog, patch_sel, patch);
                    }
                }
            });
    }

    // ── Central panel (viewports + bottom strip) ──────────────────────────────
    egui::CentralPanel::default()
        .frame(egui::Frame::new().fill(Color32::TRANSPARENT))
        .show(egui_ctx, |ui| {
            let full_rect = ui.available_rect_before_wrap();
            let bottom_height = if show_bottom_strip { 248.0f32 } else { 0.0 };
            let viewport_rect = Rect::from_min_max(
                full_rect.min,
                Pos2::new(full_rect.max.x, full_rect.max.y - bottom_height),
            );
            let bottom_rect = Rect::from_min_max(
                Pos2::new(full_rect.min.x, full_rect.max.y - bottom_height),
                full_rect.max,
            );

            // Viewport region (rendered as layout placeholders — actual 3D is underneath)
            ui.scope_builder(egui::UiBuilder::new().max_rect(viewport_rect), |ui| {
                let avail = ui.available_size();
                let split_x = avail.x * 0.75;
                let split_y = avail.y * 0.5;

                // FOH viewport — labels only; 3D renders underneath
                let foh_rect = Rect::from_min_size(full_rect.min, Vec2::new(split_x, avail.y));
                ui.painter().text(
                    Pos2::new(foh_rect.min.x + 12.0, foh_rect.min.y + 10.0),
                    egui::Align2::LEFT_TOP,
                    "FOH",
                    font_eyebrow(),
                    ACCENT,
                );
                ui.painter().text(
                    Pos2::new(foh_rect.min.x + 12.0 + 36.0, foh_rect.min.y + 10.0),
                    egui::Align2::LEFT_TOP,
                    "35mm · persp",
                    font_hint(),
                    FG_MUTED,
                );

                // Viewport toolbar
                let toolbar_rect = Rect::from_min_size(
                    Pos2::new(foh_rect.max.x - 140.0, foh_rect.min.y + 10.0),
                    Vec2::new(128.0, 24.0),
                );
                ui.painter().rect_filled(toolbar_rect, 3.0, Color32::from_rgba_premultiplied(13, 15, 16, 217));
                ui.painter().rect_stroke(toolbar_rect, 3.0, Stroke::new(1.0, BORDER_SOFT), StrokeKind::Inside);

                // Hint
                ui.painter().text(
                    Pos2::new(foh_rect.max.x - 12.0, foh_rect.max.y - 10.0),
                    egui::Align2::RIGHT_BOTTOM,
                    "SHIFT-drag orbit · scroll zoom",
                    font_hint(),
                    FG_FAINT,
                );

                // Performance HUD
                draw_perf_hud(ui, &perf, foh_rect);

                // TOP viewport — labels only; 3D renders underneath
                let top_rect = Rect::from_min_size(
                    Pos2::new(full_rect.min.x + split_x, full_rect.min.y),
                    Vec2::new(avail.x - split_x, split_y),
                );
                ui.painter().line_segment(
                    [top_rect.min, Pos2::new(top_rect.min.x, top_rect.max.y)],
                    Stroke::new(1.0, BORDER),
                );
                ui.painter().text(
                    Pos2::new(top_rect.min.x + 12.0, top_rect.min.y + 10.0),
                    egui::Align2::LEFT_TOP,
                    "TOP",
                    font_eyebrow(),
                    ACCENT,
                );
                ui.painter().text(
                    Pos2::new(top_rect.min.x + 12.0 + 32.0, top_rect.min.y + 10.0),
                    egui::Align2::LEFT_TOP,
                    "ortho",
                    font_hint(),
                    FG_MUTED,
                );

                // SIDE viewport — labels only; 3D renders underneath
                let side_rect = Rect::from_min_size(
                    Pos2::new(full_rect.min.x + split_x, full_rect.min.y + split_y),
                    Vec2::new(avail.x - split_x, avail.y - split_y),
                );
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
                    font_eyebrow(),
                    ACCENT,
                );
                ui.painter().text(
                    Pos2::new(side_rect.min.x + 12.0 + 36.0, side_rect.min.y + 10.0),
                    egui::Align2::LEFT_TOP,
                    "ortho",
                    font_hint(),
                    FG_MUTED,
                );
            });

            // Bottom strip: Patch + Library
            if !show_bottom_strip {
                return;
            }
            ui.scope_builder(egui::UiBuilder::new().max_rect(bottom_rect), |ui| {
                ui.painter().rect_filled(bottom_rect, 0.0, BG_APP);
                ui.painter().line_segment([Pos2::new(bottom_rect.min.x, bottom_rect.min.y), Pos2::new(bottom_rect.max.x, bottom_rect.min.y)], Stroke::new(1.0, BORDER));
                let avail = ui.available_size();
                let patch_width = avail.x * 1.4 / 2.4;

                // Patch panel
                let patch_rect = Rect::from_min_size(bottom_rect.min, Vec2::new(patch_width, avail.y));
                ui.scope_builder(egui::UiBuilder::new().max_rect(patch_rect), |ui| {
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
                    let p = ui.available_rect_before_wrap();
                    ui.painter().line_segment(
                        [Pos2::new(p.min.x, p.min.y), Pos2::new(p.max.x, p.min.y)],
                        Stroke::new(1.0, BORDER),
                    );
                    if !layout.minimized.contains(&PanelKind::Patch) {
                        patch::patch_panel_docked(
                            ui,
                            patch,
                            library,
                            patch_edit,
                            patch_sel,
                            &mut commands,
                        );
                    }
                });

                // Library panel
                let lib_rect = Rect::from_min_size(
                    Pos2::new(bottom_rect.min.x + patch_width, bottom_rect.min.y),
                    Vec2::new(avail.x - patch_width, avail.y),
                );
                ui.scope_builder(egui::UiBuilder::new().max_rect(lib_rect), |ui| {
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
                    let p = ui.available_rect_before_wrap();
                    ui.painter().line_segment(
                        [Pos2::new(p.min.x, p.min.y), Pos2::new(p.max.x, p.min.y)],
                        Stroke::new(1.0, BORDER),
                    );
                    if !layout.minimized.contains(&PanelKind::Library) {
                        library::library_panel_docked(
                            ui,
                            library,
                            patch,
                            venue_state,
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
                programmer::programmer_panel_docked(ui, prog, patch_sel, patch);
                if ui.button("Re-dock").clicked() {
                    layout.detached.remove(&PanelKind::Programmer);
                }
            });
    }
    if !run_mode && layout.detached.contains(&PanelKind::Patch) {
        egui::Window::new("Patch")
            .default_pos([400.0, 100.0])
            .default_width(580.0)
            .default_height(400.0)
            .resizable(true)
            .frame(float_frame)
            .show(egui_ctx, |ui| {
                patch::patch_panel_docked(ui, patch, library, patch_edit, patch_sel, &mut commands);
                if ui.button("Re-dock").clicked() {
                    layout.detached.remove(&PanelKind::Patch);
                }
            });
    }
    if !run_mode && layout.detached.contains(&PanelKind::Library) {
        egui::Window::new("Library")
            .default_pos([700.0, 100.0])
            .default_width(420.0)
            .default_height(400.0)
            .resizable(true)
            .frame(float_frame)
            .show(egui_ctx, |ui| {
                library::library_panel_docked(ui, library, patch, venue_state, &mut commands);
                if ui.button("Re-dock").clicked() {
                    layout.detached.remove(&PanelKind::Library);
                }
            });
    }
    if layout.detached.contains(&PanelKind::Cue) {
        egui::Window::new("Cue")
            .default_pos([100.0, 400.0])
            .default_width(300.0)
            .default_height(250.0)
            .resizable(true)
            .frame(float_frame)
            .show(egui_ctx, |ui| {
                cue::cue_panel_docked(ui, &mut stack, &playhead, &mut capture_mode, &mut commands);
                if ui.button("Re-dock").clicked() {
                    layout.detached.remove(&PanelKind::Cue);
                }
            });
    }

}

// ═══════════════════════════════════════════════════════════════════════════════
// Performance HUD
// ═══════════════════════════════════════════════════════════════════════════════

fn draw_perf_hud(ui: &mut egui::Ui, perf: &PerfDiagnosticsRes, viewport: Rect) {
    let lines = [
        format!("Frame  {:.1} ms", perf.frame_time_ms),
        format!("DMX    {:.2} ms  σ={:.2}", perf.dmx_tick_last_ms, perf.dmx_tick_std_dev_ms),
        format!("Beams  {}  (raymarch {})", perf.beam_count, perf.beam_raymarch_count),
        format!("GPU    {:.1} MB", perf.estimated_gpu_memory_mb),
        format!("Spawn  {:.2} ms", perf.last_fixture_spawn_ms),
    ];

    let line_h = 13.0;
    let pad = 6.0;
    let w = 148.0;
    let h = lines.len() as f32 * line_h + pad * 2.0;

    let rect = Rect::from_min_size(
        Pos2::new(viewport.max.x - w - 10.0, viewport.min.y + 10.0),
        Vec2::new(w, h),
    );

    ui.painter().rect_filled(rect, 4.0, Color32::from_rgba_premultiplied(13, 15, 16, 200));
    ui.painter().rect_stroke(rect, 4.0, Stroke::new(1.0, BORDER_SOFT), StrokeKind::Inside);

    for (i, text) in lines.iter().enumerate() {
        ui.painter().text(
            Pos2::new(rect.min.x + pad, rect.min.y + pad + i as f32 * line_h),
            egui::Align2::LEFT_TOP,
            text,
            font_hint(),
            FG_MUTED,
        );
    }
}

fn status_to_dot(s: ProtocolStatus) -> widgets::DotState {
    match s {
        ProtocolStatus::Live => widgets::DotState::Live,
        ProtocolStatus::Warn => widgets::DotState::Warn,
        ProtocolStatus::Error => widgets::DotState::Error,
        ProtocolStatus::Idle => widgets::DotState::Idle,
    }
}
