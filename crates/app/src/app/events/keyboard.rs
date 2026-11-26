use winit::{event::{KeyEvent, ElementState}, keyboard::{KeyCode, PhysicalKey}};
use glam::{Vec2, Vec3};
use bevy_ecs::prelude::*;
use engine_core::{Parent, Transform};
use engine_render::mesh::MeshKind;
use crate::app::{App, ViewDirection};
use crate::editor::*;
use crate::editor::tools::numeric_input;
use crate::geometry::*;
use crate::io::*;
use crate::types::*;

impl App {
    pub(super) fn handle_keyboard_input(&mut self, event: &KeyEvent) {ref event, ..
    }
}
