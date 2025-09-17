use crate::map::RectCoord;

/// Helper to index into a dense rectangular grid store
#[derive(Clone)]
pub struct RectDenseIndexer {
    width: u32,
    height: u32,
}

impl RectDenseIndexer {
    /// Create a new HexRowStart for a given radius
    pub fn new(width: u32, height: u32) -> Self {
        Self { width, height }
    }

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }

    /// Get the total size needed for a rectangular grid of given dimensions
    pub fn get_total_size(&self) -> usize {
        self.width() as usize * self.height() as usize
    }

    /// Return the dense store index for a given RectCoord
    pub fn get_dense_index(&self, coord: &RectCoord) -> usize {
        let idx = (coord.y * (self.width as i32) + coord.x) as usize;
        debug_assert!(idx < self.get_total_size());
        idx
    }
}
