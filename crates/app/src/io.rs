use anyhow::Result;
use bevy_ecs::prelude::*;
use engine_core::{Children, Parent, Scene, Transform};
use engine_render::mesh::{Mesh, MeshKind};
use engine_render::state::{ObjectKind, State};
use std::path::PathBuf;
use std::fs;
use crate::types::*;
use crate::geometry::*;
use crate::editor::collect_subtree;

pub fn add_mesh_node(scene: &mut Scene, render_map: &mut Vec<(Entity, usize)>, state: &mut State, mesh_kind: MeshKind, parent: Option<Entity>) -> Entity {
    let t = Transform::default();
    let e = scene.spawn(t);
    scene.set_name(e, format!("{:?}", mesh_kind));
    scene.world.entity_mut(e).insert(MeshKindComp(mesh_kind));
    
    let verts = match mesh_kind {
        MeshKind::Cube => vec![
            glam::Vec3::new(-0.5, -0.5, 0.5), glam::Vec3::new(0.5, -0.5, 0.5), glam::Vec3::new(0.5, 0.5, 0.5), glam::Vec3::new(-0.5, 0.5, 0.5),
            glam::Vec3::new(0.5, -0.5, -0.5), glam::Vec3::new(-0.5, -0.5, -0.5), glam::Vec3::new(-0.5, 0.5, -0.5), glam::Vec3::new(0.5, 0.5, -0.5),
        ],
        MeshKind::Plane => vec![
            glam::Vec3::new(-10.0, 0.0, 10.0), glam::Vec3::new(10.0, 0.0, 10.0), glam::Vec3::new(10.0, 0.0, -10.0), glam::Vec3::new(-10.0, 0.0, -10.0),
        ],
        MeshKind::Pyramid => vec![
            glam::Vec3::new(-0.5, 0.0, 0.5), glam::Vec3::new(0.5, 0.0, 0.5), glam::Vec3::new(0.5, 0.0, -0.5), glam::Vec3::new(-0.5, 0.0, -0.5),
            glam::Vec3::new(0.0, 1.0, 0.0),
        ],
        MeshKind::Cylinder => {
            let mut verts = vec![glam::Vec3::new(0.0, 0.0, 0.0), glam::Vec3::new(0.0, 1.0, 0.0)];
            for i in 0..32 {
                let a = (i as f32 / 32.0) * std::f32::consts::TAU;
                verts.push(glam::Vec3::new(0.5 * a.cos(), 0.0, 0.5 * a.sin()));
            }
            for i in 0..32 {
                let a = (i as f32 / 32.0) * std::f32::consts::TAU;
                verts.push(glam::Vec3::new(0.5 * a.cos(), 1.0, 0.5 * a.sin()));
            }
            verts
        },
        MeshKind::Sphere => {
            let mut verts = Vec::new();
            let segments = 24;
            let rings = 16;
            let radius = 0.5;
            for i in 0..=rings {
                let v = i as f32 / rings as f32;
                let phi = v * std::f32::consts::PI;
                for j in 0..=segments {
                    let u = j as f32 / segments as f32;
                    let theta = u * std::f32::consts::TAU;
                    let x = radius * phi.sin() * theta.cos();
                    let y = radius * phi.cos();
                    let z = radius * phi.sin() * theta.sin();
                    verts.push(glam::Vec3::new(x, y, z));
                }
            }
            verts
        },
        _ => vec![],
    };
    if !verts.is_empty() {
        scene.world.entity_mut(e).insert(CustomMesh { vertices: verts, deleted_faces: std::collections::HashSet::new(), custom_faces: None });
    }

    let mesh = mesh_kind.create(&state.device);
    let idx = state.add_object(mesh, ObjectKind::Solid, Some(mesh_kind));
    render_map.push((e, idx));
    if let Some(p) = parent { scene.set_parent(e, p); }
    e
}

pub fn despawn_all(scene: &mut Scene) {
    let mut all: Vec<Entity> = Vec::new();
    let mut q = scene.world.query::<Entity>();
    for e in q.iter(&scene.world) { all.push(e); }
    for e in all { let _ = scene.world.despawn(e); }
}

pub fn capture_snapshot(scene: &mut Scene) -> SavedScene {
    let mut roots_entities: Vec<Entity> = Vec::new();
    let mut q = scene.world.query_filtered::<Entity, bevy_ecs::query::Without<Parent>>();
    for e in q.iter(&scene.world) { roots_entities.push(e); }
    let roots: Vec<SavedNode> = roots_entities.iter().map(|e| serialize_subtree(scene, *e)).collect();
    SavedScene { roots }
}

pub fn serialize_subtree(scene: &Scene, entity: Entity) -> SavedNode {
    let name = scene.get_name(entity).unwrap_or("Unnamed").to_string();
    let transform = scene.world.get::<Transform>(entity).copied().unwrap_or_default();
    let is_locked = scene.world.get::<Locked>(entity).is_some();
    let mesh_kind = scene.world.get::<MeshKindComp>(entity).map(|m| m.0);
    let custom_mesh = scene.world.get::<CustomMesh>(entity).cloned();
    let children: Vec<Entity> = scene.world.get::<Children>(entity).map(|c| c.0.clone()).unwrap_or_default();
    SavedNode { name, mesh_kind, transform, is_locked, custom_mesh, children: children.iter().map(|c| serialize_subtree(scene, *c)).collect() }
}

