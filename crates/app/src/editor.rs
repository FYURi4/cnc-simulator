use std::collections::HashSet;
use glam::{Mat4, Quat, Vec2, Vec3};
use bevy_ecs::prelude::*;
use engine_core::{Children, Parent, Scene, Transform};
use engine_render::mesh::Mesh;
use engine_render::state::{ObjectKind, State};
use crate::types::*;
use crate::geometry::*;

pub mod tools;

pub fn update_edit_handles(
    state: &mut State,
    scene: &mut Scene,
    edit_mode: EditMode,
    select_mode: SelectMode,
    selected: Option<Entity>,
    edit_handles: &mut Vec<usize>,
) {
    for idx in edit_handles.drain(..) {
        state.set_object_transform(idx, Mat4::from_scale(Vec3::ZERO));
    }
    
    if edit_mode == EditMode::Edit {
        if let Some(sel) = selected {
            if let Some(custom) = scene.world.get::<CustomMesh>(sel) {
                let kind = scene.world.get::<MeshKindComp>(sel).unwrap().0;
                let faces = get_current_faces(scene, sel, kind);
                let edges = get_current_edges(&faces);
                let count = get_handles_for_mode(select_mode, &faces, &edges, &custom.vertices).len();
                for _ in 0..count {
                    let idx = state.add_object(Mesh::sphere_solid(&state.device), ObjectKind::Gizmo, None);
                    state.set_object_tint(idx, [0.0, 1.0, 1.0, 1.0]);
                    edit_handles.push(idx);
                }
            }
        }
    }
}

pub fn update_object_mesh(state: &mut State, scene: &Scene, render_map: &[(Entity, usize)], entity: Entity, selected_elements: &[Selection]) {
    if let Some(kind) = scene.world.get::<MeshKindComp>(entity).map(|m| m.0) {
        let new_vertices = build_mesh_vertices(scene, entity, kind, selected_elements);
        if let Some((_, r_idx)) = render_map.iter().find(|(e, _)| *e == entity) {
            state.update_object_vertices(*r_idx, &new_vertices);
        }
    }
}

pub fn delete_entity(scene: &mut Scene, render_map: &mut Vec<(Entity, usize)>, state: &mut State, entity: Entity) {
    let mut to_remove: Vec<Entity> = Vec::new();
    collect_subtree(scene, entity, &mut to_remove);
    if let Some(p) = scene.world.get::<Parent>(entity).map(|p| p.0) {
        if let Some(mut ch) = scene.world.get_mut::<Children>(p) { ch.0.retain(|e| *e != entity); }
    }
    for e in &to_remove {
        if let Some((_, idx)) = render_map.iter().find(|(re, _)| re == e) { state.set_object_transform(*idx, Mat4::from_scale(Vec3::ZERO)); }
    }
    render_map.retain(|(e, _)| !to_remove.contains(e));
    for e in to_remove { let _ = scene.world.despawn(e); }
}

pub fn set_selection(state: &mut State, render_map: &[(Entity, usize)], old: Option<Entity>, new: Option<Entity>) {
    if let Some(prev) = old {
        if let Some((_, idx)) = render_map.iter().find(|(e, _)| *e == prev) { state.set_object_tint(*idx, [1.0, 1.0, 1.0, 1.0]); }
    }
    if let Some(entity) = new {
        if let Some((_, idx)) = render_map.iter().find(|(e, _)| *e == entity) { state.set_object_tint(*idx, [1.0, 0.9, 0.3, 1.0]); }
    }
}

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
    for &idx in &[tx, ty, tz, rx, ry, rz, sx, sy, sz] { state.set_object_transform(idx, hide); }

    GizmoHandles {
        translate: GizmoSet { x: tx, y: ty, z: tz },
        rotate: GizmoSet { x: rx, y: ry, z: rz },
        scale: GizmoSet { x: sx, y: sy, z: sz },
    }
}

