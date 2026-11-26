use std::sync::Arc;
use anyhow::Result;
use bevy_ecs::prelude::*;
use glam::{Mat4, Quat, Vec2, Vec3};
use winit::{
    application::ApplicationHandler,
    event::{ElementState, MouseButton, MouseScrollDelta, WindowEvent},
    event_loop::ActiveEventLoop,
    keyboard::{KeyCode, PhysicalKey},
    window::{Window, WindowId},
};
use engine_core::{Parent, Scene, Transform};
use engine_render::mesh::{MeshKind, Vertex};
use engine_render::state::State;
use crate::types::*;
use crate::geometry::*;
use crate::editor::*;
use crate::io::*;
use crate::ui::*;
use super::App;
mod keyboard;
mod pointer;

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_none() {
            let window = Arc::new(
                event_loop.create_window(Window::default_attributes().with_title("CNC Simulator").with_maximized(true))
                    .expect("create window"),
            );
            self.window = Some(window.clone());
            let size = window.inner_size();
            self.viewport_size = Vec2::new(size.width as f32, size.height as f32);

            let mut state = pollster::block_on(State::new(window)).expect("init wgpu state");
            let (grid, cursor) = build_default_scene(&mut state, &mut self.scene, &mut self.render_map);
            self.grid_entity = Some(grid);
            self.cursor_entity = Some(cursor);
            self.gizmo = Some(create_gizmo_objects(&mut state));
            self.state = Some(state);

            let snap = capture_snapshot(&mut self.scene);
            self.history.record(snap);
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        if self.window.is_none() || self.state.is_none() { return; }

        let is_tab = matches!(&event, WindowEvent::KeyboardInput { event, .. } if event.physical_key == PhysicalKey::Code(KeyCode::Tab));
        let resp = self.state.as_mut().unwrap().handle_egui_event(&event);
        if resp.consumed && !matches!(event, WindowEvent::RedrawRequested) && !is_tab { return; }

        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => {
                self.viewport_size = Vec2::new(size.width as f32, size.height as f32);
                let state = self.state.as_mut().unwrap();
                state.resize(size);
                state.update_camera();
            }
            WindowEvent::CursorMoved { position, .. } => self.handle_cursor_moved(Vec2::new(position.x as f32, position.y as f32)), => {
                let new_pos = Vec2::new(position.x as f32, position.y as f32);
                let delta = new_pos - self.input.cursor_pos;
                self.input.cursor_pos = new_pos;

                if self.transform_operation.is_some()
                    && !self.input.mmb_down
                    && !self.input.rmb_down
                {
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
                        state.camera.orbit(delta.x * sensitivity, -delta.y * sensitivity);
                    }
                    state.update_camera();
                }

                if self.input.rmb_down {
                    let sensitivity = 0.005;
                    let state = self.state.as_mut().unwrap();
                    state.camera.rotate(delta.x * sensitivity, -delta.y * sensitivity);
                    state.update_camera();
                }

                if self.input.lmb_down {
                    if self.drag_start.is_some() {
                        self.drag_end = Some(new_pos);
                    } else if let Some(drag) = self.drag.clone() {
                        let d = new_pos - drag.start_mouse_screen;
                        if d.length_squared() > 4.0 { self.drag_moved = true; }

                        if self.edit_mode == EditMode::Edit {
                            if let Some(start_verts) = &drag.start_custom_verts {
                                let len_sq = drag.screen_axis_dir.length_squared();
                                if len_sq > 1.0 {
                                    let proj = d.dot(drag.screen_axis_dir) / len_sq * drag.axis_world_length;
                                    let mut new_world = drag.start_world_pos + drag.axis * proj;
                                    if self.snap.enabled {
                                        let state = self.state.as_ref().unwrap();
                                        new_world = apply_snap(new_world, drag.start_world_pos, drag.axis, &self.snap, &self.scene, &self.render_map, &state.objects, drag.entity);
                                    }
                                    let obj_gt = self.scene.global_matrix(drag.entity).unwrap_or(Mat4::IDENTITY);
                                    let local_new = obj_gt.inverse().transform_point3(new_world);
                                    let delta_local = local_new - obj_gt.inverse().transform_point3(drag.start_world_pos);
                                    let mut custom = start_verts.clone();
                                    for &idx in &drag.selected_indices { custom[idx] += delta_local; }
                                    if let Some(mut cm) = self.scene.world.get_mut::<CustomMesh>(drag.entity) { cm.vertices = custom.clone(); }
                                    if let Some(kind) = self.scene.world.get::<MeshKindComp>(drag.entity).map(|m| m.0) {
                                        let new_vertices = build_mesh_vertices(&self.scene, drag.entity, kind, &self.selected_elements);
                                        if let Some((_, r_idx)) = self.render_map.iter().find(|(e, _)| *e == drag.entity) {
                                            self.state.as_mut().unwrap().update_object_vertices(*r_idx, &new_vertices);
                                        }
                                    }
                                }
                            }
                        } else {
                            match drag.mode {
                                TransformMode::Translate => {
                                    let len_sq = drag.screen_axis_dir.length_squared();
                                    if len_sq > 1.0 {
                                        let proj = d.dot(drag.screen_axis_dir) / len_sq * drag.axis_world_length;
                                        let mut new_world = drag.start_world_pos + drag.axis * proj;
                                        if self.snap.enabled {
                                            let state = self.state.as_ref().unwrap();
                                            new_world = apply_snap(new_world, drag.start_world_pos, drag.axis, &self.snap, &self.scene, &self.render_map, &state.objects, drag.entity);
                                        }
                                        let local = if let Some(p) = self.scene.world.get::<Parent>(drag.entity) {
                                            if let Some(pg) = self.scene.global_matrix(p.0) { pg.inverse().transform_point3(new_world) } else { new_world }
                                        } else { new_world };
                                        if let Some(mut t) = self.scene.world.get_mut::<Transform>(drag.entity) { t.translation = local; }
                                    }
                                }
                                TransformMode::Rotate => {
                                    let start_vec = drag.start_mouse_screen - drag.pivot_screen;
                                    let cur_vec = new_pos - drag.pivot_screen;
                                    let mut delta_a = cur_vec.y.atan2(cur_vec.x) - start_vec.y.atan2(start_vec.x);
                                    if delta_a > PI { delta_a -= 2.0 * PI; }
                                    if delta_a < -PI { delta_a += 2.0 * PI; }
                                    delta_a = -delta_a;
                                    if self.snap.enabled {
                                        let step = 15.0_f32.to_radians();
                                        delta_a = (delta_a / step).round() * step;
                                    }
                                    let new_rot = Quat::from_axis_angle(drag.axis, delta_a) * drag.start_rotation;
                                    if let Some(mut t) = self.scene.world.get_mut::<Transform>(drag.entity) { t.rotation = new_rot; }
                                }
                                TransformMode::Scale => {
                                    let len_sq = drag.screen_axis_dir.length_squared();
                                    if len_sq > 1.0 {
                                        let proj = d.dot(drag.screen_axis_dir) / len_sq * drag.axis_world_length;
                                        let axis_idx = axis_index(drag.axis);
                                        let start_v = axis_comp(drag.start_scale, axis_idx);
                                        let new_v = (start_v + proj).max(0.01);
                                        let mut new_scale = drag.start_scale;
                                        set_axis_comp(&mut new_scale, axis_idx, new_v);
                                        if let Some(mut t) = self.scene.world.get_mut::<Transform>(drag.entity) { t.scale = new_scale; }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            WindowEvent::KeyboardInput { ref event, .. } => self.handle_keyboard_input(event), => {
                if let PhysicalKey::Code(code) = event.physical_key {
                    match event.state {
                        ElementState::Pressed => {
                            let ctrl = self.input.keys.contains(&KeyCode::ControlLeft)
                                || self.input.keys.contains(&KeyCode::ControlRight);

                            let numeric_handled =
                                if self.transform_operation.is_some() && !ctrl {
                                    if let Some(op) = self.transform_operation.as_mut() {
                                        if numeric_input::handle_key(
                                            &mut op.numeric_input,
                                            code,
                                        ) {
                                            op.numeric_value =
                                                numeric_input::parse(&op.numeric_input);
                                            true
                                        } else {
                                            false
                                        }
                                    } else {
                                        false
                                    }
                                } else {
                                    false
                                };

                            if numeric_handled {
                                self.apply_numeric_transform();
                            } else {
                                match code {
                                KeyCode::KeyX | KeyCode::KeyY | KeyCode::KeyZ
                                    if self.transform_operation.is_some() && !ctrl =>
                                {
                                    let axis = match code {
                                        KeyCode::KeyX => Vec3::X,
                                        KeyCode::KeyY => Vec3::Y,
                                        KeyCode::KeyZ => Vec3::Z,
                                        _ => unreachable!(),
                                    };

                                    if let Some(op) = self.transform_operation.as_mut() {
                                        if op.axis != Some(axis) {
                                            // Preserve the previous axis result and start a clean entry.
                                            if let Some(current) = self.scene.world.get::<Transform>(op.entity) {
                                                op.start_translation = current.translation;
                                                op.start_rotation = current.rotation;
                                                op.start_scale = current.scale;
                                            }
                                            op.numeric_input.clear();
                                            op.numeric_value = None;
                                        }
                                        op.axis = Some(axis);
                                    }

                                    self.apply_numeric_transform();
                                }
                                KeyCode::Enter | KeyCode::NumpadEnter => {
                                    // FIX: записываем историю при подтверждении G/R/S
                                    self.confirm_transform_if_changed("Enter");
                                }

                                KeyCode::Escape => {
                                    self.cancel_transform();
                                }

                                KeyCode::KeyG => { self.begin_transform(TransformMode::Translate); }
                                KeyCode::KeyR => { self.begin_transform(TransformMode::Rotate); }
                                KeyCode::KeyS => { self.begin_transform(TransformMode::Scale); }

                                KeyCode::KeyX if self.transform_operation.is_none() && !ctrl => {
                                    if self.selected.is_some() {
                                        self.delete_confirm = !self.delete_confirm;
                                        self.delete_confirm_pos = self.input.cursor_pos;
                                    }
                                }

                                KeyCode::KeyC if ctrl => {
                                    if let Some(sel) = self.selected {
                                        let snap = capture_selected(&mut self.scene, sel);
                                        self.clipboard = Some(snap);
                                    }
                                }
                                KeyCode::KeyV if ctrl => {
                                    if let Some(snap) = &self.clipboard {
                                        let state = self.state.as_mut().unwrap();
                                        let offset = Vec3::new(2.0, 2.0, 0.0);
                                        for root in &snap.roots {
                                            paste_node(&mut self.scene, &mut self.render_map, state, root, None, offset);
                                        }
                                        self.selected = None;
                                        self.selected_elements.clear();
                                        update_edit_handles(state, &mut self.scene, self.edit_mode, self.select_mode, self.selected, &mut self.edit_handles);
                                    }
                                }
                                KeyCode::KeyZ if ctrl => {
                                    self.pending_undo = true;
                                }
                                KeyCode::KeyY if ctrl => {
                                    self.pending_redo = true;
                                }
                                KeyCode::Delete => {
                                    if self.selected.is_some() {
                                        self.delete_confirm = true;
                                        self.delete_confirm_pos = Vec2::new(640.0, 360.0);
                                    }
                                }
                                KeyCode::KeyD if ctrl => {
                                    if let Some(sel) = self.selected {
                                        let snap = capture_selected(&mut self.scene, sel);
                                        let offset = Vec3::new(2.0, 2.0, 0.0);
                                        let state = self.state.as_mut().unwrap();
                                        for root in &snap.roots {
                                            paste_node(&mut self.scene, &mut self.render_map, state, root, None, offset);
                                        }
                                        self.selected = None;
                                        self.selected_elements.clear();
                                        update_edit_handles(state, &mut self.scene, self.edit_mode, self.select_mode, self.selected, &mut self.edit_handles);
                                    }
                                }
                                KeyCode::KeyA if !ctrl && self.edit_mode == EditMode::Object => {
                                    let mut all_entities: Vec<Entity> = Vec::new();
                                    let mut q = self.scene.world.query::<Entity>();
                                    for e in q.iter(&self.scene.world) { all_entities.push(e); }
                                    if let Some(first) = all_entities.first() {
                                        self.selected = Some(*first);
                                        set_selection(self.state.as_mut().unwrap(), &self.render_map, None, self.selected);
                                    }
                                }
                                KeyCode::KeyV if ctrl && (self.input.keys.contains(&KeyCode::ShiftLeft) || self.input.keys.contains(&KeyCode::ShiftRight)) => {
                                    if let Some(snap) = &self.clipboard {
                                        let state = self.state.as_mut().unwrap();
                                        for root in &snap.roots {
                                            paste_node(&mut self.scene, &mut self.render_map, state, root, None, Vec3::ZERO);
                                        }
                                        self.selected = None;
                                        self.selected_elements.clear();
                                        update_edit_handles(state, &mut self.scene, self.edit_mode, self.select_mode, self.selected, &mut self.edit_handles);
                                    }
                                }

                                KeyCode::NumpadDecimal | KeyCode::Period | KeyCode::KeyF => { self.frame_selected(); }
                                KeyCode::Home => {
                                    let state = self.state.as_mut().unwrap();
                                    state.camera.reset();
                                    state.update_camera();
                                }

                                KeyCode::Numpad1 => { self.current_view = ViewDirection::Front; let state = self.state.as_mut().unwrap(); state.camera.set_view_direction(ViewDirection::Front.dir()); state.update_camera(); }
                                KeyCode::Numpad3 => { self.current_view = ViewDirection::Right; let state = self.state.as_mut().unwrap(); state.camera.set_view_direction(ViewDirection::Right.dir()); state.update_camera(); }
                                KeyCode::Numpad7 => { self.current_view = ViewDirection::Top; let state = self.state.as_mut().unwrap(); state.camera.set_view_direction(ViewDirection::Top.dir()); state.update_camera(); }
                                KeyCode::Numpad5 => {
                                    match self.current_view {
                                        ViewDirection::Front => { self.current_view = ViewDirection::Top; let state = self.state.as_mut().unwrap(); state.camera.set_view_direction(ViewDirection::Top.dir()); state.update_camera(); }
                                        ViewDirection::Top => { self.current_view = ViewDirection::Right; let state = self.state.as_mut().unwrap(); state.camera.set_view_direction(ViewDirection::Right.dir()); state.update_camera(); }
                                        _ => { self.current_view = ViewDirection::Front; let state = self.state.as_mut().unwrap(); state.camera.set_view_direction(ViewDirection::Front.dir()); state.update_camera(); }
                                    }
                                }

                                KeyCode::Tab => {
                                    if let Some(sel) = self.selected {
                                        if let Some(mk) = self.scene.world.get::<MeshKindComp>(sel) {
                                            if mk.0 == MeshKind::Cube || mk.0 == MeshKind::Plane || mk.0 == MeshKind::Cylinder || mk.0 == MeshKind::Sphere || mk.0 == MeshKind::Pyramid {
                                                if self.edit_mode == EditMode::Object {
                                                    self.edit_mode = EditMode::Edit;
                                                    self.edit_mode_history_start = Some(self.history.pos);
                                                } else {
                                                    self.edit_mode = EditMode::Object;
                                                    self.edit_mode_history_start = None;
                                                }
                                                self.selected_elements.clear();
                                                update_edit_handles(
                                                    self.state.as_mut().unwrap(),
                                                    &mut self.scene,
                                                    self.edit_mode,
                                                    self.select_mode,
                                                    self.selected,
                                                    &mut self.edit_handles,
                                                );
                                                update_object_mesh(self.state.as_mut().unwrap(), &self.scene, &self.render_map, sel, &[]);
                                            }
                                        }
                                    }
                                }
                                KeyCode::Digit1 => {
                                    if self.edit_mode == EditMode::Edit {
                                        self.select_mode = SelectMode::Vertex;
                                        self.selected_elements.clear();
                                        update_edit_handles(self.state.as_mut().unwrap(), &mut self.scene, self.edit_mode, self.select_mode, self.selected, &mut self.edit_handles);
                                        update_object_mesh(self.state.as_mut().unwrap(), &self.scene, &self.render_map, self.selected.unwrap(), &[]);
                                    } else { self.transform_mode = TransformMode::Translate; }
                                }
                                KeyCode::Digit2 => {
                                    if self.edit_mode == EditMode::Edit {
                                        self.select_mode = SelectMode::Edge;
                                        self.selected_elements.clear();
                                        update_edit_handles(self.state.as_mut().unwrap(), &mut self.scene, self.edit_mode, self.select_mode, self.selected, &mut self.edit_handles);
                                        update_object_mesh(self.state.as_mut().unwrap(), &self.scene, &self.render_map, self.selected.unwrap(), &[]);
                                    } else { self.transform_mode = TransformMode::Rotate; }
                                }
                                KeyCode::Digit3 => {
                                    if self.edit_mode == EditMode::Edit {
                                        self.select_mode = SelectMode::Face;
                                        self.selected_elements.clear();
                                        update_edit_handles(self.state.as_mut().unwrap(), &mut self.scene, self.edit_mode, self.select_mode, self.selected, &mut self.edit_handles);
                                        update_object_mesh(self.state.as_mut().unwrap(), &self.scene, &self.render_map, self.selected.unwrap(), &[]);
                                    } else { self.transform_mode = TransformMode::Scale; }
                                }
                                KeyCode::KeyX => {
                                    if self.edit_mode == EditMode::Edit {
                                        if let Some(sel) = self.selected {
                                            let kind = self.scene.world.get::<MeshKindComp>(sel).unwrap().0;
                                            let faces = get_current_faces(&self.scene, sel, kind);
                                            if let Some(mut cm) = self.scene.world.get_mut::<CustomMesh>(sel) {
                                                match self.select_mode {
                                                    SelectMode::Face => {
                                                        for (i, face) in faces.iter().enumerate() {
                                                            if self.selected_elements.contains(&Selection::Face(face.clone())) {
                                                                cm.deleted_faces.insert(i);
                                                            }
                                                        }
                                                    }
                                                    SelectMode::Vertex => {
                                                        for s in &self.selected_elements {
                                                            if let Selection::Vertex(v) = s {
                                                                for (i, face) in faces.iter().enumerate() {
                                                                    if face.contains(v) { cm.deleted_faces.insert(i); }
                                                                }
                                                            }
                                                        }
                                                    }
                                                    _ => {}
                                                }
                                            }
                                            self.selected_elements.clear();
                                            update_object_mesh(self.state.as_mut().unwrap(), &self.scene, &self.render_map, sel, &[]);
                                            self.history.record(capture_snapshot(&mut self.scene));
                                        }
                                    }
                                }
                                KeyCode::KeyM => {
                                    if self.edit_mode == EditMode::Edit && self.select_mode == SelectMode::Vertex {
                                        if let Some(sel) = self.selected {
                                            let kind = self.scene.world.get::<MeshKindComp>(sel).unwrap().0;
                                            let mut faces = get_current_faces(&self.scene, sel, kind);
                                            if let Some(mut cm) = self.scene.world.get_mut::<CustomMesh>(sel) {
                                                let mut verts: Vec<usize> = self.selected_elements.iter().filter_map(|s| {
                                                    if let Selection::Vertex(i) = s { Some(*i) } else { None }
                                                }).collect();
                                                verts.sort_unstable();
                                                verts.dedup();
                                                if verts.len() >= 2 {
                                                    let target = verts[0];
                                                    let mut center = Vec3::ZERO;
                                                    for &v in &verts { center += cm.vertices[v]; }
                                                    center /= verts.len() as f32;
                                                    cm.vertices[target] = center;
                                                    for face in &mut faces {
                                                        for i in 0..face.len() {
                                                            if verts[1..].contains(&face[i]) { face[i] = target; }
                                                        }
                                                        let mut unique_face = Vec::new();
                                                        for &v in face.iter() {
                                                            if unique_face.last() != Some(&v) { unique_face.push(v); }
                                                        }
                                                        if unique_face.first() == unique_face.last() && unique_face.len() > 1 { unique_face.pop(); }
                                                        *face = unique_face;
                                                    }
                                                    faces.retain(|f| f.len() >= 3);
                                                    cm.custom_faces = Some(faces);
                                                    cm.deleted_faces.clear();
                                                    self.selected_elements = vec![Selection::Vertex(target)];
                                                }
                                            }
                                            update_object_mesh(self.state.as_mut().unwrap(), &self.scene, &self.render_map, sel, &self.selected_elements);
                                            self.history.record(capture_snapshot(&mut self.scene));
                                        }
                                    }
                                }
                                KeyCode::KeyZ if ctrl => {
                                    self.do_undo();
                                    self.transform_operation = None;
                                },
                                KeyCode::KeyY if ctrl => {
                                    self.do_redo();
                                    self.transform_operation = None;
                                },
                                _ => {}
                            }}
                            self.input.keys.insert(code);
                        }
                        ElementState::Released => { self.input.keys.remove(&code); }
                    }
                }
                }
                WindowEvent::MouseInput { state: btn_state, button, .. } => self.handle_mouse_input(btn_state, button), => {
                    match button {
                        MouseButton::Left => {
                            if btn_state == ElementState::Pressed {
                                self.input.lmb_down = true;
                                self.drag_moved = false;

                                if self.edit_mode == EditMode::Edit && self.select_tool == SelectTool::Box {
                                    self.drag_start = Some(self.input.cursor_pos);
                                    self.drag_end = Some(self.input.cursor_pos);
                                } else {
                                    // FIX: LMB подтверждает G/R/S с записью истории
                                    if self.confirm_transform_if_changed("LMB") {
                                        return;
                                    }
                                    handle_lmb_press(
                                        self.state.as_mut().unwrap(), &self.input, self.viewport_size,
                                        &mut self.selected, &mut self.selected_elements, &mut self.drag,
                                        self.transform_mode, self.edit_mode, self.select_mode, &mut self.scene, &self.render_map,
                                        self.cursor_entity,
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
                                        let ctrl = self.input.keys.contains(&KeyCode::ControlLeft) || self.input.keys.contains(&KeyCode::ShiftLeft);
                                        if !ctrl { self.selected_elements.clear(); }
                                        if let Some(sel) = self.selected {
                                            if let Some(custom) = self.scene.world.get::<CustomMesh>(sel) {
                                                if let Some(gt) = self.scene.global_matrix(sel) {
                                                    let kind = self.scene.world.get::<MeshKindComp>(sel).unwrap().0;
                                                    let faces = get_current_faces(&self.scene, sel, kind);
                                                    let edges = get_current_edges(&faces);
                                                    match self.select_mode {
                                                        SelectMode::Vertex => {
                                                            for (i, &v) in custom.vertices.iter().enumerate() {
                                                                let world_v = gt.transform_point3(v);
                                                                let screen_v = world_to_screen(&self.state.as_ref().unwrap().camera, self.viewport_size, world_v);
                                                                if screen_v.x >= min_x && screen_v.x <= max_x && screen_v.y >= min_y && screen_v.y <= max_y {
                                                                    self.selected_elements.push(Selection::Vertex(i));
                                                                }
                                                            }
                                                        }
                                                        SelectMode::Edge => {
                                                            for (a, b) in edges {
                                                                let va = gt.transform_point3(custom.vertices[a]);
                                                                let vb = gt.transform_point3(custom.vertices[b]);
                                                                let mid = (va + vb) * 0.5;
                                                                let screen_mid = world_to_screen(&self.state.as_ref().unwrap().camera, self.viewport_size, mid);
                                                                if screen_mid.x >= min_x && screen_mid.x <= max_x && screen_mid.y >= min_y && screen_mid.y <= max_y {
                                                                    self.selected_elements.push(Selection::Edge(a, b));
                                                                }
                                                            }
                                                        }
                                                        SelectMode::Face => {
                                                            for face in faces {
                                                                let mut center = Vec3::ZERO;
                                                                for &i in &face { center += custom.vertices[i]; }
                                                                center /= face.len() as f32;
                                                                let world_center = gt.transform_point3(center);
                                                                let screen_center = world_to_screen(&self.state.as_ref().unwrap().camera, self.viewport_size, world_center);
                                                                if screen_center.x >= min_x && screen_center.x <= max_x && screen_center.y >= min_y && screen_center.y <= max_y {
                                                                    self.selected_elements.push(Selection::Face(face));
                                                                }
                                                            }
                                                        }
                                                    }
                                                    update_object_mesh(self.state.as_mut().unwrap(), &self.scene, &self.render_map, sel, &self.selected_elements);
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
                WindowEvent::MouseWheel { delta, .. } => {
                    let scroll = match delta {
                        MouseScrollDelta::LineDelta(_, y) => y,
                        MouseScrollDelta::PixelDelta(position) => position.y as f32 / 100.0,
                    };
                    let state = self.state.as_mut().unwrap();
                    state.camera.zoom(scroll);
                    state.update_camera();
                }

                WindowEvent::RedrawRequested => { if !self.render_frame() { event_loop.exit(); } }
             _ => {}
            }
        }

        fn about_to_wait(&mut self, _el: &ActiveEventLoop) {
            if let Some(window) = &self.window { window.request_redraw(); }
        }
    }
