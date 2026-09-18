//! Fullscreen presentation of the compute pass output: tone-map the linear HDR trace
//! target, then encode for display.

use bytemuck::{Pod, Zeroable};

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct BlitUniform {
    /// 1 when `format` has no hardware sRGB encode, so the shader has to encode itself.
    encode: u32,
    /// A10's curve arm (`--tone-map`): 0 is the knee hyperbola, 1 the Narkowicz ACES
    /// fit. Set once, at `new`; a swap at anything higher frequency would be nothing.
    tone: u32,
    /// G1's warm grade (`--grade`, batch 95): 0 is the pre-95 presentation. Spends the
    /// first padding word, so the uniform's size does not move.
    grade: u32,
    /// Batch 101's `--grade-strength` dial: lerps the graded pixel back toward the
    /// pre-grade one. Spends the last padding word -- the uniform is still 16 bytes.
    /// 1.0 is bit-exact with the batch-97 grade (mix at 1.0 returns its second
    /// argument exactly), so no control pixel moves by construction.
    strength: f32,
}

pub struct BlitPass {
    pipeline: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
    /// One per possible source: `resolve`'s output, then the two temporal history buffers.
    /// Which one is on screen alternates with the frame parity, and rebuilding a bind group
    /// every frame to say so would be work for nothing.
    bind_groups: Vec<wgpu::BindGroup>,
    sampler: wgpu::Sampler,
    uniform: wgpu::Buffer,
}

impl BlitPass {
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        format: wgpu::TextureFormat,
        tone: u32,
        grade: u32,
        strength: f32,
        srcs: &[&wgpu::TextureView],
    ) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("blit"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/blit.wgsl").into()),
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("blit sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let uniform = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("blit uniform"),
            size: 16,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        queue.write_buffer(
            &uniform,
            0,
            bytemuck::bytes_of(&BlitUniform {
                encode: (format.add_srgb_suffix() != format) as u32,
                tone,
                grade,
                strength,
            }),
        );
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("blit bgl"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("blit layout"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("blit"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs"),
                compilation_options: Default::default(),
                buffers: &[],
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
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });
        let bind_groups = srcs
            .iter()
            .map(|s| make_bg(device, &layout, s, &sampler, &uniform))
            .collect();
        Self {
            pipeline,
            layout,
            bind_groups,
            sampler,
            uniform,
        }
    }

    pub fn set_presentation(&self, queue: &wgpu::Queue, format: wgpu::TextureFormat,
                            tone: u32, grade: u32, strength: f32) {
        queue.write_buffer(&self.uniform, 0, bytemuck::bytes_of(&BlitUniform {
            encode: (format.add_srgb_suffix() != format) as u32,
            tone, grade, strength,
        }));
    }

    pub fn set_sources(&mut self, device: &wgpu::Device, srcs: &[&wgpu::TextureView]) {
        self.bind_groups = srcs
            .iter()
            .map(|s| make_bg(device, &self.layout, s, &self.sampler, &self.uniform))
            .collect();
    }

    pub fn draw(&self, pass: &mut wgpu::RenderPass<'_>, source: usize) {
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &self.bind_groups[source], &[]);
        pass.draw(0..3, 0..1);
    }
}

fn make_bg(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    src: &wgpu::TextureView,
    sampler: &wgpu::Sampler,
    uniform: &wgpu::Buffer,
) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("blit bg"),
        layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(src),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::Sampler(sampler),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: uniform.as_entire_binding(),
            },
        ],
    })
}



