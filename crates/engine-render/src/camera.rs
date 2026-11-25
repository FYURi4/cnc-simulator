
use glam::{Mat4, Vec2, Vec3};

const MIN_DISTANCE: f32 = 0.05;
const MAX_DISTANCE: f32 = 100_000.0;

pub struct Camera {
    pub position: Vec3,
    pub target: Vec3,
    pub distance: f32,

    pub yaw: f32,
    pub pitch: f32,

    pub aspect: f32,
    pub fovy: f32,
    pub znear: f32,
    pub zfar: f32,
}

impl Camera {
    pub fn frame_sphere(&mut self, center: Vec3, radius: f32) {
        let vertical_half_fov = self.fovy * 0.5;
        let horizontal_half_fov =
            (vertical_half_fov.tan() * self.aspect.max(0.001)).atan();

        // Берём более узкий угол, чтобы объект помещался и по ширине, и по высоте.
        let half_fov = vertical_half_fov.min(horizontal_half_fov);

        self.target = center;

        let safe_radius = radius.max(0.05);
        self.distance = (
            safe_radius / half_fov.sin() * 1.25
        ).clamp(MIN_DISTANCE, MAX_DISTANCE);

        self.update_orbit_position();
    }

    pub fn new(aspect: f32) -> Self {
        let position = Vec3::new(6.0, 5.0, 6.0);
        let target = Vec3::ZERO;

        let direction = (target - position).normalize();
        let yaw = direction.z.atan2(direction.x);
        let pitch = direction.y.asin();

        Self {
            position,
            target,
            distance: position.distance(target),
            yaw,
            pitch,
            aspect,
            fovy: 60.0_f32.to_radians(),
            znear: 0.1,
            zfar: 1000.0,
        }
    }

    pub fn forward(&self) -> Vec3 {
        Vec3::new(
            self.yaw.cos() * self.pitch.cos(),
            self.pitch.sin(),
            self.yaw.sin() * self.pitch.cos(),
        )
        .normalize()
    }

    pub fn right(&self) -> Vec3 {
        self.forward()
            .cross(Vec3::Y)
            .normalize_or_zero()
    }

    pub fn up(&self) -> Vec3 {
        self.right()
            .cross(self.forward())
            .normalize_or_zero()
    }

    pub fn view(&self) -> Mat4 {
        Mat4::look_to_rh(self.position, self.forward(), Vec3::Y)
    }

    pub fn proj(&self) -> Mat4 {
        Mat4::perspective_rh(
            self.fovy,
            self.aspect,
            self.znear,
            self.zfar,
        )
    }

    pub fn view_proj(&self) -> Mat4 {
        self.proj() * self.view()
    }

    fn update_orbit_position(&mut self) {
        self.distance = self.distance.clamp(MIN_DISTANCE, MAX_DISTANCE);
        self.position = self.target - self.forward() * self.distance;
    }

    pub fn orbit(&mut self, delta_yaw: f32, delta_pitch: f32) {
        self.yaw += delta_yaw;
        self.pitch = (self.pitch + delta_pitch)
            .clamp(
                -89.9_f32.to_radians(),
                89.9_f32.to_radians(),
            );

        self.update_orbit_position();
    }

    // Свободный обзор с фиксированной позицией камеры.
    pub fn rotate(&mut self, delta_yaw: f32, delta_pitch: f32) {
        self.yaw += delta_yaw;
        self.pitch = (self.pitch + delta_pitch)
            .clamp(
                -89.9_f32.to_radians(),
                89.9_f32.to_radians(),
            );

        self.target = self.position + self.forward() * self.distance;
    }

    // Панорамирование в плоскости экрана.
    // delta — перемещение курсора в пикселях.
    pub fn pan(&mut self, delta: Vec2, viewport_height: f32) {
        if viewport_height <= 0.0 {
            return;
        }

        let world_per_pixel =
            (2.0 * self.distance * (self.fovy * 0.5).tan())
                / viewport_height;

        let movement =
            self.right() * (-delta.x * world_per_pixel)
            + self.up() * (delta.y * world_per_pixel);

        self.position += movement;
        self.target += movement;
    }

    // Положительное значение приближает камеру.
    pub fn zoom(&mut self, scroll: f32) {
        if scroll.abs() < f32::EPSILON {
            return;
        }

        let factor = (-scroll * 0.12).exp();

        self.distance =
            (self.distance * factor).clamp(MIN_DISTANCE, MAX_DISTANCE);

        self.update_orbit_position();
    }

    pub fn set_target(&mut self, target: Vec3) {
        self.target = target;
        self.distance = self.position.distance(target).max(MIN_DISTANCE);

        let direction = (target - self.position).normalize();
        self.yaw = direction.z.atan2(direction.x);
        self.pitch = direction.y.asin().clamp(
            -89.9_f32.to_radians(),
            89.9_f32.to_radians(),
        );
    }

    pub fn set_view_direction(&mut self, dir: Vec3) {
        self.position = self.target + dir.normalize() * self.distance;
        let direction = (self.target - self.position).normalize();
        self.yaw = direction.z.atan2(direction.x);
        self.pitch = direction.y.asin().clamp(
            -89.9_f32.to_radians(),
            89.9_f32.to_radians(),
        );
    }

    pub fn reset(&mut self) {
        self.target = Vec3::ZERO;
        self.position = Vec3::new(6.0, 5.0, 6.0);

        let direction = (self.target - self.position).normalize();
        self.yaw = direction.z.atan2(direction.x);
        self.pitch = direction.y.asin();
        self.distance = self.position.distance(self.target);
    }

    // Перемещение камеры вместе с точкой фокуса.
    // Используется существующим WASD/QE.
    pub fn translate(&mut self, delta: Vec3) {
        self.position += delta;
        self.target += delta;
    }
}
