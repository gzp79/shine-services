use crate::{
    indexed::TypedIndex,
    math::quadrangulation::{QuadEdge, QuadIndex, Quadrangulation, Rot4Idx, VertexIndex, VertexClue},
};

/// A circulator that traverses around a vertex in CCW or CW order, returning the edge (quad and edge index) of each adjacent quad.
/// Unlike iterators, circulators can move in either direction and don't have a fixed termination.
pub struct EdgeCirculator<'a> {
    quad: &'a Quadrangulation,
    vertex: VertexIndex,
    current: QuadEdge,
}

impl<'a> EdgeCirculator<'a> {
    pub fn new(quad: &'a Quadrangulation, start: VertexIndex) -> Self {
        let q = quad[start].quad;
        let edge = quad[q].find_vertex(start).unwrap().decrement();

        EdgeCirculator {
            quad,
            vertex: start,
            current: QuadEdge::new(q, edge),
        }
    }

    pub fn current(&self) -> &QuadEdge {
        &self.current
    }

    pub fn start_vertex(&self) -> VertexIndex {
        self.vertex
    }

    pub fn end_vertex(&self) -> VertexIndex {
        self.quad.vi(VertexClue::EdgeEnd(self.current.quad, self.current.edge))
    }

    pub fn quad(&self) -> QuadIndex {
        self.current.quad
    }

    pub fn edge(&self) -> Rot4Idx {
        self.current.edge
    }

    pub fn advance_ccw(&mut self) {
        assert!(self.current.quad.is_valid());
        assert!(
            self.quad
                .vi(VertexClue::EdgeStart(self.current.quad, self.current.edge))
                == self.vertex
        );

        self.current.quad = self.quad[self.current.quad].neighbors[self.current.edge.decrement()];
        self.current.edge = self.quad[self.current.quad]
            .find_vertex(self.vertex)
            .unwrap()
            .decrement();
    }

    pub fn next_ccw(&mut self) -> QuadEdge {
        let edge = *self.current();
        self.advance_ccw();
        edge
    }

    pub fn advance_cw(&mut self) {
        assert!(self.current.quad.is_valid());
        assert!(
            self.quad
                .vi(VertexClue::EdgeStart(self.current.quad, self.current.edge))
                == self.vertex
        );

        self.current.quad = self.quad[self.current.quad].neighbors[self.current.edge];
        self.current.edge = self.quad[self.current.quad]
            .find_vertex(self.vertex)
            .unwrap()
            .decrement();
    }

    pub fn next_cw(&mut self) -> QuadEdge {
        let edge = *self.current();
        self.advance_cw();
        edge
    }
}
