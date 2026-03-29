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

    pub fn chunk_quad_vertices(&self, q: i32, r: i32) -> Vec<f32> {
        self.world
            .chunk(ChunkId(q, r))
            .map(|chunk| chunk.quad_vertices())
            .unwrap_or_default()
    }

    pub fn chunk_quad_indices(&self, q: i32, r: i32) -> Vec<u32> {
        self.world
            .chunk(ChunkId(q, r))
            .map(|chunk| chunk.quad_indices())
            .unwrap_or_default()
    }

    pub fn chunk_boundary_indices(&self, q: i32, r: i32) -> Vec<u32> {
        self.world
            .chunk(ChunkId(q, r))
            .map(|chunk| chunk.boundary_indices())
            .unwrap_or_default()
    }

    pub fn chunk_dual_vertices(&self, q: i32, r: i32) -> Vec<f32> {
        self.world
            .chunk(ChunkId(q, r))
            .map(|chunk| chunk.dual_vertices())
            .unwrap_or_default()
    }

    pub fn chunk_dual_indices(&self, q: i32, r: i32) -> Vec<u32> {
        self.world
            .chunk(ChunkId(q, r))
            .map(|chunk| chunk.dual_indices())
            .unwrap_or_default()
    }

    pub fn chunk_world_offset(&self, ref_q: i32, ref_r: i32, q: i32, r: i32) -> Vec<f32> {
        let reference = ChunkId(ref_q, ref_r);
        let chunk = ChunkId(q, r);
        let pos = reference.relative_world_position(chunk);
        vec![pos.x, pos.y]
    }
}
