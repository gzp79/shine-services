use crate::{
    indexed::TypedIndex,
    math::{
        hex::{HexFlatDir, HexPointyDir, LatticeMesher},
        prng::{Pcg32, SplitMix64},
        quadrangulation::{AnchorIndex, Quadrangulation, VertexIndex},
    },
    world::{ChunkId, InnerCells, CHUNK_WORLD_SIZE, SUBDIVISION_BASE},
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
            .with_size(CHUNK_WORLD_SIZE)
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

    pub fn cell_data(&self) -> InnerCells {
        let mut vertices = Vec::with_capacity(self.mesh.finite_quad_count() * 2);
        let mut quad_map = std::collections::HashMap::new();
        for qi in self.mesh.finite_quad_index_iter() {
            if let Some(center) = self.mesh.dual_p(qi) {
                quad_map.insert(qi, (vertices.len() / 2) as u32);
                vertices.push(center.x);
                vertices.push(center.y);
            }
        }

        let site_count = self.mesh.finite_vertex_count();
        // this is just an optimistic preallocation, index count is not known in advance
        let mut indices = Vec::with_capacity(site_count * 4);
        let mut ranges = Vec::with_capacity(site_count * 2);
        let mut sites = Vec::with_capacity(site_count);

        for vi in self.mesh.finite_vertex_index_iter() {
            if self.mesh.is_boundary_vertex(vi) {
                continue;
            }

            ranges.push(indices.len() as u32);
            sites.push(vi.into_index() as u32);

            for qv in self.mesh.vertex_ring_ccw(vi) {
                indices.push(*quad_map.get(&qv.quad).unwrap());
            }

            ranges.push(indices.len() as u32);
        }

        InnerCells {
            vertices,
            indices,
            ranges,
            sites,
        }
    }

    /// Returns VertexIndex values along specified hex edge (inclusive of both corners)
    pub fn boundary_edge_vertices(&self, edge_idx: HexFlatDir) -> impl Iterator<Item = VertexIndex> + '_ {
        self.mesh.anchor_edge(AnchorIndex::new(edge_idx as usize))
    }

    /// Returns VertexIndex at specified hex corner (0..5)
    pub fn boundary_corner_vertex(&self, corner_idx: HexPointyDir) -> VertexIndex {
        // assume anchor points are corresponding to hex corners in correct  order
        self.mesh.anchor_vertex(AnchorIndex::new(corner_idx as usize))
    }
}
