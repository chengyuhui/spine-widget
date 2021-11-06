//! # Spine-rs
//! 
//! (Almost) safe binding to the `spine-c` runtime, used for 2D animation.
//! 


/// Atlas (texture) types
pub mod atlas;
pub use atlas::{Atlas, AtlasPage};

/// Animation types
pub mod anim;
pub use anim::{AnimationState, AnimationStateData};

/// Skeleton types
pub mod skel;
pub use skel::{BlendMode, Skeleton, SkeletonData, Slot};

/// Skeleton attachment types
pub mod attachment;
pub use attachment::{Attachment, AttachmentType};

/// Re-export of FFI bindings
pub use spine_sys as sys;

/// Callbacks used by Spine runtime to perform various tasks
pub trait SpineCallbacks {
    type Texture;
    type LoadError: AsRef<dyn std::error::Error + Send + Sync + 'static>;

    /// Load the texture from the given path, returns the texture and the size of the texture.
    /// The returned texture can later be retrieved by [`AtlasPage::render_object`].
    fn load_texture(
        path: &str,
        page: &AtlasPage,
    ) -> Result<(Self::Texture, u32, u32), Self::LoadError>;
}

/// Register callbacks to be used by Spine runtime,
/// you may encounter linking errors regarding `_spAtlasPage_createTexture`
/// and `_spAtlasPage_disposeTexture` without this.
#[macro_export]
macro_rules! spine_init {
    ($t: ty) => {
        #[allow(clippy::missing_safety_doc)]
        #[no_mangle]
        pub unsafe extern "C" fn _spAtlasPage_createTexture(
            this: *mut $crate::sys::spAtlasPage,
            path: *const std::os::raw::c_char,
        ) {
            let path = std::ffi::CStr::from_ptr(path).to_string_lossy();

            let page = (this as *const $crate::atlas::AtlasPage).as_ref().unwrap();

            let (obj, width, height) =
                match <$t as $crate::SpineCallbacks>::load_texture(path.as_ref(), page) {
                    Ok(v) => v,
                    Err(e) => {
                        eprintln!("{}", e);
                        return;
                    }
                };

            let this = this.as_mut().unwrap();

            this.width = width as std::os::raw::c_int;
            this.height = height as std::os::raw::c_int;
            this.rendererObject = Box::into_raw(Box::new(obj)) as *mut _;
        }

        #[allow(clippy::missing_safety_doc)]
        #[no_mangle]
        pub unsafe extern "C" fn _spAtlasPage_disposeTexture(this: *mut $crate::sys::spAtlasPage) {
            let this = this.as_mut().unwrap();

            if this.rendererObject.is_null() {
                return;
            }

            let tex =
                Box::from_raw(this.rendererObject as *mut <$t as $crate::SpineCallbacks>::Texture);
            drop(tex);

            this.rendererObject = std::ptr::null_mut();
        }
    };
}
