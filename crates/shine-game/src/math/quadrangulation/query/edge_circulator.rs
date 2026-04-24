use crate::{
    indexed::TypedIndex,
    math::quadrangulation::{QuadIndex, QuadVertex, Quadrangulation, VertexIndex},
};

/// A circulator that traverses around a vertex in CCW or CW order, returning the edge (quad and edge index) of each adjacent quad.
/// Unlike iterators, circulators can move in either direction and don't have a fixed termination.
pub struct EdgeCirculator<'a> {
    quad: &'a Quadrangulation,
    /// vertex index that this circulator is centered around.
    vertex: VertexIndex,
    /// The current position of the circulator, represented as a QuadVertex referncing the center vertex.
    current: QuadVertex,
}

impl<'a> EdgeCirculator<'a> {
    pub fn new(quad: &'a Quadrangulation, vi: VertexIndex) -> Self {
        let q = quad.vertices[vi].quad;
        let local = quad[q].find_vertex(vi).unwrap();
        let start_qv = QuadVertex { quad: q, local };

        EdgeCirculator {
            quad,
            vertex: vi,
            current: start_qv,
        }
    }

    #[inline]
    pub fn current(&self) -> QuadVertex {
        self.current
    }

    #[inline]
    pub fn start_vertex(&self) -> VertexIndex {
        self.vertex
    }

    #[inline]
    pub fn end_vertex(&self) -> VertexIndex {
        self.quad.vi(self.current.next())
    }

    #[inline]
    pub fn quad(&self) -> QuadIndex {
        self.current.quad
    }

    #[inline]
    pub fn advance_ccw(&mut self) {
        debug_assert!(self.current.quad.is_valid());
        debug_assert_eq!(self.quad.vi(self.current), self.vertex);

        let edge = self.current.incoming_edge();
        let neighbor = self.quad.edge_twin(edge);
        self.current = neighbor.start();
    }

    #[inline]
    pub fn next_ccw(&mut self) -> QuadVertex {
        let qv = self.current();
        self.advance_ccw();
        qv
    }

    #[inline]
    pub fn advance_cw(&mut self) {
        debug_assert!(self.current.quad.is_valid());
        debug_assert_eq!(self.quad.vi(self.current), self.vertex);

        let edge = self.current.outgoing_edge();
        let neighbor = self.quad.edge_twin(edge);
        self.current = neighbor.end();
    }

    #[inline]
    pub fn next_cw(&mut self) -> QuadVertex {
        let qv = self.current();
        self.advance_cw();
        qv
    }
}
