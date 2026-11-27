use crate::app::App;
use crate::editor::*;
use crate::io::*;
use crate::types::*;
use engine_core::{Parent, Transform};
use engine_render::mesh::MeshKind;
use glam::{Vec2, Vec3};

impl App {
    pub(super) fn apply_ui_action(&mut self, action: UiAction) {
        let Some(state) = self.state.as_mut() else {
            return;
        };
        match action {
            UiAction::None => {}
            UiAction::Save => {
                let _ = save_scene(&mut self.scene, SAVE_PATH);
            }
            UiAction::SaveAs => {
                if !self.save_as_path.is_empty() {
                    let _ = save_scene(&mut self.scene, &self.save_as_path);
                }
            }
            UiAction::Undo => self.do_undo(),
            UiAction::Redo => self.do_redo(),
            UiAction::SetMode(m) => self.transform_mode = m,
            UiAction::ConfirmDelete => {
                if let Some(e) = self.selected {
                    if self.scene.world.get::<Locked>(e).is_none() {
                        self.selected = None;
                        self.selected_elements.clear();
                        if self.edit_mode == EditMode::Edit {
                            self.edit_mode = EditMode::Object;
                            self.edit_mode_history_start = None;
                            update_edit_handles(
                                state,
                                &mut self.scene,
                                self.edit_mode,
                                self.select_mode,
                                self.selected,
                                &mut self.edit_handles,
                            );
                        } else {
                            self.edit_handles.clear();
                        }
                        delete_entity(&mut self.scene, &mut self.render_map, state, e);
                        self.history.record(capture_snapshot(&mut self.scene));
                    }
                }
            }
            UiAction::CancelDelete => {
                self.delete_confirm = false;
            }
            UiAction::CopySelected => {
                if let Some(sel) = self.selected {
                    let snap = capture_selected(&mut self.scene, sel);
                    self.clipboard = Some(snap);
                }
            }
            UiAction::PasteSelected => {
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
            UiAction::SetOriginToCursorConfirmed => {
                if let (Some(sel), Some(cursor_e)) = (self.selected, self.cursor_entity) {
                    let cursor_pos = self
                        .scene
                        .global_matrix(cursor_e)
                        .map(|m| m.w_axis.truncate())
                        .unwrap_or(Vec3::ZERO);
                    let obj_pos = self
                        .scene
                        .global_matrix(sel)
                        .map(|m| m.w_axis.truncate())
                        .unwrap_or(Vec3::ZERO);
                    let delta = cursor_pos - obj_pos;
                    if let Some(p) = self.scene.world.get::<Parent>(sel) {
                        if let Some(pg) = self.scene.global_matrix(p.0) {
                            let local_delta = pg.inverse().transform_vector3(delta);
                            if let Some(mut t) = self.scene.world.get_mut::<Transform>(sel) {
                                t.pivot_offset += local_delta;
                                t.translation += local_delta;
                            }
                        }
                    } else {
                        if let Some(mut t) = self.scene.world.get_mut::<Transform>(sel) {
                            t.pivot_offset += delta;
                            t.translation += delta;
                        }
                    }
                    self.history.record(capture_snapshot(&mut self.scene));
                }
            }
            UiAction::LoadGcode => {
                const GCODE_PATH: &str = "C:/project/cnc-simulator/test.nc";
                match std::fs::read_to_string(GCODE_PATH) {
                    Ok(source) => {
                        let prog = engine_core::gcode::GCodeProgram::parse(&source);
                        println!(
                            "G-code loaded: {} segments, {} errors",
                            prog.segments.len(),
                            prog.errors.len()
                        );
                        for e in prog.errors.iter().take(5) {
                            println!("  {}", e);
                        }
                        if let (Some(first), Some(last)) =
                            (prog.segments.first(), prog.segments.last())
                        {
                            println!("  from {:?} to {:?}", first.from, last.to);
                        }
                        self.gcode = prog;
                    }
                    Err(e) => eprintln!("Failed to read {}: {}", GCODE_PATH, e),
                }
            }
            UiAction::Load => {
                if std::path::PathBuf::from(SAVE_PATH).exists() {
                    if let Ok((grid, cursor)) =
                        load_scene(&mut self.scene, &mut self.render_map, state, SAVE_PATH)
                    {
                        self.grid_entity = Some(grid);
                        self.cursor_entity = Some(cursor);
                        self.selected = None;
                        self.drag = None;
                        self.editing_entity = None;
                        self.edit_mode = EditMode::Object;
                        self.selected_elements.clear();
                        update_edit_handles(
                            state,
                            &mut self.scene,
                            self.edit_mode,
                            self.select_mode,
                            self.selected,
                            &mut self.edit_handles,
                        );
                        let snap = capture_snapshot(&mut self.scene);
                        self.history.record(snap);
                    }
                }
            }
            UiAction::AddCube => {
                let parent = self.selected.or(self.grid_entity);
                let e = add_mesh_node(
                    &mut self.scene,
                    &mut self.render_map,
                    state,
                    MeshKind::Cube,
                    parent,
                );
                self.selected = Some(e);
                self.history.record(capture_snapshot(&mut self.scene));
            }
            UiAction::AddPlane => {
                let parent = self.selected.or(self.grid_entity);
                let e = add_mesh_node(
                    &mut self.scene,
                    &mut self.render_map,
                    state,
                    MeshKind::Plane,
                    parent,
                );
                self.selected = Some(e);
                self.history.record(capture_snapshot(&mut self.scene));
            }
            UiAction::AddCylinder => {
                let parent = self.selected.or(self.grid_entity);
                let e = add_mesh_node(
                    &mut self.scene,
                    &mut self.render_map,
                    state,
                    MeshKind::Cylinder,
                    parent,
                );
                self.selected = Some(e);
                self.history.record(capture_snapshot(&mut self.scene));
            }
            UiAction::AddSphere => {
                let parent = self.selected.or(self.grid_entity);
                let e = add_mesh_node(
                    &mut self.scene,
                    &mut self.render_map,
                    state,
                    MeshKind::Sphere,
                    parent,
                );
                self.selected = Some(e);
                self.history.record(capture_snapshot(&mut self.scene));
            }
            UiAction::AddPyramid => {
                let parent = self.selected.or(self.grid_entity);
                let e = add_mesh_node(
                    &mut self.scene,
                    &mut self.render_map,
                    state,
                    MeshKind::Pyramid,
                    parent,
                );
                self.selected = Some(e);
                self.history.record(capture_snapshot(&mut self.scene));
            }
            UiAction::DeleteSelected => {
                if self.selected.is_some() {
                    self.delete_confirm = true;
                    self.delete_confirm_pos = Vec2::new(640.0, 360.0);
                }
            }
            UiAction::SetOriginToCursor => {
                if let Some(sel) = self.selected {
                    if let Some(cursor_e) = self.cursor_entity {
                        let cursor_pos = self
                            .scene
                            .global_matrix(cursor_e)
                            .map(|m| m.w_axis.truncate())
                            .unwrap_or(Vec3::ZERO);
                        let obj_pos = self
                            .scene
                            .global_matrix(sel)
                            .map(|m| m.w_axis.truncate())
                            .unwrap_or(Vec3::ZERO);
                        let delta = cursor_pos - obj_pos;
                        if let Some(p) = self.scene.world.get::<Parent>(sel) {
                            if let Some(pg) = self.scene.global_matrix(p.0) {
                                let local_delta = pg.inverse().transform_vector3(delta);
                                if let Some(mut t) = self.scene.world.get_mut::<Transform>(sel) {
                                    t.pivot_offset += local_delta;
                                    t.translation += local_delta;
                                }
                            }
                        } else {
                            if let Some(mut t) = self.scene.world.get_mut::<Transform>(sel) {
                                t.pivot_offset += delta;
                                t.translation += delta;
                            }
                        }
                        self.history.record(capture_snapshot(&mut self.scene));
                    }
                }
            }
            UiAction::SetOriginToGeometry => {
                if let Some(sel) = self.selected {
                    if let Some(mut t) = self.scene.world.get_mut::<Transform>(sel) {
                        let offset = t.pivot_offset;
                        t.translation += offset;
                        t.pivot_offset = Vec3::ZERO;
                    }
                    self.history.record(capture_snapshot(&mut self.scene));
                }
            }
        }
    }
}
