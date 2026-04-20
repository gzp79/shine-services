use crate::{
    indexed::{IdxArray, IdxVec, TypedIndex},
    math::triangulation::{
        CrossingIterator, EdgeCirculator, FaceClue, FaceEdge, Rot3Idx, TriangulationBuilder, Validator, VertexClue,
    },
};
use glam::IVec2;
use std::{
    cell::RefCell,
    fmt,
    ops::{Index, IndexMut},
    rc::Rc,
};

crate::define_typed_index!(VertexIndex, "Typed index into a vertex array.");
crate::define_typed_index!(FaceIndex, "Typed index into a triangle array.");

pub struct Vertex {
    pub position: IVec2,
    pub face: FaceIndex,
}

impl Vertex {
    pub fn new() -> Self {
        Self {
            position: IVec2::ZERO,
            face: FaceIndex::NONE,
        }
    }
}

pub struct Face {
    pub vertices: IdxArray<Rot3Idx, VertexIndex, 3>,
    pub neighbors: IdxArray<Rot3Idx, FaceIndex, 3>,
    pub constraints: IdxArray<Rot3Idx, u32, 3>,
    pub tag: usize,
}

impl Face {
    pub fn new() -> Self {
        Self {
            vertices: IdxArray::from_elem(VertexIndex::NONE),
            neighbors: IdxArray::from_elem(FaceIndex::NONE),
            constraints: IdxArray::from_elem(0),
            tag: 0,
        }
    }

    pub fn with_vertices(a: VertexIndex, b: VertexIndex, c: VertexIndex) -> Self {
        Self {
            vertices: IdxArray::from([a, b, c]),
            neighbors: IdxArray::from_elem(FaceIndex::NONE),
            constraints: IdxArray::from_elem(0),
            tag: 0,
        }
    }

    pub fn find_vertex(&self, v: VertexIndex) -> Option<Rot3Idx> {
        self.vertices.iter().position(|&x| x == v).map(|i| Rot3Idx::new(i))
    }

    pub fn find_neighbor(&self, f: FaceIndex) -> Option<Rot3Idx> {
        self.neighbors.iter().position(|&x| x == f).map(|i| Rot3Idx::new(i))
    }
}

/// Store the topology graph of the triangualation
pub struct Triangulation<const DELAUNAY: bool = true> {
    pub(in crate::math::triangulation) dimension: u8,
    pub(in crate::math::triangulation) infinite_vertex: VertexIndex,
    pub(in crate::math::triangulation) vertices: IdxVec<VertexIndex, Vertex>,
    pub(in crate::math::triangulation) faces: IdxVec<FaceIndex, Face>,
    tag: Rc<RefCell<usize>>,
}

impl<const DELAUNAY: bool> Triangulation<DELAUNAY> {
    pub fn new() -> Self {
        Triangulation {
            dimension: u8::MAX,
            infinite_vertex: VertexIndex::NONE,
            vertices: Default::default(),
            faces: Default::default(),
            tag: Rc::new(RefCell::new(0)),
        }
    }
}

impl Triangulation<false> {
    pub fn new_ct() -> Self {
        Self::new()
    }
}

impl Triangulation<true> {
    pub fn new_cdt() -> Self {
        Self::new()
    }
}

impl<const DELAUNAY: bool> Triangulation<DELAUNAY> {
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.dimension == u8::MAX
    }

    #[inline]
    pub fn dimension(&self) -> u8 {
        self.dimension
    }

    pub fn clear(&mut self) {
        self.dimension = u8::MAX;
        self.infinite_vertex = VertexIndex::NONE;
        self.faces.clear();
        self.vertices.clear();
        self.tag.replace(0);
    }

    pub fn edge_circulator(&self, vertex: VertexIndex) -> EdgeCirculator<'_, DELAUNAY> {
        EdgeCirculator::new(self, vertex)
    }

    pub fn crossing_iterator(&self, v0: VertexIndex, v1: VertexIndex) -> CrossingIterator<'_, DELAUNAY> {
        CrossingIterator::new(self, v0, v1)
    }

    pub fn validator(&self) -> Validator<'_, DELAUNAY> {
        Validator::new(self)
    }

    pub fn builder(&mut self) -> TriangulationBuilder<'_, DELAUNAY> {
        TriangulationBuilder::new(self)
    }

    pub fn scope_guard(&self) -> Rc<RefCell<usize>> {
        self.tag.clone()
    }

    pub fn store_vertex(&mut self, vert: Vertex) -> VertexIndex {
        let id = self.vertices.len();
        self.vertices.push(vert);
        VertexIndex::new(id)
    }

    #[inline]
    pub fn vi<T: Into<VertexClue>>(&self, id: T) -> VertexIndex {
        let clue: VertexClue = id.into();
        match clue {
            VertexClue::VertexIndex(vi) => vi,
            VertexClue::FaceVertex(face, vertex) => self.faces[face].vertices[vertex],
            VertexClue::EdgeStart(face, edge) => self.faces[face].vertices[edge.increment()],
            VertexClue::EdgeEnd(face, edge) => self.faces[face].vertices[edge.decrement()],
        }
    }

    #[inline]
    pub fn vertex_count(&self) -> usize {
        self.vertices.len()
    }

    #[inline]
    pub fn vertex_iter(&self) -> impl Iterator<Item = &Vertex> + '_ {
        self.vertices.iter()
    }

    pub fn vertex_index_iter(&self) -> impl Iterator<Item = VertexIndex> {
        VertexIndex::range(VertexIndex::new(0), VertexIndex::new(self.vertices.len()))
    }

    #[inline]
    pub fn infinite_vertex(&self) -> VertexIndex {
        self.infinite_vertex
    }

    #[inline]
    pub fn is_infinite_vertex(&self, v: VertexIndex) -> bool {
        assert!(!self.is_empty());
        v == self.infinite_vertex
    }

    #[inline]
    pub fn is_finite_vertex(&self, v: VertexIndex) -> bool {
        !self.is_infinite_vertex(v)
    }

    pub fn store_face(&mut self, face: Face) -> FaceIndex {
        let id = self.faces.len();
        self.faces.push(face);
        FaceIndex::new(id)
    }

    #[inline]
    pub fn fi<T: Into<FaceClue>>(&self, id: T) -> FaceIndex {
        let clue: FaceClue = id.into();
        match clue {
            FaceClue::FaceIndex(fi) => fi,
        }
    }

    #[inline]
    pub fn face_count(&self) -> usize {
        self.faces.len()
    }

    #[inline]
    pub fn face_iter(&self) -> impl Iterator<Item = &Face> + '_ {
        self.faces.iter()
    }

    #[inline]
    pub fn face_index_iter(&self) -> impl Iterator<Item = FaceIndex> {
        FaceIndex::range(FaceIndex::new(0), FaceIndex::new(self.faces.len()))
    }

    #[inline]
    pub fn infinite_face(&self) -> FaceIndex {
        self.vertices[self.infinite_vertex].face
    }

    #[inline]
    pub fn is_infinite_face(&self, f: FaceIndex) -> bool {
        assert!(!self.is_empty());
        self.faces[f].find_vertex(self.infinite_vertex).is_some()
    }

    #[inline]
    pub fn is_finite_face(&self, f: FaceIndex) -> bool {
        !self.is_infinite_face(f)
    }

    #[inline]
    pub fn p<T: Into<VertexClue>>(&self, v: T) -> IVec2 {
        let vi = self.vi(v);
        self[vi].position
    }

    #[inline]
    pub fn c<T: Into<FaceEdge>>(&self, edge: T) -> u32 {
        let edge: FaceEdge = edge.into();
        self[edge.face].constraints[edge.edge]
    }
}

