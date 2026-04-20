use crate::math::quadrangulation::{QuadIndex, Rot4Idx, VertexIndex};

/// A vertex referenced by its containing quad and local vertex index (0..4)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuadVertex {
    pub quad: QuadIndex,
    pub local: Rot4Idx,
}

impl QuadVertex {
    /// Next vertex CCW around this quad
    pub fn next(&self) -> QuadVertex {
        QuadVertex {
            quad: self.quad,
            local: self.local.increment(),
        }
    }

    /// Previous vertex CCW around this quad
    pub fn prev(&self) -> QuadVertex {
        QuadVertex {
            quad: self.quad,
            local: self.local.decrement(),
        }
    }

    /// Opposite vertex across the quad
    pub fn opposite(&self) -> QuadVertex {
        QuadVertex {
            quad: self.quad,
            local: self.local.increment().increment(),
        }
    }

    /// Edge leaving this vertex (outgoing)
    pub fn outgoing_edge(&self) -> QuadEdge {
        QuadEdge {
            quad: self.quad,
            edge: self.local,
        }
    }

    /// Edge entering this vertex (incoming)
    pub fn incoming_edge(&self) -> QuadEdge {
        QuadEdge {
            quad: self.quad,
            edge: self.local.decrement(),
        }
    }
}

/// An edge referenced by its containing quad and local edge index (0..4)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuadEdge {
    pub quad: QuadIndex,
    pub edge: Rot4Idx,
}

impl QuadEdge {
    pub fn new(quad: QuadIndex, edge: Rot4Idx) -> Self {
        Self { quad, edge }
    }

    /// QuadVertex at the start of this edge
    pub fn start(&self) -> QuadVertex {
        QuadVertex {
            quad: self.quad,
            local: self.edge,
        }
    }

    /// QuadVertex at the end of this edge
    pub fn end(&self) -> QuadVertex {
        QuadVertex {
            quad: self.quad,
            local: self.edge.increment(),
        }
    }
}

/// References a vertex in the quadrangulation, used for topology queries
#[derive(Clone, Debug)]
pub enum VertexClue {
    VertexIndex(VertexIndex),
    QuadVertex(QuadIndex, Rot4Idx),
    EdgeStart(QuadIndex, Rot4Idx),
    EdgeEnd(QuadIndex, Rot4Idx),
}

impl VertexClue {
    pub fn quad_vertex(q: QuadIndex, v: Rot4Idx) -> VertexClue {
        VertexClue::QuadVertex(q, v)
    }

    pub fn edge_start(q: QuadIndex, e: Rot4Idx) -> VertexClue {
        VertexClue::EdgeStart(q, e)
    }

    pub fn edge_end(q: QuadIndex, e: Rot4Idx) -> VertexClue {
        VertexClue::EdgeEnd(q, e)
    }

    pub fn start_of(e: QuadEdge) -> VertexClue {
        VertexClue::EdgeStart(e.quad, e.edge)
    }

    pub fn end_of(e: QuadEdge) -> VertexClue {
        VertexClue::EdgeEnd(e.quad, e.edge)
    }
}

impl From<VertexIndex> for VertexClue {
    fn from(v: VertexIndex) -> VertexClue {
        VertexClue::VertexIndex(v)
    }
}

impl From<QuadVertex> for VertexClue {
    fn from(v: QuadVertex) -> VertexClue {
        VertexClue::QuadVertex(v.quad, v.local)
    }
}

/// References a quad in the quadrangulation, used for topology queries
#[derive(Clone, Debug)]
pub enum QuadClue {
    QuadIndex(QuadIndex),
}

impl From<QuadIndex> for QuadClue {
    fn from(q: QuadIndex) -> QuadClue {
        QuadClue::QuadIndex(q)
    }
}

/// Classification of an edge in the quad mesh.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuadEdgeType {
    /// Edge is shared by two finite (non-infinite) quads
    Interior,
    /// Edge is on the boundary (shared with an infinite quad)
    Boundary,
    /// The two vertices don't form an edge in the mesh
    NotAnEdge,
}
