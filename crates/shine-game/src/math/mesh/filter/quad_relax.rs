use crate::{
    indexed::TypedIndex,
    math::{
        geometry::quad_jacobian,
        mesh::{QuadFilter, QuadMesh},
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
    fn apply(&mut self, mesh: &mut QuadMesh) {
        let QuadMesh {
            topology, vertices: positions, ..
        } = mesh;

        for _ in 0..self.iterations {
            let mut is_bad = vec![false; topology.vertex_count()];
            let mut any_bad = false;

            for qi in topology.quad_indices() {
                let verts = topology.quad_vertices(qi);
                let pts: [Vec2; 4] = std::array::from_fn(|i| positions[verts[i]]);
                if quad_jacobian(&pts) < self.quality {
                    any_bad = true;
                    for &v in &verts {
                        if !topology.is_boundary_vertex(v) {
                            is_bad[v.into_index()] = true;
                        }
                    }
                }
            }

            if !any_bad {
                break;
            }

            self.buf.resize(topology.vertex_count(), Vec2::ZERO);
            for vi in topology.vertex_indices() {
                self.buf[vi.into_index()] = positions[vi];
            }

            for vi in topology.vertex_indices() {
                if !is_bad[vi.into_index()] {
                    continue;
                }
                let avg = topology.neighbor_avg(vi, &self.buf);
                let old = self.buf[vi.into_index()];
                positions[vi] = old + self.strength * (avg - old);
            }
        }
    }
}
