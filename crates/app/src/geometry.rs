use std::collections::HashSet;
use glam::{Mat4, Quat, Vec2, Vec3};
use bevy_ecs::prelude::*;
use engine_core::Scene;
use engine_render::camera::Camera;
use engine_render::mesh::{MeshKind, Vertex};
use engine_render::state::{ObjectKind, RenderObject};
use crate::types::*;

pub fn apply_snap(pos: Vec3, start_pos: Vec3, axis: Vec3, snap: &SnapSettings, scene: &Scene, render_map: &[(Entity, usize)], objects: &[RenderObject], ignore_entity: Entity) -> Vec3 {
    if !snap.enabled { return pos; }
    match snap.target {
        SnapTarget::Increment | SnapTarget::Grid => {
            let dist = (pos - start_pos).dot(axis);
            start_pos + axis * ((dist / snap.increment).round() * snap.increment)
        }
        SnapTarget::Origin => {
            if pos.length() < 0.5 { Vec3::ZERO } else { pos }
        }
        SnapTarget::Vertex | SnapTarget::Edge => {
            let mut best_dist = 0.2f32;
            let mut best_point = pos;
            
            for (e, idx) in render_map {
                if *e == ignore_entity { continue; }
                if objects[*idx].kind != ObjectKind::Solid { continue; }
                
                if let Some(gt) = scene.global_matrix(*e) {
                    let (amin, amax) = match objects[*idx].mesh_kind {
                        Some(MeshKind::Plane) => (Vec3::new(-0.5, -0.1, -0.5), Vec3::new(0.5, 0.1, 0.5)),
                        _ => (Vec3::splat(-0.05), Vec3::splat(0.05))
                    };
                    
                    let mut corners = Vec::new();
                    for x in [amin.x, amax.x] { for y in [amin.y, amax.y] { for z in [amin.z, amax.z] {
                        corners.push(gt.transform_point3(Vec3::new(x, y, z)));
                    }}}
                    
                    if snap.target == SnapTarget::Vertex {
                        for c in &corners {
                            let d = (*c - pos).length();
                            if d < best_dist { best_dist = d; best_point = *c; }
                        }
                    } else {
                        let edges = get_edges(MeshKind::Cube);
                        for (i, j) in edges {
                            let mid = (corners[i] + corners[j]) * 0.5;
                            let d = (mid - pos).length();
                            if d < best_dist { best_dist = d; best_point = mid; }
                        }
                    }
                }
            }
            best_point
        }
    }
}

pub fn axis_index(v: Vec3) -> usize { if v.x.abs() > 0.5 { 0 } else if v.y.abs() > 0.5 { 1 } else { 2 } }
pub fn axis_comp(v: Vec3, idx: usize) -> f32 { match idx { 0 => v.x, 1 => v.y, _ => v.z } }
pub fn set_axis_comp(v: &mut Vec3, idx: usize, val: f32) { match idx { 0 => v.x = val, 1 => v.y = val, _ => v.z = val }; }

pub fn ray_triangle(ro: Vec3, rd: Vec3, a: Vec3, b: Vec3, c: Vec3) -> Option<f32> {
    let edge1 = b - a;
    let edge2 = c - a;
    let h = rd.cross(edge2);
    let det = edge1.dot(h);
    if det.abs() < 1e-6 { return None; }
    let inv_det = 1.0 / det;
    let s = ro - a;
    let u = s.dot(h) * inv_det;
    if u < 0.0 || u > 1.0 { return None; }
    let q = s.cross(edge1);
    let v = rd.dot(q) * inv_det;
    if v < 0.0 || u + v > 1.0 { return None; }
    let t = edge2.dot(q) * inv_det;
    if t > 1e-6 { Some(t) } else { None }
}

pub fn dist_to_segment_screen(camera: &Camera, screen_size: Vec2, mouse: Vec2, a: Vec3, b: Vec3) -> f32 {
    let p0 = world_to_screen(camera, screen_size, a);
    let p1 = world_to_screen(camera, screen_size, b);
    let ab = p1 - p0;
    let len_sq = ab.length_squared();
    if len_sq < 1e-6 { return (mouse - p0).length(); }
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
    (camera.position, (far_h.truncate() / far_h.w - near_h.truncate() / near_h.w).normalize())
}

pub fn world_to_screen(camera: &Camera, screen_size: Vec2, p: Vec3) -> Vec2 {
    let clip = camera.view_proj() * glam::vec4(p.x, p.y, p.z, 1.0);
    if clip.w.abs() < 1e-6 { return Vec2::ZERO; }
    let ndc = clip.truncate() / clip.w;
    Vec2::new((ndc.x + 1.0) * 0.5 * screen_size.x, (1.0 - ndc.y) * 0.5 * screen_size.y)
}

