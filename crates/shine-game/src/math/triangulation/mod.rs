//! Constrained Delaunay Triangulation
//!
//! ## Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────┐
//! │ Triangulation                                   │
//! │  - Core data structure (vertices, faces)        │
//! │  - Primitives: split_edge, flip (mutations/)    │
//! │  - Queries: edge_circulator, twin_edge (query/) │
//! └─────────────────────────────────────────────────┘
//!                        ▲
//!                        │ uses
//!                        │
//! ┌─────────────────────────────────────────────────┐
//! │ BuilderState (builder/state.rs)                 │
//! │  - Work buffers for algorithms                  │
//! │  - Delaunay: delaunay_* (builder/delaunay.rs)   │
//! │  - Constraints: add_constraint_* (constraints.rs)│
//! └─────────────────────────────────────────────────┘
//!                        ▲
//!                        │ coordinates
//!                        │
//! ┌─────────────────────────────────────────────────┐
//! │ TriangulationBuilder (builder/builder.rs)       │
//! │  - Public API: add_vertex, add_contour, etc.    │
//! │  - Entry point: tri.builder()                   │
//! └─────────────────────────────────────────────────┘
//! ```

mod builder;
mod check;
mod mutations;
mod predicates;
mod query;
mod rot3_index;
mod triangulation;

pub use self::{
    builder::TriangulationBuilder,
    check::{GeometryChecker, TopologyChecker},
    query::{Crossing, CrossingIterator, EdgeCirculator, Location},
    rot3_index::Rot3Idx,
    triangulation::{Face, FaceClue, FaceEdge, FaceIndex, FaceVertex, Triangulation, Vertex, VertexClue, VertexIndex},
};
