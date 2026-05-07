use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts};
use crate::FixtureLibraryRes;

pub fn library_panel(mut ctx: EguiContexts, mut res: ResMut<FixtureLibraryRes>) {
    egui::Window::new("Fixture Library")
        .default_pos([10.0, 370.0])
        .default_width(370.0)
        .default_height(220.0)
        .resizable(true)
        .show(&ctx.ctx_mut().expect("egui context"), |ui| {
            // ── Loaded fixtures list ──────────────────────────────────────────
            if res.library.is_empty() {
                ui.label(
                    egui::RichText::new("No GDTF fixtures loaded.")
                        .color(egui::Color32::GRAY),
                );
            } else {
                egui::ScrollArea::vertical()
                    .id_salt("lib_scroll")
                    .max_height(140.0)
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        egui::Grid::new("lib_grid")
                            .num_columns(3)
                            .striped(true)
                            .spacing([10.0, 3.0])
                            .show(ui, |ui| {
                                ui.strong("Manufacturer");
                                ui.strong("Name");
                                ui.strong("Modes");
                                ui.end_row();

                                for ft in res.library.all() {
                                    ui.label(&ft.manufacturer);
                                    ui.label(&ft.name);
                                    ui.monospace(format!("{}", ft.dmx_modes.len()));
                                    ui.end_row();
                                }
                            });
                    });
            }

            ui.separator();

            // ── Import GDTF ───────────────────────────────────────────────────
            ui.label(egui::RichText::new("Import GDTF").strong());
            ui.horizontal(|ui| {
                ui.add(
                    egui::TextEdit::singleline(&mut res.import_path)
                        .hint_text("Path to .gdtf file…")
                        .desired_width(260.0),
                );
                let load_clicked = ui.button("Load").clicked();
                if load_clicked {
                    load_gdtf(&mut res);
                }
            });

            ui.label(
                egui::RichText::new("Tip: drag a .gdtf file path from Finder or type it above.")
                    .small()
                    .color(egui::Color32::GRAY),
            );

            if let Some(err) = &res.import_error.clone() {
                ui.colored_label(egui::Color32::from_rgb(255, 80, 80), err);
            }
        });
}

fn load_gdtf(res: &mut FixtureLibraryRes) {
    let path = res.import_path.trim().to_string();
    if path.is_empty() {
        res.import_error = Some("Please enter a file path.".into());
        return;
    }
    match std::fs::read(&path) {
        Ok(data) => match res.library.load(&data) {
            Ok(_id) => {
                res.import_error = None;
                res.import_path.clear();
            }
            Err(e) => res.import_error = Some(format!("Parse error: {e}")),
        },
        Err(e) => res.import_error = Some(format!("Cannot read file: {e}")),
    }
}
