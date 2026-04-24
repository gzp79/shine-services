use thiserror::Error as ThisError;

#[derive(Debug, ThisError, PartialEq, Eq)]
pub enum QuadError {
    #[error("Boundary must have even length, got {0}")]
    OddBoundary(usize),

    #[error("Boundary vertex {vertex} >= vertex_count {vertex_count}")]
    BoundaryVertexOutOfRange { vertex: usize, vertex_count: usize },

    #[error("Duplicate boundary vertex {0}")]
    DuplicateBoundaryVertex(usize),

    #[error("Quad vertex {vertex} >= vertex_count {vertex_count}")]
    QuadVertexOutOfRange { vertex: usize, vertex_count: usize },

    #[error("Quad references infinite vertex at index {0}")]
    QuadReferencesInfinite(usize),

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

    #[error("Infinite quad {quad} has {count} infinite vertices (expected 1)")]
    InvalidInfiniteQuadStructure { quad: usize, count: usize },

    #[error("infinite_quad_count mismatch: field says {expected}, actual {actual}")]
    InfiniteQuadCountMismatch { expected: usize, actual: usize },

    #[error("Vertex {vertex} ring traversal does not form a closed loop")]
    VertexRingNotClosed { vertex: usize },

    #[error("Anchor edge {edge} is not a subsequence of boundary vertices")]
    InvalidAnchorEdge { edge: usize },

    #[error("vertex_quad[{vertex}] references vertex {actual} instead of {vertex}")]
    VertexQuadMismatch { vertex: usize, actual: usize },

    #[error("Quad {quad} has duplicate vertex {vertex}")]
    DegenerateQuad { quad: usize, vertex: usize },

    #[error("Quad {quad} is not reachable from any vertex ring")]
    UnreachableQuad { quad: usize },

    #[error("Edge twin is not an involution: quad {quad} edge {edge}")]
    EdgeTwinNotInvolution { quad: usize, edge: usize },

    #[error("Infinite quads are not compact: infinite quad {infinite_quad} precedes finite quad {finite_quad}")]
    InfiniteQuadsNotCompact { infinite_quad: usize, finite_quad: usize },

    #[error("Quad {quad} has negative or zero area (non-CCW winding)")]
    NegativeQuadArea { quad: usize },

    #[error("Self-intersection detected between quad {quad1} and quad {quad2}")]
    SelfIntersection { quad1: usize, quad2: usize },

    #[error("Invalid hexagon: {0}")]
    InvalidHexagon(String),
}
