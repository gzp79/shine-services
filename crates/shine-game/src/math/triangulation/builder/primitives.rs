use super::TriangulationBuilder;
use crate::{
    indexed::TypedIndex,
    math::triangulation::{Face, FaceEdge, FaceIndex, Rot3Idx, Vertex, VertexIndex},
};
use glam::IVec2;

impl<'a, const DELAUNAY: bool> TriangulationBuilder<'a, DELAUNAY> {
    pub fn set_dimension(&mut self, dim: u8) {
        assert!(dim <= 2);
        self.tri.dimension = dim;
    }

    pub fn create_infinite_vertex(&mut self) -> VertexIndex {
        assert!(self.tri.infinite_vertex.is_none());
        let v = self.tri.store_vertex(Vertex::new());
        self.tri.infinite_vertex = v;
        v
    }

    pub fn create_vertex_with_position(&mut self, p: IVec2) -> VertexIndex {
        let mut v = Vertex::new();
        v.position = p;
        self.tri.store_vertex(v)
    }

    pub fn create_face(&mut self) -> FaceIndex {
        self.tri.store_face(Face::new())
    }

    pub fn create_face_with_vertices(&mut self, v0: VertexIndex, v1: VertexIndex, v2: VertexIndex) -> FaceIndex {
        self.tri.store_face(Face::with_vertices(v0, v1, v2))
    }

    pub fn clear_constraint<E: Into<FaceEdge>>(&mut self, edge: E) {
        let edge: FaceEdge = edge.into();
        let nf = self.tri[edge.face].neighbors[edge.edge];
        let ni = self.tri[nf].find_neighbor(edge.face).unwrap();
        self.tri[edge.face].constraints[edge.edge] = 0;
        self.tri[nf].constraints[ni] = 0;
    }

    pub fn merge_constraint<E: Into<FaceEdge>>(&mut self, edge: E, c: u32) {
        let edge: FaceEdge = edge.into();
        let nf = self.tri[edge.face].neighbors[edge.edge];
        let ni = self.tri[nf].find_neighbor(edge.face).unwrap();
        self.tri[edge.face].constraints[edge.edge] |= c;
        self.tri[nf].constraints[ni] |= c;
    }

    pub fn copy_constraint_partial(&mut self, f_from: FaceIndex, i_from: Rot3Idx, f_to: FaceIndex, i_to: Rot3Idx) {
        let c = self.tri[f_from].constraints[i_from];
        self.tri[f_to].constraints[i_to] = c;
    }

    pub fn set_adjacent<A: Into<FaceEdge>, B: Into<FaceEdge>>(&mut self, a: A, b: B) {
        let FaceEdge { face: f0, edge: i0 } = a.into();
        let FaceEdge { face: f1, edge: i1 } = b.into();
        assert!(i0.is_valid() && i1.is_valid());
        assert!(u8::from(i0) <= self.tri.dimension() && u8::from(i1) <= self.tri.dimension());
        self.tri[f0].neighbors[i0] = f1;
        self.tri[f1].neighbors[i1] = f0;
    }

    pub fn move_adjacent<T: Into<FaceEdge>, S: Into<FaceEdge>>(&mut self, target: T, source: S) {
        let FaceEdge { face, edge } = source.into();
        let n = self.tri[face].neighbors[edge];
        let i = self.tri[n].find_neighbor(face).unwrap();
        self.set_adjacent(target, FaceEdge::new(n, i));
    }

    #[inline]
    pub fn get_vertices_orientation(&self, v0: VertexIndex, v1: VertexIndex, v2: VertexIndex) -> i64 {
        use crate::math::triangulation::predicates::orient2d;
        assert!(self.tri.is_finite_vertex(v0) && self.tri.is_finite_vertex(v1) && self.tri.is_finite_vertex(v2));

        let a = self.tri.p(v0);
        let b = self.tri.p(v1);
        let c = self.tri.p(v2);

        orient2d(a, b, c)
    }

    #[inline]
    pub fn get_edge_vertex_orientation(&self, f: FaceIndex, i: Rot3Idx, v: VertexIndex) -> i64 {
        let va = v;
        let vb = self.tri[f].vertices[i.increment()];
        let vc = self.tri[f].vertices[i.decrement()];
        self.get_vertices_orientation(va, vb, vc)
    }
}
