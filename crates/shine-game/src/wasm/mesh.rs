use crate::dto::IndexedMesh;
use wasm_bindgen::prelude::*;

/// WASM wrapper for IndexedMesh - exposes mesh geometry to TypeScript
#[wasm_bindgen]
pub struct WasmIndexedMesh(IndexedMesh);

#[wasm_bindgen]
impl WasmIndexedMesh {
    /// Create a new mesh with vertices, indices, and polygon section markers
    #[wasm_bindgen(constructor)]
    pub fn new(vertices: Vec<f32>, indices: Vec<u32>, polygon_starts: Vec<u32>) -> Self {
        Self(IndexedMesh::new(vertices, indices, polygon_starts))
    }

    /// Get flat vertex buffer [x,y,x,y,...]
    pub fn vertices(&self) -> Vec<f32> {
        self.0.vertices.clone()
    }

    /// Get polygon index buffer
    pub fn indices(&self) -> Vec<u32> {
        self.0.indices.clone()
    }

    /// Get polygon section start offsets
    pub fn polygon_starts(&self) -> Vec<u32> {
        self.0.polygon_starts.clone()
    }

    /// Get wire index buffer (empty if no wires)
    pub fn wire_indices(&self) -> Vec<u32> {
        self.0.wire_indices.clone()
    }

    /// Get wire section start offsets (empty if no wires)
    pub fn wire_starts(&self) -> Vec<u32> {
        self.0.wire_starts.clone()
    }

    /// Set wire data (indices and section starts)
    pub fn set_wires(&mut self, wire_indices: Vec<u32>, wire_starts: Vec<u32>) {
        self.0.wire_indices = wire_indices;
        self.0.wire_starts = wire_starts;
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
