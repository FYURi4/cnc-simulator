use glam::{Mat4, Vec2, Vec3};
use bevy_ecs::prelude::*;
use engine_core::Scene;
use engine_render::mesh::Vertex;
use crate::app::App;
use crate::editor::*;
use crate::io::capture_snapshot;
use crate::types::*;

impl App {
    pub(super) fn render_frame(&mut self) -> bool {
                    let dt = Self::frame_dt(&mut self.input);
                    {
                        let state = self.state.as_mut().unwrap();
                        Self::update_camera(&self.input, state, dt);
                    }

                    self.scene.update();

                    {
                        let state = self.state.as_mut().unwrap();
                        for (entity, idx) in &self.render_map {
                            if let Some(mut mat) = self.scene.global_matrix(*entity) {
                                if Some(*entity) == self.cursor_entity {
                                    let cursor_world = mat.w_axis.truncate();
                                    let distance = state.camera.position.distance(cursor_world).max(1e-5);
                                    let world_view_height = 2.0 * distance * (state.camera.fovy * 0.5).tan();
                                    // Cursor geometry spans 0.4 world units. Keep it about 28 px wide.
                                    let cursor_scale = (world_view_height * 28.0
                                        / self.viewport_size.y.max(1.0) / 0.4)
                                        .clamp(1e-5, 1e5);
                                    mat = mat * Mat4::from_scale(Vec3::splat(cursor_scale));
                                }
                                state.set_object_transform(*idx, mat);
                            }
                        }
                        if self.edit_mode == EditMode::Edit {
                            if let Some(sel) = self.selected {
                                if let Some(custom) = self.scene.world.get::<CustomMesh>(sel) {
                                    if let Some(gt) = self.scene.global_matrix(sel) {
                                        let kind = self.scene.world.get::<MeshKindComp>(sel).unwrap().0;
                                        let faces = get_current_faces(&self.scene, sel, kind);
                                        let edges = get_current_edges(&faces);
                                        let handles = get_handles_for_mode(self.select_mode, &faces, &edges, &custom.vertices);
                                        
                                        // Compute proportional handle size from bounding box
                                        let mut min = Vec3::splat(f32::INFINITY);
                                        let mut max = Vec3::splat(f32::NEG_INFINITY);
                                        for v in &custom.vertices {
                                            let wp = gt.transform_point3(*v);
                                            min = min.min(wp);
                                            max = max.max(wp);
                                        }
                                        let bbox_size = (max - min).length().max(0.001);
                                        let handle_scale = bbox_size * 0.08; // 8% of bounding box diagonal
                                        
                                        for (i, &local_pos) in handles.iter().enumerate() {
                                            if i < self.edit_handles.len() {
                                                let world = gt.transform_point3(local_pos);
                                                state.set_object_transform(self.edit_handles[i], Mat4::from_translation(world) * Mat4::from_scale(Vec3::splat(handle_scale)));
                                                let is_selected = self.selected_elements.iter().any(|s| match s {
                                                    Selection::Vertex(idx) => self.select_mode == SelectMode::Vertex && *idx == i,
                                                    Selection::Edge(a, b) => {
                                                        if self.select_mode == SelectMode::Edge { edges.get(i) == Some(&(*a, *b)) } else { false }
                                                    },
                                                    Selection::Face(face_indices) => {
                                                        if self.select_mode == SelectMode::Face { faces.get(i) == Some(face_indices) } else { false }
                                                    }
                                                });
                                                if is_selected {
                                                    state.set_object_tint(self.edit_handles[i], [1.0, 0.5, 0.0, 1.0]);
                                                } else {
                                                    state.set_object_tint(self.edit_handles[i], [0.0, 1.0, 1.0, 1.0]);
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }

                    update_gizmo_positions(self.state.as_mut().unwrap(), self.gizmo.as_ref(), self.selected, &self.selected_elements, self.edit_mode, self.transform_mode, &self.scene, self.viewport_size);

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

                        if let Some((mode_label, axis_label, value)) = &numeric_overlay {
                            egui::Area::new(egui::Id::new("numeric_transform_input"))
                                .order(egui::Order::Foreground)
                                .fixed_pos(egui::pos2(
                                    (self.viewport_size.x * 0.5 - 110.0).max(8.0),
                                    (self.viewport_size.y - 76.0).max(8.0),
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

                        egui::TopBottomPanel::top("top_bar").show(ctx, |ui| {
                            egui::MenuBar::new().ui(ui, |ui| {
                                ui.menu_button("File", |ui| {
                                    if ui.button("💾 Save").clicked() { ui_action = UiAction::Save; ui.close(); }
                                    if ui.button("💾 Save As...").clicked() { self.show_save_as = true; ui.close(); }
                                    if ui.button("📂 Load").clicked() { ui_action = UiAction::Load; ui.close(); }
                                    if ui.button("⚙ Load G-code").clicked() { ui_action = UiAction::LoadGcode; ui.close(); }
                                });
                                ui.menu_button("Add", |ui| {
                                    if ui.button("Cube").clicked() { ui_action = UiAction::AddCube; ui.close(); }
                                    if ui.button("Plane").clicked() { ui_action = UiAction::AddPlane; ui.close(); }
                                    if ui.button("Cylinder").clicked() { ui_action = UiAction::AddCylinder; ui.close(); }
                                    if ui.button("Sphere").clicked() { ui_action = UiAction::AddSphere; ui.close(); }
                                    if ui.button("Pyramid").clicked() { ui_action = UiAction::AddPyramid; ui.close(); }
                                });
                                ui.menu_button("Edit", |ui| {
                                    if ui.button("Undo (Ctrl+Z)").clicked() { ui_action = UiAction::Undo; ui.close(); }
                                    if ui.button("Redo (Ctrl+Y)").clicked() { ui_action = UiAction::Redo; ui.close(); }
                                    ui.separator();
                                    if ui.button("Copy (Ctrl+C)").clicked() { ui_action = UiAction::CopySelected; ui.close(); }
                                    if ui.button("Paste (Ctrl+V)").clicked() { ui_action = UiAction::PasteSelected; ui.close(); }
                                });
                                ui.menu_button("Space", |ui| {
                                    ui.heading("Grid Settings");
                                    ui.separator();
                                    ui.checkbox(&mut self.grid_settings.visible, "Visible");
                                    ui.add(egui::Slider::new(&mut grid_extent_ui, 10.0..=500.0).logarithmic(true).text("Size"));
                                    let grid_mm = (grid_extent_ui * 1000.0).round() as i32;
                                    let grid_m = grid_extent_ui / 1000.0;
                                    ui.horizontal(|ui| {
                                        ui.label("Unit:");
                                        ui.selectable_value(&mut self.grid_settings.unit, GridUnit::Mm, "mm");
                                        ui.selectable_value(&mut self.grid_settings.unit, GridUnit::M, "m");
                                    });
                                    match self.grid_settings.unit {
                                        GridUnit::Mm => { ui.label(format!("{} mm", grid_mm)); }
                                        GridUnit::M => { ui.label(format!("{:.3} m", grid_m)); }
                                    }
                                    ui.separator();
                                    ui.label("Grid Step (mm):");
                                    ui.horizontal(|ui| {
                                        ui.label("X:");
                                        let _r = ui.add(egui::DragValue::new(&mut self.grid_settings.step_x).speed(0.5).range(0.1..=100000.0).suffix(" mm"));
                                    });
                                    ui.horizontal(|ui| {
                                        ui.label("Y:");
                                        let r = ui.add(egui::DragValue::new(&mut self.grid_settings.step_y).speed(0.5).range(0.1..=100000.0).suffix(" mm"));
                                    });
                                    ui.separator();
                                    ui.label("Color:");
                                    let c = &mut self.grid_settings.color;
                                    if ui.button("🎨").clicked() {
                                        self.show_color_picker = !self.show_color_picker;
                                        if self.show_color_picker {
                                            self.color_picker_hex = format!("{:02X}{:02X}{:02X}",
                                                (c[0] * 255.0) as u8, (c[1] * 255.0) as u8, (c[2] * 255.0) as u8);
                                        }
                                    }
                                    ui.label(format!("{:02X} {:02X} {:02X}", (c[0]*255.0) as u8, (c[1]*255.0) as u8, (c[2]*255.0) as u8));
                                    ui.label("Type:");
                                    ui.horizontal(|ui| {
                                        ui.selectable_value(&mut self.grid_settings.grid_type, GridType::Grid, "Grid");
                                        ui.selectable_value(&mut self.grid_settings.grid_type, GridType::Dots, "Dots");
                                        ui.selectable_value(&mut self.grid_settings.grid_type, GridType::DotsMajor, "Dots Major");
                                        ui.selectable_value(&mut self.grid_settings.grid_type, GridType::DotsOnly, "Dots Only");
                                    });
                                });
                                ui.menu_button("View", |ui| {
                                    egui::ComboBox::from_id_salt("view_direction")
                                        .selected_text(ui_view.label())
                                        .show_ui(ui, |ui| {
                                            ui.selectable_value(&mut ui_view, ViewDirection::Top, "Top (Y+)");
                                            ui.selectable_value(&mut ui_view, ViewDirection::Bottom, "Bottom (Y-)");
                                            ui.selectable_value(&mut ui_view, ViewDirection::Front, "Front (Z+)");
                                            ui.selectable_value(&mut ui_view, ViewDirection::Back, "Back (Z-)");
                                            ui.selectable_value(&mut ui_view, ViewDirection::Left, "Left (X-)");
                                            ui.selectable_value(&mut ui_view, ViewDirection::Right, "Right (X+)");
                                            ui.selectable_value(&mut ui_view, ViewDirection::Iso, "Iso");
                                        });
                                    if self.selected.is_some() {
                                        if ui.button("📍 Set Origin").clicked() { ui_action = UiAction::SetOriginToCursorConfirmed; ui.close(); }
                                    }
                                });
                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    ui.toggle_value(&mut snap.enabled, "🧲 Snap");
                                    egui::ComboBox::from_id_salt("snap_target")
                                        .selected_text(format!("{:?}", snap.target))
                                        .show_ui(ui, |ui| {
                                            ui.selectable_value(&mut snap.target, SnapTarget::Increment, "Increment");
                                            ui.selectable_value(&mut snap.target, SnapTarget::Grid, "Grid");
                                            ui.selectable_value(&mut snap.target, SnapTarget::Origin, "Origin");
                                            ui.selectable_value(&mut snap.target, SnapTarget::Vertex, "Vertex");
                                            ui.selectable_value(&mut snap.target, SnapTarget::Edge, "Edge");
                                        });
                                });
                            });
                        });

                        egui::SidePanel::left("tools")
                            .resizable(false)
                            .exact_width(40.0)
                            .show(ctx, |ui| {
                                ui.vertical_centered(|ui| {
                                    ui.add_space(10.0);
                                    if edit_m == EditMode::Object {
                                        if ui.selectable_label(mode == TransformMode::Translate, "✥").clicked() { ui_action = UiAction::SetMode(TransformMode::Translate); }
                                        if ui.selectable_label(mode == TransformMode::Rotate, "⟲").clicked() { ui_action = UiAction::SetMode(TransformMode::Rotate); }
                                        if ui.selectable_label(mode == TransformMode::Scale, "⤢").clicked() { ui_action = UiAction::SetMode(TransformMode::Scale); }
                                    } else {
                                        if ui.selectable_label(sel_m == SelectMode::Vertex, "1").on_hover_text("Vertex").clicked() { ui_sel_m = SelectMode::Vertex; }
                                        if ui.selectable_label(sel_m == SelectMode::Edge, "2").on_hover_text("Edge").clicked() { ui_sel_m = SelectMode::Edge; }
                                        if ui.selectable_label(sel_m == SelectMode::Face, "3").on_hover_text("Face").clicked() { ui_sel_m = SelectMode::Face; }
                                        ui.separator();
                                        if ui.selectable_label(sel_tool == SelectTool::Click, "🖱️").on_hover_text("Select Click").clicked() { ui_sel_tool = SelectTool::Click; }
                                        if ui.selectable_label(sel_tool == SelectTool::Box, "⬚").on_hover_text("Select Box").clicked() { ui_sel_tool = SelectTool::Box; }
                                    }
                                });
                            });

                        egui::SidePanel::right("right_panel")
                            .resizable(true)
                            .default_width(280.0)
                            .show(ctx, |ui| {
                                egui::TopBottomPanel::top("outliner")
                                    .resizable(true)
                                    .default_height(200.0)
                                    .show_inside(ui, |ui| {
                                        ui.heading("Outliner");
                                        ui.separator();
                                        egui::ScrollArea::vertical().show(ui, |ui| {
                                            let roots = {
                                                let mut roots_vec = Vec::new();
                                                let mut q = scene.world.query_filtered::<Entity, bevy_ecs::query::Without<Parent>>();
                                                for e in q.iter(&scene.world) { roots_vec.push(e); }
                                                roots_vec.into_iter().filter(|e| scene.get_name(*e) != Some("3D Cursor")).collect::<Vec<_>>()
                                            };
                                            for root in roots {
                                                show_node(ui, &mut scene, root, selected, &mut editing, &mut new_selection);
                                            }
                                        });
                                    });

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

                        if self.delete_confirm {
                            let dialog_pos = egui::pos2(
                                (self.delete_confirm_pos.x + 15.0).clamp(0.0, ctx.viewport_rect().max.x - 200.0),
                                (self.delete_confirm_pos.y + 15.0).clamp(0.0, ctx.viewport_rect().max.y - 80.0),
                            );
                            egui::Area::new(egui::Id::new("delete_confirm"))
                                .fixed_pos(dialog_pos)
                                .show(ctx, |ui| {
                                    let layout = egui::Layout::centered_and_justified(egui::Direction::LeftToRight);
                                    ui.allocate_ui_with_layout(egui::Vec2::ZERO, layout, |ui| {
                                        ui.horizontal(|ui| {
                                            ui.label("Удалить объект?");
                                            if ui.button("Да").clicked() {
                                                ui_action = UiAction::ConfirmDelete;
                                                self.delete_confirm = false;
                                                ui.close();
                                            }
                                            if ui.button("Нет").clicked() {
                                                self.delete_confirm = false;
                                                ui.close();
                                            }
                                        });
                                    });
                                });
                        }

                        if self.show_save_as {
                            egui::Window::new("Save As")
                                .default_size([320.0, 100.0])
                                .show(ctx, |ui| {
                                    ui.label("File path:");
                                    ui.text_edit_singleline(&mut self.save_as_path);
                                    ui.separator();
                                    ui.horizontal(|ui| {
                                        if ui.button("Save").clicked() {
                                            ui_action = UiAction::SaveAs;
                                            self.show_save_as = false;
                                        }
                                        if ui.button("Cancel").clicked() {
                                            self.show_save_as = false;
                                        }
                                    });
                                });
                        }

                        if self.show_color_picker {
                            egui::Window::new("Grid Color")
                                .default_size([220.0, 160.0])
                                .show(ctx, |ui| {
                                    let c = &mut self.grid_settings.color;
                                    let mut rgb = [
                                        (c[0] * 255.0) as u8,
                                        (c[1] * 255.0) as u8,
                                        (c[2] * 255.0) as u8,
                                    ];
                                    if ui.color_edit_button_srgb(&mut rgb).changed() {
                                        c[0] = rgb[0] as f32 / 255.0;
                                        c[1] = rgb[1] as f32 / 255.0;
                                        c[2] = rgb[2] as f32 / 255.0;
                                        self.color_picker_hex = format!("{:02X}{:02X}{:02X}", rgb[0], rgb[1], rgb[2]);
                                    }
                                    ui.separator();
                                    ui.horizontal(|ui| {
                                        ui.label("HEX:");
                                        let resp = ui.add(egui::TextEdit::singleline(&mut self.color_picker_hex)
                                            .font(egui::FontId::monospace(12.0)));
                                        if resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                                            let hex = self.color_picker_hex.trim().trim_start_matches('#');
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
                                        self.show_color_picker = false;
                                        self.color_picker_hex = format!("{:02X}{:02X}{:02X}", rgb[0], rgb[1], rgb[2]);
                                    }
                                });
                        }
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
