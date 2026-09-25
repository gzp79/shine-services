use crate::world::generation::{Generation, GenerationGuard};

#[derive(Debug, Clone, Default)]
struct InnerCellData {
    /// Vertex positions packed as [x, y, x, y, ...]
    vertices: Vec<f32>,
    /// Polygon indices - flat index array
    indices: Vec<u32>,
    /// Index ranges forming a closed polygon packed as [start0, end0, start1, end1, ...] pairs
    ranges: Vec<u32>,
    /// Cell id of each polygon in the same order as the polygon indices.
    cell_ids: Vec<u32>,
}

/// Standalone geometry snapshot of a chunk's internal cells guarded by the chunk's generation.
pub struct InnerCells {
    data: InnerCellData,
    guard: GenerationGuard,
}

impl InnerCells {
    pub fn new(
        vertices: Vec<f32>,
        indices: Vec<u32>,
        ranges: Vec<u32>,
        cell_ids: Vec<u32>,
        generation: &Generation,
    ) -> Self {
        Self {
            data: InnerCellData {
                vertices,
                indices,
                ranges,
                cell_ids,
            },
            guard: GenerationGuard::new(generation),
        }
    }

    /// Whether the source chunk is unchanged; `false` means every accessor returns `None`.
    pub fn is_valid(&self) -> bool {
        self.guard.is_valid()
    }

    /// The captured buffers while the source chunk is unchanged, `None` otherwise.
    fn get(&self) -> Option<&InnerCellData> {
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
