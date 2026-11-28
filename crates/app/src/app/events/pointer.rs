use crate::app::App;
use crate::editor::*;
use crate::geometry::*;
use crate::io::*;
use crate::types::*;
use engine_core::{Parent, Transform};
use glam::{Mat4, Quat, Vec2, Vec3};
use winit::{
    dpi::PhysicalPosition,
    event::{ElementState, MouseButton},
    keyboard::KeyCode,
};

impl App {
    pub(super) fn handle_cursor_moved(&mut self, position: PhysicalPosition<f64>) {
        let new_pos = Vec2::new(position.x as f32, position.y as f32);
        let delta = new_pos - self.input.cursor_pos;
        self.input.cursor_pos = new_pos;

        if self.transform_operation.is_some() && !self.input.mmb_down && !self.input.rmb_down {
            self.update_transform_operation(new_pos);
        }

        if self.input.mmb_down {
            let state = self.state.as_mut().unwrap();
            if self.input.keys.contains(&KeyCode::ShiftLeft)
                || self.input.keys.contains(&KeyCode::ShiftRight)
            {
                state.camera.pan(delta, self.viewport_size.y);
            } else {
                let sensitivity = 0.005;
                state
                    .camera
                    .orbit(delta.x * sensitivity, -delta.y * sensitivity);
            }
            state.update_camera();
        }

        if self.input.rmb_down {
            let sensitivity = 0.005;
            let state = self.state.as_mut().unwrap();
            state
                .camera
                .rotate(delta.x * sensitivity, -delta.y * sensitivity);
            state.update_camera();
        }

        if self.input.lmb_down {
            if self.drag_start.is_some() {
                self.drag_end = Some(new_pos);
            } else if let Some(drag) = self.drag.clone() {
                let d = new_pos - drag.start_mouse_screen;
                if d.length_squared() > 4.0 {
                    self.drag_moved = true;
                }

                if self.edit_mode == EditMode::Edit {
                    if let Some(start_verts) = &drag.start_custom_verts {
                        let len_sq = drag.screen_axis_dir.length_squared();
                        if len_sq > 1.0 {
                            let proj =
                                d.dot(drag.screen_axis_dir) / len_sq * drag.axis_world_length;
                            let mut new_world = drag.start_world_pos + drag.axis * proj;
                            if self.snap.enabled {
                                let state = self.state.as_ref().unwrap();
                                new_world = apply_snap(
                                    new_world,
                                    drag.start_world_pos,
                                    drag.axis,
                                    &self.snap,
                                    &self.grid_settings,
                                    &self.scene,
                                    &self.render_map,
                                    &state.objects,
                                    drag.entity,
                                );
                            }
                            let obj_gt = self
                                .scene
                                .global_matrix(drag.entity)
                                .unwrap_or(Mat4::IDENTITY);
                            let local_new = obj_gt.inverse().transform_point3(new_world);
                            let delta_local =
                                local_new - obj_gt.inverse().transform_point3(drag.start_world_pos);
                            let mut custom = start_verts.clone();
                            for &idx in &drag.selected_indices {
                                custom[idx] += delta_local;
                            }
                            if let Some(mut cm) =
                                self.scene.world.get_mut::<CustomMesh>(drag.entity)
                            {
                                cm.vertices = custom.clone();
                            }
                            if let Some(kind) = self
                                .scene
                                .world
                                .get::<MeshKindComp>(drag.entity)
                                .map(|m| m.0)
                            {
                                let new_vertices = build_mesh_vertices(
                                    &self.scene,
                                    drag.entity,
                                    kind,
                                    &self.selected_elements,
                                );
                                if let Some((_, r_idx)) =
                                    self.render_map.iter().find(|(e, _)| *e == drag.entity)
                                {
                                    self.state
                                        .as_mut()
                                        .unwrap()
                                        .update_object_vertices(*r_idx, &new_vertices);
                                }
                            }
                        }
                    }
                } else {
                    match drag.mode {
                        TransformMode::Translate => {
                            let len_sq = drag.screen_axis_dir.length_squared();
                            if len_sq > 1.0 {
                                let proj =
                                    d.dot(drag.screen_axis_dir) / len_sq * drag.axis_world_length;
                                let mut new_world = drag.start_world_pos + drag.axis * proj;
                                if self.snap.enabled {
                                    let state = self.state.as_ref().unwrap();
                                    new_world = apply_snap(
                                        new_world,
                                        drag.start_world_pos,
                                        drag.axis,
                                        &self.snap,
                                        &self.grid_settings,
                                        &self.scene,
                                        &self.render_map,
                                        &state.objects,
                                        drag.entity,
                                    );
                                }
                                let local =
                                    if let Some(p) = self.scene.world.get::<Parent>(drag.entity) {
                                        if let Some(pg) = self.scene.global_matrix(p.0) {
                                            pg.inverse().transform_point3(new_world)
                                        } else {
                                            new_world
                                        }
                                    } else {
                                        new_world
                                    };
                                if let Some(mut t) =
                                    self.scene.world.get_mut::<Transform>(drag.entity)
                                {
                                    t.translation = local;
                                }
                            }
                        }
                        TransformMode::Rotate => {
                            let start_vec = drag.start_mouse_screen - drag.pivot_screen;
                            let cur_vec = new_pos - drag.pivot_screen;
                            let mut delta_a =
                                cur_vec.y.atan2(cur_vec.x) - start_vec.y.atan2(start_vec.x);
                            if delta_a > PI {
                                delta_a -= 2.0 * PI;
                            }
                            if delta_a < -PI {
                                delta_a += 2.0 * PI;
                            }
                            delta_a = -delta_a;
                            if self.snap.enabled {
                                let step = 15.0_f32.to_radians();
                                delta_a = (delta_a / step).round() * step;
                            }
                            let new_rot = self.world_rotation_delta_local(
                                drag.entity,
                                drag.start_rotation,
                                drag.axis,
                                delta_a,
                            );
                            if let Some(mut t) = self.scene.world.get_mut::<Transform>(drag.entity)
                            {
                                t.rotation = new_rot;
                            }
                        }
                        TransformMode::Scale => {
                            let len_sq = drag.screen_axis_dir.length_squared();
                            if len_sq > 1.0 {
                                let proj =
                                    d.dot(drag.screen_axis_dir) / len_sq * drag.axis_world_length;
                                let axis_idx = drag.axis_index;
                                let start_v = axis_comp(drag.start_scale, axis_idx);
                                let new_v = (start_v + proj).max(0.01);
                                let mut new_scale = drag.start_scale;
                                set_axis_comp(&mut new_scale, axis_idx, new_v);
                                if let Some(mut t) =
                                    self.scene.world.get_mut::<Transform>(drag.entity)
                                {
                                    t.scale = new_scale;
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    pub(super) fn handle_mouse_input(&mut self, btn_state: ElementState, button: MouseButton) {
        match button {
            MouseButton::Left => {
                if btn_state == ElementState::Pressed {
                    self.input.lmb_down = true;
                    self.drag_moved = false;

                    if self.edit_mode == EditMode::Edit && self.select_tool == SelectTool::Box {
                        self.drag_start = Some(self.input.cursor_pos);
                        self.drag_end = Some(self.input.cursor_pos);
                    } else {
                        // An active transform remains a preview until Enter is pressed.
                        // Do not let a mouse click silently commit it.
                        if self.transform_operation.is_some() {
                            return;
                        }
                        handle_lmb_press(
                            self.state.as_mut().unwrap(),
                            &self.input,
                            self.viewport_size,
                            &mut self.selected,
                            &mut self.selected_elements,
                            &mut self.drag,
                            self.transform_mode,
                            self.edit_mode,
                            self.select_mode,
                            &mut self.scene,
                            &self.render_map,
                            self.cursor_entity,
                            self.transform_space,
                        );
                    }
                } else {
                    self.input.lmb_down = false;
                    if let Some(start) = self.drag_start.take() {
                        if let Some(end) = self.drag_end.take() {
                            let min_x = start.x.min(end.x);
                            let max_x = start.x.max(end.x);
                            let min_y = start.y.min(end.y);
                            let max_y = start.y.max(end.y);
                            let ctrl = self.input.keys.contains(&KeyCode::ControlLeft)
                                || self.input.keys.contains(&KeyCode::ShiftLeft);
                            if !ctrl {
                                self.selected_elements.clear();
                            }
                            if let Some(sel) = self.selected {
                                if let Some(custom) = self.scene.world.get::<CustomMesh>(sel) {
                                    if let Some(gt) = self.scene.global_matrix(sel) {
                                        let kind =
                                            self.scene.world.get::<MeshKindComp>(sel).unwrap().0;
                                        let faces = get_current_faces(&self.scene, sel, kind);
                                        let edges = get_current_edges(&faces);
                                        match self.select_mode {
                                            SelectMode::Vertex => {
                                                for (i, &v) in custom.vertices.iter().enumerate() {
                                                    let world_v = gt.transform_point3(v);
                                                    let screen_v = world_to_screen(
                                                        &self.state.as_ref().unwrap().camera,
                                                        self.viewport_size,
                                                        world_v,
                                                    );
                                                    if screen_v.x >= min_x
                                                        && screen_v.x <= max_x
                                                        && screen_v.y >= min_y
                                                        && screen_v.y <= max_y
                                                    {
                                                        self.selected_elements
                                                            .push(Selection::Vertex(i));
                                                    }
                                                }
                                            }
                                            SelectMode::Edge => {
                                                for (a, b) in edges {
                                                    let va =
                                                        gt.transform_point3(custom.vertices[a]);
                                                    let vb =
                                                        gt.transform_point3(custom.vertices[b]);
                                                    let mid = (va + vb) * 0.5;
                                                    let screen_mid = world_to_screen(
                                                        &self.state.as_ref().unwrap().camera,
                                                        self.viewport_size,
                                                        mid,
                                                    );
                                                    if screen_mid.x >= min_x
                                                        && screen_mid.x <= max_x
                                                        && screen_mid.y >= min_y
                                                        && screen_mid.y <= max_y
                                                    {
                                                        self.selected_elements
                                                            .push(Selection::Edge(a, b));
                                                    }
                                                }
                                            }
                                            SelectMode::Face => {
                                                for face in faces {
                                                    let mut center = Vec3::ZERO;
                                                    for &i in &face {
                                                        center += custom.vertices[i];
                                                    }
                                                    center /= face.len() as f32;
                                                    let world_center = gt.transform_point3(center);
                                                    let screen_center = world_to_screen(
                                                        &self.state.as_ref().unwrap().camera,
                                                        self.viewport_size,
                                                        world_center,
                                                    );
                                                    if screen_center.x >= min_x
                                                        && screen_center.x <= max_x
                                                        && screen_center.y >= min_y
                                                        && screen_center.y <= max_y
                                                    {
                                                        self.selected_elements
                                                            .push(Selection::Face(face));
                                                    }
                                                }
                                            }
                                        }
                                        update_object_mesh(
                                            self.state.as_mut().unwrap(),
                                            &self.scene,
                                            &self.render_map,
                                            sel,
                                            &self.selected_elements,
                                        );
                                    }
                                }
                            }
                        }
                    } else {
                        if self.drag_moved && self.drag.is_some() {
                            let snap = capture_snapshot(&mut self.scene);
                            self.history.record(snap);
                        }
                        self.drag = None;
                        self.drag_moved = false;
                    }
                }
            }
            MouseButton::Right => {
                self.cancel_transform();
                self.input.rmb_down = btn_state == ElementState::Pressed;
            }

            MouseButton::Middle => {
                self.input.mmb_down = btn_state == ElementState::Pressed;
            }
            _ => {}
        }
    }
}
