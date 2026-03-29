//! Error types for quad mesh topology construction and validation.

use thiserror::Error as ThisError;

/// Errors that can occur during quad topology construction.
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
}
