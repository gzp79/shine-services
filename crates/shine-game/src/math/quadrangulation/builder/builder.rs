use crate::{
    indexed::{IdxVec, TypedIndex},
    math::quadrangulation::{AnchorIndex, QuadIndex, Quadrangulation, Rot4Idx, VertexIndex},
};
use rand::{prelude::IndexedRandom, seq::SliceRandom, Rng};
use std::ops::Index;

/// Index mappings after randomization
#[derive(Debug, Clone)]
pub struct RandomizationMap {
    /// Maps old vertex index to new vertex index
    pub vertex_map: IdxVec<VertexIndex, VertexIndex>,
    /// Maps old quad index to new quad index
    pub quad_map: IdxVec<QuadIndex, QuadIndex>,
}

impl RandomizationMap {
    pub fn vertex<V: Into<VertexIndex>>(&self, old: V) -> VertexIndex {
        self.vertex_map[old.into()]
    }

    pub fn quad<Q: Into<QuadIndex>>(&self, old: Q) -> QuadIndex {
        self.quad_map[old.into()]
    }
}

impl Index<VertexIndex> for RandomizationMap {
    type Output = VertexIndex;

    #[inline]
    fn index(&self, old: VertexIndex) -> &VertexIndex {
        &self.vertex_map[old]
    }
}

impl Index<QuadIndex> for RandomizationMap {
    type Output = QuadIndex;

    #[inline]
    fn index(&self, old: QuadIndex) -> &QuadIndex {
        &self.quad_map[old]
    }
}

/// Builder for mutating a Quadrangulation
pub struct QuadBuilder<'a> {
    quad: &'a mut Quadrangulation,
}

impl<'a> QuadBuilder<'a> {
    /// Create a new builder for the given quadrangulation
    pub fn new(quad: &'a mut Quadrangulation) -> Self {
        Self { quad }
    }

    /// Randomize the internal ordering of vertices and quads.
    /// Returns a map from old indices to new indices.
    ///
    /// This randomizes:
    /// - Vertex array order
    /// - Quad array order
    /// - The quad referenced by each vertex (to make iteration start less predictable)
    /// - Updates all internal references to maintain topology
    pub fn randomize<R: Rng>(&mut self, rng: &mut R) -> RandomizationMap {
        let vertex_count = self.quad.vertices.len();
        let quad_count = self.quad.quads.len();

        // Create random permutations
        let mut vertex_permutation: Vec<usize> = (0..vertex_count).collect();
        vertex_permutation.shuffle(rng);

        // Separate finite and infinite quads, then shuffle each group separately
        // (infinite quads must come after finite quads)
        let mut finite_quads: Vec<usize> = Vec::new();
        let mut infinite_quads: Vec<usize> = Vec::new();
        for i in 0..quad_count {
            if self.quad.is_infinite_quad(QuadIndex::new(i)) {
                infinite_quads.push(i);
            } else {
                finite_quads.push(i);
            }
        }
        finite_quads.shuffle(rng);
        infinite_quads.shuffle(rng);

        // Combine: finite quads first, then infinite quads
        let mut quad_permutation = finite_quads;
        quad_permutation.extend(infinite_quads);

        // Build index mapping (old -> new)
        let mut vertex_map = IdxVec::with_capacity(vertex_count);
        for (new_idx, &old_idx) in vertex_permutation.iter().enumerate() {
            while vertex_map.len() <= old_idx {
                vertex_map.push(VertexIndex::NONE);
            }
            vertex_map[VertexIndex::new(old_idx)] = VertexIndex::new(new_idx);
        }

        let mut quad_map = IdxVec::with_capacity(quad_count);
        for (new_idx, &old_idx) in quad_permutation.iter().enumerate() {
            while quad_map.len() <= old_idx {
                quad_map.push(QuadIndex::NONE);
            }
            quad_map[QuadIndex::new(old_idx)] = QuadIndex::new(new_idx);
        }

        // Reorder vertices and update their quad references
        let mut new_vertices = IdxVec::with_capacity(vertex_count);
        for &old_idx in &vertex_permutation {
            let vertex = &self.quad.vertices[VertexIndex::new(old_idx)];
            let mut new_vertex_quad = quad_map[vertex.quad];

            // Randomize which adjacent quad this vertex references
            // (making iteration start less predictable)
            if vertex.quad.is_valid() {
                let adjacent_quads = self.get_adjacent_quads(VertexIndex::new(old_idx));
                if !adjacent_quads.is_empty() {
                    let random_quad = *adjacent_quads.choose(rng).unwrap();
                    new_vertex_quad = quad_map[random_quad];
                }
            }

            new_vertices.push(crate::math::quadrangulation::Vertex {
                position: vertex.position,
                quad: new_vertex_quad,
            });
        }

        // Reorder quads and update all references
        let mut new_quads = IdxVec::with_capacity(quad_count);
        for &old_idx in &quad_permutation {
            let quad = &self.quad.quads[QuadIndex::new(old_idx)];

            // Build new vertex and neighbor arrays with updated indices
            let new_vertices_arr = [
                vertex_map[quad.vertices[Rot4Idx::new(0)]],
                vertex_map[quad.vertices[Rot4Idx::new(1)]],
                vertex_map[quad.vertices[Rot4Idx::new(2)]],
                vertex_map[quad.vertices[Rot4Idx::new(3)]],
            ];

            let new_neighbors_arr = [
                quad_map[quad.neighbors[Rot4Idx::new(0)]],
                quad_map[quad.neighbors[Rot4Idx::new(1)]],
                quad_map[quad.neighbors[Rot4Idx::new(2)]],
                quad_map[quad.neighbors[Rot4Idx::new(3)]],
            ];

            new_quads.push(crate::math::quadrangulation::Quad {
                vertices: new_vertices_arr.into(),
                neighbors: new_neighbors_arr.into(),
            });
        }

        // Update anchor vertices
        for i in 0..self.quad.anchor_vertices.len() {
            let anchor_idx = AnchorIndex::new(i);
            self.quad.anchor_vertices[anchor_idx] = vertex_map[self.quad.anchor_vertices[anchor_idx]];
        }

        // Update infinite vertex
        self.quad.infinite_vertex = vertex_map[self.quad.infinite_vertex];

        // Replace with reordered data
        self.quad.vertices = new_vertices;
        self.quad.quads = new_quads;

        RandomizationMap { vertex_map, quad_map }
    }