pub fn update_gizmo_positions(
    state: &mut State, gizmo: Option<&GizmoHandles>, selected: Option<Entity>, 
    selected_elements: &[Selection], edit_mode: EditMode, mode: TransformMode, scene: &Scene
) {
    let Some(gizmo) = gizmo else { return };
    let hide = Mat4::from_scale(Vec3::ZERO);

    for gs in [&gizmo.translate, &gizmo.rotate, &gizmo.scale] {
        state.set_object_transform(gs.x, hide);
        state.set_object_transform(gs.y, hide);
        state.set_object_transform(gs.z, hide);
    }

    let Some(sel) = selected else { return };
    if scene.world.get::<Locked>(sel).is_some() { return; }
    
    let mut pos = scene.global_matrix(sel).map(|m| m.w_axis.truncate()).unwrap_or(Vec3::ZERO);
    
    // Compute proportional gizmo scale from bounding box
    let gizmo_scale = if let Some(custom) = scene.world.get::<CustomMesh>(sel) {
        if let Some(gt) = scene.global_matrix(sel) {
            let mut min = Vec3::splat(f32::INFINITY);
            let mut max = Vec3::splat(f32::NEG_INFINITY);
            for v in &custom.vertices {
                let wp = gt.transform_point3(*v);
                min = min.min(wp);
                max = max.max(wp);
            }
            let bbox_size = (max - min).length().max(0.001);
            // Gizmo size ~30% of bounding box
            bbox_size * 0.30
        } else { 0.03 }
    } else { 0.03 };
    
    if edit_mode == EditMode::Edit {
        if let Some(custom) = scene.world.get::<CustomMesh>(sel) {
            if let Some(gt) = scene.global_matrix(sel) {
                let mut sum = Vec3::ZERO;
                let mut count = 0;
                for sel_elem in selected_elements {
                    match sel_elem {
                        Selection::Vertex(i) => { sum += custom.vertices[*i]; count += 1; }
                        Selection::Edge(a, b) => { sum += custom.vertices[*a] + custom.vertices[*b]; count += 2; }
                        Selection::Face(indices) => {
                            for &i in indices.iter() { sum += custom.vertices[i]; count += 1; }
                        }
                    }
                }
                if count > 0 {
                    pos = gt.transform_point3(sum / count as f32);
                }
            }
        }
    }

    let active = match mode {
        TransformMode::Translate => &gizmo.translate,
        TransformMode::Rotate => &gizmo.rotate,
        TransformMode::Scale => &gizmo.scale,
    };

    match mode {
        TransformMode::Translate | TransformMode::Scale => {
            state.set_object_transform(active.x, Mat4::from_rotation_translation(Quat::from_rotation_z(-PI / 2.0), pos) * Mat4::from_scale(Vec3::splat(gizmo_scale)));
            state.set_object_transform(active.y, Mat4::from_rotation_translation(Quat::IDENTITY, pos) * Mat4::from_scale(Vec3::splat(gizmo_scale)));
            state.set_object_transform(active.z, Mat4::from_rotation_translation(Quat::from_rotation_x(PI / 2.0), pos) * Mat4::from_scale(Vec3::splat(gizmo_scale)));
        }
        TransformMode::Rotate => {
            state.set_object_transform(active.x, Mat4::from_rotation_translation(Quat::from_rotation_y(-PI / 2.0), pos) * Mat4::from_scale(Vec3::splat(gizmo_scale)));
            state.set_object_transform(active.y, Mat4::from_rotation_translation(Quat::from_rotation_x(-PI / 2.0), pos) * Mat4::from_scale(Vec3::splat(gizmo_scale)));
            state.set_object_transform(active.z, Mat4::from_rotation_translation(Quat::IDENTITY, pos) * Mat4::from_scale(Vec3::splat(gizmo_scale)));
        }
    }
}

