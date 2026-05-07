use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts};
use crate::PatchRes;

pub fn patch_panel(mut ctx: EguiContexts, patch: Res<PatchRes>) {
    egui::Window::new("Patch")
        .default_pos([290.0, 10.0])
        .default_width(520.0)
        .default_height(260.0)
        .resizable(true)
        .show(&ctx.ctx_mut().expect("egui context"), |ui| {
            let count = patch.0.len();

            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    egui::Grid::new("patch_grid")
                        .num_columns(6)
                        .striped(true)
                        .spacing([10.0, 3.0])
                        .show(ui, |ui| {
                            // Header
                            ui.strong("#");
                            ui.strong("Name");
                            ui.strong("Fixture Type");
                            ui.strong("Mode");
                            ui.strong("Univ");
                            ui.strong("Ch");
                            ui.end_row();

                            // Rows — collect into a sorted vec so order is stable
                            let mut fixtures: Vec<_> = patch.0.fixtures().collect();
                            fixtures.sort_by_key(|f| f.id.0);

                            for f in fixtures {
                                ui.monospace(
                                    egui::RichText::new(format!("{:>3}", f.id.0 + 1))
                                        .color(egui::Color32::from_rgb(150, 200, 255)),
                                );
                                ui.label(&f.name);
                                ui.label(truncate(&f.fixture_type_id, 22));
                                ui.label(truncate(&f.dmx_mode, 12));
                                ui.monospace(format!("{}", f.address.universe));
                                ui.monospace(format!("{}", f.address.channel));
                                ui.end_row();
                            }
                        });
                });

            ui.separator();
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new(format!("{} fixture{} patched", count, if count == 1 { "" } else { "s" }))
                        .color(egui::Color32::GRAY)
                        .small(),
                );
            });
        });
}

fn truncate(s: &str, max: usize) -> &str {
    match s.char_indices().nth(max) {
        Some((idx, _)) => &s[..idx],
        None => s,
    }
}
