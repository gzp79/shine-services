/// Cell data of the internal cells of a chunk
#[derive(Debug, Clone, Default)]
pub struct InnerCells {
    /// Vertex positions packed as [x, y, x, y, ...]
    pub vertices: Vec<f32>,
    /// Polygon indices - flat index array
    pub indices: Vec<u32>,
    /// Index ranges forming a closed polygon packed as [start0, end0, start1, end1, ...] pairs
    pub polygon_ranges: Vec<u32>,
    /// Site id of each polygon in the same order as the polygon indices.
    pub sites: Vec<u32>,
}

/// Cell data of the edge cells of a chunk
#[derive(Debug, Clone, Default)]
pub struct EdgeCells {
    /// Vertex positions packed as [x, y, x, y, ...]
    pub vertices: Vec<f32>,
    /// Polygon indices - flat index array
    pub indices: Vec<u32>,
    /// Index ranges forming a closed polygon packed as [start0, end0, start1, end1, ...] pairs
    pub polygon_ranges: Vec<u32>,
    /// Site id of the owner chunk for each polygon in the same order as the polygon indices.
    pub owner_sites: Vec<u32>,
    /// Site id of the neighbor chunk for each polygon in the same order as the polygon indices.
    pub neighbor_sites: Vec<u32>,
}

/// Cell data of the corner cells of a chunk
#[derive(Debug, Clone, Default)]
pub struct CornerCells {
    /// Vertex positions packed as [x, y, x, y, ...]
    pub vertices: Vec<f32>,
    /// Site id of the owner chunk
    pub owner_site: u32,
    /// Site id of neighbor in clockwise direction
    pub cw_site: u32,
    /// Site id of neighbor in counter-clockwise direction
    pub ccw_site: u32,
}
