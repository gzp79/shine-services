use crate::world::generation::{Generation, GenerationGuard};

crate::define_enum_index! {
    /// Which side of an EdgeCells polygon a tile belongs to.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub EdgeSide {
        0 => Owner,
        1 => Neighbor,
    }
}

#[derive(Debug, Clone, Default)]
struct EdgeCellData {
    /// Vertex positions packed as [x, y, x, y, ...]
    vertices: Vec<f32>,
    /// Polygon indices - flat index array
    indices: Vec<u32>,
    /// Index ranges forming a closed polygon packed as [start0, end0, start1, end1, ...] pairs
    ranges: Vec<u32>,
    /// Cell id pairs in the same order as the polygon indices [owner_cell_id, neighbor_cell_id, owner_cell_id, neighbor_cell_id, ...]
    cell_ids: Vec<u32>,
}

/// Standalone geometry snapshot of a chunk's edge cells guarded by the owner and neighbor chunk generations.
pub struct EdgeCells {
    data: EdgeCellData,
    guards: [GenerationGuard; 2],
}

impl EdgeCells {
    pub fn new(
        vertices: Vec<f32>,
        indices: Vec<u32>,
        ranges: Vec<u32>,
        cell_ids: Vec<u32>,
        owner: &Generation,
        neighbor: &Generation,
    ) -> Self {
        Self {
            data: EdgeCellData {
                vertices,
                indices,
                ranges,
                cell_ids,
            },
            guards: [GenerationGuard::new(owner), GenerationGuard::new(neighbor)],
        }
    }

    /// Whether both source chunks are unchanged; `false` means every accessor returns `None`.
    pub fn is_valid(&self) -> bool {
        self.guards.iter().all(GenerationGuard::is_valid)
    }

    /// The captured buffers while both source chunks are unchanged, `None` otherwise.
    fn get(&self) -> Option<&EdgeCellData> {
        self.is_valid().then_some(&self.data)
    }

    pub fn vertices(&self) -> Option<&[f32]> {
        self.get().map(|d| d.vertices.as_slice())
    }

    pub fn indices(&self) -> Option<&[u32]> {
        self.get().map(|d| d.indices.as_slice())
    }

    pub fn ranges(&self) -> Option<&[u32]> {
        self.get().map(|d| d.ranges.as_slice())
    }

    pub fn cell_ids(&self) -> Option<&[u32]> {
        self.get().map(|d| d.cell_ids.as_slice())
    }
}
