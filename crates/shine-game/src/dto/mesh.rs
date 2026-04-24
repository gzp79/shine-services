use glam::Vec2;

/// Indexed mesh DTO for transferring geometry data
#[derive(Debug, Clone, Default)]
pub struct IndexedMesh {
    pub vertices: Vec<f32>,       // Flat [x,y,x,y,...] array
    pub indices: Vec<u32>,        // Flat index array
    pub polygon_ranges: Vec<u32>, // Flat [start0, end0, start1, end1, ...] pairs
    pub wire_indices: Vec<u32>,   // Wire line segments (empty if no wires)
    pub wire_ranges: Vec<u32>,    // Flat [start0, end0, start1, end1, ...] pairs
}

impl IndexedMesh {
    pub fn from_polyline(polyline: &[Vec2]) -> Self {
        let vertices = polyline.into_iter().flat_map(|v| vec![v.x, v.y]).collect();
        let indices = (0..polyline.len() as u32).collect();
        let polygon_ranges = vec![0, polyline.len() as u32];

        Self {
            vertices,
            indices,
            polygon_ranges,
            wire_indices: Vec::new(),
            wire_ranges: Vec::new(),
        }
    }

    /// Create a scoped appender that captures current offsets
    /// Allows safe append of vertices, polygons, and wires in any order
    pub fn append(&mut self) -> MeshAppender<'_> {
        MeshAppender {
            vertex_offset: self.vertex_count() as u32,
            index_offset: self.indices.len() as u32,
            wire_offset: self.wire_indices.len() as u32,
            mesh: self,
        }
    }

    /// Get the current vertex count
    pub fn vertex_count(&self) -> usize {
        self.vertices.len() / 2
    }

    /// Get polygon count
    pub fn polygon_count(&self) -> usize {
        self.polygon_ranges.len() / 2
    }

    /// Get wire count
    pub fn wire_count(&self) -> usize {
        self.wire_ranges.len() / 2
    }
}

/// Scoped appender for safely adding vertices, polygons, and wires to a mesh
/// Captures offsets at creation, so append order doesn't matter
pub struct MeshAppender<'a> {
    mesh: &'a mut IndexedMesh,
    vertex_offset: u32,
    index_offset: u32,
    wire_offset: u32,
}

impl<'a> MeshAppender<'a> {
    /// Append vertices (flat [x,y,x,y,...] array)
    pub fn vertices(self, vertices: &[f32]) -> Self {
        self.mesh.vertices.extend_from_slice(vertices);
        self
    }

    /// Append polygon indices and ranges
    /// Indices are offset by the initial vertex count
    /// Ranges are offset by the initial index count
    pub fn polygons(self, indices: &[u32], polygon_ranges: &[u32]) -> Self {
        // Append indices with vertex offset
        self.mesh
            .indices
            .extend(indices.iter().map(|&idx| idx + self.vertex_offset));

        // Append polygon ranges with index offset (every value is offset)
        self.mesh
            .polygon_ranges
            .extend(polygon_ranges.iter().map(|&r| r + self.index_offset));

        self
    }

    /// Append wire indices and ranges
    /// Wire indices are offset by the initial vertex count
    /// Wire ranges are offset by the initial wire index count
    pub fn wires(self, wire_indices: &[u32], wire_ranges: &[u32]) -> Self {
        // Append wire indices with vertex offset
        self.mesh
            .wire_indices
            .extend(wire_indices.iter().map(|&idx| idx + self.vertex_offset));

        // Append wire ranges with wire offset (every value is offset)
        self.mesh
            .wire_ranges
            .extend(wire_ranges.iter().map(|&r| r + self.wire_offset));

        self
    }
}
