use crate::{texture::TextureID, vertex::Vertex};

pub struct ScratchBuffers {
    index: usize,
    vertex_buffers: Vec<(TextureID, Vec<Vertex>)>,
    index_buffers: Vec<(TextureID, Vec<u16>)>,
}

impl ScratchBuffers {
    pub fn new() -> Self {
        Self {
            index: 0,
            vertex_buffers: Vec::new(),
            index_buffers: Vec::new(),
        }
    }

    pub fn clear(&mut self) {
        self.vertex_buffers.iter_mut().for_each(|(_, v)| v.clear());
        self.index_buffers.iter_mut().for_each(|(_, v)| v.clear());
        self.index = 0;
    }

    /// Get the latest buffer available to `tex_id`, create a new one if the texture ID has
    /// changed since the last call.
    pub fn get_buffers_mut(&mut self, tex_id: TextureID) -> (&mut Vec<Vertex>, &mut Vec<u16>) {
        let vb_last = self.vertex_buffers.get_mut(self.index);
        let ib_last = self.index_buffers.get_mut(self.index);

        match (vb_last, ib_last) {
            (Some((vb_id, _)), Some((ib_id, _))) => {
                debug_assert_eq!(vb_id, ib_id);

                if *vb_id != tex_id {
                    self.index += 1;

                    let vb_next = self.vertex_buffers.get_mut(self.index);
                    let ib_next = self.index_buffers.get_mut(self.index);

                    match (vb_next, ib_next) {
                        (Some((vb_next_id, vb_next)), Some((ib_next_id, ib_next))) => {
                            debug_assert!(vb_next.is_empty() && ib_next.is_empty());
                            *vb_next_id = tex_id;
                            *ib_next_id = tex_id;
                        }
                        (None, None) => {
                            self.vertex_buffers.push((tex_id, Vec::new()));
                            self.index_buffers.push((tex_id, Vec::new()));
                        }
                        _ => panic!(),
                    }
                }
            }
            (None, None) => {
                // Create new buffers
                self.vertex_buffers.push((tex_id, Vec::new()));
                self.index_buffers.push((tex_id, Vec::new()));
            }
            _ => panic!(),
        }

        (
            &mut self.vertex_buffers[self.index].1,
            &mut self.index_buffers[self.index].1,
        )
    }

    pub fn iter_mut(
        &mut self,
    ) -> impl Iterator<Item = (TextureID, &mut Vec<Vertex>, &mut Vec<u16>)> {
        self.vertex_buffers
            .iter_mut()
            .zip(self.index_buffers.iter_mut())
            .filter_map(|((tex_id_v, vb), (tex_id_i, ib))| {
                debug_assert_eq!(tex_id_v, tex_id_i);

                if vb.is_empty() || ib.is_empty() {
                    None
                } else {
                    Some((*tex_id_v, vb, ib))
                }
            })
    }
}
