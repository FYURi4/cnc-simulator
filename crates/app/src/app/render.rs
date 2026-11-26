use glam::{Mat4, Vec2, Vec3};
use bevy_ecs::prelude::*;
use engine_core::Scene;
use engine_render::mesh::Vertex;
use crate::app::App;
mod scene;
mod top_bar;
mod tools_panel;
mod numeric_overlay;
mod dialogs;
mod outliner;
use crate::editor::*;
use crate::io::capture_snapshot;
use crate::types::*;

impl App {
    pub(super) fn render_frame(&mut self) -> bool {
        self.update_scene_render_state();

                    let mut ui_action = UiAction::None;
                    let mut scene = std::mem::replace(&mut self.scene, Scene::new());
                    let selected = self.selected;
                    let mut editing = self.editing_entity;
                    let mut new_selection = None;
                    let mode = self.transform_mode;
                    let edit_m = self.edit_mode;
                    let sel_m = self.select_mode;
                    let sel_tool = self.select_tool;
                    let can_undo = self.history.can_undo();
                    let can_redo = self.history.can_redo();
                    let cursor_e = self.cursor_entity;
                    let snap = &mut self.snap;
                    let mut ui_sel_m = self.select_mode;
                    let mut ui_sel_tool = self.select_tool;
                    let mut ui_view = self.current_view;
                    let drag_start = self.drag_start;
                    let drag_end = self.drag_end;
                    let selected_elements = self.selected_elements.clone();
                    let render_map_clone = self.render_map.clone();
                    let numeric_overlay = self.transform_operation.as_ref().map(|op| {
                        let axis = match op.axis {
                            Some(a) if a == Vec3::X => "X",
                            Some(a) if a == Vec3::Y => "Y",
                            Some(a) if a == Vec3::Z => "Z",
                            _ => "Free",
                        };
                        (format!("{:?}", op.mode), axis.to_string(), op.numeric_input.clone())
                    });
                    let state = self.state.as_mut().unwrap();
                    let mut pending_vertex_update: Option<(Entity, Vec<Vertex>)> = None;
                    let mut grid_extent_ui = state.grid_extent;

                    // FIX: флаг "было ли изменение через numeric input".
                    // Ставится внутри closure, применяется после.
                    let mut pending_history_record = false;
                    let grid_unit = self.grid_settings.unit;

                    let result = state.render_with_ui(|ctx| {
                        let unit = grid_unit;
                        let format_val = |v: f32| -> String {
                            match unit {
                                GridUnit::Mm => format!("{} mm", (v * 1000.0) as i32),
                                GridUnit::M => format!("{:.3} m", v),
                            }
                        };
                        let drag_speed = match unit {
                            GridUnit::Mm => 0.02,
                            GridUnit::M => 0.00002,
                        };

                        self.draw_numeric_overlay(ctx, numeric_overlay.as_ref());

                        self.draw_top_bar(ctx, &mut ui_action, &mut grid_extent_ui, &mut ui_view, snap);

                        Self::draw_tools_panel(ctx, edit_m, mode, sel_m, sel_tool, &mut ui_action, &mut ui_sel_m, &mut ui_sel_tool);

                        egui::SidePanel::right("right_panel")
                            .resizable(true)
                            .default_width(280.0)
                            .show(ctx, |ui| {
                                Self::draw_outliner(ui, &mut scene, selected, &mut editing, &mut new_selection);

                                egui::CentralPanel::default().show_inside(ui, |ui| {
                                    ui.heading(if edit_m == EditMode::Object { "Properties" } else { "Edit Mode" });
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
                                                    for s in &selected_elements {
                                                        match s {
                                                            Selection::Vertex(i) => { unique_verts.insert(*i); }
                                                            Selection::Edge(a, b) => { unique_verts.insert(*a); unique_verts.insert(*b); }
                                                            Selection::Face(indices) => { for &i in indices { unique_verts.insert(i); } }
                                                        }
                                                    }
                                                    if !unique_verts.is_empty() {
                                                        let mut center = Vec3::ZERO;
                                                        for &i in &unique_verts { center += custom.vertices[i]; }
                                                        center /= unique_verts.len() as f32;
                                                        let mut new_center = center;
                                                        ui.label("Selection Center:");
                                                        let mut changed = false;
                                                        egui::Grid::new("sel_grid").show(ui, |ui| {
                                                            ui.label("Location X");
                                                            let r = ui.add(egui::DragValue::new(&mut new_center.x).speed(drag_speed));
                                                            ui.label(format_val(new_center.x));
                                                            changed |= r.changed();
                                                            if r.drag_stopped() || r.lost_focus() { pending_history_record = true; }
                                                            ui.end_row();
                                                            ui.label("Location Y");
                                                            let r = ui.add(egui::DragValue::new(&mut new_center.y).speed(drag_speed));
                                                            ui.label(format_val(new_center.y));
                                                            changed |= r.changed();
                                                            if r.drag_stopped() || r.lost_focus() { pending_history_record = true; }
                                                            ui.end_row();
                                                            ui.label("Location Z");
                                                            let r = ui.add(egui::DragValue::new(&mut new_center.z).speed(drag_speed));
                                                            ui.label(format_val(new_center.z));
                                                            changed |= r.changed();
                                                            if r.drag_stopped() || r.lost_focus() { pending_history_record = true; }
                                                            ui.end_row();
                                                        });
                                                        if changed {
                                                            let delta = new_center - center;
                                                            for &i in &unique_verts { custom.vertices[i] += delta; }
                                                            let kind = scene.world.get::<MeshKindComp>(sel).unwrap().0;
                                                            let new_vertices = build_mesh_vertices(&scene, sel, kind, &selected_elements);
                                                            if render_map_clone.iter().any(|(e, _)| *e == sel) {
                                                                pending_vertex_update = Some((sel, new_vertices));
                                                            }
                                                        }
                                                    }
                                                }
                                            } else {
                                                if edit_m == EditMode::Object {
                                                    ui.menu_button("Set Origin", |ui| {
                                                        if ui.button("To 3D Cursor").clicked() { ui_action = UiAction::SetOriginToCursor; ui.close(); }
                                                        if ui.button("To Geometry").clicked() { ui_action = UiAction::SetOriginToGeometry; ui.close(); }
                                                    });
                                                    ui.separator();
                                                }

                                                if let Some(mut t) = scene.world.get_mut::<Transform>(sel) {
                                                    ui.label("Transform:");
                                                    ui.label("Scale");
                                                    egui::Grid::new("scale_grid").num_columns(3).show(ui, |ui| {
ui.label("Scale X");
                                                        let r = ui.add(egui::DragValue::new(&mut t.scale.x).speed(0.01).range(0.0001..=100000.0));
                                                        ui.label(format!("{:.3}", t.scale.x));
                                                        if r.drag_stopped() || r.lost_focus() { pending_history_record = true; }
                                                        ui.end_row();

                                                        ui.label("Scale Y");
                                                        let r = ui.add(egui::DragValue::new(&mut t.scale.y).speed(0.01).range(0.0001..=100000.0));
                                                        ui.label(format!("{:.3}", t.scale.y));
                                                        if r.drag_stopped() || r.lost_focus() { pending_history_record = true; }
                                                        ui.end_row();

                                                        ui.label("Scale Z");
                                                        let r = ui.add(egui::DragValue::new(&mut t.scale.z).speed(0.01).range(0.0001..=100000.0));
                                                        ui.label(format!("{:.3}", t.scale.z));
                                                        if r.drag_stopped() || r.lost_focus() { pending_history_record = true; }
                                                        ui.end_row();
                                                    });
                                                    ui.separator();
                                                    egui::Grid::new("transform_grid").show(ui, |ui| {
                                                        ui.label("Location X");
                                                        let r = ui.add(egui::DragValue::new(&mut t.translation.x).speed(drag_speed));
                                                        ui.label(format_val(t.translation.x));
                                                        if r.drag_stopped() || r.lost_focus() { pending_history_record = true; }
                                                        ui.end_row();
                                                        ui.label("Location Y");
                                                        let r = ui.add(egui::DragValue::new(&mut t.translation.y).speed(drag_speed));
                                                        ui.label(format_val(t.translation.y));
                                                        if r.drag_stopped() || r.lost_focus() { pending_history_record = true; }
                                                        ui.end_row();
                                                        ui.label("Location Z");
                                                        let r = ui.add(egui::DragValue::new(&mut t.translation.z).speed(drag_speed));
                                                        ui.label(format_val(t.translation.z));
                                                        if r.drag_stopped() || r.lost_focus() { pending_history_record = true; }
                                                        ui.end_row();

                                                        let (rx_rad, ry_rad, rz_rad) =
                                                            t.rotation.to_euler(glam::EulerRot::XYZ);

                                                        // Отображаем и редактируем градусы.
                                                        let mut rx = rx_rad.to_degrees();
                                                        let mut ry = ry_rad.to_degrees();
                                                        let mut rz = rz_rad.to_degrees();

                                                        let mut rot_changed = false;

                                                        ui.label("Rotation X");
                                                        let r = ui.add(
                                                            egui::DragValue::new(&mut rx)
                                                                .speed(0.5)
                                                                .suffix("°")
                                                        );
                                                        rot_changed |= r.changed();
                                                        if r.drag_stopped() || r.lost_focus() {
                                                            pending_history_record = true;
                                                        }
                                                        ui.end_row();

                                                        ui.label("Rotation Y");
                                                        let r = ui.add(
                                                            egui::DragValue::new(&mut ry)
                                                                .speed(0.5)
                                                                .suffix("°")
                                                        );
                                                        rot_changed |= r.changed();
                                                        if r.drag_stopped() || r.lost_focus() {
                                                            pending_history_record = true;
                                                        }
                                                        ui.end_row();

                                                        ui.label("Rotation Z");
                                                        let r = ui.add(
                                                            egui::DragValue::new(&mut rz)
                                                                .speed(0.5)
                                                                .suffix("°")
                                                        );
                                                        rot_changed |= r.changed();
                                                        if r.drag_stopped() || r.lost_focus() {
                                                            pending_history_record = true;
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
                                                    let r = ui.add(egui::DragValue::new(&mut t.translation.x).speed(drag_speed));
                                                    ui.label(format_val(t.translation.x));
                                                    if r.drag_stopped() || r.lost_focus() { pending_history_record = true; }
                                                    ui.end_row();
                                                    ui.label("Location Y");
                                                    let r = ui.add(egui::DragValue::new(&mut t.translation.y).speed(drag_speed));
                                                    ui.label(format_val(t.translation.y));
                                                    if r.drag_stopped() || r.lost_focus() { pending_history_record = true; }
                                                    ui.end_row();
                                                    ui.label("Location Z");
                                                    let r = ui.add(egui::DragValue::new(&mut t.translation.z).speed(drag_speed));
                                                    ui.label(format_val(t.translation.z));
                                                    if r.drag_stopped() || r.lost_focus() { pending_history_record = true; }
                                                    ui.end_row();
                                                }
                                            });
                                        }
                                    });
                                });
                            });

