use crate::math::hex::PatchCoord;

/// Helper to index into a dense store
#[derive(Clone)]
pub struct PatchDenseIndexer {
    subdivision: u32,
    // the size of the grid on a patch
    grid_size: usize,
}

impl PatchDenseIndexer {
    /// Create a new index for a given subdivision depth
    pub fn new(subdivision: u32) -> Self {
        let grid_size = 2_usize.pow(subdivision);
        Self { subdivision, grid_size }
    }

    pub fn subdivision(&self) -> u32 {
        self.subdivision
    }

    /// Get the total size needed for a rectangular grid of given dimensions
    pub fn get_total_size(&self) -> usize {
        3 * self.grid_size * self.grid_size
    }

    /// Return the dense store index for a given PatchCoord
    pub fn get_dense_index(&self, coord: &PatchCoord) -> usize {
        let patch_size = self.grid_size * self.grid_size;
        let idx = coord.p as usize * patch_size + coord.u as usize * self.grid_size + coord.v as usize;
        debug_assert!(idx < self.get_total_size());
        idx
    }

    /// Return the PatchCoord for a given dense store index
    pub fn get_coord(&self, index: usize) -> PatchCoord {
        debug_assert!(index < self.get_total_size());
        let patch_size = self.grid_size * self.grid_size;
        let p = (index / patch_size) as i32;
        let remainder = index % patch_size;
        let u = (remainder / self.grid_size) as i32;
        let v = (remainder % self.grid_size) as i32;
        PatchCoord::new(p, u, v)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use itertools::assert_equal;
    use shine_test::test;

    fn test_dense_indices(subdivision: u32) {
        let indexer = PatchDenseIndexer::new(subdivision);
        let grid_size = 2_i32.pow(subdivision);

        // Collect all valid coordinates
        let coords: Vec<_> = (0..3)
            .flat_map(|p| (0..grid_size).flat_map(move |u| (0..grid_size).map(move |v| PatchCoord::new(p, u, v))))
            .collect();

        let total_size = indexer.get_total_size();
        assert_eq!(total_size, coords.len());

        // Get dense indices for all coordinates
        let mut indices: Vec<_> = coords.iter().map(|coord| indexer.get_dense_index(coord)).collect();
        indices.sort_unstable();

        // Check if indices are continuous from 0 to len-1
        assert_equal(indices.iter().cloned(), 0..total_size);

        // Check if conversion back to coordinates works correctly
        for coord in coords.iter() {
            let index = indexer.get_dense_index(coord);
            let coord_back = indexer.get_coord(index);
            assert_eq!(*coord, coord_back);
        }
    }

    #[test]
    fn test_dense_indices_0() {
        test_dense_indices(0);
    }

    #[test]
    fn test_dense_indices_1() {
        test_dense_indices(1);
    }

    #[test]
    fn test_dense_indices_2() {
        test_dense_indices(2);
    }

    #[test]
    fn test_dense_indices_3() {
        test_dense_indices(3);
    }

    #[test]
    fn test_dense_indices_big() {
        test_dense_indices(6);
        test_dense_indices(7);
    }
}
