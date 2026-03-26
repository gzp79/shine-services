use crate::world::{ChunkId, World};
use wasm_bindgen::prelude::*;

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

    pub fn init_chunk(&mut self, q: i32, r: i32) {
        self.world.init_chunk(ChunkId(q, r));
    }

    pub fn remove_chunk(&mut self, q: i32, r: i32) {
        self.world.remove_chunk(ChunkId(q, r));
    }

    pub fn chunk_vertices(&self, q: i32, r: i32) -> Vec<f32> {
        self.world.chunk_vertices(ChunkId(q, r))
    }

    pub fn chunk_quad_indices(&self, q: i32, r: i32) -> Vec<u32> {
        self.world.chunk_quad_indices(ChunkId(q, r))
    }

    pub fn chunk_border_indices(&self, q: i32, r: i32) -> Vec<u32> {
        self.world.chunk_border_indices(ChunkId(q, r))
    }

    pub fn chunk_world_offset(&self, ref_q: i32, ref_r: i32, q: i32, r: i32) -> Vec<f32> {
        self.world.chunk_world_offset(ChunkId(ref_q, ref_r), ChunkId(q, r))
    }
}
