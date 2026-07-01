mod builder;
mod mutations;
mod predicates;
mod query;
mod rot3_index;
mod triangulation;
mod types;
mod validation;

pub use self::{
    builder::TriangulationBuilder,
    query::{Crossing, CrossingIterator, EdgeCirculator, Location},
    rot3_index::Rot3Idx,
    triangulation::{FaceIndex, Triangle, Triangulation, Vertex, VertexIndex},
    types::{FaceClue, FaceEdge, FaceVertex, VertexClue},
    validation::{debug, Validator},
};
