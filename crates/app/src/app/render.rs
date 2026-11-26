use glam::Vec3;
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
mod properties;
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
                    let viewport_size = self.viewport_size;
                    let grid_settings = &mut self.grid_settings;
                    let show_save_as = &mut self.show_save_as;
                    let show_color_picker = &mut self.show_color_picker;
                    let color_picker_hex = &mut self.color_picker_hex;
                    let save_as_path = &mut self.save_as_path;
                    let delete_confirm = &mut self.delete_confirm;
                    let delete_confirm_pos = self.delete_confirm_pos;
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
                    let grid_unit = grid_settings.unit;

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

                        Self::draw_numeric_overlay(ctx, viewport_size, numeric_overlay.as_ref());

                        Self::draw_top_bar(ctx, &mut ui_action, &mut grid_extent_ui, &mut ui_view, snap, grid_settings, show_save_as, show_color_picker, color_picker_hex, selected);

                        Self::draw_tools_panel(ctx, edit_m, mode, sel_m, sel_tool, &mut ui_action, &mut ui_sel_m, &mut ui_sel_tool);

                        egui::SidePanel::right("right_panel")
                            .resizable(true)
                            .default_width(280.0)
                            .show(ctx, |ui| {
                                Self::draw_outliner(ui, &mut scene, selected, &mut editing, &mut new_selection);

                                Self::draw_properties_panel(ui, &mut scene, selected, edit_m, &selected_elements, drag_speed, &format_val, &mut pending_history_record, &mut ui_action, &render_map_clone, &mut pending_vertex_update, cursor_e);
                            });

                        Self::draw_dialogs(ctx, drag_start, drag_end, &mut ui_action, delete_confirm, delete_confirm_pos, show_save_as, save_as_path, show_color_picker, color_picker_hex, &mut grid_settings.color);
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
