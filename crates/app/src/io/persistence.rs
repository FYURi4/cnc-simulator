use super::{capture_snapshot, despawn_all};
use crate::types::*;
use anyhow::Result;
use bevy_ecs::prelude::*;
use engine_core::Scene;
use engine_render::mesh::{Mesh, MeshKind};
use engine_render::state::{ObjectKind, State};
use std::fs;

pub fn save_scene(scene: &mut Scene, path: &str) -> Result<()> {
    let snap = capture_snapshot(scene);
    let json = serde_json::to_string_pretty(&snap)?;
    std::fs::write(path, json)?;
    Ok(())
}

pub fn load_scene(
    scene: &mut Scene,
    render_map: &mut Vec<(Entity, usize)>,
    state: &mut State,
    path: &str,
) -> Result<(Entity, Entity)> {
    let data = fs::read_to_string(path)?;
    let saved: SavedScene = serde_json::from_str(&data)?;

    for (e, idx) in render_map.iter() {
        let _ = e;
        state.set_object_transform(*idx, glam::Mat4::from_scale(glam::Vec3::ZERO));
    }
    render_map.clear();
    despawn_all(scene);

    let mut new_grid = None;
    let mut new_cursor = None;
    for node in saved.roots {
        let is_grid = node.name == "Grid";
        let is_cursor = node.name == "3D Cursor";
        let e = spawn_saved_node(scene, render_map, state, node, None);
        if is_grid {
            new_grid = Some(e);
        }
        if is_cursor {
            new_cursor = Some(e);
        }
    }

    Ok((
        new_grid.ok_or_else(|| anyhow::anyhow!("No Grid found"))?,
        new_cursor.ok_or_else(|| anyhow::anyhow!("No 3D Cursor found"))?,
    ))
}

pub fn spawn_saved_node(
    scene: &mut Scene,
    render_map: &mut Vec<(Entity, usize)>,
    state: &mut State,
    node: SavedNode,
    parent: Option<Entity>,
) -> Entity {
    let e = scene.spawn(node.transform);
    scene.set_name(e, node.name.clone());
    if node.is_locked {
        scene.world.entity_mut(e).insert(Locked);
    }

    let is_grid = node.name == "Grid";
    let is_cursor = node.name == "3D Cursor";
    let is_folder = node.is_folder;
    if is_folder { scene.world.entity_mut(e).insert(Folder); }
    let obj_kind = if is_grid {
        ObjectKind::Grid
    } else if is_cursor {
        ObjectKind::Gizmo
    } else {
        ObjectKind::Solid
    };
    let kind_opt = if is_folder {
        None
    } else if is_grid {
        Some(MeshKind::Plane)
    } else if is_cursor {
        Some(MeshKind::Cursor3D)
    } else {
        node.mesh_kind
    };

    if let Some(kind) = kind_opt {
        scene.world.entity_mut(e).insert(MeshKindComp(kind));
        if let Some(cm) = node.custom_mesh {
            scene.world.entity_mut(e).insert(cm);
        }
        // Keep the grid mesh large enough for the full 500 m grid-extent setting.
        let mesh = if is_grid {
            Mesh::plane(&state.device, 1000.0)
        } else {
            kind.create(&state.device)
        };
        let idx = state.add_object(mesh, obj_kind, Some(kind));
        if is_cursor {
            state.set_object_tint(idx, [1.0, 0.2, 0.2, 1.0]);
        }
        render_map.push((e, idx));
    }
    if let Some(p) = parent {
        scene.set_parent(e, p);
    }
    for child in node.children {
        spawn_saved_node(scene, render_map, state, child, Some(e));
    }
    e
}
