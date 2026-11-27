use glam::{Quat, Vec2, Vec3};
use engine_core::{Parent, Transform};
use crate::geometry::*;
use crate::io::capture_snapshot;
use crate::types::*;
use crate::app::App;

impl App {
pub(crate) fn apply_numeric_transform(&mut self) {
        let Some(op) = self.transform_operation.clone() else {
            return;
        };

        let Some(value) = op.numeric_value else {
            if let Some(mut t) = self.scene.world.get_mut::<Transform>(op.entity) {
                t.translation = op.start_translation;
                t.rotation = op.start_rotation;
                t.scale = op.start_scale;
            }
            return;
        };

        if !value.is_finite() {
            return;
        }

        let absolute = self.numeric_input_mode == NumericInputMode::Absolute;

        match op.mode {
            TransformMode::Translate => {
                let axis = op.axis.unwrap_or(Vec3::X).normalize();
                let world_delta = if absolute {
                    axis * (value - op.pivot_world.dot(axis))
                } else {
                    axis * value
                };

                let local_delta = if let Some(parent) = self.scene.world.get::<Parent>(op.entity) {
                    if let Some(parent_matrix) = self.scene.global_matrix(parent.0) {
                        parent_matrix.inverse().transform_vector3(world_delta)
                    } else {
                        world_delta
                    }
                } else {
                    world_delta
                };

                if let Some(mut t) = self.scene.world.get_mut::<Transform>(op.entity) {
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

                let angle_rad = value.to_radians();
                let rotation = if absolute {
                    let start = op.start_rotation.normalize();
                    let start_vector = Vec3::new(start.x, start.y, start.z);
                    let projected = start_vector.dot(axis);
                    let twist = Quat::from_xyzw(
                        axis.x * projected,
                        axis.y * projected,
                        axis.z * projected,
                        start.w,
                    ).normalize();
                    let current_angle = 2.0 * (Vec3::new(twist.x, twist.y, twist.z).dot(axis))
                        .atan2(twist.w);
                    Quat::from_axis_angle(axis, angle_rad - current_angle) * op.start_rotation
                } else {
                    Quat::from_axis_angle(axis, angle_rad) * op.start_rotation
                };

                if let Some(mut t) = self.scene.world.get_mut::<Transform>(op.entity) {
                    t.rotation = rotation;
                }
            }

            TransformMode::Scale => {
                if !absolute && value <= 0.0 {
                    return;
                }

                let scale = if absolute {
                    match op.axis {
                        Some(axis) if axis == Vec3::X =>
                            Vec3::new(value, op.start_scale.y, op.start_scale.z),
                        Some(axis) if axis == Vec3::Y =>
                            Vec3::new(op.start_scale.x, value, op.start_scale.z),
                        Some(axis) if axis == Vec3::Z =>
                            Vec3::new(op.start_scale.x, op.start_scale.y, value),
                        _ => Vec3::splat(value),
                    }
                } else {
                    match op.axis {
                        Some(axis) if axis == Vec3::X =>
                            Vec3::new(op.start_scale.x * value, op.start_scale.y, op.start_scale.z),
                        Some(axis) if axis == Vec3::Y =>
                            Vec3::new(op.start_scale.x, op.start_scale.y * value, op.start_scale.z),
                        Some(axis) if axis == Vec3::Z =>
                            Vec3::new(op.start_scale.x, op.start_scale.y, op.start_scale.z * value),
                        _ => op.start_scale * value,
                    }
                };

                if scale.is_finite() {
                    if let Some(mut t) = self.scene.world.get_mut::<Transform>(op.entity) {
                        t.scale = scale;
                    }
                }
            }
        }
    }

pub(crate) fn begin_transform(&mut self, mode: TransformMode) {
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

pub(crate) fn confirm_transform_if_changed(&mut self, label: &str) -> bool {
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

pub(crate) fn cancel_transform(&mut self) -> bool {
        let Some(op) = self.transform_operation.take() else { return false; };
        if let Some(mut t) = self.scene.world.get_mut::<Transform>(op.entity) {
            t.translation = op.start_translation;
            t.rotation = op.start_rotation;
            t.scale = op.start_scale;
        }
        println!("Transform cancelled");
        true
    }
pub(crate) fn update_transform_operation(&mut self, mouse_pos: Vec2) {
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
}
