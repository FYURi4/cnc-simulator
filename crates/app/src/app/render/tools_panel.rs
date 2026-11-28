use crate::app::App;
use crate::types::{EditMode, SelectMode, SelectTool, TransformMode, UiAction};

impl App {
    pub(super) fn draw_tools_panel(
        ctx: &egui::Context,
        edit_m: EditMode,
        mode: TransformMode,
        sel_m: SelectMode,
        sel_tool: SelectTool,
        ui_action: &mut UiAction,
        ui_sel_m: &mut SelectMode,
        ui_sel_tool: &mut SelectTool,
    ) {
        egui::SidePanel::left("tools")
            .resizable(false)
            .exact_width(40.0)
            .show(ctx, |ui| {
                ui.vertical_centered(|ui| {
                    ui.add_space(10.0);
                    if edit_m == EditMode::Object {
                        if ui
                            .selectable_label(mode == TransformMode::Translate, "✥")
                            .clicked()
                        {
                            *ui_action = UiAction::SetMode(TransformMode::Translate);
                        }
                        if ui
                            .selectable_label(mode == TransformMode::Rotate, "⟲")
                            .clicked()
                        {
                            *ui_action = UiAction::SetMode(TransformMode::Rotate);
                        }
                        if ui
                            .selectable_label(mode == TransformMode::Scale, "⤢")
                            .clicked()
                        {
                            *ui_action = UiAction::SetMode(TransformMode::Scale);
                        }
                    } else {
                        if ui
                            .selectable_label(sel_m == SelectMode::Vertex, "1")
                            .on_hover_text("Vertex")
                            .clicked()
                        {
                            *ui_sel_m = SelectMode::Vertex;
                        }
                        if ui
                            .selectable_label(sel_m == SelectMode::Edge, "2")
                            .on_hover_text("Edge")
                            .clicked()
                        {
                            *ui_sel_m = SelectMode::Edge;
                        }
                        if ui
                            .selectable_label(sel_m == SelectMode::Face, "3")
                            .on_hover_text("Face")
                            .clicked()
                        {
                            *ui_sel_m = SelectMode::Face;
                        }
                        ui.separator();
                        if ui
                            .selectable_label(sel_tool == SelectTool::Click, "🖱️")
                            .on_hover_text("Select Click")
                            .clicked()
                        {
                            *ui_sel_tool = SelectTool::Click;
                        }
                        if ui
                            .selectable_label(sel_tool == SelectTool::Box, "⬚")
                            .on_hover_text("Select Box")
                            .clicked()
                        {
                            *ui_sel_tool = SelectTool::Box;
                        }
                    }
                });
            });
    }
}
