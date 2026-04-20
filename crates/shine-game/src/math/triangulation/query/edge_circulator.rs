use crate::{
    indexed::TypedIndex,
    math::triangulation::{FaceEdge, FaceIndex, Rot3Idx, Triangulation, VertexClue, VertexIndex},
};

/// An iterator that circulates around a vertex in CCW or CW order
/// returning the edge outgoing edge (face and edge index) of each adjacent face.
pub struct EdgeCirculator<'a, const DELAUNAY: bool> {
    tri: &'a Triangulation<DELAUNAY>,
    vertex: VertexIndex,
    current: FaceEdge,
}

impl<'a, const DELAUNAY: bool> EdgeCirculator<'a, DELAUNAY> {
    pub fn new(tri: &Triangulation<DELAUNAY>, start: VertexIndex) -> EdgeCirculator<'_, DELAUNAY> {
        assert_eq!(tri.dimension(), 2);
        let face = tri[start].triangle;
        let edge = tri[face].find_vertex(start).unwrap().decrement();

        EdgeCirculator {
            tri,
            vertex: start,
            current: FaceEdge::new(face, edge),
        }
    }

    pub fn current(&self) -> &FaceEdge {
        &self.current
    }

    pub fn start_vertex(&self) -> VertexIndex {
        self.vertex
    }

    pub fn end_vertex(&self) -> VertexIndex {
        self.tri.vi(VertexClue::end_of(self.current))
    }

    pub fn face(&self) -> FaceIndex {
        self.current.triangle
    }

    pub fn edge(&self) -> Rot3Idx {
        self.current.edge
    }

    pub fn advance_ccw(&mut self) {
        assert_eq!(self.tri.dimension(), 2);
        assert!(self.current.triangle.is_valid());
        assert!(self.tri.vi(VertexClue::start_of(self.current)) == self.vertex);

        self.current.triangle = self.tri[self.current.triangle].neighbors[self.current.edge.decrement()];
        self.current.edge = self.tri[self.current.triangle]
            .find_vertex(self.vertex)
            .unwrap()
            .decrement();
    }

    pub fn next_ccw(&mut self) -> FaceEdge {
        let edge = *self.current();
        self.advance_ccw();
        edge
    }

    pub fn advance_cw(&mut self) {
        assert_eq!(self.tri.dimension(), 2);
        assert!(self.current.triangle.is_valid());
        assert!(self.tri.vi(VertexClue::start_of(self.current)) == self.vertex);

        self.current.triangle = self.tri[self.current.triangle].neighbors[self.current.edge];
        self.current.edge = self.tri[self.current.triangle]
            .find_vertex(self.vertex)
            .unwrap()
            .decrement();
    }

    pub fn next_cw(&mut self) -> FaceEdge {
        let edge = *self.current();
        self.advance_cw();
        edge
    }
}

impl<const DELAUNAY: bool> Triangulation<DELAUNAY> {
    pub fn edge_circulator(&self, vertex: VertexIndex) -> EdgeCirculator<'_, DELAUNAY> {
        EdgeCirculator::new(self, vertex)
    }
}
