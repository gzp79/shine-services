use crate::{
    wasm::world::WasmChunk,
    world::{ChunkId, World, CELL_WORLD_SIZE, CHUNK_WORLD_SIZE},
};
use tracing::info_span;
use wasm_bindgen::prelude::*;

#[wasm_bindgen(typescript_custom_section)]
const TS_WASM_WORLD: &str = r#"
interface WasmWorld {
    chunk_world_offset(ref_q: number, ref_r: number, q: number, r: number): [number, number];
}
"#;

/// The exported world root. Wraps the core `World` handle; the `Rc<RefCell<..>>` graph and all
/// staleness tracking live in core, so this layer only marshals ids and forwards calls.
#[wasm_bindgen]
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

    /// Handle to a loaded chunk, or `undefined` if no chunk is loaded at `(q, r)`.
    pub fn chunk(&self, q: i32, r: i32) -> Option<WasmChunk> {
        self.world.chunk(ChunkId(q, r)).map(WasmChunk::new)
    }
}
