use super::{gizmo_world_scale, set_selection, update_object_mesh};
use crate::geometry::*;
use crate::types::*;
use bevy_ecs::prelude::*;
use engine_core::{Scene, Transform};
use engine_render::state::State;
use glam::{Vec2, Vec3};
use std::collections::HashSet;
pub fn handle_lmb_press(
    state: &mut State,
    input: &InputState,
    viewport_size: Vec2,
    selected: &mut Option<Entity>,
    selected_elements: &mut Vec<Selection>,
    drag: &mut Option<DragState>,
    mode: TransformMode,
    edit_mode: EditMode,
    select_mode: SelectMode,
    scene: &mut Scene,
    render_map: &[(Entity, usize)],
    cursor_entity: Option<Entity>,
) {
    let (ray_o, ray_d) = ray_from_screen(&state.camera, input.cursor_pos, viewport_size);

    let shift = input.keys.contains(&winit::keyboard::KeyCode::ShiftLeft)
        || input.keys.contains(&winit::keyboard::KeyCode::ShiftRight);
    if shift && edit_mode == EditMode::Object {
        if let Some(cursor_e) = cursor_entity {
            if ray_d.y.abs() > 1e-6 {
                let t = -ray_o.y / ray_d.y;
                if t > 0.0 {
                    let hit = ray_o + ray_d * t;
                    if let Some(mut tr) = scene.world.get_mut::<Transform>(cursor_e) {
                        tr.translation = hit;
                    }
                }
            }
        }
        return;
    }

    if let Some(sel) = *selected {
        let mut gizmo_pos = scene
            .global_matrix(sel)
            .map(|m| m.w_axis.truncate())
            .unwrap_or(Vec3::ZERO);
        if edit_mode == EditMode::Edit {
            if let Some(custom) = scene.world.get::<CustomMesh>(sel) {
                if let Some(gt) = scene.global_matrix(sel) {
                    let mut sum = Vec3::ZERO;
                    let mut count = 0;
                    for sel_elem in selected_elements.iter() {
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
                        gizmo_pos = gt.transform_point3(sum / count as f32);
                    }
                }
            }
        }

        let gizmo_scale = gizmo_world_scale(state, viewport_size, gizmo_pos);
        let axis_idx = match mode {
            TransformMode::Rotate => pick_gizmo_ring(ray_o, ray_d, gizmo_pos, gizmo_scale),
            _ => pick_gizmo(ray_o, ray_d, gizmo_pos, gizmo_scale),
        };
        if let Some(idx) = axis_idx {
            let axis = match idx {
                0 => Vec3::X,
                1 => Vec3::Y,
                _ => Vec3::Z,
            };
            let pivot_screen = world_to_screen(&state.camera, viewport_size, gizmo_pos);
            let axis_world_length = gizmo_scale * 0.45;
            let screen_axis_dir = world_to_screen(
                &state.camera,
                viewport_size,
                gizmo_pos + axis * axis_world_length,
            ) - pivot_screen;
            let transform = scene
                .world
                .get::<Transform>(sel)
                .copied()
                .unwrap_or_default();

            let mut selected_indices = HashSet::new();
            for sel_elem in selected_elements.iter() {
                match sel_elem {
                    Selection::Vertex(i) => {
                        selected_indices.insert(*i);
                    }
                    Selection::Edge(a, b) => {
                        selected_indices.insert(*a);
                        selected_indices.insert(*b);
                    }
                    Selection::Face(indices) => {
                        for &i in indices.iter() {
                            selected_indices.insert(i);
                        }
                    }
                }
            }

            *drag = Some(DragState {
                entity: sel,
                mode,
                axis,
                start_world_pos: gizmo_pos,
                start_mouse_screen: input.cursor_pos,
                pivot_screen,
                screen_axis_dir,
                axis_world_length,
                start_rotation: transform.rotation,
                start_scale: transform.scale,
                start_custom_verts: scene
                    .world
                    .get::<CustomMesh>(sel)
                    .map(|c| c.vertices.clone()),
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
                                let screen_v =
                                    world_to_screen(&state.camera, viewport_size, world_v);
                                let dist = (screen_v - input.cursor_pos).length();
                                let depth = (world_v - state.camera.position).length();

                                if dist < best_dist
                                    || (dist - best_dist).abs() < 2.0 && depth < best_depth
                                {
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
                                let dist = dist_to_segment_screen(
                                    &state.camera,
                                    viewport_size,
                                    input.cursor_pos,
                                    va,
                                    vb,
                                );
                                let mid = (va + vb) * 0.5;
                                let depth = (mid - state.camera.position).length();

                                if dist < best_dist
                                    || (dist - best_dist).abs() < 2.0 && depth < best_depth
                                {
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
                                    if t < best_t {
                                        best_t = t;
                                        best_selection = Some(Selection::Face(face.clone()));
                                    }
                                }
                                if face.len() > 3 {
                                    let p3 = gt.transform_point3(custom.vertices[face[3]]);
                                    if let Some(t) = ray_triangle(ray_o, ray_d, p0, p2, p3) {
                                        if t < best_t {
                                            best_t = t;
                                            best_selection = Some(Selection::Face(face.clone()));
                                        }
                                    }
                                }
                            }
                        }
                    }

                    if let Some(sel_elem) = best_selection {
                        if shift {
                            if let Some(pos) = selected_elements.iter().position(|x| x == &sel_elem)
                            {
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
