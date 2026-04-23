use crate::{
    dto::IndexedMesh,
    math::quadrangulation::{Quadrangulation, VertexIndex},
};
use std::collections::HashMap;

/// Extracts primal mesh data (vertices, quads, anchor edges) from a Quadrangulation
pub struct PrimalExtractor<'a> {
    mesh: &'a Quadrangulation,
    offset: glam::Vec2,
    pub vert_map: HashMap<VertexIndex, usize>,
    pub vertices: Vec<f32>,
}

impl<'a> PrimalExtractor<'a> {
    /// Create a new primal extractor with optional position offset
    pub fn new(mesh: &'a Quadrangulation, offset: glam::Vec2) -> Self {
        Self {
            mesh,
            offset,
            vert_map: HashMap::new(),
            vertices: Vec::new(),
        }
    }

    /// Extract vertices and build vertex mapping
    pub fn extract_vertices(&mut self) {
        self.vertices.clear();
        self.vert_map.clear();
        self.vertices.reserve(self.mesh.vertex_count() * 2);

        for vi in self.mesh.finite_vertex_index_iter() {
            let p = self.mesh.p(vi) + self.offset;
            self.vert_map.insert(vi, self.vert_map.len());
            self.vertices.push(p.x);
            self.vertices.push(p.y);
        }
    }

    /// Extract quad indices as polygons (returns indices and ranges)
    pub fn extract_quads(&self) -> (Vec<u32>, Vec<u32>) {
        let mut indices = Vec::with_capacity(self.mesh.quad_count() * 4);
        let mut polygon_ranges = Vec::with_capacity(self.mesh.quad_count() * 2);

        for q in self.mesh.finite_quad_iter() {
            let start = indices.len() as u32;
            for v in q.vertices.iter() {
                indices.push(self.vert_map[v] as u32);
            }
            let end = indices.len() as u32;
            polygon_ranges.push(start);
            polygon_ranges.push(end);
        }

        (indices, polygon_ranges)
    }

    /// Extract anchor edges as wire indices (returns wire_indices and ranges)
    pub fn extract_anchors(&self) -> (Vec<u32>, Vec<u32>) {
        let mut wire_indices = Vec::new();
        let mut wire_ranges = Vec::new();

        for ai in self.mesh.anchor_index_iter() {
            let start = wire_indices.len() as u32;
            wire_indices.extend(self.mesh.anchor_edge(ai).map(|vi| self.vert_map[&vi] as u32));
            let end = wire_indices.len() as u32;
            wire_ranges.push(start);
            wire_ranges.push(end);
        }

        (wire_indices, wire_ranges)
    }

    /// Build complete dual mesh of the internal (finite) quads
    #[must_use]
    pub fn build_internal_mesh(mut self) -> IndexedMesh {
        self.extract_vertices();
        let (indices, polygon_ranges) = self.extract_quads();
        IndexedMesh {
            vertices: self.vertices,
            indices,
            polygon_ranges,
            wire_indices: Vec::new(),
            wire_ranges: Vec::new(),
        }
    }

    /// Build complete mesh with quads and anchor wires
    #[must_use]
    pub fn build_internal_mesh_with_anchors(mut self) -> IndexedMesh {
        self.extract_vertices();
        let (indices, polygon_ranges) = self.extract_quads();
        let (wire_indices, wire_ranges) = self.extract_anchors();
        IndexedMesh {
            vertices: self.vertices,
            indices,
            polygon_ranges,
            wire_indices,
            wire_ranges,
        }
    }
}

impl Quadrangulation {
    /// Create a primal mesh extractor
    pub fn primal_extractor(&self, offset: glam::Vec2) -> PrimalExtractor<'_> {
        PrimalExtractor::new(self, offset)
    }
}
