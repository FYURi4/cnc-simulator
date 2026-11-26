use std::sync::Arc;
use std::time::Instant;
use anyhow::Result;
use crate::editor::tools::numeric_input;
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

pub struct App {
    pub state: Option<State>,
    pub window: Option<Arc<Window>>,
    pub input: InputState,
    pub scene: Scene,
    pub render_map: Vec<(Entity, usize)>,
    pub gizmo: Option<GizmoHandles>,
    pub selected: Option<Entity>,
    pub editing_entity: Option<Entity>,
    pub drag: Option<DragState>,
    pub drag_moved: bool,
    pub viewport_size: Vec2,
    pub grid_entity: Option<Entity>,
    pub cursor_entity: Option<Entity>,
    pub transform_mode: TransformMode,
    pub edit_mode: EditMode,
    pub select_mode: SelectMode,
    pub selected_elements: Vec<Selection>,
    pub edit_handles: Vec<usize>,
    pub snap: SnapSettings,
    pub grid_settings: GridSettings,
    pub gcode: engine_core::gcode::GCodeProgram,
    pub history: History,
    pub select_tool: SelectTool,
    pub drag_start: Option<Vec2>,
    pub drag_end: Option<Vec2>,
    pub edit_mode_history_start: Option<usize>,
    pub transform_operation: Option<TransformOperation>,
    pub delete_confirm: bool,
    pub delete_confirm_pos: Vec2,
    pub clipboard: Option<SavedScene>,
    pub current_view: ViewDirection,
    pub pending_undo: bool,
    pub pending_redo: bool,
    pub show_color_picker: bool,
    pub color_picker_hex: String,
    pub save_as_path: String,
    pub show_save_as: bool,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum ViewDirection {
    Top, Bottom, Front, Back, Left, Right, Iso,
}

impl ViewDirection {
    pub fn dir(&self) -> Vec3 {
        match self {
            ViewDirection::Top => Vec3::Y,
            ViewDirection::Bottom => -Vec3::Y,
            ViewDirection::Front => Vec3::Z,
            ViewDirection::Back => -Vec3::Z,
            ViewDirection::Left => -Vec3::X,
            ViewDirection::Right => Vec3::X,
            ViewDirection::Iso => Vec3::new(1.0, 0.5, 1.0).normalize(),
        }
    }
    pub fn label(&self) -> &str {
        match self {
            ViewDirection::Top => "Top (Y+)",
            ViewDirection::Bottom => "Bottom (Y-)",
            ViewDirection::Front => "Front (Z+)",
            ViewDirection::Back => "Back (Z-)",
            ViewDirection::Left => "Left (X-)",
            ViewDirection::Right => "Right (X+)",
            ViewDirection::Iso => "Iso",
        }
    }
}

impl App {
    pub fn new() -> Self {
        Self {
            state: None, window: None, input: InputState::new(), scene: Scene::new(),
            render_map: Vec::new(), gizmo: None, selected: None, editing_entity: None,
            drag: None, drag_moved: false, viewport_size: Vec2::new(1280.0, 720.0),
            grid_entity: None, cursor_entity: None, transform_mode: TransformMode::Translate,
            edit_mode: EditMode::Object, select_mode: SelectMode::Face, selected_elements: Vec::new(), edit_handles: Vec::new(),
            snap: SnapSettings { enabled: false, target: SnapTarget::Increment, increment: 0.5 },
            grid_settings: GridSettings { visible: true, color: [0.5, 0.5, 0.5, 1.0], grid_type: GridType::Grid, unit: GridUnit::Mm, step_x: 5.0, step_y: 5.0 },
            gcode: engine_core::gcode::GCodeProgram::default(),
            history: History::new(),
            select_tool: SelectTool::Click,
            drag_start: None,
            drag_end: None,
            edit_mode_history_start: None,
            transform_operation: None,
            delete_confirm: false,
            delete_confirm_pos: Vec2::ZERO,
            clipboard: None,
            current_view: ViewDirection::Iso,
            pending_undo: false,
            pending_redo: false,
            show_color_picker: false,
            color_picker_hex: String::new(),
            save_as_path: String::from("scene.json"),
            show_save_as: false,
        }
    }

