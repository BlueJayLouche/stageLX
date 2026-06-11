use bevy::prelude::*;
use bevy_egui::egui::{self, Color32, Pos2, Rect, RichText, Sense, Stroke, StrokeKind, Ui, Vec2};

use crate::theme::*;
use crate::widgets;
use crate::{PatchRes, PatchSelection};
use stagelx_show::Programmer;

// ═══════════════════════════════════════════════════════════════════════════════
// Programmer Panel (docked / inline)
// ═══════════════════════════════════════════════════════════════════════════════

pub fn programmer_panel_docked(
    ui: &mut Ui,
    prog: &mut Programmer,
    patch_sel: &mut PatchSelection,
    patch: &PatchRes,
) {
    let available_width = ui.available_width();

    // ── Selection bar ─────────────────────────────────────────────────────────
    {
        let bar_height = 28.0;
        let (rect, _response) = ui.allocate_exact_size(Vec2::new(available_width, bar_height), Sense::hover());
        if ui.is_rect_visible(rect) {
            let painter = ui.painter();
            painter.rect_filled(rect, 3.0, BG_INPUT);
            painter.rect_stroke(rect, 3.0, Stroke::new(1.0, BORDER_SOFT), StrokeKind::Inside);

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
                font_body(),
                FG,
            );
            let ids_width = ui.painter().layout_no_wrap(
                ids_text.clone(),
                font_body(),
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
                font_body(),
                FG_MUTED,
            );

            // Count
            let count_text = format!("{} / {}", patch_sel.selected_ids.len(), 24);
            painter.text(
                Pos2::new(rect.max.x - 8.0, center_y),
                egui::Align2::RIGHT_CENTER,
                &count_text,
                font_status(),
                FG_MUTED,
            );
        }
    }

    // ── Clear Selection ───────────────────────────────────────────────────────
    if !patch_sel.selected_ids.is_empty() {
        ui.horizontal(|ui| {
            ui.add_space(6.0);
            let btn = egui::Button::new(
                RichText::new("Clear Selection").size(10.0).color(FG_SECONDARY),
            )
            .fill(Color32::TRANSPARENT)
            .stroke(Stroke::new(1.0, BORDER_SOFT))
            .min_size(Vec2::new(0.0, 20.0));
            if ui.add(btn).clicked() {
                for id in &patch_sel.selected_ids {
                    prog.fixture_values.remove(id);
                }
                patch_sel.selected_ids.clear();
            }
        });
    }

    // Derive capability flags from the selected fixtures' channel maps.
    // When nothing is selected, show every section so the UI doesn't go blank.
    let (has_pan_tilt, has_gobo, has_color_macro, has_rotation) =
        if patch_sel.selected_ids.is_empty() {
            (true, true, true, true)
        } else {
            let mut pt = false;
            let mut gb = false;
            let mut cm = false;
            let mut rot = false;
            for id in &patch_sel.selected_ids {
                if let Some(f) = patch.0.get(*id) {
                    if f.channel_map.pan.is_some() || f.channel_map.tilt.is_some() { pt = true; }
                    if f.channel_map.gobo.is_some() { gb = true; }
                    if f.channel_map.color_macro.is_some() { cm = true; }
                    if f.channel_map.rotation.is_some() { rot = true; }
                }
            }
            (pt, gb, cm, rot)
        };

    egui::ScrollArea::vertical()
        .id_salt("programmer_scroll")
        .auto_shrink([false, true])
        .show(ui, |ui| {
        let available_width = ui.available_width();
    // ── Intensity ─────────────────────────────────────────────────────────────
    widgets::section_header(ui, "Intensity", Some("0–100%"));
    ui.horizontal(|ui| {
        ui.add_space(((available_width - 80.0) * 0.5).max(0.0));
        let mut dimmer_pct = prog.dimmer * 100.0;
        ui.add(widgets::Fader::new(&mut dimmer_pct, "Dimmer")
            .unit("%")
            .range(0.0, 100.0)
            .format(|v| format!("{:.0}", v)));
        prog.dimmer = dimmer_pct / 100.0;

        ui.add_space(24.0);
        let mut strobe_hz = prog.strobe * 25.0;
        ui.add(widgets::Fader::new(&mut strobe_hz, "Strobe")
            .unit("Hz")
            .range(0.0, 25.0)
            .format(|v| if v < 0.5 { "OFF".into() } else { format!("{:.0}", v) })
            .accent(WARNING));
        prog.strobe = strobe_hz / 25.0;
    });

    // ── Position ──────────────────────────────────────────────────────────────
    if has_pan_tilt {
    widgets::section_header(ui, "Position", Some("±270° / ±135°"));
    ui.horizontal(|ui| {
        ui.add_space(((available_width - 250.0) * 0.5).max(0.0));
        let pan_deg = (prog.pan - 0.5) * prog.pan_range;
        let mut pan_val = pan_deg;
        ui.add(widgets::Encoder::new(&mut pan_val, "Pan")
            .range(-270.0, 270.0)
            .default_value(0.0)
            .decimals(1)
            .unit("°")
            .sub_label("ABS"));
        prog.pan = (pan_val / prog.pan_range) + 0.5;

        ui.add_space(14.0);

        let tilt_deg = (prog.tilt - 0.5) * prog.tilt_range;
        let mut tilt_val = tilt_deg;
        ui.add(widgets::Encoder::new(&mut tilt_val, "Tilt")
            .range(-135.0, 135.0)
            .default_value(0.0)
            .decimals(1)
            .unit("°")
            .sub_label("ABS"));
        prog.tilt = (tilt_val / prog.tilt_range) + 0.5;

        ui.add_space(14.0);

        let zoom_deg = 5.0 + prog.zoom * 40.0;
        let mut zoom_val = zoom_deg;
        ui.add(widgets::Encoder::new(&mut zoom_val, "Zoom")
            .range(5.0, 45.0)
            .default_value(25.0)
            .decimals(0)
            .unit("°")
            .sub_label("BEAM"));
        prog.zoom = (zoom_val - 5.0) / 40.0;
    });
    } // has_pan_tilt

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

    // Active color row
    {
        let row_height = 32.0;
        let (rect, _response) = ui.allocate_exact_size(Vec2::new(available_width, row_height), Sense::hover());
        if ui.is_rect_visible(rect) {
            let painter = ui.painter();
            painter.rect_filled(rect, 3.0, BG_INPUT);
            painter.rect_stroke(rect, 3.0, Stroke::new(1.0, BORDER_SOFT), StrokeKind::Inside);

            let [r, g, b] = prog.color;
            let swatch_rect = Rect::from_min_size(
                Pos2::new(rect.min.x + 8.0, rect.min.y + 5.0),
                Vec2::splat(22.0),
            );
            painter.rect_filled(swatch_rect, 2.0, Color32::from_rgb((r*255.0) as u8, (g*255.0) as u8, (b*255.0) as u8));
            painter.rect_stroke(swatch_rect, 2.0, Stroke::new(1.0, Color32::from_rgba_premultiplied(0, 0, 0, 102)), StrokeKind::Inside);

            painter.text(
                Pos2::new(rect.min.x + 38.0, rect.center().y - 4.0),
                egui::Align2::LEFT_CENTER,
                preset_name,
                font_body(),
                FG,
            );
            painter.text(
                Pos2::new(rect.min.x + 38.0, rect.center().y + 8.0),
                egui::Align2::LEFT_CENTER,
                format!("#{:02X}{:02X}{:02X}", (r*255.0) as u8, (g*255.0) as u8, (b*255.0) as u8),
                font_hint(),
                FG_MUTED,
            );
        }
        let pick_rect = egui::Rect::from_min_size(
            egui::Pos2::new(rect.max.x - 48.0, rect.center().y - 11.0),
            egui::Vec2::new(40.0, 22.0),
        );
        let mut pick_clicked = false;
        ui.scope_builder(egui::UiBuilder::new().max_rect(pick_rect), |ui| {
            if ui.add_sized([40.0, 22.0], egui::Button::new(RichText::new("PICK").size(9.0).color(FG_SECONDARY)).fill(BG_RAISED).stroke(Stroke::new(1.0, BORDER))).clicked() {
                pick_clicked = true;
            }
        });
        if pick_clicked {
            // TODO: open colour picker popover
        }
    }

    // Swatch grid — reuses color_presets defined above
    let colors = color_presets;
    let swatch_width = 42.0;
    let items_per_row = ((available_width / swatch_width).floor() as usize).max(4);

    let mut selected_swatch = None;
    for (i, (_name, _color)) in colors.iter().enumerate() {
        if i % items_per_row == 0 {
            ui.horizontal(|ui| {
                for j in 0..items_per_row {
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

    // ── Gobo ──────────────────────────────────────────────────────────────────
    if has_gobo {
    widgets::section_header(ui, "Gobo", Some("wheel 1 · 4 slots"));
    let gobos = [("Open", 0), ("Dots", 1), ("Breakup", 2), ("Star", 3)];
    ui.horizontal(|ui| {
        ui.add_space(((available_width - 200.0) * 0.5).max(0.0));
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
                painter.rect_stroke(rect, 3.0, Stroke::new(1.0, if selected { ACCENT_DIM } else { BORDER_SOFT }), StrokeKind::Inside);

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
                    font_body(),
                    if selected { FG } else { FG_SECONDARY },
                );
            }
        }
    });

    // Spin slider
    ui.horizontal(|ui| {
        ui.label(RichText::new("Spin").size(10.0).color(FG_MUTED).strong());
        let slider_width = (available_width - 120.0).max(0.0);
        let (rect, response) = ui.allocate_exact_size(Vec2::new(slider_width, 12.0), Sense::drag());
        if response.dragged() {
            let delta = response.drag_delta().x;
            prog.gobo_spin = (prog.gobo_spin + delta * 0.02).clamp(-3.0, 3.0);
        }
        if ui.is_rect_visible(rect) {
            let painter = ui.painter();
            let track_rect = Rect::from_min_size(Pos2::new(rect.min.x, rect.min.y + 4.0), Vec2::new(slider_width, 4.0));
            painter.rect_filled(track_rect, 2.0, BG_INPUT);
            painter.rect_stroke(track_rect, 2.0, Stroke::new(1.0, BORDER_SOFT), StrokeKind::Inside);
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
    } // has_gobo

    // ── FX: Color Mode ────────────────────────────────────────────────────────
    if has_color_macro {
    // (name, representative display color for the swatch, mid-point DMX value)
    const COLOR_MODES: &[(&str, [u8; 3], u8)] = &[
        ("Off",    [20,  20,  20],  2),
        ("Red",    [220, 30,  30],  13),
        ("Green",  [30,  200, 50],  28),
        ("Blue",   [30,  60,  220], 43),
        ("White",  [220, 220, 220], 58),
        ("R+G",    [200, 180, 0],   73),
        ("R+B",    [180, 0,   160], 88),
        ("R+W",    [220, 140, 140], 103),
        ("G+B",    [0,   180, 180], 118),
        ("G+W",    [140, 220, 140], 133),
        ("B+W",    [140, 140, 220], 148),
        ("RGB",    [200, 160, 80],  163),
        ("RGW",    [220, 200, 140], 178),
        ("GBW",    [100, 200, 200], 193),
        ("RGBW",   [220, 220, 200], 208),
        ("Auto4",  [180, 100, 180], 223),
        ("Auto7",  [160, 160, 160], 243),
    ];

    widgets::section_header(ui, "Color Mode", Some("Ch 1 — preset"));

    // Display current mode name
    let current_name = COLOR_MODES.iter()
        .find(|(_, _, dmx)| *dmx == prog.color_macro
            || (prog.color_macro > dmx.saturating_sub(7) && prog.color_macro <= *dmx + 7))
        .map(|(n, _, _)| *n)
        .unwrap_or("—");
    ui.horizontal(|ui| {
        ui.label(hint_secondary(format!("Active: {}  (DMX {})", current_name, prog.color_macro)));
    });

    let swatch_w = 42.0;
    let per_row = ((available_width / swatch_w).floor() as usize).max(4).min(COLOR_MODES.len());
    let mut selected_mode: Option<u8> = None;
    for chunk_start in (0..COLOR_MODES.len()).step_by(per_row) {
        ui.horizontal(|ui| {
            for (name, rgb, dmx) in &COLOR_MODES[chunk_start..(chunk_start + per_row).min(COLOR_MODES.len())] {
                let is_sel = prog.color_macro == *dmx;
                let c = Color32::from_rgb(rgb[0], rgb[1], rgb[2]);
                if widgets::swatch(ui, c, name, is_sel).clicked() {
                    selected_mode = Some(*dmx);
                }
            }
        });
    }
    if let Some(dmx) = selected_mode {
        prog.color_macro = dmx;
        // Reflect the preset's representative colour in the display RGB so the
        // 3-D viewport tints macro-colour fixtures correctly (DMX still sends the
        // color_macro channel; RGB is ignored by fixtures without RGB channels).
        if let Some((_, rgb, _)) = COLOR_MODES.iter().find(|(_, _, d)| *d == dmx) {
            prog.color = [rgb[0] as f32 / 255.0, rgb[1] as f32 / 255.0, rgb[2] as f32 / 255.0];
        }
    }
    } // has_color_macro

    // ── FX: Motor Rotation ────────────────────────────────────────────────────
    if has_rotation {
    widgets::section_header(ui, "Motor", Some("speed · 128–255 DMX"));
    ui.horizontal(|ui| {
        ui.add_space(((available_width - 80.0) * 0.5).max(0.0));
        // Fader: 0 = stopped (DMX 0), 1 = full speed (DMX 255).
        // 128–255 is the "speed" range on the Mini Kinta (0–127 is index).
        let mut rot_pct = prog.rotation * 100.0;
        ui.add(widgets::Fader::new(&mut rot_pct, "Speed")
            .unit("%")
            .range(0.0, 100.0)
            .format(|v| if v < 0.5 { "STOP".into() } else { format!("{:.0}", v) })
            .accent(WARNING));
        prog.rotation = rot_pct / 100.0;
    });
    } // has_rotation

    // ── Quick actions ─────────────────────────────────────────────────────────
    // Tier 1 #6: divider positioned properly
    let divider_rect = ui.available_rect_before_wrap();
    ui.painter().line_segment(
        [Pos2::new(divider_rect.min.x, divider_rect.min.y), Pos2::new(divider_rect.max.x, divider_rect.min.y)],
        Stroke::new(1.0, BORDER_SOFT),
    );
    ui.horizontal(|ui| {
        let btn_count = if has_pan_tilt { 4 } else { 3 };
        let item_spacing_x = ui.spacing().item_spacing.x;
        let btn_width = (ui.available_width() - (btn_count as f32 - 1.0) * item_spacing_x) / btn_count as f32;
        if ui.add_sized([btn_width, 24.0], egui::Button::new("Black").fill(BG_RAISED).stroke(Stroke::new(1.0, BORDER))).clicked() {
            prog.dimmer = 0.0;
        }
        if ui.add_sized([btn_width, 24.0], egui::Button::new("Full").fill(BG_RAISED).stroke(Stroke::new(1.0, BORDER))).clicked() {
            prog.dimmer = 1.0;
            prog.color = [1.0, 1.0, 1.0];
        }
        if has_pan_tilt {
            if ui.add_sized([btn_width, 24.0], egui::Button::new("Home").fill(BG_RAISED).stroke(Stroke::new(1.0, BORDER))).clicked() {
                prog.pan = 0.5;
                prog.tilt = 0.5;
            }
        }
        if ui.add_sized([btn_width, 24.0], egui::Button::new("Reset").fill(Color32::TRANSPARENT).stroke(Stroke::NONE)).clicked() {
            *prog = Programmer::default();
        }
    });

    });

    // Hotkey hint
    ui.horizontal(|ui| {
        let hint = if has_pan_tilt {
            "←↑→↓ pan/tilt  ·  +/− dimmer  ·  Z zoom  ·  W/X/C colour"
        } else {
            "+/− dimmer  ·  W/X/C colour"
        };
        ui.add_space(((available_width - 300.0) * 0.5).max(0.0));
        ui.label(
            RichText::new(hint)
                .size(9.0)
                .monospace()
                .color(FG_FAINT),
        );
    });
}
