use crate::world::{CornerCells, CornerSide as CoreCornerSide, EdgeCells, EdgeSide as CoreEdgeSide, InnerCells};
use js_sys::{Float32Array, Uint32Array, Uint8Array};
use wasm_bindgen::prelude::*;

/// Which side of an EdgeCells polygon a tile belongs to. Matches Rust EdgeSide indices exactly.
#[wasm_bindgen]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EdgeSide {
    Owner = 0,
    Neighbor = 1,
}

impl From<EdgeSide> for CoreEdgeSide {
    fn from(side: EdgeSide) -> Self {
        CoreEdgeSide::from_index(side as usize)
    }
}

/// Which side of a CornerCells polygon a tile belongs to. Matches Rust CornerSide indices exactly.
#[wasm_bindgen]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CornerSide {
    Owner = 0,
    CcwNeighbor = 1,
    CwNeighbor = 2,
}

impl From<CornerSide> for CoreCornerSide {
    fn from(side: CornerSide) -> Self {
        CoreCornerSide::from_index(side as usize)
    }
}

/// Zero-copy WASM view over an InnerCells snapshot. Accessors return views into Wasm linear memory
/// (clone on the JS side to outlive the next call), or `undefined` once the source chunk changed.
#[wasm_bindgen]
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

    pub fn tile_ids(&self) -> Option<Uint32Array> {
        self.0.tile_ids().map(|v| unsafe { Uint32Array::view(v) })
    }

    pub fn tile_vertices(&self) -> Option<Uint8Array> {
        self.0.tile_vertices().map(|v| unsafe { Uint8Array::view(v) })
    }

    pub fn tile_distortions(&self) -> Option<Float32Array> {
        self.0.tile_distortions().map(|v| unsafe { Float32Array::view(v) })
    }

    /// Packed [tile_id, vertex, tile_id, vertex, ...] pairs of every quad bordering `cell_id`.
    pub fn cell_tiles(&self, cell_id: u32) -> Option<Uint32Array> {
        let flat: Vec<u32> = self
            .0
            .cell_tiles(cell_id)?
            .flat_map(|(tile_id, vertex)| [tile_id, vertex as u32])
            .collect();
        Some(Uint32Array::from(flat.as_slice()))
    }
}

impl From<InnerCells> for WasmInnerCells {
    fn from(data: InnerCells) -> Self {
        Self(data)
    }
}

/// Zero-copy WASM view over an EdgeCells snapshot. Accessors return views into Wasm linear memory
/// (clone on the JS side to outlive the next call), or `undefined` once the source chunk changed.
#[wasm_bindgen]
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

    pub fn tile_ids(&self) -> Option<Uint32Array> {
        self.0.tile_ids().map(|v| unsafe { Uint32Array::view(v) })
    }

    pub fn tile_vertices(&self) -> Option<Uint8Array> {
        self.0.tile_vertices().map(|v| unsafe { Uint8Array::view(v) })
    }

    pub fn tile_distortions(&self) -> Option<Float32Array> {
        self.0.tile_distortions().map(|v| unsafe { Float32Array::view(v) })
    }

    /// Packed [tile_id, vertex, tile_id, vertex, ...] pairs of every quad bordering `cell_id` on the given `side`.
    pub fn cell_tiles(&self, side: EdgeSide, cell_id: u32) -> Option<Uint32Array> {
        let flat: Vec<u32> = self
            .0
            .cell_tiles(side.into(), cell_id)?
            .flat_map(|(tile_id, vertex)| [tile_id, vertex as u32])
            .collect();
        Some(Uint32Array::from(flat.as_slice()))
    }
}

impl From<EdgeCells> for WasmEdgeCells {
    fn from(data: EdgeCells) -> Self {
        Self(data)
    }
}

/// Zero-copy WASM view over a CornerCells snapshot. Accessors return views into Wasm linear memory
/// (clone on the JS side to outlive the next call), or `undefined` once the source chunk changed.
#[wasm_bindgen]
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

    pub fn tile_ids(&self) -> Option<Uint32Array> {
        self.0.tile_ids().map(|v| unsafe { Uint32Array::view(v) })
    }

    pub fn tile_vertices(&self) -> Option<Uint8Array> {
        self.0.tile_vertices().map(|v| unsafe { Uint8Array::view(v) })
    }

    pub fn tile_distortions(&self) -> Option<Float32Array> {
        self.0.tile_distortions().map(|v| unsafe { Float32Array::view(v) })
    }

    /// Packed [tile_id, vertex, tile_id, vertex, ...] pairs of every quad bordering `cell_id` on the given `side`.
    pub fn cell_tiles(&self, side: CornerSide, cell_id: u32) -> Option<Uint32Array> {
        let flat: Vec<u32> = self
            .0
            .cell_tiles(side.into(), cell_id)?
            .flat_map(|(tile_id, vertex)| [tile_id, vertex as u32])
            .collect();
        Some(Uint32Array::from(flat.as_slice()))
    }
}

impl From<CornerCells> for WasmCornerCells {
    fn from(data: CornerCells) -> Self {
        Self(data)
    }
}
