use egui;
use glam::Vec2;
use crate::app::App;
use crate::types::UiAction;

impl App {
    pub(super) fn draw_dialogs(ctx: &egui::Context, drag_start: Option<Vec2>, drag_end: Option<Vec2>, ui_action: &mut UiAction, delete_confirm: &mut bool, delete_confirm_pos: Vec2, show_save_as: &mut bool, save_as_path: &mut String, show_color_picker: &mut bool, color_picker_hex: &mut String, grid_color: &mut [f32; 4]) {
        if let (Some(start), Some(end)) = (drag_start, drag_end) {
                            let rect = egui::Rect::from_two_pos(
                                egui::pos2(start.x, start.y),
                                egui::pos2(end.x, end.y),
                            );
                            egui::Area::new(egui::Id::new("box_select"))
                                .fixed_pos(rect.min)
                                .show(ctx, |ui| {
                                    let painter = ui.painter();
                                    let r = egui::Rect::from_min_size(egui::Pos2::ZERO, rect.size());
                                    painter.rect_filled(r, 0.0, egui::Color32::from_rgba_unmultiplied(255, 255, 255, 50));
                                    painter.rect_stroke(r, 0.0, egui::Stroke::new(1.0_f32, egui::Color32::WHITE), egui::StrokeKind::Outside);
                                });
                        }

                        if *delete_confirm {
                            let dialog_pos = egui::pos2(
                                (delete_confirm_pos.x + 15.0).clamp(0.0, ctx.viewport_rect().max.x - 200.0),
                                (delete_confirm_pos.y + 15.0).clamp(0.0, ctx.viewport_rect().max.y - 80.0),
                            );
                            egui::Area::new(egui::Id::new("delete_confirm"))
                                .fixed_pos(dialog_pos)
                                .show(ctx, |ui| {
                                    let layout = egui::Layout::centered_and_justified(egui::Direction::LeftToRight);
                                    ui.allocate_ui_with_layout(egui::Vec2::ZERO, layout, |ui| {
                                        ui.horizontal(|ui| {
                                            ui.label("Удалить объект?");
                                            if ui.button("Да").clicked() {
                                                *ui_action = UiAction::ConfirmDelete;
                                                *delete_confirm = false;
                                                ui.close();
                                            }
                                            if ui.button("Нет").clicked() {
                                                *delete_confirm = false;
                                                ui.close();
                                            }
                                        });
                                    });
                                });
                        }

                        if *show_save_as {
                            egui::Window::new("Save As")
                                .default_size([320.0, 100.0])
                                .show(ctx, |ui| {
                                    ui.label("File path:");
                                    ui.text_edit_singleline(save_as_path);
                                    ui.separator();
                                    ui.horizontal(|ui| {
                                        if ui.button("Save").clicked() {
                                            *ui_action = UiAction::SaveAs;
                                            *show_save_as = false;
                                        }
                                        if ui.button("Cancel").clicked() {
                                            *show_save_as = false;
                                        }
                                    });
                                });
                        }

                        if *show_color_picker {
                            egui::Window::new("Grid Color")
                                .default_size([220.0, 160.0])
                                .show(ctx, |ui| {
                                    let c = grid_color;
                                    let mut rgb = [
                                        (c[0] * 255.0) as u8,
                                        (c[1] * 255.0) as u8,
                                        (c[2] * 255.0) as u8,
                                    ];
                                    if ui.color_edit_button_srgb(&mut rgb).changed() {
                                        c[0] = rgb[0] as f32 / 255.0;
                                        c[1] = rgb[1] as f32 / 255.0;
                                        c[2] = rgb[2] as f32 / 255.0;
                                        *color_picker_hex = format!("{:02X}{:02X}{:02X}", rgb[0], rgb[1], rgb[2]);
                                    }
                                    ui.separator();
                                    ui.horizontal(|ui| {
                                        ui.label("HEX:");
                                        let resp = ui.add(egui::TextEdit::singleline(color_picker_hex)
                                            .font(egui::FontId::monospace(12.0)));
                                        if resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                                            let hex = *color_picker_hex.trim().trim_start_matches('#');
                                            if hex.len() == 6 {
                                                if let Ok(val) = u32::from_str_radix(hex, 16) {
                                                    let r = ((val >> 16) & 0xFF) as f32 / 255.0;
                                                    let g = ((val >> 8) & 0xFF) as f32 / 255.0;
                                                    let b = (val & 0xFF) as f32 / 255.0;
                                                    c[0] = r;
                                                    c[1] = g;
                                                    c[2] = b;
                                                    rgb[0] = (r * 255.0) as u8;
                                                    rgb[1] = (g * 255.0) as u8;
                                                    rgb[2] = (b * 255.0) as u8;
                                                }
                                            }
                                        }
                                    });
                                    ui.separator();
                                    if ui.button("OK").clicked() {
                                        *show_color_picker = false;
                                        *color_picker_hex = format!("{:02X}{:02X}{:02X}", rgb[0], rgb[1], rgb[2]);
                                    }
                                });
                        }
    }
}
