use crate::{
    wasm::{
        math::{WasmHexFlatDir, WasmHexPointyDir},
        world::{WasmChangeLog, WasmCornerCells, WasmEdgeCells, WasmInnerCells},
    },
    world::{base_layer::Base, ChunkId, World, CELL_WORLD_SIZE, CHUNK_WORLD_SIZE},
};
use tracing::info_span;
use wasm_bindgen::prelude::*;

#[wasm_bindgen(typescript_custom_section)]
const TS_WASM_WORLD: &str = r#"
interface World {
    chunk_world_offset(ref_q: number, ref_r: number, q: number, r: number): [number, number];
}
"#;

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

    pub fn update_base_layer(&self, q: i32, r: i32) -> Option<WasmChangeLog> {
        self.world.update_layer(ChunkId(q, r), Base).map(Into::into)
    }

    pub fn sync_base_layer(&self, q: i32, r: i32) -> Option<WasmChangeLog> {
        self.update_base_layer(q, r)
    }
}
