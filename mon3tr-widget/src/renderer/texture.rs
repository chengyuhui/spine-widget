use std::sync::{atomic::{AtomicU32, Ordering}, Arc};

use image::DynamicImage;
use spine::atlas::{AtlasFilter, AtlasWrap};

// use super::backend::hardware::HardwareTexture;

static TEX_ID: AtomicU32 = AtomicU32::new(0);

pub struct TextureConfig {
    pub mag_filter: AtlasFilter,
    pub min_filter: AtlasFilter,
    pub u_wrap: AtlasWrap,
    pub v_wrap: AtlasWrap,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TextureID(u32);

pub struct Texture {
    id: TextureID,
    image: Arc<DynamicImage>,
    config: TextureConfig,
}

impl Texture {
    pub fn new(image: DynamicImage, config: TextureConfig) -> Self {
        Self {
            id: TextureID(TEX_ID.fetch_add(1, Ordering::Relaxed)),
            image: Arc::new(image),
            config,
        }
    }

    pub fn id(&self) -> TextureID {
        self.id
    }

    pub fn image(&self) -> Arc<DynamicImage> {
        Arc::clone(&self.image)
    }

    pub fn config(&self) -> &TextureConfig {
        &self.config
    }
}
