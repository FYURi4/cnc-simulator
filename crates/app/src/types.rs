use bevy_ecs::prelude::*;
use engine_core::Transform;
use engine_render::mesh::MeshKind;
use glam::{Quat, Vec2, Vec3};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::time::Instant;

pub const SAVE_PATH: &str = "scene.json";
pub const PI: f32 = std::f32::consts::PI;

#[derive(Component, Clone, Copy)]
pub struct Locked;

#[derive(Component, Clone, Copy)]
pub struct MeshKindComp(pub MeshKind);

#[derive(Component, Clone, Serialize, Deserialize)]
pub struct CustomMesh {
    pub vertices: Vec<Vec3>,
    #[serde(default)]
    pub deleted_faces: HashSet<usize>,
    #[serde(default)]
    pub custom_faces: Option<Vec<Vec<usize>>>,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum TransformMode {
    Translate,
    Rotate,
    Scale,
}

#[derive(Clone, Copy, PartialEq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NumericInputMode {
    Absolute,
    Relative,
}

impl Default for NumericInputMode {
    fn default() -> Self {
        Self::Absolute
    }
}

impl NumericInputMode {
    fn settings_path() -> std::path::PathBuf {
        std::env::current_dir()
            .unwrap_or_else(|_| std::path::PathBuf::from("."))
            .join("editor_settings.json")
    }

    pub fn load() -> Self {
        std::fs::read_to_string(Self::settings_path())
            .ok()
            .and_then(|json| serde_json::from_str(&json).ok())
            .unwrap_or_default()
    }

    pub fn save(self) {
        if let Ok(json) = serde_json::to_string_pretty(&self) {
            if let Err(error) = std::fs::write(Self::settings_path(), json) {
                eprintln!("Failed to save editor settings: {error}");
            }
        }
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum EditMode {
    Object,
    Edit,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum SelectMode {
    Vertex,
    Edge,
    Face,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum SelectTool {
    Click,
    Box,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Selection {
    Vertex(usize),
    Edge(usize, usize),
    Face(Vec<usize>),
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum SnapTarget {
    Increment,
    Grid,
    Origin,
    Vertex,
    Edge,
}

pub struct SnapSettings {
    pub enabled: bool,
    pub target: SnapTarget,
    pub increment: f32,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum GridType {
    Grid,
    Dots,
    DotsMajor,
    DotsOnly,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum GridUnit {
    Mm,
    M,
}

pub struct GridSettings {
    pub visible: bool,
    pub color: [f32; 4],
    pub grid_type: GridType,
    pub unit: GridUnit,
    pub step_x: f32,
    pub step_y: f32,
}

pub struct InputState {
    pub keys: HashSet<winit::keyboard::KeyCode>,
    pub last_frame: Instant,
    pub cursor_pos: Vec2,
    pub lmb_down: bool,
    pub rmb_down: bool,
    pub mmb_down: bool,
}

impl InputState {
    pub fn new() -> Self {
        Self {
            keys: HashSet::new(),
            last_frame: Instant::now(),
            cursor_pos: Vec2::ZERO,
            lmb_down: false,
            rmb_down: false,
            mmb_down: false,
        }
    }
}

pub struct GizmoSet {
    pub x: usize,
    pub y: usize,
    pub z: usize,
}
pub struct GizmoHandles {
    pub translate: GizmoSet,
    pub rotate: GizmoSet,
    pub scale: GizmoSet,
}

#[derive(Clone)]
pub struct DragState {
    pub entity: Entity,
    pub mode: TransformMode,
    pub axis: Vec3,
    pub start_world_pos: Vec3,
    pub start_mouse_screen: Vec2,
    pub pivot_screen: Vec2,
    pub screen_axis_dir: Vec2,
    pub axis_world_length: f32,
    pub start_translation: Vec3,
    pub start_rotation: Quat,
    pub start_scale: Vec3,
    pub start_custom_verts: Option<Vec<Vec3>>,
    pub selected_indices: Vec<usize>,
}

#[derive(Clone)]
pub struct TransformOperation {
    pub entity: Entity,
    pub mode: TransformMode,

    // Ограничение по оси: X, Y или Z.
    pub axis: Option<Vec3>,

    // Исходная трансформация объекта.
    pub start_translation: Vec3,
    pub start_rotation: Quat,
    pub start_scale: Vec3,

    // Параметры взаимодействия с мышью.
    pub start_mouse_screen: Vec2,
    pub pivot_screen: Vec2,
    pub pivot_world: Vec3,

    // Числовой ввод.
    pub numeric_input: String,
    pub numeric_value: Option<f32>,
}

pub enum UiAction {
    None,
    Save,
    SaveAs,
    Load,
    AddCube,
    AddPlane,
    AddCylinder,
    AddSphere,
    AddPyramid,
    LoadGcode,
    Undo,
    Redo,
    SetMode(TransformMode),
    SetOriginToCursor,
    SetOriginToGeometry,
    ConfirmDelete,
    CopySelected,
    PasteSelected,
    CancelDelete,
    DeleteSelected,
    SetOriginToCursorConfirmed,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct SavedNode {
    pub name: String,
    pub mesh_kind: Option<MeshKind>,
    pub transform: Transform,
    pub is_locked: bool,
    pub custom_mesh: Option<CustomMesh>,
    pub children: Vec<SavedNode>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct SavedScene {
    pub roots: Vec<SavedNode>,
}

pub struct History {
    pub stack: Vec<SavedScene>,
    pub pos: usize,
}

impl History {
    pub fn new() -> Self {
        Self {
            stack: Vec::new(),
            pos: 0,
        }
    }
    pub fn record(&mut self, snapshot: SavedScene) {
        self.stack.truncate(self.pos + 1);
        self.stack.push(snapshot);
        self.pos = self.stack.len() - 1;
        if self.stack.len() > 100 {
            self.stack.remove(0);
            if self.pos > 0 {
                self.pos -= 1;
            }
        }
    }
    pub fn can_undo(&self) -> bool {
        self.pos > 0
    }
    pub fn can_redo(&self) -> bool {
        self.pos + 1 < self.stack.len()
    }
}
