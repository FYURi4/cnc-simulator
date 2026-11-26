use std::sync::Arc;
use anyhow::Result;
use egui_wgpu::ScreenDescriptor;
use egui_winit::State as EguiWinitState;
use wgpu::util::DeviceExt;
use winit::window::Window;

use crate::camera::Camera;
use crate::mesh::{Mesh, MeshKind, Vertex};
use crate::texture::DepthTexture;

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct CameraUniform {
    view_proj: [[f32; 4]; 4],
    eye: [f32; 3],
    grid_extent: f32,
    grid_color: [f32; 4],
    grid_type: f32,
    grid_visible: f32,
    grid_step_x: f32,
    grid_step_y: f32,
}

impl CameraUniform {
    fn from_camera(cam: &Camera, grid_color: [f32; 3], grid_type: f32, grid_visible: bool, grid_step_x: f32, grid_step_y: f32) -> Self {
        Self {
            view_proj: cam.view_proj().to_cols_array_2d(),
            eye: cam.position.into(),
            grid_extent: 50.0,
            grid_color: [grid_color[0], grid_color[1], grid_color[2], 1.0],
            grid_type,
            grid_visible: if grid_visible { 1.0 } else { 0.0 },
            grid_step_x,
            grid_step_y,
        }
    }
}

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct ObjectUniform {
    model: [[f32; 4]; 4],
    tint: [f32; 4],
}

#[derive(Clone, Copy, PartialEq)]
pub enum ObjectKind {
    Grid,
    Solid,
    Gizmo,
}

pub struct RenderObject {
    pub mesh: Mesh,
    pub buffer: wgpu::Buffer,
    pub bind_group: wgpu::BindGroup,
    pub kind: ObjectKind,
    pub model: glam::Mat4,
    pub tint: [f32; 4],
    pub mesh_kind: Option<MeshKind>,
}

pub struct State {
    pub grid_extent: f32,
    pub grid_color: [f32; 3],
    pub grid_visible: bool,
    pub grid_type: f32,
    pub grid_step_x: f32,
    pub grid_step_y: f32,
    pub surface: wgpu::Surface<'static>,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub config: wgpu::SurfaceConfiguration,
    pub depth_texture: DepthTexture,
    pub camera: Camera,
    pub camera_buffer: wgpu::Buffer,
    pub camera_bind_group: wgpu::BindGroup,
    pub objects: Vec<RenderObject>,
    pub window: Arc<Window>,
    pub egui_ctx: egui::Context,
    pub egui_state: EguiWinitState,
    pub egui_renderer: egui_wgpu::Renderer,
    object_bgl: wgpu::BindGroupLayout,
    grid_pipeline: wgpu::RenderPipeline,
    solid_pipeline: wgpu::RenderPipeline,
}

impl State {
    pub async fn new(window: Arc<Window>) -> Result<Self> {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
        let surface = instance.create_surface(window.clone())?;

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await?;

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("device"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
                memory_hints: wgpu::MemoryHints::default(),
                experimental_features: wgpu::ExperimentalFeatures::disabled(),
                trace: wgpu::Trace::Off,
            })
            .await?;

