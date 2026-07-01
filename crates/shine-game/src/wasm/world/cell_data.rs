use crate::{
    mesh::AsPolygonMesh,
    world::{CornerCells, EdgeCells, InnerCells},
};
use js_sys::{Float32Array, Uint32Array};
use wasm_bindgen::prelude::*;

/// Zero-copy WASM view over InnerCells.
/// All accessors return views into Wasm linear memory — clone on the JS side
#[wasm_bindgen]
pub struct InnerCellsHandle(InnerCells);

#[wasm_bindgen]
impl InnerCellsHandle {
    #[wasm_bindgen(getter)]
    pub fn vertices(&self) -> Float32Array {
        unsafe { Float32Array::view(&self.0.vertices) }
    }

    #[wasm_bindgen(getter)]
    pub fn indices(&self) -> Uint32Array {
        unsafe { Uint32Array::view(&self.0.indices) }
    }

    #[wasm_bindgen(getter)]
    pub fn ranges(&self) -> Uint32Array {
        unsafe { Uint32Array::view(&self.0.ranges) }
    }

    #[wasm_bindgen(getter)]
    pub fn sites(&self) -> Uint32Array {
        unsafe { Uint32Array::view(&self.0.sites) }
    }

    #[wasm_bindgen(getter)]
    pub fn tiles(&self) -> Uint32Array {
        unsafe { Uint32Array::view(&self.0.tiles) }
    }

    #[wasm_bindgen(getter)]
    pub fn tile_distortions(&self) -> Float32Array {
        unsafe { Float32Array::view(&self.0.tile_distortions) }
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
    #[wasm_bindgen(getter)]
    pub fn vertices(&self) -> Float32Array {
        unsafe { Float32Array::view(&self.0.vertices) }
    }

    #[wasm_bindgen(getter)]
    pub fn indices(&self) -> Uint32Array {
        unsafe { Uint32Array::view(&self.0.indices) }
    }

    #[wasm_bindgen(getter)]
    pub fn ranges(&self) -> Uint32Array {
        unsafe { Uint32Array::view(&self.0.ranges) }
    }

    #[wasm_bindgen(getter)]
    pub fn sites(&self) -> Uint32Array {
        unsafe { Uint32Array::view(&self.0.sites) }
    }

    #[wasm_bindgen(getter)]
    pub fn tiles(&self) -> Uint32Array {
        unsafe { Uint32Array::view(&self.0.tiles) }
    }

    #[wasm_bindgen(getter)]
    pub fn tile_distortions(&self) -> Float32Array {
        unsafe { Float32Array::view(&self.0.tile_distortions) }
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
    #[wasm_bindgen(getter)]
    pub fn vertices(&self) -> Float32Array {
        unsafe { Float32Array::view(&self.0.vertices) }
    }

    #[wasm_bindgen(getter)]
    pub fn indices(&self) -> Uint32Array {
        unsafe { Uint32Array::view(self.0.indices()) }
    }

    #[wasm_bindgen(getter)]
    pub fn ranges(&self) -> Uint32Array {
        unsafe { Uint32Array::view(self.0.ranges()) }
    }

    #[wasm_bindgen(getter)]
    pub fn sites(&self) -> Uint32Array {
        unsafe { Uint32Array::view(&self.0.sites) }
    }

    #[wasm_bindgen(getter)]
    pub fn tiles(&self) -> Uint32Array {
        unsafe { Uint32Array::view(&self.0.tiles) }
    }

    #[wasm_bindgen(getter)]
    pub fn tile_distortions(&self) -> Float32Array {
        unsafe { Float32Array::view(&self.0.tile_distortions) }
    }
}

impl From<CornerCells> for CornerCellsHandle {
    fn from(data: CornerCells) -> Self {
        Self(data)
    }
}
