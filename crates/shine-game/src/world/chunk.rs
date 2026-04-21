use crate::{
    indexed::TypedIndex,
    math::{
        hex::LatticeMesher,
        prng::{Pcg32, SplitMix64},
        quadrangulation::{Quadrangulation, VertexIndex},
    },
    world::{ChunkId, CHUNK_WORLD_SIZE, SUBDIVISION_BASE},
};
use std::{cell::RefCell, rc::Rc};

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
    pub mesh: Quadrangulation,
}

impl Chunk {
    pub fn new(parent_seed: &SplitMix64, id: ChunkId) -> Self {
        let rng_streams = ChunkRngStreams::new(parent_seed.create_seed(id.id_64()));
        let topology = LatticeMesher::new(SUBDIVISION_BASE, rng_streams.mesh.clone())
            .with_world_size(CHUNK_WORLD_SIZE)
            .generate();

        Self { rng_streams, mesh: topology }
    }

    /// Flat (real) quad vertex positions [x, y, x, y, ...]
    pub fn quad_vertices(&self) -> Vec<f32> {
        let mut flat = Vec::with_capacity(self.mesh.vertex_count() * 2);
        for vi in self.mesh.finite_vertex_index_iter() {
            let p = self.mesh[vi].position;
            flat.push(p.x);
            flat.push(p.y);
        }
        flat
    }

    /// Flat (real) quad indices [a, b, c, d, ...].
    pub fn quad_indices(&self) -> Vec<u32> {
        let mut indices = Vec::with_capacity(self.mesh.finite_quad_count() * 4);
        for qi in self.mesh.finite_quad_index_iter() {
            let verts = self.mesh.quad_vertices(qi);
            for &v in verts {
                indices.push(v.into_index() as u32);
            }
        }
        indices
    }

    /// Flat boundary edge indices [a, b, ...].
    pub fn boundary_indices(&self) -> Vec<u32> {
        // Each boundary vertex corresponds to one edge, so N vertices = N edges
        let edge_count = self.mesh.boundary_vertex_count();
        let mut flat = Vec::with_capacity(edge_count * 2);
        for [a, b] in self.mesh.boundary_edges() {
            flat.push(a);
            flat.push(b);
        }
        flat
    }

    /// Dual mesh vertices (quad centroids) [x, y, x, y, ...]
    pub fn dual_vertices(&self) -> Vec<f32> {
        let quad_count = self.mesh.finite_quad_count();
        let mut flat = Vec::with_capacity(quad_count * 2);

        for qi in self.mesh.finite_quad_index_iter() {
            if let Some(center) = self.mesh.dual_p(qi) {
                flat.push(center.x);
                flat.push(center.y);
            }
        }

        flat
    }

    /// Flat dual polygon indices referencing dual vertices.
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

        for vi in self.mesh.finite_vertex_index_iter() {
            let start_len = indices.len();

            // Collect QuadIndex for all real quads around this vertex
            for qv in self.mesh.vertex_ring_ccw(vi) {
                if !self.mesh.is_infinite_quad(qv.quad) {
                    // Map QuadIndex to its position in quad_indices() enumeration
                    let mut dual_idx = 0;
                    for (i, qi) in self.mesh.finite_quad_index_iter().enumerate() {
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

    /// Returns VertexIndex values along specified hex edge (0..5)
    pub fn boundary_edge_vertices(&self, edge_idx: u8) -> Vec<VertexIndex> {
        use crate::math::quadrangulation::AnchorIndex;
        self.mesh.anchor_edge(AnchorIndex::new(edge_idx as usize)).collect()
    }

    /// Returns VertexIndex at specified hex corner (0..5)
    pub fn boundary_corner_vertex(&self, corner_idx: u8) -> VertexIndex {
        use crate::math::quadrangulation::AnchorIndex;
        self.mesh.anchor_vertex(AnchorIndex::new(corner_idx as usize))
    }
}
