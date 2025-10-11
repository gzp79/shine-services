use crate::map::AxialCoord;

// todo: Convert HexDenseIndexer to Interned

/// Helper to index into a dense hexagonal grid store
#[derive(Clone)]
pub struct HexDenseIndexer {
    radius: u32,
    row_starts: Vec<usize>,
}

impl HexDenseIndexer {
    /// Create a new HexRowStart for a given radius
    pub fn new(radius: u32) -> Self {
        let diameter = radius * 2 + 1;
        let mut row_starts = Vec::with_capacity(diameter as usize);
        let mut current_start = 0;
        let mut current_width = (radius + 1) as usize;

        // Calculate start indices for each row
        for r in -(radius as i32)..=radius as i32 {
            row_starts.push(current_start);
            current_start += current_width;
            if r < 0 {
                current_width += 1;
            } else {
                current_width -= 1;
            }
        }
        row_starts.push(current_start);

        Self { radius, row_starts }
    }

    pub fn radius(&self) -> u32 {
        self.radius
    }

    /// Get the total size needed for a hexagonal grid of given radius
    pub fn get_total_size(&self) -> usize {
        *self.row_starts.last().unwrap()
    }

    /// Return the dense store index for a given radius and AxialCoord
    pub fn get_dense_index(&self, coord: &AxialCoord) -> usize {
        let r = self.radius as i32;
        let (a, b) = (coord.r + r, coord.q + r);
        let row = a;
        let col = b - (r - a).max(0);
        let row_start = self.row_starts[row as usize];
        let idx = row_start + col as usize;
        debug_assert!(idx < self.get_total_size());
        idx
    }

    /// Return the AxialCoord for a given dense store index
    pub fn get_coord(&self, index: usize) -> AxialCoord {
        debug_assert!(index < self.get_total_size());

        // Find the row such that row_starts[row] <= index < row_starts[row+1]
        let row = match self.row_starts.binary_search(&index) {
            Ok(pos) => pos,
            Err(pos) => pos - 1,
        };

        let row_start = self.row_starts[row];
        let col = index - row_start;
        let r = self.radius as i32;
        let a = row as i32;
        let b = col as i32 + (r - a).max(0);
        AxialCoord { r: a - r, q: b - r }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use itertools::assert_equal;
    use shine_test::test;

    fn test_dense_indices(radius: u32) {
        let indexer = HexDenseIndexer::new(radius);

        // Collect all coordinates in spiral order
        let center = AxialCoord::new(0, 0);
        let coords: Vec<_> = center.spiral(radius).collect();

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
        // test for both even and odd radii
        test_dense_indices(31);
        test_dense_indices(32);
    }
}
