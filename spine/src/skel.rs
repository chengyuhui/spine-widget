use std::{
    ffi::{CStr, CString},
    fmt::{Debug, Formatter},
    marker::PhantomData,
    slice,
    sync::Arc,
};

use anyhow::{bail, Result};
use spine_sys::{
    spAnimationState_apply, spBlendMode, spBlendMode_SP_BLEND_MODE_ADDITIVE,
    spBlendMode_SP_BLEND_MODE_MULTIPLY, spBlendMode_SP_BLEND_MODE_NORMAL,
    spBlendMode_SP_BLEND_MODE_SCREEN, spSkeleton, spSkeletonBinary_create,
    spSkeletonBinary_dispose, spSkeletonBinary_readSkeletonDataFile, spSkeletonData,
    spSkeletonData_dispose, spSkeleton_create, spSkeleton_dispose, spSkeleton_setAttachment,
    spSkeleton_updateWorldTransform, spSlot, spSlotData,
};

use crate::{AnimationState, Atlas, Attachment, anim::Animation};

#[derive(Debug)]
pub(crate) struct SkelDataPtr(pub(crate) *mut spSkeletonData);
impl Drop for SkelDataPtr {
    fn drop(&mut self) {
        log::info!("SkeletonData@{:x} dropped", self.0 as usize);
        unsafe { spSkeletonData_dispose(self.0) };
    }
}

#[derive(Debug, Clone)]
pub struct SkeletonData {
    pub(crate) ptr: Arc<SkelDataPtr>,
    _atlas: Atlas,
}

impl SkeletonData {
    pub fn new_binary(atlas: &Atlas, path: &str, scale: f32) -> Result<Self> {
        let path = CString::new(path).unwrap();

        let inner = unsafe {
            let binary = spSkeletonBinary_create(atlas.ptr.0);
            (*binary).scale = scale;

            let skel_data = spSkeletonBinary_readSkeletonDataFile(binary, path.as_ptr());
            if skel_data.is_null() {
                spSkeletonBinary_dispose(binary);
                bail!(
                    "Failed to create skeleton data from file: {:?}",
                    CStr::from_ptr((*binary).error)
                );
            }
            // Dispose the spSkeletonBinary as we no longer need it after loading.
            spSkeletonBinary_dispose(binary);

            skel_data
        };

        Ok(SkeletonData {
            ptr: Arc::new(SkelDataPtr(inner)),
            _atlas: atlas.clone(),
        })
    }

    pub fn width(&self) -> f32 {
        unsafe { (*self.ptr.0).width }
    }

    pub fn height(&self) -> f32 {
        unsafe { (*self.ptr.0).height }
    }

    pub fn slots(&self) -> &[&SlotData] {
        unsafe {
            let slots = (*self.ptr.0).slots as *mut &SlotData;
            let len = (*self.ptr.0).slotsCount as usize;
            slice::from_raw_parts(slots, len)
        }
    }

    pub fn animations(&self) -> &[&Animation] {
        unsafe {
            let animations = (*self.ptr.0).animations as *mut &Animation;
            let len = (*self.ptr.0).animationsCount as usize;
            slice::from_raw_parts(animations, len)
        }
    }
}

#[repr(C)]
pub struct SlotData<'d> {
    inner: spSlotData,
    _skel_data: PhantomData<&'d SkeletonData>,
}

impl<'d> SlotData<'d> {
    pub fn name(&self) -> &str {
        unsafe { CStr::from_ptr(self.inner.name).to_str().unwrap() }
    }

    pub fn attachment_name(&self) -> &str {
        unsafe { CStr::from_ptr(self.inner.attachmentName).to_str().unwrap() }
    }
}

impl<'d> Debug for SlotData<'d> {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        f.debug_struct("SlotData")
            .field("name", &self.name())
            .field("attachment_name", &self.attachment_name())
            .finish()
    }
}

#[derive(Debug)]
pub struct Skeleton {
    ptr: *mut spSkeleton,
    _data: SkeletonData,
}

impl Skeleton {
    pub fn new(skel_data: &SkeletonData) -> Result<Self> {
        let inner = unsafe { spSkeleton_create(skel_data.ptr.0) };
        if inner.is_null() {
            bail!("Failed to create skeleton");
        }

        Ok(Skeleton {
            ptr: inner,
            _data: skel_data.clone(),
        })
    }

    pub fn set_x(&mut self, x: f32) {
        unsafe {
            (*self.ptr).x = x;
        }
    }

    pub fn set_y(&mut self, y: f32) {
        unsafe {
            (*self.ptr).y = y;
        }
    }

    /// Apply animation state to the skeleton.
    pub fn apply_animation(&mut self, anim: &AnimationState) {
        unsafe { spAnimationState_apply(anim.ptr, self.ptr) }
    }

    /// Calculate world transforms for rendering
    pub fn update_world_transform(&mut self) {
        unsafe { spSkeleton_updateWorldTransform(self.ptr) }
    }

    pub fn set_flip_x(&mut self, flip: bool) {
        unsafe {
            (*self.ptr).flipX = if flip { 1 } else { 0 };
        }
    }

    pub fn set_flip_y(&mut self, flip: bool) {
        unsafe {
            (*self.ptr).flipY = if flip { 1 } else { 0 };
        }
    }

    pub fn tint_color(&self) -> [f32; 4] {
        unsafe {
            let this = *self.ptr;
            [this.r, this.g, this.b, this.a]
        }
    }

    pub fn set_attachment(&mut self, slot: &str, attachment: &str) {
        let slot = CString::new(slot).unwrap();
        let attachment = CString::new(attachment).unwrap();

        unsafe {
            spSkeleton_setAttachment(self.ptr, slot.as_ptr(), attachment.as_ptr());
        }
    }

    pub fn slots(&self) -> &[&Slot] {
        unsafe {
            let this = *self.ptr;
            let slots = this.drawOrder as *mut &Slot;
            let len = this.slotsCount as usize;
            slice::from_raw_parts(slots, len)
        }
    }
}

impl Drop for Skeleton {
    fn drop(&mut self) {
        unsafe { spSkeleton_dispose(self.ptr) };
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum BlendMode {
    Normal,
    Additive,
    Multiply,
    Screen,
}

impl From<spBlendMode> for BlendMode {
    fn from(mode: spBlendMode) -> Self {
        #[allow(non_upper_case_globals)]
        match mode {
            spBlendMode_SP_BLEND_MODE_NORMAL => BlendMode::Normal,
            spBlendMode_SP_BLEND_MODE_ADDITIVE => BlendMode::Additive,
            spBlendMode_SP_BLEND_MODE_MULTIPLY => BlendMode::Multiply,
            spBlendMode_SP_BLEND_MODE_SCREEN => BlendMode::Screen,
            _ => BlendMode::Normal,
        }
    }
}

#[repr(C)]
#[derive(Debug)]
pub struct Slot<'sk> {
    pub(crate) inner: spSlot,
    skel: PhantomData<&'sk Skeleton>,
}

impl<'sk> Slot<'sk> {
    pub fn blend_mode(&self) -> BlendMode {
        unsafe { BlendMode::from((*self.inner.data).blendMode) }
    }

    pub fn tint_color(&self) -> [f32; 4] {
        let this = &self.inner;
        [this.r, this.g, this.b, this.a]
    }

    pub fn attachment(&self) -> Option<Attachment<'_, 'sk>> {
        if self.inner.attachment.is_null() {
            None
        } else {
            Some(Attachment::new(self.inner.attachment, self))
        }
    }
}