impl<const DELAUNAY: bool> Index<VertexIndex> for Triangulation<DELAUNAY> {
    type Output = Vertex;

    #[inline]
    fn index(&self, v: VertexIndex) -> &Self::Output {
        &self.vertices[v]
    }
}

impl<const DELAUNAY: bool> IndexMut<VertexIndex> for Triangulation<DELAUNAY> {
    #[inline]
    fn index_mut(&mut self, v: VertexIndex) -> &mut Self::Output {
        &mut self.vertices[v]
    }
}

impl<const DELAUNAY: bool> Index<VertexClue> for Triangulation<DELAUNAY> {
    type Output = Vertex;

    #[inline]
    fn index(&self, v: VertexClue) -> &Self::Output {
        &self.vertices[self.vi(v)]
    }
}

impl<const DELAUNAY: bool> IndexMut<VertexClue> for Triangulation<DELAUNAY> {
    #[inline]
    fn index_mut(&mut self, v: VertexClue) -> &mut Self::Output {
        let vi = self.vi(v);
        &mut self.vertices[vi]
    }
}

impl<const DELAUNAY: bool> Index<FaceIndex> for Triangulation<DELAUNAY> {
    type Output = Face;

    #[inline]
    fn index(&self, f: FaceIndex) -> &Self::Output {
        &self.faces[f]
    }
}

impl<const DELAUNAY: bool> IndexMut<FaceIndex> for Triangulation<DELAUNAY> {
    #[inline]
    fn index_mut(&mut self, f: FaceIndex) -> &mut Self::Output {
        &mut self.faces[f]
    }
}

impl<const DELAUNAY: bool> Index<FaceClue> for Triangulation<DELAUNAY> {
    type Output = Face;

    #[inline]
    fn index(&self, v: FaceClue) -> &Self::Output {
        &self.faces[self.fi(v)]
    }
}

impl<const DELAUNAY: bool> IndexMut<FaceClue> for Triangulation<DELAUNAY> {
    #[inline]
    fn index_mut(&mut self, v: FaceClue) -> &mut Self::Output {
        let fi = self.fi(v);
        &mut self.faces[fi]
    }
}

impl<const DELAUNAY: bool> fmt::Debug for Triangulation<DELAUNAY> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Tri {{ V[ ")?;
        for v in self.vertex_index_iter() {
            if self.is_infinite_vertex(v) {
                write!(f, "*")?;
            }
            let p = self.vertices[v].position;
            write!(f, "{:?}:({},{}), ", v, p.x, p.y)?;
        }
        writeln!(f, "]")?;

        write!(f, "VF[ ")?;
        for v in self.vertex_index_iter() {
            write!(f, "{:?}->{:?}, ", v, self.vertices[v].face)?;
        }
        writeln!(f, "]")?;

        write!(f, "FV[ ")?;
        for t in self.face_index_iter() {
            if self.is_infinite_face(t) {
                write!(f, "*")?;
            }
            write!(
                f,
                "{:?}->({:?},{:?},{:?}), ",
                t,
                self.faces[t].vertices[Rot3Idx::new(0)],
                self.faces[t].vertices[Rot3Idx::new(1)],
                self.faces[t].vertices[Rot3Idx::new(2)]
            )?;
        }
        writeln!(f, "]")?;

        write!(f, "FN[ ")?;
        for t in self.face_index_iter() {
            if self.is_infinite_face(t) {
                write!(f, "*")?;
            }
            write!(
                f,
                "{:?}->({:?},{:?},{:?}), ",
                t,
                self.faces[t].neighbors[Rot3Idx::new(0)],
                self.faces[t].neighbors[Rot3Idx::new(1)],
                self.faces[t].neighbors[Rot3Idx::new(2)]
            )?;
        }
        writeln!(f, "] }}")
    }
}
