use crate::app::App;
use crate::geometry::*;
use crate::types::*;
use bevy_ecs::prelude::*;
use egui;
use engine_core::{Scene, Transform};
use engine_render::mesh::Vertex;
use glam::Vec3;

impl App {
    pub(super) fn draw_properties_panel(
        ui: &mut egui::Ui,
        scene: &mut Scene,
        selected: Option<Entity>,
        edit_m: EditMode,
        selected_elements: &[Selection],
        drag_speed: f32,
        format_val: &dyn Fn(f32) -> String,
        pending_history_record: &mut bool,
        ui_action: &mut UiAction,
        render_map_clone: &[(Entity, usize)],
        pending_vertex_update: &mut Option<(Entity, Vec<Vertex>)>,
        cursor_e: Option<Entity>,
    ) {
        egui::CentralPanel::default().show_inside(ui, |ui| {
            ui.heading(if edit_m == EditMode::Object {
                "Properties"
            } else {
                "Edit Mode"
            });
            ui.separator();
            egui::ScrollArea::vertical().show(ui, |ui| {
                if let Some(sel) = selected {
                    if let Some(name) = scene.get_name(sel) {
                        ui.label(format!("Name: {}", name));
                    }
                    ui.separator();

                    if edit_m == EditMode::Edit && !selected_elements.is_empty() {
                        if let Some(mut custom) = scene.world.get_mut::<CustomMesh>(sel) {
                            let mut unique_verts = std::collections::HashSet::new();
                            for s in selected_elements {
                                match s {
                                    Selection::Vertex(i) => {
                                        unique_verts.insert(*i);
                                    }
                                    Selection::Edge(a, b) => {
                                        unique_verts.insert(*a);
                                        unique_verts.insert(*b);
                                    }
                                    Selection::Face(indices) => {
                                        for &i in indices {
                                            unique_verts.insert(i);
                                        }
                                    }
                                }
                            }
                            if !unique_verts.is_empty() {
                                let mut center = Vec3::ZERO;
                                for &i in &unique_verts {
                                    center += custom.vertices[i];
                                }
                                center /= unique_verts.len() as f32;
                                let mut new_center = center;
                                ui.label("Selection Center:");
                                let mut changed = false;
                                egui::Grid::new("sel_grid").show(ui, |ui| {
                                    ui.label("Location X");
                                    let r = ui.add(
                                        egui::DragValue::new(&mut new_center.x).speed(drag_speed),
                                    );
                                    ui.label(format_val(new_center.x));
                                    changed |= r.changed();
                                    if r.drag_stopped() || r.lost_focus() {
                                        *pending_history_record = true;
                                    }
                                    ui.end_row();
                                    ui.label("Location Y");
                                    let r = ui.add(
                                        egui::DragValue::new(&mut new_center.y).speed(drag_speed),
                                    );
                                    ui.label(format_val(new_center.y));
                                    changed |= r.changed();
                                    if r.drag_stopped() || r.lost_focus() {
                                        *pending_history_record = true;
                                    }
                                    ui.end_row();
                                    ui.label("Location Z");
                                    let r = ui.add(
                                        egui::DragValue::new(&mut new_center.z).speed(drag_speed),
                                    );
                                    ui.label(format_val(new_center.z));
                                    changed |= r.changed();
                                    if r.drag_stopped() || r.lost_focus() {
                                        *pending_history_record = true;
                                    }
                                    ui.end_row();
                                });
                                if changed {
                                    let delta = new_center - center;
                                    for &i in &unique_verts {
                                        custom.vertices[i] += delta;
                                    }
                                    let kind = scene.world.get::<MeshKindComp>(sel).unwrap().0;
                                    let new_vertices =
                                        build_mesh_vertices(scene, sel, kind, selected_elements);
                                    if render_map_clone.iter().any(|(e, _)| *e == sel) {
                                        *pending_vertex_update = Some((sel, new_vertices));
                                    }
                                }
                            }
                        }
                    } else {
                        if edit_m == EditMode::Object {
                            ui.menu_button("Set Origin", |ui| {
                                if ui.button("To 3D Cursor").clicked() {
                                    *ui_action = UiAction::SetOriginToCursor;
                                    ui.close();
                                }
                                if ui.button("To Geometry").clicked() {
                                    *ui_action = UiAction::SetOriginToGeometry;
                                    ui.close();
                                }
                            });
                            ui.separator();
                        }

                        if let Some(mut t) = scene.world.get_mut::<Transform>(sel) {
                            ui.label("Transform:");
                            ui.label("Scale");
                            egui::Grid::new("scale_grid").num_columns(3).show(ui, |ui| {
                                ui.label("Scale X");
                                let r = ui.add(
                                    egui::DragValue::new(&mut t.scale.x)
                                        .speed(0.01)
                                        .range(0.0001..=100000.0),
                                );
                                ui.label(format!("{:.3}", t.scale.x));
                                if r.drag_stopped() || r.lost_focus() {
                                    *pending_history_record = true;
                                }
                                ui.end_row();

                                ui.label("Scale Y");
                                let r = ui.add(
                                    egui::DragValue::new(&mut t.scale.y)
                                        .speed(0.01)
                                        .range(0.0001..=100000.0),
                                );
                                ui.label(format!("{:.3}", t.scale.y));
                                if r.drag_stopped() || r.lost_focus() {
                                    *pending_history_record = true;
                                }
                                ui.end_row();

                                ui.label("Scale Z");
                                let r = ui.add(
                                    egui::DragValue::new(&mut t.scale.z)
                                        .speed(0.01)
                                        .range(0.0001..=100000.0),
                                );
                                ui.label(format!("{:.3}", t.scale.z));
                                if r.drag_stopped() || r.lost_focus() {
                                    *pending_history_record = true;
                                }
                                ui.end_row();
                            });
                            ui.separator();
                            egui::Grid::new("transform_grid").show(ui, |ui| {
                                ui.label("Location X");
                                let r = ui.add(
                                    egui::DragValue::new(&mut t.translation.x).speed(drag_speed),
                                );
                                ui.label(format_val(t.translation.x));
                                if r.drag_stopped() || r.lost_focus() {
                                    *pending_history_record = true;
                                }
                                ui.end_row();
                                ui.label("Location Y");
                                let r = ui.add(
                                    egui::DragValue::new(&mut t.translation.y).speed(drag_speed),
                                );
                                ui.label(format_val(t.translation.y));
                                if r.drag_stopped() || r.lost_focus() {
                                    *pending_history_record = true;
                                }
                                ui.end_row();
                                ui.label("Location Z");
                                let r = ui.add(
                                    egui::DragValue::new(&mut t.translation.z).speed(drag_speed),
                                );
                                ui.label(format_val(t.translation.z));
                                if r.drag_stopped() || r.lost_focus() {
                                    *pending_history_record = true;
                                }
                                ui.end_row();

                                let (rx_rad, ry_rad, rz_rad) =
                                    t.rotation.to_euler(glam::EulerRot::XYZ);

                                // Отображаем и редактируем градусы.
                                let mut rx = rx_rad.to_degrees();
                                let mut ry = ry_rad.to_degrees();
                                let mut rz = rz_rad.to_degrees();

                                let mut rot_changed = false;

                                ui.label("Rotation X");
                                let r =
                                    ui.add(egui::DragValue::new(&mut rx).speed(0.5).suffix("°"));
                                rot_changed |= r.changed();
                                if r.drag_stopped() || r.lost_focus() {
                                    *pending_history_record = true;
                                }
                                ui.end_row();

                                ui.label("Rotation Y");
                                let r =
                                    ui.add(egui::DragValue::new(&mut ry).speed(0.5).suffix("°"));
                                rot_changed |= r.changed();
                                if r.drag_stopped() || r.lost_focus() {
                                    *pending_history_record = true;
                                }
                                ui.end_row();

                                ui.label("Rotation Z");
                                let r =
                                    ui.add(egui::DragValue::new(&mut rz).speed(0.5).suffix("°"));
                                rot_changed |= r.changed();
                                if r.drag_stopped() || r.lost_focus() {
                                    *pending_history_record = true;
                                }
                                ui.end_row();

                                if rot_changed {
                                    t.rotation = glam::Quat::from_euler(
                                        glam::EulerRot::XYZ,
                                        rx.to_radians(),
                                        ry.to_radians(),
                                        rz.to_radians(),
                                    );
                                }
                            });
                        }
                    }
                } else if let Some(cursor_e) = cursor_e {
                    ui.label("3D Cursor:");
                    egui::Grid::new("cursor_grid").show(ui, |ui| {
                        if let Some(mut t) = scene.world.get_mut::<Transform>(cursor_e) {
                            ui.label("Location X");
                            let r = ui
                                .add(egui::DragValue::new(&mut t.translation.x).speed(drag_speed));
                            ui.label(format_val(t.translation.x));
                            if r.drag_stopped() || r.lost_focus() {
                                *pending_history_record = true;
                            }
                            ui.end_row();
                            ui.label("Location Y");
                            let r = ui
                                .add(egui::DragValue::new(&mut t.translation.y).speed(drag_speed));
                            ui.label(format_val(t.translation.y));
                            if r.drag_stopped() || r.lost_focus() {
                                *pending_history_record = true;
                            }
                            ui.end_row();
                            ui.label("Location Z");
                            let r = ui
                                .add(egui::DragValue::new(&mut t.translation.z).speed(drag_speed));
                            ui.label(format_val(t.translation.z));
                            if r.drag_stopped() || r.lost_focus() {
                                *pending_history_record = true;
                            }
                            ui.end_row();
                        }
                    });
                }
            });
        });
    }
}
