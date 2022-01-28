use wgpu::util::DeviceExt;
use winit::{dpi::PhysicalSize, window::Window};

use crate::config::Config;

#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct ScalingUniform {
    window_width: f32,
    window_height: f32,
    scale: f32,
    bottom_offset: f32,
}

#[derive(Debug)]
pub struct ScalingState {
    uniform: ScalingUniform,
    uniform_dirty: bool,
    buffer: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    dpi_scale_factor: f64,
}

impl ScalingState {
    pub fn new(
        window: &Window,
        device: &wgpu::Device,
        config: &Config,
    ) -> (Self, wgpu::BindGroupLayout) {
        let scaling_uniform = {
            let window_logical_size = window.inner_size().to_logical::<f32>(window.scale_factor());
            ScalingUniform {
                window_width: window_logical_size.width,
                window_height: window_logical_size.height,
                scale: config.scale,
                bottom_offset: config.bottom_offset,
            }
        };

        let scaling_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Scaling Buffer"),
            contents: bytemuck::cast_slice(&[scaling_uniform]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let scaling_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
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
                label: Some("scaling_bind_group_layout"),
            });

        let scaling_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &scaling_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: scaling_buffer.as_entire_binding(),
            }],
            label: Some("scaling_bind_group"),
        });

        (
            Self {
                uniform: scaling_uniform,
                uniform_dirty: false,
                buffer: scaling_buffer,
                bind_group: scaling_bind_group,
                dpi_scale_factor: window.scale_factor(),
            },
            scaling_bind_group_layout,
        )
    }

    pub fn resize(&mut self, size: PhysicalSize<u32>, scale_factor: f64) {
        let window_logical_size = size.to_logical::<f32>(scale_factor);
        self.uniform.window_width = window_logical_size.width;
        self.uniform.window_height = window_logical_size.height;
        self.uniform_dirty = true;
    }

    pub fn model_scaling(&self) -> f32 {
        self.uniform.scale
    }

    /// This also marks the uniform data as dirty, regardless of whether it actually changed.
    pub fn model_scaling_mut(&mut self) -> &mut f32 {
        self.uniform_dirty = true;
        &mut self.uniform.scale
    }

    /// Write the current uniform data to GPU if needed.
    pub fn write_to_gpu(&self, queue: &wgpu::Queue) {
        if self.uniform_dirty {
            queue.write_buffer(&self.buffer, 0, bytemuck::cast_slice(&[self.uniform]));
        }
    }

    pub fn bind_group(&self) -> &wgpu::BindGroup {
        &self.bind_group
    }
}
