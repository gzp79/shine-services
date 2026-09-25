use crate::{
    indexed::TypedIndex,
    math::quadrangulation::Rot4Idx,
    wasm::{
        math::{WasmHexFlatDir, WasmHexPointyDir},
        world::{WasmChangeLog, WasmCornerCells, WasmEdgeCells, WasmInnerCells},
    },
    world::{
        base_layer::{Base, Clear, SetCell, SetQuadrant},
        CellIndex, ChunkId, TileIndex, World, CELL_WORLD_SIZE, CHUNK_WORLD_SIZE,
    },
};
use serde::Deserialize;
use tracing::info_span;
use wasm_bindgen::prelude::*;

#[wasm_bindgen(typescript_custom_section)]
const TS_WASM_WORLD: &str = r#"
export type BaseLayerOp =
    | { op: "sync" }
    | { op: "clear"; value: number }
    | { op: "setQuadrant"; tile: number; quadrant: number; value: number }
    | { op: "setCell"; cell: number; value: number };

interface World {
    chunk_world_offset(ref_q: number, ref_r: number, q: number, r: number): [number, number];
    update_base_layer(q: number, r: number, op: BaseLayerOp): ChangeLog | undefined;
}
"#;

/// Mirrors the `BaseLayerOp` TS union declared above; `serde`'s tag/rename match the JS field names.
#[derive(Deserialize)]
#[serde(tag = "op", rename_all = "camelCase")]
enum WasmBaseLayerOp {
    Sync,
    Clear { value: u8 },
    SetQuadrant { tile: u32, quadrant: u8, value: u8 },
    SetCell { cell: u32, value: u8 },
}

#[wasm_bindgen(js_name = "World")]
pub struct WasmWorld {
    world: World,
}

#[wasm_bindgen]
impl WasmWorld {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self { world: World::new() }
    }

    pub fn init_chunk(&self, q: i32, r: i32) {
        let _span = info_span!("init_chunk", q, r).entered();
        self.world.init_chunk(ChunkId(q, r));
    }

    pub fn remove_chunk(&self, q: i32, r: i32) {
        self.world.remove_chunk(ChunkId(q, r));
    }

    pub fn const_chunk_world_size(&self) -> f32 {
        CHUNK_WORLD_SIZE
    }
    pub fn const_cell_world_size(&self) -> f32 {
        CELL_WORLD_SIZE
    }

    #[wasm_bindgen(skip_typescript)]
    pub fn chunk_world_offset(&self, ref_q: i32, ref_r: i32, q: i32, r: i32) -> Vec<f32> {
        let reference = ChunkId(ref_q, ref_r);
        let chunk = ChunkId(q, r);
        let pos = reference.relative_world_position(chunk);
        vec![pos.x, pos.y]
    }

    pub fn inner_cells(&self, q: i32, r: i32) -> Option<WasmInnerCells> {
        self.world.inner_cells(ChunkId(q, r)).map(Into::into)
    }

    pub fn edge_cells(&self, q: i32, r: i32, edge_idx: WasmHexFlatDir) -> Option<WasmEdgeCells> {
        self.world.edge_cells(ChunkId(q, r), edge_idx.into()).map(Into::into)
    }

    pub fn corner_cells(&self, q: i32, r: i32, corner_idx: WasmHexPointyDir) -> Option<WasmCornerCells> {
        self.world
            .corner_cells(ChunkId(q, r), corner_idx.into())
            .map(Into::into)
    }

    pub fn hex_vertices(&self, q: i32, r: i32) -> Option<Vec<f32>> {
        self.world.hex_vertices(ChunkId(q, r))
    }

    #[wasm_bindgen(skip_typescript)]
    pub fn update_base_layer(&self, q: i32, r: i32, op: JsValue) -> Result<Option<WasmChangeLog>, JsValue> {
        let op: WasmBaseLayerOp = serde_wasm_bindgen::from_value(op)?;
        let id = ChunkId(q, r);
        let change_log = match op {
            WasmBaseLayerOp::Sync => self.world.update_layer(id, Base),
            WasmBaseLayerOp::Clear { value } => self.world.update_layer(id, Clear { value }),
            WasmBaseLayerOp::SetQuadrant { tile, quadrant, value } => self.world.update_layer(
                id,
                SetQuadrant {
                    tile: TileIndex::new(tile as usize),
                    quadrant: Rot4Idx::new(quadrant as usize),
                    value,
                },
            ),
            WasmBaseLayerOp::SetCell { cell, value } => self.world.update_layer(
                id,
                SetCell {
                    cell: CellIndex::new(cell as usize),
                    value,
                },
            ),
        };
        Ok(change_log.map(Into::into))
    }
}