    fn apply_numeric_transform(&mut self) {
        let Some(op) = self.transform_operation.clone() else {
            return;
        };

        let Some(value) = op.numeric_value else {
            // Незавершённое или очищенное число:
            // возвращаем исходную трансформацию.
            if let Some(mut t) =
                self.scene.world.get_mut::<Transform>(op.entity)
            {
                t.translation = op.start_translation;
                t.rotation = op.start_rotation;
                t.scale = op.start_scale;
            }
            return;
        };

        if !value.is_finite() {
            return;
        }

        match op.mode {
            TransformMode::Translate => {
                let axis = op.axis.unwrap_or(Vec3::X);
                let world_delta = axis * value;

                let local_delta =
                    if let Some(parent) =
                        self.scene.world.get::<Parent>(op.entity)
                    {
                        if let Some(parent_matrix) =
                            self.scene.global_matrix(parent.0)
                        {
                            parent_matrix
                                .inverse()
                                .transform_vector3(world_delta)
                        } else {
                            world_delta
                        }
                    } else {
                        world_delta
                    };

                if let Some(mut t) =
                    self.scene.world.get_mut::<Transform>(op.entity)
                {
                    t.translation = op.start_translation + local_delta;
                }
            }

            TransformMode::Rotate => {
                let axis = op.axis.unwrap_or_else(|| {
                    self.state
                        .as_ref()
                        .map(|s| s.camera.forward())
                        .unwrap_or(Vec3::Z)
                }).normalize();

                // Пользователь вводит градусы.
                // Quat получает радианы.
                let angle_rad = value.to_radians();

                let rotation =
                    Quat::from_axis_angle(axis, angle_rad)
                        * op.start_rotation;

                if let Some(mut t) =
                    self.scene.world.get_mut::<Transform>(op.entity)
                {
                    t.rotation = rotation;
                }
            }

            TransformMode::Scale => {
                if value <= 0.0 {
                    return;
                }

                let scale = match op.axis {
                    Some(axis) if axis == Vec3::X => {
                        Vec3::new(
                            op.start_scale.x * value,
                            op.start_scale.y,
                            op.start_scale.z,
                        )
                    }

                    Some(axis) if axis == Vec3::Y => {
                        Vec3::new(
                            op.start_scale.x,
                            op.start_scale.y * value,
                            op.start_scale.z,
                        )
                    }

                    Some(axis) if axis == Vec3::Z => {
                        Vec3::new(
                            op.start_scale.x,
                            op.start_scale.y,
                            op.start_scale.z * value,
                        )
                    }

                    _ => op.start_scale * value,
                };

                if let Some(mut t) =
                    self.scene.world.get_mut::<Transform>(op.entity)
                {
                    t.scale = scale;
                }
            }
        }
    }

    fn begin_transform(&mut self, mode: TransformMode) {
        if self.edit_mode != EditMode::Object {
            return;
        }

        let Some(entity) = self.selected else {
            return;
        };

        let (start_translation, start_rotation, start_scale) =
            if let Some(active) = self.transform_operation.clone() {
                if active.mode == mode && active.entity == entity {
                    // Повторная клавиша сбрасывает текущий ввод.
                    if let Some(mut t) =
                        self.scene.world.get_mut::<Transform>(entity)
                    {
                        t.translation = active.start_translation;
                        t.rotation = active.start_rotation;
                        t.scale = active.start_scale;
                    }

                    (
                        active.start_translation,
                        active.start_rotation,
                        active.start_scale,
                    )
                } else {
                    // При переключении инструмента фиксируем предыдущий.
                    self.confirm_transform_if_changed("Mode switch");

                    let Some(t) =
                        self.scene.world.get::<Transform>(entity)
                    else {
                        return;
                    };

                    (t.translation, t.rotation, t.scale)
                }
            } else {
                let Some(t) =
                    self.scene.world.get::<Transform>(entity)
                else {
                    return;
                };

                (t.translation, t.rotation, t.scale)
            };

        let Some(state) = self.state.as_ref() else {
            return;
        };

        let Some(global_matrix) = self.scene.global_matrix(entity) else {
            return;
        };

        let pivot_world = global_matrix.transform_point3(Vec3::ZERO);
        let pivot_screen =
            world_to_screen(&state.camera, self.viewport_size, pivot_world);

        self.transform_operation = Some(TransformOperation {
            entity,
            mode,
            axis: None,

            start_translation,
            start_rotation,
            start_scale,

            start_mouse_screen: self.input.cursor_pos,
            pivot_screen,
            pivot_world,

            numeric_input: String::new(),
            numeric_value: None,
        });

        println!("Transform started: {:?}", mode);
    }
    /// FIX: общий хелпер для подтверждения G/R/S с записью в историю.
    /// Возвращает true, если трансформация была активна и была отменена/подтверждена.
    fn confirm_transform_if_changed(&mut self, label: &str) -> bool {
        let Some(op) = self.transform_operation.take() else { return false; };
        let changed = if let Some(t) = self.scene.world.get::<Transform>(op.entity) {
            t.translation != op.start_translation
                || t.rotation != op.start_rotation
                || t.scale != op.start_scale
        } else { false };
        if changed {
            self.history.record(capture_snapshot(&mut self.scene));
            println!("Transform confirmed ({})", label);
        }
        true
    }