        let size = window.inner_size();
        let caps = surface.get_capabilities(&adapter);
        let format = caps
            .formats
            .iter()
            .copied()
            .find(|f| f.is_srgb())
            .unwrap_or(caps.formats[0]);

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode: wgpu::PresentMode::Fifo,
            alpha_mode: caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &config);

        let depth_texture = DepthTexture::new(&device, &config)?;
        let camera = Camera::new(config.width as f32 / config.height as f32);
        let camera_uniform = CameraUniform::from_camera(&camera, [0.5, 0.5, 0.5], 0.0, true, 0.001, 0.001);

        let camera_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("camera_uniform"),
            contents: bytemuck::bytes_of(&camera_uniform),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let camera_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("camera_bgl"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });

        let camera_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("camera_bg"),
            layout: &camera_bgl,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: camera_buffer.as_entire_binding(),
            }],
        });

        let object_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("object_bgl"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });

        let make_pipeline = |shader_src: &'static str, label: &str, is_grid: bool| -> wgpu::RenderPipeline {
            let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some(label),
                source: wgpu::ShaderSource::Wgsl(shader_src.into()),
            });
            let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some(label),
                bind_group_layouts: &[&camera_bgl, &object_bgl],
                push_constant_ranges: &[],
            });
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(label),
                layout: Some(&layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs_main"),
                    buffers: &[Vertex::layout()],
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some("fs_main"),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: config.format,
                        blend: if is_grid { Some(wgpu::BlendState::ALPHA_BLENDING) } else { Some(wgpu::BlendState::REPLACE) },
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                }),
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleList,
                    cull_mode: None,
                    ..Default::default()
                },
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: DepthTexture::FORMAT,
                    depth_write_enabled: !is_grid,
                    depth_compare: wgpu::CompareFunction::Less,
                    stencil: wgpu::StencilState::default(),
                    bias: wgpu::DepthBiasState::default(),
                }),
                multisample: wgpu::MultisampleState::default(),
                multiview: None,
                cache: None,
            })
        };

        let grid_pipeline = make_pipeline(include_str!("../shaders/mesh.wgsl"), "grid_pipeline", true);
        let solid_pipeline = make_pipeline(include_str!("../shaders/solid.wgsl"), "solid_pipeline", false);

        let egui_ctx = egui::Context::default();
        let egui_state = EguiWinitState::new(
            egui_ctx.clone(),
            egui::ViewportId::ROOT,
            &window,
            Some(window.scale_factor() as f32),
            None,
            None,
        );
        let egui_renderer = egui_wgpu::Renderer::new(
            &device,
            config.format,
            egui_wgpu::RendererOptions::default(),
        );

        Ok(Self {
            surface,
            device,
            queue,
            config,
            depth_texture,
            camera,
            camera_buffer,
            camera_bind_group,
            grid_extent: 50.0,
            grid_color: [0.5, 0.5, 0.5],
            grid_visible: true,
            grid_type: 0.0,
            grid_step_x: 0.001, // 1mm
            grid_step_y: 0.001, // 1mm
            objects: Vec::new(),
            window,
            egui_ctx,
            egui_state,
            egui_renderer,
            object_bgl,
            grid_pipeline,
            solid_pipeline,
        })
    }

    pub fn handle_egui_event(&mut self, event: &winit::event::WindowEvent) -> egui_winit::EventResponse {
        self.egui_state.on_window_event(&self.window, event)
    }

    pub fn add_object(&mut self, mesh: Mesh, kind: ObjectKind, mesh_kind: Option<MeshKind>) -> usize {
        let buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("object_uniform"),
            size: std::mem::size_of::<ObjectUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("object_bg"),
            layout: &self.object_bgl,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: buffer.as_entire_binding(),
            }],
        });
        let idx = self.objects.len();
        self.objects.push(RenderObject {
            mesh,
            buffer,
            bind_group,
            kind,
            model: glam::Mat4::IDENTITY,
            tint: [1.0, 1.0, 1.0, 1.0],
            mesh_kind,
        });
        self.set_object_transform(idx, glam::Mat4::IDENTITY);
        self.set_object_tint(idx, [1.0, 1.0, 1.0, 1.0]);
        idx
    }

        pub fn update_object_vertices(&mut self, idx: usize, vertices: &[Vertex]) {
        if idx >= self.objects.len() { return; }
        let obj = &mut self.objects[idx];

        let num_indices = vertices.len() as u32;
        if num_indices == 0 { return; }

        let new_v_size = (num_indices as usize * std::mem::size_of::<Vertex>()) as u64;
        let new_i_size = (num_indices as usize * std::mem::size_of::<u16>()) as u64;

        // Генерируем последовательные индексы (0, 1, 2, ...) для триангулированного меша
        let indices: Vec<u16> = (0..num_indices).map(|i| i as u16).collect();

        if obj.mesh.vertex_buffer.size() < new_v_size || obj.mesh.index_buffer.size() < new_i_size {
            use wgpu::util::DeviceExt;
            let v_buf = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("dynamic_vertices"),
                contents: bytemuck::cast_slice(vertices),
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            });
            let i_buf = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("dynamic_indices"),
                contents: bytemuck::cast_slice(&indices),
                usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
            });
            obj.mesh.vertex_buffer = v_buf;
            obj.mesh.index_buffer = i_buf;
        } else {
            self.queue.write_buffer(&obj.mesh.vertex_buffer, 0, bytemuck::cast_slice(vertices));
            self.queue.write_buffer(&obj.mesh.index_buffer, 0, bytemuck::cast_slice(&indices));
        }
        obj.mesh.index_count = num_indices;
    }

    pub fn set_object_transform(&mut self, idx: usize, mat: glam::Mat4) {
        if idx >= self.objects.len() { return; }
        self.objects[idx].model = mat;
        let u = ObjectUniform {
            model: mat.to_cols_array_2d(),
            tint: self.objects[idx].tint,
        };
        self.queue.write_buffer(&self.objects[idx].buffer, 0, bytemuck::bytes_of(&u));
    }

    pub fn set_object_tint(&mut self, idx: usize, tint: [f32; 4]) {
        if idx >= self.objects.len() { return; }
        self.objects[idx].tint = tint;
        let u = ObjectUniform {
            model: self.objects[idx].model.to_cols_array_2d(),
            tint,
        };
        self.queue.write_buffer(&self.objects[idx].buffer, 0, bytemuck::bytes_of(&u));
    }

    pub fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        if new_size.width > 0 && new_size.height > 0 {
            self.config.width = new_size.width;
            self.config.height = new_size.height;
            self.camera.aspect = self.config.width as f32 / self.config.height as f32;
            self.surface.configure(&self.device, &self.config);
            self.depth_texture = DepthTexture::new(&self.device, &self.config).expect("recreate depth texture");
        }
    }

    pub fn update_camera(&mut self) {
        let mut uniform = CameraUniform::from_camera(&self.camera, self.grid_color, self.grid_type, self.grid_visible, self.grid_step_x, self.grid_step_y);
        uniform.grid_extent = self.grid_extent;
        self.queue.write_buffer(&self.camera_buffer, 0, bytemuck::bytes_of(&uniform));
    }

    pub fn render_with_ui<F>(&mut self, mut ui_fn: F) -> Result<()>
    where F: FnMut(&egui::Context) {
        let raw_input = self.egui_state.take_egui_input(&self.window);
        let full_output = self.egui_ctx.run(raw_input, &mut ui_fn);

        let paint_jobs = self.egui_ctx.tessellate(full_output.shapes, full_output.pixels_per_point);
        let screen_descriptor = ScreenDescriptor {
            size_in_pixels: [self.config.width, self.config.height],
            pixels_per_point: full_output.pixels_per_point,
        };

        for (id, image_delta) in &full_output.textures_delta.set {
            self.egui_renderer.update_texture(&self.device, &self.queue, *id, image_delta);
        }

        let output = self.surface.get_current_texture()?;
        let view = output.texture.create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("render_encoder"),
        });

        let cmd_buffers = self.egui_renderer.update_buffers(
            &self.device, &self.queue, &mut encoder, &paint_jobs, &screen_descriptor,
        );

        {
            let mut rp = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("solid_pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color { r: 0.05, g: 0.05, b: 0.08, a: 1.0 }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth_texture.view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            rp.set_bind_group(0, &self.camera_bind_group, &[]);
            rp.set_pipeline(&self.solid_pipeline);
            for obj in self.objects.iter().filter(|o| o.kind == ObjectKind::Solid) {
                rp.set_bind_group(1, &obj.bind_group, &[]);
                rp.set_vertex_buffer(0, obj.mesh.vertex_buffer.slice(..));
                rp.set_index_buffer(obj.mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint16);
                rp.draw_indexed(0..obj.mesh.index_count, 0, 0..1);
            }
        }

        {
            let mut rp = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("grid_pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth_texture.view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            rp.set_bind_group(0, &self.camera_bind_group, &[]);
            rp.set_pipeline(&self.grid_pipeline);
            for obj in self.objects.iter().filter(|o| o.kind == ObjectKind::Grid) {
                rp.set_bind_group(1, &obj.bind_group, &[]);
                rp.set_vertex_buffer(0, obj.mesh.vertex_buffer.slice(..));
                rp.set_index_buffer(obj.mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint16);
                rp.draw_indexed(0..obj.mesh.index_count, 0, 0..1);
            }
        }

        {
            let mut rp = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("gizmo_pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth_texture.view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            rp.set_pipeline(&self.solid_pipeline);
            rp.set_bind_group(0, &self.camera_bind_group, &[]);
            for obj in self.objects.iter().filter(|o| o.kind == ObjectKind::Gizmo) {
                rp.set_bind_group(1, &obj.bind_group, &[]);
                rp.set_vertex_buffer(0, obj.mesh.vertex_buffer.slice(..));
                rp.set_index_buffer(obj.mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint16);
                rp.draw_indexed(0..obj.mesh.index_count, 0, 0..1);
            }
        }

        {
            let mut rp = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("egui_pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            }).forget_lifetime();
            self.egui_renderer.render(&mut rp, &paint_jobs, &screen_descriptor);
        }

        for id in &full_output.textures_delta.free {
            self.egui_renderer.free_texture(id);
        }

        self.egui_state.handle_platform_output(&self.window, full_output.platform_output);

        let mut cmds = cmd_buffers;
        cmds.push(encoder.finish());
        self.queue.submit(cmds.into_iter());
        output.present();
        Ok(())
    }
}