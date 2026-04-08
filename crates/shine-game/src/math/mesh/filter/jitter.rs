use crate::math::{
    mesh::{QuadFilter, QuadMesh},
    rand::{StableRng, StableRngExt},
};
use glam::Vec2;

/// Random jitter displacement for [`QuadMesh`] positions.
pub struct Jitter {
    amplitude: f32,
    rng: Box<dyn StableRng>,
}

impl Jitter {
    pub fn new(amplitude: f32, rng: impl StableRng + 'static) -> Self {
        Self { amplitude, rng: Box::new(rng) }
    }
}

impl QuadFilter for Jitter {
    /// Displaces every interior vertex by a random offset scaled by `amplitude`.
    /// Boundary vertices stay fixed.
    fn apply(&mut self, mesh: &mut QuadMesh) {
        let QuadMesh {
            topology, vertices: positions, ..
        } = mesh;

        for vi in topology.vertex_indices() {
            if topology.is_boundary_vertex(vi) {
                continue;
            }

            let pos = positions[vi];
            let dx = self.rng.float_signed() * self.amplitude;
            let dy = self.rng.float_signed() * self.amplitude;
            positions[vi] = pos + Vec2::new(dx, dy);
        }
    }
}
