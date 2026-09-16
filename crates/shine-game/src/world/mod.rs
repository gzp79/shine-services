#![allow(clippy::module_inception)]

mod chunk;
mod chunk_id;
mod corner_cells;
mod edge_cells;
mod generation;
mod inner_cells;
mod layer;
mod world;

pub use self::{
    chunk::{CellIndex, Chunk, ChunkHandle, TileIndex},
    chunk_id::ChunkId,
    corner_cells::{CornerCells, CornerSide},
    edge_cells::{EdgeCells, EdgeSide},
    generation::{Generation, WeakGeneration},
    inner_cells::InnerCells,
    layer::{BaseLayer, Layer},
    world::{WeakWorld, World, CELL_WORLD_SIZE, CHUNK_WORLD_SIZE, SUBDIVISION_BASE, SUBDIVISION_COUNT},
};
