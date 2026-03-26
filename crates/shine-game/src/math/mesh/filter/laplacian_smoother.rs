use super::quad_filter::QuadFilter;
use crate::{indexed::TypedIndex, math::mesh::QuadMesh};
use glam::Vec2;

/// Laplacian smoothing for [`QuadMesh`].
///
/// [`apply`](QuadFilter::apply) runs `iterations` Jacobi-style relaxation
/// steps, moving interior vertices toward the average of their edge-connected
/// neighbors. Boundary vertices are never moved.
pub struct LaplacianSmoother {
    strength: f32,
    iterations: u32,
    buf: Vec<Vec2>,
}

impl LaplacianSmoother {
    pub fn new(strength: f32, iterations: u32) -> Self {
        debug_assert!((0.0..=1.0).contains(&strength));
        Self {
            strength,
            iterations,
            buf: Vec::new(),
        }
    }

    fn step(&mut self, mesh: &mut QuadMesh) {
        self.buf.resize(mesh.vertex_count(), Vec2::ZERO);

        for vi in mesh.vertex_indices() {
            self.buf[vi.into_index()] = mesh.position(vi);
        }

        for vi in mesh.vertex_indices() {
            if mesh.is_boundary_vertex(vi) {
                continue;
            }
            let avg = mesh.topology().neighbor_avg(vi, &self.buf);
            let old = self.buf[vi.into_index()];
            mesh.positions_mut()[vi] = old + self.strength * (avg - old);
        }
    }
}

impl QuadFilter for LaplacianSmoother {
    fn apply(&mut self, mesh: &mut QuadMesh) {
        for _ in 0..self.iterations {
            self.step(mesh);
        }
    }
}
