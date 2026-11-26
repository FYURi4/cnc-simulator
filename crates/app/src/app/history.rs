use glam::{Mat4, Vec3};
use crate::app::App;
use crate::editor::{update_edit_handles, update_object_mesh};
use crate::io::{despawn_all, spawn_saved_node};
use crate::types::{EditMode, SavedScene};

impl App {
pub(super) fn do_undo(&mut self) {
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

pub(super) fn do_redo(&mut self) {
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

pub(super) fn restore_snapshot(&mut self, snap: SavedScene) {
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
}
