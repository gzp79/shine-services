use crate::math::triangulation::{FaceEdge, Triangulation, VertexClue, VertexIndex};

impl<const DELAUNAY: bool> Triangulation<DELAUNAY> {
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
