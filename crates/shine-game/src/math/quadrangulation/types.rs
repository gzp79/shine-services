use crate::math::quadrangulation::{QuadIdx, VertIdx};

/// A quad with its local edge index (0..4)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuadEdge {
    pub quad: QuadIdx,
    pub edge: u8,
}

impl QuadEdge {
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
            local: (self.edge + 1) % 4,
        }
    }
}

/// A quad with a vertex's local position (0..4) within it
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuadVertex {
    pub quad: QuadIdx,
    pub local: u8,
}

impl QuadVertex {
    /// Next vertex CCW around this quad
    pub fn next(&self) -> QuadVertex {
        QuadVertex {
            quad: self.quad,
            local: (self.local + 1) % 4,
        }
    }

    /// Previous vertex CCW around this quad
    pub fn prev(&self) -> QuadVertex {
        QuadVertex {
            quad: self.quad,
            local: (self.local + 3) % 4,
        }
    }

    /// Opposite vertex across the quad
    pub fn opposite(&self) -> QuadVertex {
        QuadVertex {
            quad: self.quad,
            local: (self.local + 2) % 4,
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
            edge: (self.local + 3) % 4,
        }
    }
}

/// References a vertex in the quadrangulation, used for topology queries
#[derive(Clone, Debug)]
pub enum VertexClue {
    VertexIndex(VertIdx),
    QuadVertex(QuadIdx, u8),
    EdgeStart(QuadIdx, u8),
    EdgeEnd(QuadIdx, u8),
}

impl VertexClue {
    pub fn quad_vertex(q: QuadIdx, v: u8) -> VertexClue {
        VertexClue::QuadVertex(q, v)
    }

    pub fn edge_start(q: QuadIdx, e: u8) -> VertexClue {
        VertexClue::EdgeStart(q, e)
    }

    pub fn edge_end(q: QuadIdx, e: u8) -> VertexClue {
        VertexClue::EdgeEnd(q, e)
    }

    pub fn start_of(e: QuadEdge) -> VertexClue {
        VertexClue::EdgeStart(e.quad, e.edge)
    }

    pub fn end_of(e: QuadEdge) -> VertexClue {
        VertexClue::EdgeEnd(e.quad, e.edge)
    }
}

impl From<VertIdx> for VertexClue {
    fn from(v: VertIdx) -> VertexClue {
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
    QuadIndex(QuadIdx),
}

impl From<QuadIdx> for QuadClue {
    fn from(q: QuadIdx) -> QuadClue {
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
