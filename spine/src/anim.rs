use std::{
    ffi::{CStr, CString},
    fmt::Debug,
    os::raw::c_int,
    sync::Arc,
};

use anyhow::{bail, Result};
use spine_sys::{
    spAnimation, spAnimationState, spAnimationStateData, spAnimationStateData_create,
    spAnimationStateData_dispose, spAnimationState_addAnimationByName,
    spAnimationState_addEmptyAnimation, spAnimationState_clearTrack, spAnimationState_clearTracks,
    spAnimationState_create, spAnimationState_dispose, spAnimationState_setAnimationByName,
    spAnimationState_setEmptyAnimation, spAnimationState_update,
};

use crate::SkeletonData;

#[derive(Debug)]
struct AnimStateDataPtr(*mut spAnimationStateData);
impl Drop for AnimStateDataPtr {
    fn drop(&mut self) {
        unsafe { spAnimationStateData_dispose(self.0) };
    }
}

#[derive(Debug, Clone)]
pub struct AnimationStateData {
    ptr: Arc<AnimStateDataPtr>,
    _skel_data: SkeletonData,
}

impl AnimationStateData {
    pub fn new(skel_data: &SkeletonData, default_mix: f32) -> Result<Self> {
        let inner = unsafe { spAnimationStateData_create(skel_data.ptr.0) };
        if inner.is_null() {
            bail!("Failed to create animation state data");
        }

        unsafe {
            (*inner).defaultMix = default_mix;
        }

        Ok(AnimationStateData {
            ptr: Arc::new(AnimStateDataPtr(inner)),
            _skel_data: skel_data.clone(),
        })
    }
}

pub struct AnimationState {
    pub(crate) ptr: *mut spAnimationState,
    _data: AnimationStateData,
}

impl AnimationState {
    pub fn new(anim_state_data: &AnimationStateData) -> Result<Self> {
        let inner = unsafe { spAnimationState_create(anim_state_data.ptr.0) };
        if inner.is_null() {
            bail!("Failed to create animation state");
        }

        Ok(AnimationState {
            ptr: inner,
            _data: anim_state_data.clone(),
        })
    }

    /// Update the animation state by time delta.
    pub fn update(&mut self, delta: f32) {
        unsafe {
            spAnimationState_update(self.ptr, delta);
        }
    }

    pub fn set_animation_by_name(&mut self, track_index: usize, name: &str, loop_: bool) {
        let c_str = CString::new(name).unwrap();
        unsafe {
            spAnimationState_setAnimationByName(
                self.ptr,
                track_index as c_int,
                c_str.as_ptr(),
                if loop_ { 1 } else { 0 },
            );
        }
    }

    pub fn add_animation_by_name(
        &mut self,
        track_index: usize,
        name: &str,
        loop_: bool,
        delay: f32,
    ) {
        let c_str = CString::new(name).unwrap();
        unsafe {
            spAnimationState_addAnimationByName(
                self.ptr,
                track_index as c_int,
                c_str.as_ptr(),
                if loop_ { 1 } else { 0 },
                delay,
            );
        }
    }

    pub fn set_empty_animation(&self, track_index: usize, mix_duration: f32) {
        unsafe {
            spAnimationState_setEmptyAnimation(self.ptr, track_index as c_int, mix_duration);
        }
    }

    pub fn add_empty_animation(&self, track_index: usize, mix_duration: f32, delay: f32) {
        unsafe {
            spAnimationState_addEmptyAnimation(self.ptr, track_index as c_int, mix_duration, delay);
        }
    }

    pub fn clear_tracks(&mut self) {
        unsafe { spAnimationState_clearTracks(self.ptr) }
    }

    pub fn clear_track(&mut self, track_index: usize) {
        unsafe { spAnimationState_clearTrack(self.ptr, track_index as c_int) }
    }
}

impl Drop for AnimationState {
    fn drop(&mut self) {
        unsafe { spAnimationState_dispose(self.ptr) };
    }
}

#[repr(C)]
pub struct Animation {
    pub(crate) inner: spAnimation,
}

impl Animation {
    pub fn name(&self) -> &str {
        unsafe { CStr::from_ptr(self.inner.name).to_str().unwrap() }
    }

    pub fn duration(&self) -> f32 {
        self.inner.duration
    }
}

impl Debug for Animation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Animation")
            .field("name", &self.name())
            .field("duration", &self.duration())
            .finish()
    }
}
