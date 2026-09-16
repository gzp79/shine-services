use crate::world::generation::{Generation, GenerationGuard};

/// Geometry buffers of a chunk's internal cells. Access goes through the guarded accessors on `InnerCells`.
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
    /// Tile id of each vertex in the same order as the vertex positions.
    tile_ids: Vec<u32>,
    /// Tile-local vertex (0..4) of each polygon index entry, in the same order as `indices`
    tile_vertices: Vec<u8>,
    /// Tile distortion in the same order as tile_ids packed as [x, y, ...], where each octet corresponds to a single tile
    tile_distortions: Vec<f32>,
}

impl InnerCellData {
    /// (tile_id, quad-local vertex 0..4) pairs of every quad bordering `cell_id`.
    fn cell_tiles(&self, cell_id: u32) -> impl Iterator<Item = (u32, u8)> + '_ {
        // `cell_ids` is sorted ascending, so this is a binary search.
        let range = self
            .cell_ids
            .binary_search(&cell_id)
            .ok()
            .map(|i| (self.ranges[2 * i] as usize, self.ranges[2 * i + 1] as usize));
        range.into_iter().flat_map(move |(s, e)| {
            (s..e).map(move |k| (self.tile_ids[self.indices[k] as usize], self.tile_vertices[k]))
        })
    }
}

/// Standalone geometry snapshot of a chunk's internal cells guarded by the chunk's generation.
pub struct InnerCells {
    data: InnerCellData,
    guard: GenerationGuard,
}

impl InnerCells {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        vertices: Vec<f32>,
        indices: Vec<u32>,
        ranges: Vec<u32>,
        cell_ids: Vec<u32>,
        tile_ids: Vec<u32>,
        tile_vertices: Vec<u8>,
        tile_distortions: Vec<f32>,
        generation: &Generation,
    ) -> Self {
        Self {
            data: InnerCellData {
                vertices,
                indices,
                ranges,
                cell_ids,
                tile_ids,
                tile_vertices,
                tile_distortions,
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

    pub fn tile_ids(&self) -> Option<&[u32]> {
        self.get().map(|d| d.tile_ids.as_slice())
    }

    pub fn tile_vertices(&self) -> Option<&[u8]> {
        self.get().map(|d| d.tile_vertices.as_slice())
    }

    pub fn tile_distortions(&self) -> Option<&[f32]> {
        self.get().map(|d| d.tile_distortions.as_slice())
    }

    /// (tile_id, quad-local vertex 0..4) pairs of every quad bordering `cell_id`.
    pub fn cell_tiles(&self, cell_id: u32) -> Option<impl Iterator<Item = (u32, u8)> + '_> {
        self.get().map(|d| d.cell_tiles(cell_id))
    }
}
