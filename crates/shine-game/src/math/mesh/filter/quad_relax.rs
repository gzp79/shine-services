use crate::{
    indexed::TypedIndex,
    math::{
        geometry::is_quad_well_shaped,
        mesh::QuadMesh,
    },
};
use super::quad_filter::QuadFilter;
use glam::Vec2;

/// Targeted Laplacian relaxation that only moves vertices of badly-shaped quads.
///
/// [`apply`](QuadFilter::apply) identifies quads below `min_quality`
/// and relaxes their interior vertices toward the neighbor average.
/// Iterates until all quads pass or `max_iterations` is reached.
pub struct QuadRelax {
    min_quality: f32,
    strength: f32,
    max_iterations: u32,
    buf: Vec<Vec2>,
}

impl QuadRelax {
    pub fn new(min_quality: f32, strength: f32, max_iterations: u32) -> Self {
        Self {
            min_quality,
            strength,
            max_iterations,
            buf: Vec::new(),
        }
    }
}

impl QuadFilter for QuadRelax {
    fn apply(&mut self, mesh: &mut QuadMesh) {
        for _ in 0..self.max_iterations {
            let mut is_bad = vec![false; mesh.vertex_count()];
            let mut any_bad = false;

            for qi in mesh.quad_indices() {
                let verts = mesh.quad_vertices(qi);
                let pts: [Vec2; 4] = std::array::from_fn(|i| mesh.position(verts[i]));
                if !is_quad_well_shaped(&pts, self.min_quality) {
                    any_bad = true;
                    for &v in &verts {
                        if !mesh.is_boundary_vertex(v) {
                            is_bad[v.into_index()] = true;
                        }
                    }
                }
            }

            if !any_bad {
                break;
            }

            self.buf.resize(mesh.vertex_count(), Vec2::ZERO);
            for vi in mesh.vertex_indices() {
                self.buf[vi.into_index()] = mesh.position(vi);
            }

            for vi in mesh.vertex_indices() {
                if !is_bad[vi.into_index()] {
                    continue;
                }
                let avg = mesh.topology().neighbor_avg(vi, &self.buf);
                let old = self.buf[vi.into_index()];
                mesh.positions_mut()[vi] = old + self.strength * (avg - old);
            }
        }
    }
}