pub fn save_scene(scene: &mut Scene, path: &str) -> Result<()> {
    let snap = capture_snapshot(scene);
    let json = serde_json::to_string_pretty(&snap)?;
    std::fs::write(path, json)?;
    Ok(())
}

pub fn load_scene(scene: &mut Scene, render_map: &mut Vec<(Entity, usize)>, state: &mut State, path: &str) -> Result<(Entity, Entity)> {
    let data = fs::read_to_string(path)?;
    let saved: SavedScene = serde_json::from_str(&data)?;

    for (e, idx) in render_map.iter() { let _ = e; state.set_object_transform(*idx, glam::Mat4::from_scale(glam::Vec3::ZERO)); }
    render_map.clear();
    despawn_all(scene);

    let mut new_grid = None;
    let mut new_cursor = None;
    for node in saved.roots {
        let is_grid = node.name == "Grid";
        let is_cursor = node.name == "3D Cursor";
        let e = spawn_saved_node(scene, render_map, state, node, None);
        if is_grid { new_grid = Some(e); }
        if is_cursor { new_cursor = Some(e); }
    }

    Ok((new_grid.ok_or_else(|| anyhow::anyhow!("No Grid found"))?, new_cursor.ok_or_else(|| anyhow::anyhow!("No 3D Cursor found"))?))
}

pub fn spawn_saved_node(scene: &mut Scene, render_map: &mut Vec<(Entity, usize)>, state: &mut State, node: SavedNode, parent: Option<Entity>) -> Entity {
    let e = scene.spawn(node.transform);
    scene.set_name(e, node.name.clone());
    if node.is_locked { scene.world.entity_mut(e).insert(Locked); }
    
    let is_grid = node.name == "Grid";
    let is_cursor = node.name == "3D Cursor";
    let obj_kind = if is_grid { ObjectKind::Grid } else if is_cursor { ObjectKind::Gizmo } else { ObjectKind::Solid };
    let kind_opt = if is_grid { Some(MeshKind::Plane) } else if is_cursor { Some(MeshKind::Cursor3D) } else { node.mesh_kind };

    if let Some(kind) = kind_opt {
        scene.world.entity_mut(e).insert(MeshKindComp(kind));
        if let Some(cm) = node.custom_mesh {
            scene.world.entity_mut(e).insert(cm);
        }
        let mesh = kind.create(&state.device);
        let idx = state.add_object(mesh, obj_kind, Some(kind));
        if is_cursor { state.set_object_tint(idx, [1.0, 0.2, 0.2, 1.0]); }
        render_map.push((e, idx));
    }
    if let Some(p) = parent { scene.set_parent(e, p); }
    for child in node.children { spawn_saved_node(scene, render_map, state, child, Some(e)); }
    e
}

pub fn build_default_scene(state: &mut State, scene: &mut Scene, render_map: &mut Vec<(Entity, usize)>) -> (Entity, Entity) {
    let plane_e = scene.spawn(Transform::default());
    scene.world.entity_mut(plane_e).insert(Locked);
    scene.set_name(plane_e, "Grid");
    scene.world.entity_mut(plane_e).insert(MeshKindComp(MeshKind::Plane));
    let plane_idx = state.add_object(Mesh::plane(&state.device, 500.0), ObjectKind::Grid, Some(MeshKind::Plane));
    render_map.push((plane_e, plane_idx));

    let cursor_e = scene.spawn(Transform::default());
    scene.set_name(cursor_e, "3D Cursor");
    scene.world.entity_mut(cursor_e).insert(MeshKindComp(MeshKind::Cursor3D));
    let cursor_idx = state.add_object(Mesh::cursor_3d(&state.device), ObjectKind::Gizmo, Some(MeshKind::Cursor3D));
    state.set_object_tint(cursor_idx, [1.0, 0.2, 0.2, 1.0]);
    render_map.push((cursor_e, cursor_idx));

    let cube_e = scene.spawn(Transform::default());
    scene.set_name(cube_e, "Cube");
    scene.world.entity_mut(cube_e).insert(MeshKindComp(MeshKind::Cube));
    
    let verts = vec![
        glam::Vec3::new(-0.5, -0.5, 0.5), glam::Vec3::new(0.5, -0.5, 0.5), glam::Vec3::new(0.5, 0.5, 0.5), glam::Vec3::new(-0.5, 0.5, 0.5),
        glam::Vec3::new(0.5, -0.5, -0.5), glam::Vec3::new(-0.5, -0.5, -0.5), glam::Vec3::new(-0.5, 0.5, -0.5), glam::Vec3::new(0.5, 0.5, -0.5),
    ];
    scene.world.entity_mut(cube_e).insert(CustomMesh { vertices: verts, deleted_faces: std::collections::HashSet::new(), custom_faces: None });

    scene.set_parent(cube_e, plane_e);
    let cube_idx = state.add_object(Mesh::cube(&state.device, 1.0), ObjectKind::Solid, Some(MeshKind::Cube));
    render_map.push((cube_e, cube_idx));
    
    crate::editor::update_object_mesh(state, scene, render_map, cube_e, &[]);

    (plane_e, cursor_e)
}

use engine_core::gcode::GCodeProgram;

pub fn load_gcode_program(path: &str) -> Result<GCodeProgram> {
    let source = fs::read_to_string(path)?;
    Ok(GCodeProgram::parse(&source))
}