pub fn ray_aabb(orig: Vec3, dir: Vec3, amin: Vec3, amax: Vec3) -> Option<f32> {
    let inv = Vec3::new(if dir.x.abs() < 1e-9 { 1e9 } else { 1.0 / dir.x }, if dir.y.abs() < 1e-9 { 1e9 } else { 1.0 / dir.y }, if dir.z.abs() < 1e-9 { 1e9 } else { 1.0 / dir.z });
    let t0 = (amin - orig) * inv;
    let t1 = (amax - orig) * inv;
    let tmin = t0.min(t1);
    let tmax = t0.max(t1);
    let enter = tmin.max_element();
    let exit = tmax.min_element();
    if exit < 0.0 || enter > exit { None } else { Some(enter.max(0.0)) }
}

pub fn pick_object(scene: &Scene, render_map: &[(Entity, usize)], objects: &[RenderObject], ray_o: Vec3, ray_d: Vec3) -> Option<Entity> {
    let mut best: Option<(Entity, f32)> = None;
    for (entity, idx) in render_map {
        if objects[*idx].kind != ObjectKind::Solid { continue; }
        if scene.world.get::<Locked>(*entity).is_some() { continue; }

        let Some(gt) = scene.global_matrix(*entity) else { continue };
        
        if let Some(custom) = scene.world.get::<CustomMesh>(*entity) {
            let kind = scene.world.get::<MeshKindComp>(*entity).map(|m| m.0).unwrap_or(MeshKind::Cube);
            if kind == MeshKind::Plane {
                let p0 = gt.transform_point3(custom.vertices[0]);
                let p1 = gt.transform_point3(custom.vertices[1]);
                let p2 = gt.transform_point3(custom.vertices[2]);
                let p3 = gt.transform_point3(custom.vertices[3]);
                if let Some(t) = ray_triangle(ray_o, ray_d, p0, p1, p2).or_else(|| ray_triangle(ray_o, ray_d, p0, p2, p3)) {
                    if best.map_or(true, |(_, d)| t < d) { best = Some((*entity, t)); }
                }
            } else {
                let inv = gt.inverse();
                let lo = inv.transform_point3(ray_o);
                let ld = inv.transform_vector3(ray_d);
                if let Some(t) = ray_aabb(lo, ld, Vec3::splat(-0.05), Vec3::splat(0.05)) {
                    let world_hit = gt.transform_point3(lo + ld * t);
                    let dist = (world_hit - ray_o).length();
                    if best.map_or(true, |(_, d)| dist < d) { best = Some((*entity, dist)); }
                }
            }
        } else {
            let inv = gt.inverse();
            let lo = inv.transform_point3(ray_o);
            let ld = inv.transform_vector3(ray_d);
            if let Some(t) = ray_aabb(lo, ld, Vec3::splat(-0.05), Vec3::splat(0.05)) {
                let world_hit = gt.transform_point3(lo + ld * t);
                let dist = (world_hit - ray_o).length();
                if best.map_or(true, |(_, d)| dist < d) { best = Some((*entity, dist)); }
            }
        }
    }
    best.map(|(e, _)| e)
}

pub fn pick_gizmo(ray_o: Vec3, ray_d: Vec3, world_pos: Vec3) -> Option<usize> {
    let rots = [Quat::from_rotation_z(-PI / 2.0), Quat::IDENTITY, Quat::from_rotation_x(PI / 2.0)];
    let amin = Vec3::new(-0.06, 0.0, -0.06);
    let amax = Vec3::new(0.06, 0.45, 0.06);
    let mut best: Option<(usize, f32)> = None;
    for (i, rot) in rots.iter().enumerate() {
        let mat = Mat4::from_rotation_translation(*rot, world_pos);
        let inv = mat.inverse();
        if let Some(t) = ray_aabb(inv.transform_point3(ray_o), inv.transform_vector3(ray_d), amin, amax) {
            if best.map_or(true, |(_, bt)| t < bt) { best = Some((i, t)); }
        }
    }
    best.map(|(i, _)| i)
}

pub fn pick_gizmo_ring(ray_o: Vec3, ray_d: Vec3, world_pos: Vec3) -> Option<usize> {
    let mut best: Option<(usize, f32)> = None;
    for (i, normal) in [Vec3::X, Vec3::Y, Vec3::Z].iter().enumerate() {
        let denom = ray_d.dot(*normal);
        if denom.abs() < 1e-6 { continue; }
        let t = (world_pos - ray_o).dot(*normal) / denom;
        if t < 0.0 { continue; }
        let offset = (ray_o + ray_d * t) - world_pos;
        let in_plane = offset - *normal * offset.dot(*normal);
        if (in_plane.length() - 0.4).abs() < 0.05 {
            if best.map_or(true, |(_, bt)| t < bt) { best = Some((i, t)); }
        }
    }
    best.map(|(i, _)| i)
}

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
