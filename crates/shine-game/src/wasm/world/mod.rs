#![allow(clippy::module_inception)]

mod cell_data;
mod chunk;
mod world;

pub use self::{
    cell_data::{WasmCornerCells, WasmEdgeCells, WasmInnerCells},
    chunk::WasmChunk,
};
