use crate::{
    indexed::TypedIndex,
    math::triangulation::{Face, FaceEdge, FaceIndex, Rot3Idx, Triangulation, Vertex, VertexIndex},
};
use glam::IVec2;

/// Low-level primitive operations for building triangulations.
///
/// These methods provide basic operations for creating and manipulating
/// triangulation elements (vertices, faces) and their relationships.
impl<const DELAUNAY: bool> Triangulation<DELAUNAY> {
    pub(crate) fn set_dimension(&mut self, dim: u8) {
        assert!(dim <= 2);
        self.dimension = dim;
    }

    pub(crate) fn create_infinite_vertex(&mut self) -> VertexIndex {
        assert!(self.infinite_vertex.is_none());
        let v = self.store_vertex(Vertex::new());
        self.infinite_vertex = v;
        v
    }

    pub(crate) fn create_vertex_with_position(&mut self, p: IVec2) -> VertexIndex {
        let mut v = Vertex::new();
        v.position = p;
        self.store_vertex(v)
    }

    pub(crate) fn create_face(&mut self) -> FaceIndex {
        self.store_face(Face::new())
    }

    pub(crate) fn create_face_with_vertices(&mut self, v0: VertexIndex, v1: VertexIndex, v2: VertexIndex) -> FaceIndex {
        self.store_face(Face::with_vertices(v0, v1, v2))
    }

    pub(crate) fn clear_constraint<E: Into<FaceEdge>>(&mut self, edge: E) {
        let edge: FaceEdge = edge.into();
        let nf = self[edge.face].neighbors[edge.edge];
        let ni = self[nf].find_neighbor(edge.face).unwrap();
        self[edge.face].constraints[edge.edge] = 0;
        self[nf].constraints[ni] = 0;
    }

    pub(crate) fn merge_constraint<E: Into<FaceEdge>>(&mut self, edge: E, c: u32) {
        let edge: FaceEdge = edge.into();
        let nf = self[edge.face].neighbors[edge.edge];
        let ni = self[nf].find_neighbor(edge.face).unwrap();
        self[edge.face].constraints[edge.edge] |= c;
        self[nf].constraints[ni] |= c;
    }

    pub(crate) fn copy_constraint_partial(
        &mut self,
        f_from: FaceIndex,
        i_from: Rot3Idx,
        f_to: FaceIndex,
        i_to: Rot3Idx,
    ) {
        let c = self[f_from].constraints[i_from];
        self[f_to].constraints[i_to] = c;
    }

    pub(crate) fn set_adjacent<A: Into<FaceEdge>, B: Into<FaceEdge>>(&mut self, a: A, b: B) {
        let FaceEdge { face: f0, edge: i0 } = a.into();
        let FaceEdge { face: f1, edge: i1 } = b.into();
        assert!(i0.is_valid() && i1.is_valid());
        assert!(u8::from(i0) <= self.dimension() && u8::from(i1) <= self.dimension());
        self[f0].neighbors[i0] = f1;
        self[f1].neighbors[i1] = f0;
    }

    pub(crate) fn move_adjacent<T: Into<FaceEdge>, S: Into<FaceEdge>>(&mut self, target: T, source: S) {
        let FaceEdge { face, edge } = source.into();
        let n = self[face].neighbors[edge];
        let i = self[n].find_neighbor(face).unwrap();
        self.set_adjacent(target, FaceEdge::new(n, i));
    }
}
