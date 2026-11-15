use bevy_ecs::prelude::*;
use glam::{Mat4, Quat, Vec3};
use serde::{Deserialize, Serialize};

#[derive(Component, Clone, Copy, Debug, Serialize, Deserialize)]
pub struct Transform {
    pub translation: Vec3,
    pub rotation: Quat,
    pub scale: Vec3,
    pub pivot_offset: Vec3, // Смещение Origin (пивота)
}

impl Default for Transform {
    fn default() -> Self {
        Self {
            translation: Vec3::ZERO,
            rotation: Quat::IDENTITY,
            scale: Vec3::ONE,
            pivot_offset: Vec3::ZERO,
        }
    }
}

impl Transform {
    pub fn from_translation(t: Vec3) -> Self {
        Self { translation: t, ..Default::default() }
    }

    pub fn matrix(&self) -> Mat4 {
        // Матрица: Перемещение * Вращение * Масштаб * Смещение пивота
        Mat4::from_scale_rotation_translation(self.scale, self.rotation, self.translation)
            * Mat4::from_translation(-self.pivot_offset)
    }
}

#[derive(Component, Clone, Copy, Debug)]
pub struct GlobalTransform(pub Mat4);

impl Default for GlobalTransform {
    fn default() -> Self {
        Self(Mat4::IDENTITY)
    }
}