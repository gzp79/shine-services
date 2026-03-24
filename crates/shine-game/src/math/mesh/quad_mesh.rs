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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::indexed::TypedIndex;
    use shine_test::test;

    /// Build a simple 2-quad mesh for testing:
    /// ```text
    ///  3---2---5
    ///  |   |   |
    ///  0---1---4
    /// ```
    /// Quad 0: [0, 1, 2, 3], Quad 1: [1, 4, 5, 2]
    fn two_quad_mesh() -> QuadMesh {
        let positions = vec![
            Vec2::new(0.0, 0.0), // 0
            Vec2::new(1.0, 0.0), // 1
            Vec2::new(1.0, 1.0), // 2
            Vec2::new(0.0, 1.0), // 3
            Vec2::new(2.0, 0.0), // 4
            Vec2::new(2.0, 1.0), // 5
        ];
        let quads = vec![
            [VertIdx::new(0), VertIdx::new(1), VertIdx::new(2), VertIdx::new(3)],
            [VertIdx::new(1), VertIdx::new(4), VertIdx::new(5), VertIdx::new(2)],
        ];
        let is_boundary = vec![true, false, false, true, true, true];
        let mut mesh = QuadMesh::new(positions, quads, is_boundary);
        mesh.sort_vertex_rings();
        mesh
    }

    #[test]
    fn test_quad_count() {
        let mesh = two_quad_mesh();
        assert_eq!(mesh.vertex_count(), 6);
        assert_eq!(mesh.quad_count(), 2);
    }

    #[test]
    fn test_quad_vertices() {
        let mesh = two_quad_mesh();
        let v = mesh.quad_vertices(QuadIdx::new(0));
        assert_eq!(v[0], VertIdx::new(0));
        assert_eq!(v[1], VertIdx::new(1));
        assert_eq!(v[2], VertIdx::new(2));
        assert_eq!(v[3], VertIdx::new(3));
    }

    #[test]
    fn test_neighbor_symmetry() {
        let mesh = two_quad_mesh();
        let n01 = mesh.quad_neighbor(QuadIdx::new(0), 1);
        assert_eq!(n01, QuadIdx::new(1));

        let n10 = mesh.quad_neighbor(QuadIdx::new(1), 3);
        assert_eq!(n10, QuadIdx::new(0));
    }

    #[test]
    fn test_boundary_vertex() {
        let mesh = two_quad_mesh();
        assert!(mesh.is_boundary_vertex(VertIdx::new(0)));
        assert!(!mesh.is_boundary_vertex(VertIdx::new(1)));
        assert!(!mesh.is_boundary_vertex(VertIdx::new(2)));
        assert!(mesh.is_boundary_vertex(VertIdx::new(3)));
    }

    #[test]
    fn test_vertex_ring_local_indices() {
        let mesh = two_quad_mesh();
        for vi in mesh.vertex_indices() {
            for r in mesh.vertex_ring(vi) {
                let verts = mesh.quad_vertices(r.quad);
                assert_eq!(verts[r.local as usize], vi);
            }
        }
    }

    #[test]
    fn test_ghost_quads_close_rings() {
        let mesh = two_quad_mesh();
        for qi in mesh.quad_indices() {
            for k in 0..4 {
                let n = mesh.quad_neighbor(qi, k);
                assert!(!n.is_none(), "real quad {:?} edge {} still has NONE neighbor", qi, k);
            }
        }
    }

    #[test]
    fn test_neighbor_avg() {
        let mesh = two_quad_mesh();
        let positions: Vec<Vec2> = (0..mesh.vertex_count())
            .map(|i| mesh.position(VertIdx::new(i)))
            .collect();

        // Vertex 1 (interior): neighbors are 0, 2, 4
        let avg = mesh.topology().neighbor_avg(VertIdx::new(1), &positions);
        let expected = (positions[0] + positions[2] + positions[4]) / 3.0;
        assert!((avg - expected).length() < 1e-6, "avg = {:?}", avg);
    }
}
