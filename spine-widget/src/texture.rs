use std::sync::atomic::{AtomicU32, Ordering};

use anyhow::*;
use image::{DynamicImage, GenericImageView};
use spine::atlas::{AtlasFilter, AtlasWrap};
use wgpu::util::DeviceExt;

static TEX_ID: AtomicU32 = AtomicU32::new(0);

pub struct TextureConfig {
    pub mag_filter: AtlasFilter,
    pub min_filter: AtlasFilter,
    pub u_wrap: AtlasWrap,
    pub v_wrap: AtlasWrap,
}

pub struct Texture {
    id: u32,
    state: TextureState,
    config: TextureConfig,
}

enum TextureState {
    Uninitialized(DynamicImage),
    Initialized(InitializedTexture),
}

impl Texture {
    pub fn new(image: DynamicImage, config: TextureConfig) -> Self {
        Self {
            id: TEX_ID.fetch_add(1, Ordering::SeqCst),
            state: TextureState::Uninitialized(image),
            config,
        }
    }

    pub fn id(&self) -> u32 {
        self.id
    }

    pub fn initialize(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        layout: &wgpu::BindGroupLayout,
        label: Option<&str>,
    ) -> Result<()> {
        let image = match &self.state {
            TextureState::Uninitialized(image) => image,
            TextureState::Initialized(_) => return Ok(()),
        };

        let texture =
            InitializedTexture::from_image(device, queue, layout, image, &self.config, label)?;

        self.state = TextureState::Initialized(texture);

        Ok(())
    }

    pub fn get_texture(&self) -> &InitializedTexture {
        match &self.state {
            TextureState::Uninitialized(_) => panic!("Texture is not initialized"),
            TextureState::Initialized(texture) => texture,
        }
    }
}

pub struct InitializedTexture {
    pub texture: wgpu::Texture,
    pub view: wgpu::TextureView,
    pub sampler: wgpu::Sampler,
    pub bind_group: wgpu::BindGroup,
}

impl InitializedTexture {
    pub fn from_image(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        layout: &wgpu::BindGroupLayout,
        img: &image::DynamicImage,
        config: &TextureConfig,
        label: Option<&str>,
    ) -> Result<Self> {
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

        Ok(Self {
            texture,
            view,
            sampler,
            bind_group,
        })
    }
}
