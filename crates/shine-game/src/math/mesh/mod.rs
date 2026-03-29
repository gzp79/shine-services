//! Quad mesh data structures and filters.
//!
//! This module provides a topologically closed quad mesh representation using ghost
//! vertices and ghost quads to enable consistent CCW navigation around all vertices,
//! including boundary vertices.
//!
//! ## Core Types
//!
//! - [`QuadMesh`]: Quad mesh with positions and topology
//! - [`QuadTopology`]: Pure topology (no positions) with ghost vertex/quads
//! - [`QuadEdge`], [`QuadVertex`]: Navigation types for CCW traversal
//! - [`EdgeType`]: Classification of edges (Interior/Boundary/NotAnEdge)
//!
//! ## Filters
//!
//! - [`QuadRelax`]: Laplacian smoothing for quad meshes
//! - [`VertexRepulsion`]: Edge-length and diagonal-length equalization
//! - [`Jitter`]: Random vertex perturbation
//! - [`LaplacianSmoother`]: Standard Laplacian smoothing

pub mod filter;
mod quad_error;
mod quad_mesh;
mod quad_topology;

pub use self::{
    filter::{Jitter, LaplacianSmoother, QuadFilter, QuadRelax, VertexRepulsion},
    quad_error::QuadTopologyError,
    quad_mesh::QuadMesh,
    quad_topology::{EdgeType, QuadEdge, QuadIdx, QuadTopology, QuadVertex, VertIdx},
};
