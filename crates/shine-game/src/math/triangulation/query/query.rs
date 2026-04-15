use crate::math::triangulation::{
    predicates::orient2d, FaceEdge, FaceIndex, Rot3Idx, Triangulation, VertexClue, VertexIndex,
};

/// Navigation and geometric query helpers
impl<const DELAUNAY: bool> Triangulation<DELAUNAY> {
    /// Get the twin edge (opposite side of the same edge)
    #[inline]
    pub fn twin_edge<E: Into<FaceEdge>>(&self, edge: E) -> FaceEdge {
        let edge: FaceEdge = edge.into();
        let nf = self[edge.face].neighbors[edge.edge];
        let i = self[nf]
            .find_neighbor(edge.face)
            .expect("Neighbor should have back-reference");
        FaceEdge::new(nf, i)
    }

    /// Find an edge connecting two vertices
    pub fn find_edge_by_vertex(&self, a: VertexIndex, b: VertexIndex) -> Option<FaceEdge> {
        let mut iter = self.edge_circulator(a);
        let start = iter.next_ccw();
        let mut edge = start;
        loop {
            if self.vi(VertexClue::end_of(edge)) == b {
                break Some(edge);
            }

            edge = iter.next_ccw();
            if edge == start {
                break None;
            }
        }
    }

    /// Get the orientation of three vertices (positive = CCW, negative = CW, zero = collinear)
    #[inline]
    pub(crate) fn get_vertices_orientation(&self, v0: VertexIndex, v1: VertexIndex, v2: VertexIndex) -> i64 {
        assert!(self.is_finite_vertex(v0) && self.is_finite_vertex(v1) && self.is_finite_vertex(v2));

        let a = self.p(v0);
        let b = self.p(v1);
        let c = self.p(v2);

        orient2d(a, b, c)
    }

    /// Get the orientation of a vertex relative to an edge
    #[inline]
    pub(crate) fn get_edge_vertex_orientation(&self, f: FaceIndex, i: Rot3Idx, v: VertexIndex) -> i64 {
        let va = v;
        let vb = self[f].vertices[i.increment()];
        let vc = self[f].vertices[i.decrement()];
        self.get_vertices_orientation(va, vb, vc)
    }
}
