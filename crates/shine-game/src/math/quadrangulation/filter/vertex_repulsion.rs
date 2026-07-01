use crate::math::quadrangulation::{QuadFilter, Quadrangulation, VertexIndex};
use glam::Vec2;

/// Edge-length and diagonal-length equalization relaxation for [`Quadrangulation`].
pub struct VertexRepulsion {
    strength: f32,
    iterations: u32,
    updates: Vec<(VertexIndex, Vec2)>,
}

impl VertexRepulsion {
    pub fn new(strength: f32, iterations: u32) -> Self {
        Self {
            strength,
            iterations,
            updates: Vec::new(),
        }
    }
}

impl QuadFilter for VertexRepulsion {
    fn apply(&mut self, mesh: &mut Quadrangulation) {
        for _ in 0..self.iterations {
            self.updates.clear();
            self.updates.reserve(mesh.vertex_count());

            for vi in mesh.vertex_index_range() {
                if vi == mesh.infinite_vertex() || mesh.is_boundary_vertex(vi) {
                    continue;
                }
                let pi = mesh.p(vi);

                // Pass 1: compute separate average lengths for edges (+1) and diagonals (+2).
                // The separate averages are kept so their natural length ratio is preserved rather than mixed.
                let mut sum_edge = 0.0f32;
                let mut count_edge = 0u32;
                let mut sum_diag = 0.0f32;
                let mut count_diag = 0u32;
                for r in mesh.vertex_ring_ccw(vi) {
                    debug_assert!(
                        mesh.is_finite_quad(r.quad),
                        "vertex ring should only contain internal vertices"
                    );
                    let verts = mesh.quad_vertices(r.quad);
                    for (offset, sum, count) in [
                        (1usize, &mut sum_edge, &mut count_edge),
                        (2usize, &mut sum_diag, &mut count_diag),
                    ] {
                        let local_idx: usize = r.local.into();
                        let vj = verts[(local_idx + offset) % 4];
                        let dist = (pi - mesh.p(vj)).length();
                        if dist < 1e-6 {
                            continue;
                        }
                        *sum += dist;
                        *count += 1;
                    }
                }
                if count_edge == 0 && count_diag == 0 {
                    // degenerate case, 0 lenghted edge or diagonal
                    continue;
                }
                let avg_edge = sum_edge / count_edge as f32;
                let avg_diag = sum_diag / count_diag as f32;

                // Pass 2: compute ideal position — centroid of points at the respective
                // average distance from each edge and diagonal neighbor.
                let mut ideal_sum = Vec2::ZERO;
                let mut ideal_count = 0u32;
                for r in mesh.vertex_ring_ccw(vi) {
                    let verts = mesh.quad_vertices(r.quad);
                    for (offset, avg_len) in [(1usize, avg_edge), (2usize, avg_diag)] {
                        if avg_len == 0.0 {
                            continue;
                        }
                        let local_idx: usize = r.local.into();
                        let vj = verts[(local_idx + offset) % 4];
                        let delta = pi - mesh.p(vj); // direction: j → i
                        let dist = delta.length();
                        if dist < 1e-6 {
                            continue;
                        }
                        ideal_sum += mesh.p(vj) + avg_len * (delta / dist);
                        ideal_count += 1;
                    }
                }
                if ideal_count == 0 {
                    continue;
                }
                let ideal = ideal_sum / ideal_count as f32;
                self.updates.push((vi, pi + self.strength * (ideal - pi)));
            }

            // Apply all new positions
            for (vi, new_pos) in self.updates.drain(..) {
                mesh[vi].position = new_pos;
            }
        }
    }
}
