use anyhow::Result;
use winit::dpi::PhysicalSize;

pub mod backend;

pub mod texture;
pub use texture::Texture;

use crate::buffer::ScratchBuffers;

pub trait Renderer {
    fn resize(&mut self, size: PhysicalSize<u32>, scale_factor: f64);
    fn update(&mut self);
    fn register_texture(&mut self, texture: &Texture);
    fn render(&mut self, buffers: &mut ScratchBuffers) -> Result<()>;
}