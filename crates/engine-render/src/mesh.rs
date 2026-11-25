use bytemuck::{Pod, Zeroable};
use glam::Vec3; // <-- Убрали Quat, так как он больше не нужен
use serde::{Deserialize, Serialize};

#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
pub struct Vertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub uv: [f32; 2],
    pub color: [f32; 3], // <-- Поле для подсветки
}

impl Vertex {
    pub fn layout() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Vertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute { offset: 0, shader_location: 0, format: wgpu::VertexFormat::Float32x3 },
                wgpu::VertexAttribute { offset: 12, shader_location: 1, format: wgpu::VertexFormat::Float32x3 },
                wgpu::VertexAttribute { offset: 24, shader_location: 2, format: wgpu::VertexFormat::Float32x2 },
                wgpu::VertexAttribute { offset: 32, shader_location: 3, format: wgpu::VertexFormat::Float32x3 },
            ],
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum MeshKind {
    Cube,
    Plane,
    Cylinder,
    Sphere,
    Pyramid,
    Ring,
    Cursor3D,
}

impl MeshKind {
    pub fn create(self, device: &wgpu::Device) -> Mesh {
        match self {
            MeshKind::Cube => Mesh::cube(device, 0.01),
            MeshKind::Plane => Mesh::plane(device, 20.0),
            MeshKind::Cylinder => Mesh::cylinder(device),
            MeshKind::Sphere => Mesh::sphere_solid(device),
            MeshKind::Pyramid => Mesh::pyramid(device),
            MeshKind::Ring => Mesh::ring(device),
            MeshKind::Cursor3D => Mesh::cursor_3d(device),
        }
    }
}

pub struct Mesh {
    pub vertex_buffer: wgpu::Buffer,
    pub index_buffer: wgpu::Buffer,
    pub index_count: u32,
}