    /// Helper to get all adjacent quads for a vertex
    fn get_adjacent_quads(&self, vi: VertexIndex) -> Vec<QuadIndex> {
        let mut quads = Vec::new();
        let start_quad = self.quad.vertices[vi].quad;

        if !start_quad.is_valid() {
            return quads;
        }

        // Simple traversal to collect all adjacent quads
        let mut current_quad = start_quad;
        loop {
            quads.push(current_quad);

            // Find next quad by following edges
            let quad = &self.quad.quads[current_quad];
            if let Some(local_idx) = quad.find_vertex(vi) {
                let incoming_edge = local_idx.decrement();
                let next_quad = quad.neighbors[incoming_edge];

                if next_quad == start_quad || !next_quad.is_valid() {
                    break;
                }
                current_quad = next_quad;
            } else {
                break;
            }

            // Safety: prevent infinite loops
            if quads.len() > 1000 {
                break;
            }
        }

        quads
    }
}

impl Quadrangulation {
    /// Create a builder for this quadrangulation
    pub fn builder(&mut self) -> QuadBuilder<'_> {
        QuadBuilder { quad: self }
    }

    pub fn dump(&self) {
        let mut svg = crate::math::debug::SvgDump::new();
        svg.add_default_styles();
        svg.add_quad(self, std::iter::empty());

        // Find workspace root by walking up from CARGO_MANIFEST_DIR
        let output_path = std::env::var("CARGO_MANIFEST_DIR")
            .ok()
            .and_then(|p| {
                let mut path = std::path::PathBuf::from(p);
                // Walk up to workspace root (should have Cargo.toml with [workspace])
                while path.parent().is_some() {
                    path.pop();
                    if path.join("Cargo.toml").exists() {
                        return Some(path.join("temp").join("quad"));
                    }
                }
                None
            })
            .unwrap_or_else(|| std::path::PathBuf::from("temp/quad"));
        std::fs::create_dir_all(&output_path).ok();

        let svg_path = output_path.join("quad_mesh_debug.svg");
        if let Ok(mut file) = std::fs::File::create(&svg_path) {
            svg.write(&mut file).ok();
            log::error!("DEBUG: Quad mesh SVG dumped to: {}", svg_path.display());
        }
    }
}
