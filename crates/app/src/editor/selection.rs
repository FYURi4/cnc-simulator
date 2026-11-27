use crate::geometry::*;
use crate::types::*;
use bevy_ecs::prelude::*;
use engine_core::{Children, Parent, Scene};
use engine_render::mesh::Mesh;
use engine_render::state::{ObjectKind, State};
use glam::{Mat4, Vec3};
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
                let count =
                    get_handles_for_mode(select_mode, &faces, &edges, &custom.vertices).len();
                for _ in 0..count {
                    let idx = state.add_object(
                        Mesh::sphere_solid(&state.device),
                        ObjectKind::Gizmo,
                        None,
                    );
                    state.set_object_tint(idx, [0.0, 1.0, 1.0, 1.0]);
                    edit_handles.push(idx);
                }
            }
        }
    }
}

pub fn update_object_mesh(
    state: &mut State,
    scene: &Scene,
    render_map: &[(Entity, usize)],
    entity: Entity,
    selected_elements: &[Selection],
) {
    if let Some(kind) = scene.world.get::<MeshKindComp>(entity).map(|m| m.0) {
        let new_vertices = build_mesh_vertices(scene, entity, kind, selected_elements);
        if let Some((_, r_idx)) = render_map.iter().find(|(e, _)| *e == entity) {
            state.update_object_vertices(*r_idx, &new_vertices);
        }
    }
}

pub fn delete_entity(
    scene: &mut Scene,
    render_map: &mut Vec<(Entity, usize)>,
    state: &mut State,
    entity: Entity,
) {
    let mut to_remove: Vec<Entity> = Vec::new();
    collect_subtree(scene, entity, &mut to_remove);
    if let Some(p) = scene.world.get::<Parent>(entity).map(|p| p.0) {
        if let Some(mut ch) = scene.world.get_mut::<Children>(p) {
            ch.0.retain(|e| *e != entity);
        }
    }
    for e in &to_remove {
        if let Some((_, idx)) = render_map.iter().find(|(re, _)| re == e) {
            state.set_object_transform(*idx, Mat4::from_scale(Vec3::ZERO));
        }
    }
    render_map.retain(|(e, _)| !to_remove.contains(e));
    for e in to_remove {
        let _ = scene.world.despawn(e);
    }
}

pub fn set_selection(
    state: &mut State,
    render_map: &[(Entity, usize)],
    old: Option<Entity>,
    new: Option<Entity>,
) {
    if let Some(prev) = old {
        if let Some((_, idx)) = render_map.iter().find(|(e, _)| *e == prev) {
            state.set_object_tint(*idx, [1.0, 1.0, 1.0, 1.0]);
        }
    }
    if let Some(entity) = new {
        if let Some((_, idx)) = render_map.iter().find(|(e, _)| *e == entity) {
            state.set_object_tint(*idx, [1.0, 0.9, 0.3, 1.0]);
        }
    }
}

pub fn collect_subtree(scene: &Scene, entity: Entity, out: &mut Vec<Entity>) {
    out.push(entity);
    let children: Vec<Entity> = scene
        .world
        .get::<Children>(entity)
        .map(|c| c.0.clone())
        .unwrap_or_default();
    for c in children {
        collect_subtree(scene, c, out);
    }
}
