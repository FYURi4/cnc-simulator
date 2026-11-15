use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;
use anyhow::Result;
use glam::{Mat4, Quat, Vec2, Vec3};
use serde::{Deserialize, Serialize};
use winit::{
    application::ApplicationHandler,
    event::{ElementState, MouseButton, WindowEvent},
    event_loop::{ActiveEventLoop, EventLoop},
    keyboard::{KeyCode, PhysicalKey},
    window::{Window, WindowId},
};

use bevy_ecs::entity::Entity;
use engine_core::{Children, Parent, Scene, Transform};
use engine_render::camera::Camera;
use engine_render::mesh::{Mesh, MeshKind, Vertex};
use engine_render::state::{ObjectKind, State};

const SAVE_PATH: &str = "scene.json";
const PI: f32 = std::f32::consts::PI;

#[derive(bevy_ecs::component::Component, Clone, Copy)]
struct Locked;

#[derive(bevy_ecs::component::Component, Clone, Copy)]
struct MeshKindComp(pub MeshKind);

#[derive(bevy_ecs::component::Component, Clone)]
struct CustomMesh {
    pub vertices: Vec<Vec3>,
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum TransformMode {
    Translate,
    Rotate,
    Scale,
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum EditMode {
    Object,
    Edit,
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum SelectMode {
    Vertex,
    Edge,
    Face,
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum SelectTool {
    Click,
    Box,
}

#[derive(Clone, Debug, PartialEq)]
enum Selection {
    Vertex(usize),
    Edge(usize, usize),
    Face(Vec<usize>),
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum SnapTarget {
    Increment,
    Grid,
    Origin,
    Vertex,
    Edge,
}

struct SnapSettings {
    enabled: bool,
    target: SnapTarget,
    increment: f32,
}

struct InputState {
    keys: HashSet<KeyCode>,
    last_frame: Instant,
    cursor_pos: Vec2,
    lmb_down: bool,
    rmb_down: bool,
}

impl InputState {
    fn new() -> Self {
        Self {
            keys: HashSet::new(),
            last_frame: Instant::now(),
            cursor_pos: Vec2::ZERO,
            lmb_down: false,
            rmb_down: false,
        }
    }
}

struct GizmoSet {
    x: usize,
    y: usize,
    z: usize,
}

struct GizmoHandles {
    translate: GizmoSet,
    rotate: GizmoSet,
    scale: GizmoSet,
}

#[derive(Clone)]
struct DragState {
    entity: Entity,
    mode: TransformMode,
    axis: Vec3,
    start_world_pos: Vec3,
    start_mouse_screen: Vec2,
    pivot_screen: Vec2,
    screen_axis_dir: Vec2,
    start_translation: Vec3,
    start_rotation: Quat,
    start_scale: Vec3,
    start_custom_verts: Option<Vec<Vec3>>,
    selected_indices: Vec<usize>,
}

enum UiAction {
    None,
    Save,
    Load,
    AddCube,
    AddPlane,
    AddCylinder,
    AddSphere,
    AddPyramid,
    DeleteSelected,
    Undo,
    Redo,
    SetMode(TransformMode),
    SetOriginToCursor,
    SetOriginToGeometry,
}

#[derive(Serialize, Deserialize, Clone)]
struct SavedNode {
    name: String,
    mesh_kind: Option<MeshKind>,
    transform: Transform,
    is_locked: bool,
    custom_vertices: Option<Vec<Vec3>>,
    children: Vec<SavedNode>,
}

#[derive(Serialize, Deserialize, Clone)]
struct SavedScene {
    roots: Vec<SavedNode>,
}

struct History {
    stack: Vec<SavedScene>,
    pos: usize,
}

impl History {
    fn new() -> Self { Self { stack: Vec::new(), pos: 0 } }
    fn record(&mut self, snapshot: SavedScene) {
        self.stack.truncate(self.pos + 1);
        self.stack.push(snapshot);
        self.pos = self.stack.len() - 1;
        if self.stack.len() > 100 {
            self.stack.remove(0);
            if self.pos > 0 { self.pos -= 1; }
        }
    }
    fn can_undo(&self) -> bool { self.pos > 0 }
    fn can_redo(&self) -> bool { self.pos + 1 < self.stack.len() }
}

struct App {
    state: Option<State>,
    window: Option<Arc<Window>>,
    input: InputState,
    scene: Scene,
    render_map: Vec<(Entity, usize)>,
    gizmo: Option<GizmoHandles>,
    selected: Option<Entity>,
    editing_entity: Option<Entity>,
    drag: Option<DragState>,
    drag_moved: bool,
    viewport_size: Vec2,
    grid_entity: Option<Entity>,
    cursor_entity: Option<Entity>,
    transform_mode: TransformMode,
    edit_mode: EditMode,
    select_mode: SelectMode,
    selected_elements: Vec<Selection>,
    edit_handles: Vec<usize>,
    snap: SnapSettings,
    history: History,
    
    select_tool: SelectTool,
    drag_start: Option<Vec2>,
    drag_end: Option<Vec2>,
    edit_mode_history_start: Option<usize>,
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_none() {
            let window = Arc::new(
                event_loop.create_window(Window::default_attributes().with_title("CNC Simulator"))
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
                self.state.as_mut().unwrap().resize(size);
            }
            WindowEvent::CursorMoved { position, .. } => {
                let new_pos = Vec2::new(position.x as f32, position.y as f32);
                let delta = new_pos - self.input.cursor_pos;
                self.input.cursor_pos = new_pos;

                if self.input.rmb_down {
                    let sens = 0.005;
                    let state = self.state.as_mut().unwrap();
                    state.camera.rotate(delta.x * sens, -delta.y * sens);
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
                                    let proj = d.dot(drag.screen_axis_dir) / len_sq;
                                    let mut new_world = drag.start_world_pos + drag.axis * proj;
                                    
                                    if self.snap.enabled {
                                        let state = self.state.as_ref().unwrap();
                                        new_world = apply_snap(new_world, drag.start_world_pos, drag.axis, &self.snap, &self.scene, &self.render_map, &state.objects, drag.entity);
                                    }

                                    let obj_gt = self.scene.global_matrix(drag.entity).unwrap_or(Mat4::IDENTITY);
                                    let local_new = obj_gt.inverse().transform_point3(new_world);
                                    let delta_local = local_new - obj_gt.inverse().transform_point3(drag.start_world_pos);

                                    let mut custom = start_verts.clone();
                                    for &idx in &drag.selected_indices {
                                        custom[idx] += delta_local;
                                    }

                                    if let Some(mut cm) = self.scene.world.get_mut::<CustomMesh>(drag.entity) {
                                        cm.vertices = custom.clone();
                                    }
                                    
                                    if let Some(kind) = self.scene.world.get::<MeshKindComp>(drag.entity).map(|m| m.0) {
                                        let new_vertices = build_mesh_vertices(kind, &custom, &self.selected_elements);
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
                                        let proj = d.dot(drag.screen_axis_dir) / len_sq;
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
                                    let new_rot = Quat::from_axis_angle(drag.axis, delta_a) * drag.start_rotation;
                                    if let Some(mut t) = self.scene.world.get_mut::<Transform>(drag.entity) { t.rotation = new_rot; }
                                }
                                TransformMode::Scale => {
                                    let len_sq = drag.screen_axis_dir.length_squared();
                                    if len_sq > 1.0 {
                                        let proj = d.dot(drag.screen_axis_dir) / len_sq;
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
            WindowEvent::KeyboardInput { ref event, .. } => {
                if let PhysicalKey::Code(code) = event.physical_key {
                    match event.state {
                        ElementState::Pressed => {
                            let ctrl = self.input.keys.contains(&KeyCode::ControlLeft) || self.input.keys.contains(&KeyCode::ControlRight);
                            match code {
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
                                                self.update_edit_handles();
                                                update_object_mesh(self.state.as_mut().unwrap(), &self.scene, &self.render_map, sel, &[]);
                                            }
                                        }
                                    }
                                }
                                KeyCode::Digit1 => {
                                    if self.edit_mode == EditMode::Edit { self.select_mode = SelectMode::Vertex; self.selected_elements.clear(); update_object_mesh(self.state.as_mut().unwrap(), &self.scene, &self.render_map, self.selected.unwrap(), &[]); }
                                    else { self.transform_mode = TransformMode::Translate; }
                                }
                                KeyCode::Digit2 => {
                                    if self.edit_mode == EditMode::Edit { self.select_mode = SelectMode::Edge; self.selected_elements.clear(); update_object_mesh(self.state.as_mut().unwrap(), &self.scene, &self.render_map, self.selected.unwrap(), &[]); }
                                    else { self.transform_mode = TransformMode::Rotate; }
                                }
                                KeyCode::Digit3 => {
                                    if self.edit_mode == EditMode::Edit { self.select_mode = SelectMode::Face; self.selected_elements.clear(); update_object_mesh(self.state.as_mut().unwrap(), &self.scene, &self.render_map, self.selected.unwrap(), &[]); }
                                    else { self.transform_mode = TransformMode::Scale; }
                                }
                                KeyCode::KeyZ if ctrl => self.do_undo(),
                                KeyCode::KeyY if ctrl => self.do_redo(),
                                _ => {}
                            }
                            self.input.keys.insert(code);
                        }
                        ElementState::Released => { self.input.keys.remove(&code); }
                    }
                }
            }
            WindowEvent::MouseInput { state: btn_state, button, .. } => {
                match button {
                    MouseButton::Left => {
                        if btn_state == ElementState::Pressed {
                            self.input.lmb_down = true;
                            self.drag_moved = false;
                            
                            if self.edit_mode == EditMode::Edit && self.select_tool == SelectTool::Box {
                                // Если нажали ЛКМ и это Box Select (просто инициализируем рамку)
                                // В реальном приложении здесь нужно проверить, не нажали ли мы на гизмо.
                                // Пока для простоты Box Select срабатывает всегда, если не зажато гизмо.
                                self.drag_start = Some(self.input.cursor_pos);
                                self.drag_end = Some(self.input.cursor_pos);
                            } else {
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
                                                        let edges = get_edges(self.scene.world.get::<MeshKindComp>(sel).unwrap().0);
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
                                                        let faces = get_faces(self.scene.world.get::<MeshKindComp>(sel).unwrap().0);
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
                    MouseButton::Right => { self.input.rmb_down = btn_state == ElementState::Pressed; }
                    _ => {}
                }
            }
            WindowEvent::RedrawRequested => {
                let dt = Self::frame_dt(&mut self.input);
                {
                    let state = self.state.as_mut().unwrap();
                    Self::update_camera(&self.input, state, dt);
                }

                self.scene.update();

                {
                    let state = self.state.as_mut().unwrap();
                    for (entity, idx) in &self.render_map {
                        if let Some(mat) = self.scene.global_matrix(*entity) {
                            state.set_object_transform(*idx, mat);
                        }
                    }
                    if self.edit_mode == EditMode::Edit {
                        if let Some(sel) = self.selected {
                            if let Some(custom) = self.scene.world.get::<CustomMesh>(sel) {
                                if let Some(gt) = self.scene.global_matrix(sel) {
                                    for (i, &v) in custom.vertices.iter().enumerate() {
                                        if i < self.edit_handles.len() {
                                            let world = gt.transform_point3(v);
                                            state.set_object_transform(self.edit_handles[i], Mat4::from_translation(world) * Mat4::from_scale(Vec3::splat(0.05)));
                                            
                                            let is_selected = self.selected_elements.iter().any(|s| match s {
                                                Selection::Vertex(idx) => *idx == i,
                                                Selection::Edge(a, b) => *a == i || *b == i,
                                                Selection::Face(indices) => indices.contains(&i),
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

                update_gizmo_positions(self.state.as_mut().unwrap(), self.gizmo.as_ref(), self.selected, &self.selected_elements, self.edit_mode, self.transform_mode, &self.scene);

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
                let drag_start = self.drag_start;
                let drag_end = self.drag_end;
                let state = self.state.as_mut().unwrap();

                let result = state.render_with_ui(|ctx| {
                    egui::TopBottomPanel::top("top_bar").show(ctx, |ui| {
                        egui::MenuBar::new().ui(ui, |ui| {
                            ui.menu_button("File", |ui| {
                                if ui.button("💾 Save").clicked() { ui_action = UiAction::Save; ui.close(); }
                                if ui.button("📂 Load").clicked() { ui_action = UiAction::Load; ui.close(); }
                            });
                            ui.menu_button("Add", |ui| {
                                if ui.button("Cube").clicked() { ui_action = UiAction::AddCube; ui.close(); }
                                if ui.button("Plane").clicked() { ui_action = UiAction::AddPlane; ui.close(); }
                                if ui.button("Cylinder").clicked() { ui_action = UiAction::AddCylinder; ui.close(); }
                                if ui.button("Sphere").clicked() { ui_action = UiAction::AddSphere; ui.close(); }
                                if ui.button("Pyramid").clicked() { ui_action = UiAction::AddPyramid; ui.close(); }
                            });
                            if ui.button("🗑 Delete").clicked() { ui_action = UiAction::DeleteSelected; }
                            ui.separator();
                            if ui.add_enabled(can_undo, egui::Button::new("↶")).clicked() { ui_action = UiAction::Undo; }
                            if ui.add_enabled(can_redo, egui::Button::new("↷")).clicked() { ui_action = UiAction::Redo; }
                            
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                egui::ComboBox::from_id_salt("snap_target")
                                    .selected_text(format!("{:?}", snap.target))
                                    .show_ui(ui, |ui| {
                                        ui.selectable_value(&mut snap.target, SnapTarget::Increment, "Increment");
                                        ui.selectable_value(&mut snap.target, SnapTarget::Grid, "Grid");
                                        ui.selectable_value(&mut snap.target, SnapTarget::Origin, "Origin");
                                        ui.selectable_value(&mut snap.target, SnapTarget::Vertex, "Vertex");
                                        ui.selectable_value(&mut snap.target, SnapTarget::Edge, "Edge");
                                    });
                                ui.toggle_value(&mut snap.enabled, "🧲 Snap");
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
                                        let roots = collect_roots(&mut scene);
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
                                        
                                        if edit_m == EditMode::Object {
                                            ui.menu_button("Set Origin", |ui| {
                                                if ui.button("To 3D Cursor").clicked() { ui_action = UiAction::SetOriginToCursor; ui.close(); }
                                                if ui.button("To Geometry").clicked() { ui_action = UiAction::SetOriginToGeometry; ui.close(); }
                                            });
                                            ui.separator();
                                        }

                                        if let Some(mut t) = scene.world.get_mut::<Transform>(sel) {
                                            ui.label("Transform:");
                                            egui::Grid::new("transform_grid").show(ui, |ui| {
                                                ui.label("Location X");
                                                ui.add(egui::DragValue::new(&mut t.translation.x).speed(0.02));
                                                ui.end_row();
                                                ui.label("Location Y");
                                                ui.add(egui::DragValue::new(&mut t.translation.y).speed(0.02));
                                                ui.end_row();
                                                ui.label("Location Z");
                                                ui.add(egui::DragValue::new(&mut t.translation.z).speed(0.02));
                                                ui.end_row();

                                                let (mut rx, mut ry, mut rz) = t.rotation.to_euler(glam::EulerRot::XYZ);
                                                let mut rot_changed = false;
                                                ui.label("Rotation X");
                                                rot_changed |= ui.add(egui::DragValue::new(&mut rx).speed(0.01)).changed();
                                                ui.end_row();
                                                ui.label("Rotation Y");
                                                rot_changed |= ui.add(egui::DragValue::new(&mut ry).speed(0.01)).changed();
                                                ui.end_row();
                                                ui.label("Rotation Z");
                                                rot_changed |= ui.add(egui::DragValue::new(&mut rz).speed(0.01)).changed();
                                                ui.end_row();
                                                if rot_changed {
                                                    t.rotation = glam::Quat::from_euler(glam::EulerRot::XYZ, rx, ry, rz);
                                                }

                                                ui.label("Scale X");
                                                ui.add(egui::DragValue::new(&mut t.scale.x).speed(0.02).range(0.01..=100.0));
                                                ui.end_row();
                                                ui.label("Scale Y");
                                                ui.add(egui::DragValue::new(&mut t.scale.y).speed(0.02).range(0.01..=100.0));
                                                ui.end_row();
                                                ui.label("Scale Z");
                                                ui.add(egui::DragValue::new(&mut t.scale.z).speed(0.02).range(0.01..=100.0));
                                                ui.end_row();
                                            });
                                        }
                                    } else if let Some(cursor_e) = cursor_e {
                                        ui.label("3D Cursor:");
                                        egui::Grid::new("cursor_grid").show(ui, |ui| {
                                            if let Some(mut t) = scene.world.get_mut::<Transform>(cursor_e) {
                                                ui.label("Location X");
                                                ui.add(egui::DragValue::new(&mut t.translation.x).speed(0.05));
                                                ui.end_row();
                                                ui.label("Location Y");
                                                ui.add(egui::DragValue::new(&mut t.translation.y).speed(0.05));
                                                ui.end_row();
                                                ui.label("Location Z");
                                                ui.add(egui::DragValue::new(&mut t.translation.z).speed(0.05));
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
                                painter.rect_stroke(r, 0.0, egui::Stroke::new(1.0, egui::Color32::WHITE), egui::StrokeKind::Outside);
                            });
                    }
                });
                self.scene = scene;
                self.editing_entity = editing;
                self.select_mode = ui_sel_m;
                self.select_tool = ui_sel_tool;

                if let Err(e) = result {
                    eprintln!("render error: {e:?}");
                    event_loop.exit();
                    return;
                }

                if let Some(new_sel) = new_selection {
                    let old = self.selected;
                    if old != Some(new_sel) {
                        self.selected = Some(new_sel);
                        set_selection(self.state.as_mut().unwrap(), &self.render_map, old, self.selected);
                        self.edit_mode = EditMode::Object;
                        self.selected_elements.clear();
                        self.update_edit_handles();
                    }
                }

                self.apply_ui_action(ui_action);
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _el: &ActiveEventLoop) {
        if let Some(window) = &self.window { window.request_redraw(); }
    }
}

impl App {
    fn frame_dt(input: &mut InputState) -> f32 {
        let now = Instant::now();
        let dt = (now - input.last_frame).as_secs_f32().min(0.1);
        input.last_frame = now;
        dt
    }

    fn update_camera(input: &InputState, state: &mut State, dt: f32) {
        let speed = if input.keys.contains(&KeyCode::ShiftLeft) || input.keys.contains(&KeyCode::ShiftRight) { 15.0 } else { 5.0 };
        let forward = state.camera.forward();
        let right = state.camera.right();

        let mut dir = Vec3::ZERO;
        if input.keys.contains(&KeyCode::KeyW) { dir += forward; }
        if input.keys.contains(&KeyCode::KeyS) { dir -= forward; }
        if input.keys.contains(&KeyCode::KeyD) { dir += right; }
        if input.keys.contains(&KeyCode::KeyA) { dir -= right; }
        if input.keys.contains(&KeyCode::KeyE) { dir += Vec3::Y; }
        if input.keys.contains(&KeyCode::KeyQ) { dir -= Vec3::Y; }

        if dir.length_squared() > 0.0 {
            dir = dir.normalize();
            state.camera.translate(dir * speed * dt);
            state.update_camera();
        }
    }

    fn update_edit_handles(&mut self) {
        let state = self.state.as_mut().unwrap();
        for idx in self.edit_handles.drain(..) {
            state.set_object_transform(idx, Mat4::from_scale(Vec3::ZERO));
        }
        if self.edit_mode == EditMode::Edit {
            if let Some(sel) = self.selected {
                if let Some(custom) = self.scene.world.get::<CustomMesh>(sel) {
                    for _ in 0..custom.vertices.len() {
                        // ИСПРАВЛЕНИЕ: Используем sphere_solid вместо sphere
                        let idx = state.add_object(Mesh::sphere_solid(&state.device), ObjectKind::Gizmo, None);
                        state.set_object_tint(idx, [0.0, 1.0, 1.0, 1.0]);
                        self.edit_handles.push(idx);
                    }
                }
            }
        }
    }

    fn do_undo(&mut self) {
        if let Some(start) = self.edit_mode_history_start {
            if self.history.pos <= start {
                self.edit_mode = EditMode::Object;
                self.edit_mode_history_start = None;
                self.selected_elements.clear();
                self.update_edit_handles();
                if let Some(sel) = self.selected {
                    update_object_mesh(self.state.as_mut().unwrap(), &self.scene, &self.render_map, sel, &[]);
                }
                return;
            }
        }
        if self.history.can_undo() {
            self.history.pos -= 1;
            let snap = self.history.stack[self.history.pos].clone();
            self.restore_snapshot(snap);
        }
    }

    fn do_redo(&mut self) {
        if self.history.can_redo() {
            self.history.pos += 1;
            let snap = self.history.stack[self.history.pos].clone();
            self.restore_snapshot(snap);
        }
    }

    fn restore_snapshot(&mut self, snap: SavedScene) {
        let state = self.state.as_mut().unwrap();
        for (_, idx) in &self.render_map { state.set_object_transform(*idx, Mat4::from_scale(Vec3::ZERO)); }
        self.render_map.clear();
        despawn_all(&mut self.scene);

        let mut new_grid = None;
        let mut new_cursor = None;
        for node in snap.roots {
            let is_grid = node.name == "Grid";
            let is_cursor = node.name == "3D Cursor";
            let e = spawn_saved_node(&mut self.scene, &mut self.render_map, state, node, None);
            if is_grid { new_grid = Some(e); }
            if is_cursor { new_cursor = Some(e); }
        }
        self.grid_entity = new_grid;
        self.cursor_entity = new_cursor;
        self.selected = None;
        self.drag = None;
        self.editing_entity = None;
        self.edit_mode = EditMode::Object;
        self.selected_elements.clear();
        self.update_edit_handles();
    }

    fn apply_ui_action(&mut self, action: UiAction) {
        let Some(state) = self.state.as_mut() else { return };
        match action {
            UiAction::None => {}
            UiAction::Undo => self.do_undo(),
            UiAction::Redo => self.do_redo(),
            UiAction::SetMode(m) => self.transform_mode = m,
            UiAction::Save => { let _ = save_scene(&mut self.scene, SAVE_PATH); }
            UiAction::Load => {
                if PathBuf::from(SAVE_PATH).exists() {
                    if let Ok((grid, cursor)) = load_scene(&mut self.scene, &mut self.render_map, state, SAVE_PATH) {
                        self.grid_entity = Some(grid);
                        self.cursor_entity = Some(cursor);
                        self.selected = None;
                        self.drag = None;
                        self.editing_entity = None;
                        self.edit_mode = EditMode::Object;
                        self.selected_elements.clear();
                        self.update_edit_handles();
                        let snap = capture_snapshot(&mut self.scene);
                        self.history.record(snap);
                    }
                }
            }
            UiAction::AddCube => {
                let parent = self.selected.or(self.grid_entity);
                let e = add_mesh_node(&mut self.scene, &mut self.render_map, state, MeshKind::Cube, parent);
                self.selected = Some(e);
                self.history.record(capture_snapshot(&mut self.scene));
            }
            UiAction::AddPlane => {
                let parent = self.selected.or(self.grid_entity);
                let e = add_mesh_node(&mut self.scene, &mut self.render_map, state, MeshKind::Plane, parent);
                self.selected = Some(e);
                self.history.record(capture_snapshot(&mut self.scene));
            }
            UiAction::AddCylinder => {
                let parent = self.selected.or(self.grid_entity);
                let e = add_mesh_node(&mut self.scene, &mut self.render_map, state, MeshKind::Cylinder, parent);
                self.selected = Some(e);
                self.history.record(capture_snapshot(&mut self.scene));
            }
            UiAction::AddSphere => {
                let parent = self.selected.or(self.grid_entity);
                let e = add_mesh_node(&mut self.scene, &mut self.render_map, state, MeshKind::Sphere, parent);
                self.selected = Some(e);
                self.history.record(capture_snapshot(&mut self.scene));
            }
            UiAction::AddPyramid => {
                let parent = self.selected.or(self.grid_entity);
                let e = add_mesh_node(&mut self.scene, &mut self.render_map, state, MeshKind::Pyramid, parent);
                self.selected = Some(e);
                self.history.record(capture_snapshot(&mut self.scene));
            }
            UiAction::DeleteSelected => {
                if let Some(e) = self.selected {
                    if self.scene.world.get::<Locked>(e).is_none() {
                        self.selected = None;
                        delete_entity(&mut self.scene, &mut self.render_map, state, e);
                        self.history.record(capture_snapshot(&mut self.scene));
                    }
                }
            }
            UiAction::SetOriginToCursor => {
                if let Some(sel) = self.selected {
                    if let Some(cursor_e) = self.cursor_entity {
                        let cursor_pos = self.scene.global_matrix(cursor_e).map(|m| m.w_axis.truncate()).unwrap_or(Vec3::ZERO);
                        let obj_pos = self.scene.global_matrix(sel).map(|m| m.w_axis.truncate()).unwrap_or(Vec3::ZERO);
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

fn apply_snap(pos: Vec3, start_pos: Vec3, axis: Vec3, snap: &SnapSettings, scene: &Scene, render_map: &[(Entity, usize)], objects: &[engine_render::state::RenderObject], ignore_entity: Entity) -> Vec3 {
    if !snap.enabled { return pos; }
    match snap.target {
        SnapTarget::Increment | SnapTarget::Grid => {
            let dist = (pos - start_pos).dot(axis);
            start_pos + axis * ((dist / snap.increment).round() * snap.increment)
        }
        SnapTarget::Origin => {
            if pos.length() < 0.5 { Vec3::ZERO } else { pos }
        }
        SnapTarget::Vertex | SnapTarget::Edge => {
            let mut best_dist = 0.2f32;
            let mut best_point = pos;
            
            for (e, idx) in render_map {
                if *e == ignore_entity { continue; }
                if objects[*idx].kind != ObjectKind::Solid { continue; }
                
                if let Some(gt) = scene.global_matrix(*e) {
                    let (amin, amax) = match objects[*idx].mesh_kind {
                        Some(MeshKind::Plane) => (Vec3::new(-10.0, -0.1, -10.0), Vec3::new(10.0, 0.1, 10.0)),
                        _ => (Vec3::splat(-0.5), Vec3::splat(0.5))
                    };
                    
                    let mut corners = Vec::new();
                    for x in [amin.x, amax.x] { for y in [amin.y, amax.y] { for z in [amin.z, amax.z] {
                        corners.push(gt.transform_point3(Vec3::new(x, y, z)));
                    }}}
                    
                    if snap.target == SnapTarget::Vertex {
                        for c in &corners {
                            let d = (*c - pos).length();
                            if d < best_dist { best_dist = d; best_point = *c; }
                        }
                    } else {
                        let edges = [
                            (0, 1), (1, 3), (3, 2), (2, 0), (4, 5), (5, 7), (7, 6), (6, 4), (0, 4), (1, 5), (2, 6), (3, 7)
                        ];
                        for (i, j) in edges {
                            let mid = (corners[i] + corners[j]) * 0.5;
                            let d = (mid - pos).length();
                            if d < best_dist { best_dist = d; best_point = mid; }
                        }
                    }
                }
            }
            best_point
        }
    }
}

fn axis_index(v: Vec3) -> usize { if v.x.abs() > 0.5 { 0 } else if v.y.abs() > 0.5 { 1 } else { 2 } }
fn axis_comp(v: Vec3, idx: usize) -> f32 { match idx { 0 => v.x, 1 => v.y, _ => v.z } }
fn set_axis_comp(v: &mut Vec3, idx: usize, val: f32) { match idx { 0 => v.x = val, 1 => v.y = val, _ => v.z = val }; }

fn collect_roots(scene: &mut Scene) -> Vec<Entity> {
    let mut q = scene.world.query_filtered::<Entity, bevy_ecs::query::Without<Parent>>();
    q.iter(&scene.world).filter(|e| scene.get_name(*e) != Some("3D Cursor")).collect()
}

fn collect_subtree(scene: &Scene, entity: Entity, out: &mut Vec<Entity>) {
    out.push(entity);
    let children: Vec<Entity> = scene.world.get::<Children>(entity).map(|c| c.0.clone()).unwrap_or_default();
    for c in children { collect_subtree(scene, c, out); }
}

fn despawn_all(scene: &mut Scene) {
    let mut all: Vec<Entity> = Vec::new();
    let mut q = scene.world.query::<Entity>();
    for e in q.iter(&scene.world) { all.push(e); }
    for e in all { let _ = scene.world.despawn(e); }
}

fn add_mesh_node(scene: &mut Scene, render_map: &mut Vec<(Entity, usize)>, state: &mut State, mesh_kind: MeshKind, parent: Option<Entity>) -> Entity {
    let t = Transform { translation: Vec3::new(0.0, 0.5, 0.0), ..Default::default() };
    let e = scene.spawn(t);
    scene.set_name(e, format!("{:?}", mesh_kind));
    scene.world.entity_mut(e).insert(MeshKindComp(mesh_kind));
    
    let verts = match mesh_kind {
        MeshKind::Cube => vec![
            Vec3::new(-0.5, -0.5, 0.5), Vec3::new(0.5, -0.5, 0.5), Vec3::new(0.5, 0.5, 0.5), Vec3::new(-0.5, 0.5, 0.5),
            Vec3::new(0.5, -0.5, -0.5), Vec3::new(-0.5, -0.5, -0.5), Vec3::new(-0.5, 0.5, -0.5), Vec3::new(0.5, 0.5, -0.5),
        ],
        MeshKind::Plane => vec![
            Vec3::new(-10.0, 0.0, 10.0), Vec3::new(10.0, 0.0, 10.0), Vec3::new(10.0, 0.0, -10.0), Vec3::new(-10.0, 0.0, -10.0),
        ],
        MeshKind::Pyramid => vec![
            Vec3::new(-0.5, 0.0, 0.5), Vec3::new(0.5, 0.0, 0.5), Vec3::new(0.5, 0.0, -0.5), Vec3::new(-0.5, 0.0, -0.5),
            Vec3::new(0.0, 1.0, 0.0),
        ],
        MeshKind::Cylinder => {
            let mut verts = vec![Vec3::new(0.0, 0.0, 0.0), Vec3::new(0.0, 1.0, 0.0)];
            for i in 0..32 {
                let a = (i as f32 / 32.0) * std::f32::consts::TAU;
                verts.push(Vec3::new(0.5 * a.cos(), 0.0, 0.5 * a.sin()));
            }
            for i in 0..32 {
                let a = (i as f32 / 32.0) * std::f32::consts::TAU;
                verts.push(Vec3::new(0.5 * a.cos(), 1.0, 0.5 * a.sin()));
            }
            verts
        },
        MeshKind::Sphere => {
            let mut verts = Vec::new();
            let segments = 24;
            let rings = 16;
            let radius = 0.5;
            for i in 0..=rings {
                let v = i as f32 / rings as f32;
                let phi = v * std::f32::consts::PI;
                for j in 0..=segments {
                    let u = j as f32 / segments as f32;
                    let theta = u * std::f32::consts::TAU;
                    let x = radius * phi.sin() * theta.cos();
                    let y = radius * phi.cos();
                    let z = radius * phi.sin() * theta.sin();
                    verts.push(Vec3::new(x, y, z));
                }
            }
            verts
        },
        _ => vec![],
    };
    if !verts.is_empty() {
        scene.world.entity_mut(e).insert(CustomMesh { vertices: verts });
    }

    let mesh = mesh_kind.create(&state.device);
    let idx = state.add_object(mesh, ObjectKind::Solid, Some(mesh_kind));
    render_map.push((e, idx));
    if let Some(p) = parent { scene.set_parent(e, p); }
    e
}

fn update_object_mesh(state: &mut State, scene: &Scene, render_map: &[(Entity, usize)], entity: Entity, selected_elements: &[Selection]) {
    if let Some(kind) = scene.world.get::<MeshKindComp>(entity).map(|m| m.0) {
        if let Some(custom) = scene.world.get::<CustomMesh>(entity) {
            let new_vertices = build_mesh_vertices(kind, &custom.vertices, selected_elements);
            if let Some((_, r_idx)) = render_map.iter().find(|(e, _)| *e == entity) {
                state.update_object_vertices(*r_idx, &new_vertices);
            }
        }
    }
}

fn delete_entity(scene: &mut Scene, render_map: &mut Vec<(Entity, usize)>, state: &mut State, entity: Entity) {
    let mut to_remove: Vec<Entity> = Vec::new();
    collect_subtree(scene, entity, &mut to_remove);
    if let Some(p) = scene.world.get::<Parent>(entity).map(|p| p.0) {
        if let Some(mut ch) = scene.world.get_mut::<Children>(p) { ch.0.retain(|e| *e != entity); }
    }
    for e in &to_remove {
        if let Some((_, idx)) = render_map.iter().find(|(re, _)| re == e) { state.set_object_transform(*idx, Mat4::from_scale(Vec3::ZERO)); }
    }
    render_map.retain(|(e, _)| !to_remove.contains(e));
    for e in to_remove { let _ = scene.world.despawn(e); }
}

fn set_selection(state: &mut State, render_map: &[(Entity, usize)], old: Option<Entity>, new: Option<Entity>) {
    if let Some(prev) = old {
        if let Some((_, idx)) = render_map.iter().find(|(e, _)| *e == prev) { state.set_object_tint(*idx, [1.0, 1.0, 1.0, 1.0]); }
    }
    if let Some(entity) = new {
        if let Some((_, idx)) = render_map.iter().find(|(e, _)| *e == entity) { state.set_object_tint(*idx, [1.0, 0.9, 0.3, 1.0]); }
    }
}

fn capture_snapshot(scene: &mut Scene) -> SavedScene {
    let mut roots_entities: Vec<Entity> = Vec::new();
    let mut q = scene.world.query_filtered::<Entity, bevy_ecs::query::Without<Parent>>();
    for e in q.iter(&scene.world) { roots_entities.push(e); }
    let roots: Vec<SavedNode> = roots_entities.iter().map(|e| serialize_subtree(scene, *e)).collect();
    SavedScene { roots }
}

fn serialize_subtree(scene: &Scene, entity: Entity) -> SavedNode {
    let name = scene.get_name(entity).unwrap_or("Unnamed").to_string();
    let transform = scene.world.get::<Transform>(entity).copied().unwrap_or_default();
    let is_locked = scene.world.get::<Locked>(entity).is_some();
    let mesh_kind = scene.world.get::<MeshKindComp>(entity).map(|m| m.0);
    let custom_vertices = scene.world.get::<CustomMesh>(entity).map(|c| c.vertices.clone());
    let children: Vec<Entity> = scene.world.get::<Children>(entity).map(|c| c.0.clone()).unwrap_or_default();
    SavedNode { name, mesh_kind, transform, is_locked, custom_vertices, children: children.iter().map(|c| serialize_subtree(scene, *c)).collect() }
}

fn save_scene(scene: &mut Scene, path: &str) -> Result<()> {
    let snap = capture_snapshot(scene);
    let json = serde_json::to_string_pretty(&snap)?;
    std::fs::write(path, json)?;
    Ok(())
}

fn load_scene(scene: &mut Scene, render_map: &mut Vec<(Entity, usize)>, state: &mut State, path: &str) -> Result<(Entity, Entity)> {
    let data = std::fs::read_to_string(path)?;
    let saved: SavedScene = serde_json::from_str(&data)?;

    for (e, idx) in render_map.iter() { let _ = e; state.set_object_transform(*idx, Mat4::from_scale(Vec3::ZERO)); }
    render_map.clear();
    despawn_all(scene);

    let mut new_grid = None;
    let mut new_cursor = None;
    for node in saved.roots {
        let is_grid = node.name == "Grid";
        let is_cursor = node.name == "3D Cursor";
        let e = spawn_saved_node(scene, render_map, state, node, None);
        if is_grid { new_grid = Some(e); }
        if is_cursor { new_cursor = Some(e); }
    }

    Ok((new_grid.ok_or_else(|| anyhow::anyhow!("No Grid found"))?, new_cursor.ok_or_else(|| anyhow::anyhow!("No 3D Cursor found"))?))
}

fn spawn_saved_node(scene: &mut Scene, render_map: &mut Vec<(Entity, usize)>, state: &mut State, node: SavedNode, parent: Option<Entity>) -> Entity {
    let e = scene.spawn(node.transform);
    scene.set_name(e, node.name.clone());
    if node.is_locked { scene.world.entity_mut(e).insert(Locked); }
    
    let is_grid = node.name == "Grid";
    let is_cursor = node.name == "3D Cursor";
    let obj_kind = if is_grid { ObjectKind::Grid } else if is_cursor { ObjectKind::Gizmo } else { ObjectKind::Solid };
    let kind_opt = if is_grid { Some(MeshKind::Plane) } else if is_cursor { Some(MeshKind::Cursor3D) } else { node.mesh_kind };

    if let Some(kind) = kind_opt {
        scene.world.entity_mut(e).insert(MeshKindComp(kind));
        if let Some(cv) = node.custom_vertices {
            scene.world.entity_mut(e).insert(CustomMesh { vertices: cv });
        }
        let mesh = kind.create(&state.device);
        let idx = state.add_object(mesh, obj_kind, Some(kind));
        if is_cursor { state.set_object_tint(idx, [1.0, 0.2, 0.2, 1.0]); }
        render_map.push((e, idx));
    }
    if let Some(p) = parent { scene.set_parent(e, p); }
    for child in node.children { spawn_saved_node(scene, render_map, state, child, Some(e)); }
    e
}

fn create_gizmo_objects(state: &mut State) -> GizmoHandles {
    let tx = state.add_object(Mesh::arrow(&state.device), ObjectKind::Gizmo, None);
    let ty = state.add_object(Mesh::arrow(&state.device), ObjectKind::Gizmo, None);
    let tz = state.add_object(Mesh::arrow(&state.device), ObjectKind::Gizmo, None);
    state.set_object_tint(tx, [0.95, 0.25, 0.25, 1.0]);
    state.set_object_tint(ty, [0.25, 0.95, 0.25, 1.0]);
    state.set_object_tint(tz, [0.25, 0.45, 0.95, 1.0]);

    let rx = state.add_object(Mesh::ring(&state.device), ObjectKind::Gizmo, None);
    let ry = state.add_object(Mesh::ring(&state.device), ObjectKind::Gizmo, None);
    let rz = state.add_object(Mesh::ring(&state.device), ObjectKind::Gizmo, None);
    state.set_object_tint(rx, [0.95, 0.25, 0.25, 1.0]);
    state.set_object_tint(ry, [0.25, 0.95, 0.25, 1.0]);
    state.set_object_tint(rz, [0.25, 0.45, 0.95, 1.0]);

    let sx = state.add_object(Mesh::arrow(&state.device), ObjectKind::Gizmo, None);
    let sy = state.add_object(Mesh::arrow(&state.device), ObjectKind::Gizmo, None);
    let sz = state.add_object(Mesh::arrow(&state.device), ObjectKind::Gizmo, None);
    state.set_object_tint(sx, [0.95, 0.25, 0.25, 1.0]);
    state.set_object_tint(sy, [0.25, 0.95, 0.25, 1.0]);
    state.set_object_tint(sz, [0.25, 0.45, 0.95, 1.0]);

    let hide = Mat4::from_scale(Vec3::ZERO);
    for &idx in &[tx, ty, tz, rx, ry, rz, sx, sy, sz] { state.set_object_transform(idx, hide); }

    GizmoHandles {
        translate: GizmoSet { x: tx, y: ty, z: tz },
        rotate: GizmoSet { x: rx, y: ry, z: rz },
        scale: GizmoSet { x: sx, y: sy, z: sz },
    }
}

fn update_gizmo_positions(
    state: &mut State, gizmo: Option<&GizmoHandles>, selected: Option<Entity>, 
    selected_elements: &[Selection], edit_mode: EditMode, mode: TransformMode, scene: &Scene
) {
    let Some(gizmo) = gizmo else { return };
    let hide = Mat4::from_scale(Vec3::ZERO);

    for gs in [&gizmo.translate, &gizmo.rotate, &gizmo.scale] {
        state.set_object_transform(gs.x, hide);
        state.set_object_transform(gs.y, hide);
        state.set_object_transform(gs.z, hide);
    }

    let Some(sel) = selected else { return };
    if scene.world.get::<Locked>(sel).is_some() { return; }
    
    let mut pos = scene.global_matrix(sel).map(|m| m.w_axis.truncate()).unwrap_or(Vec3::ZERO);
    
    if edit_mode == EditMode::Edit {
        if let Some(custom) = scene.world.get::<CustomMesh>(sel) {
            if let Some(gt) = scene.global_matrix(sel) {
                let mut sum = Vec3::ZERO;
                let mut count = 0;
                for sel_elem in selected_elements {
                    match sel_elem {
                        Selection::Vertex(i) => { sum += custom.vertices[*i]; count += 1; }
                        Selection::Edge(a, b) => { sum += custom.vertices[*a] + custom.vertices[*b]; count += 2; }
                        Selection::Face(indices) => {
                            for &i in indices.iter() { sum += custom.vertices[i]; count += 1; }
                        }
                    }
                }
                if count > 0 {
                    pos = gt.transform_point3(sum / count as f32);
                }
            }
        }
    }

    let active = match mode {
        TransformMode::Translate => &gizmo.translate,
        TransformMode::Rotate => &gizmo.rotate,
        TransformMode::Scale => &gizmo.scale,
    };

    match mode {
        TransformMode::Translate | TransformMode::Scale => {
            state.set_object_transform(active.x, Mat4::from_rotation_translation(Quat::from_rotation_z(-PI / 2.0), pos));
            state.set_object_transform(active.y, Mat4::from_rotation_translation(Quat::IDENTITY, pos));
            state.set_object_transform(active.z, Mat4::from_rotation_translation(Quat::from_rotation_x(PI / 2.0), pos));
        }
        TransformMode::Rotate => {
            state.set_object_transform(active.x, Mat4::from_rotation_translation(Quat::from_rotation_y(-PI / 2.0), pos));
            state.set_object_transform(active.y, Mat4::from_rotation_translation(Quat::from_rotation_x(-PI / 2.0), pos));
            state.set_object_transform(active.z, Mat4::from_rotation_translation(Quat::IDENTITY, pos));
        }
    }
}

fn handle_lmb_press(
    state: &mut State, input: &InputState, viewport_size: Vec2, selected: &mut Option<Entity>,
    selected_elements: &mut Vec<Selection>, drag: &mut Option<DragState>, mode: TransformMode, edit_mode: EditMode, select_mode: SelectMode,
    scene: &mut Scene, render_map: &[(Entity, usize)], cursor_entity: Option<Entity>,
) {
    let (ray_o, ray_d) = ray_from_screen(&state.camera, input.cursor_pos, viewport_size);

    let shift = input.keys.contains(&KeyCode::ShiftLeft) || input.keys.contains(&KeyCode::ShiftRight);
    if shift && edit_mode == EditMode::Object {
        if let Some(cursor_e) = cursor_entity {
            if ray_d.y.abs() > 1e-6 {
                let t = -ray_o.y / ray_d.y;
                if t > 0.0 {
                    let hit = ray_o + ray_d * t;
                    if let Some(mut tr) = scene.world.get_mut::<Transform>(cursor_e) { tr.translation = hit; }
                }
            }
        }
        return;
    }

    if let Some(sel) = *selected {
        let mut gizmo_pos = scene.global_matrix(sel).map(|m| m.w_axis.truncate()).unwrap_or(Vec3::ZERO);
        if edit_mode == EditMode::Edit {
            if let Some(custom) = scene.world.get::<CustomMesh>(sel) {
                if let Some(gt) = scene.global_matrix(sel) {
                    let mut sum = Vec3::ZERO;
                    let mut count = 0;
                    for sel_elem in selected_elements.iter() {
                        match sel_elem {
                            Selection::Vertex(i) => { sum += custom.vertices[*i]; count += 1; }
                            Selection::Edge(a, b) => { sum += custom.vertices[*a] + custom.vertices[*b]; count += 2; }
                            Selection::Face(indices) => {
                                for &i in indices.iter() { sum += custom.vertices[i]; count += 1; }
                            }
                        }
                    }
                    if count > 0 {
                        gizmo_pos = gt.transform_point3(sum / count as f32);
                    }
                }
            }
        }

        let axis_idx = match mode {
            TransformMode::Rotate => pick_gizmo_ring(ray_o, ray_d, gizmo_pos),
            _ => pick_gizmo(ray_o, ray_d, gizmo_pos),
        };
        if let Some(idx) = axis_idx {
            let axis = match idx { 0 => Vec3::X, 1 => Vec3::Y, _ => Vec3::Z };
            let pivot_screen = world_to_screen(&state.camera, viewport_size, gizmo_pos);
            let screen_axis_dir = world_to_screen(&state.camera, viewport_size, gizmo_pos + axis) - pivot_screen;
            let transform = scene.world.get::<Transform>(sel).copied().unwrap_or_default();

            let mut selected_indices = HashSet::new();
            for sel_elem in selected_elements.iter() {
                match sel_elem {
                    Selection::Vertex(i) => { selected_indices.insert(*i); }
                    Selection::Edge(a, b) => { selected_indices.insert(*a); selected_indices.insert(*b); }
                    Selection::Face(indices) => { for &i in indices.iter() { selected_indices.insert(i); } }
                }
            }

            *drag = Some(DragState {
                entity: sel, mode, axis, start_world_pos: gizmo_pos, start_mouse_screen: input.cursor_pos,
                pivot_screen, screen_axis_dir, start_translation: transform.translation,
                start_rotation: transform.rotation, start_scale: transform.scale,
                start_custom_verts: scene.world.get::<CustomMesh>(sel).map(|c| c.vertices.clone()),
                selected_indices: selected_indices.into_iter().collect(),
            });
            return;
        }
    }

    if edit_mode == EditMode::Edit {
        if let Some(sel) = *selected {
            if let Some(custom) = scene.world.get::<CustomMesh>(sel) {
                if let Some(gt) = scene.global_matrix(sel) {
                    let mut best_dist = 0.25;
                    let mut best_selection = None;
                    let mut best_t = f32::INFINITY;
                    
                    match select_mode {
                        SelectMode::Vertex => {
                            for (i, &v) in custom.vertices.iter().enumerate() {
                                let world_v = gt.transform_point3(v);
                                let dist = (world_v - (ray_o + ray_d * (world_v - ray_o).dot(ray_d))).length();
                                if dist < best_dist { best_dist = dist; best_selection = Some(Selection::Vertex(i)); }
                            }
                        }
                        SelectMode::Edge => {
                            let edges = get_edges(scene.world.get::<MeshKindComp>(sel).unwrap().0);
                            for (a, b) in edges {
                                let va = gt.transform_point3(custom.vertices[a]);
                                let vb = gt.transform_point3(custom.vertices[b]);
                                let dist = dist_to_segment(ray_o, ray_d, va, vb);
                                if dist < best_dist { best_dist = dist; best_selection = Some(Selection::Edge(a, b)); }
                            }
                        }
                        SelectMode::Face => {
                            let faces = get_faces(scene.world.get::<MeshKindComp>(sel).unwrap().0);
                            for face in faces {
                                let p0 = gt.transform_point3(custom.vertices[face[0]]);
                                let p1 = gt.transform_point3(custom.vertices[face[1]]);
                                let p2 = gt.transform_point3(custom.vertices[face[2]]);
                                let p3 = if face.len() > 3 { gt.transform_point3(custom.vertices[face[3]]) } else { p2 };
                                if let Some(t) = ray_quad(ray_o, ray_d, p0, p1, p2, p3) {
                                    if t < best_t { best_t = t; best_selection = Some(Selection::Face(face.clone())); }
                                }
                            }
                        }
                    }
                    
                    if let Some(sel_elem) = best_selection {
                        if shift {
                            if let Some(pos) = selected_elements.iter().position(|x| x == &sel_elem) {
                                selected_elements.remove(pos);
                            } else {
                                selected_elements.push(sel_elem);
                            }
                        } else {
                            selected_elements.clear();
                            selected_elements.push(sel_elem);
                        }
                    } else {
                        if !shift {
                            selected_elements.clear();
                        }
                    }
                    update_object_mesh(state, scene, render_map, sel, selected_elements);
                }
            }
        }
        return;
    }

    let picked = pick_object(scene, render_map, &state.objects, ray_o, ray_d);
    let old = *selected;
    *selected = picked;
    set_selection(state, render_map, old, picked);
    if old != picked {
        selected_elements.clear();
    }
}

fn get_edges(kind: MeshKind) -> Vec<(usize, usize)> {
    if kind == MeshKind::Cube {
        vec![
            (0, 1), (1, 2), (2, 3), (3, 0),
            (4, 5), (5, 6), (6, 7), (7, 4),
            (0, 4), (1, 5), (2, 6), (3, 7)
        ]
    } else if kind == MeshKind::Plane {
        vec![(0, 1), (1, 2), (2, 3), (3, 0)]
    } else if kind == MeshKind::Pyramid {
        vec![(0,1),(1,2),(2,3),(3,0),(0,4),(1,4),(2,4),(3,4)]
    } else { vec![] }
}

fn get_faces(kind: MeshKind) -> Vec<Vec<usize>> {
    if kind == MeshKind::Cube {
        vec![
            vec![0, 1, 2, 3],
            vec![4, 5, 6, 7],
            vec![1, 4, 7, 2],
            vec![5, 0, 3, 6],
            vec![3, 2, 7, 6],
            vec![5, 4, 1, 0],
        ]
    } else if kind == MeshKind::Plane {
        vec![vec![0, 1, 2, 3]]
    } else if kind == MeshKind::Pyramid {
        vec![
            vec![0, 3, 2, 1],
            vec![0, 1, 4],
            vec![1, 2, 4],
            vec![2, 3, 4],
            vec![3, 0, 4],
        ]
    } else { vec![] }
}

fn dist_to_segment(ro: Vec3, rd: Vec3, a: Vec3, b: Vec3) -> f32 {
    let ab = b - a;
    let t = (ro - a).dot(ab) / ab.length_squared();
    let t = t.clamp(0.0, 1.0);
    let p = a + ab * t;
    (p - (ro + rd * (p - ro).dot(rd))).length()
}

fn ray_quad(ro: Vec3, rd: Vec3, a: Vec3, b: Vec3, c: Vec3, d: Vec3) -> Option<f32> {
    let normal = (b - a).cross(c - a).normalize();
    let denom = rd.dot(normal);
    if denom.abs() < 1e-6 { return None; }
    let t = (a - ro).dot(normal) / denom;
    if t < 0.0 { return None; }
    let p = ro + rd * t;
    
    let area = |v0: Vec3, v1: Vec3, v2: Vec3| (v1 - v0).cross(v2 - v0).length();
    let total = area(a, b, c) + area(a, c, d);
    let p1 = area(p, a, b) + area(p, b, c) + area(p, c, d) + area(p, d, a);
    if (p1 - total).abs() < 0.001 { Some(t) } else { None }
}

fn ray_from_screen(camera: &Camera, screen_pos: Vec2, screen_size: Vec2) -> (Vec3, Vec3) {
    let ndc_x = (screen_pos.x / screen_size.x) * 2.0 - 1.0;
    let ndc_y = 1.0 - (screen_pos.y / screen_size.y) * 2.0;
    let inv_vp = camera.view_proj().inverse();
    let near_h = inv_vp * glam::vec4(ndc_x, ndc_y, 0.0, 1.0);
    let far_h = inv_vp * glam::vec4(ndc_x, ndc_y, 1.0, 1.0);
    (camera.position, (far_h.truncate() / far_h.w - near_h.truncate() / near_h.w).normalize())
}

fn world_to_screen(camera: &Camera, screen_size: Vec2, p: Vec3) -> Vec2 {
    let clip = camera.view_proj() * glam::vec4(p.x, p.y, p.z, 1.0);
    if clip.w.abs() < 1e-6 { return Vec2::ZERO; }
    let ndc = clip.truncate() / clip.w;
    Vec2::new((ndc.x + 1.0) * 0.5 * screen_size.x, (1.0 - ndc.y) * 0.5 * screen_size.y)
}

fn ray_aabb(orig: Vec3, dir: Vec3, amin: Vec3, amax: Vec3) -> Option<f32> {
    let inv = Vec3::new(if dir.x.abs() < 1e-9 { 1e9 } else { 1.0 / dir.x }, if dir.y.abs() < 1e-9 { 1e9 } else { 1.0 / dir.y }, if dir.z.abs() < 1e-9 { 1e9 } else { 1.0 / dir.z });
    let t0 = (amin - orig) * inv;
    let t1 = (amax - orig) * inv;
    let tmin = t0.min(t1);
    let tmax = t0.max(t1);
    let enter = tmin.max_element();
    let exit = tmax.min_element();
    if exit < 0.0 || enter > exit { None } else { Some(enter.max(0.0)) }
}

fn pick_object(scene: &Scene, render_map: &[(Entity, usize)], objects: &[engine_render::state::RenderObject], ray_o: Vec3, ray_d: Vec3) -> Option<Entity> {
    let mut best: Option<(Entity, f32)> = None;
    for (entity, idx) in render_map {
        if objects[*idx].kind != ObjectKind::Solid { continue; }
        if scene.world.get::<Locked>(*entity).is_some() { continue; }

        let Some(gt) = scene.global_matrix(*entity) else { continue };
        
        if let Some(custom) = scene.world.get::<CustomMesh>(*entity) {
            let kind = scene.world.get::<MeshKindComp>(*entity).map(|m| m.0).unwrap_or(MeshKind::Cube);
            if kind == MeshKind::Plane {
                let p0 = gt.transform_point3(custom.vertices[0]);
                let p1 = gt.transform_point3(custom.vertices[1]);
                let p2 = gt.transform_point3(custom.vertices[2]);
                let p3 = gt.transform_point3(custom.vertices[3]);
                if let Some(t) = ray_quad(ray_o, ray_d, p0, p1, p2, p3) {
                    if best.map_or(true, |(_, d)| t < d) { best = Some((*entity, t)); }
                }
            } else {
                let inv = gt.inverse();
                let lo = inv.transform_point3(ray_o);
                let ld = inv.transform_vector3(ray_d);
                if let Some(t) = ray_aabb(lo, ld, Vec3::splat(-0.5), Vec3::splat(0.5)) {
                    let world_hit = gt.transform_point3(lo + ld * t);
                    let dist = (world_hit - ray_o).length();
                    if best.map_or(true, |(_, d)| dist < d) { best = Some((*entity, dist)); }
                }
            }
        } else {
            let inv = gt.inverse();
            let lo = inv.transform_point3(ray_o);
            let ld = inv.transform_vector3(ray_d);
            if let Some(t) = ray_aabb(lo, ld, Vec3::splat(-0.5), Vec3::splat(0.5)) {
                let world_hit = gt.transform_point3(lo + ld * t);
                let dist = (world_hit - ray_o).length();
                if best.map_or(true, |(_, d)| dist < d) { best = Some((*entity, dist)); }
            }
        }
    }
    best.map(|(e, _)| e)
}

fn pick_gizmo(ray_o: Vec3, ray_d: Vec3, world_pos: Vec3) -> Option<usize> {
    let rots = [Quat::from_rotation_z(-PI / 2.0), Quat::IDENTITY, Quat::from_rotation_x(PI / 2.0)];
    let amin = Vec3::new(-0.06, 0.0, -0.06);
    let amax = Vec3::new(0.06, 0.45, 0.06);
    let mut best: Option<(usize, f32)> = None;
    for (i, rot) in rots.iter().enumerate() {
        let mat = Mat4::from_rotation_translation(*rot, world_pos);
        let inv = mat.inverse();
        if let Some(t) = ray_aabb(inv.transform_point3(ray_o), inv.transform_vector3(ray_d), amin, amax) {
            if best.map_or(true, |(_, bt)| t < bt) { best = Some((i, t)); }
        }
    }
    best.map(|(i, _)| i)
}

fn pick_gizmo_ring(ray_o: Vec3, ray_d: Vec3, world_pos: Vec3) -> Option<usize> {
    let mut best: Option<(usize, f32)> = None;
    for (i, normal) in [Vec3::X, Vec3::Y, Vec3::Z].iter().enumerate() {
        let denom = ray_d.dot(*normal);
        if denom.abs() < 1e-6 { continue; }
        let t = (world_pos - ray_o).dot(*normal) / denom;
        if t < 0.0 { continue; }
        let offset = (ray_o + ray_d * t) - world_pos;
        let in_plane = offset - *normal * offset.dot(*normal);
        if (in_plane.length() - 0.4).abs() < 0.05 {
            if best.map_or(true, |(_, bt)| t < bt) { best = Some((i, t)); }
        }
    }
    best.map(|(i, _)| i)
}

fn show_node(ui: &mut egui::Ui, scene: &mut Scene, entity: Entity, selected: Option<Entity>, editing: &mut Option<Entity>, new_selection: &mut Option<Entity>) {
    if scene.get_name(entity) == Some("3D Cursor") { return; }
    let children: Vec<Entity> = scene.world.get::<Children>(entity).map(|c| c.0.clone()).unwrap_or_default();
    let raw_name = scene.get_name(entity).map(|s| s.to_string()).unwrap_or_else(|| format!("{:?}", entity));
    let is_selected = selected == Some(entity);

    let id = ui.make_persistent_id(entity);
    let state = egui::collapsing_header::CollapsingState::load_with_default_open(ui.ctx(), id, true);

    state.show_header(ui, |ui| {
        if *editing == Some(entity) {
            let mut name = raw_name.clone();
            let response = ui.add(egui::TextEdit::singleline(&mut name).desired_width(120.0));
            if response.lost_focus() || ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                if !name.is_empty() { scene.set_name(entity, name); }
                *editing = None;
            }
        } else {
            let label = if is_selected { egui::RichText::new(raw_name.clone()).strong().color(egui::Color32::YELLOW) } else { egui::RichText::new(raw_name.clone()) };
            let resp = ui.add(egui::Button::selectable(is_selected, label));
            let rest_rect = egui::Rect::from_min_max(egui::pos2(resp.rect.right(), resp.rect.top()), ui.max_rect().right_bottom());
            let row_resp = ui.interact(rest_rect, id.with("row_click"), egui::Sense::click());
            let combined = resp.union(row_resp);
            if combined.clicked() { *new_selection = Some(entity); }
            if combined.double_clicked() { *editing = Some(entity); }
        }
    }).body(|ui| {
        for child in children { show_node(ui, scene, child, selected, editing, new_selection); }
    });
}

fn build_default_scene(state: &mut State, scene: &mut Scene, render_map: &mut Vec<(Entity, usize)>) -> (Entity, Entity) {
    let plane_e = scene.spawn(Transform::default());
    scene.world.entity_mut(plane_e).insert(Locked);
    scene.set_name(plane_e, "Grid");
    scene.world.entity_mut(plane_e).insert(MeshKindComp(MeshKind::Plane));
    let plane_idx = state.add_object(Mesh::plane(&state.device, 20.0), ObjectKind::Grid, Some(MeshKind::Plane));
    render_map.push((plane_e, plane_idx));

    let cursor_e = scene.spawn(Transform::default());
    scene.set_name(cursor_e, "3D Cursor");
    scene.world.entity_mut(cursor_e).insert(MeshKindComp(MeshKind::Cursor3D));
    let cursor_idx = state.add_object(Mesh::cursor_3d(&state.device), ObjectKind::Gizmo, Some(MeshKind::Cursor3D));
    state.set_object_tint(cursor_idx, [1.0, 0.2, 0.2, 1.0]);
    render_map.push((cursor_e, cursor_idx));

    let cube_e = scene.spawn(Transform { translation: Vec3::new(0.0, 0.5, 0.0), ..Default::default() });
    scene.set_name(cube_e, "Cube");
    scene.world.entity_mut(cube_e).insert(MeshKindComp(MeshKind::Cube));
    
    let verts = vec![
        Vec3::new(-0.5, -0.5, 0.5), Vec3::new(0.5, -0.5, 0.5), Vec3::new(0.5, 0.5, 0.5), Vec3::new(-0.5, 0.5, 0.5),
        Vec3::new(0.5, -0.5, -0.5), Vec3::new(-0.5, -0.5, -0.5), Vec3::new(-0.5, 0.5, -0.5), Vec3::new(0.5, 0.5, -0.5),
    ];
    scene.world.entity_mut(cube_e).insert(CustomMesh { vertices: verts });

    scene.set_parent(cube_e, plane_e);
    let cube_idx = state.add_object(Mesh::cube(&state.device, 1.0), ObjectKind::Solid, Some(MeshKind::Cube));
    render_map.push((cube_e, cube_idx));

    (plane_e, cursor_e)
}

fn build_mesh_vertices(kind: MeshKind, verts: &[Vec3], selected: &[Selection]) -> Vec<Vertex> {
    let mut out = Vec::new();
    let mut sel_set = HashSet::new();
    for s in selected {
        match s {
            Selection::Vertex(i) => { sel_set.insert(*i); }
            Selection::Edge(a, b) => { sel_set.insert(*a); sel_set.insert(*b); }
            Selection::Face(indices) => { for &i in indices { sel_set.insert(i); } }
        }
    }

    let get_color = |idx: usize| -> [f32; 3] {
        if sel_set.contains(&idx) { [1.0, 0.5, 0.0] } else { [1.0, 1.0, 1.0] }
    };

    if kind == MeshKind::Cube {
        let faces = [
            [0, 1, 2, 3], [4, 5, 6, 7], [1, 4, 7, 2], [5, 0, 3, 6], [3, 2, 7, 6], [5, 4, 1, 0],
        ];
        let normals = [
            Vec3::Z, Vec3::NEG_Z, Vec3::X, Vec3::NEG_X, Vec3::Y, Vec3::NEG_Y
        ];
        for (idx, face) in faces.iter().enumerate() {
            let v0 = verts[face[0]]; let v1 = verts[face[1]]; let v2 = verts[face[2]]; let v3 = verts[face[3]];
            let normal = normals[idx];
            out.extend_from_slice(&[
                Vertex { position: v0.into(), normal: normal.into(), uv: [0.0, 1.0], color: get_color(face[0]) },
                Vertex { position: v1.into(), normal: normal.into(), uv: [1.0, 1.0], color: get_color(face[1]) },
                Vertex { position: v2.into(), normal: normal.into(), uv: [1.0, 0.0], color: get_color(face[2]) },
                Vertex { position: v3.into(), normal: normal.into(), uv: [0.0, 0.0], color: get_color(face[3]) },
            ]);
        }
    } else if kind == MeshKind::Plane {
        let p0 = verts[0]; let p1 = verts[1]; let p2 = verts[2]; let p3 = verts[3];
        out.extend_from_slice(&[
            Vertex { position: p0.into(), normal: Vec3::Y.into(), uv: [0.0, 1.0], color: get_color(0) },
            Vertex { position: p1.into(), normal: Vec3::Y.into(), uv: [1.0, 1.0], color: get_color(1) },
            Vertex { position: p2.into(), normal: Vec3::Y.into(), uv: [1.0, 0.0], color: get_color(2) },
            Vertex { position: p3.into(), normal: Vec3::Y.into(), uv: [0.0, 0.0], color: get_color(3) },
        ]);
    } else if kind == MeshKind::Pyramid {
        let a = verts[0]; let b = verts[1]; let c = verts[2]; let d = verts[3]; let apex = verts[4];
        let n = (d - a).cross(c - a).normalize();
        out.extend_from_slice(&[
            Vertex { position: a.into(), normal: n.into(), uv: [0.0, 0.0], color: get_color(0) },
            Vertex { position: d.into(), normal: n.into(), uv: [1.0, 0.0], color: get_color(3) },
            Vertex { position: c.into(), normal: n.into(), uv: [1.0, 1.0], color: get_color(2) },
            Vertex { position: b.into(), normal: n.into(), uv: [0.0, 1.0], color: get_color(1) },
        ]);
        let sides = [(a, b, 0, 1), (b, c, 1, 2), (c, d, 2, 3), (d, a, 3, 0)];
        for (p0, p1, i0, i1) in sides {
            let n = (p1 - p0).cross(apex - p0).normalize();
            out.extend_from_slice(&[
                Vertex { position: p0.into(), normal: n.into(), uv: [0.0, 0.0], color: get_color(i0) },
                Vertex { position: p1.into(), normal: n.into(), uv: [1.0, 0.0], color: get_color(i1) },
                Vertex { position: apex.into(), normal: n.into(), uv: [0.5, 1.0], color: get_color(4) },
            ]);
        }
    } else if kind == MeshKind::Cylinder {
        let n = 32;
        let center_bot = verts[0];
        let center_top = verts[1];
        
        for i in 0..n {
            let cur = 2 + i;
            let next = 2 + (i + 1) % n;
            out.extend_from_slice(&[
                Vertex { position: center_bot.into(), normal: Vec3::NEG_Y.into(), uv: [0.5, 0.5], color: get_color(0) },
                Vertex { position: verts[next].into(), normal: Vec3::NEG_Y.into(), uv: [0.0, 0.0], color: get_color(next) },
                Vertex { position: verts[cur].into(), normal: Vec3::NEG_Y.into(), uv: [1.0, 0.0], color: get_color(cur) },
            ]);
        }
        for i in 0..n {
            let cur = 34 + i;
            let next = 34 + (i + 1) % n;
            out.extend_from_slice(&[
                Vertex { position: center_top.into(), normal: Vec3::Y.into(), uv: [0.5, 0.5], color: get_color(1) },
                Vertex { position: verts[cur].into(), normal: Vec3::Y.into(), uv: [0.0, 0.0], color: get_color(cur) },
                Vertex { position: verts[next].into(), normal: Vec3::Y.into(), uv: [1.0, 0.0], color: get_color(next) },
            ]);
        }
        for i in 0..n {
            let b0 = 2 + i;
            let b1 = 2 + (i + 1) % n;
            let t0 = 34 + i;
            let t1 = 34 + (i + 1) % n;
            let p0 = verts[b0]; let p1 = verts[b1]; let p2 = verts[t1]; let p3 = verts[t0];
            let normal = (p1 - p0).cross(p2 - p0).normalize();
            out.extend_from_slice(&[
                Vertex { position: p0.into(), normal: normal.into(), uv: [0.0, 1.0], color: get_color(b0) },
                Vertex { position: p1.into(), normal: normal.into(), uv: [1.0, 1.0], color: get_color(b1) },
                Vertex { position: p2.into(), normal: normal.into(), uv: [1.0, 0.0], color: get_color(t1) },
                Vertex { position: p3.into(), normal: normal.into(), uv: [0.0, 0.0], color: get_color(t0) },
            ]);
        }
    } else if kind == MeshKind::Sphere {
        let segments = 24;
        let rings = 16;
        for i in 0..rings {
            for j in 0..segments {
                let a = i * (segments + 1) + j;
                let b = a + segments + 1;
                
                let p0 = verts[a];
                let p1 = verts[b];
                let p2 = verts[a + 1];
                let p3 = verts[b + 1];
                
                out.extend_from_slice(&[
                    Vertex { position: p0.into(), normal: p0.normalize().into(), uv: [0.0, 0.0], color: get_color(a) },
                    Vertex { position: p1.into(), normal: p1.normalize().into(), uv: [0.0, 1.0], color: get_color(b) },
                    Vertex { position: p2.into(), normal: p2.normalize().into(), uv: [1.0, 0.0], color: get_color(a + 1) },
                ]);
                out.extend_from_slice(&[
                    Vertex { position: p1.into(), normal: p1.normalize().into(), uv: [0.0, 1.0], color: get_color(b) },
                    Vertex { position: p3.into(), normal: p3.normalize().into(), uv: [1.0, 1.0], color: get_color(b + 1) },
                    Vertex { position: p2.into(), normal: p2.normalize().into(), uv: [1.0, 0.0], color: get_color(a + 1) },
                ]);
            }
        }
    }
    out
}

fn main() -> Result<()> {
    env_logger::init();
    let event_loop = EventLoop::new()?;
    let mut app = App {
        state: None, window: None, input: InputState::new(), scene: Scene::new(),
        render_map: Vec::new(), gizmo: None, selected: None, editing_entity: None,
        drag: None, drag_moved: false, viewport_size: Vec2::new(1280.0, 720.0),
        grid_entity: None, cursor_entity: None, transform_mode: TransformMode::Translate,
        edit_mode: EditMode::Object, select_mode: SelectMode::Vertex, selected_elements: Vec::new(), edit_handles: Vec::new(),
        snap: SnapSettings { enabled: false, target: SnapTarget::Increment, increment: 0.5 },
        history: History::new(),
        select_tool: SelectTool::Click,
        drag_start: None,
        drag_end: None,
        edit_mode_history_start: None,
    };
    event_loop.run_app(&mut app)?;
    Ok(())
}