                        self.draw_dialogs(ctx, drag_start, drag_end, &mut ui_action);
                    });

                    let gs = &mut self.grid_settings;
                    let grid_type_val = match gs.grid_type {
                        GridType::Grid => 0.0,
                        GridType::Dots => 1.0,
                        GridType::DotsMajor => 2.0,
                        GridType::DotsOnly => 3.0,
                    };
                    let needs_update = state.grid_color != gs.color[..3] 
                        || state.grid_type != grid_type_val 
                        || state.grid_visible != gs.visible
                        || state.grid_step_x != gs.step_x / 1000.0
                        || state.grid_step_y != gs.step_y / 1000.0;
                    if needs_update {
                        state.grid_color = [gs.color[0], gs.color[1], gs.color[2]];
                        state.grid_type = grid_type_val;
                        state.grid_visible = gs.visible;
                        state.grid_step_x = gs.step_x / 1000.0;
                        state.grid_step_y = gs.step_y / 1000.0;
                        state.update_camera();
                    }
                    if state.grid_extent != grid_extent_ui {
                        state.grid_extent = grid_extent_ui;
                        state.update_camera();
                    }

                    if ui_view != self.current_view {
                        self.current_view = ui_view;
                        state.camera.set_view_direction(ui_view.dir());
                        state.update_camera();
                    }

                    self.scene = scene;
                    self.editing_entity = editing;

                    // FIX: запись в историю после numeric input
                    if pending_history_record {
                        self.history.record(capture_snapshot(&mut self.scene));
                    }

                    if let Some((sel, verts)) = pending_vertex_update {
                        if let Some((_, r_idx)) = self.render_map.iter().find(|(e, _)| *e == sel) {
                            self.state.as_mut().unwrap().update_object_vertices(*r_idx, &verts);
                        }
                    }

                    if self.select_mode != ui_sel_m {
                        self.select_mode = ui_sel_m;
                        self.selected_elements.clear();
                        update_edit_handles(self.state.as_mut().unwrap(), &mut self.scene, self.edit_mode, self.select_mode, self.selected, &mut self.edit_handles);
                    }
                    self.select_tool = ui_sel_tool;

                    if let Err(e) = result {
                        eprintln!("render error: {e:?}");
                        return false;
                    }

                    if let Some(new_sel) = new_selection {
                        let old = self.selected;
                        if old != Some(new_sel) {
                            self.selected = Some(new_sel);
                            set_selection(self.state.as_mut().unwrap(), &self.render_map, old, self.selected);
                            self.edit_mode = EditMode::Object;
                            self.selected_elements.clear();
                            update_edit_handles(self.state.as_mut().unwrap(), &mut self.scene, self.edit_mode, self.select_mode, self.selected, &mut self.edit_handles);
                        }
                    }

                    self.apply_ui_action(ui_action);

                    if self.pending_undo {
                        self.pending_undo = false;
                        self.do_undo();
                    }
                    if self.pending_redo {
                        self.pending_redo = false;
                        self.do_redo();
                    }
                
        true
    }
}