impl Mesh {
    pub fn plane(device: &wgpu::Device, size: f32) -> Self {
        let h = size * 0.5;
        let vertices = [
            Vertex { position: [-h, 0.0,  h], normal: [0.0, 1.0, 0.0], uv: [0.0, 1.0], color: [1.0; 3] },
            Vertex { position: [ h, 0.0,  h], normal: [0.0, 1.0, 0.0], uv: [1.0, 1.0], color: [1.0; 3] },
            Vertex { position: [ h, 0.0, -h], normal: [0.0, 1.0, 0.0], uv: [1.0, 0.0], color: [1.0; 3] },
            Vertex { position: [-h, 0.0, -h], normal: [0.0, 1.0, 0.0], uv: [0.0, 0.0], color: [1.0; 3] },
        ];
        let indices: &[u16] = &[0, 1, 2, 0, 2, 3];

        use wgpu::util::DeviceExt;
        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("plane_vertices"),
            contents: bytemuck::cast_slice(&vertices),
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        });
        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("plane_indices"),
            contents: bytemuck::cast_slice(indices),
            usage: wgpu::BufferUsages::INDEX,
        });
        Self { vertex_buffer, index_buffer, index_count: indices.len() as u32 }
    }

    pub fn cube(device: &wgpu::Device, size: f32) -> Self {
        let h = size * 0.5;
        let face = |normal: [f32; 3], v: [[f32; 3]; 4]| -> [Vertex; 4] {
            [
                Vertex { position: v[0], normal, uv: [0.0, 1.0], color: [1.0; 3] },
                Vertex { position: v[1], normal, uv: [1.0, 1.0], color: [1.0; 3] },
                Vertex { position: v[2], normal, uv: [1.0, 0.0], color: [1.0; 3] },
                Vertex { position: v[3], normal, uv: [0.0, 0.0], color: [1.0; 3] },
            ]
        };
        let faces = [
            face([0.0, 0.0, 1.0], [[-h, -h, h], [h, -h, h], [h, h, h], [-h, h, h]]),
            face([0.0, 0.0, -1.0], [[h, -h, -h], [-h, -h, -h], [-h, h, -h], [h, h, -h]]),
            face([1.0, 0.0, 0.0], [[h, -h, h], [h, -h, -h], [h, h, -h], [h, h, h]]),
            face([-1.0, 0.0, 0.0], [[-h, -h, -h], [-h, -h, h], [-h, h, h], [-h, h, -h]]),
            face([0.0, 1.0, 0.0], [[-h, h, h], [h, h, h], [h, h, -h], [-h, h, -h]]),
            face([0.0, -1.0, 0.0], [[-h, -h, -h], [h, -h, -h], [h, -h, h], [-h, -h, h]]),
        ];
        let mut vertices: Vec<Vertex> = Vec::with_capacity(24);
        for f in faces.iter() { vertices.extend_from_slice(f); }
        let mut indices: Vec<u16> = Vec::with_capacity(36);
        for i in 0..6u16 {
            let b = i * 4;
            indices.extend_from_slice(&[b, b + 1, b + 2, b, b + 2, b + 3]);
        }

        use wgpu::util::DeviceExt;
        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("cube_vertices"),
            contents: bytemuck::cast_slice(&vertices),
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        });
        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("cube_indices"),
            contents: bytemuck::cast_slice(&indices),
            usage: wgpu::BufferUsages::INDEX,
        });
        Self { vertex_buffer, index_buffer, index_count: indices.len() as u32 }
    }

    pub fn cylinder(device: &wgpu::Device) -> Self {
        let mut vertices = Vec::new();
        let mut indices = Vec::new();
        let segments = 32;
        let radius = 0.5;
        let height = 1.0;

        for i in 0..segments {
            let a0 = (i as f32 / segments as f32) * std::f32::consts::TAU;
            let a1 = ((i + 1) as f32 / segments as f32) * std::f32::consts::TAU;
            let x0 = radius * a0.cos(); let z0 = radius * a0.sin();
            let x1 = radius * a1.cos(); let z1 = radius * a1.sin();
            let n0 = Vec3::new(x0, 0.0, z0).normalize();
            let n1 = Vec3::new(x1, 0.0, z1).normalize();
            let base = vertices.len() as u16;
            vertices.push(Vertex { position: [x0, 0.0, z0], normal: n0.into(), uv: [0.0, 1.0], color: [1.0; 3] });
            vertices.push(Vertex { position: [x1, 0.0, z1], normal: n1.into(), uv: [1.0, 1.0], color: [1.0; 3] });
            vertices.push(Vertex { position: [x1, height, z1], normal: n1.into(), uv: [1.0, 0.0], color: [1.0; 3] });
            vertices.push(Vertex { position: [x0, height, z0], normal: n0.into(), uv: [0.0, 0.0], color: [1.0; 3] });
            indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
        }

        let mut add_cap = |y: f32, normal: Vec3, flip: bool| {
            let center = vertices.len() as u16;
            vertices.push(Vertex { position: [0.0, y, 0.0], normal: normal.into(), uv: [0.5, 0.5], color: [1.0; 3] });
            let start = vertices.len() as u16;
            for i in 0..segments {
                let a = (i as f32 / segments as f32) * std::f32::consts::TAU;
                let x = radius * a.cos(); let z = radius * a.sin();
                vertices.push(Vertex { position: [x, y, z], normal: normal.into(), uv: [x * 0.5 + 0.5, z * 0.5 + 0.5], color: [1.0; 3] });
            }
            for i in 0..segments {
                let cur = start + i;
                let next = start + ((i + 1) % segments);
                if flip { indices.extend_from_slice(&[center, next, cur]); }
                else { indices.extend_from_slice(&[center, cur, next]); }
            }
        };
        add_cap(0.0, Vec3::NEG_Y, false);
        add_cap(height, Vec3::Y, true);

        use wgpu::util::DeviceExt;
        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("cylinder_vertices"),
            contents: bytemuck::cast_slice(&vertices),
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        });
        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("cylinder_indices"),
            contents: bytemuck::cast_slice(&indices),
            usage: wgpu::BufferUsages::INDEX,
        });
        Self { vertex_buffer, index_buffer, index_count: indices.len() as u32 }
    }

    pub fn sphere_solid(device: &wgpu::Device) -> Self {
        let mut vertices = Vec::new();
        let mut indices = Vec::new();
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
                let normal = Vec3::new(x, y, z).normalize();
                vertices.push(Vertex { position: [x, y, z], normal: normal.into(), uv: [u, v], color: [1.0; 3] });
            }
        }
        for i in 0..rings {
            for j in 0..segments {
                let a = i * (segments + 1) + j;
                let b = a + segments + 1;
                indices.extend_from_slice(&[a as u16, (a + 1) as u16, b as u16]);
                indices.extend_from_slice(&[b as u16, (a + 1) as u16, (b + 1) as u16]);
            }
        }

        use wgpu::util::DeviceExt;
        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("sphere_vertices"),
            contents: bytemuck::cast_slice(&vertices),
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        });
        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("sphere_indices"),
            contents: bytemuck::cast_slice(&indices),
            usage: wgpu::BufferUsages::INDEX,
        });
        Self { vertex_buffer, index_buffer, index_count: indices.len() as u32 }
    }

    pub fn pyramid(device: &wgpu::Device) -> Self {
        let mut vertices = Vec::new();
        let mut indices = Vec::new();
        
        let a = [-0.5, 0.0, 0.5];
        let b = [0.5, 0.0, 0.5];
        let c = [0.5, 0.0, -0.5];
        let d = [-0.5, 0.0, -0.5];
        let apex = [0.0, 1.0, 0.0];
        
        // Основание
        let p0 = Vec3::from(a); let p1 = Vec3::from(d); let p2 = Vec3::from(c);
        let n = (p1 - p0).cross(p2 - p0).normalize();
        let base = vertices.len() as u16;
        vertices.push(Vertex { position: a, normal: n.into(), uv: [0.0, 0.0], color: [1.0; 3] });
        vertices.push(Vertex { position: d, normal: n.into(), uv: [1.0, 0.0], color: [1.0; 3] });
        vertices.push(Vertex { position: c, normal: n.into(), uv: [1.0, 1.0], color: [1.0; 3] });
        vertices.push(Vertex { position: b, normal: n.into(), uv: [0.0, 1.0], color: [1.0; 3] });
        indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);

        // Боковые грани
        let sides = [(a, b), (b, c), (c, d), (d, a)];
        for (p_start, p_end) in sides {
            let v0 = Vec3::from(p_start);
            let v1 = Vec3::from(p_end);
            let v2 = Vec3::from(apex);
            let n = (v1 - v0).cross(v2 - v0).normalize();
            let base = vertices.len() as u16;
            vertices.push(Vertex { position: p_start, normal: n.into(), uv: [0.0, 0.0], color: [1.0; 3] });
            vertices.push(Vertex { position: p_end, normal: n.into(), uv: [1.0, 0.0], color: [1.0; 3] });
            vertices.push(Vertex { position: apex, normal: n.into(), uv: [0.5, 1.0], color: [1.0; 3] });
            indices.extend_from_slice(&[base, base + 1, base + 2]);
        }

        use wgpu::util::DeviceExt;
        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("pyramid_vertices"),
            contents: bytemuck::cast_slice(&vertices),
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        });
        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("pyramid_indices"),
            contents: bytemuck::cast_slice(&indices),
            usage: wgpu::BufferUsages::INDEX,
        });
        Self { vertex_buffer, index_buffer, index_count: indices.len() as u32 }
    }

    pub fn arrow(device: &wgpu::Device) -> Self {
        let mut vertices: Vec<Vertex> = Vec::new();
        let mut indices: Vec<u16> = Vec::new();
        let segments: u16 = 12;
        let shaft_r: f32 = 0.012;
        let shaft_h: f32 = 0.35;
        let tip_r: f32 = 0.05;
        let tip_h: f32 = 0.10;

        for i in 0..segments {
            let a0 = (i as f32 / segments as f32) * std::f32::consts::TAU;
            let a1 = ((i + 1) as f32 / segments as f32) * std::f32::consts::TAU;
            let c0 = a0.cos(); let s0 = a0.sin();
            let c1 = a1.cos(); let s1 = a1.sin();
            let base = vertices.len() as u16;
            vertices.push(Vertex { position: [shaft_r * c0, 0.0,     shaft_r * s0], normal: [c0, 0.0, s0], uv: [0.0, 0.0], color: [1.0; 3] });
            vertices.push(Vertex { position: [shaft_r * c1, 0.0,     shaft_r * s1], normal: [c1, 0.0, s1], uv: [1.0, 0.0], color: [1.0; 3] });
            vertices.push(Vertex { position: [shaft_r * c1, shaft_h, shaft_r * s1], normal: [c1, 0.0, s1], uv: [1.0, 1.0], color: [1.0; 3] });
            vertices.push(Vertex { position: [shaft_r * c0, shaft_h, shaft_r * s0], normal: [c0, 0.0, s0], uv: [0.0, 1.0], color: [1.0; 3] });
            indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
        }

        let slant = (tip_r * tip_r + tip_h * tip_h).sqrt();
        for i in 0..segments {
            let a0 = (i as f32 / segments as f32) * std::f32::consts::TAU;
            let a1 = ((i + 1) as f32 / segments as f32) * std::f32::consts::TAU;
            let c0 = a0.cos(); let s0 = a0.sin();
            let c1 = a1.cos(); let s1 = a1.sin();
            let n0 = [tip_h * c0 / slant, tip_r / slant, tip_h * s0 / slant];
            let n1 = [tip_h * c1 / slant, tip_r / slant, tip_h * s1 / slant];
            let na = [(n0[0] + n1[0]) * 0.5, (n0[1] + n1[1]) * 0.5, (n0[2] + n1[2]) * 0.5];
            let base = vertices.len() as u16;
            vertices.push(Vertex { position: [tip_r * c0, shaft_h, tip_r * s0], normal: n0, uv: [0.0, 0.0], color: [1.0; 3] });
            vertices.push(Vertex { position: [tip_r * c1, shaft_h, tip_r * s1], normal: n1, uv: [1.0, 0.0], color: [1.0; 3] });
            vertices.push(Vertex { position: [0.0, shaft_h + tip_h, 0.0],    normal: na, uv: [0.5, 1.0], color: [1.0; 3] });
            indices.extend_from_slice(&[base, base + 1, base + 2]);
        }

        let bot_center = vertices.len() as u16;
        vertices.push(Vertex { position: [0.0, shaft_h, 0.0], normal: [0.0, -1.0, 0.0], uv: [0.5, 0.5], color: [1.0; 3] });
        let bot_start = vertices.len() as u16;
        for i in 0..segments {
            let a = (i as f32 / segments as f32) * std::f32::consts::TAU;
            let c = a.cos(); let s = a.sin();
            vertices.push(Vertex { position: [tip_r * c, shaft_h, tip_r * s], normal: [0.0, -1.0, 0.0], uv: [c * 0.5 + 0.5, s * 0.5 + 0.5], color: [1.0; 3] });
        }
        for i in 0..segments {
            let cur = bot_start + i;
            let next = bot_start + ((i + 1) % segments);
            indices.extend_from_slice(&[bot_center, cur, next]);
        }

        use wgpu::util::DeviceExt;
        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("arrow_vertices"),
            contents: bytemuck::cast_slice(&vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("arrow_indices"),
            contents: bytemuck::cast_slice(&indices),
            usage: wgpu::BufferUsages::INDEX,
        });
        Self { vertex_buffer, index_buffer, index_count: indices.len() as u32 }
    }

    pub fn ring(device: &wgpu::Device) -> Self {
        let mut vertices: Vec<Vertex> = Vec::new();
        let mut indices: Vec<u16> = Vec::new();
        let major_r = 0.4f32;
        let minor_r = 0.02f32;
        let major_seg: u16 = 32;
        let minor_seg: u16 = 8;

        for i in 0..major_seg {
            let a = (i as f32 / major_seg as f32) * std::f32::consts::TAU;
            let (ca, sa) = (a.cos(), a.sin());
            for j in 0..minor_seg {
                let b = (j as f32 / minor_seg as f32) * std::f32::consts::TAU;
                let (cb, sb) = (b.cos(), b.sin());
                let r = major_r + minor_r * cb;
                let pos = [r * ca, r * sa, minor_r * sb];
                let normal = [cb * ca, cb * sa, sb];
                vertices.push(Vertex { position: pos, normal, uv: [i as f32 / major_seg as f32, j as f32 / minor_seg as f32], color: [1.0; 3] });
            }
        }

        for i in 0..major_seg {
            let i_next = (i + 1) % major_seg;
            for j in 0..minor_seg {
                let j_next = (j + 1) % minor_seg;
                let a = i * minor_seg + j;
                let b = i * minor_seg + j_next;
                let c = i_next * minor_seg + j_next;
                let d = i_next * minor_seg + j;
                indices.extend_from_slice(&[a, b, c, a, c, d]);
            }
        }

        use wgpu::util::DeviceExt;
        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("ring_vertices"),
            contents: bytemuck::cast_slice(&vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("ring_indices"),
            contents: bytemuck::cast_slice(&indices),
            usage: wgpu::BufferUsages::INDEX,
        });
        Self { vertex_buffer, index_buffer, index_count: indices.len() as u32 }
    }

    pub fn cursor_3d(device: &wgpu::Device) -> Self {
        let mut vertices: Vec<Vertex> = Vec::new();
        let mut indices: Vec<u16> = Vec::new();

        let major_r = 0.2f32;
        let minor_r = 0.015f32;
        let major_seg: u16 = 32;
        let minor_seg: u16 = 6;

        for i in 0..major_seg {
            let a = (i as f32 / major_seg as f32) * std::f32::consts::TAU;
            let (ca, sa) = (a.cos(), a.sin());
            for j in 0..minor_seg {
                let b = (j as f32 / minor_seg as f32) * std::f32::consts::TAU;
                let (cb, sb) = (b.cos(), b.sin());
                let r = major_r + minor_r * cb;
                let pos = [r * ca, minor_r * sb, r * sa];
                let normal = [cb * ca, sb, cb * sa];
                vertices.push(Vertex { position: pos, normal, uv: [0.0, 0.0], color: [1.0; 3] });
            }
        }
        for i in 0..major_seg {
            let i_next = (i + 1) % major_seg;
            for j in 0..minor_seg {
                let j_next = (j + 1) % minor_seg;
                let a = i * minor_seg + j;
                let b = i * minor_seg + j_next;
                let c = i_next * minor_seg + j_next;
                let d = i_next * minor_seg + j;
                indices.extend_from_slice(&[a, b, c, a, c, d]);
            }
        }

        let mut add_line = |p1: Vec3, p2: Vec3, up: Vec3| {
            let dir = (p2 - p1).normalize();
            let right = dir.cross(up).normalize();
            let thick = 0.01;
            let off = right * thick;
            let base = vertices.len() as u16;
            vertices.push(Vertex { position: (p1 - off).into(), normal: [0.0, 1.0, 0.0], uv: [0.0, 0.0], color: [1.0; 3] });
            vertices.push(Vertex { position: (p1 + off).into(), normal: [0.0, 1.0, 0.0], uv: [1.0, 0.0], color: [1.0; 3] });
            vertices.push(Vertex { position: (p2 + off).into(), normal: [0.0, 1.0, 0.0], uv: [1.0, 1.0], color: [1.0; 3] });
            vertices.push(Vertex { position: (p2 - off).into(), normal: [0.0, 1.0, 0.0], uv: [0.0, 1.0], color: [1.0; 3] });
            indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
        };

        let y = Vec3::Y;
        add_line(Vec3::new(-0.4, 0.0, 0.0), Vec3::new(-0.2, 0.0, 0.0), y);
        add_line(Vec3::new(0.2, 0.0, 0.0), Vec3::new(0.4, 0.0, 0.0), y);
        add_line(Vec3::new(0.0, 0.0, -0.4), Vec3::new(0.0, 0.0, -0.2), y);
        add_line(Vec3::new(0.0, 0.0, 0.2), Vec3::new(0.0, 0.0, 0.4), y);

        use wgpu::util::DeviceExt;
        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("cursor_vertices"),
            contents: bytemuck::cast_slice(&vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("cursor_indices"),
            contents: bytemuck::cast_slice(&indices),
            usage: wgpu::BufferUsages::INDEX,
        });
        Self { vertex_buffer, index_buffer, index_count: indices.len() as u32 }
    }
}