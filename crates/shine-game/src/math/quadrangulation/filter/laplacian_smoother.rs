use crate::{
    indexed::TypedIndex,
    math::quadrangulation::{QuadFilter, Quadrangulation},
};
use glam::Vec2;

/// Laplacian smoothing for [`Quadrangulation`].
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

    fn step(&mut self, mesh: &mut Quadrangulation) {
        self.buf.resize(mesh.vertex_count(), Vec2::ZERO);

        let vertices: Vec<_> = mesh.finite_vertex_index_iter().collect();
        for vi in &vertices {
            self.buf[vi.into_index()] = mesh[*vi].position;
        }

        for vi in vertices {
            if mesh.is_boundary_vertex(vi) {
                continue;
            }
            let avg = mesh.neighbor_avg(vi, &self.buf);
            let old = self.buf[vi.into_index()];
            mesh[vi].position = old + self.strength * (avg - old);
        }
    }
}

impl QuadFilter for LaplacianSmoother {
    /// Runs `iterations` Jacobi-style relaxation steps, moving interior vertices toward
    /// the average of their edge-connected neighbors. Boundary vertices are never moved.
    fn apply(&mut self, mesh: &mut Quadrangulation) {
        for _ in 0..self.iterations {
            self.step(mesh);
        }
    }
}
