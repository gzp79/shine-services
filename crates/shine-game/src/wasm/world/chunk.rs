use crate::{
    wasm::{
        math::{HexFlatDir, HexPointyDir},
        world::{CornerCellsHandle, EdgeCellsHandle, InnerCellsHandle},
    },
    world::ChunkHandle,
};
use wasm_bindgen::prelude::*;

/// Handle to a loaded chunk. Wraps a core `ChunkHandle`, which holds only weak references into
/// the world and revalidates on every access, so a stale handle returns `undefined` instead of
/// reading moved or freed data.
#[wasm_bindgen]
pub struct WasmChunk {
    handle: ChunkHandle,
}

impl WasmChunk {
    pub(crate) fn new(handle: ChunkHandle) -> Self {
        Self { handle }
    }
}

#[wasm_bindgen]
impl WasmChunk {
    pub fn inner_cells(&self) -> Option<InnerCellsHandle> {
        self.handle.inner_cells().map(Into::into)
    }

    pub fn edge_cells(&self, edge_idx: HexFlatDir) -> Option<EdgeCellsHandle> {
        self.handle.edge_cells(edge_idx.into()).map(Into::into)
    }

    pub fn corner_cells(&self, corner_idx: HexPointyDir) -> Option<CornerCellsHandle> {
        self.handle.corner_cells(corner_idx.into()).map(Into::into)
    }
}
