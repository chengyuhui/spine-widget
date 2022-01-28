use std::collections::HashMap;

use anyhow::Result;
use wgpu::IndexFormat;
use winit::window::Window;

mod display;
mod scaling;
mod texture;

pub use texture::HardwareTexture;

use crate::{
    buffer::ScratchBuffers,
    config::Config,
    renderer::{texture::TextureID, Renderer},
    vertex::Vertex,
};

pub struct HardwareRenderer {
    display: display::Display,
    scaling: scaling::ScalingState,

    render_pipeline: wgpu::RenderPipeline,
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,

    texture_bind_group_layout: wgpu::BindGroupLayout,
    textures: HashMap<TextureID, HardwareTexture>,
}

impl HardwareRenderer {
    pub async fn new(window: &Window, config: &Config) -> Result<Self> {
        let display = display::Display::new(window).await;
        let device = &display.device;

        let texture_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            multisampled: false,
                            view_dimension: wgpu::TextureViewDimension::D2,
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
                label: Some("texture_bind_group_layout"),
            });

        let shader = device.create_shader_module(&wgpu::ShaderModuleDescriptor {
            label: Some("Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shader.wgsl").into()),
        });

        let (scaling, scaling_bind_group_layout) =
            scaling::ScalingState::new(&window, device, config);

        let render_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Render Pipeline Layout"),
                bind_group_layouts: &[&texture_bind_group_layout, &scaling_bind_group_layout],
                push_constant_ranges: &[],
            });

        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Render Pipeline"),
            layout: Some(&render_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "main_v",
                buffers: &[Vertex::desc()],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "main_f",
                targets: &[wgpu::ColorTargetState {
                    format: display.config.format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                }],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList, // Three vertices -> triangle
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw, // 2.
                cull_mode: None,
                // Setting this to anything other than Fill requires Features::NON_FILL_POLYGON_MODE
                polygon_mode: wgpu::PolygonMode::Fill,
                // Requires Features::CONSERVATIVE_RASTERIZATION
                conservative: false,
                unclipped_depth: false,
            },
            depth_stencil: None, // No depth/stencil buffer.
            multisample: wgpu::MultisampleState {
                count: 1,                         // 2.
                mask: !0,                         // All of them.
                alpha_to_coverage_enabled: false, // No anti-aliasing for now.
            },
            multiview: None,
        });

        let vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Vertex Buffer"),
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            size: 1024 * 128,
            mapped_at_creation: false,
        });

        let index_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Index Buffer"),
            usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
            size: 1024 * 128,
            mapped_at_creation: false,
        });

        Ok(Self {
            display,
            scaling,
            render_pipeline,
            vertex_buffer,
            index_buffer,
            texture_bind_group_layout,
            textures: HashMap::new(),
        })
    }
}

impl Renderer for HardwareRenderer {
    fn resize(&mut self, size: winit::dpi::PhysicalSize<u32>, scale_factor: f64) {
        self.display.resize(size.width, size.height);
        self.scaling.resize(size, scale_factor);
    }

    fn update(&mut self) {
        self.scaling.write_to_gpu(&self.display.queue);
    }

    fn register_texture(&mut self, texture: &crate::renderer::Texture) {
        let id = texture.id();
        if self.textures.contains_key(&id) {
            return;
        }

        let hw_texture = HardwareTexture::from_image(
            &self.display.device,
            &self.display.queue,
            &self.texture_bind_group_layout,
            texture.image(),
            texture.config(),
            None,
        );

        self.textures.insert(id, hw_texture);
    }

    fn render(&mut self, buffers: &mut ScratchBuffers) -> Result<()> {
        let queue = &self.display.queue;

        let output = self.display.surface.get_current_texture()?;
        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut cleared = false;

        for (tex_id, vb, ib) in buffers.iter_mut() {
            {
                let len = vb.len();
                let vb_pad = len % 4;
                if vb_pad != 0 {
                    vb.resize(vb.len() + 4 - vb_pad, Default::default());
                }
            }
            let ib_len = {
                let len = ib.len();
                let ib_pad = len % 4;
                if ib_pad != 0 {
                    ib.resize(ib.len() + 4 - ib_pad, 0);
                }
                len
            };

            queue.write_buffer(&self.vertex_buffer, 0, bytemuck::cast_slice(vb));
            queue.write_buffer(&self.index_buffer, 0, bytemuck::cast_slice(ib));

            let mut encoder =
                self.display
                    .device
                    .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                        label: Some("Render Encoder"),
                    });

            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Render Pass"),
                color_attachments: &[wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: if cleared {
                            wgpu::LoadOp::Load
                        } else {
                            cleared = true;
                            wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT)
                        },
                        store: true,
                    },
                }],
                depth_stencil_attachment: None,
            });

            render_pass.set_pipeline(&self.render_pipeline);
            render_pass.set_bind_group(
                0,
                &self.textures.get(&tex_id).unwrap().bind_group,
                &[],
            );
            render_pass.set_bind_group(1, self.scaling.bind_group(), &[]);
            render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
            render_pass.set_index_buffer(self.index_buffer.slice(..), IndexFormat::Uint16);

            render_pass.draw_indexed(0..ib_len as u32, 0, 0..1);

            drop(render_pass);
            queue.submit(std::iter::once(encoder.finish()));
        }

        output.present();

        Ok(())
    }
}
