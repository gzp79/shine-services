use crate::dto::IndexedMesh;
use js_sys::{Float32Array, Uint32Array};
use wasm_bindgen::prelude::*;

/// Zero-copy WASM view over an IndexedMesh.
/// All accessors return views into Wasm linear memory — clone on the JS side
/// (e.g. `arr.slice()`) if the data must outlive this object or any further Wasm call.
#[wasm_bindgen]
#[derive(Clone)]
pub struct IndexedMeshHandle(IndexedMesh);

#[wasm_bindgen]
impl IndexedMeshHandle {
    pub fn vertices(&self) -> Float32Array {
        unsafe { Float32Array::view(&self.0.vertices) }
    }

    pub fn indices(&self) -> Uint32Array {
        unsafe { Uint32Array::view(&self.0.indices) }
    }

    pub fn polygon_ranges(&self) -> Uint32Array {
        unsafe { Uint32Array::view(&self.0.polygon_ranges) }
    }

    pub fn wire_indices(&self) -> Uint32Array {
        unsafe { Uint32Array::view(&self.0.wire_indices) }
    }

    pub fn wire_ranges(&self) -> Uint32Array {
        unsafe { Uint32Array::view(&self.0.wire_ranges) }
    }

    pub fn has_wires(&self) -> bool {
        !self.0.wire_indices.is_empty()
    }
}

impl From<IndexedMesh> for IndexedMeshHandle {
    fn from(mesh: IndexedMesh) -> Self {
        Self(mesh)
    }
}

impl From<IndexedMeshHandle> for IndexedMesh {
    fn from(view: IndexedMeshHandle) -> Self {
        view.0
    }
}
