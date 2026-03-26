use crate::{
    indexed::IdxVec,
    math::mesh::{QuadIdx, QuadTopology, QuadVertRef, VertIdx},
};
use glam::Vec2;

/// Quad mesh with positions and topology.
///
/// Wraps [`QuadTopology`] (adjacency, ghost quads, vertex rings) together
/// with vertex positions. Call [`sort_vertex_rings`] after positions are
/// finalized to sort each ring rotationally (CCW by quad centroid angle).
pub struct QuadMesh {
    positions: IdxVec<VertIdx, Vec2>,
    topology: QuadTopology,
}

impl QuadMesh {
    /// Build a quad mesh from positions, quads, and boundary flags.
    ///
    /// Ghost quads are added for boundary edges. Vertex rings are unsorted —
    /// call [`sort_vertex_rings`] once positions are finalized.
    pub fn new(positions: Vec<Vec2>, quads: Vec<[VertIdx; 4]>, is_boundary: Vec<bool>) -> Self {
        let vertex_count = positions.len();
        let positions = IdxVec::from_vec(positions);
        let topology = QuadTopology::new(vertex_count, quads, is_boundary);

        Self { positions, topology }
    }

    pub fn topology(&self) -> &QuadTopology {
        &self.topology
    }

    pub fn position(&self, vi: VertIdx) -> Vec2 {
        self.positions[vi]
    }

    pub fn positions(&self) -> &IdxVec<VertIdx, Vec2> {
        &self.positions
    }

    pub fn positions_mut(&mut self) -> &mut IdxVec<VertIdx, Vec2> {
        &mut self.positions
    }

    pub fn vertex_count(&self) -> usize {
        self.topology.real_vertex_count()
    }

    pub fn quad_count(&self) -> usize {
        self.topology.real_quad_count()
    }

    pub fn quad_vertices(&self, qi: QuadIdx) -> [VertIdx; 4] {
        self.topology.quad_vertices(qi)
    }

    pub fn quad_neighbor(&self, qi: QuadIdx, edge: usize) -> QuadIdx {
        self.topology.quad_neighbor(qi, edge)
    }

    pub fn vertex_ring(&self, vi: VertIdx) -> &[QuadVertRef] {
        self.topology.vertex_ring(vi)
    }

    pub fn is_boundary_vertex(&self, vi: VertIdx) -> bool {
        self.topology.is_boundary_vertex(vi)
    }

    pub fn is_boundary_edge(&self, qi: QuadIdx, edge: usize) -> bool {
        self.topology.is_boundary_edge(qi, edge)
    }

    pub fn quad_indices(&self) -> impl Iterator<Item = QuadIdx> {
        self.topology.real_quad_indices()
    }

    pub fn vertex_indices(&self) -> impl Iterator<Item = VertIdx> {
        self.topology.vertex_indices()
    }

    /// Sort each vertex's quad ring rotationally (CCW by quad centroid angle).
    pub fn sort_vertex_rings(&mut self) {
        let positions = self.positions.as_slice();
        self.topology.sort_vertex_rings(positions);
    }

    pub fn into_parts(self) -> (QuadTopology, IdxVec<VertIdx, Vec2>) {
        (self.topology, self.positions)
    }
}
