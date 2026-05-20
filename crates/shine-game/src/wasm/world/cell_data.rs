use crate::world::{CornerCells, EdgeCells, InnerCells};
use js_sys::{Float32Array, Uint32Array};
use wasm_bindgen::prelude::*;

/// Zero-copy WASM view over InnerCells.
/// All accessors return views into Wasm linear memory — clone on the JS side
#[wasm_bindgen]
pub struct InnerCellsHandle(InnerCells);

#[wasm_bindgen]
impl InnerCellsHandle {
    pub fn vertices(&self) -> Float32Array {
        unsafe { Float32Array::view(&self.0.vertices) }
    }

    pub fn indices(&self) -> Uint32Array {
        unsafe { Uint32Array::view(&self.0.indices) }
    }

    pub fn polygon_ranges(&self) -> Uint32Array {
        unsafe { Uint32Array::view(&self.0.polygon_ranges) }
    }

    pub fn sites(&self) -> Uint32Array {
        unsafe { Uint32Array::view(&self.0.sites) }
    }
}

impl From<InnerCells> for InnerCellsHandle {
    fn from(data: InnerCells) -> Self {
        Self(data)
    }
}

/// Zero-copy WASM view over EdgeCells.
/// All accessors return views into Wasm linear memory — clone on the JS side
#[wasm_bindgen]
pub struct EdgeCellsHandle(EdgeCells);

#[wasm_bindgen]
impl EdgeCellsHandle {
    pub fn vertices(&self) -> Float32Array {
        unsafe { Float32Array::view(&self.0.vertices) }
    }

    pub fn indices(&self) -> Uint32Array {
        unsafe { Uint32Array::view(&self.0.indices) }
    }

    pub fn polygon_ranges(&self) -> Uint32Array {
        unsafe { Uint32Array::view(&self.0.polygon_ranges) }
    }

    pub fn owner_sites(&self) -> Uint32Array {
        unsafe { Uint32Array::view(&self.0.owner_sites) }
    }

    pub fn neighbor_sites(&self) -> Uint32Array {
        unsafe { Uint32Array::view(&self.0.neighbor_sites) }
    }
}

impl From<EdgeCells> for EdgeCellsHandle {
    fn from(data: EdgeCells) -> Self {
        Self(data)
    }
}

/// Zero-copy WASM view over CornerCells.
/// All accessors return views into Wasm linear memory — clone on the JS side
#[wasm_bindgen]
pub struct CornerCellsHandle(CornerCells);

#[wasm_bindgen]
impl CornerCellsHandle {
    pub fn vertices(&self) -> Float32Array {
        unsafe { Float32Array::view(&self.0.vertices) }
    }

    pub fn owner_site(&self) -> u32 {
        self.0.owner_site
    }

    pub fn cw_site(&self) -> u32 {
        self.0.cw_site
    }

    pub fn ccw_site(&self) -> u32 {
        self.0.ccw_site
    }
}

impl From<CornerCells> for CornerCellsHandle {
    fn from(data: CornerCells) -> Self {
        Self(data)
    }
}
