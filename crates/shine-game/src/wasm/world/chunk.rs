use crate::{
    indexed::TypedIndex,
    math::quadrangulation::AnchorIndex,
    wasm::{
        math::{WasmHexFlatDir, WasmHexPointyDir},
        world::{WasmChangeLog, WasmCornerCells, WasmEdgeCells, WasmInnerCells},
    },
    world::ChunkHandle,
};
use wasm_bindgen::prelude::*;

/// Handle to a loaded chunk. Wraps a core `ChunkHandle`, which holds only weak references into
/// the world and revalidates on every access, so a stale handle returns `undefined` instead of
/// reading moved or freed data.
#[wasm_bindgen(js_name = "Chunk")]
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
    pub fn inner_cells(&self) -> Option<WasmInnerCells> {
        self.handle.inner_cells().map(Into::into)
    }

    pub fn edge_cells(&self, edge_idx: WasmHexFlatDir) -> Option<WasmEdgeCells> {
        self.handle.edge_cells(edge_idx.into()).map(Into::into)
    }

    pub fn corner_cells(&self, corner_idx: WasmHexPointyDir) -> Option<WasmCornerCells> {
        self.handle.corner_cells(corner_idx.into()).map(Into::into)
    }

    /// Applies the update operation to the chunk's base layer and returns a
    /// read-only `ChangeLog` over the result immediately. The base layer is locked for any other
    /// consumer until the returned handle is dropped, which unlocks
    /// it. Returns `undefined` if the handle is stale or the layer is already locked.
    pub fn update(&self /*, update: TBD */) -> Option<WasmChangeLog> {
        self.handle.update(|_| {}).map(Into::into)
    }

    /// Get changes since the last update to the chunk's base layer and returns a
    /// read-only `ChangeLog` over the result immediately. The base layer is locked for any other
    /// consumer until the returned handle is dropped.
    pub fn changes(&self) -> Option<WasmChangeLog> {
        self.handle.update(|_| {}).map(Into::into)
    }

    /// The 6 hexagon boundary corners in chunk-local space as 12 floats `[x, y, ...]`, or
    /// `undefined` if the handle is stale.
    pub fn hex_vertices(&self) -> Option<Vec<f32>> {
        self.handle.with_chunk(|chunk| {
            let mut vertices = Vec::with_capacity(12);
            for i in 0..6 {
                let vi = chunk.mesh().anchor_vertex(AnchorIndex::new(i));
                let p = chunk.mesh().p(vi);
                vertices.push(p.x);
                vertices.push(p.y);
            }
            vertices
        })
    }
}
