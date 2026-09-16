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

/// Geometry buffers of a chunk's corner cells (single polygon, at most ~10 vertices). Access goes
/// through the guarded accessors on `CornerCells`.
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
    /// Packed owner chunk and tile id pairs in the same order as the vertex positions [owner, tile_id, owner, tile_id, ...],
    /// where 0 means the owning chunk, 1 the ccw neighbor chunk, and 2 the cw (2*ccw) neighbor chunk
    tile_ids: Vec<u32>,
    /// Tile-local vertex (0..4) of each vertex in the same order as the vertex positions.
    tile_vertices: Vec<u8>,
    /// Tile distortion in the same order as tile_ids packed as [x, y, ...], where each octet corresponds to a single tile
    tile_distortions: Vec<f32>,
}

impl CornerCellData {
    /// (tile_id, quad-local vertex 0..4) pairs of every quad bordering `cell_id` on the given `side`.
    fn cell_tiles(&self, side: CornerSide, cell_id: u32) -> impl Iterator<Item = (u32, u8)> + '_ {
        let valid = self.cell_ids.get(side.into_index()) == Some(&cell_id);
        let side = side.into_index() as u32;
        (0..self.tile_vertices.len())
            .filter(move |&k| valid && self.tile_ids[2 * k] == side)
            .map(move |k| (self.tile_ids[2 * k + 1], self.tile_vertices[k]))
    }
}

/// Standalone geometry snapshot of a chunk's corner cells guarded by the three source chunk generations.
pub struct CornerCells {
    data: CornerCellData,
    guards: [GenerationGuard; 3],
}

impl CornerCells {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        vertices: Vec<f32>,
        indices: Vec<u32>,
        ranges: [u32; 2],
        cell_ids: Vec<u32>,
        tile_ids: Vec<u32>,
        tile_vertices: Vec<u8>,
        tile_distortions: Vec<f32>,
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
                tile_ids,
                tile_vertices,
                tile_distortions,
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

    pub fn tile_ids(&self) -> Option<&[u32]> {
        self.get().map(|d| d.tile_ids.as_slice())
    }

    pub fn tile_vertices(&self) -> Option<&[u8]> {
        self.get().map(|d| d.tile_vertices.as_slice())
    }

    pub fn tile_distortions(&self) -> Option<&[f32]> {
        self.get().map(|d| d.tile_distortions.as_slice())
    }

    /// (tile_id, quad-local vertex 0..4) pairs of every quad bordering `cell_id` on the given `side`.
    pub fn cell_tiles(&self, side: CornerSide, cell_id: u32) -> Option<impl Iterator<Item = (u32, u8)> + '_> {
        self.get().map(|d| d.cell_tiles(side, cell_id))
    }
}