pub fn handle_lmb_press(
    state: &mut State, input: &InputState, viewport_size: Vec2, selected: &mut Option<Entity>,
    selected_elements: &mut Vec<Selection>, drag: &mut Option<DragState>, mode: TransformMode, edit_mode: EditMode, select_mode: SelectMode,
    scene: &mut Scene, render_map: &[(Entity, usize)], cursor_entity: Option<Entity>,
) {
    let (ray_o, ray_d) = ray_from_screen(&state.camera, input.cursor_pos, viewport_size);

    let shift = input.keys.contains(&winit::keyboard::KeyCode::ShiftLeft) || input.keys.contains(&winit::keyboard::KeyCode::ShiftRight);
    if shift && edit_mode == EditMode::Object {
        if let Some(cursor_e) = cursor_entity {
            if ray_d.y.abs() > 1e-6 {
                let t = -ray_o.y / ray_d.y;
                if t > 0.0 {
                    let hit = ray_o + ray_d * t;
                    if let Some(mut tr) = scene.world.get_mut::<Transform>(cursor_e) { tr.translation = hit; }
                }
            }
        }
        return;
    }

    if let Some(sel) = *selected {
        let mut gizmo_pos = scene.global_matrix(sel).map(|m| m.w_axis.truncate()).unwrap_or(Vec3::ZERO);
        if edit_mode == EditMode::Edit {
            if let Some(custom) = scene.world.get::<CustomMesh>(sel) {
                if let Some(gt) = scene.global_matrix(sel) {
                    let mut sum = Vec3::ZERO;
                    let mut count = 0;
                    for sel_elem in selected_elements.iter() {
                        match sel_elem {
                            Selection::Vertex(i) => { sum += custom.vertices[*i]; count += 1; }
                            Selection::Edge(a, b) => { sum += custom.vertices[*a] + custom.vertices[*b]; count += 2; }
                            Selection::Face(indices) => {
                                for &i in indices.iter() { sum += custom.vertices[i]; count += 1; }
                            }
                        }
                    }
                    if count > 0 {
                        gizmo_pos = gt.transform_point3(sum / count as f32);
                    }
                }
            }
        }

        let axis_idx = match mode {
            TransformMode::Rotate => pick_gizmo_ring(ray_o, ray_d, gizmo_pos),
            _ => pick_gizmo(ray_o, ray_d, gizmo_pos),
        };
        if let Some(idx) = axis_idx {
            let axis = match idx { 0 => Vec3::X, 1 => Vec3::Y, _ => Vec3::Z };
            let pivot_screen = world_to_screen(&state.camera, viewport_size, gizmo_pos);
            let screen_axis_dir = world_to_screen(&state.camera, viewport_size, gizmo_pos + axis) - pivot_screen;
            let transform = scene.world.get::<Transform>(sel).copied().unwrap_or_default();

            let mut selected_indices = HashSet::new();
            for sel_elem in selected_elements.iter() {
                match sel_elem {
                    Selection::Vertex(i) => { selected_indices.insert(*i); }
                    Selection::Edge(a, b) => { selected_indices.insert(*a); selected_indices.insert(*b); }
                    Selection::Face(indices) => { for &i in indices.iter() { selected_indices.insert(i); } }
                }
            }

            *drag = Some(DragState {
                entity: sel, mode, axis, start_world_pos: gizmo_pos, start_mouse_screen: input.cursor_pos,
                pivot_screen, screen_axis_dir, start_translation: transform.translation,
                start_rotation: transform.rotation, start_scale: transform.scale,
                start_custom_verts: scene.world.get::<CustomMesh>(sel).map(|c| c.vertices.clone()),
                selected_indices: selected_indices.into_iter().collect(),
            });
            return;
        }
    }

    if edit_mode == EditMode::Edit {
        if let Some(sel) = *selected {
            if let Some(custom) = scene.world.get::<CustomMesh>(sel) {
                if let Some(gt) = scene.global_matrix(sel) {
                    let mut best_selection = None;
                    let kind = scene.world.get::<MeshKindComp>(sel).unwrap().0;
                    let faces = get_current_faces(scene, sel, kind);
                    let edges = get_current_edges(&faces);
                    
                    match select_mode {
                        SelectMode::Vertex => {
                            let mut best_dist = 15.0;
                            let mut best_depth = f32::INFINITY;
                            for (i, &v) in custom.vertices.iter().enumerate() {
                                let world_v = gt.transform_point3(v);
                                let screen_v = world_to_screen(&state.camera, viewport_size, world_v);
                                let dist = (screen_v - input.cursor_pos).length();
                                let depth = (world_v - state.camera.position).length();
                                
                                if dist < best_dist || (dist - best_dist).abs() < 2.0 && depth < best_depth {
                                    best_dist = dist;
                                    best_depth = depth;
                                    best_selection = Some(Selection::Vertex(i));
                                }
                            }
                        }
                        SelectMode::Edge => {
                            let mut best_dist = 15.0;
                            let mut best_depth = f32::INFINITY;
                            for (a, b) in edges {
                                let va = gt.transform_point3(custom.vertices[a]);
                                let vb = gt.transform_point3(custom.vertices[b]);
                                let dist = dist_to_segment_screen(&state.camera, viewport_size, input.cursor_pos, va, vb);
                                let mid = (va + vb) * 0.5;
                                let depth = (mid - state.camera.position).length();
                                
                                if dist < best_dist || (dist - best_dist).abs() < 2.0 && depth < best_depth {
                                    best_dist = dist;
                                    best_depth = depth;
                                    best_selection = Some(Selection::Edge(a, b));
                                }
                            }
                        }
                        SelectMode::Face => {
                            let mut best_t = f32::INFINITY;
                            for face in faces {
                                let p0 = gt.transform_point3(custom.vertices[face[0]]);
                                let p1 = gt.transform_point3(custom.vertices[face[1]]);
                                let p2 = gt.transform_point3(custom.vertices[face[2]]);
                                if let Some(t) = ray_triangle(ray_o, ray_d, p0, p1, p2) {
                                    if t < best_t { best_t = t; best_selection = Some(Selection::Face(face.clone())); }
                                }
                                if face.len() > 3 {
                                    let p3 = gt.transform_point3(custom.vertices[face[3]]);
                                    if let Some(t) = ray_triangle(ray_o, ray_d, p0, p2, p3) {
                                        if t < best_t { best_t = t; best_selection = Some(Selection::Face(face.clone())); }
                                    }
                                }
                            }
                        }
                    }
                    
                    if let Some(sel_elem) = best_selection {
                        if shift {
                            if let Some(pos) = selected_elements.iter().position(|x| x == &sel_elem) {
                                selected_elements.remove(pos);
                            } else {
                                selected_elements.push(sel_elem);
                            }
                        } else {
                            selected_elements.clear();
                            selected_elements.push(sel_elem);
                        }
                    } else {
                        if !shift {
                            selected_elements.clear();
                        }
                    }
                    update_object_mesh(state, scene, render_map, sel, selected_elements);
                }
            }
        }
        return;
    }

    let picked = pick_object(scene, render_map, &state.objects, ray_o, ray_d);
    let old = *selected;
    *selected = picked;
    set_selection(state, render_map, old, picked);
    if old != picked {
        selected_elements.clear();
    }
}

pub fn collect_subtree(scene: &Scene, entity: Entity, out: &mut Vec<Entity>) {
    out.push(entity);
    let children: Vec<Entity> = scene.world.get::<Children>(entity).map(|c| c.0.clone()).unwrap_or_default();
    for c in children { collect_subtree(scene, c, out); }
}