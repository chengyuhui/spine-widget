use std::sync::{Arc, Weak};

use anyhow::Result;
use image::{DynamicImage, GenericImageView};
use spine::atlas::{AtlasFilter, AtlasWrap};
use wgpu::util::DeviceExt;

use crate::renderer::texture::TextureConfig;

pub struct HardwareTexture {
    pub texture: wgpu::Texture,
    pub view: wgpu::TextureView,
    pub sampler: wgpu::Sampler,
    pub bind_group: wgpu::BindGroup,
    pub image: Weak<DynamicImage>, // TODO: cleanup when image is dropped
}

impl HardwareTexture {
    pub fn from_image(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        layout: &wgpu::BindGroupLayout,
        img: Arc<DynamicImage>,
        config: &TextureConfig,
        label: Option<&str>,
    ) -> Self {
        let rgba = img.as_rgba8().unwrap();
        let dimensions = img.dimensions();

        let size = wgpu::Extent3d {
            width: dimensions.0,
            height: dimensions.1,
            depth_or_array_layers: 1,
        };
        let texture = device.create_texture_with_data(
            queue,
            &wgpu::TextureDescriptor {
                label,
                size,
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8UnormSrgb,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            },
            rgba,
        );

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: match config.u_wrap {
                AtlasWrap::MirroredRepeat => wgpu::AddressMode::MirrorRepeat,
                AtlasWrap::ClampToEdge => wgpu::AddressMode::ClampToEdge,
                AtlasWrap::Repeat => wgpu::AddressMode::Repeat,
            },
            address_mode_v: match config.v_wrap {
                AtlasWrap::MirroredRepeat => wgpu::AddressMode::MirrorRepeat,
                AtlasWrap::ClampToEdge => wgpu::AddressMode::ClampToEdge,
                AtlasWrap::Repeat => wgpu::AddressMode::Repeat,
            },
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: match config.mag_filter {
                AtlasFilter::Nearest => wgpu::FilterMode::Nearest,
                AtlasFilter::Linear => wgpu::FilterMode::Linear,
                _ => wgpu::FilterMode::Linear,
            },
            min_filter: match config.min_filter {
                AtlasFilter::Nearest => wgpu::FilterMode::Nearest,
                AtlasFilter::Linear => wgpu::FilterMode::Linear,
                _ => wgpu::FilterMode::Linear,
            },
            mipmap_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
            label: Some("cartoon_bind_group"),
        });

        Self {
            texture,
            view,
            sampler,
            bind_group,
            image: Arc::downgrade(&img),
        }
    }

    pub fn should_gc(&self) -> bool {
        self.image.upgrade().is_none()
    }
}
