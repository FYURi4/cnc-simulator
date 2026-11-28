use crate::app::App;
use crate::editor::*;
use crate::io::*;
use crate::types::*;
use engine_core::{Parent, Transform};
use engine_render::mesh::MeshKind;
use glam::Vec3;

fn set_object_pivot(
    scene: &mut engine_core::Scene,
    entity: bevy_ecs::prelude::Entity,
    pivot: Vec3,
) {
    if let Some(mut transform) = scene.world.get_mut::<Transform>(entity) {
        let delta = pivot - transform.pivot_offset;
        let rotation = transform.rotation;
        let scale = transform.scale;
        transform.translation += rotation * (scale * delta);
        transform.pivot_offset = pivot;
    }
}

fn selected_container(
    scene: &engine_core::Scene,
    selected: Option<bevy_ecs::prelude::Entity>,
    grid: Option<bevy_ecs::prelude::Entity>,
) -> Option<bevy_ecs::prelude::Entity> {
    if let Some(selected) = selected {
        if scene.world.get::<Folder>(selected).is_some() {
            return Some(selected);
        }
        if let Some(parent) = scene.world.get::<Parent>(selected).map(|p| p.0) {
            if scene.world.get::<Folder>(parent).is_some() {
                return Some(parent);
            }
        }
    }
    grid
}

