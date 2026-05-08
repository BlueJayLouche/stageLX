use bevy::prelude::*;
use bevy_egui::egui::{self, Color32, Pos2, Rect, RichText, Sense, Stroke, StrokeKind, Ui, Vec2};

use crate::theme::*;
use crate::widgets;
use crate::{PatchRes, PatchSelection};
use stagelx_state::Programmer;

// Legacy entry point (no longer registered as a system, kept for API compat)
pub fn programmer_panel(mut _ctx: bevy_egui::EguiContexts, mut _prog: ResMut<Programmer>) {
    // Obsolete — all UI now routes through ui_root_system in lib.rs
}

// ═══════════════════════════════════════════════════════════════════════════════
// Programmer Panel (docked / inline)
// ═══════════════════════════════════════════════════════════════════════════════

pub fn programmer_panel_docked(
    ui: &mut Ui,
    prog: &mut Programmer,
    patch_sel: &PatchSelection,
    patch: &PatchRes,
) {
    let available_width = ui.available_width();
    ui.set_min_width(available_width);

    // ── Selection bar ─────────────────────────────────────────────────────────
    {
        let bar_height = 28.0;
        let (rect, _response) = ui.allocate_exact_size(Vec2::new(available_width, bar_height), Sense::hover());
        if ui.is_rect_visible(rect) {
            let painter = ui.painter();
            painter.rect_filled(rect, 3.0, BG_INPUT);
            painter.rect_stroke(rect, 3.0, Stroke::new(1.0, BORDER_SOFT), StrokeKind::Middle);

            let mut cursor_x = rect.min.x + 8.0;
            let center_y = rect.center().y;

            // tx dot
            painter.circle_filled(Pos2::new(cursor_x + 3.0, center_y), 3.0, ACCENT);
            painter.circle_filled(Pos2::new(cursor_x + 3.0, center_y), 6.0, GLOW_TX);
            cursor_x += 14.0;

            // Selected fixture IDs
            let ids: Vec<String> = patch_sel.selected_ids.iter().map(|id| (id.0 + 1).to_string()).collect();
            let ids_text = if ids.is_empty() {
                "—".to_string()
            } else {
                ids.join("·")
            };
            painter.text(
                Pos2::new(cursor_x, center_y),
                egui::Align2::LEFT_CENTER,
                &ids_text,
                egui::TextStyle::Body.resolve(ui.style()),
                FG,
            );
            let ids_width = ui.painter().layout_no_wrap(
                ids_text.clone(),
                egui::TextStyle::Body.resolve(ui.style()),
                FG,
            ).size().x;
            cursor_x += ids_width + 10.0;

            // Fixture names, sorted by id for stable ordering
            let mut named: Vec<(u32, &str)> = patch_sel.selected_ids.iter()
                .filter_map(|id| patch.0.get(*id).map(|f| (id.0, f.name.as_str())))
                .collect();
            named.sort_by_key(|(id, _)| *id);
            let names_text = if named.is_empty() {
                "—".to_string()
            } else {
                named.iter().map(|(_, n)| *n).collect::<Vec<_>>().join(" · ")
            };
            painter.text(
                Pos2::new(cursor_x, center_y),
                egui::Align2::LEFT_CENTER,
                &names_text,
                egui::TextStyle::Body.resolve(ui.style()),
                FG_MUTED,
            );

            // Count
            let count_text = format!("{} / {}", patch_sel.selected_ids.len(), 24);
            painter.text(
                Pos2::new(rect.max.x - 8.0, center_y),
                egui::Align2::RIGHT_CENTER,
                &count_text,
                egui::TextStyle::Body.resolve(ui.style()),
                FG_MUTED,
            );
        }
        ui.add_space(12.0);
    }

    // ── Intensity ─────────────────────────────────────────────────────────────
    widgets::section_header(ui, "Intensity", Some("0–100%"));
    ui.horizontal(|ui| {
        ui.add_space((available_width - 80.0) * 0.5);
        let mut dimmer_pct = prog.dimmer;
        ui.add(widgets::Fader::new(&mut dimmer_pct, "Dimmer").unit("%"));
        prog.dimmer = dimmer_pct;
        ui.add_space(24.0);
        let mut strobe_norm = prog.strobe;
        let mut strobe_pct = strobe_norm * 100.0;
        ui.add(widgets::Fader::new(&mut strobe_pct, "Strobe").unit("Hz").accent(WARNING));
        if strobe_pct < 1.0 {
            // Keep as 0 for OFF display
            prog.strobe = 0.0;
        } else {
            prog.strobe = strobe_pct / 100.0;
        }
    });
    ui.add_space(14.0);

    // ── Position ──────────────────────────────────────────────────────────────
    widgets::section_header(ui, "Position", Some("±270° / ±135°"));
    ui.horizontal(|ui| {
        ui.add_space((available_width - 250.0) * 0.5);
        let pan_deg = (prog.pan - 0.5) * prog.pan_range;
        let mut pan_val = pan_deg;
        ui.add(widgets::Encoder::new(&mut pan_val, "Pan").range(-270.0, 270.0).decimals(1).unit("°").sub("ABS"));
        prog.pan = (pan_val / prog.pan_range) + 0.5;
        ui.add_space(14.0);

        let tilt_deg = (prog.tilt - 0.5) * prog.tilt_range;
        let mut tilt_val = tilt_deg;
        ui.add(widgets::Encoder::new(&mut tilt_val, "Tilt").range(-135.0, 135.0).decimals(1).unit("°").sub("ABS"));
        prog.tilt = (tilt_val / prog.tilt_range) + 0.5;
        ui.add_space(14.0);

        let zoom_deg = 5.0 + prog.zoom * 40.0;
        let mut zoom_val = zoom_deg;
        ui.add(widgets::Encoder::new(&mut zoom_val, "Zoom").range(5.0, 45.0).decimals(0).unit("°").sub("BEAM"));
        prog.zoom = (zoom_val - 5.0) / 40.0;
    });
    ui.add_space(14.0);

    // ── Colour ────────────────────────────────────────────────────────────────
    let color_presets: &[(&str, [f32; 3])] = &[
        ("White",   [1.0, 1.0, 1.0]),
        ("Red",     [1.0, 0.0, 0.0]),
        ("Amber",   [1.0, 0.55, 0.0]),
        ("Green",   [0.0, 1.0, 0.0]),
        ("Cyan",    [0.0, 0.9, 1.0]),
        ("Blue",    [0.0, 0.3, 1.0]),
        ("Magenta", [1.0, 0.0, 0.8]),
        ("UV",      [0.2, 0.0, 1.0]),
    ];
    let preset_name = color_presets.iter()
        .find(|(_, c)| {
            (prog.color[0] - c[0]).abs() < 0.01
                && (prog.color[1] - c[1]).abs() < 0.01
                && (prog.color[2] - c[2]).abs() < 0.01
        })
        .map(|(name, _)| *name)
        .unwrap_or("Custom");

    ui.horizontal(|ui| {
        widgets::eyebrow_widget(ui, "Colour");
        ui.label(hint_secondary("RGB · 8-bit"));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let [r, g, b] = prog.color;
            ui.label(hint_secondary(format!("{}·{}·{}", (r*255.0) as u8, (g*255.0) as u8, (b*255.0) as u8)));
        });
    });
    ui.add_space(8.0);

    // Active color row
    {
        let row_height = 32.0;
        let (rect, _response) = ui.allocate_exact_size(Vec2::new(available_width, row_height), Sense::hover());
        if ui.is_rect_visible(rect) {
            let painter = ui.painter();
            painter.rect_filled(rect, 3.0, BG_INPUT);
            painter.rect_stroke(rect, 3.0, Stroke::new(1.0, BORDER_SOFT), StrokeKind::Middle);

            let [r, g, b] = prog.color;
            let swatch_rect = Rect::from_min_size(
                Pos2::new(rect.min.x + 8.0, rect.min.y + 5.0),
                Vec2::splat(22.0),
            );
            painter.rect_filled(swatch_rect, 2.0, Color32::from_rgb((r*255.0) as u8, (g*255.0) as u8, (b*255.0) as u8));
            painter.rect_stroke(swatch_rect, 2.0, Stroke::new(1.0, Color32::from_rgba_premultiplied(0, 0, 0, 102)), StrokeKind::Middle);

            painter.text(
                Pos2::new(rect.min.x + 38.0, rect.center().y - 4.0),
                egui::Align2::LEFT_CENTER,
                preset_name,
                egui::TextStyle::Body.resolve(ui.style()),
                FG,
            );
            painter.text(
                Pos2::new(rect.min.x + 38.0, rect.center().y + 8.0),
                egui::Align2::LEFT_CENTER,
                &format!("#{:02X}{:02X}{:02X}", (r*255.0) as u8, (g*255.0) as u8, (b*255.0) as u8),
                egui::TextStyle::Body.resolve(ui.style()),
                FG_MUTED,
            );
        }
        let pick_rect = egui::Rect::from_min_size(
            egui::Pos2::new(rect.max.x - 48.0, rect.center().y - 11.0),
            egui::Vec2::new(40.0, 22.0),
        );
        let mut pick_clicked = false;
        ui.allocate_ui_at_rect(pick_rect, |ui| {
            if ui.add_sized([40.0, 22.0], egui::Button::new(RichText::new("PICK").size(9.0).color(FG_SECONDARY)).fill(BG_RAISED).stroke(Stroke::new(1.0, BORDER))).clicked() {
                pick_clicked = true;
            }
        });
        if pick_clicked {
            // TODO: open colour picker popover
        }
        ui.add_space(8.0);
    }

    // Swatch grid — reuses color_presets defined above
    let colors = color_presets;

    let mut selected_swatch = None;
    for (i, (name, color)) in colors.iter().enumerate() {
        if i % 8 == 0 {
            ui.horizontal(|ui| {
                for j in 0..8 {
                    if let Some((name, color)) = colors.get(i + j) {
                        let [r, g, b] = *color;
                        let is_selected = (prog.color[0] - r).abs() < 0.01
                            && (prog.color[1] - g).abs() < 0.01
                            && (prog.color[2] - b).abs() < 0.01;
                        let c = Color32::from_rgb((r * 255.0) as u8, (g * 255.0) as u8, (b * 255.0) as u8);
                        if widgets::swatch(ui, c, name, is_selected).clicked() {
                            selected_swatch = Some(*color);
                        }
                    }
                }
            });
        }
    }
    if let Some(c) = selected_swatch {
        prog.color = c;
    }
    ui.add_space(14.0);

    // ── Gobo ──────────────────────────────────────────────────────────────────
    widgets::section_header(ui, "Gobo", Some("wheel 1 · 4 slots"));
    let gobos = [("Open", 0), ("Dots", 1), ("Breakup", 2), ("Star", 3)];
    ui.horizontal(|ui| {
        ui.add_space((available_width - 200.0) * 0.5);
        for (name, idx) in gobos {
            let selected = prog.gobo_index == idx;
            let size = Vec2::new(available_width.min(200.0) / 4.0 - 4.0, available_width.min(200.0) / 4.0 - 4.0);
            let (rect, response) = ui.allocate_exact_size(size.max(Vec2::splat(48.0)), Sense::click());
            if response.clicked() {
                prog.gobo_index = idx;
            }
            if ui.is_rect_visible(rect) {
                let painter = ui.painter();
                painter.rect_filled(rect, 3.0, if selected { BG_RAISED } else { BG_INPUT });
                painter.rect_stroke(rect, 3.0, Stroke::new(1.0, if selected { ACCENT_DIM } else { BORDER_SOFT }), StrokeKind::Middle);

                // Simple SVG-like glyphs
                let cx = rect.center().x;
                let cy = rect.center().y - 4.0;
                let stroke_c = if selected { ACCENT } else { FG_MUTED };
                match idx {
                    0 => {
                        painter.circle_stroke(Pos2::new(cx, cy), 10.0, Stroke::new(1.0, stroke_c));
                    }
                    1 => {
                        painter.circle_stroke(Pos2::new(cx, cy), 10.0, Stroke::new(1.0, stroke_c));
                        for (dx, dy) in [(-3.0, -3.0), (3.0, -3.0), (-3.0, 3.0), (3.0, 3.0), (0.0, 0.0)] {
                            painter.circle_filled(Pos2::new(cx + dx, cy + dy), 1.2, stroke_c);
                        }
                    }
                    2 => {
                        painter.circle_stroke(Pos2::new(cx, cy), 10.0, Stroke::new(1.0, stroke_c));
                        painter.line_segment([Pos2::new(cx - 5.0, cy - 2.0), Pos2::new(cx - 2.0, cy - 3.0)], Stroke::new(0.8, stroke_c));
                        painter.line_segment([Pos2::new(cx - 2.0, cy - 3.0), Pos2::new(cx + 2.0, cy - 1.0)], Stroke::new(0.8, stroke_c));
                        painter.line_segment([Pos2::new(cx + 2.0, cy - 1.0), Pos2::new(cx + 5.0, cy - 2.0)], Stroke::new(0.8, stroke_c));
                    }
                    3 => {
                        painter.circle_stroke(Pos2::new(cx, cy), 10.0, Stroke::new(1.0, stroke_c));
                        // Simple star approximation
                        let star_points: Vec<Pos2> = (0..10).map(|i| {
                            let angle = (i as f32 * 36.0 - 90.0).to_radians();
                            let radius = if i % 2 == 0 { 8.0 } else { 4.0 };
                            Pos2::new(cx + radius * angle.cos(), cy + radius * angle.sin())
                        }).collect();
                        painter.add(egui::epaint::PathShape::convex_polygon(star_points, stroke_c, Stroke::NONE));
                    }
                    _ => {}
                }

                painter.text(
                    Pos2::new(rect.center().x, rect.max.y - 4.0),
                    egui::Align2::CENTER_BOTTOM,
                    name,
                    egui::TextStyle::Body.resolve(ui.style()),
                    if selected { FG } else { FG_SECONDARY },
                );
            }
        }
    });

    // Spin slider
    ui.horizontal(|ui| {
        ui.label(RichText::new("Spin").size(10.0).color(FG_MUTED).strong());
        let slider_width = available_width - 120.0;
        let (rect, response) = ui.allocate_exact_size(Vec2::new(slider_width, 12.0), Sense::drag());
        if response.dragged() {
            let delta = response.drag_delta().x;
            prog.gobo_spin = (prog.gobo_spin + delta * 0.02).clamp(-3.0, 3.0);
        }
        if ui.is_rect_visible(rect) {
            let painter = ui.painter();
            let track_rect = Rect::from_min_size(Pos2::new(rect.min.x, rect.min.y + 4.0), Vec2::new(slider_width, 4.0));
            painter.rect_filled(track_rect, 2.0, BG_INPUT);
            painter.rect_stroke(track_rect, 2.0, Stroke::new(1.0, BORDER_SOFT), StrokeKind::Middle);
            // Center detent
            painter.line_segment([Pos2::new(track_rect.center().x, track_rect.min.y), Pos2::new(track_rect.center().x, track_rect.max.y)], Stroke::new(1.0, BORDER_STRONG));
            // Fill from center
            let norm = prog.gobo_spin / 3.0;
            let fill_width = norm.abs() * slider_width * 0.5;
            if fill_width > 0.5 {
                let fill_rect = if norm > 0.0 {
                    Rect::from_min_max(Pos2::new(track_rect.center().x, track_rect.min.y), Pos2::new(track_rect.center().x + fill_width, track_rect.max.y))
                } else {
                    Rect::from_min_max(Pos2::new(track_rect.center().x - fill_width, track_rect.min.y), Pos2::new(track_rect.center().x, track_rect.max.y))
                };
                painter.rect_filled(fill_rect, 2.0, ACCENT);
            }
        }
        let spin_text = if prog.gobo_spin.abs() < 0.05 {
            "OFF".to_string()
        } else {
            format!("{:+.1} r/s", prog.gobo_spin)
        };
        ui.label(RichText::new(spin_text).size(11.0).monospace().color(FG));
    });
    ui.add_space(14.0);

    // ── Quick actions ─────────────────────────────────────────────────────────
    ui.painter().line_segment([Pos2::new(ui.min_rect().min.x, ui.cursor().min.y), Pos2::new(ui.min_rect().max.x, ui.cursor().min.y)], Stroke::new(1.0, BORDER_SOFT));
    ui.add_space(8.0);
    ui.horizontal(|ui| {
        let btn_width = (available_width - 12.0) / 4.0;
        if ui.add_sized([btn_width, 24.0], egui::Button::new("Black").fill(BG_RAISED).stroke(Stroke::new(1.0, BORDER))).clicked() {
            prog.dimmer = 0.0;
        }
        if ui.add_sized([btn_width, 24.0], egui::Button::new("Full").fill(BG_RAISED).stroke(Stroke::new(1.0, BORDER))).clicked() {
            prog.dimmer = 1.0;
            prog.color = [1.0, 1.0, 1.0];
        }
        if ui.add_sized([btn_width, 24.0], egui::Button::new("Home").fill(BG_RAISED).stroke(Stroke::new(1.0, BORDER))).clicked() {
            prog.pan = 0.5;
            prog.tilt = 0.5;
        }
        if ui.add_sized([btn_width, 24.0], egui::Button::new("Reset").fill(Color32::TRANSPARENT).stroke(Stroke::NONE)).clicked() {
            *prog = Programmer::default();
        }
    });
    ui.add_space(8.0);

    // Hotkey hint
    ui.horizontal(|ui| {
        ui.add_space((available_width - 300.0) * 0.5);
        ui.label(
            RichText::new("←↑→↓ pan/tilt  ·  +/− dimmer  ·  Z zoom  ·  W/X/C colour")
                .size(9.0)
                .monospace()
                .color(FG_FAINT),
        );
    });
}
