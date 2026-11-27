use crate::app::App;
use crate::types::CustomMesh;
use glam::Vec3;

impl App {
    pub(super) fn frame_selected(&mut self) {
        let (center, radius) = if let Some(entity) = self.selected {
            let Some(global_matrix) = self.scene.global_matrix(entity) else {
                return;
            };
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
}
