use super::ray_aabb;
use glam::{Mat4, Quat, Vec3};
pub fn gizmo_axis_rotation(axis_index: usize) -> Quat {
    let axis = match axis_index {
        0 => Vec3::X,
        1 => Vec3::Y,
        _ => Vec3::Z,
    };
    Quat::from_rotation_arc(Vec3::Y, axis)
}

pub fn pick_gizmo(ray_o: Vec3, ray_d: Vec3, world_pos: Vec3, scale: f32) -> Option<usize> {
    let amin = Vec3::new(-0.06, 0.0, -0.06);
    let amax = Vec3::new(0.06, 0.45, 0.06);
    let mut best: Option<(usize, f32)> = None;
    for i in 0..3 {
        let rot = gizmo_axis_rotation(i);
        let mat = Mat4::from_rotation_translation(rot, world_pos)
            * Mat4::from_scale(Vec3::splat(scale.max(1e-6)));
        let inv = mat.inverse();
        if let Some(t) = ray_aabb(
            inv.transform_point3(ray_o),
            inv.transform_vector3(ray_d),
            amin,
            amax,
        ) {
            if best.is_none_or(|(_, bt)| t < bt) {
            best = Some((i, t));
        }
    }
    best.map(|(i, _)| i)
}

pub fn pick_gizmo_ring(ray_o: Vec3, ray_d: Vec3, world_pos: Vec3, scale: f32) -> Option<usize> {
    let mut best: Option<(usize, f32)> = None;
    let ring_radius = 0.4 * scale.max(1e-6);
    let hit_tolerance = 0.05 * scale.max(1e-6);
    for (i, normal) in [Vec3::X, Vec3::Y, Vec3::Z].iter().enumerate() {
        let denom = ray_d.dot(*normal);
        if denom.abs() < 1e-6 {
            continue;
        }
        let t = (world_pos - ray_o).dot(*normal) / denom;
        if t < 0.0 {
            continue;
        }
        let offset = (ray_o + ray_d * t) - world_pos;
        let in_plane = offset - *normal * offset.dot(*normal);
        if (in_plane.length() - ring_radius).abs() < hit_tolerance {
            if best.map_or(true, |(_, bt)| t < bt) {
                best = Some((i, t));
            }
        }
    }
    best.map(|(i, _)| i)
}
