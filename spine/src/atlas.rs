use std::{ffi::CString, ptr::null_mut, sync::Arc};

use anyhow::{bail, Result};
use spine_sys::{spAtlas, spAtlasPage, spAtlasRegion, spAtlas_createFromFile, spAtlas_dispose};

#[derive(Debug)]
pub(crate) struct AtlasPtr(pub(crate) *mut spAtlas);
impl Drop for AtlasPtr {
    fn drop(&mut self) {
        unsafe { spAtlas_dispose(self.0) };
    }
}

#[derive(Debug, Clone)]
pub struct Atlas {
    pub(crate) ptr: Arc<AtlasPtr>,
}

impl Atlas {
    pub fn new(path: &str) -> Result<Self> {
        let c_str = CString::new(path).unwrap();

        let inner = unsafe { spAtlas_createFromFile(c_str.as_ptr(), null_mut()) };
        if inner.is_null() {
            bail!("Failed to create atlas from file: {}", path);
        }

        Ok(Atlas {
            ptr: Arc::new(AtlasPtr(inner)),
        })
    }
}

#[repr(C)]
#[derive(Debug)]
pub struct AtlasRegion {
    pub(crate) inner: spAtlasRegion,
}

impl AtlasRegion {
    #[inline]
    pub fn page(&self) -> &AtlasPage {
        unsafe { &*(self.inner.page as *const AtlasPage) }
    }
}

#[repr(C)]
#[derive(Debug)]
pub struct AtlasPage {
    pub(crate) inner: spAtlasPage,
}

impl AtlasPage {
    /// Retrieve the texture object returned in [`crate::SpineCallbacks`].
    /// 
    /// # Safety
    /// This is unsafe if the type given does not match the type actually put as texture.
    #[inline]
    pub unsafe fn render_object<T>(&self) -> Option<&mut T> {
        (self.inner.rendererObject as *mut T).as_mut()
    }

    pub fn mag_filter(&self) -> AtlasFilter {
        unsafe { std::mem::transmute(self.inner.magFilter)}
    }

    pub fn min_filter(&self) -> AtlasFilter {
        unsafe { std::mem::transmute(self.inner.minFilter)}
    }

    pub fn u_wrap(&self) -> AtlasWrap {
        unsafe { std::mem::transmute(self.inner.uWrap) }
    }

    pub fn v_wrap(&self) -> AtlasWrap {
        unsafe { std::mem::transmute(self.inner.vWrap) }
    }

    pub fn width(&self) -> u32 {
        self.inner.width as u32
    }

    pub fn height(&self) -> u32 {
        self.inner.height as u32
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[repr(i32)]
pub enum AtlasFilter {
    Unknown = 0,
    Nearest = 1,
    Linear = 2,
    Mipmap = 3,
    MipmapNearestNearest = 4,
    MipmapLinearNearest = 5,
    MipmapNearestLinear = 6,
    MipmapLinearLinear = 7,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[repr(i32)]
pub enum AtlasWrap {
    MirroredRepeat = 0,
    ClampToEdge = 1,
    Repeat = 2,
}
