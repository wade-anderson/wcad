use wgpu::util::DeviceExt;

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Vertex {
    pub position: [f32; 2],
    pub color: [f32; 3],
}

impl Vertex {
    pub fn desc<'a>() -> wgpu::VertexBufferLayout<'a> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Vertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x2,
                },
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[f32; 2]>() as wgpu::BufferAddress,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x3,
                },
            ],
        }
    }
}

pub struct Renderer {
    device: wgpu::Device,
    queue: wgpu::Queue,
    render_pipeline: wgpu::RenderPipeline,
    camera_buffer: wgpu::Buffer,
    camera_bind_group: wgpu::BindGroup,
    
    // Cached resources
    render_texture: Option<wgpu::Texture>,
    output_buffer: Option<wgpu::Buffer>,
    width: u32,
    height: u32,
}

impl Renderer {
    pub async fn new() -> Self {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });

        let adapter = instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: None,
            force_fallback_adapter: false,
        }).await.expect("Failed to find an appropriate adapter");

        let (device, queue) = adapter.request_device(
            &wgpu::DeviceDescriptor {
                label: None,
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
            },
            None,
        ).await.expect("Failed to create device");

        let shader = device.create_shader_module(wgpu::include_wgsl!("shader.wgsl"));

        let camera_bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }
            ],
            label: Some("camera_bind_group_layout"),
        });

        let camera_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Camera Buffer"),
            contents: bytemuck::cast_slice(&[
                1.0f32, 0.0, 0.0, 0.0,
                0.0, 1.0, 0.0, 0.0,
                0.0, 0.0, 1.0, 0.0,
                0.0, 0.0, 0.0, 1.0,
            ]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let camera_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &camera_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: camera_buffer.as_entire_binding(),
                }
            ],
            label: Some("camera_bind_group"),
        });

        let render_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Render Pipeline Layout"),
            bind_group_layouts: &[&camera_bind_group_layout],
            push_constant_ranges: &[],
        });

        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Render Pipeline"),
            layout: Some(&render_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                buffers: &[Vertex::desc()],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format: wgpu::TextureFormat::Bgra8Unorm,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview: None,
        });

        Self {
            device,
            queue,
            render_pipeline,
            camera_buffer,
            camera_bind_group,
            render_texture: None,
            output_buffer: None,
            width: 0,
            height: 0,
        }
    }

    pub fn update_view(&self, offset: [f32; 2], zoom: f32, width: f32, height: f32) {
        let matrix = calculate_projection_matrix(offset, zoom, width, height);
        let matrix_ref: &[[f32; 4]; 4] = matrix.as_ref();
        self.queue.write_buffer(&self.camera_buffer, 0, bytemuck::cast_slice(matrix_ref));
    }

    pub fn render(&mut self, width: u32, height: u32, vertices: &[Vertex], indices: &[u32]) -> Vec<u8> {
        if width == 0 || height == 0 {
            return Vec::new();
        }

        // Recreate resources if size changed
        if self.width != width || self.height != height || self.render_texture.is_none() {
            let texture_desc = wgpu::TextureDescriptor {
                size: wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Bgra8Unorm,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
                label: Some("Render Texture"),
                view_formats: &[],
            };
            self.render_texture = Some(self.device.create_texture(&texture_desc));

            let u32_size = std::mem::size_of::<u32>() as u32;
            let bytes_per_row = (u32_size * width + 255) & !255;
            let output_buffer_size = (bytes_per_row * height) as wgpu::BufferAddress;
            let output_buffer_desc = wgpu::BufferDescriptor {
                size: output_buffer_size,
                usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                label: Some("Output Buffer"),
                mapped_at_creation: false,
            };
            self.output_buffer = Some(self.device.create_buffer(&output_buffer_desc));
            self.width = width;
            self.height = height;
        }

        let render_texture = self.render_texture.as_ref().unwrap();
        let output_buffer = self.output_buffer.as_ref().unwrap();
        let view = render_texture.create_view(&wgpu::TextureViewDescriptor::default());

        let vertex_buffer = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Vertex Buffer"),
            contents: bytemuck::cast_slice(vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let index_buffer = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Index Buffer"),
            contents: bytemuck::cast_slice(indices),
            usage: wgpu::BufferUsages::INDEX,
        });

        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: None,
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color { r: 0.1, g: 0.1, b: 0.1, a: 1.0 }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                occlusion_query_set: None,
                timestamp_writes: None,
            });

            render_pass.set_pipeline(&self.render_pipeline);
            render_pass.set_bind_group(0, &self.camera_bind_group, &[]);
            render_pass.set_vertex_buffer(0, vertex_buffer.slice(..));
            render_pass.set_index_buffer(index_buffer.slice(..), wgpu::IndexFormat::Uint32);
            render_pass.draw_indexed(0..indices.len() as u32, 0, 0..1);
        }

        let u32_size = std::mem::size_of::<u32>() as u32;
        let bytes_per_row = (u32_size * width + 255) & !255;

        encoder.copy_texture_to_buffer(
            wgpu::ImageCopyTexture {
                aspect: wgpu::TextureAspect::All,
                texture: render_texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
            },
            wgpu::ImageCopyBuffer {
                buffer: output_buffer,
                layout: wgpu::ImageDataLayout {
                    offset: 0,
                    bytes_per_row: Some(bytes_per_row),
                    rows_per_image: Some(height),
                },
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );

        self.queue.submit(Some(encoder.finish()));

        let buffer_slice = output_buffer.slice(..);
        let (tx, rx) = std::sync::mpsc::channel();
        buffer_slice.map_async(wgpu::MapMode::Read, move |v| tx.send(v).unwrap());
        self.device.poll(wgpu::Maintain::Wait);

        if let Ok(Ok(())) = rx.recv() {
            let data = buffer_slice.get_mapped_range();
            let mut result = Vec::with_capacity((u32_size * width * height) as usize);
            
            for chunk in data.chunks_exact(bytes_per_row as usize) {
                result.extend_from_slice(&chunk[..(u32_size * width) as usize]);
            }
            
            drop(data);
            output_buffer.unmap();
            result
        } else {
            panic!("Failed to read buffer from GPU");
        }
    }
}

fn calculate_projection_matrix(offset: [f32; 2], zoom: f32, width: f32, height: f32) -> nalgebra::Matrix4<f32> {
    let aspect = width / height;
    let ortho = nalgebra::Orthographic3::new(
        -aspect / zoom + offset[0],
        aspect / zoom + offset[0],
        -1.0 / zoom + offset[1],
        1.0 / zoom + offset[1],
        -1.0,
        1.0,
    );
    ortho.to_homogeneous()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_projection_matrix_identity() {
        // At zoom 1.0, offset 0.0, 1:1 aspect ratio, 
        // the matrix should project (-1,-1) to (-1,-1) and (1,1) to (1,1)
        let matrix = calculate_projection_matrix([0.0, 0.0], 1.0, 100.0, 100.0);
        let p = nalgebra::Vector4::new(1.0, 1.0, 0.0, 1.0);
        let result = matrix * p;
        assert!((result.x - 1.0).abs() < 1e-6);
        assert!((result.y - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_projection_matrix_zoom() {
        // At zoom 2.0, everything should be twice as large in clip space
        let matrix = calculate_projection_matrix([0.0, 0.0], 2.0, 100.0, 100.0);
        let p = nalgebra::Vector4::new(0.5, 0.5, 0.0, 1.0);
        let result = matrix * p;
        assert!((result.x - 1.0).abs() < 1e-6);
        assert!((result.y - 1.0).abs() < 1e-6);
    }
}
