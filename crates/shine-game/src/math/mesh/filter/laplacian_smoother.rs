use crate::{
    indexed::TypedIndex,
    math::mesh::{QuadFilter, QuadMesh},
};
use glam::Vec2;

/// Laplacian smoothing for [`QuadMesh`].
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
        let QuadMesh {
            topology, vertices: positions, ..
        } = mesh;

        self.buf.resize(topology.vertex_count(), Vec2::ZERO);

        for vi in topology.vertex_indices() {
            self.buf[vi.into_index()] = positions[vi];
        }

        for vi in topology.vertex_indices() {
            if topology.is_boundary_vertex(vi) {
                continue;
            }
            let avg = topology.neighbor_avg(vi, &self.buf);
            let old = self.buf[vi.into_index()];
            positions[vi] = old + self.strength * (avg - old);
        }
    }
}

impl QuadFilter for LaplacianSmoother {
    /// Runs `iterations` Jacobi-style relaxation steps, moving interior vertices toward
    /// the average of their edge-connected neighbors. Boundary vertices are never moved.
    fn apply(&mut self, mesh: &mut QuadMesh) {
        for _ in 0..self.iterations {
            self.step(mesh);
        }
    }
}
