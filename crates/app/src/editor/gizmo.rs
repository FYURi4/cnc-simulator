use crate::geometry::*;
use crate::types::*;
use bevy_ecs::prelude::*;
use engine_core::Scene;
use engine_render::mesh::Mesh;
use engine_render::state::{ObjectKind, State};
use glam::{Mat4, Quat, Vec2, Vec3};
pub fn create_gizmo_objects(state: &mut State) -> GizmoHandles {
    let tx = state.add_object(Mesh::arrow(&state.device), ObjectKind::Gizmo, None);
    let ty = state.add_object(Mesh::arrow(&state.device), ObjectKind::Gizmo, None);
    let tz = state.add_object(Mesh::arrow(&state.device), ObjectKind::Gizmo, None);
    state.set_object_tint(tx, [0.95, 0.25, 0.25, 1.0]);
    state.set_object_tint(ty, [0.25, 0.95, 0.25, 1.0]);
    state.set_object_tint(tz, [0.25, 0.45, 0.95, 1.0]);

    let rx = state.add_object(Mesh::ring(&state.device), ObjectKind::Gizmo, None);
    let ry = state.add_object(Mesh::ring(&state.device), ObjectKind::Gizmo, None);
    let rz = state.add_object(Mesh::ring(&state.device), ObjectKind::Gizmo, None);
    state.set_object_tint(rx, [0.95, 0.25, 0.25, 1.0]);
    state.set_object_tint(ry, [0.25, 0.95, 0.25, 1.0]);
    state.set_object_tint(rz, [0.25, 0.45, 0.95, 1.0]);

    let sx = state.add_object(Mesh::arrow(&state.device), ObjectKind::Gizmo, None);
    let sy = state.add_object(Mesh::arrow(&state.device), ObjectKind::Gizmo, None);
    let sz = state.add_object(Mesh::arrow(&state.device), ObjectKind::Gizmo, None);
    state.set_object_tint(sx, [0.95, 0.25, 0.25, 1.0]);
    state.set_object_tint(sy, [0.25, 0.95, 0.25, 1.0]);
    state.set_object_tint(sz, [0.25, 0.45, 0.95, 1.0]);

    let hide = Mat4::from_scale(Vec3::ZERO);
    for &idx in &[tx, ty, tz, rx, ry, rz, sx, sy, sz] {
        state.set_object_transform(idx, hide);
    }

    GizmoHandles {
        translate: GizmoSet {
            x: tx,
            y: ty,
            z: tz,
        },
        rotate: GizmoSet {
            x: rx,
            y: ry,
            z: rz,
        },
        scale: GizmoSet {
            x: sx,
            y: sy,
            z: sz,
        },
    }
}

pub fn gizmo_world_scale(state: &State, viewport_size: Vec2, world_pos: Vec3) -> f32 {
    // The local gizmo arrow is 0.45 units long. Keep it around 85 screen pixels.
    let distance = state.camera.position.distance(world_pos).max(1e-5);
    let viewport_height = viewport_size.y.max(1.0);
    let world_view_height = 2.0 * distance * (state.camera.fovy * 0.5).tan();
    (world_view_height * 85.0 / viewport_height / 0.45).clamp(1e-5, 1e5)
}

pub fn update_gizmo_positions(
    state: &mut State,
    gizmo: Option<&GizmoHandles>,
    selected: Option<Entity>,
    selected_elements: &[Selection],
    edit_mode: EditMode,
    mode: TransformMode,
    scene: &Scene,
    viewport_size: Vec2,
) {
    let Some(gizmo) = gizmo else { return };
    let hide = Mat4::from_scale(Vec3::ZERO);

    for gs in [&gizmo.translate, &gizmo.rotate, &gizmo.scale] {
        state.set_object_transform(gs.x, hide);
        state.set_object_transform(gs.y, hide);
        state.set_object_transform(gs.z, hide);
    }

    let Some(sel) = selected else { return };
    if scene.world.get::<Locked>(sel).is_some() {
        return;
    }

    let mut pos = scene
        .global_matrix(sel)
        .map(|m| m.w_axis.truncate())
        .unwrap_or(Vec3::ZERO);

    if edit_mode == EditMode::Edit {
        if let Some(custom) = scene.world.get::<CustomMesh>(sel) {
            if let Some(gt) = scene.global_matrix(sel) {
                let mut sum = Vec3::ZERO;
                let mut count = 0;
                for sel_elem in selected_elements {
                    match sel_elem {
                        Selection::Vertex(i) => {
                            sum += custom.vertices[*i];
                            count += 1;
                        }
                        Selection::Edge(a, b) => {
                            sum += custom.vertices[*a] + custom.vertices[*b];
                            count += 2;
                        }
                        Selection::Face(indices) => {
                            for &i in indices.iter() {
                                sum += custom.vertices[i];
                                count += 1;
                            }
                        }
                    }
                }
                if count > 0 {
                    pos = gt.transform_point3(sum / count as f32);
                }
            }
        }
    }

    // Calculate scale after edit-mode selection has adjusted the gizmo pivot.
    let gizmo_scale = gizmo_world_scale(state, viewport_size, pos);

    let active = match mode {
        TransformMode::Translate => &gizmo.translate,
        TransformMode::Rotate => &gizmo.rotate,
        TransformMode::Scale => &gizmo.scale,
    };

    match mode {
        TransformMode::Translate | TransformMode::Scale => {
            state.set_object_transform(
                active.x,
                Mat4::from_rotation_translation(gizmo_axis_rotation(0), pos)
                    * Mat4::from_scale(Vec3::splat(gizmo_scale)),
            );
            state.set_object_transform(
                active.y,
                Mat4::from_rotation_translation(gizmo_axis_rotation(1), pos)
                    * Mat4::from_scale(Vec3::splat(gizmo_scale)),
            );
            state.set_object_transform(
                active.z,
                Mat4::from_rotation_translation(gizmo_axis_rotation(2), pos)
                    * Mat4::from_scale(Vec3::splat(gizmo_scale)),
            );
        }
        TransformMode::Rotate => {
            state.set_object_transform(
                active.x,
                Mat4::from_rotation_translation(Quat::from_rotation_y(-PI / 2.0), pos)
                    * Mat4::from_scale(Vec3::splat(gizmo_scale)),
            );
            state.set_object_transform(
                active.y,
                Mat4::from_rotation_translation(Quat::from_rotation_x(-PI / 2.0), pos)
                    * Mat4::from_scale(Vec3::splat(gizmo_scale)),
            );
            state.set_object_transform(
                active.z,
                Mat4::from_rotation_translation(Quat::IDENTITY, pos)
                    * Mat4::from_scale(Vec3::splat(gizmo_scale)),
            );
        }
    }
}
