use glam::{Quat, Vec3};
use engine_core::{Parent, Transform};
use crate::geometry::world_to_screen;
use crate::io::capture_snapshot;
use crate::types::{EditMode, TransformMode, TransformOperation};
use crate::app::App;

impl App {
pub(crate) fn apply_numeric_transform(&mut self) {
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

pub(super) fn begin_transform(&mut self, mode: TransformMode) {
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

pub(super) fn confirm_transform_if_changed(&mut self, label: &str) -> bool {
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

pub(super) fn cancel_transform(&mut self) -> bool {
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
