use crate::{
    indexed::IdxVec,
    math::mesh::{QuadIdx, QuadTopology, VertIdx},
    world::{Chunk, ChunkId},
};
use glam::Vec2;
use std::collections::HashMap;

/// The core subdivision depth to align chunks
pub const SUBDIVISION_BASE: u32 = 4;
/// The numbe of cells on the edge of a chunk
pub const SUBDIVISION_COUNT: u32 = 2u32.pow(SUBDIVISION_BASE);

/// The world size of a chunk (in meter)
pub const CHUNK_WORLD_SIZE: f32 = 1000.0;
/// The "ideal" length of the side of a cell (in meter)
pub const CELL_WORLD_SIZE: f32 = CHUNK_WORLD_SIZE / SUBDIVISION_COUNT as f32;

/// Merge vertex rings from two chunks, replacing ghost quads with real data from neighbor.
/// Returns CCW-ordered quad centers forming the complete dual cell polygon.
///
/// Note: This is the critical piece - ring alignment logic needs careful implementation.
/// Ghost quads in one chunk correspond to real quads in the neighbor.
pub(crate) fn merge_vertex_ring_dual(
    topo1: &QuadTopology,
    centers1: &IdxVec<QuadIdx, Vec2>,
    v1: VertIdx,
    topo2: &QuadTopology,
    centers2: &IdxVec<QuadIdx, Vec2>,
    v2: VertIdx,
) -> Vec<Vec2> {
    // Collect rings with ghost markers
    let ring1: Vec<Option<Vec2>> = topo1
        .vertex_ring_ccw(v1)
        .map(|qv| {
            if topo1.is_ghost_quad(qv.quad) {
                None
            } else {
                Some(centers1[qv.quad])
            }
        })
        .collect();

    let ring2: Vec<Option<Vec2>> = topo2
        .vertex_ring_ccw(v2)
        .map(|qv| {
            if topo2.is_ghost_quad(qv.quad) {
                None
            } else {
                Some(centers2[qv.quad])
            }
        })
        .collect();

    // Find where ghost quad sequence starts in each ring
    let ghost_start1 = ring1.iter().position(|opt| opt.is_none());
    let ghost_start2 = ring2.iter().position(|opt| opt.is_none());

    // Build merged ring by taking real quads from both
    let mut merged = Vec::new();

    if let (Some(gs1), Some(gs2)) = (ghost_start1, ghost_start2) {
        // Add real quads from ring1 before ghost sequence
        for opt in ring1.iter().take(gs1) {
            if let Some(pos) = opt {
                merged.push(*pos);
            }
        }

        // Add real quads from ring2 before its ghost sequence
        // These fill the gap left by ring1's ghosts
        for opt in ring2.iter().take(gs2) {
            if let Some(pos) = opt {
                merged.push(*pos);
            }
        }
    } else {
        // No ghost quads found - just use ring1 (interior vertex)
        for opt in ring1.iter() {
            if let Some(pos) = opt {
                merged.push(*pos);
            }
        }
    }

    merged
}

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

    pub fn chunk_vertices(&self, id: ChunkId) -> Vec<f32> {
        self.chunk(id).map(|c| c.quad_vertices()).unwrap_or_default()
    }

    pub fn chunk_quad_indices(&self, id: ChunkId) -> Vec<u32> {
        self.chunk(id).map(|c| c.quad_indices()).unwrap_or_default()
    }

    pub fn chunk_boundary_indices(&self, id: ChunkId) -> Vec<u32> {
        self.chunk(id).map(|c| c.boundary_indices()).unwrap_or_default()
    }

    pub fn chunk_world_offset(&self, reference: ChunkId, target: ChunkId) -> Vec<f32> {
        if self.chunk(reference).is_none() {
            return vec![];
        }
        let offset = reference.relative_world_position(target);
        vec![offset.x, offset.y]
    }

    /// Returns dual polygons for boundary edge cells owned by the given chunk.
    /// Format: (vertices, indices, starts) matching Chunk::dual_polygons()
    pub fn boundary_edge_dual_polygons(
        &self,
        owner_id: ChunkId,
        edge_idx: u8,
    ) -> Option<(Vec<f32>, Vec<u32>, Vec<u32>)> {
        let owner = self.chunk(owner_id)?;
        let neighbor_id = owner_id.neighbor(edge_idx);
        let neighbor = self.chunk(neighbor_id)?;

        let owner_verts = owner.boundary_edge_vertices(edge_idx);
        let neighbor_edge_idx = (edge_idx + 3) % 6; // Opposite edge
        let neighbor_verts = neighbor.boundary_edge_vertices(neighbor_edge_idx);

        let mut vertices = Vec::new();
        let mut indices = Vec::new();
        let mut starts = vec![0];

        for i in 0..owner_verts.len() {
            // Neighbor vertices are in reverse order (opposite direction)
            let owner_vi = owner_verts[i];
            let neighbor_vi = neighbor_verts[neighbor_verts.len() - 1 - i];

            // Merge vertex rings from both chunks
            let polygon = merge_vertex_ring_dual(
                &owner.topology,
                &owner.quad_centers,
                owner_vi,
                &neighbor.topology,
                &neighbor.quad_centers,
                neighbor_vi,
            );

            if polygon.len() >= 3 {
                let start_idx = (vertices.len() / 2) as u32;
                for (j, pos) in polygon.iter().enumerate() {
                    vertices.push(pos.x);
                    vertices.push(pos.y);
                    indices.push(start_idx + j as u32);
                }
                starts.push(indices.len() as u32);
            } else {
                // Degenerate polygon - skip it
                starts.push(indices.len() as u32);
            }
        }

        Some((vertices, indices, starts))
    }

    /// Returns dual polygon for boundary vertex cell (single triangular cell).
    /// TODO: Implement full 3-chunk merge
    pub fn boundary_vertex_dual_polygon(
        &self,
        _owner_id: ChunkId,
        _vertex_idx: u8,
    ) -> Option<(Vec<f32>, Vec<u32>, Vec<u32>)> {
        // Stub: return empty for now
        None
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

        let border = world.chunk_boundary_indices(id);
        assert!(!border.is_empty(), "border indices should not be empty");
        assert_eq!(border.len() % 2, 0, "border indices should be pairs");
    }

    #[test]
    fn test_uninitialized_chunk_returns_empty() {
        let world = World::new();
        let id = ChunkId(999, 999);

        assert!(world.chunk_vertices(id).is_empty());
        assert!(world.chunk_quad_indices(id).is_empty());
        assert!(world.chunk_boundary_indices(id).is_empty());
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

    #[test]
    fn test_merge_vertex_ring_dual() {
        // Create two test chunks that are neighbors
        let chunk1_id = ChunkId::ORIGIN;
        let chunk2_id = chunk1_id.neighbor(0); // North neighbor

        let mut world = World::new();
        world.init_chunk(chunk1_id);
        world.init_chunk(chunk2_id);

        let chunk1 = world.chunk(chunk1_id).unwrap();
        let chunk2 = world.chunk(chunk2_id).unwrap();

        // Get a boundary vertex from each chunk (on shared edge)
        let edge0_verts1 = chunk1.boundary_edge_vertices(0);
        let edge3_verts2 = chunk2.boundary_edge_vertices(3); // Opposite edge

        // Merge rings for first boundary vertex
        let v1 = edge0_verts1[0];
        let v2 = edge3_verts2[edge3_verts2.len() - 1]; // Reversed order

        let merged = merge_vertex_ring_dual(
            &chunk1.topology,
            &chunk1.quad_centers,
            v1,
            &chunk2.topology,
            &chunk2.quad_centers,
            v2,
        );

        // Merged ring should have no gaps (all quads present)
        assert!(merged.len() >= 3, "Merged ring should have at least 3 quads");

        // Ring should form a closed polygon
        // (specific count depends on subdivision level)
    }

    #[test]
    fn test_boundary_edge_dual_polygons() {
        let mut world = World::new();
        let chunk1_id = ChunkId::ORIGIN;
        let chunk2_id = chunk1_id.neighbor(0); // North neighbor

        world.init_chunk(chunk1_id);
        world.init_chunk(chunk2_id);

        // Get dual polygons for edge 0 of chunk1 (shared with chunk2)
        let result = world.boundary_edge_dual_polygons(chunk1_id, 0);

        assert!(result.is_some(), "Should return polygons when both chunks loaded");

        let (vertices, indices, starts) = result.unwrap();

        // Should have vertices
        assert!(!vertices.is_empty(), "Should have vertices");
        assert_eq!(vertices.len() % 2, 0, "Vertices should be pairs (x, y)");

        // Should have polygon data
        assert!(!indices.is_empty(), "Should have indices");
        assert!(!starts.is_empty(), "Should have starts");
        assert_eq!(starts[0], 0, "First start should be 0");

        // Number of polygons should match subdivisions per edge
        let num_polygons = starts.len() - 1;
        let expected = 1 << SUBDIVISION_BASE;
        assert_eq!(num_polygons, expected, "Should have {} boundary cells", expected);
    }
}
