use crate::{
    indexed::IdxVec,
    math::{
        geometry::quad_jacobian,
        quadrangulation::{QuadFilter, Quadrangulation, VertexIndex},
    },
};
use glam::Vec2;
use std::array;

/// Targeted Laplacian relaxation that only moves vertices of badly-shaped quads.
pub struct QuadRelax {
    quality: f32,
    strength: f32,
    iterations: u32,
    is_marked: IdxVec<VertexIndex, bool>,
    updates: Vec<(VertexIndex, Vec2)>,
}

impl QuadRelax {
    pub fn new(quality: f32, strength: f32, iterations: u32) -> Self {
        Self {
            quality,
            strength,
            iterations,
            is_marked: IdxVec::new(),
            updates: Vec::new(),
        }
    }
}

impl QuadFilter for QuadRelax {
    /// Identifies quads below `quality` and relaxes their interior vertices toward
    /// the neighbor average. Iterates until all quads pass or `iterations` is reached.
    fn apply(&mut self, mesh: &mut Quadrangulation) {
        for _ in 0..self.iterations {
            self.is_marked.clear();
            self.is_marked.resize(mesh.vertex_count(), false);
            let mut dirty_count = 0;

            // Find all bad quads and mark their interior vertices for relaxation.
            for qi in mesh.finite_quad_index_iter() {
                let verts = mesh.quad_vertices(qi);
                let pts: [Vec2; 4] = array::from_fn(|i| mesh[verts[i]].position);
                if quad_jacobian(&pts) < self.quality {
                    for &v in verts {
                        if mesh.is_finite_vertex(v) && !mesh.is_boundary_vertex(v) {
                            if !self.is_marked[v] {
                                dirty_count += 1;
                            }
                            self.is_marked[v] = true;
                        }
                    }
                }
            }

            log::trace!("QuadRelax iteration: {dirty_count} vertices marked for relaxation");
            if dirty_count == 0 {
                break;
            }

            // Calculate new positions using current state
            self.updates.clear();
            self.updates.reserve(dirty_count);

            for vi in mesh.vertex_index_range() {
                if self.is_marked[vi] {
                    let old = mesh[vi].position;
                    let avg = mesh.average_adjacent_positions(vi);
                    self.updates.push((vi, old + self.strength * (avg - old)));
                }
            }

            // Apply all new positions
            for (vi, new_pos) in self.updates.drain(..) {
                mesh[vi].position = new_pos;
            }
        }
    }
}
