use crate::mesh::AsPolygonMesh;

/// Cell data of the internal cells of a chunk
#[derive(Debug, Clone, Default)]
pub struct InnerCells {
    /// Vertex positions packed as [x, y, x, y, ...]
    pub vertices: Vec<f32>,
    /// Polygon indices - flat index array
    pub indices: Vec<u32>,
    /// Index ranges forming a closed polygon packed as [start0, end0, start1, end1, ...] pairs
    pub ranges: Vec<u32>,
    /// Site id of each polygon in the same order as the polygon indices.
    pub sites: Vec<u32>,
    /// Tile id of each vertex in the same order as the vertex positions.
    pub tiles: Vec<u32>,
    /// Tile distortion in the same order as tiles packed as [x, y, ...], where each octet corresponds to a single tile
    pub tile_distortions: Vec<f32>,
}

impl AsPolygonMesh for InnerCells {
    fn vertices(&self) -> &[f32] {
        &self.vertices
    }

    fn indices(&self) -> &[u32] {
        &self.indices
    }

    fn ranges(&self) -> &[u32] {
        &self.ranges
    }
}

/// Cell data of the edge cells of a chunk
#[derive(Debug, Clone, Default)]
pub struct EdgeCells {
    /// Vertex positions packed as [x, y, x, y, ...]
    pub vertices: Vec<f32>,
    /// Polygon indices - flat index array
    pub indices: Vec<u32>,
    /// Index ranges forming a closed polygon packed as [start0, end0, start1, end1, ...] pairs
    pub ranges: Vec<u32>,
    /// Site id pairs in the same order as the polygon indices [owner_site, neighbor_site, owner_site, neighbor_site, ...]
    pub sites: Vec<u32>,
    /// Packed owner chunk and tile id pairs in the same order as the vertex positions [owner, tile_id, owner, tile_id, ...],
    /// where 0 means the owning chunk, 1 the neighbor chunk
    pub tiles: Vec<u32>,
    /// Tile distortion in the same order as tiles packed as [x, y, ...], where each octet corresponds to a single tile
    pub tile_distortions: Vec<f32>,
}

impl AsPolygonMesh for EdgeCells {
    fn vertices(&self) -> &[f32] {
        &self.vertices
    }

    fn indices(&self) -> &[u32] {
        &self.indices
    }

    fn ranges(&self) -> &[u32] {
        &self.ranges
    }
}

/// Cell data of the corner cells of a chunk (single polygon, at most ~10 vertices)
#[derive(Debug, Clone, Default)]
pub struct CornerCells {
    /// Vertex positions packed as [x, y, x, y, ...]
    pub vertices: Vec<f32>,
    /// Polygon indices (0..vertex_count)
    pub indices: Vec<u32>,
    /// Index range [0, vertex_count]
    pub ranges: [u32; 2],
    /// Site id in the [owner, ccw_neighbor, cw_neighbor (same as 2*ccw neighbor) ] order
    pub sites: Vec<u32>,
    /// Packed owner chunk and tile id pairs in the same order as the vertex positions [owner, tile_id, owner, tile_id, ...],
    /// where 0 means the owning chunk, 1 the ccw neighbor chunk, and 2 the cw (2*ccw) neighbor chunk
    pub tiles: Vec<u32>,
    /// Tile distortion in the same order as tiles packed as [x, y, ...], where each octet corresponds to a single tile
    pub tile_distortions: Vec<f32>,
}

impl AsPolygonMesh for CornerCells {
    fn vertices(&self) -> &[f32] {
        &self.vertices
    }

    fn indices(&self) -> &[u32] {
        &self.indices
    }

    fn ranges(&self) -> &[u32] {
        &self.ranges
    }
}
