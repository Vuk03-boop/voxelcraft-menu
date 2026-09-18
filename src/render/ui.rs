//! Immediate-mode 2D overlay: colored rects and bitmap text, one draw per frame.

use super::font;
use bytemuck::{Pod, Zeroable};
use wgpu::util::DeviceExt;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct UiVertex {
    pub pos: [f32; 2],
    pub uv: [f32; 2],
    pub color: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct UiUniform {
    screen: [f32; 2],
    pad: [f32; 2],
}

pub struct UiRenderer {
    pipeline: wgpu::RenderPipeline,
    bind_group: wgpu::BindGroup,
    uniform: wgpu::Buffer,
    vbuf: wgpu::Buffer,
    vcap: usize,
    pub verts: Vec<UiVertex>,
}

impl UiRenderer {
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue, format: wgpu::TextureFormat) -> Self {
        let (pixels, w, h) = font::build_font_texture();
        let tex = device.create_texture_with_data(
            queue,
            &wgpu::TextureDescriptor {
                label: Some("font"),
                size: wgpu::Extent3d {
                    width: w,
                    height: h,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::R8Unorm,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            },
            wgpu::util::TextureDataOrder::LayerMajor,
            &pixels,
        );
        let view = tex.create_view(&Default::default());
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });
        let uniform = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("ui uniform"),
            size: 16,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("ui bgl"),
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
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("ui bg"),
            layout: &bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniform.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        });
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("ui"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/ui.wgsl").into()),
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("ui layout"),
            bind_group_layouts: &[Some(&bgl)],
            immediate_size: 0,
        });
        let vlayout = wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<UiVertex>() as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &wgpu::vertex_attr_array![0 => Float32x2, 1 => Float32x2, 2 => Float32x4],
        };
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("ui"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs"),
                compilation_options: Default::default(),
                buffers: &[Some(vlayout)],
            },
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });
        let vcap = 1 << 16;
        let vbuf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("ui vbuf"),
            size: (vcap * std::mem::size_of::<UiVertex>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        Self {
            pipeline,
            bind_group,
            uniform,
            vbuf,
            vcap,
            verts: Vec::new(),
        }
    }

    pub fn rect(&mut self, x: f32, y: f32, w: f32, h: f32, color: [f32; 4]) {
        let uv = [-1.0, -1.0];
        let v = |px: f32, py: f32| UiVertex {
            pos: [px, py],
            uv,
            color,
        };
        self.verts.extend_from_slice(&[
            v(x, y),
            v(x + w, y),
            v(x + w, y + h),
            v(x, y),
            v(x + w, y + h),
            v(x, y + h),
        ]);
    }

    pub fn text(&mut self, x: f32, y: f32, scale: f32, s: &str, color: [f32; 4]) {
        let mut cx = x;
        let cw = font::CELL_W as f32 * scale;
        let gw = font::GLYPH_W as f32 * scale;
        let gh = font::GLYPH_H as f32 * scale;
        let total_w = (font::COUNT * font::CELL_W) as f32;
        let total_h = font::CELL_H as f32;
        for ch in s.bytes() {
            let ch = ch.to_ascii_uppercase();
            if ch != b' ' && ch >= font::FIRST && (ch as u32) < font::FIRST as u32 + font::COUNT {
                let i = (ch - font::FIRST) as f32;
                let u0 = (i * font::CELL_W as f32) / total_w;
                let u1 = (i * font::CELL_W as f32 + font::GLYPH_W as f32) / total_w;
                let v0 = 0.0;
                let v1 = font::GLYPH_H as f32 / total_h;
                let v = |px: f32, py: f32, u: f32, vv: f32| UiVertex {
                    pos: [px, py],
                    uv: [u, vv],
                    color,
                };
                self.verts.extend_from_slice(&[
                    v(cx, y, u0, v0),
                    v(cx + gw, y, u1, v0),
                    v(cx + gw, y + gh, u1, v1),
                    v(cx, y, u0, v0),
                    v(cx + gw, y + gh, u1, v1),
                    v(cx, y + gh, u0, v1),
                ]);
            }
            cx += cw;
        }
    }

    pub fn text_shadow(&mut self, x: f32, y: f32, scale: f32, s: &str, color: [f32; 4]) {
        self.text(x + scale, y + scale, scale, s, [0.0, 0.0, 0.0, 0.8]);
        self.text(x, y, scale, s, color);
    }

    pub fn draw(
        &mut self,
        queue: &wgpu::Queue,
        pass: &mut wgpu::RenderPass<'_>,
        screen: (u32, u32),
    ) {
        if self.verts.is_empty() {
            return;
        }
        if self.verts.len() > self.vcap {
            self.verts.truncate(self.vcap);
        }
        queue.write_buffer(
            &self.uniform,
            0,
            bytemuck::bytes_of(&UiUniform {
                screen: [screen.0 as f32, screen.1 as f32],
                pad: [0.0; 2],
            }),
        );
        queue.write_buffer(&self.vbuf, 0, bytemuck::cast_slice(&self.verts));
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &self.bind_group, &[]);
        pass.set_vertex_buffer(0, self.vbuf.slice(..));
        pass.draw(0..self.verts.len() as u32, 0..1);
        self.verts.clear();
    }
}



