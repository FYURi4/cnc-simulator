use crate::types::*;
use bevy_ecs::prelude::*;
use engine_core::{Children, Parent, Scene, Transform};
use engine_render::mesh::{Mesh, MeshKind};
use engine_render::state::{ObjectKind, State};
use glam::Vec3;

pub fn add_mesh_node(
    scene: &mut Scene,
    render_map: &mut Vec<(Entity, usize)>,
    state: &mut State,
    mesh_kind: MeshKind,
    parent: Option<Entity>,
) -> Entity {
    let t = Transform::default();
    let e = scene.spawn(t);
    scene.set_name(e, format!("{:?}", mesh_kind));
    scene.world.entity_mut(e).insert(MeshKindComp(mesh_kind));

    let verts = match mesh_kind {
        // 100mm = 0.1m (width/depth), height = 100mm = 0.1m
        MeshKind::Cube => {
            let h = 0.005; // 5mm half-extent
            vec![
                glam::Vec3::new(-h, -h, h),
                glam::Vec3::new(h, -h, h),
                glam::Vec3::new(h, h, h),
                glam::Vec3::new(-h, h, h),
                glam::Vec3::new(h, -h, -h),
                glam::Vec3::new(-h, -h, -h),
                glam::Vec3::new(-h, h, -h),
                glam::Vec3::new(h, h, -h),
            ]
        }
        // 1000mm = 1m plane.
        MeshKind::Plane => {
            let h = 0.005; // 5mm half-extent = 10mm plane
            vec![
                glam::Vec3::new(-h, 0.0, h),
                glam::Vec3::new(h, 0.0, h),
                glam::Vec3::new(h, 0.0, -h),
                glam::Vec3::new(-h, 0.0, -h),
            ]
        }
        MeshKind::Pyramid => {
            let h = 0.005; // 10mm base
            vec![
                glam::Vec3::new(-h, 0.0, h),
                glam::Vec3::new(h, 0.0, h),
                glam::Vec3::new(h, 0.0, -h),
                glam::Vec3::new(-h, 0.0, -h),
                glam::Vec3::new(0.0, 0.01, 0.0), // height 10mm
            ]
        }
        MeshKind::Cylinder => {
            let mut verts = vec![
                glam::Vec3::new(0.0, 0.0, 0.0),
                glam::Vec3::new(0.0, 0.01, 0.0),
            ];
            let radius = 0.005; // 5mm radius = 10mm diameter
            let height = 0.01; // 10mm height
            for i in 0..32 {
                let a = (i as f32 / 32.0) * std::f32::consts::TAU;
                verts.push(glam::Vec3::new(radius * a.cos(), 0.0, radius * a.sin()));
            }
            for i in 0..32 {
                let a = (i as f32 / 32.0) * std::f32::consts::TAU;
                verts.push(glam::Vec3::new(radius * a.cos(), height, radius * a.sin()));
            }
            verts
        }
        MeshKind::Sphere => {
            let mut verts = Vec::new();
            let segments = 24;
            let rings = 16;
            let radius = 0.005; // 5mm radius = 10mm diameter
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
        }
        _ => vec![],
    };
    if !verts.is_empty() {
        scene.world.entity_mut(e).insert(CustomMesh {
            vertices: verts,
            deleted_faces: std::collections::HashSet::new(),
            custom_faces: None,
        });
    }

    let mesh = mesh_kind.create(&state.device);
    let idx = state.add_object(mesh, ObjectKind::Solid, Some(mesh_kind));
    render_map.push((e, idx));
    if let Some(p) = parent {
        scene.set_parent(e, p);
    }
    crate::editor::update_object_mesh(state, scene, render_map, e, &[]);
    e
}

pub fn paste_node(
    scene: &mut Scene,
    render_map: &mut Vec<(Entity, usize)>,
    state: &mut State,
    node: &SavedNode,
    parent: Option<Entity>,
    offset: Vec3,
) -> Entity {
    let mut t = node.transform;
    t.translation += offset;
    let e = scene.spawn(t);
    scene.set_name(e, node.name.clone());
    if node.is_locked {
        scene.world.entity_mut(e).insert(Locked);
    }

    let is_grid = node.name == "Grid";
    let is_cursor = node.name == "3D Cursor";
    let is_folder = node.is_folder;
    if is_folder {
        scene.world.entity_mut(e).insert(Folder);
    }
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
        if let Some(cm) = &node.custom_mesh {
            scene.world.entity_mut(e).insert(cm.clone());
        }
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
        crate::editor::update_object_mesh(state, scene, render_map, e, &[]);
    }
    if let Some(p) = parent {
        scene.set_parent(e, p);
    }
    for child in &node.children {
        paste_node(scene, render_map, state, child, Some(e), offset);
    }
    e
}

