use crate::math::triangulation::{FaceIndex, Rot3Idx, VertexIndex};

/// Selection of a vertex by a triangle and an index
#[derive(Clone, Copy, Debug)]
pub struct FaceVertex {
    pub triangle: FaceIndex,
    pub vertex: Rot3Idx,
}

impl FaceVertex {
    pub fn new(f: FaceIndex, v: Rot3Idx) -> FaceVertex {
        FaceVertex { triangle: f, vertex: v }
    }
}

impl From<(FaceIndex, Rot3Idx)> for FaceVertex {
    fn from(value: (FaceIndex, Rot3Idx)) -> Self {
        FaceVertex::new(value.0, value.1)
    }
}

/// Selection of an edge by a triangle and an index
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FaceEdge {
    pub triangle: FaceIndex,
    pub edge: Rot3Idx,
}

impl FaceEdge {
    pub fn new(f: FaceIndex, e: Rot3Idx) -> FaceEdge {
        FaceEdge { triangle: f, edge: e }
    }

    pub fn next(&self) -> FaceEdge {
        FaceEdge::new(self.triangle, self.edge.increment())
    }

    pub fn prev(&self) -> FaceEdge {
        FaceEdge::new(self.triangle, self.edge.decrement())
    }
}

impl From<(FaceIndex, Rot3Idx)> for FaceEdge {
    fn from(value: (FaceIndex, Rot3Idx)) -> Self {
        FaceEdge::new(value.0, value.1)
    }
}

/// References a vertex in the triangulation, used for topology queries
#[derive(Clone, Debug)]
pub enum VertexClue {
    VertexIndex(VertexIndex),
    FaceVertex(FaceIndex, Rot3Idx),
    EdgeStart(FaceIndex, Rot3Idx),
    EdgeEnd(FaceIndex, Rot3Idx),
}

impl VertexClue {
    pub fn face_vertex(f: FaceIndex, v: Rot3Idx) -> VertexClue {
        VertexClue::FaceVertex(f, v)
    }

    pub fn edge_start(f: FaceIndex, e: Rot3Idx) -> VertexClue {
        VertexClue::EdgeStart(f, e)
    }

    pub fn edge_end(f: FaceIndex, e: Rot3Idx) -> VertexClue {
        VertexClue::EdgeEnd(f, e)
    }

    pub fn start_of(e: FaceEdge) -> VertexClue {
        VertexClue::EdgeStart(e.triangle, e.edge)
    }

    pub fn end_of(e: FaceEdge) -> VertexClue {
        VertexClue::EdgeEnd(e.triangle, e.edge)
    }
}

impl From<VertexIndex> for VertexClue {
    fn from(v: VertexIndex) -> VertexClue {
        VertexClue::VertexIndex(v)
    }
}

impl From<FaceVertex> for VertexClue {
    fn from(v: FaceVertex) -> VertexClue {
        VertexClue::FaceVertex(v.triangle, v.vertex)
    }
}

/// References a triangle in the triangulation, used for topology queries
#[derive(Clone, Debug)]
pub enum FaceClue {
    FaceIndex(FaceIndex),
}

impl From<FaceIndex> for FaceClue {
    fn from(f: FaceIndex) -> FaceClue {
        FaceClue::FaceIndex(f)
    }
}
