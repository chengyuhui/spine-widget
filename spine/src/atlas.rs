use std::{ffi::{CString, CStr}, marker::PhantomData, ptr::null_mut, slice, sync::Arc};

use anyhow::{bail, Result};
use spine_sys::{spAtlas, spAtlasPage, spAtlasRegion, spAtlas_createFromFile, spAtlas_dispose};

#[derive(Debug)]
pub(crate) struct AtlasPtr(pub(crate) *mut spAtlas);
impl Drop for AtlasPtr {
    fn drop(&mut self) {
        log::info!("Atlas@{:x} dropped", self.0 as usize);
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

    // pub fn regions(&self) -> &[AtlasRegion] {
    //     unsafe {
    //         let regions = (*self.ptr.0).regions as *mut AtlasRegion;
    //         let mut count = 0;
    //         while !regions.offset(count).is_null() {
    //             count += 1;
    //         }
    //         slice::from_raw_parts(regions, count as usize)
    //     }
    // }

    pub fn first_region(&self) -> Option<&AtlasRegion> {
        unsafe { ((*self.ptr.0).regions as *const AtlasRegion).as_ref() }
    }

    pub fn pages(&self) -> &[AtlasPage] {
        unsafe {
            let pages = (*self.ptr.0).pages as *mut AtlasPage;
            let mut count = 0;
            while !pages.offset(count).is_null() {
                count += 1;
            }
            slice::from_raw_parts(pages, count as usize)
        }
    }
}

#[repr(C)]
#[derive(Debug)]
pub struct AtlasRegion {
    pub(crate) inner: spAtlasRegion,
}

impl AtlasRegion {
    pub fn name(&self) -> &str {
        unsafe { CStr::from_ptr(self.inner.name).to_str().unwrap() }
    }

    #[inline]
    pub fn page(&self) -> &AtlasPage {
        unsafe { &*(self.inner.page as *const AtlasPage) }
    }

    pub fn x(&self) -> u32 {
        self.inner.x as u32
    }

    pub fn y(&self) -> u32 {
        self.inner.y as u32
    }

    pub fn width(&self) -> u32 {
        self.inner.width as u32
    }

    pub fn height(&self) -> u32 {
        self.inner.height as u32
    }

    pub fn u(&self) -> f32 {
        self.inner.u as f32
    }

    pub fn v(&self) -> f32 {
        self.inner.v as f32
    }

    pub fn u2(&self) -> f32 {
        self.inner.u2 as f32
    }

    pub fn v2(&self) -> f32 {
        self.inner.v2 as f32
    }

    pub fn offset_x(&self) -> f32 {
        self.inner.offsetX as f32
    }

    pub fn offset_y(&self) -> f32 {
        self.inner.offsetY as f32
    }

    pub fn original_width(&self) -> u32 {
        self.inner.originalWidth as u32
    }

    pub fn original_height(&self) -> u32 {
        self.inner.originalHeight as u32
    }

    pub fn index(&self) -> u32 {
        self.inner.index as u32
    }

    pub fn rotated(&self) -> bool {
        self.inner.rotate != 0
    }

    pub fn flipped(&self) -> bool {
        self.inner.flip != 0
    }

    pub fn next_region(&self) -> Option<&'static AtlasRegion> {
        unsafe { (self.inner.next as *const AtlasRegion).as_ref() }
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
        unsafe { std::mem::transmute(self.inner.magFilter) }
    }

    pub fn min_filter(&self) -> AtlasFilter {
        unsafe { std::mem::transmute(self.inner.minFilter) }
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
