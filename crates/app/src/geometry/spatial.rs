use bevy_ecs::prelude::*;
use engine_core::Scene;
use engine_render::camera::Camera;
use engine_render::mesh::MeshKind;
use engine_render::state::{ObjectKind, RenderObject};
use glam::{Vec2, Vec3};

use super::topology::{get_current_edges, get_current_faces};
use crate::types::*;

#[expect(
    clippy::too_many_arguments,
    reason = "Snap evaluation needs scene, grid, and renderer context.",
)]
pub fn apply_snap(
    pos: Vec3,
    start_pos: Vec3,
    axis: Vec3,
    cursor_world: Option<Vec3>,
    snap: &SnapSettings,
    grid: &GridSettings,
    scene: &Scene,
    render_map: &[(Entity, usize)],
    objects: &[RenderObject],
    ignore_entity: Entity,
) -> Vec3 {
    if !snap.enabled {
        return pos;
    }

    let direction = axis.normalize_or_zero();
    match snap.target {
        SnapTarget::Cursor => {
            let Some(cursor_world) = cursor_world else {
                return pos;
            };
            if direction.length_squared() < 1e-6 {
                cursor_world
            } else {
                let distance = (cursor_world - start_pos).dot(direction);
                start_pos + direction * distance
            }
        }
        SnapTarget::Increment => {
            let step = snap.increment.max(1e-6);
            let distance = (pos - start_pos).dot(direction);
            start_pos + direction * ((distance / step).round() * step)
        }
        SnapTarget::Grid => {
            // Grid coordinates are expressed in metres; the UI stores millimetres.
            // Grid styles (lines/dots/major dots) share the same underlying lattice.
            let step_x = (grid.step_x / 1000.0).max(1e-6);
            let step_z = (grid.step_y / 1000.0).max(1e-6);

            if direction.length_squared() < 1e-6 {
                return Vec3::new(
                    (pos.x / step_x).round() * step_x,
                    pos.y,
                    (pos.z / step_z).round() * step_z,
                );
            }

            if direction.x.abs() > 0.999 {
                let target_x = (pos.x / step_x).round() * step_x;
                start_pos + direction * ((target_x - start_pos.x) / direction.x)
            } else if direction.z.abs() > 0.999 {
                let target_z = (pos.z / step_z).round() * step_z;
                start_pos + direction * ((target_z - start_pos.z) / direction.z)
            } else {
                // For local/oblique axes, use the effective grid spacing projected
                // onto that axis instead of silently falling back to Snap.increment.
                let projected_step =
                    ((direction.x * step_x).powi(2) + (direction.z * step_z).powi(2)).sqrt();
                let step = if projected_step > 1e-6 {
                    projected_step
                } else {
                    snap.increment.max(1e-6)
                };
                let distance = (pos - start_pos).dot(direction);
                start_pos + direction * ((distance / step).round() * step)
            }
        }
        SnapTarget::Origin => {
            if direction.length_squared() < 1e-6 {
                return if pos.length() < 0.2 { Vec3::ZERO } else { pos };
            }
            let origin_projection = start_pos - direction * start_pos.dot(direction);
            if (pos - origin_projection).length() < 0.2 {
                origin_projection
            } else {
                pos
            }
        }
        SnapTarget::Vertex | SnapTarget::Edge => {
            let mut best_dist = 0.2_f32;
            let mut best_point = pos;

            for (entity, object_idx) in render_map {
                if *entity == ignore_entity
                    || objects
                        .get(*object_idx)
                        .is_none_or(|object| object.kind != ObjectKind::Solid)
                {
                    continue;
                }

                let Some(global) = scene.global_matrix(*entity) else {
                    continue;
                };
                let Some(custom) = scene.world.get::<CustomMesh>(*entity) else {
                    continue;
                };

                if snap.target == SnapTarget::Vertex {
                    for &vertex in &custom.vertices {
                        let world_vertex = global.transform_point3(vertex);
                        let candidate = if direction.length_squared() > 1e-6 {
                            start_pos + direction * (world_vertex - start_pos).dot(direction)
                        } else {
                            world_vertex
                        };
                        let distance = (candidate - pos).length();
                        if distance < best_dist {
                            best_dist = distance;
                            best_point = candidate;
                        }
                    }
                } else {
                    let kind = scene
                        .world
                        .get::<MeshKindComp>(*entity)
                        .map(|component| component.0)
                        .unwrap_or(MeshKind::Cube);
                    let faces = get_current_faces(scene, *entity, kind);
                    for (a, b) in get_current_edges(&faces) {
                        let world_a = global.transform_point3(custom.vertices[a]);
                        let world_b = global.transform_point3(custom.vertices[b]);
                        let edge = world_b - world_a;
                        let edge_len_sq = edge.length_squared();
                        if edge_len_sq < 1e-10 {
                            continue;
                        }
                        let t = ((pos - world_a).dot(edge) / edge_len_sq).clamp(0.0, 1.0);
                        let edge_point = world_a + edge * t;
                        let candidate = if direction.length_squared() > 1e-6 {
                            start_pos + direction * (edge_point - start_pos).dot(direction)
                        } else {
                            edge_point
                        };
                        let distance = (candidate - pos).length();
                        if distance < best_dist {
                            best_dist = distance;
                            best_point = candidate;
                        }
                    }
                }
            }
            best_point
        }
    }
}

