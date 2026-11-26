use egui;
use crate::app::App;

impl App {
    pub(super) fn draw_numeric_overlay(ctx: &egui::Context, viewport_size: glam::Vec2, numeric_overlay: Option<&(String, String, String)>) {
        if let Some((mode_label, axis_label, value)) = &numeric_overlay {
                            egui::Area::new(egui::Id::new("numeric_transform_input"))
                                .order(egui::Order::Foreground)
                                .fixed_pos(egui::pos2(
                                    (viewport_size.x * 0.5 - 110.0).max(8.0),
                                    (viewport_size.y - 76.0).max(8.0),
                                ))
                                .show(ctx, |ui| {
                                    egui::Frame::popup(ui.style()).show(ui, |ui| {
                                        ui.horizontal(|ui| {
                                            ui.label(egui::RichText::new(format!("{} · {}", mode_label, axis_label)).strong());
                                            ui.monospace(if value.is_empty() { "Enter value" } else { value.as_str() });
                                        });
                                    });
                                });
                        }
    }
}
