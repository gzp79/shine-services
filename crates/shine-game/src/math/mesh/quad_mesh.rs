use crate::{
    indexed::IdxVec,
    math::mesh::{QuadTopology, QuadTopologyError, VertIdx},
};
use glam::Vec2;

/// Quad mesh with positions and topology.
///
/// Combines geometric vertex positions with topological connectivity via ghost vertex
/// and ghost quads for closed manifold traversal.
pub struct QuadMesh {
    pub topology: QuadTopology,
    pub positions: IdxVec<VertIdx, Vec2>,
}

impl QuadMesh {
    /// Create a new quad mesh from vertex positions and quad connectivity.
    ///
    /// # Arguments
    ///
    /// * `positions` - 2D positions for each real vertex (ghost vertex has no position)
    /// * `polygon` - Boundary vertices in CCW order (must have even length)
    /// * `quads` - Quad vertex indices in CCW winding order `[v0, v1, v2, v3]`
    ///
    /// # Returns
    ///
    /// Returns `Err` if:
    /// - Boundary polygon has odd length
    /// - Boundary or quad vertices are out of range
    /// - Boundary vertices are not unique
    /// - Quads reference the ghost vertex
    /// - Topology is incomplete (edges without neighbors)
    pub fn new(
        positions: Vec<Vec2>,
        polygon: Vec<VertIdx>,
        quads: Vec<[VertIdx; 4]>,
    ) -> Result<Self, QuadTopologyError> {
        let vertex_count = positions.len();
        let positions = IdxVec::from_vec(positions);
        let topology = QuadTopology::new(vertex_count, polygon, quads)?;

        Ok(Self { topology, positions })
    }

    pub fn position(&self, vi: VertIdx) -> Vec2 {
        self.positions[vi]
    }

    pub fn topology(&self) -> &QuadTopology {
        &self.topology
    }

    pub fn vertex_indices(&self) -> impl Iterator<Item = VertIdx> {
        self.topology.vertex_indices()
    }

    pub fn into_parts(self) -> (QuadTopology, IdxVec<VertIdx, Vec2>) {
        (self.topology, self.positions)
    }
}
