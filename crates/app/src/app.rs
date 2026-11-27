use std::sync::Arc;
use std::time::Instant;
use bevy_ecs::prelude::Entity;
use glam::{Vec2, Vec3};
use winit::window::Window;
use engine_core::Scene;
use engine_render::state::State;
use crate::types::*;

mod actions;
mod events;
mod camera;
mod history;
mod render;

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

    }
