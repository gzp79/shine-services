use crate::{
    indexed::TypedIndex,
    math::quadrangulation::{QuadFilter, QuadMesh},
};
use glam::Vec2;

/// Edge-length and diagonal-length equalization relaxation for [`QuadMesh`].
pub struct VertexRepulsion {
    strength: f32,
    iterations: u32,
    buf: Vec<Vec2>,
}

impl VertexRepulsion {
    pub fn new(strength: f32, iterations: u32) -> Self {
        Self {
            strength,
            iterations,
            buf: Vec::new(),
        }
    }
}

impl QuadFilter for VertexRepulsion {
    fn apply(&mut self, mesh: &mut QuadMesh) {
        let QuadMesh {
            topology, vertices: positions, ..
        } = mesh;

        let n = topology.vertex_count();
        if n == 0 {
            return;
        }

        debug_assert_eq!(n, positions.len());

        for _ in 0..self.iterations {
            // Snapshot positions so all updates read from the frozen state.
            self.buf.resize(n, Vec2::ZERO);
            for vi in topology.vertex_indices() {
                self.buf[vi.into_index()] = positions[vi];
            }

            for vi in topology.vertex_indices() {
                if topology.is_boundary_vertex(vi) {
                    continue;
                }
                let i = vi.into_index();

                // Pass 1: compute separate average lengths for edges (+1) and diagonals (+2).
                // The separate averages are kept so their natural length ratio is preserved rather than mixed.
                let mut sum_edge = 0.0f32;
                let mut count_edge = 0u32;
                let mut sum_diag = 0.0f32;
                let mut count_diag = 0u32;
                for r in topology.vertex_ring_ccw(vi) {
                    let verts = topology.quad_vertices(r.quad);
                    for (offset, sum, count) in [
                        (1usize, &mut sum_edge, &mut count_edge),
                        (2usize, &mut sum_diag, &mut count_diag),
                    ] {
                        let vj = verts[(r.local as usize + offset) % 4];
                        let Some(j) = vj.try_into_index() else { continue };
                        let dist = (self.buf[i] - self.buf[j]).length();
                        if dist < 1e-6 {
                            continue;
                        }
                        *sum += dist;
                        *count += 1;
                    }
                }
                if count_edge == 0 && count_diag == 0 {
                    continue;
                }
                let avg_edge = if count_edge > 0 {
                    sum_edge / count_edge as f32
                } else {
                    0.0
                };
                let avg_diag = if count_diag > 0 {
                    sum_diag / count_diag as f32
                } else {
                    0.0
                };

                // Pass 2: compute ideal position — centroid of points at the respective
                // average distance from each edge and diagonal neighbor.
                let mut ideal_sum = Vec2::ZERO;
                let mut ideal_count = 0u32;
                for r in topology.vertex_ring_ccw(vi) {
                    let verts = topology.quad_vertices(r.quad);
                    for (offset, avg_len) in [(1usize, avg_edge), (2usize, avg_diag)] {
                        if avg_len == 0.0 {
                            continue;
                        }
                        let vj = verts[(r.local as usize + offset) % 4];
                        let Some(j) = vj.try_into_index() else { continue };
                        let delta = self.buf[i] - self.buf[j]; // direction: j → i
                        let dist = delta.length();
                        if dist < 1e-6 {
                            continue;
                        }
                        ideal_sum += self.buf[j] + avg_len * (delta / dist);
                        ideal_count += 1;
                    }
                }
                if ideal_count == 0 {
                    continue;
                }
                let ideal = ideal_sum / ideal_count as f32;
                positions[vi] = self.buf[i] + self.strength * (ideal - self.buf[i]);
            }
        }
    }
}
