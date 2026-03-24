use crate::math::{
    mesh::QuadMesh,
    rand::{StableRng, StableRngExt},
};
use super::quad_filter::QuadFilter;
use glam::Vec2;

/// Random jitter displacement for [`QuadMesh`] positions.
///
/// [`apply`](QuadFilter::apply) displaces every interior vertex by a
/// random offset scaled by `amplitude`. Boundary vertices stay fixed.
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
    fn apply(&mut self, mesh: &mut QuadMesh) {
        for vi in mesh.vertex_indices() {
            if mesh.is_boundary_vertex(vi) {
                continue;
            }

            let pos = mesh.position(vi);
            let dx = self.rng.float_signed() * self.amplitude;
            let dy = self.rng.float_signed() * self.amplitude;
            mesh.positions_mut()[vi] = pos + Vec2::new(dx, dy);
        }
    }
}
