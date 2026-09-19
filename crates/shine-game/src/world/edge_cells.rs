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
    /// Packed owner chunk and tile id pairs in the same order as the vertex positions [owner, tile_id, owner, tile_id, ...],
    /// where 0 means the owning chunk, 1 the neighbor chunk
    tile_ids: Vec<u32>,
    /// Tile-local vertex (0..4) of each polygon index entry, in the same order as `indices`
    tile_vertices: Vec<u8>,
    /// Tile distortion in the same order as tile_ids packed as [x, y, ...], where each octet corresponds to a single tile
    tile_distortions: Vec<f32>,
}

impl EdgeCellData {
    /// (tile_id, quad-local vertex 0..4) pairs of every quad bordering `cell_id` on the given `side`.
    fn cell_tiles(&self, side: EdgeSide, cell_id: u32) -> impl Iterator<Item = (u32, u8)> + '_ {
        let side = side.into_index();
        let range = self
            .cell_ids
            .iter()
            .skip(side)
            .step_by(2)
            .position(|&c| c == cell_id)
            .map(|i| (self.ranges[2 * i] as usize, self.ranges[2 * i + 1] as usize));
        let side = side as u32;
        range.into_iter().flat_map(move |(s, e)| {
            (s..e).filter_map(move |k| {
                let p = self.indices[k] as usize;
                (self.tile_ids[2 * p] == side).then(|| (self.tile_ids[2 * p + 1], self.tile_vertices[k]))
            })
        })
    }
}

/// Standalone geometry snapshot of a chunk's edge cells guarded by the owner and neighbor chunk generations.
pub struct EdgeCells {
    data: EdgeCellData,
    guards: [GenerationGuard; 2],
}

impl EdgeCells {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        vertices: Vec<f32>,
        indices: Vec<u32>,
        ranges: Vec<u32>,
        cell_ids: Vec<u32>,
        tile_ids: Vec<u32>,
        tile_vertices: Vec<u8>,
        tile_distortions: Vec<f32>,
        owner: &Generation,
        neighbor: &Generation,
    ) -> Self {
        Self {
            data: EdgeCellData {
                vertices,
                indices,
                ranges,
                cell_ids,
                tile_ids,
                tile_vertices,
                tile_distortions,
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
    pub fn cell_tiles(&self, side: EdgeSide, cell_id: u32) -> Option<impl Iterator<Item = (u32, u8)> + '_> {
        self.get().map(|d| d.cell_tiles(side, cell_id))
    }
}
