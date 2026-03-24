use crate::{
    indexed::TypedIndex,
    math::{geometry::quad_signed_area, mesh::QuadMesh},
};
use super::quad_filter::QuadFilter;
use glam::Vec2;

/// Energy-based quad mesh relaxation.
///
/// Minimizes a combined energy via gradient descent:
/// - **Area**: each quad's area should match the average (hex area / num quads).
/// - **Shape**: edge lengths should be uniform (quads tend toward squares).
///
/// Only interior vertices are moved. Boundary vertices stay fixed.
/// The step is normalized so `step_size` is a fraction of the target edge length.
pub struct EnergyRelax {
    area_weight: f32,
    shape_weight: f32,
    step_size: f32,
    iterations: u32,
}

impl EnergyRelax {
    pub fn new(area_weight: f32, shape_weight: f32, step_size: f32, iterations: u32) -> Self {
        Self {
            area_weight,
            shape_weight,
            step_size,
            iterations,
        }
    }
}

impl QuadFilter for EnergyRelax {
    fn apply(&mut self, mesh: &mut QuadMesh) {
        let num_quads = mesh.quad_count();
        if num_quads == 0 {
            return;
        }

        // Target area = average real quad area
        let total_area: f32 = mesh
            .quad_indices()
            .map(|qi| {
                let verts = mesh.quad_vertices(qi);
                let pts: [Vec2; 4] = std::array::from_fn(|i| mesh.position(verts[i]));
                quad_signed_area(&pts).abs()
            })
            .sum();
        let target_area = total_area / num_quads as f32;
        let target_edge = target_area.sqrt();
        let target_edge_sq = target_area;

        for _ in 0..self.iterations {
            let mut grad = vec![Vec2::ZERO; mesh.vertex_count()];
            let mut valence = vec![0u32; mesh.vertex_count()];

            for qi in mesh.quad_indices() {
                let verts = mesh.quad_vertices(qi);
                let pts: [Vec2; 4] = std::array::from_fn(|i| mesh.position(verts[i]));

                for k in 0..4 {
                    valence[verts[k].into_index()] += 1;
                }

                // ── Area energy: (A / A_target - 1)² ────────────────────
                let area = quad_signed_area(&pts);
                let area_err = area / target_area - 1.0;
                // ∂A/∂p_k = 0.5 * (p_{k+1}.y - p_{k-1}.y, p_{k-1}.x - p_{k+1}.x)
                // Gradient of normalized energy: 2 * area_err * ∂A/∂p_k / A_target
                // Multiply by target_edge to make the gradient have units of length
                for k in 0..4 {
                    let prev = pts[(k + 3) % 4];
                    let next = pts[(k + 1) % 4];
                    let da = Vec2::new(next.y - prev.y, prev.x - next.x) * 0.5;
                    grad[verts[k].into_index()] += self.area_weight * 2.0 * area_err * da / target_area;
                }

                // ── Shape energy: Σ_edges (L²/L²_target - 1)² ──────────
                for i in 0..4 {
                    let j = (i + 1) % 4;
                    let edge = pts[j] - pts[i];
                    let len_sq = edge.length_squared();
                    let shape_err = len_sq / target_edge_sq - 1.0;
                    // ∂(L²)/∂p_i = -2*edge, ∂(L²)/∂p_j = 2*edge
                    let d = self.shape_weight * 4.0 * shape_err * edge / target_edge_sq;
                    grad[verts[i].into_index()] -= d;
                    grad[verts[j].into_index()] += d;
                }
            }

            // Gradient descent: normalize per-vertex by valence and clamp to target_edge * step_size
            let max_step = target_edge * self.step_size;
            for vi in mesh.vertex_indices() {
                if mesh.is_boundary_vertex(vi) {
                    continue;
                }
                let idx = vi.into_index();
                let mut g = grad[idx];
                if valence[idx] > 0 {
                    g /= valence[idx] as f32;
                }
                let len = g.length();
                if len > max_step {
                    g *= max_step / len;
                }
                mesh.positions_mut()[vi] = mesh.position(vi) - g;
            }
        }
    }
}
