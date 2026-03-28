use crate::{
    indexed::TypedIndex,
    world::{Chunk, ChunkId},
};
use std::collections::HashMap;

/// The core subdivision depth to align chunks
pub const SUBDIVISION_BASE: u32 = 4;
/// The numbe of cells on the edge of a chunk
pub const SUBDIVISION_COUNT: u32 = 2u32.pow(SUBDIVISION_BASE);

/// The world size of a chunk (in meter)
pub const CHUNK_WORLD_SIZE: f32 = 1000.0;
/// The "ideal" length of the side of a cell (in meter)
pub const CELL_WORLD_SIZE: f32 = CHUNK_WORLD_SIZE / SUBDIVISION_COUNT as f32;

pub struct World {
    chunks: HashMap<ChunkId, Chunk>,
}

impl World {
    pub fn new() -> Self {
        Self { chunks: HashMap::new() }
    }

    pub fn init_chunk(&mut self, id: ChunkId) {
        self.chunks.insert(id, Chunk::new(id));
    }

    pub fn chunk(&self, id: ChunkId) -> Option<&Chunk> {
        self.chunks.get(&id)
    }

    pub fn remove_chunk(&mut self, id: ChunkId) {
        self.chunks.remove(&id);
    }

    /// Flat vertex positions [x, y, x, y, ...]. Empty if chunk not found.
    pub fn chunk_vertices(&self, id: ChunkId) -> Vec<f32> {
        let Some(chunk) = self.chunks.get(&id) else {
            return Vec::new();
        };
        // vertices IdxVec contains only real vertices (from LatticeMesher, before ghost quads)
        debug_assert_eq!(chunk.vertices.len(), chunk.topology.real_vertex_count());
        let mut flat = Vec::with_capacity(chunk.topology.real_vertex_count() * 2);
        for vi in chunk.topology.vertex_indices() {
            let p = chunk.vertices[vi];
            flat.push(p.x);
            flat.push(p.y);
        }
        flat
    }

    /// Flat quad indices [a, b, c, d, ...]. Empty if chunk not found.
    /// Safety: real quads only reference real vertices (ghost vertices only appear in ghost quads).
    pub fn chunk_quad_indices(&self, id: ChunkId) -> Vec<u32> {
        let Some(chunk) = self.chunks.get(&id) else {
            return Vec::new();
        };
        let mut indices = Vec::with_capacity(chunk.topology.real_quad_count() * 4);
        for qi in chunk.topology.real_quad_indices() {
            let verts = chunk.topology.quad_vertices(qi);
            for &v in &verts {
                indices.push(v.into_index() as u32);
            }
        }
        indices
    }

    /// Flat border edge indices [a, b, ...]. Empty if chunk not found.
    pub fn chunk_border_indices(&self, id: ChunkId) -> Vec<u32> {
        let Some(chunk) = self.chunks.get(&id) else {
            return Vec::new();
        };
        let edges = chunk.topology.border_edges();
        let mut flat = Vec::with_capacity(edges.len() * 2);
        for [a, b] in edges {
            flat.push(a);
            flat.push(b);
        }
        flat
    }

    /// World offset [x, y] of `chunk` relative to `reference`. Empty if target chunk not found.
    /// Note: only the target chunk needs to be initialized — the offset is computed from
    /// ChunkId math alone, but we check existence to match the "empty for missing" contract.
    /// The reference chunk does NOT need to be initialized.
    pub fn chunk_world_offset(&self, reference: ChunkId, chunk: ChunkId) -> Vec<f32> {
        if !self.chunks.contains_key(&chunk) {
            return Vec::new();
        }
        let pos = reference.relative_world_position(chunk);
        vec![pos.x, pos.y]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use shine_test::test;

    #[test]
    fn test_init_and_query_chunk() {
        let mut world = World::new();
        let id = ChunkId::ORIGIN;
        world.init_chunk(id);

        let verts = world.chunk_vertices(id);
        assert!(!verts.is_empty(), "vertices should not be empty after init");
        assert_eq!(verts.len() % 2, 0, "vertices should have even length (x,y pairs)");

        let indices = world.chunk_quad_indices(id);
        assert!(!indices.is_empty(), "quad indices should not be empty");
        assert_eq!(indices.len() % 4, 0, "quad indices should be multiple of 4");

        let border = world.chunk_border_indices(id);
        assert!(!border.is_empty(), "border indices should not be empty");
        assert_eq!(border.len() % 2, 0, "border indices should be pairs");
    }

    #[test]
    fn test_uninitialized_chunk_returns_empty() {
        let world = World::new();
        let id = ChunkId(999, 999);

        assert!(world.chunk_vertices(id).is_empty());
        assert!(world.chunk_quad_indices(id).is_empty());
        assert!(world.chunk_border_indices(id).is_empty());
        assert!(world.chunk_world_offset(ChunkId::ORIGIN, id).is_empty());
    }

    #[test]
    fn test_remove_chunk() {
        let mut world = World::new();
        let id = ChunkId::ORIGIN;
        world.init_chunk(id);
        assert!(!world.chunk_vertices(id).is_empty());

        world.remove_chunk(id);
        assert!(world.chunk_vertices(id).is_empty());
    }

    #[test]
    fn test_chunk_world_offset_origin() {
        let mut world = World::new();
        let origin = ChunkId::ORIGIN;
        world.init_chunk(origin);

        let offset = world.chunk_world_offset(origin, origin);
        assert_eq!(offset.len(), 2);
        assert!((offset[0]).abs() < f32::EPSILON, "same chunk offset x should be 0");
        assert!((offset[1]).abs() < f32::EPSILON, "same chunk offset y should be 0");
    }

    #[test]
    fn test_chunk_world_offset_neighbor() {
        let mut world = World::new();
        let origin = ChunkId::ORIGIN;
        let neighbor = ChunkId(1, 0); // q+1, same r
        world.init_chunk(origin);
        world.init_chunk(neighbor);

        let offset = world.chunk_world_offset(origin, neighbor);
        assert_eq!(offset.len(), 2);
        // q+1 neighbor: x should be positive (1.5 * CHUNK_WORLD_SIZE), y should be non-zero
        assert!(offset[0] > 0.0, "q+1 neighbor should have positive x offset");
    }
}
