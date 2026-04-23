use crate::{
    dto::IndexedMesh,
    math::quadrangulation::{QuadIndex, Quadrangulation},
};
use std::collections::HashMap;

/// Extracts dual mesh data (dual vertices, dual polygons) from a Quadrangulation
pub struct DualExtractor<'a> {
    mesh: &'a Quadrangulation,
    offset: glam::Vec2,
    pub quad_map: HashMap<QuadIndex, usize>,
    pub vertices: Vec<f32>,
}

impl<'a> DualExtractor<'a> {
    /// Create a new dual extractor with optional position offset
    pub fn new(mesh: &'a Quadrangulation, offset: glam::Vec2) -> Self {
        Self {
            mesh,
            offset,
            quad_map: HashMap::new(),
            vertices: Vec::new(),
        }
    }

    /// Extract dual vertices (quad centroids) and build quad mapping
    pub fn extract_vertices(&mut self) {
        self.vertices.clear();
        self.quad_map.clear();
        self.vertices.reserve(self.mesh.quad_count() * 2);

        for qi in self.mesh.finite_quad_index_iter() {
            if let Some(center) = self.mesh.dual_p(qi) {
                let p = center + self.offset;
                self.quad_map.insert(qi, self.quad_map.len());
                self.vertices.push(p.x);
                self.vertices.push(p.y);
            }
        }
    }

    /// Extract dual polygons (one per interior primal vertex)
    pub fn extract_polygons(&self) -> (Vec<u32>, Vec<u32>) {
        let mut indices = Vec::new();
        let mut polygon_ranges = Vec::new();

        for vi in self.mesh.finite_vertex_index_iter() {
            // Skip boundary vertices (they touch ghost quads)
            if self.mesh.is_boundary_vertex(vi) {
                continue;
            }

            let start = indices.len() as u32;

            // Collect indices of finite quads around this vertex
            for qv in self.mesh.vertex_ring_ccw(vi) {
                if !self.mesh.is_infinite_quad(qv.quad) {
                    let &dual_idx = self.quad_map.get(&qv.quad).expect("QuadIndex should be in quad_map");
                    indices.push(dual_idx as u32);
                }
            }

            let end = indices.len() as u32;
            polygon_ranges.push(start);
            polygon_ranges.push(end);
        }

        (indices, polygon_ranges)
    }

    /// Build complete dual mesh of the internal (finite) polygons
    #[must_use]
    pub fn build_internal_mesh(mut self) -> IndexedMesh {
        self.extract_vertices();
        let (indices, polygon_ranges) = self.extract_polygons();
        IndexedMesh {
            vertices: self.vertices,
            indices,
            polygon_ranges,
            wire_indices: Vec::new(),
            wire_ranges: Vec::new(),
        }
    }
}

impl Quadrangulation {
    /// Create a dual mesh extractor
    pub fn dual_extractor(&self, offset: glam::Vec2) -> DualExtractor<'_> {
        DualExtractor::new(self, offset)
    }
}