pub fn axis_comp(v: Vec3, idx: usize) -> f32 {
    match idx {
        0 => v.x,
        1 => v.y,
        _ => v.z,
    }
}

pub fn set_axis_comp(v: &mut Vec3, idx: usize, val: f32) {
    match idx {
        0 => v.x = val,
        1 => v.y = val,
        _ => v.z = val,
    };
}

pub fn ray_triangle(ro: Vec3, rd: Vec3, a: Vec3, b: Vec3, c: Vec3) -> Option<f32> {
    let edge1 = b - a;
    let edge2 = c - a;
    let h = rd.cross(edge2);
    let det = edge1.dot(h);
    if det.abs() < 1e-6 {
        return None;
    }
    let inv_det = 1.0 / det;
    let s = ro - a;
    let u = s.dot(h) * inv_det;
    if u < 0.0 || u > 1.0 {
        return None;
    }
    let q = s.cross(edge1);
    let v = rd.dot(q) * inv_det;
    if v < 0.0 || u + v > 1.0 {
        return None;
    }
    let t = edge2.dot(q) * inv_det;
    if t > 1e-6 {
        Some(t)
    } else {
        None
    }
}

pub fn dist_to_segment_screen(
    camera: &Camera,
    screen_size: Vec2,
    mouse: Vec2,
    a: Vec3,
    b: Vec3,
) -> f32 {
    let p0 = world_to_screen(camera, screen_size, a);
    let p1 = world_to_screen(camera, screen_size, b);
    let ab = p1 - p0;
    let len_sq = ab.length_squared();
    if len_sq < 1e-6 {
        return (mouse - p0).length();
    }
    let t = (mouse - p0).dot(ab) / len_sq;
    let t = t.clamp(0.0, 1.0);
    let p = p0 + ab * t;
    (mouse - p).length()
}

pub fn ray_from_screen(camera: &Camera, screen_pos: Vec2, screen_size: Vec2) -> (Vec3, Vec3) {
    let ndc_x = (screen_pos.x / screen_size.x) * 2.0 - 1.0;
    let ndc_y = 1.0 - (screen_pos.y / screen_size.y) * 2.0;
    let inv_vp = camera.view_proj().inverse();
    let near_h = inv_vp * glam::vec4(ndc_x, ndc_y, 0.0, 1.0);
    let far_h = inv_vp * glam::vec4(ndc_x, ndc_y, 1.0, 1.0);
    (
        camera.position,
        (far_h.truncate() / far_h.w - near_h.truncate() / near_h.w).normalize(),
    )
}

