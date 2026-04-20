use crate::math::{
    prng::{StableRng, StableRngExt},
    quadrangulation::{QuadFilter, Quadrangulation},
};
use glam::Vec2;

/// Random jitter displacement for [`Quadrangulation`] positions.
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
    fn apply(&mut self, mesh: &mut Quadrangulation) {
        let vertices: Vec<_> = mesh.finite_vertex_index_iter().collect();
        for vi in vertices {
            if mesh.is_boundary_vertex(vi) {
                continue;
            }

            let pos = mesh[vi].position;
            let dx = self.rng.float_signed() * self.amplitude;
            let dy = self.rng.float_signed() * self.amplitude;
            mesh[vi].position = pos + Vec2::new(dx, dy);
        }
    }
}
