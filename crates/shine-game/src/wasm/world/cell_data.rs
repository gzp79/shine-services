use crate::world::{CornerCells, EdgeCells, InnerCells, TileGeometries};
use js_sys::{Float32Array, Uint32Array};
use wasm_bindgen::prelude::*;

/// Which side of an EdgeCells polygon a tile belongs to. Matches Rust EdgeSide indices exactly.
#[wasm_bindgen]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EdgeSide {
    Owner = 0,
    Neighbor = 1,
}

/// Which side of a CornerCells polygon a tile belongs to. Matches Rust CornerSide indices exactly.
#[wasm_bindgen]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CornerSide {
    Owner = 0,
    CcwNeighbor = 1,
    CwNeighbor = 2,
}

/// Zero-copy WASM view over an InnerCells snapshot.
#[wasm_bindgen(js_name = "InnerCells")]
pub struct WasmInnerCells(InnerCells);

#[wasm_bindgen]
impl WasmInnerCells {
    /// Whether the source chunk is unchanged; `false` means every accessor returns `undefined`.
    pub fn valid(&self) -> bool {
        self.0.is_valid()
    }

    pub fn vertices(&self) -> Option<Float32Array> {
        self.0.vertices().map(|v| unsafe { Float32Array::view(v) })
    }

    pub fn indices(&self) -> Option<Uint32Array> {
        self.0.indices().map(|v| unsafe { Uint32Array::view(v) })
    }

    pub fn ranges(&self) -> Option<Uint32Array> {
        self.0.ranges().map(|v| unsafe { Uint32Array::view(v) })
    }

    pub fn cell_ids(&self) -> Option<Uint32Array> {
        self.0.cell_ids().map(|v| unsafe { Uint32Array::view(v) })
    }
}

impl From<InnerCells> for WasmInnerCells {
    fn from(data: InnerCells) -> Self {
        Self(data)
    }
}

/// Zero-copy WASM view over an EdgeCells snapshot.
#[wasm_bindgen(js_name = "EdgeCells")]
pub struct WasmEdgeCells(EdgeCells);

#[wasm_bindgen]
impl WasmEdgeCells {
    /// Whether the source chunk is unchanged; `false` means every accessor returns `undefined`.
    pub fn valid(&self) -> bool {
        self.0.is_valid()
    }

    pub fn vertices(&self) -> Option<Float32Array> {
        self.0.vertices().map(|v| unsafe { Float32Array::view(v) })
    }

    pub fn indices(&self) -> Option<Uint32Array> {
        self.0.indices().map(|v| unsafe { Uint32Array::view(v) })
    }

    pub fn ranges(&self) -> Option<Uint32Array> {
        self.0.ranges().map(|v| unsafe { Uint32Array::view(v) })
    }

    pub fn cell_ids(&self) -> Option<Uint32Array> {
        self.0.cell_ids().map(|v| unsafe { Uint32Array::view(v) })
    }
}

impl From<EdgeCells> for WasmEdgeCells {
    fn from(data: EdgeCells) -> Self {
        Self(data)
    }
}

/// Zero-copy WASM view over a CornerCells snapshot.
#[wasm_bindgen(js_name = "CornerCells")]
pub struct WasmCornerCells(CornerCells);

#[wasm_bindgen]
impl WasmCornerCells {
    /// Whether the source chunk is unchanged; `false` means every accessor returns `undefined`.
    pub fn valid(&self) -> bool {
        self.0.is_valid()
    }

    pub fn vertices(&self) -> Option<Float32Array> {
        self.0.vertices().map(|v| unsafe { Float32Array::view(v) })
    }

    pub fn indices(&self) -> Option<Uint32Array> {
        self.0.indices().map(|v| unsafe { Uint32Array::view(v) })
    }

    pub fn ranges(&self) -> Option<Uint32Array> {
        self.0.ranges().map(|v| unsafe { Uint32Array::view(v) })
    }

    pub fn cell_ids(&self) -> Option<Uint32Array> {
        self.0.cell_ids().map(|v| unsafe { Uint32Array::view(v) })
    }
}

impl From<CornerCells> for WasmCornerCells {
    fn from(data: CornerCells) -> Self {
        Self(data)
    }
}

/// Zero-copy WASM view over a TileGeometries snapshot.
#[wasm_bindgen(js_name = "TileGeometries")]
pub struct WasmTileGeometries(TileGeometries);

#[wasm_bindgen]
impl WasmTileGeometries {
    /// Whether the source chunk is unchanged; `false` means every accessor returns `undefined`.
    pub fn valid(&self) -> bool {
        self.0.is_valid()
    }

    pub fn tile_distortions(&self) -> Option<Float32Array> {
        self.0.tile_distortions().map(|v| unsafe { Float32Array::view(v) })
    }

    /// Number of tiles (`tile_distortions().length / 8`), `undefined` if the source chunk changed.
    pub fn tile_count(&self) -> Option<usize> {
        self.0.tile_count()
    }
}

impl From<TileGeometries> for WasmTileGeometries {
    fn from(data: TileGeometries) -> Self {
        Self(data)
    }
}
