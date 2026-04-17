use std::{cell::RefCell, rc::Rc};

use crate::{
    indexed::{IdxVec, TypedIndex},
    math::{
        hex::{AxialCoord, LatticeMesher},
        mesh::{QuadIdx, QuadMesh, QuadTopology, VertIdx},
        prng::{hash_u32_2, Pcg32, SplitMix64},
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
        hash_u32_2(self.0 as u32, self.1 as u32)
    }

    pub fn id_64(&self) -> u64 {
        let high = self.0 as u64;
        let low = self.1 as u64;
        (high << 32) | low
    }

    pub fn neighbor(&self, direction: u8) -> ChunkId {
        AxialCoord::from(*self).neighbor(direction).into()
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

/// Stable random streams for different aspects of chunk generation.
/// Streams are cheap, create a new one for each aspect to ensure deterministic independence.
pub struct ChunkRngStreams {
    pub mesh: Rc<RefCell<Pcg32>>,
}

impl ChunkRngStreams {
    pub fn new(mut seed: SplitMix64) -> Self {
        let mesh = Rc::new(RefCell::new(seed.next_stream()));
        Self { mesh }
    }
}

pub struct Chunk {
    pub rng_streams: ChunkRngStreams,
    pub topology: QuadTopology,
    pub vertices: IdxVec<VertIdx, Vec2>,
    pub quad_centers: IdxVec<QuadIdx, Vec2>,
}

impl Chunk {
    pub fn new(parent_seed: &SplitMix64, id: ChunkId) -> Self {
        let rng_streams = ChunkRngStreams::new(parent_seed.create_seed(id.id_64()));
        let mesh = LatticeMesher::new(SUBDIVISION_BASE, rng_streams.mesh.clone())
            .with_world_size(CHUNK_WORLD_SIZE)
            .generate();
        let QuadMesh {
            topology,
            vertices,
            quad_centers,
        } = mesh;

        Self {
            rng_streams,
            topology,
            vertices,
            quad_centers,
        }
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
            let center = self.quad_centers[qi];
            flat.push(center.x);
            flat.push(center.y);
        }

        flat
    }

    /// Flat dual polygon indices referencing quad_centers.
    /// Each vertex's surrounding quads form a dual polygon.
    /// Returns (indices, starts) where starts[vi] marks the beginning of vertex vi's polygon.
    ///
    /// Example: For a vertex surrounded by 4 quads (indices 0,1,2,3 in dual_vertices):
    /// - indices: [0, 1, 2, 3]
    /// - starts: [0, 4] (polygon starts at 0, next would start at 4)
    pub fn dual_polygons(&self) -> (Vec<u32>, Vec<u32>) {
        let mut indices = Vec::new();
        let mut starts = Vec::new();
        starts.push(0);

        for vi in self.topology.vertex_indices() {
            let start_len = indices.len();

            // Collect QuadIdx for all real quads around this vertex
            for qv in self.topology.vertex_ring_ccw(vi) {
                if !self.topology.is_ghost_quad(qv.quad) {
                    // Map QuadIdx to its position in quad_indices() enumeration
                    let mut dual_idx = 0;
                    for (i, qi) in self.topology.quad_indices().enumerate() {
                        if qi == qv.quad {
                            dual_idx = i as u32;
                            break;
                        }
                    }
                    indices.push(dual_idx);
                }
            }

            // Only record if we found at least 3 quads (valid polygon)
            if indices.len() - start_len >= 3 {
                starts.push(indices.len() as u32);
            } else {
                // Degenerate polygon, remove indices and don't advance start
                indices.truncate(start_len);
                starts.push(start_len as u32);
            }
        }

        (indices, starts)
    }

    /// Returns VertIdx values along specified hex edge (0..5)
    pub fn boundary_edge_vertices(&self, edge_idx: u8) -> Vec<VertIdx> {
        self.topology.anchor_edge(edge_idx as usize).collect()
    }

    /// Returns VertIdx at specified hex corner (0..5)
    pub fn boundary_corner_vertex(&self, corner_idx: u8) -> VertIdx {
        self.topology.anchor_vertices[corner_idx as usize]
    }
}
