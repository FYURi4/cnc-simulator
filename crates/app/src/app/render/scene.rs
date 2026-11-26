use glam::{Mat4, Vec3};
use crate::app::App;
use crate::editor::*;
use crate::types::*;

impl App {
    pub(super) fn update_scene_render_state(&mut self) {
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

                    
    }

}
