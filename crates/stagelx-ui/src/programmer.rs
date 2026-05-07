use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts};
use crate::Programmer;

pub fn programmer_panel(mut ctx: EguiContexts, mut prog: ResMut<Programmer>) {
    egui::Window::new("Programmer")
        .default_pos([10.0, 10.0])
        .default_width(270.0)
        .resizable(false)
        .show(&ctx.ctx_mut().expect("egui context"), |ui| {
            // ── Dimmer ────────────────────────────────────────────────────────
            ui.label(
                egui::RichText::new("DIMMER")
                    .strong()
                    .color(egui::Color32::from_rgb(255, 220, 60)),
            );
            ui.horizontal(|ui| {
                ui.add(
                    egui::Slider::new(&mut prog.dimmer, 0.0..=1.0)
                        .show_value(false)
                        .clamping(egui::SliderClamping::Always),
                );
                ui.monospace(format!("{:.0}%", prog.dimmer * 100.0));
            });

            ui.add_space(8.0);

            // ── Position ──────────────────────────────────────────────────────
            ui.label(
                egui::RichText::new("POSITION")
                    .strong()
                    .color(egui::Color32::from_rgb(100, 180, 255)),
            );

            let pan_deg = (prog.pan - 0.5) * prog.pan_range;
            ui.horizontal(|ui| {
                ui.add(
                    egui::Slider::new(&mut prog.pan, 0.0..=1.0)
                        .show_value(false)
                        .clamping(egui::SliderClamping::Always),
                );
                ui.monospace(format!("Pan  {:+.1}°", pan_deg));
            });

            let tilt_deg = (prog.tilt - 0.5) * prog.tilt_range;
            ui.horizontal(|ui| {
                ui.add(
                    egui::Slider::new(&mut prog.tilt, 0.0..=1.0)
                        .show_value(false)
                        .clamping(egui::SliderClamping::Always),
                );
                ui.monospace(format!("Tilt {:+.1}°", tilt_deg));
            });

            ui.add_space(8.0);

            // ── Beam ──────────────────────────────────────────────────────────
            ui.label(
                egui::RichText::new("BEAM")
                    .strong()
                    .color(egui::Color32::from_rgb(180, 255, 180)),
            );
            ui.horizontal(|ui| {
                ui.add(
                    egui::Slider::new(&mut prog.zoom, 0.0..=1.0)
                        .show_value(false)
                        .clamping(egui::SliderClamping::Always),
                );
                let angle_deg = 5.0 + prog.zoom * 40.0;
                ui.monospace(format!("Zoom {:.0}°", angle_deg));
            });
            ui.horizontal(|ui| {
                ui.add(
                    egui::Slider::new(&mut prog.strobe, 0.0..=1.0)
                        .show_value(false)
                        .clamping(egui::SliderClamping::Always),
                );
                if prog.strobe < 0.01 {
                    ui.monospace("Strobe OFF");
                } else {
                    ui.monospace(format!("Strobe {:.0} Hz", prog.strobe * 25.0));
                }
            });

            ui.add_space(8.0);

            // ── Colour ────────────────────────────────────────────────────────
            ui.label(
                egui::RichText::new("COLOUR")
                    .strong()
                    .color(egui::Color32::from_rgb(255, 160, 80)),
            );
            ui.color_edit_button_rgb(&mut prog.color);

            ui.add_space(4.0);
            ui.horizontal_wrapped(|ui| {
                for (label, color) in [
                    ("White",  [1.0_f32, 1.0, 1.0]),
                    ("Red",    [1.0, 0.0, 0.0]),
                    ("Green",  [0.0, 1.0, 0.0]),
                    ("Blue",   [0.0, 0.3, 1.0]),
                    ("Amber",  [1.0, 0.55, 0.0]),
                    ("Cyan",   [0.0, 0.9, 1.0]),
                    ("Magenta",[1.0, 0.0, 0.8]),
                    ("UV",     [0.2, 0.0, 1.0]),
                ] {
                    if ui.small_button(label).clicked() {
                        prog.color = color;
                    }
                }
            });

            ui.add_space(8.0);

            // ── Gobo ──────────────────────────────────────────────────────────
            ui.label(
                egui::RichText::new("GOBO")
                    .strong()
                    .color(egui::Color32::from_rgb(255, 200, 120)),
            );
            ui.horizontal_wrapped(|ui| {
                for (i, label) in ["Open", "Dots", "Breakup", "Star"].iter().enumerate() {
                    let selected = prog.gobo_index == i;
                    if ui
                        .selectable_label(selected, *label)
                        .clicked()
                    {
                        prog.gobo_index = i;
                    }
                }
            });
            ui.horizontal(|ui| {
                ui.add(
                    egui::Slider::new(&mut prog.gobo_spin, -3.0..=3.0)
                        .show_value(false)
                        .clamping(egui::SliderClamping::Always),
                );
                if prog.gobo_spin.abs() < 0.05 {
                    ui.monospace("Spin OFF");
                } else {
                    ui.monospace(format!("Spin {:+.1} r/s", prog.gobo_spin));
                }
            });

            ui.separator();

            // ── Quick actions ─────────────────────────────────────────────────
            ui.horizontal(|ui| {
                if ui.button("Blackout").clicked() {
                    prog.dimmer = 0.0;
                }
                if ui.button("Full ON").clicked() {
                    prog.dimmer = 1.0;
                    prog.color = [1.0, 1.0, 1.0];
                }
                if ui.button("Home").clicked() {
                    prog.pan = 0.5;
                    prog.tilt = 0.5;
                }
                if ui.button("Reset All").clicked() {
                    *prog = Programmer::default();
                }
            });

            ui.add_space(4.0);
            ui.label(
                egui::RichText::new("Arrow keys = Pan/Tilt  |  +/- = Dimmer  |  Z/z = Zoom  |  W/X/C = colour")
                    .small()
                    .color(egui::Color32::GRAY),
            );
        });
}
