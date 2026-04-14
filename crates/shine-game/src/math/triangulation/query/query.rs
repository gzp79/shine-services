use crate::math::triangulation::{FaceEdge, Triangulation, VertexClue, VertexIndex};

impl<const DELAUNAY: bool> Triangulation<DELAUNAY> {
    pub fn twin_edge<E: Into<FaceEdge>>(&self, edge: E) -> FaceEdge {
        let edge: FaceEdge = edge.into();
        let nf = self[edge.face].neighbors[edge.edge];
        let i = self[nf]
            .find_neighbor(edge.face)
            .expect("Neighbor should have back-reference");
        FaceEdge::new(nf, i)
    }

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
}
