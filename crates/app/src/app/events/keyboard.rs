use crate::app::{App, ViewDirection};
use crate::editor::tools::numeric_input;
use crate::editor::*;
use crate::geometry::*;
use crate::io::*;
use crate::types::*;
use bevy_ecs::prelude::*;
use engine_core::Transform;
use engine_render::mesh::MeshKind;
use glam::{Vec2, Vec3};
use winit::{
    event::{ElementState, KeyEvent},
    keyboard::{KeyCode, PhysicalKey},
};

impl App {
    pub(super) fn handle_keyboard_input(&mut self, event: &KeyEvent) {
        if let PhysicalKey::Code(code) = event.physical_key {
            match event.state {
                ElementState::Pressed => {
                    let ctrl = self.input.keys.contains(&KeyCode::ControlLeft)
                        || self.input.keys.contains(&KeyCode::ControlRight);

                    let numeric_handled = if self.transform_operation.is_some() && !ctrl {
                        if let Some(op) = self.transform_operation.as_mut() {
                            if numeric_input::handle_key(&mut op.numeric_input, code) {
                                op.numeric_value = numeric_input::parse(&op.numeric_input);
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
                                        // Every axis is evaluated from the operation's original
                                        // transform. Switching axes must never commit the preview.
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

                            KeyCode::KeyG => {
                                self.begin_transform(TransformMode::Translate);
                            }
                            KeyCode::KeyR => {
                                self.begin_transform(TransformMode::Rotate);
                            }
                            KeyCode::KeyS => {
                                self.begin_transform(TransformMode::Scale);
                            }

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
                                        paste_node(
                                            &mut self.scene,
                                            &mut self.render_map,
                                            state,
                                            root,
                                            None,
                                            offset,
                                        );
                                    }
                                    self.selected = None;
                                    self.selected_elements.clear();
                                    update_edit_handles(
                                        state,
                                        &mut self.scene,
                                        self.edit_mode,
                                        self.select_mode,
                                        self.selected,
                                        &mut self.edit_handles,
                                    );
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
                                        paste_node(
                                            &mut self.scene,
                                            &mut self.render_map,
                                            state,
                                            root,
                                            None,
                                            offset,
                                        );
                                    }
                                    self.selected = None;
                                    self.selected_elements.clear();
                                    update_edit_handles(
                                        state,
                                        &mut self.scene,
                                        self.edit_mode,
                                        self.select_mode,
                                        self.selected,
                                        &mut self.edit_handles,
                                    );
                                }
                            }
                            KeyCode::KeyA if !ctrl && self.edit_mode == EditMode::Object => {
                                let mut all_entities: Vec<Entity> = Vec::new();
                                let mut q = self.scene.world.query::<Entity>();
                                for e in q.iter(&self.scene.world) {
                                    all_entities.push(e);
                                }
                                if let Some(first) = all_entities.first() {
                                    self.selected = Some(*first);
                                    set_selection(
                                        self.state.as_mut().unwrap(),
                                        &self.render_map,
                                        None,
                                        self.selected,
                                    );
                                }
                            }
                            KeyCode::KeyV
                                if ctrl
                                    && (self.input.keys.contains(&KeyCode::ShiftLeft)
                                        || self.input.keys.contains(&KeyCode::ShiftRight)) =>
                            {
                                if let Some(snap) = &self.clipboard {
                                    let state = self.state.as_mut().unwrap();
                                    for root in &snap.roots {
                                        paste_node(
                                            &mut self.scene,
                                            &mut self.render_map,
                                            state,
                                            root,
                                            None,
                                            Vec3::ZERO,
                                        );
                                    }
                                    self.selected = None;
                                    self.selected_elements.clear();
                                    update_edit_handles(
                                        state,
                                        &mut self.scene,
                                        self.edit_mode,
                                        self.select_mode,
                                        self.selected,
                                        &mut self.edit_handles,
                                    );
                                }
                            }

                            KeyCode::NumpadDecimal | KeyCode::Period | KeyCode::KeyF => {
                                self.frame_selected();
                            }
                            KeyCode::Home => {
                                let state = self.state.as_mut().unwrap();
                                state.camera.reset();
                                state.update_camera();
                            }

                            KeyCode::Numpad1 => {
                                self.current_view = ViewDirection::Front;
                                let state = self.state.as_mut().unwrap();
                                state.camera.set_view_direction(ViewDirection::Front.dir());
                                state.update_camera();
                            }
                            KeyCode::Numpad3 => {
                                self.current_view = ViewDirection::Right;
                                let state = self.state.as_mut().unwrap();
                                state.camera.set_view_direction(ViewDirection::Right.dir());
                                state.update_camera();
                            }
                            KeyCode::Numpad7 => {
                                self.current_view = ViewDirection::Top;
                                let state = self.state.as_mut().unwrap();
                                state.camera.set_view_direction(ViewDirection::Top.dir());
                                state.update_camera();
                            }
                            KeyCode::Numpad5 => match self.current_view {
                                ViewDirection::Front => {
                                    self.current_view = ViewDirection::Top;
                                    let state = self.state.as_mut().unwrap();
                                    state.camera.set_view_direction(ViewDirection::Top.dir());
                                    state.update_camera();
                                }
                                ViewDirection::Top => {
                                    self.current_view = ViewDirection::Right;
                                    let state = self.state.as_mut().unwrap();
                                    state.camera.set_view_direction(ViewDirection::Right.dir());
                                    state.update_camera();
                                }
                                _ => {
                                    self.current_view = ViewDirection::Front;
                                    let state = self.state.as_mut().unwrap();
                                    state.camera.set_view_direction(ViewDirection::Front.dir());
                                    state.update_camera();
                                }
                            },

                            KeyCode::Tab => {
                                if let Some(sel) = self.selected {
                                    if let Some(mk) = self.scene.world.get::<MeshKindComp>(sel) {
                                        if mk.0 == MeshKind::Cube
                                            || mk.0 == MeshKind::Plane
                                            || mk.0 == MeshKind::Cylinder
                                            || mk.0 == MeshKind::Sphere
                                            || mk.0 == MeshKind::Pyramid
                                        {
                                            if self.edit_mode == EditMode::Object {
                                                self.edit_mode = EditMode::Edit;
                                                self.edit_mode_history_start =
                                                    Some(self.history.pos);
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
                                            update_object_mesh(
                                                self.state.as_mut().unwrap(),
                                                &self.scene,
                                                &self.render_map,
                                                sel,
                                                &[],
                                            );
                                        }
                                    }
                                }
                            }
                            KeyCode::Digit1 => {
                                if self.edit_mode == EditMode::Edit {
                                    self.select_mode = SelectMode::Vertex;
                                    self.selected_elements.clear();
                                    update_edit_handles(
                                        self.state.as_mut().unwrap(),
                                        &mut self.scene,
                                        self.edit_mode,
                                        self.select_mode,
                                        self.selected,
                                        &mut self.edit_handles,
                                    );
                                    update_object_mesh(
                                        self.state.as_mut().unwrap(),
                                        &self.scene,
                                        &self.render_map,
                                        self.selected.unwrap(),
                                        &[],
                                    );
                                } else {
                                    self.transform_mode = TransformMode::Translate;
                                }
                            }
                            KeyCode::Digit2 => {
                                if self.edit_mode == EditMode::Edit {
                                    self.select_mode = SelectMode::Edge;
                                    self.selected_elements.clear();
                                    update_edit_handles(
                                        self.state.as_mut().unwrap(),
                                        &mut self.scene,
                                        self.edit_mode,
                                        self.select_mode,
                                        self.selected,
                                        &mut self.edit_handles,
                                    );
                                    update_object_mesh(
                                        self.state.as_mut().unwrap(),
                                        &self.scene,
                                        &self.render_map,
                                        self.selected.unwrap(),
                                        &[],
                                    );
                                } else {
                                    self.transform_mode = TransformMode::Rotate;
                                }
                            }
                            KeyCode::Digit3 => {
                                if self.edit_mode == EditMode::Edit {
                                    self.select_mode = SelectMode::Face;
                                    self.selected_elements.clear();
                                    update_edit_handles(
                                        self.state.as_mut().unwrap(),
                                        &mut self.scene,
                                        self.edit_mode,
                                        self.select_mode,
                                        self.selected,
                                        &mut self.edit_handles,
                                    );
                                    update_object_mesh(
                                        self.state.as_mut().unwrap(),
                                        &self.scene,
                                        &self.render_map,
                                        self.selected.unwrap(),
                                        &[],
                                    );
                                } else {
                                    self.transform_mode = TransformMode::Scale;
                                }
                            }
                            KeyCode::KeyX => {
                                if self.edit_mode == EditMode::Edit {
                                    if let Some(sel) = self.selected {
                                        let kind =
                                            self.scene.world.get::<MeshKindComp>(sel).unwrap().0;
                                        let faces = get_current_faces(&self.scene, sel, kind);
                                        if let Some(mut cm) =
                                            self.scene.world.get_mut::<CustomMesh>(sel)
                                        {
                                            match self.select_mode {
                                                SelectMode::Face => {
                                                    for (i, face) in faces.iter().enumerate() {
                                                        if self.selected_elements.contains(
                                                            &Selection::Face(face.clone()),
                                                        ) {
                                                            cm.deleted_faces.insert(i);
                                                        }
                                                    }
                                                }
                                                SelectMode::Vertex => {
                                                    for s in &self.selected_elements {
                                                        if let Selection::Vertex(v) = s {
                                                            for (i, face) in
                                                                faces.iter().enumerate()
                                                            {
                                                                if face.contains(v) {
                                                                    cm.deleted_faces.insert(i);
                                                                }
                                                            }
                                                        }
                                                    }
                                                }
                                                _ => {}
                                            }
                                        }
                                        self.selected_elements.clear();
                                        update_object_mesh(
                                            self.state.as_mut().unwrap(),
                                            &self.scene,
                                            &self.render_map,
                                            sel,
                                            &[],
                                        );
                                        self.history.record(capture_snapshot(&mut self.scene));
                                    }
                                }
                            }
                            KeyCode::KeyM => {
                                if self.edit_mode == EditMode::Edit
                                    && self.select_mode == SelectMode::Vertex
                                {
                                    if let Some(sel) = self.selected {
                                        let kind =
                                            self.scene.world.get::<MeshKindComp>(sel).unwrap().0;
                                        let mut faces = get_current_faces(&self.scene, sel, kind);
                                        if let Some(mut cm) =
                                            self.scene.world.get_mut::<CustomMesh>(sel)
                                        {
                                            let mut verts: Vec<usize> = self
                                                .selected_elements
                                                .iter()
                                                .filter_map(|s| {
                                                    if let Selection::Vertex(i) = s {
                                                        Some(*i)
                                                    } else {
                                                        None
                                                    }
                                                })
                                                .collect();
                                            verts.sort_unstable();
                                            verts.dedup();
                                            if verts.len() >= 2 {
                                                let target = verts[0];
                                                let mut center = Vec3::ZERO;
                                                for &v in &verts {
                                                    center += cm.vertices[v];
                                                }
                                                center /= verts.len() as f32;
                                                cm.vertices[target] = center;
                                                for face in &mut faces {
                                                    for vertex in face.iter_mut() {
                                                        if verts[1..].contains(&*vertex) {
                                                            *vertex = target;
                                                        }
                                                    }
                                                    let mut unique_face = Vec::new();
                                                    for &v in face.iter() {
                                                        if unique_face.last() != Some(&v) {
                                                            unique_face.push(v);
                                                        }
                                                    }
                                                    if unique_face.first() == unique_face.last()
                                                        && unique_face.len() > 1
                                                    {
                                                        unique_face.pop();
                                                    }
                                                    *face = unique_face;
                                                }
                                                faces.retain(|f| f.len() >= 3);
                                                cm.custom_faces = Some(faces);
                                                cm.deleted_faces.clear();
                                                self.selected_elements =
                                                    vec![Selection::Vertex(target)];
                                            }
                                        }
                                        update_object_mesh(
                                            self.state.as_mut().unwrap(),
                                            &self.scene,
                                            &self.render_map,
                                            sel,
                                            &self.selected_elements,
                                        );
                                        self.history.record(capture_snapshot(&mut self.scene));
                                    }
                                }
                            }
                            KeyCode::KeyZ if ctrl => {
                                self.do_undo();
                                self.transform_operation = None;
                            }
                            KeyCode::KeyY if ctrl => {
                                self.do_redo();
                                self.transform_operation = None;
                            }
                            _ => {}
                        }
                    }
                    self.input.keys.insert(code);
                }
                ElementState::Released => {
                    self.input.keys.remove(&code);
                }
            }
        }
    }
}
