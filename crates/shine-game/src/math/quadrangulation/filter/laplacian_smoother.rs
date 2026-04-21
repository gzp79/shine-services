use crate::math::quadrangulation::{QuadFilter, Quadrangulation, VertexIndex};
use glam::Vec2;

/// Laplacian smoothing for [`Quadrangulation`].
pub struct LaplacianSmoother {
    strength: f32,
    iterations: u32,
    updates: Vec<(VertexIndex, Vec2)>,
}

impl LaplacianSmoother {
    pub fn new(strength: f32, iterations: u32) -> Self {
        debug_assert!((0.0..=1.0).contains(&strength));
        Self {
            strength,
            iterations,
            updates: Vec::new(),
        }
    }

    fn step(&mut self, mesh: &mut Quadrangulation) {
        self.updates.clear();
        self.updates.reserve(mesh.vertex_count());

        // Calculate new positions using current state
        for vi in mesh.vertex_index_range() {
            let old = mesh[vi].position;

            if vi != mesh.infinite_vertex() && !mesh.is_boundary_vertex(vi) {
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

impl QuadFilter for LaplacianSmoother {
    /// Runs `iterations` Jacobi-style relaxation steps, moving interior vertices toward
    /// the average of their edge-connected neighbors. Boundary vertices are never moved.
    fn apply(&mut self, mesh: &mut Quadrangulation) {
        for _ in 0..self.iterations {
            self.step(mesh);
        }
    }
}
