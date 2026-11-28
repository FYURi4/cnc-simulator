use crate::app::{App, ViewDirection};
use crate::types::{
    GridSettings, GridType, GridUnit, NumericInputMode, SnapSettings, SnapTarget, UiAction,
};

impl App {
    #[expect(clippy::too_many_arguments, reason = "The top bar edits independent settings and actions owned by the app.")]
    pub(super) fn draw_top_bar(
        ctx: &egui::Context,
        ui_action: &mut UiAction,
        grid_extent_ui: &mut f32,
        ui_view: &mut ViewDirection,
        snap: &mut SnapSettings,
        grid_settings: &mut GridSettings,
        show_save_as: &mut bool,
        show_color_picker: &mut bool,
        color_picker_hex: &mut String,
        selected: Option<bevy_ecs::prelude::Entity>,
        numeric_input_mode: &mut NumericInputMode,
    ) {
        egui::TopBottomPanel::top("top_bar").show(ctx, |ui| {
            egui::MenuBar::new().ui(ui, |ui| {
                ui.menu_button("File", |ui| {
                    if ui.button("💾 Save").clicked() {
                        *ui_action = UiAction::Save;
                        ui.close();
                    }
                    if ui.button("💾 Save As...").clicked() {
                        *show_save_as = true;
                        ui.close();
                    }
                    if ui.button("📂 Load").clicked() {
                        *ui_action = UiAction::Load;
                        ui.close();
                    }
                    if ui.button("⚙ Load G-code").clicked() {
                        *ui_action = UiAction::LoadGcode;
                        ui.close();
                    }
                });
                ui.menu_button("Add", |ui| {
                    if ui.button("Cube").clicked() {
                        *ui_action = UiAction::AddCube;
                        ui.close();
                    }
                    if ui.button("Plane").clicked() {
                        *ui_action = UiAction::AddPlane;
                        ui.close();
                    }
                    if ui.button("Cylinder").clicked() {
                        *ui_action = UiAction::AddCylinder;
                        ui.close();
                    }
                    if ui.button("Sphere").clicked() {
                        *ui_action = UiAction::AddSphere;
                        ui.close();
                    }
                    if ui.button("Pyramid").clicked() {
                        *ui_action = UiAction::AddPyramid;
                        ui.close();
                    }
                });
                ui.menu_button("Edit", |ui| {
                    if ui.button("Undo (Ctrl+Z)").clicked() {
                        *ui_action = UiAction::Undo;
                        ui.close();
                    }
                    if ui.button("Redo (Ctrl+Y)").clicked() {
                        *ui_action = UiAction::Redo;
                        ui.close();
                    }
                    ui.separator();
                    if ui.button("Copy (Ctrl+C)").clicked() {
                        *ui_action = UiAction::CopySelected;
                        ui.close();
                    }
                    if ui.button("Paste (Ctrl+V)").clicked() {
                        *ui_action = UiAction::PasteSelected;
                        ui.close();
                    }
                });
                ui.menu_button("Settings", |ui| {
                    ui.heading("Numeric Transform Input");
                    ui.separator();
                    ui.radio_value(numeric_input_mode, NumericInputMode::Absolute, "Absolute");
                    ui.radio_value(numeric_input_mode, NumericInputMode::Relative, "Relative");
                    ui.separator();
                    match numeric_input_mode {
                        NumericInputMode::Absolute => {
                            ui.label("Entered values set the final transform.")
                        }
                        NumericInputMode::Relative => {
                            ui.label("Entered values are applied relative to the start.")
                        }
                    };
                });
                ui.menu_button("Space", |ui| {
                    ui.heading("Grid Settings");
                    ui.separator();
                    ui.checkbox(&mut grid_settings.visible, "Visible");
                    ui.add(
                        egui::Slider::new(grid_extent_ui, 10.0..=500.0)
                            .logarithmic(true)
                            .text("Size"),
                    );
                    let grid_mm = (*grid_extent_ui * 1000.0).round() as i32;
                    let grid_m = *grid_extent_ui / 1000.0;
                    ui.horizontal(|ui| {
                        ui.label("Unit:");
                        ui.selectable_value(&mut grid_settings.unit, GridUnit::Mm, "mm");
                        ui.selectable_value(&mut grid_settings.unit, GridUnit::M, "m");
                    });
                    match grid_settings.unit {
                        GridUnit::Mm => {
                            ui.label(format!("{} mm", grid_mm));
                        }
                        GridUnit::M => {
                            ui.label(format!("{:.3} m", grid_m));
                        }
                    }
                    ui.separator();
                    ui.label("Grid Step (mm):");
                    ui.horizontal(|ui| {
                        ui.label("X:");
                        let _r = ui.add(
                            egui::DragValue::new(&mut grid_settings.step_x)
                                .speed(0.5)
                                .range(0.1..=100000.0)
                                .suffix(" mm"),
                        );
                    });
                    ui.horizontal(|ui| {
                        ui.label("Y:");
                        ui.add(
                            egui::DragValue::new(&mut grid_settings.step_y)
                                .speed(0.5)
                                .range(0.1..=100000.0)
                                .suffix(" mm"),
                        );
                    });
                    ui.separator();
                    ui.label("Color:");
                    let c = &mut grid_settings.color;
                    if ui.button("🎨").clicked() {
                        *show_color_picker = !*show_color_picker;
                        if *show_color_picker {
                            *color_picker_hex = format!(
                                "{:02X}{:02X}{:02X}",
                                (c[0] * 255.0) as u8,
                                (c[1] * 255.0) as u8,
                                (c[2] * 255.0) as u8
                            );
                        }
                    }
                    ui.label(format!(
                        "{:02X} {:02X} {:02X}",
                        (c[0] * 255.0) as u8,
                        (c[1] * 255.0) as u8,
                        (c[2] * 255.0) as u8
                    ));
                    ui.label("Type:");
                    ui.horizontal(|ui| {
                        ui.selectable_value(&mut grid_settings.grid_type, GridType::Grid, "Grid");
                        ui.selectable_value(&mut grid_settings.grid_type, GridType::Dots, "Dots");
                        ui.selectable_value(
                            &mut grid_settings.grid_type,
                            GridType::DotsMajor,
                            "Dots Major",
                        );
                        ui.selectable_value(
                            &mut grid_settings.grid_type,
                            GridType::DotsOnly,
                            "Dots Only",
                        );
                    });
                });
                ui.menu_button("View", |ui| {
                    egui::ComboBox::from_id_salt("view_direction")
                        .selected_text(ui_view.label())
                        .show_ui(ui, |ui| {
                            ui.selectable_value(ui_view, ViewDirection::Top, "Top (Y+)");
                            ui.selectable_value(ui_view, ViewDirection::Bottom, "Bottom (Y-)");
                            ui.selectable_value(ui_view, ViewDirection::Front, "Front (Z+)");
                            ui.selectable_value(ui_view, ViewDirection::Back, "Back (Z-)");
                            ui.selectable_value(ui_view, ViewDirection::Left, "Left (X-)");
                            ui.selectable_value(ui_view, ViewDirection::Right, "Right (X+)");
                            ui.selectable_value(ui_view, ViewDirection::Iso, "Iso");
                        });
                    if selected.is_some() && ui.button("📍 Set Origin").clicked() {
                        *ui_action = UiAction::SetOriginToCursorConfirmed;
                        ui.close();
                    }
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.toggle_value(&mut snap.enabled, "🧲 Snap");
                    egui::ComboBox::from_id_salt("snap_target")
                        .selected_text(format!("{:?}", snap.target))
                        .show_ui(ui, |ui| {
                            ui.selectable_value(
                                &mut snap.target,
                                SnapTarget::Increment,
                                "Increment",
                            );
                            ui.selectable_value(&mut snap.target, SnapTarget::Grid, "Grid");
                            ui.selectable_value(&mut snap.target, SnapTarget::Origin, "Origin");
                            ui.selectable_value(&mut snap.target, SnapTarget::Vertex, "Vertex");
                            ui.selectable_value(&mut snap.target, SnapTarget::Edge, "Edge");
                        });
                });
            });
        });
    }
}
