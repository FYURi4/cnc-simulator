use egui;
use glam::Vec2;
use crate::app::App;
use crate::types::UiAction;

impl App {
    pub(super) fn draw_overlays(
        &mut self,
        ctx: &egui::Context,
        numeric_overlay: Option<&(String, String, String)>,
        drag_start: Option<Vec2>,
        drag_end: Option<Vec2>,
        ui_action: &mut UiAction,
    ) {
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

                        self.draw_top_bar(ctx, &mut ui_action, &mut grid_extent_ui, &mut ui_view, snap);

                        Self::draw_tools_panel(ctx, edit_m, mode, sel_m, sel_tool, &mut ui_action, &mut ui_sel_m, &mut ui_sel_tool);

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
                                                        if ui.button("To 3D Cursor").clicked() { *ui_action = UiAction::SetOriginToCursor; ui.close(); }
                                                        if ui.button("To Geometry").clicked() { *ui_action = UiAction::SetOriginToGeometry; ui.close(); }
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
                                                *ui_action = UiAction::ConfirmDelete;
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
                                            *ui_action = UiAction::SaveAs;
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
    }
}