pub fn world_to_screen(camera: &Camera, screen_size: Vec2, p: Vec3) -> Vec2 {
    let clip = camera.view_proj() * glam::vec4(p.x, p.y, p.z, 1.0);
    if clip.w.abs() < 1e-6 {
        return Vec2::ZERO;
    }
    let ndc = clip.truncate() / clip.w;
    Vec2::new(
        (ndc.x + 1.0) * 0.5 * screen_size.x,
        (1.0 - ndc.y) * 0.5 * screen_size.y,
    )
}

pub fn ray_aabb(orig: Vec3, dir: Vec3, amin: Vec3, amax: Vec3) -> Option<f32> {
    let inv = Vec3::new(
        if dir.x.abs() < 1e-9 { 1e9 } else { 1.0 / dir.x },
        if dir.y.abs() < 1e-9 { 1e9 } else { 1.0 / dir.y },
        if dir.z.abs() < 1e-9 { 1e9 } else { 1.0 / dir.z },
    );
    let t0 = (amin - orig) * inv;
    let t1 = (amax - orig) * inv;
    let tmin = t0.min(t1);
    let tmax = t0.max(t1);
    let enter = tmin.max_element();
    let exit = tmax.min_element();
    if exit < 0.0 || enter > exit {
        None
    } else {
        Some(enter.max(0.0))
    }
}

pub fn pick_object(
    scene: &Scene,
    render_map: &[(Entity, usize)],
    objects: &[RenderObject],
    ray_o: Vec3,
    ray_d: Vec3,
) -> Option<Entity> {
    let mut best: Option<(Entity, f32)> = None;
    for (entity, idx) in render_map {
        if objects[*idx].kind != ObjectKind::Solid {
            continue;
        }
        if scene.world.get::<Locked>(*entity).is_some() {
            continue;
        }

        let Some(gt) = scene.global_matrix(*entity) else {
            continue;
        };

        if let Some(custom) = scene.world.get::<CustomMesh>(*entity) {
            let kind = scene
                .world
                .get::<MeshKindComp>(*entity)
                .map(|m| m.0)
                .unwrap_or(MeshKind::Cube);
            if kind == MeshKind::Plane {
                let p0 = gt.transform_point3(custom.vertices[0]);
                let p1 = gt.transform_point3(custom.vertices[1]);
                let p2 = gt.transform_point3(custom.vertices[2]);
                let p3 = gt.transform_point3(custom.vertices[3]);
                if let Some(t) = ray_triangle(ray_o, ray_d, p0, p1, p2)
                    .or_else(|| ray_triangle(ray_o, ray_d, p0, p2, p3))
                {
                    if best.is_none_or(|(_, d)| t < d) {
                        best = Some((*entity, t));
                    }
                }
            } else {
                let inv = gt.inverse();
                let lo = inv.transform_point3(ray_o);
                let ld = inv.transform_vector3(ray_d);
                if let Some(t) = ray_aabb(lo, ld, Vec3::splat(-0.05), Vec3::splat(0.05)) {
                    let world_hit = gt.transform_point3(lo + ld * t);
                    let dist = (world_hit - ray_o).length();
                    if best.is_none_or(|(_, d)| dist < d) {
                        best = Some((*entity, dist));
                    }
                }
            }
        } else {
            let inv = gt.inverse();
            let lo = inv.transform_point3(ray_o);
            let ld = inv.transform_vector3(ray_d);
            if let Some(t) = ray_aabb(lo, ld, Vec3::splat(-0.05), Vec3::splat(0.05)) {
                let world_hit = gt.transform_point3(lo + ld * t);
                let dist = (world_hit - ray_o).length();
                if best.is_none_or(|(_, d)| dist < d) {
                    best = Some((*entity, dist));
                }
            }
        }
    }
    best.map(|(e, _)| e)
}
