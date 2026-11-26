use std::collections::HashSet;
use glam::{Mat4, Quat, Vec2, Vec3};
use bevy_ecs::prelude::*;
use engine_core::Scene;
use engine_render::mesh::{Mesh, MeshKind, Vertex};
use engine_render::state::{ObjectKind, State};
use crate::types::*;
pub fn get_current_faces(scene: &Scene, entity: Entity, kind: MeshKind) -> Vec<Vec<usize>> {
    if let Some(custom) = scene.world.get::<CustomMesh>(entity) {
        if let Some(cf) = &custom.custom_faces {
            let mut filtered = Vec::new();
            for (i, face) in cf.iter().enumerate() {
                if !custom.deleted_faces.contains(&i) {
                    filtered.push(face.clone());
                }
            }
            return filtered;
        }
        let default_faces = get_faces(kind);
        let mut filtered = Vec::new();
        for (i, face) in default_faces.into_iter().enumerate() {
            if !custom.deleted_faces.contains(&i) {
                filtered.push(face);
            }
        }
        return filtered;
    }
    get_faces(kind)
}

pub fn get_current_edges(faces: &[Vec<usize>]) -> Vec<(usize, usize)> {
    let mut edges = HashSet::new();
    for face in faces {
        for i in 0..face.len() {
            let a = face[i];
            let b = face[(i + 1) % face.len()];
            if a < b { edges.insert((a, b)); } else { edges.insert((b, a)); }
        }
    }
    let mut edge_vec: Vec<(usize, usize)> = edges.into_iter().collect();
    edge_vec.sort_unstable();
    edge_vec
}

pub fn get_edges(kind: MeshKind) -> Vec<(usize, usize)> {
    if kind == MeshKind::Cube {
        vec![
            (0, 1), (1, 2), (2, 3), (3, 0),
            (4, 5), (5, 6), (6, 7), (7, 4),
            (0, 5), (1, 4), (2, 7), (3, 6),
        ]
    } else if kind == MeshKind::Plane {
        vec![(0, 1), (1, 2), (2, 3), (3, 0)]
    } else if kind == MeshKind::Pyramid {
        vec![(0,1),(1,2),(2,3),(3,0),(0,4),(1,4),(2,4),(3,4)]
    } else { vec![] }
}

pub fn get_faces(kind: MeshKind) -> Vec<Vec<usize>> {
    if kind == MeshKind::Cube {
        vec![
            vec![0, 1, 2, 3],
            vec![4, 5, 6, 7],
            vec![1, 4, 7, 2],
            vec![5, 0, 3, 6],
            vec![3, 2, 7, 6],
            vec![5, 4, 1, 0],
        ]
    } else if kind == MeshKind::Plane {
        vec![vec![0, 1, 2, 3]]
    } else if kind == MeshKind::Pyramid {
        vec![
            vec![0, 3, 2, 1],
            vec![0, 1, 4],
            vec![1, 2, 4],
            vec![2, 3, 4],
            vec![3, 0, 4],
        ]
    } else if kind == MeshKind::Cylinder {
        // Cylinder vertex layout in mesh.rs:
        // 0..127: side vertices (32 segs × 4 verts: bl, br, tr, tl)
        // 128: center bottom, 129..160: bottom ring (32)
        // 161: center top, 162..193: top ring (32)
        let mut faces = Vec::new();
        // Side faces - winding must match outward normals from build_mesh_vertices
        for i in 0..32 {
            let b0 = i * 4;
            let b1 = b0 + 1;
            let t1 = b0 + 2;
            let t0 = b0 + 3;
            faces.push(vec![b0, b1, t1, t0]);
        }
        // Bottom cap
        let mut bottom = vec![128usize];
        for i in (129..161).rev() { bottom.push(i); }
        faces.push(bottom);
        // Top cap
        let mut top = vec![161usize];
        for i in 162..194 { top.push(i); }
        faces.push(top);
        faces
    } else if kind == MeshKind::Sphere {
        // Sphere: segments=24, rings=16
        // vertices[i][j] = ring i, segment j -> flat index = i*(segments+1) + j
        let segments = 24usize;
        let rings = 16usize;
        let mut faces = Vec::new();
        for i in 0..rings {
            for j in 0..segments {
                let a = i * (segments + 1) + j;
                let b = a + 1;
                let c = (i + 1) * (segments + 1) + j;
                let d = c + 1;
                faces.push(vec![a, b, d, c]);
            }
        }
        faces
    } else {
        vec![]
    }
}

