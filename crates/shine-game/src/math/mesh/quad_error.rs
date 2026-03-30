use thiserror::Error as ThisError;

#[derive(Debug, ThisError)]
pub enum QuadTopologyError {
    #[error("Boundary must have even length, got {0}")]
    OddBoundary(usize),

    #[error("Boundary vertex {vertex} >= vertex_count {vertex_count}")]
    BoundaryVertexOutOfRange { vertex: usize, vertex_count: usize },

    #[error("Duplicate boundary vertex {0}")]
    DuplicateBoundaryVertex(usize),

    #[error("Quad vertex {vertex} >= vertex_count {vertex_count}")]
    QuadVertexOutOfRange { vertex: usize, vertex_count: usize },

    #[error("Quad references ghost vertex at index {0}")]
    QuadReferencesGhost(usize),

    #[error("Incomplete topology: quad {quad} edge {edge} ({vertices:?}) has no neighbor")]
    IncompleteTopology {
        quad: usize,
        edge: usize,
        vertices: (usize, usize),
    },

    #[error("Vertex {0} has no associated quad")]
    VertexHasNoQuad(usize),

    #[error("Quad {quad} edge {edge} has invalid twin: twin edge does not point back")]
    InvalidEdgeTwin { quad: usize, edge: usize },

    #[error("Ghost quad {quad} has {count} ghost vertices (expected 1)")]
    InvalidGhostQuadStructure { quad: usize, count: usize },

    #[error("Vertex {vertex} ring traversal does not form a closed loop")]
    VertexRingNotClosed { vertex: usize },
}
