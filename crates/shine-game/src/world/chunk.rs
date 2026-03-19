use crate::math::hex::AxialCoord;

/// Unique identifier of a chunk of the map.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct ChunkId(pub usize, pub usize);

impl ChunkId {
    /// Return the relative axial coordinate of a chunk id.
    /// This function interprets the chunk coordinates as the q,r components of the axial coordinates.
    pub fn relative_axial_coord(&self, id: ChunkId) -> AxialCoord {
        let dx = id.0 as isize - self.0 as isize;
        let dy = id.1 as isize - self.1 as isize;
        AxialCoord::new(dx.try_into().unwrap(), dy.try_into().unwrap())
    }
}