    fn cancel_transform(&mut self) -> bool {
        let Some(op) = self.transform_operation.take() else { return false; };
        if let Some(mut t) = self.scene.world.get_mut::<Transform>(op.entity) {
            t.translation = op.start_translation;
            t.rotation = op.start_rotation;
            t.scale = op.start_scale;
        }
        println!("Transform cancelled");
        true
    }
}

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
            WindowEvent::CursorMoved { position, .. } => {
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
            WindowEvent::KeyboardInput { ref event, .. } => {
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
                WindowEvent::MouseInput { state: btn_state, button, .. } => {
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

        fn update_camera(_input: &InputState, _state: &mut State, _dt: f32) {
        }

        fn update_transform_operation(&mut self, mouse_pos: Vec2) {
            let Some(operation) = self.transform_operation.clone() else { return; };
            let Some(state) = self.state.as_ref() else { return; };
            let delta = mouse_pos - operation.start_mouse_screen;
            if !operation.numeric_input.is_empty() {
                return;
            }

            match operation.mode {
                TransformMode::Translate => {
                    let units_per_pixel =
                        2.0 * state.camera.distance
                        * (state.camera.fovy * 0.5).tan()
                        / self.viewport_size.y.max(1.0);
                    let world_delta = if let Some(axis) = operation.axis {
                        let probe_length = (state.camera.position.distance(operation.pivot_world) * 0.1)
                            .max(1e-5);
                        let axis_screen = world_to_screen(
                            &state.camera,
                            self.viewport_size,
                            operation.pivot_world + axis * probe_length,
                        ) - operation.pivot_screen;
                        let axis_screen_len_sq = axis_screen.length_squared();
                        if axis_screen_len_sq > 1e-6 {
                            axis * (delta.dot(axis_screen) / axis_screen_len_sq * probe_length)
                        } else {
                            Vec3::ZERO
                        }
                    } else {
                        state.camera.right() * delta.x * units_per_pixel
                            - state.camera.up() * delta.y * units_per_pixel
                    };
                    let local_delta = if let Some(parent) = self.scene.world.get::<Parent>(operation.entity) {
                        if let Some(parent_matrix) = self.scene.global_matrix(parent.0) {
                            parent_matrix.inverse().transform_vector3(world_delta)
                        } else { world_delta }
                    } else { world_delta };
                    if let Some(mut transform) = self.scene.world.get_mut::<Transform>(operation.entity) {
                        transform.translation = operation.start_translation + local_delta;
                    }
                }
                TransformMode::Rotate => {
                    let mut angle = if operation.axis.is_some() {
                        delta.x * 0.01
                    } else {
                        let start_vec = operation.start_mouse_screen - operation.pivot_screen;
                        let cur_vec = mouse_pos - operation.pivot_screen;
                        let mut a = cur_vec.y.atan2(cur_vec.x) - start_vec.y.atan2(start_vec.x);
                        if a > PI { a -= 2.0 * PI; }
                        if a < -PI { a += 2.0 * PI; }
                        a
                    };
                    if self.snap.enabled {
                        let step = 15.0_f32.to_radians();
                        angle = (angle / step).round() * step;
                    }
                    let axis = operation.axis.unwrap_or_else(|| state.camera.forward()).normalize();
                    let new_rot = Quat::from_axis_angle(axis, angle) * operation.start_rotation;
                    if let Some(mut transform) = self.scene.world.get_mut::<Transform>(operation.entity) {
                        transform.rotation = new_rot;
                    }
                }
                TransformMode::Scale => {
                    let scale_delta = if let Some(axis) = operation.axis {
                        let probe_length = (state.camera.position.distance(operation.pivot_world) * 0.1)
                            .max(1e-5);
                        let axis_screen = world_to_screen(
                            &state.camera,
                            self.viewport_size,
                            operation.pivot_world + axis * probe_length,
                        ) - operation.pivot_screen;
                        let axis_screen_len_sq = axis_screen.length_squared();
                        if axis_screen_len_sq > 1e-6 {
                            delta.dot(axis_screen) / axis_screen_len_sq * probe_length
                        } else {
                            0.0
                        }
                    } else {
                        delta.x * 0.01
                    };
                    let factor = (1.0 + scale_delta).max(0.01);
                    let new_scale = match operation.axis {
                        Some(a) if a == Vec3::X => Vec3::new(operation.start_scale.x * factor, operation.start_scale.y, operation.start_scale.z),
                        Some(a) if a == Vec3::Y => Vec3::new(operation.start_scale.x, operation.start_scale.y * factor, operation.start_scale.z),
                        Some(a) if a == Vec3::Z => Vec3::new(operation.start_scale.x, operation.start_scale.y, operation.start_scale.z * factor),
                        _ => operation.start_scale * factor,
                    };
                    if let Some(mut transform) = self.scene.world.get_mut::<Transform>(operation.entity) {
                        transform.scale = new_scale;
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
                    update_edit_handles(self.state.as_mut().unwrap(), &mut self.scene, self.edit_mode, self.select_mode, self.selected, &mut self.edit_handles);
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
                self.selected = None;
                self.selected_elements.clear();
                self.edit_handles.clear();
                self.drag = None;
                self.transform_operation = None;
            }
        }

        fn do_redo(&mut self) {
            if self.history.can_redo() {
                self.history.pos += 1;
                let snap = self.history.stack[self.history.pos].clone();
                self.restore_snapshot(snap);
                self.selected = None;
                self.selected_elements.clear();
                self.edit_handles.clear();
                self.drag = None;
                self.transform_operation = None;
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
            self.selected_elements.clear();
            update_edit_handles(self.state.as_mut().unwrap(), &mut self.scene, self.edit_mode, self.select_mode, self.selected, &mut self.edit_handles);
            self.state.as_mut().unwrap().update_camera();
        }

        fn frame_selected(&mut self) {
            let (center, radius) = if let Some(entity) = self.selected {
                let Some(global_matrix) = self.scene.global_matrix(entity) else { return; };
                if let Some(mesh) = self.scene.world.get::<CustomMesh>(entity) {
                    if mesh.vertices.is_empty() {
                        (global_matrix.transform_point3(Vec3::ZERO), 0.05)
                    } else {
                        let mut min = Vec3::splat(f32::INFINITY);
                        let mut max = Vec3::splat(f32::NEG_INFINITY);
                        for vertex in &mesh.vertices {
                            let world_pos = global_matrix.transform_point3(*vertex);
                            min = min.min(world_pos);
                            max = max.max(world_pos);
                        }
                        let center = (min + max) * 0.5;
                        let mut radius: f32 = 0.0;
                        for vertex in &mesh.vertices {
                            let world_pos = global_matrix.transform_point3(*vertex);
                            radius = radius.max(world_pos.distance(center));
                        }
                        (center, radius.max(0.001))
                    }
                } else {
                    (global_matrix.transform_point3(Vec3::ZERO), 0.05)
                }
            } else {
                // Frame the grid (1m plane)
                let grid_radius = 0.75; // 750mm to fit the 1m plane
                (Vec3::ZERO, grid_radius)
            };
            let state = self.state.as_mut().unwrap();
            state.camera.frame_sphere(center, radius);
            state.update_camera();
        }

        fn apply_ui_action(&mut self, action: UiAction) {
            let Some(state) = self.state.as_mut() else { return };
            match action {
                UiAction::None => {}
                UiAction::Save => { let _ = save_scene(&mut self.scene, SAVE_PATH); }
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
                                update_edit_handles(state, &mut self.scene, self.edit_mode, self.select_mode, self.selected, &mut self.edit_handles);
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
                            paste_node(&mut self.scene, &mut self.render_map, state, root, None, offset);
                        }
                        self.selected = None;
                        self.selected_elements.clear();
                        update_edit_handles(state, &mut self.scene, self.edit_mode, self.select_mode, self.selected, &mut self.edit_handles);
                    }
                }
                UiAction::SetOriginToCursorConfirmed => {
                    if let (Some(sel), Some(cursor_e)) = (self.selected, self.cursor_entity) {
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
                UiAction::LoadGcode => {
                    const GCODE_PATH: &str = "C:/project/cnc-simulator/test.nc";
                    match std::fs::read_to_string(GCODE_PATH) {
                        Ok(source) => {
                            let prog = engine_core::gcode::GCodeProgram::parse(&source);
                            println!("G-code loaded: {} segments, {} errors", prog.segments.len(), prog.errors.len());
                            for e in prog.errors.iter().take(5) { println!("  {}", e); }
                            if let (Some(first), Some(last)) = (prog.segments.first(), prog.segments.last()) {
                                println!("  from {:?} to {:?}", first.from, last.to);
                            }
                            self.gcode = prog;
                        }
                        Err(e) => eprintln!("Failed to read {}: {}", GCODE_PATH, e),
                    }
                }
                UiAction::Load => {
                    if std::path::PathBuf::from(SAVE_PATH).exists() {
                        if let Ok((grid, cursor)) = load_scene(&mut self.scene, &mut self.render_map, state, SAVE_PATH) {
                            self.grid_entity = Some(grid);
                            self.cursor_entity = Some(cursor);
                            self.selected = None;
                            self.drag = None;
                            self.editing_entity = None;
                            self.edit_mode = EditMode::Object;
                            self.selected_elements.clear();
                            update_edit_handles(state, &mut self.scene, self.edit_mode, self.select_mode, self.selected, &mut self.edit_handles);
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
                    if self.selected.is_some() {
                        self.delete_confirm = true;
                        self.delete_confirm_pos = Vec2::new(640.0, 360.0);
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
