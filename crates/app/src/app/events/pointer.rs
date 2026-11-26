use std::collections::HashSet;
use glam::{Mat4, Quat, Vec2, Vec3};
use bevy_ecs::prelude::*;
use winit::event::{ElementState, MouseButton};
use engine_core::{Parent, Transform};
use engine_render::mesh::MeshKind;
use crate::app::App;
use crate::editor::*;
use crate::geometry::*;
use crate::io::*;
use crate::types::*;

impl App {
    pub(super) fn handle_cursor_moved(&mut self, new_pos: Vec2) {position, ..
    }

    pub(super) fn handle_mouse_input(&mut self, btn_state: ElementState, button: MouseButton) {state: btn_state, button, ..
    }
}
