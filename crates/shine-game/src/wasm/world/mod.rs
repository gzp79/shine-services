#![allow(clippy::module_inception)]

mod cell_data;
mod chunk;
mod world;

pub use self::{
    cell_data::{CornerCellsHandle, EdgeCellsHandle, InnerCellsHandle},
    chunk::WasmChunk,
};
