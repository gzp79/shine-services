use crate::world::generation::{Generation, GenerationGuard};

crate::define_enum_index! {
    /// Which side of a CornerCells polygon a tile belongs to.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub CornerSide {
        0 => Owner,
        1 => CcwNeighbor,
        2 => CwNeighbor,
    }
}

#[derive(Debug, Clone, Default)]
struct CornerCellData {
    /// Vertex positions packed as [x, y, x, y, ...]
    vertices: Vec<f32>,
    /// Polygon indices (0..vertex_count)
    indices: Vec<u32>,
    /// Index range [0, vertex_count]
    ranges: [u32; 2],
    /// Cell id in the [owner, ccw_neighbor, cw_neighbor (same as 2*ccw neighbor) ] order
    cell_ids: Vec<u32>,
}

/// Standalone geometry snapshot of a chunk's corner cells guarded by the three source chunk generations.
pub struct CornerCells {
    data: CornerCellData,
    guards: [GenerationGuard; 3],
}

impl CornerCells {
    pub fn new(
        vertices: Vec<f32>,
        indices: Vec<u32>,
        ranges: [u32; 2],
        cell_ids: Vec<u32>,
        owner: &Generation,
        ccw_neighbor: &Generation,
        cw_neighbor: &Generation,
    ) -> Self {
        Self {
            data: CornerCellData {
                vertices,
                indices,
                ranges,
                cell_ids,
            },
            guards: [
                GenerationGuard::new(owner),
                GenerationGuard::new(ccw_neighbor),
                GenerationGuard::new(cw_neighbor),
            ],
        }
    }

    /// Whether all three source chunks are unchanged; `false` means every accessor returns `None`.
    pub fn is_valid(&self) -> bool {
        self.guards.iter().all(GenerationGuard::is_valid)
    }

    /// The captured buffers while all three source chunks are unchanged, `None` otherwise.
    fn get(&self) -> Option<&CornerCellData> {
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
