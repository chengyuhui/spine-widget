use std::{ffi::CStr, marker::PhantomData, slice};

use spine_sys::{spAttachment, spAttachmentType_SP_ATTACHMENT_MESH, spAttachmentType_SP_ATTACHMENT_PATH, spAttachmentType_SP_ATTACHMENT_REGION, spMeshAttachment, spMeshAttachment_computeWorldVertices, spRegionAttachment, spRegionAttachment_computeWorldVertices};

use crate::{atlas::AtlasRegion, Slot};

#[derive(Debug)]
pub enum AttachmentType<'s, 'tex> {
    Region(RegionAttachment<'s, 'tex>),
    // BoundingBox(BoundingBoxAttachment),
    Mesh(MeshAttachment<'s, 'tex>),
    // LinkedMesh(LinkedMeshAttachment),
    Path(PathAttachment),
}

#[derive(Debug)]
pub struct Attachment<'s, 'tex> {
    ptr: *mut spAttachment,
    slot: &'s Slot<'s>,
    _tex: PhantomData<&'tex ()>,
}

impl<'s, 'tex> Attachment<'s, 'tex> {
    pub(crate) fn new(ptr: *mut spAttachment, slot: &'s Slot) -> Self {
        Self {
            ptr,
            slot,
            _tex: PhantomData,
        }
    }

    pub fn as_inner(&self) -> AttachmentType<'s, 'tex> {
        unsafe {
            #[allow(non_upper_case_globals)]
            match (*self.ptr).type_ {
                spAttachmentType_SP_ATTACHMENT_REGION => AttachmentType::Region(RegionAttachment {
                    ptr: self.ptr as *mut _,
                    slot: self.slot,
                    _tex: PhantomData,
                }),
                spAttachmentType_SP_ATTACHMENT_MESH => AttachmentType::Mesh(MeshAttachment {
                    ptr: self.ptr as *mut _,
                    slot: self.slot,
                    _tex: PhantomData,
                }),
                spAttachmentType_SP_ATTACHMENT_PATH => AttachmentType::Path(PathAttachment),
                _ => unimplemented!("Unimplemented attachment type: {}", (*self.ptr).type_),
            }
        }
    }

    pub fn name(&self) -> &str {
        unsafe {
            let this = *self.ptr;
            CStr::from_ptr(this.name).to_str().unwrap()
        }
    }
}

#[derive(Debug)]
pub struct RegionAttachment<'s, 'tex> {
    ptr: *mut spRegionAttachment,
    slot: &'s Slot<'s>,
    _tex: PhantomData<&'tex ()>,
}

impl<'a, 'tex> RegionAttachment<'a, 'tex> {
    /// Number of world vertices in this mesh (2 f32 per vertex)
    #[inline]
    pub fn world_vertices_count(&self) -> usize {
        4
    }

    #[inline]
    pub fn atlas_region(&self) -> &'tex AtlasRegion {
        unsafe {
            let this = *self.ptr;
            &*(this.rendererObject as *const AtlasRegion)
        }
    }

    pub fn compute_world_vertices(&self, positions: &mut Vec<[f32; 2]>) {
        let count = self.world_vertices_count();

        if positions.len() < count {
            positions.reserve(count - positions.len());
        };

        unsafe {
            spRegionAttachment_computeWorldVertices(
                self.ptr,
                self.slot.inner.bone,
                positions.as_mut_ptr() as *mut _,
            );
            positions.set_len(count);
        }
    }

    /// Get the uniform UV value of the vertex at the given index.
    pub fn uv(&self, index: usize) -> (f32, f32) {
        assert!(index < self.world_vertices_count());

        unsafe {
            let this = *self.ptr;
            (this.uvs[index * 2], this.uvs[index * 2 + 1])
        }
    }
}

#[derive(Debug)]
pub struct MeshAttachment<'s, 'tex> {
    ptr: *mut spMeshAttachment,
    slot: &'s Slot<'s>,
    _tex: PhantomData<&'tex ()>,
}

impl<'a, 'tex> MeshAttachment<'a, 'tex> {
    /// Number of triangles in this mesh
    #[inline]
    pub fn triangles_count(&self) -> usize {
        unsafe { (*self.ptr).trianglesCount as usize }
    }

    /// Number of world vertices in this mesh (2 f32 per vertex)
    #[inline]
    pub fn world_vertices_count(&self) -> usize {
        unsafe { (*self.ptr).super_.worldVerticesLength as usize / 2 }
    }

    pub fn compute_world_vertices(&self, positions: &mut Vec<[f32; 2]>) {
        let count = self.world_vertices_count();

        if positions.len() < count {
            positions.reserve(count - positions.len());
        };

        unsafe {
            spMeshAttachment_computeWorldVertices(
                self.ptr,
                &self.slot.inner as *const _ as *mut _,
                positions.as_mut_ptr() as *mut _,
            );
            positions.set_len(count);
        }
    }

    /// Get the uniform UV value of the vertex at the given index.
    pub fn uv(&self, index: usize) -> (f32, f32) {
        assert!(index < self.world_vertices_count());

        unsafe {
            let this = *self.ptr;
            let uv = this.uvs.add(index * 2);
            (*uv, *uv.offset(1))
        }
    }

    #[inline]
    pub fn atlas_region(&self) -> &'tex AtlasRegion {
        unsafe {
            let this = *self.ptr;
            &*(this.rendererObject as *const AtlasRegion)
        }
    }

    #[inline]
    pub fn indices(&self) -> &[u16] {
        unsafe {
            let this = *self.ptr;
            slice::from_raw_parts(this.triangles, this.trianglesCount as usize)
        }
    }
}

#[derive(Debug)]
pub struct PathAttachment;
