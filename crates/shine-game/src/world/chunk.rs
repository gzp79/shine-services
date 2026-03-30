use crate::{
    indexed::{IdxVec, TypedIndex},
    math::{
        hex::{AxialCoord, LatticeMesher},
        mesh::{QuadTopology, VertIdx},
        rand::Xorshift32,
    },
    world::{CHUNK_WORLD_SIZE, SUBDIVISION_BASE},
};
use glam::Vec2;

/// Unique identifier of a chunk of the map.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct ChunkId(pub i32, pub i32);

impl ChunkId {
    pub const ORIGIN: ChunkId = ChunkId(0, 0);

    /// World-space offset from `self` to `other`.
    pub fn relative_world_position(&self, other: ChunkId) -> Vec2 {
        let rel = AxialCoord::new(other.0 - self.0, other.1 - self.1);
        rel.center_position(CHUNK_WORLD_SIZE)
    }

    /// Deterministic 32-bit hash from chunk coordinates.
    /// Uses golden-ratio mixing + murmur3 finalizer for good avalanche.
    pub fn hash32(&self) -> u32 {
        let a = self.0 as u32;
        let b = self.1 as u32;
        let mut h = a.wrapping_mul(0x9e3779b9).wrapping_add(b);
        h ^= h >> 16;
        h = h.wrapping_mul(0x45d9f3b);
        h ^= h >> 16;
        h
    }
}

impl From<ChunkId> for AxialCoord {
    fn from(id: ChunkId) -> Self {
        AxialCoord::new(id.0, id.1)
    }
}

impl From<AxialCoord> for ChunkId {
    fn from(c: AxialCoord) -> Self {
        ChunkId(c.q, c.r)
    }
}

pub struct Chunk {
    pub topology: QuadTopology,
    pub vertices: IdxVec<VertIdx, Vec2>,
}

impl Chunk {
    pub fn new(id: ChunkId) -> Self {
        let rng = Xorshift32::new(id.hash32());
        let mesh = LatticeMesher::new(SUBDIVISION_BASE, rng)
            .with_world_size(CHUNK_WORLD_SIZE)
            .generate();
        let (topology, vertices) = mesh.into_parts();
        Self { topology, vertices }
    }

    /// Flat (real) quad vertex positions [x, y, x, y, ...]
    pub fn quad_vertices(&self) -> Vec<f32> {
        debug_assert_eq!(self.vertices.len(), self.topology.vertex_count());
        let mut flat = Vec::with_capacity(self.topology.vertex_count() * 2);
        for vi in self.topology.vertex_indices() {
            let p = self.vertices[vi];
            flat.push(p.x);
            flat.push(p.y);
        }
        flat
    }

    /// Flat (real) quad indices [a, b, c, d, ...].
    pub fn quad_indices(&self) -> Vec<u32> {
        let mut indices = Vec::with_capacity(self.topology.quad_count() * 4);
        for qi in self.topology.quad_indices() {
            let verts = self.topology.quad_vertices(qi);
            for &v in &verts {
                indices.push(v.into_index() as u32);
            }
        }
        indices
    }

    /// Flat boundary edge indices [a, b, ...].
    pub fn boundary_indices(&self) -> Vec<u32> {
        // Each boundary vertex corresponds to one edge, so N vertices = N edges
        let edge_count = self.topology.boundary_vertex_count();
        let mut flat = Vec::with_capacity(edge_count * 2);
        for [a, b] in self.topology.boundary_edges() {
            flat.push(a);
            flat.push(b);
        }
        flat
    }

    /// Dual mesh vertices (quad centroids) [x, y, x, y, ...]
    pub fn dual_vertices(&self) -> Vec<f32> {
        let quad_count = self.topology.quad_count();
        let mut flat = Vec::with_capacity(quad_count * 2);

        for qi in self.topology.quad_indices() {
            let verts = self.topology.quad_vertices(qi);
            let mut cx = 0.0f32;
            let mut cy = 0.0f32;
            for &v in &verts {
                let p = self.vertices[v];
                cx += p.x;
                cy += p.y;
            }
            flat.push(cx / 4.0);
            flat.push(cy / 4.0);
        }

        flat
    }

    /// Dual mesh edge indices [a, b, ...] connecting adjacent quad centroids
    pub fn dual_indices(&self) -> Vec<u32> {
        let mut edges = Vec::new();
        let mut quad_to_dual: std::collections::HashMap<usize, u32> = std::collections::HashMap::new();

        // Map quad indices to dual vertex indices
        for (dual_idx, qi) in self.topology.quad_indices().enumerate() {
            quad_to_dual.insert(qi.into_index(), dual_idx as u32);
        }

        // Find edges between adjacent quads
        for qi in self.topology.quad_indices() {
            let Some(&dual_idx) = quad_to_dual.get(&qi.into_index()) else {
                continue;
            };

            for edge_idx in 0..4 {
                let qe = crate::math::mesh::QuadEdge { quad: qi, edge: edge_idx as u8 };
                let neighbor = self.topology.edge_twin(qe);
                if self.topology.is_ghost_quad(neighbor.quad) {
                    continue; // Skip ghost neighbors
                }

                let Some(&neighbor_dual_idx) = quad_to_dual.get(&neighbor.quad.into_index()) else {
                    continue;
                };

                // Only add each edge once (avoid duplicates)
                if dual_idx < neighbor_dual_idx {
                    edges.push(dual_idx);
                    edges.push(neighbor_dual_idx);
                }
            }
        }

        edges
    }
}
