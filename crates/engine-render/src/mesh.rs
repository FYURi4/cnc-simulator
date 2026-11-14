use bytemuck::{Pod, Zeroable};

#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
pub struct Vertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub uv: [f32; 2],
}

impl Vertex {
    pub fn layout() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Vertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute { offset: 0,  shader_location: 0, format: wgpu::VertexFormat::Float32x3 },
                wgpu::VertexAttribute { offset: 12, shader_location: 1, format: wgpu::VertexFormat::Float32x3 },
                wgpu::VertexAttribute { offset: 24, shader_location: 2, format: wgpu::VertexFormat::Float32x2 },
            ],
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
            Vertex { position: [-h, 0.0,  h], normal: [0.0, 1.0, 0.0], uv: [0.0, 1.0] },
            Vertex { position: [ h, 0.0,  h], normal: [0.0, 1.0, 0.0], uv: [1.0, 1.0] },
            Vertex { position: [ h, 0.0, -h], normal: [0.0, 1.0, 0.0], uv: [1.0, 0.0] },
            Vertex { position: [-h, 0.0, -h], normal: [0.0, 1.0, 0.0], uv: [0.0, 0.0] },
        ];
        let indices: &[u16] = &[0, 1, 2, 0, 2, 3];

        use wgpu::util::DeviceExt;
        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("plane_vertices"),
            contents: bytemuck::cast_slice(&vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("plane_indices"),
            contents: bytemuck::cast_slice(indices),
            usage: wgpu::BufferUsages::INDEX,
        });

        Self { vertex_buffer, index_buffer, index_count: indices.len() as u32 }
    }
}
