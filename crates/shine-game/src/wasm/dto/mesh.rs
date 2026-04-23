use crate::dto::IndexedMesh;
use wasm_bindgen::prelude::*;

/// WASM wrapper for IndexedMesh - exposes mesh geometry to TypeScript
#[wasm_bindgen]
#[derive(Clone)]
pub struct WasmIndexedMesh(IndexedMesh);

#[wasm_bindgen]
impl WasmIndexedMesh {
    /// Get flat vertex buffer [x,y,x,y,...]
    pub fn vertices(&self) -> Vec<f32> {
        self.0.vertices.clone()
    }

    /// Get polygon index buffer
    pub fn indices(&self) -> Vec<u32> {
        self.0.indices.clone()
    }

    /// Get polygon ranges [start0, end0, start1, end1, ...]
    pub fn polygon_ranges(&self) -> Vec<u32> {
        self.0.polygon_ranges.clone()
    }

    /// Get wire index buffer (empty if no wires)
    pub fn wire_indices(&self) -> Vec<u32> {
        self.0.wire_indices.clone()
    }

    /// Get wire ranges [start0, end0, start1, end1, ...]
    pub fn wire_ranges(&self) -> Vec<u32> {
        self.0.wire_ranges.clone()
    }

    /// Check if mesh has wire data
    pub fn has_wires(&self) -> bool {
        !self.0.wire_indices.is_empty()
    }
}

impl From<IndexedMesh> for WasmIndexedMesh {
    fn from(mesh: IndexedMesh) -> Self {
        Self(mesh)
    }
}

impl From<WasmIndexedMesh> for IndexedMesh {
    fn from(wasm: WasmIndexedMesh) -> Self {
        wasm.0
    }
}
