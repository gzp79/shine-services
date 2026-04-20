use crate::{
    indexed::TypedIndex,
    math::{
        geometry::quad_jacobian,
        quadrangulation::{QuadFilter, Quadrangulation},
    },
};
use glam::Vec2;

/// Targeted Laplacian relaxation that only moves vertices of badly-shaped quads.
pub struct QuadRelax {
    quality: f32,
    strength: f32,
    iterations: u32,
    buf: Vec<Vec2>,
}

impl QuadRelax {
    pub fn new(quality: f32, strength: f32, iterations: u32) -> Self {
        Self {
            quality,
            strength,
            iterations,
            buf: Vec::new(),
        }
    }
}

impl QuadFilter for QuadRelax {
    /// Identifies quads below `quality` and relaxes their interior vertices toward
    /// the neighbor average. Iterates until all quads pass or `iterations` is reached.
    fn apply(&mut self, mesh: &mut Quadrangulation) {
        for _ in 0..self.iterations {
            let mut is_bad = vec![false; mesh.vertex_count()];
            let mut any_bad = false;

            for qi in mesh.finite_quad_index_iter() {
                let verts = mesh.quad_vertices(qi);
                let pts: [Vec2; 4] = std::array::from_fn(|i| mesh[verts[i]].position);
                if quad_jacobian(&pts) < self.quality {
                    any_bad = true;
                    for &v in verts {
                        if !mesh.is_boundary_vertex(v) {
                            is_bad[v.into_index()] = true;
                        }
                    }
                }
            }

            if !any_bad {
                break;
            }

            let vertices: Vec<_> = mesh.finite_vertex_index_iter().collect();

            self.buf.clear();
            self.buf.resize(mesh.vertex_count(), Vec2::ZERO);

            // Calculate new positions using current state
            for &vi in &vertices {
                if is_bad[vi.into_index()] {
                    let avg = mesh.average_adjacent_positions(vi);
                    let old = mesh[vi].position;
                    self.buf[vi.into_index()] = old + self.strength * (avg - old);
                } else {
                    self.buf[vi.into_index()] = mesh[vi].position;
                }
            }

            // Apply all new positions
            for vi in vertices {
                mesh[vi].position = self.buf[vi.into_index()];
            }
        }
    }
}