pub fn despawn_all(scene: &mut Scene) {
    let mut all: Vec<Entity> = Vec::new();
    let mut q = scene.world.query::<Entity>();
    for e in q.iter(&scene.world) {
        all.push(e);
    }
    for e in all {
        let _ = scene.world.despawn(e);
    }
}

pub fn capture_snapshot(scene: &mut Scene) -> SavedScene {
    let mut roots_entities: Vec<Entity> = Vec::new();
    let mut q = scene
        .world
        .query_filtered::<Entity, bevy_ecs::query::Without<Parent>>();
    for e in q.iter(&scene.world) {
        roots_entities.push(e);
    }
    let roots: Vec<SavedNode> = roots_entities
        .iter()
        .map(|e| serialize_subtree(scene, *e))
        .collect();
    SavedScene { roots }
}

pub fn capture_selected(scene: &mut Scene, entity: Entity) -> SavedScene {
    let root = serialize_subtree(scene, entity);
    SavedScene { roots: vec![root] }
}

pub fn serialize_subtree(scene: &Scene, entity: Entity) -> SavedNode {
    let name = scene.get_name(entity).unwrap_or("Unnamed").to_string();
    let transform = scene
        .world
        .get::<Transform>(entity)
        .copied()
        .unwrap_or_default();
    let is_locked = scene.world.get::<Locked>(entity).is_some();
    let mesh_kind = scene.world.get::<MeshKindComp>(entity).map(|m| m.0);
    let custom_mesh = scene.world.get::<CustomMesh>(entity).cloned();
    let children: Vec<Entity> = scene
        .world
        .get::<Children>(entity)
        .map(|c| c.0.clone())
        .unwrap_or_default();
    SavedNode {
        name,
        mesh_kind,
        transform,
        is_locked,
        is_folder: scene.world.get::<Folder>(entity).is_some(),
        custom_mesh,
        children: children
            .iter()
            .map(|c| serialize_subtree(scene, *c))
            .collect(),
    }
}

pub fn build_default_scene(
    state: &mut State,
    scene: &mut Scene,
    render_map: &mut Vec<(Entity, usize)>,
) -> (Entity, Entity) {
    let plane_e = scene.spawn(Transform::default());
    scene.world.entity_mut(plane_e).insert(Locked);
    scene.set_name(plane_e, "Grid");
    scene
        .world
        .entity_mut(plane_e)
        .insert(MeshKindComp(MeshKind::Plane));
    // 1000 m plane covers the maximum 500 m grid radius.
    let plane_idx = state.add_object(
        Mesh::plane(&state.device, 1000.0),
        ObjectKind::Grid,
        Some(MeshKind::Plane),
    );
    render_map.push((plane_e, plane_idx));

    let cursor_e = scene.spawn(Transform::default());
    scene.set_name(cursor_e, "3D Cursor");
    scene
        .world
        .entity_mut(cursor_e)
        .insert(MeshKindComp(MeshKind::Cursor3D));
    let cursor_idx = state.add_object(
        Mesh::cursor_3d(&state.device),
        ObjectKind::Gizmo,
        Some(MeshKind::Cursor3D),
    );
    state.set_object_tint(cursor_idx, [1.0, 0.2, 0.2, 1.0]);
    render_map.push((cursor_e, cursor_idx));

    let cube_e = scene.spawn(Transform::default());
    scene.set_name(cube_e, "Cube");
    scene
        .world
        .entity_mut(cube_e)
        .insert(MeshKindComp(MeshKind::Cube));

    let verts = {
        let h = 0.005; // 5mm half-extent = 10mm cube
        vec![
            glam::Vec3::new(-h, -h, h),
            glam::Vec3::new(h, -h, h),
            glam::Vec3::new(h, h, h),
            glam::Vec3::new(-h, h, h),
            glam::Vec3::new(h, -h, -h),
            glam::Vec3::new(-h, -h, -h),
            glam::Vec3::new(-h, h, -h),
            glam::Vec3::new(h, h, -h),
        ]
    };
    scene.world.entity_mut(cube_e).insert(CustomMesh {
        vertices: verts,
        deleted_faces: std::collections::HashSet::new(),
        custom_faces: None,
    });

    scene.set_parent(cube_e, plane_e);
    let cube_idx = state.add_object(
        Mesh::cube(&state.device, 0.01),
        ObjectKind::Solid,
        Some(MeshKind::Cube),
    );
    render_map.push((cube_e, cube_idx));

    crate::editor::update_object_mesh(state, scene, render_map, cube_e, &[]);

    (plane_e, cursor_e)
}
