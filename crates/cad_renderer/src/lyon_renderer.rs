use lyon::tessellation::*;
use slint::wgpu_27::wgpu;
use truck_modeling::*;

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct LyonVertex {
    pub position: [f32; 3], // [x, y, z]
    pub color: [f32; 4],    // [r, g, b, a]
}

pub struct LyonLineRenderer {
    pipeline: wgpu::RenderPipeline,
    vertex_buffer: Option<wgpu::Buffer>,
    index_buffer: Option<wgpu::Buffer>,
    index_count: u32,
    camera_buffer: wgpu::Buffer,
    camera_bind_group: wgpu::BindGroup,
}

impl LyonLineRenderer {
    pub fn new(device: &wgpu::Device) -> Self {
        // 1. シェーダーの読み込み
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Lyon Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("lyon_shader/lyon_shader.wgsl").into()),
        });

        // 2. カメラ行列用 Uniform / BindGroup の作成
        let camera_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Lyon Camera Buffer"),
            size: 64, // mat4x4 f32
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Lyon Bind Group Layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });

        let camera_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Lyon Bind Group"),
            layout: &bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: camera_buffer.as_entire_binding(),
            }],
        });

        // create the rendering pipeline
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Lyon Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Lyon Render Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<LyonVertex>() as wgpu::BufferAddress,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x4],
                }],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: wgpu::TextureFormat::Rgba8Unorm,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        Self {
            pipeline,
            vertex_buffer: None,
            index_buffer: None,
            index_count: 0,
            camera_buffer,
            camera_bind_group,
        }
    }

    pub fn update_buffers(&mut self, device: &wgpu::Device, raw_lines: &Vec<Vec<Point3>>) {
        let mut geometry: VertexBuffers<LyonVertex, u16> = VertexBuffers::new();
        let mut tessellator = StrokeTessellator::new();

        for line in raw_lines {
            if line.len() < 2 {
                continue;
            }
            let mut path_builder = lyon::path::Path::builder();

            // 3D空間の座標を一旦lyon（2D平面想定、ここではローカル平面のX,Yとする）にダミーキャスト
            // ※本来は配置平面マトリクスに応じて変換します
            path_builder.begin(lyon::math::point(line[0].x as f32, line[0].y as f32));
            for pt in line.iter().skip(1) {
                path_builder.line_to(lyon::math::point(pt.x as f32, pt.y as f32));
            }
            path_builder.end(false);
            let path = path_builder.build();

            // 太さ 3.0 ピクセルの綺麗な線を生成
            let options = StrokeOptions::default()
                .with_line_width(0.15)
                .with_line_join(LineJoin::Round)
                .with_line_cap(LineCap::Round);

            let _ = tessellator.tessellate_path(
                &path,
                &options,
                &mut BuffersBuilder::new(&mut geometry, |vertex: StrokeVertex| {
                    LyonVertex {
                        // ここでZ座標を保持（または平面上に乗せる処理）
                        position: [vertex.position().x, vertex.position().y, 0.0],
                        color: [0.0, 0.0, 0.0, 1.0],
                    }
                }),
            );
        }

        if geometry.vertices.is_empty() {
            return;
        }

        // write to GPU buffers
        use wgpu::util::DeviceExt;
        self.vertex_buffer = Some(
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Lyon Vertex Buffer"),
                contents: bytemuck::cast_slice(&geometry.vertices),
                usage: wgpu::BufferUsages::VERTEX,
            }),
        );
        self.index_buffer = Some(
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Lyon Index Buffer"),
                contents: bytemuck::cast_slice(&geometry.indices),
                usage: wgpu::BufferUsages::INDEX,
            }),
        );
        self.index_count = geometry.indices.len() as u32;
    }

    // カメラ行列の同期
    pub fn update_camera(&self, queue: &wgpu::Queue, matrix: Matrix4) {
        // f64 の Matrix4 を f32 配列に変換して送る
        let f32_matrix: [f32; 16] = [
            matrix.x.x as f32,
            matrix.x.y as f32,
            matrix.x.z as f32,
            matrix.x.w as f32,
            matrix.y.x as f32,
            matrix.y.y as f32,
            matrix.y.z as f32,
            matrix.y.w as f32,
            matrix.z.x as f32,
            matrix.z.y as f32,
            matrix.z.z as f32,
            matrix.z.w as f32,
            matrix.w.x as f32,
            matrix.w.y as f32,
            matrix.w.z as f32,
            matrix.w.w as f32,
        ];
        queue.write_buffer(&self.camera_buffer, 0, bytemuck::cast_slice(&f32_matrix));
    }

    // 重ね書き実行
    pub fn draw<'a>(&'a self, rpass: &mut wgpu::RenderPass<'a>) {
        if let (Some(v_buf), Some(i_buf)) = (&self.vertex_buffer, &self.index_buffer) {
            rpass.set_pipeline(&self.pipeline);
            rpass.set_bind_group(0, &self.camera_bind_group, &[]);
            rpass.set_vertex_buffer(0, v_buf.slice(..));
            rpass.set_index_buffer(i_buf.slice(..), wgpu::IndexFormat::Uint16);
            rpass.draw_indexed(0..self.index_count, 0, 0..1);
        }
    }
}
