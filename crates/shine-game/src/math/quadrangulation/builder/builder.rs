use crate::{
    indexed::{IdxVec, TypedIndex},
    math::{
        debug::SvgDump,
        quadrangulation::{
            builder::state::BuilderState, AnchorIndex, QuadIndex, Quadrangulation, Rot4Idx, VertexIndex,
        },
    },
};
use rand::{prelude::IndexedRandom, seq::SliceRandom, Rng};
use std::{ops::Index, path::PathBuf};

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
    pub(super) quad: &'a mut Quadrangulation,
    pub(super) state: BuilderState,
}

impl<'a> QuadBuilder<'a> {
    pub fn new(quad: &'a mut Quadrangulation) -> Self {
        Self {
            quad,
            state: BuilderState::new(),
        }
    }

    pub fn quad(&self) -> &Quadrangulation {
        self.quad
    }

    pub fn with_debug<P: Into<PathBuf>>(mut self, verbosity: usize, path: P) -> Self {
        self.state = self.state.with_debug(verbosity, path);
        self
    }

    pub fn dump<F>(&self, verbosity: usize, name: &str, f: F)
    where
        F: FnOnce(&mut SvgDump),
    {
        self.state.dump(verbosity, name, f);
    }

    /// Randomize the internal ordering of vertices and quads.
    /// Returns a map from old indices to new indices.
    ///
    /// Randomizes vertex/quad array order and each vertex's referenced quad,
    /// updating all internal references to maintain topology.
    pub fn randomize<R: Rng>(&mut self, rng: &mut R) -> RandomizationMap {
        let vertex_count = self.quad.vertices.len();
        let quad_count = self.quad.quads.len();

        let mut vertex_permutation: Vec<usize> = (0..vertex_count).collect();
        vertex_permutation.shuffle(rng);

        // Separate finite and infinite quads, shuffle each group independently.
        // Infinite quads must stay after finite quads.
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
        let mut quad_permutation = finite_quads;
        quad_permutation.extend(infinite_quads);

        // Build old→new index maps
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

        // Reorder vertices, randomizing each vertex's starting quad for iteration unpredictability
        let mut new_vertices = IdxVec::with_capacity(vertex_count);
        for &old_idx in &vertex_permutation {
            let vertex = &self.quad.vertices[VertexIndex::new(old_idx)];
            let mut new_vertex_quad = quad_map[vertex.quad];

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

        // Reorder quads, updating all vertex and neighbor references
        let mut new_quads = IdxVec::with_capacity(quad_count);
        for &old_idx in &quad_permutation {
            let quad = &self.quad.quads[QuadIndex::new(old_idx)];

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

        for i in 0..self.quad.anchor_vertices.len() {
            let anchor_idx = AnchorIndex::new(i);
            self.quad.anchor_vertices[anchor_idx] = vertex_map[self.quad.anchor_vertices[anchor_idx]];
        }

        self.quad.infinite_vertex = vertex_map[self.quad.infinite_vertex];
        self.quad.vertices = new_vertices;
        self.quad.quads = new_quads;

        self.state.dump(1, "after_randomize", |svg| {
            svg.add_quad(self.quad, std::iter::empty());
        });

        RandomizationMap { vertex_map, quad_map }
    }

    /// Collect all quads adjacent to a vertex by traversing the vertex ring.
    fn get_adjacent_quads(&self, vi: VertexIndex) -> Vec<QuadIndex> {
        let mut quads = Vec::new();
        let start_quad = self.quad.vertices[vi].quad;
        if !start_quad.is_valid() {
            return quads;
        }

        let mut current = start_quad;
        loop {
            quads.push(current);
            let quad = &self.quad.quads[current];
            let Some(local_idx) = quad.find_vertex(vi) else { break };
            let next = quad.neighbors[local_idx.decrement()];
            if next == start_quad || !next.is_valid() {
                break;
            }
            current = next;
        }

        quads
    }
}

impl Quadrangulation {
    pub fn builder(&mut self) -> QuadBuilder<'_> {
        QuadBuilder::new(self)
    }
}
