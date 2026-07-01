use crate::mesh::{AsPolygonMesh, AsWiredPolygonMesh};
use glam::Vec2;

/// Polygon mesh DTO for transferring geometry data
#[derive(Debug, Clone, Default)]
pub struct WiredPolygonMesh {
    /// Vertex positions packed as [x, y, x, y, ...]
    pub vertices: Vec<f32>,
    /// Polygon indices - flat index array
    pub indices: Vec<u32>,
    /// Index ranges [start0, end0, start1, end1, ...] pairs
    pub ranges: Vec<u32>,
    /// Wire line segments (empty if no wires)
    pub wire_indices: Vec<u32>,
    /// Wire index ranges [start0, end0, start1, end1, ...] pairs
    pub wire_ranges: Vec<u32>,
}

impl WiredPolygonMesh {
    pub fn from_polyline(polyline: &[Vec2]) -> Self {
        let vertices = polyline.iter().flat_map(|v| [v.x, v.y]).collect();
        let indices = (0..polyline.len() as u32).collect();
        let ranges = vec![0, polyline.len() as u32];

        Self {
            vertices,
            indices,
            ranges,
            wire_indices: Vec::new(),
            wire_ranges: Vec::new(),
        }
    }

    /// Create a scoped appender that captures current offsets
    pub fn append(&mut self) -> MeshAppender<'_> {
        MeshAppender {
            vertex_offset: self.vertex_count() as u32,
            index_offset: self.indices.len() as u32,
            wire_offset: self.wire_indices.len() as u32,
            mesh: self,
        }
    }

    pub fn vertex_count(&self) -> usize {
        self.vertices.len() / 2
    }

    pub fn polygon_count(&self) -> usize {
        self.ranges.len() / 2
    }

    pub fn wire_count(&self) -> usize {
        self.wire_ranges.len() / 2
    }
}

impl AsPolygonMesh for WiredPolygonMesh {
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

impl AsWiredPolygonMesh for WiredPolygonMesh {
    fn wire_indices(&self) -> &[u32] {
        &self.wire_indices
    }

    fn wire_ranges(&self) -> &[u32] {
        &self.wire_ranges
    }
}

/// Scoped appender for safely adding vertices, polygons, and wires to a mesh.
/// Captures offsets at creation, so append order doesn't matter.
pub struct MeshAppender<'a> {
    mesh: &'a mut WiredPolygonMesh,
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

    /// Append polygon indices and ranges (indices offset by vertex count, ranges by index count)
    pub fn polygons(self, indices: &[u32], ranges: &[u32]) -> Self {
        self.mesh
            .indices
            .extend(indices.iter().map(|&idx| idx + self.vertex_offset));
        self.mesh.ranges.extend(ranges.iter().map(|&r| r + self.index_offset));
        self
    }

    /// Append wire indices and ranges (wire indices offset by vertex count, ranges by wire count)
    pub fn wires(self, wire_indices: &[u32], wire_ranges: &[u32]) -> Self {
        self.mesh
            .wire_indices
            .extend(wire_indices.iter().map(|&idx| idx + self.vertex_offset));
        self.mesh
            .wire_ranges
            .extend(wire_ranges.iter().map(|&r| r + self.wire_offset));
        self
    }
}