pub fn get_handles_for_mode(mode: SelectMode, faces: &[Vec<usize>], edges: &[(usize, usize)], verts: &[Vec3]) -> Vec<Vec3> {
    let mut res = Vec::new();
    match mode {
        SelectMode::Vertex => {
            for &v in verts { res.push(v); }
        }
        SelectMode::Edge => {
            for &(a, b) in edges {
                res.push((verts[a] + verts[b]) * 0.5);
            }
        }
        SelectMode::Face => {
            for face in faces {
                let mut center = Vec3::ZERO;
                for &i in face { center += verts[i]; }
                res.push(center / face.len() as f32);
            }
        }
    }
    res
}

pub fn build_mesh_vertices(scene: &Scene, entity: Entity, kind: MeshKind, selected: &[Selection]) -> Vec<Vertex> {
    let mut out = Vec::new();
    let mut sel_set = HashSet::new();
    for s in selected {
        match s {
            Selection::Vertex(i) => { sel_set.insert(*i); }
            Selection::Edge(a, b) => { sel_set.insert(*a); sel_set.insert(*b); }
            Selection::Face(indices) => { for &i in indices { sel_set.insert(i); } }
        }
    }

    let get_color = |idx: usize| -> [f32; 3] {
        if sel_set.contains(&idx) { [1.0, 0.5, 0.0] } else { [1.0, 1.0, 1.0] }
    };

    let verts = if let Some(custom) = scene.world.get::<CustomMesh>(entity) {
        custom.vertices.clone()
    } else {
        vec![]
    };

    if verts.is_empty() { return out; }

    let faces = get_current_faces(scene, entity, kind);

    // Compute proper normals for cylinder side faces
    let cylinder_side_normals: Vec<Vec3> = if kind == MeshKind::Cylinder && !verts.is_empty() {
        let mut normals = Vec::new();
        // Side vertices: 0..127 (32 segs × 4)
        for i in 0..128 {
            let v = verts[i];
            // Horizontal normal from cylinder axis outward
            normals.push(Vec3::new(-v.x, 0.0, -v.z).normalize());
        }
        // Bottom cap normal
        normals.push(Vec3::NEG_Y);
        for _ in 129..161 { normals.push(Vec3::NEG_Y); }
        // Top cap normal
        normals.push(Vec3::Y);
        for _ in 162..194 { normals.push(Vec3::Y); }
        normals
    } else {
        vec![]
    };

    for face in faces {
        if face.len() < 3 { continue; }
        
        let v0 = verts[face[0]];
        let v1 = verts[face[1]];
        let v2 = verts[face[2]];
        let normal = if !cylinder_side_normals.is_empty() {
            cylinder_side_normals[face[0]]
        } else {
            (v1 - v0).cross(v2 - v0).normalize()
        };

        out.push(Vertex { position: v0.into(), normal: normal.into(), uv: [0.0, 1.0], color: get_color(face[0]) });
        out.push(Vertex { position: v1.into(), normal: normal.into(), uv: [1.0, 1.0], color: get_color(face[1]) });
        out.push(Vertex { position: v2.into(), normal: normal.into(), uv: [1.0, 0.0], color: get_color(face[2]) });

        if face.len() > 3 {
            let v3 = verts[face[3]];
            let n3 = if !cylinder_side_normals.is_empty() { cylinder_side_normals[face[3]] } else { normal };
            out.push(Vertex { position: v0.into(), normal: normal.into(), uv: [0.0, 1.0], color: get_color(face[0]) });
            out.push(Vertex { position: v2.into(), normal: normal.into(), uv: [1.0, 0.0], color: get_color(face[2]) });
            out.push(Vertex { position: v3.into(), normal: n3.into(), uv: [0.0, 0.0], color: get_color(face[3]) });
        }
    }
    out
}