impl App {
    pub(super) fn apply_ui_action(&mut self, action: UiAction) {
        let Some(state) = self.state.as_mut() else {
            return;
        };
        match action {
            UiAction::None => {}
            UiAction::Save => {
                let path = self.current_project_path.as_deref().unwrap_or(&self.save_as_path).to_owned();
                match save_project(&mut self.scene, &path, &self.grid_settings) {
                    Ok(()) => self.current_project_path = Some(path),
                    Err(error) => eprintln!("Failed to save project: {error}"),
                }
            }
            UiAction::SaveAs => {
                if !self.save_as_path.trim().is_empty() {
                    match save_project(&mut self.scene, &self.save_as_path, &self.grid_settings) {
                        Ok(()) => self.current_project_path = Some(self.save_as_path.clone()),
                        Err(error) => eprintln!("Failed to save project: {error}"),
                    }
                }
            }
            UiAction::Undo => self.do_undo(),
            UiAction::Redo => self.do_redo(),
            UiAction::SetMode(m) => self.transform_mode = m,
            UiAction::CreateFolder => {
                let folder = self.scene.spawn(Transform::default());
                self.scene.set_name(folder, "Collection");
                self.scene.world.entity_mut(folder).insert(Folder);
                let selected = self.selected;
                let selected_parent = selected.and_then(|entity| self.scene.world.get::<Parent>(entity).map(|parent| parent.0));
                let folder_parent = selected
                    .filter(|entity| self.scene.world.get::<Folder>(*entity).is_some())
                    .or(selected_parent)
                    .or(self.grid_entity);
                if let Some(parent) = folder_parent {
                    self.scene.set_parent(folder, parent);
                }
                if let Some(child) = selected {
                    let protected = Some(child) == self.grid_entity || Some(child) == self.cursor_entity;
                    let is_object_or_folder = self.scene.world.get::<Transform>(child).is_some()
                        && (self.scene.world.get::<MeshKindComp>(child).is_some()
                            || self.scene.world.get::<Folder>(child).is_some());
                    if !protected && is_object_or_folder && Some(child) != folder_parent {
                        self.scene.reparent_preserve_world(child, folder);
                    }
                }
                self.selected = Some(folder);
                self.selected_elements.clear();
                self.history.record(capture_snapshot(&mut self.scene));
            }
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
            UiAction::SelectionToCursor => {
                if let (Some(selected), Some(cursor)) = (self.selected, self.cursor_entity) {
                    if let Some(cursor_matrix) = self.scene.global_matrix(cursor) {
                        let cursor_world = cursor_matrix.w_axis.truncate();
                        let local_position = self
                            .scene
                            .world
                            .get::<Parent>(selected)
                            .and_then(|parent| self.scene.global_matrix(parent.0))
                            .map(|parent_matrix| {
                                parent_matrix.inverse().transform_point3(cursor_world)
                            })
                            .unwrap_or(cursor_world);
                        if let Some(mut transform) = self.scene.world.get_mut::<Transform>(selected)
                        {
                            transform.translation = local_position;
                        }
                        self.history.record(capture_snapshot(&mut self.scene));
                    }
                }
            }
            UiAction::CopySelected => {
                if let Some(sel) = self.selected {
                    let snap = capture_selected(&mut self.scene, sel);
                    self.clipboard = Some(snap);
                }
            }
            UiAction::PasteSelected => {
                let parent = selected_container(&self.scene, self.selected, self.grid_entity);
                if let Some(snap) = &self.clipboard {
                    let state = self.state.as_mut().unwrap();
                    let offset = Vec3::new(2.0, 2.0, 0.0);
                    for root in &snap.roots {
                        paste_node(&mut self.scene, &mut self.render_map, state, root, parent, offset);
                    }
                    self.selected = None;
                    self.selected_elements.clear();
                    update_edit_handles(state, &mut self.scene, self.edit_mode, self.select_mode, self.selected, &mut self.edit_handles);
                }
            }
            UiAction::SetOriginToCursorConfirmed => {
                if let (Some(sel), Some(cursor_e)) = (self.selected, self.cursor_entity) {
                    if let (Some(cursor_matrix), Some(object_matrix)) = (
                        self.scene.global_matrix(cursor_e),
                        self.scene.global_matrix(sel),
                    ) {
                        let cursor_world = cursor_matrix.w_axis.truncate();
                        let pivot_local = object_matrix.inverse().transform_point3(cursor_world);
                        set_object_pivot(&mut self.scene, sel, pivot_local);
                        self.history.record(capture_snapshot(&mut self.scene));
                    }
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
                let path = self.save_as_path.clone();
                if std::path::PathBuf::from(&path).exists() {
                    match load_project(&mut self.scene, &mut self.render_map, state, &path, &mut self.grid_settings) {
                        Ok((grid, cursor)) => {
                            self.current_project_path = Some(path.clone());
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
                        self.history = History::new();
                        self.history.record(snap);
                        }
                        Err(error) => eprintln!("Failed to open project: {error}"),
                    }
                } else {
                    eprintln!("Project file not found: {path}");
                }
            }
            UiAction::AddCube => {
                let parent = selected_container(&self.scene, self.selected, self.grid_entity);
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
                let parent = selected_container(&self.scene, self.selected, self.grid_entity);
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
                let parent = selected_container(&self.scene, self.selected, self.grid_entity);
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
                let parent = selected_container(&self.scene, self.selected, self.grid_entity);
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
                let parent = selected_container(&self.scene, self.selected, self.grid_entity);
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
            UiAction::SetOriginToCursor => {
                if let (Some(sel), Some(cursor_e)) = (self.selected, self.cursor_entity) {
                    if let (Some(cursor_matrix), Some(object_matrix)) = (
                        self.scene.global_matrix(cursor_e),
                        self.scene.global_matrix(sel),
                    ) {
                        let cursor_world = cursor_matrix.w_axis.truncate();
                        let pivot_local = object_matrix.inverse().transform_point3(cursor_world);
                        set_object_pivot(&mut self.scene, sel, pivot_local);
                        self.history.record(capture_snapshot(&mut self.scene));
                    }
                }
            }
            UiAction::SetOriginToGeometry => {
                if let Some(sel) = self.selected {
                    let pivot = self
                        .scene
                        .world
                        .get::<CustomMesh>(sel)
                        .map(|mesh| {
                            if mesh.vertices.is_empty() {
                                Vec3::ZERO
                            } else {
                                mesh.vertices.iter().copied().sum::<Vec3>()
                                    / mesh.vertices.len() as f32
                            }
                        })
                        .unwrap_or(Vec3::ZERO);
                    set_object_pivot(&mut self.scene, sel, pivot);
                    self.history.record(capture_snapshot(&mut self.scene));
                }
            }
            UiAction::SetOriginToSelection => {
                if let Some(sel) = self.selected {
                    let pivot = self.selected_elements.last().and_then(|selection| {
                        let mesh = self.scene.world.get::<CustomMesh>(sel)?;
                        match selection {
                            Selection::Vertex(index) => mesh.vertices.get(*index).copied(),
                            Selection::Edge(a, b) => {
                                Some((*mesh.vertices.get(*a)? + *mesh.vertices.get(*b)?) * 0.5)
                            }
                            Selection::Face(indices) => {
                                let mut center = Vec3::ZERO;
                                let mut count = 0usize;
                                for index in indices {
                                    if let Some(vertex) = mesh.vertices.get(*index) {
                                        center += *vertex;
                                        count += 1;
                                    }
                                }
                                (count > 0).then_some(center / count as f32)
                            }
                        }
                    });
                    if let Some(pivot) = pivot {
                        set_object_pivot(&mut self.scene, sel, pivot);
                        self.history.record(capture_snapshot(&mut self.scene));
                    }
                }
            }
        }
    }
}
